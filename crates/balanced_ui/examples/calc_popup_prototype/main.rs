// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! PROTOTYPE, throwaway: a calculator popup on amount fields, calendar gaps on date fields.
//!
//! Question (ticket #214): what should the calculator popup on amount fields look like and how
//! should it work, and does the date calendar (#91, built) need more than it has? Research is in
//! `docs/research/quicken-date-and-calculator-popups.md` (#360).
//!
//! Applied from earlier decisions: the amount field (#90) evaluates typed `+ - * /` through the
//! real `AmountReader` and previews `= $33.33` / `~ $33.33`; the date field (#91) has a 📅 button,
//! Alt+↓, a one-month calendar that picks on pointer press, and the date keys.
//!
//! Four variants differ in how the calculator is offered:
//!   A  Anchored keypad: display over a 4x5 keypad, tied to the amount field
//!   B  Expression helper: display and a row of operator buttons; digits from the keyboard
//!   C  Calculator window: Tools > Calculator, a movable window with a Paste button
//!   D  Typed only: no popup, as built today
//!
//! Toggles (1-9, 0) switch the triggers, the keyboard flow and the calendar's gaps; see `TOGGLES`.
//!
//! Run:
//!   cargo run -p balanced_ui --example calc_popup_prototype -- \
//!     [--variant a|b|c|d] [--log <dir>/events.jsonl] [--control <dir>/control.txt]
//!
//! Control-file verbs (besides the harness's `prompt` and `spotlight`): `variant <key>`,
//! `toggle <key|number> [on|off]`, `screenshot [label]`, `dock on|off`, `reload`, `quit`,
//! `focus <date|payee|memo|payment|deposit>`, `fill <field> <text>` (set a field's text, as if
//! typed), `calc open [payment|deposit]`, `calc type <text>`, `calc enter`, `calc esc`,
//! `calc equals`, `cal open|close`, `tools calculator`, `goto open|<date>`, `record`.
//!
//! Fixed-seed stub data, English literals, en-US dates and USD. None of this is production code.

#![allow(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::pedantic,
    clippy::too_many_lines,
    clippy::wildcard_enum_match_arm,
    clippy::unwrap_used,
    clippy::str_to_string,
    clippy::let_underscore_must_use,
    clippy::needless_pass_by_ref_mut,
    clippy::use_self,
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::integer_division,
    clippy::shadow_unrelated,
    clippy::string_slice,
    clippy::print_stdout,
    clippy::disallowed_methods,
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::type_complexity,
    clippy::struct_excessive_bools,
    clippy::fn_params_excessive_bools,
    dead_code,
    missing_debug_implementations,
    unreachable_pub
)]

use balanced_i18n::{AmountParseError, AmountReader, AmountStyle, Currency, Localizer};
use balanced_prototype_harness::{Args, Harness};
use eframe::egui;
use egui::{
    Align, Align2, Area, Button, CentralPanel, Color32, Context, Event, Frame, Id, Key, Modifiers,
    Order, Panel, Rect, RichText, Sense, Sides, Stroke, StrokeKind, TextEdit, TextStyle, Ui,
    ViewportBuilder, ViewportCommand, Window, pos2, vec2,
};
use egui_extras::{Column, TableBuilder};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde_json::json;

/// A stand-in for the real `preferences` module, holding only what `theme.rs` reads, so the real
/// theme compiles into the example without the modules the real Preferences pull in.
mod preferences {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub(crate) enum Scheme {
        #[default]
        Light,
        Dark,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub(crate) enum FontFace {
        #[default]
        Egui,
        NotoSans,
        LiberationSans,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub(crate) enum Gridlines {
        #[default]
        Off,
        Horizontal,
        Vertical,
        Both,
    }

    pub(crate) struct Preferences {
        pub(crate) scheme: Scheme,
        pub(crate) font_face: FontFace,
        pub(crate) dense_rows: bool,
        pub(crate) gridlines: Gridlines,
        pub(crate) alternating_rows: bool,
        pub(crate) monospace_money: bool,
        pub(crate) coloured_negatives: bool,
    }

    impl Default for Preferences {
        fn default() -> Self {
            Self {
                scheme: Scheme::Light,
                font_face: FontFace::Egui,
                dense_rows: false,
                gridlines: Gridlines::Off,
                alternating_rows: true,
                monospace_money: false,
                coloured_negatives: true,
            }
        }
    }
}

// The real theme module, compiled into the example: the prototype gets the decided look (#16)
// without making `theme` public.
#[allow(dead_code)]
#[path = "../../src/theme.rs"]
mod theme;

use theme::{Theme, ThemeSettings};

// ---------------------------------------------------------------------------------------------
// Dates and amounts

/// 2026-10-20, as days since 1970-01-01.
const TODAY: i64 = 20_746;
const HISTORY_DAYS: i64 = 420;
const START: i64 = TODAY - HISTORY_DAYS;

/// Howard Hinnant's `civil_from_days`: (year, month, day).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn fmt_day(days: i64) -> String {
    let (y, m, d) = civil(days);
    format!("{m}/{d}/{y}")
}

/// Parses `m/d/yyyy`, `m/d` (this year) or `m/d/yy`.
fn parse_day(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("t") {
        return Some(TODAY);
    }
    let mut parts = s.split(['/', '-', '.']);
    let m: i64 = parts.next()?.trim().parse().ok()?;
    let d: i64 = parts.next()?.trim().parse().ok()?;
    let y: i64 = match parts.next() {
        Some(y) => {
            let y: i64 = y.trim().parse().ok()?;
            if y < 100 { 2000 + y } else { y }
        }
        None => civil(TODAY).0,
    };
    if parts.next().is_some() || !(1900..=2200).contains(&y) {
        return None;
    }
    let days = days_from_civil(y, m, d);
    (civil(days) == (y, m, d)).then_some(days)
}

fn days_in_month(y: i64, m: i64) -> i64 {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    days_from_civil(ny, nm, 1) - days_from_civil(y, m, 1)
}

/// `days` moved by `months`, clamped to the target month's length.
fn add_months(days: i64, months: i64) -> i64 {
    let (y, m, d) = civil(days);
    let index = y * 12 + (m - 1) + months;
    let (ny, nm) = (index.div_euclid(12), index.rem_euclid(12) + 1);
    days_from_civil(ny, nm, d.min(days_in_month(ny, nm)))
}

fn month_name(m: i64) -> &'static str {
    [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ][(m - 1) as usize]
}

/// 0 = Sunday.
fn weekday(days: i64) -> i64 {
    (days + 4).rem_euclid(7)
}

fn fmt_cents(cents: i64) -> String {
    let neg = cents < 0;
    let abs = cents.unsigned_abs();
    let whole = (abs / 100).to_string();
    let mut grouped = String::new();
    for (i, ch) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{}{grouped}.{:02}", if neg { "-" } else { "" }, abs % 100)
}

// ---------------------------------------------------------------------------------------------
// Stub data

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    /// Roughly `typical` cents, +-60%.
    fn around(&mut self, typical: i64) -> i64 {
        let spread = (typical * 6 / 5).max(1) as u64;
        (typical * 2 / 5 + self.below(spread) as i64).max(1)
    }
}

/// (payee, memo, typical cents).
const SPENDING: &[(&str, &str, i64)] = &[
    ("Green Valley Market", "Groceries", 7_800),
    ("Shell", "Fuel", 4_800),
    ("Netflix", "Streaming", 1_599),
    ("Blue Bottle Coffee", "", 650),
    ("City Water & Power", "Utilities", 9_500),
    ("Amazon", "Order", 3_900),
    ("Trattoria Roma", "Dinner", 5_200),
    ("Pharmacy Plus", "", 2_300),
    ("Hardware Depot", "Fence repair", 6_400),
    ("Metro Transit", "Monthly pass", 2_750),
    ("Spotify", "Streaming", 1_099),
    ("Book Nook", "", 2_400),
    ("Corner Bakery", "", 900),
];

const SEED: u64 = 0x00ca_1c01_a707_2026;

struct Row {
    date: i64,
    payee: String,
    memo: String,
    /// Negative is a payment.
    cents: i64,
}

fn generate() -> Vec<Row> {
    let mut rng = Lcg(SEED);
    let mut rows = Vec::new();
    for day in START..=TODAY {
        let (_, _, d) = civil(day);
        if (day - START) % 14 == 3 {
            rows.push(Row {
                date: day,
                payee: "Acme Corp Payroll".to_owned(),
                memo: "Paycheck".to_owned(),
                cents: 185_037,
            });
        }
        if d == 1 {
            rows.push(Row {
                date: day,
                payee: "Maple Court Apartments".to_owned(),
                memo: "Rent".to_owned(),
                cents: -145_000,
            });
        }
        if d == 15 {
            rows.push(Row {
                date: day,
                payee: "Visa Rewards".to_owned(),
                memo: "Card payment".to_owned(),
                cents: -rng.around(110_000),
            });
        }
        for _ in 0..rng.below(4) {
            if rng.below(100) < 55 {
                let (payee, memo, typical) = SPENDING[rng.below(SPENDING.len() as u64) as usize];
                rows.push(Row {
                    date: day,
                    payee: payee.to_owned(),
                    memo: memo.to_owned(),
                    cents: -rng.around(typical),
                });
            }
        }
    }
    rows
}

fn balances(rows: &[Row]) -> Vec<i64> {
    let mut running = 420_000;
    rows.iter()
        .map(|r| {
            running += r.cents;
            running
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Variants and toggles

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Variant {
    A,
    B,
    C,
    D,
}

const VARIANT_KEYS: [(Key, &str); 4] = [(Key::A, "A"), (Key::B, "B"), (Key::C, "C"), (Key::D, "D")];

impl Variant {
    const ALL: [Self; 4] = [Self::A, Self::B, Self::C, Self::D];

    fn key(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
            Self::D => "d",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|v| v.key().eq_ignore_ascii_case(key))
    }

    fn label(self) -> &'static str {
        match self {
            Self::A => "A · Anchored keypad",
            Self::B => "B · Expression helper",
            Self::C => "C · Calculator window",
            Self::D => "D · Typed only (as built)",
        }
    }

    fn step(self, delta: isize) -> Self {
        let n = Self::ALL.len() as isize;
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0) as isize;
        Self::ALL[(i + delta).rem_euclid(n) as usize]
    }

    /// A and B open a popup tied to the field.
    fn field_popup(self) -> bool {
        matches!(self, Self::A | Self::B)
    }
}

const BUTTON: usize = 0;
const ALT_DOWN: usize = 1;
const OPERATOR: usize = 2;
const FROM_FIELD: usize = 3;
const OUTSIDE_COMMITS: usize = 4;
const FOCUS_NEXT: usize = 5;
const ROUNDED_TILDE: usize = 6;
const TODAY_BUTTON: usize = 7;
const CAL_KEYS: usize = 8;
const GO_TO_DATE: usize = 9;

/// (key, label, applies to, starting value).
const TOGGLES: &[(&str, &str, &str, bool)] = &[
    (
        "button",
        "Calculator button beside each amount field",
        "A B",
        true,
    ),
    ("alt_down", "Alt+Down opens the calculator", "A B", true),
    (
        "operator",
        "Typing + * / (or − after a number) opens it",
        "A B",
        false,
    ),
    (
        "from_field",
        "Starts from the field's number (off: from 0)",
        "A B C",
        true,
    ),
    (
        "outside",
        "Click outside commits the result (off: cancels)",
        "A B",
        false,
    ),
    (
        "focus_next",
        "After Enter, focus moves to the next field",
        "A B",
        false,
    ),
    ("tilde", "A rounded result is marked ~", "A B C", true),
    ("today", "Calendar: Today button", "all", false),
    (
        "cal_keys",
        "Calendar: Left/Right day, Up/Down week, Enter picks, Esc closes",
        "all",
        false,
    ),
    ("goto", "Ctrl+G opens Go to date", "all", false),
];

const TOGGLE_KEYS: [(Key, &str); 10] = [
    (Key::Num1, "1"),
    (Key::Num2, "2"),
    (Key::Num3, "3"),
    (Key::Num4, "4"),
    (Key::Num5, "5"),
    (Key::Num6, "6"),
    (Key::Num7, "7"),
    (Key::Num8, "8"),
    (Key::Num9, "9"),
    (Key::Num0, "0"),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    Payment,
    Deposit,
}

impl Field {
    fn name(self) -> &'static str {
        match self {
            Self::Payment => "payment",
            Self::Deposit => "deposit",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Payment => "Payment",
            Self::Deposit => "Deposit",
        }
    }

    fn id(self) -> Id {
        Id::new(("amount_field", self.name()))
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "payment" => Some(Self::Payment),
            "deposit" => Some(Self::Deposit),
            _ => None,
        }
    }
}

const DATE_ID: &str = "date_field";
const AMOUNT_W: f32 = 110.0;

fn id_of(name: &str) -> Id {
    Id::new(("entry", name))
}

// ---------------------------------------------------------------------------------------------
// The app

/// The field popup (variants A and B): it owns its expression.
struct Popup {
    field: Field,
    expr: String,
    /// Take keyboard focus and put the cursor at the end on the next frame.
    focus: bool,
    via: &'static str,
}

/// The floating Tools > Calculator window (variant C).
struct CalcWindow {
    expr: String,
    focus: bool,
}

struct Proto {
    h: Harness,
    variant: Variant,
    o: [bool; 10],
    theme: Theme,
    dock_open: bool,
    status: String,

    loc: Localizer,
    usd: Currency,
    reader: AmountReader,

    rows: Vec<Row>,
    balances: Vec<i64>,
    scroll_to: Option<usize>,

    // The entry row.
    date: i64,
    date_text: String,
    date_dirty: bool,
    payee: String,
    memo: String,
    payment: String,
    deposit: String,
    entry_error: Option<String>,

    popup: Option<Popup>,
    window: Option<CalcWindow>,
    last_amount: Field,
    rects: [Rect; 2],
    calc_button_rects: [Rect; 2],
    popup_rect: Rect,

    cal_open: bool,
    cal_view: (i64, i64),
    cal_rect: Rect,
    date_rect: Rect,
    cal_button_rect: Rect,

    goto: Option<String>,
    /// While Go to date is open, the date field machinery drives its box, and the entry
    /// row's date waits here: date, text, dirty, field rect, 📅 rect.
    parked: Option<(i64, String, bool, Rect, Rect)>,
    /// The register row last clicked or gone to; Go to date starts from its date.
    selected_row: Option<usize>,
    focus_request: Option<(Id, bool)>,
}

impl Proto {
    fn new(args: &Args) -> Self {
        let variant = args
            .variant
            .as_deref()
            .and_then(Variant::from_key)
            .unwrap_or(Variant::A);
        let mut h = Harness::new(args);
        h.event("variant", json!({ "to": variant.key(), "via": "start" }));
        let mut o = [false; 10];
        for (i, t) in TOGGLES.iter().enumerate() {
            o[i] = t.3;
        }
        let loc = Localizer::for_tests();
        let usd = Currency::new("USD", 2).expect("USD");
        let reader = loc.amount_reader(usd);
        let rows = generate();
        let bal = balances(&rows);
        let (y, m, _) = civil(TODAY);
        Self {
            h,
            variant,
            o,
            theme: Theme::new(ThemeSettings::default()),
            dock_open: true,
            status: "Ready".to_owned(),
            loc,
            usd,
            reader,
            rows,
            balances: bal,
            scroll_to: None,
            date: TODAY,
            date_text: fmt_day(TODAY),
            date_dirty: false,
            payee: String::new(),
            memo: String::new(),
            payment: String::new(),
            deposit: String::new(),
            entry_error: None,
            popup: None,
            window: None,
            last_amount: Field::Payment,
            rects: [Rect::NOTHING; 2],
            calc_button_rects: [Rect::NOTHING; 2],
            popup_rect: Rect::NOTHING,
            cal_open: false,
            cal_view: (y, m),
            cal_rect: Rect::NOTHING,
            date_rect: Rect::NOTHING,
            cal_button_rect: Rect::NOTHING,
            goto: None,
            parked: None,
            selected_row: None,
            focus_request: None,
        }
    }

    fn emit(&mut self, command: &str, source: &str) {
        self.h.event(
            "command",
            json!({ "would_emit": command, "source": source }),
        );
        self.status = format!("{source}: would emit {command}");
    }

    fn set_variant(&mut self, to: Variant, via: &str) {
        if to == self.variant {
            return;
        }
        self.h.event(
            "variant",
            json!({ "from": self.variant.key(), "to": to.key(), "via": via }),
        );
        self.variant = to;
        if !to.field_popup() {
            self.popup = None;
        }
        if to != Variant::C {
            self.window = None;
        }
        self.h.screenshot(format!("variant-{}", to.key()));
    }

    fn set_toggle(&mut self, key: &str, value: Option<bool>, via: &str) {
        let index = key
            .parse::<usize>()
            .ok()
            .map(|n| if n == 0 { 9 } else { n - 1 })
            .filter(|i| *i < TOGGLES.len())
            .or_else(|| TOGGLES.iter().position(|t| t.0 == key));
        let Some(i) = index else {
            self.h.event("control_unknown", json!({ "toggle": key }));
            return;
        };
        let new = value.unwrap_or(!self.o[i]);
        if self.o[i] == new && via != "click" {
            return;
        }
        self.o[i] = new;
        self.h.event(
            "toggle",
            json!({ "key": TOGGLES[i].0, "value": new, "via": via, "applies_to": TOGGLES[i].2 }),
        );
        self.h.screenshot(format!(
            "toggle-{}-{}",
            TOGGLES[i].0,
            if new { "on" } else { "off" }
        ));
    }

    fn set_dock(&mut self, open: bool, via: &str) {
        self.dock_open = open;
        self.h.event("dock", json!({ "open": open, "via": via }));
    }

    fn text_of(&self, field: Field) -> &String {
        match field {
            Field::Payment => &self.payment,
            Field::Deposit => &self.deposit,
        }
    }

    fn text_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::Payment => &mut self.payment,
            Field::Deposit => &mut self.deposit,
        }
    }

    // -----------------------------------------------------------------------------------------
    // Control

    fn apply_control(&mut self, ctx: &Context, cmd: &str) {
        let (verb, rest) = cmd
            .split_once(' ')
            .map_or((cmd, ""), |(v, r)| (v, r.trim()));
        let bad = |s: &mut Self| s.h.event("control_unknown", json!({ "command": cmd }));
        match verb {
            "variant" => match Variant::from_key(rest) {
                Some(v) => self.set_variant(v, "control"),
                None => bad(self),
            },
            "toggle" => {
                let (key, value) = rest
                    .split_once(' ')
                    .map_or((rest, None), |(k, v)| (k, Some(v.trim() == "on")));
                self.set_toggle(key, value, "control");
            }
            "focus" => {
                let id = match rest {
                    "date" => Some(Id::new(DATE_ID)),
                    "payee" | "memo" => Some(id_of(rest)),
                    other => Field::from_name(other).map(Field::id),
                };
                match id {
                    Some(id) => self.focus_request = Some((id, true)),
                    None => bad(self),
                }
            }
            "fill" => {
                let (field, text) = rest.split_once(' ').unwrap_or((rest, ""));
                match field {
                    "date" => {
                        self.date_text = text.to_owned();
                        self.date_dirty = true;
                        self.commit_date_text("control");
                    }
                    "payee" => self.payee = text.to_owned(),
                    "memo" => self.memo = text.to_owned(),
                    other => match Field::from_name(other) {
                        Some(f) => {
                            *self.text_mut(f) = text.to_owned();
                            self.last_amount = f;
                        }
                        None => bad(self),
                    },
                }
            }
            "calc" => {
                let (sub, arg) = rest.split_once(' ').unwrap_or((rest, ""));
                match sub {
                    "open" => {
                        let field = Field::from_name(arg).unwrap_or(Field::Payment);
                        self.open_calculator(field, "control");
                    }
                    "type" => {
                        if let Some(p) = &mut self.popup {
                            p.expr.push_str(arg);
                            p.focus = true;
                        } else if let Some(w) = &mut self.window {
                            w.expr.push_str(arg);
                            w.focus = true;
                        } else {
                            bad(self);
                        }
                    }
                    "enter" => self.popup_commit("control"),
                    "esc" => self.popup_cancel("control"),
                    "equals" => self.equals(),
                    _ => bad(self),
                }
            }
            "cal" => match rest {
                "open" => self.open_calendar("control"),
                "close" => self.close_calendar("control"),
                _ => bad(self),
            },
            "tools" => {
                if rest == "calculator" {
                    self.open_window("control");
                } else {
                    bad(self);
                }
            }
            "goto" => {
                if rest == "open" {
                    self.open_goto("control");
                } else if let Some(d) = parse_day(rest) {
                    self.go_to_date(d, "control");
                } else {
                    bad(self);
                }
            }
            "record" => self.record("control"),
            "reload" => {
                self.rows = generate();
                self.balances = balances(&self.rows);
                self.payment.clear();
                self.deposit.clear();
                self.payee.clear();
                self.memo.clear();
                self.popup = None;
                self.window = None;
                self.close_calendar("reload");
                self.set_date(TODAY, "reload");
                self.h.event("reload", json!({ "seed": SEED }));
            }
            "screenshot" => self
                .h
                .screenshot(if rest.is_empty() { "control" } else { rest }),
            "dock" => self.set_dock(rest != "off", "control"),
            "quit" => {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            _ => bad(self),
        }
    }

    // -----------------------------------------------------------------------------------------
    // The date field and its calendar

    /// The focused date field: the entry row's, or the Go to date box's while it is open.
    fn date_id(&self) -> Id {
        if self.parked.is_some() {
            Id::new("goto_date")
        } else {
            Id::new(DATE_ID)
        }
    }

    /// The date text box and its 📅 button, for whichever field is active (`date_id`).
    fn date_widget(&mut self, ui: &mut Ui, ctx: &Context, name: &str) {
        ui.spacing_mut().item_spacing.x = 4.0;
        let id = self.date_id();
        let focused = ctx.memory(|m| m.has_focus(id));
        if focused {
            self.date_input(ctx);
        }
        if !self.date_dirty {
            self.date_text = fmt_day(self.date);
        }
        let r = ui.add(
            TextEdit::singleline(&mut self.date_text)
                .id(id)
                .desired_width(92.0)
                .event_filter(egui::EventFilter {
                    escape: true,
                    ..Default::default()
                }),
        );
        self.h.track(&r, format!("{name} field"));
        self.date_rect = r.rect;
        if r.changed() {
            self.date_dirty = true;
        }
        if r.lost_focus() && self.date_dirty && !self.cal_open {
            self.commit_date_text("blur");
        }
        if r.lost_focus()
            && self.date_dirty
            && self.cal_open
            && !ctx.input(|i| i.pointer.any_down())
        {
            self.commit_date_text("blur");
        }
        let b = ui.add(Button::new("📅").min_size(vec2(26.0, 0.0)));
        self.h.track(&b, format!("{name} calendar button"));
        self.cal_button_rect = b.rect;
        if b.clicked() {
            if self.cal_open {
                self.close_calendar("button");
            } else {
                self.open_calendar("button");
            }
        }
    }

    /// Opens Go to date on the selected row's date (today when none is selected), in a box
    /// that is the date field itself: date keys, 📅, the calendar.
    fn open_goto(&mut self, via: &str) {
        if self.parked.is_some() {
            return;
        }
        self.close_calendar("Go to date");
        let seed = self
            .selected_row
            .and_then(|i| self.rows.get(i))
            .map_or(TODAY, |r| r.date);
        self.parked = Some((
            self.date,
            std::mem::take(&mut self.date_text),
            self.date_dirty,
            self.date_rect,
            self.cal_button_rect,
        ));
        self.date = seed;
        self.date_text = fmt_day(seed);
        self.date_dirty = false;
        self.goto = Some(String::new());
        self.focus_request = Some((self.date_id(), true));
        self.h.event(
            "goto",
            json!({ "open": true, "via": via, "seed": fmt_day(seed) }),
        );
    }

    /// Closes Go to date and gives the date field back to the entry row.
    fn close_goto(&mut self) {
        self.close_calendar("Go to date closed");
        self.goto = None;
        if let Some((date, text, dirty, rect, button)) = self.parked.take() {
            self.date = date;
            self.date_text = text;
            self.date_dirty = dirty;
            self.date_rect = rect;
            self.cal_button_rect = button;
        }
    }

    fn set_date(&mut self, date: i64, via: &str) {
        if self.date != date {
            self.h.event(
                "date",
                json!({ "from": fmt_day(self.date), "to": fmt_day(date), "via": via }),
            );
        }
        self.date = date;
        self.date_text = fmt_day(date);
        self.date_dirty = false;
        let (y, m, _) = civil(date);
        self.cal_view = (y, m);
    }

    fn commit_date_text(&mut self, via: &str) {
        if let Some(d) = parse_day(&self.date_text) {
            self.set_date(d, via);
        } else {
            self.entry_error = Some(format!("“{}” is not a date (m/d/yyyy)", self.date_text));
            self.date_text = fmt_day(self.date);
            self.date_dirty = false;
        }
    }

    fn open_calendar(&mut self, via: &str) {
        if self.cal_open {
            return;
        }
        self.cal_open = true;
        let (y, m, _) = civil(self.date);
        self.cal_view = (y, m);
        self.h
            .event("calendar", json!({ "open": true, "via": via }));
        self.focus_request = Some((self.date_id(), false));
    }

    fn close_calendar(&mut self, via: &str) {
        if !self.cal_open {
            return;
        }
        self.cal_open = false;
        self.h
            .event("calendar", json!({ "open": false, "via": via }));
    }

    /// Keys that act on the date field while it has focus.
    fn date_input(&mut self, ctx: &Context) {
        if ctx.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowDown)) {
            if self.cal_open {
                self.close_calendar("Alt+↓");
            } else {
                self.open_calendar("Alt+↓");
            }
            return;
        }
        if self.cal_open && self.o[CAL_KEYS] {
            let step =
                |ctx: &Context, key: Key| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key));
            let mut moved = None;
            if step(ctx, Key::ArrowLeft) {
                moved = Some(self.date - 1);
            } else if step(ctx, Key::ArrowRight) {
                moved = Some(self.date + 1);
            } else if step(ctx, Key::ArrowUp) {
                moved = Some(self.date - 7);
            } else if step(ctx, Key::ArrowDown) {
                moved = Some(self.date + 7);
            } else if step(ctx, Key::PageUp) {
                moved = Some(add_months(self.date, -1));
            } else if step(ctx, Key::PageDown) {
                moved = Some(add_months(self.date, 1));
            }
            if let Some(d) = moved {
                self.set_date(d, "calendar key");
                self.focus_request = Some((self.date_id(), true));
                return;
            }
            if step(ctx, Key::Enter) {
                self.close_calendar("Enter");
                return;
            }
            if step(ctx, Key::Escape) {
                self.close_calendar("Esc");
                return;
            }
        }
        if self.date_dirty {
            return;
        }
        let step = |ctx: &Context, key: Key| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key));
        let mut moved = None;
        if step(ctx, Key::ArrowUp) {
            moved = Some(self.date + 1);
        } else if step(ctx, Key::ArrowDown) {
            moved = Some(self.date - 1);
        } else if step(ctx, Key::PageUp) {
            moved = Some(add_months(self.date, -1));
        } else if step(ctx, Key::PageDown) {
            moved = Some(add_months(self.date, 1));
        }
        let typed = ctx.input_mut(|i| {
            let mut found = None;
            i.events.retain(|ev| {
                if found.is_none()
                    && let Event::Text(t) = ev
                    && matches!(
                        t.as_str(),
                        "t" | "T" | "m" | "M" | "h" | "H" | "y" | "Y" | "r" | "R" | "+" | "-" | "="
                    )
                {
                    found = t.chars().next();
                    return false;
                }
                true
            });
            found
        });
        if let Some(c) = typed {
            let (y, m, _) = civil(self.date);
            moved = Some(match c.to_ascii_lowercase() {
                't' => TODAY,
                '+' | '=' => self.date + 1,
                '-' => self.date - 1,
                'm' => days_from_civil(y, m, 1),
                'h' => days_from_civil(y, m, days_in_month(y, m)),
                'y' => days_from_civil(y, 1, 1),
                _ => days_from_civil(y, 12, 31),
            });
        }
        if let Some(d) = moved {
            self.set_date(d, "date key");
            self.focus_request = Some((self.date_id(), true));
        }
    }

    fn calendar_popup(&mut self, ctx: &Context) {
        if !self.cal_open {
            return;
        }
        let anchor = self.date_rect.left_top() - vec2(0.0, 4.0);
        let mut pick = None;
        let mut close = None;
        let out = Area::new(Id::new("calendar_popup"))
            .order(Order::Foreground)
            .pivot(Align2::LEFT_BOTTOM)
            .fixed_pos(anchor)
            .show(ctx, |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    let (y, m) = self.cal_view;
                    // Drawn as the built calendar draws it (`date_field.rs`, #91): square days
                    // one row high, flat, the selected day filled and today outlined, the title
                    // spanning the five day columns between ◀ and ▶.
                    let d = self.theme.row_height();
                    let size = vec2(d, d);
                    let gap = ui.spacing().item_spacing.x;
                    let cal_w = 7.0 * d + 6.0 * gap;
                    ui.horizontal(|ui| {
                        let prev = flat_cell(ui, size, "◀", false, false);
                        self.h.track(&prev, "calendar: previous month");
                        ui.add_sized(
                            vec2(5.0 * d + 4.0 * gap, d),
                            egui::Label::new(format!("{} {y}", month_name(m))),
                        );
                        let next = flat_cell(ui, size, "▶", false, false);
                        self.h.track(&next, "calendar: next month");
                        let delta = if prev.is_pointer_button_down_on() {
                            Some(-1)
                        } else if next.is_pointer_button_down_on() {
                            Some(1)
                        } else {
                            None
                        };
                        if let Some(delta) = delta {
                            if ui.input(|i| i.pointer.any_pressed()) {
                                let idx = y * 12 + (m - 1) + delta;
                                self.cal_view = (idx.div_euclid(12), idx.rem_euclid(12) + 1);
                            }
                        }
                    });
                    let first = days_from_civil(y, m, 1);
                    let lead = weekday(first);
                    let n = days_in_month(y, m);
                    ui.horizontal(|ui| {
                        for w in ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"] {
                            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                            ui.painter().text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                w,
                                TextStyle::Small.resolve(ui.style()),
                                ui.visuals().weak_text_color(),
                            );
                        }
                    });
                    // Only the weeks this month touches.
                    let weeks = (lead + n + 6) / 7;
                    for week in 0..weeks {
                        ui.horizontal(|ui| {
                            for col in 0..7 {
                                let day = week * 7 + col - lead + 1;
                                if (1..=n).contains(&day) {
                                    let date = first + day - 1;
                                    let r = flat_cell(
                                        ui,
                                        size,
                                        &day.to_string(),
                                        date == self.date,
                                        date == TODAY,
                                    );
                                    self.h.track(&r, format!("calendar day {day}"));
                                    if r.is_pointer_button_down_on() {
                                        pick = Some(date);
                                    }
                                } else {
                                    ui.allocate_exact_size(size, Sense::hover());
                                }
                            }
                        });
                    }
                    if self.o[TODAY_BUTTON] {
                        ui.separator();
                        let r = ui.add_sized(vec2(cal_w, d), Button::new("Today").frame(false));
                        self.h.track(&r, "calendar: Today");
                        if r.is_pointer_button_down_on() {
                            pick = Some(TODAY);
                            close = Some("Today button");
                        }
                    }
                    if self.o[CAL_KEYS] {
                        ui.label(
                            RichText::new("Left/Right day · Up/Down week · PgUp/PgDn month · Enter · Esc")
                                .small()
                                .weak(),
                        );
                    }
                });
            });
        self.cal_rect = out.response.rect;
        self.h.region(self.cal_rect, "calendar popup");
        if let Some(date) = pick {
            self.set_date(date, "calendar click");
            self.emit(
                &format!("SetDate({})", fmt_day(date)),
                "calendar day pressed",
            );
            if close.is_none() {
                close = Some("day pressed");
            }
            self.focus_request = Some((self.date_id(), true));
        }
        // A press anywhere else closes it. A press on the popup keeps the date field focused.
        let (pressed, down, pos) = ctx.input(|i| {
            (
                i.pointer.any_pressed(),
                i.pointer.any_down(),
                i.pointer.interact_pos(),
            )
        });
        if let Some(pos) = pos {
            let inside = self.cal_rect.contains(pos);
            if pressed
                && !inside
                && !self.date_rect.contains(pos)
                && !self.cal_button_rect.contains(pos)
            {
                close = Some("click outside");
            }
            if down && inside {
                self.focus_request = Some((self.date_id(), false));
            }
        }
        if let Some(via) = close {
            self.close_calendar(via);
        }
    }

    fn go_to_date(&mut self, date: i64, via: &str) {
        let at = self
            .rows
            .iter()
            .position(|r| r.date >= date)
            .unwrap_or(self.rows.len().saturating_sub(1));
        self.scroll_to = Some(at);
        self.selected_row = Some(at);
        self.emit(&format!("GoToDate({})", fmt_day(date)), via);
        self.status = format!(
            "Went to {}: row {} of {}",
            fmt_day(self.rows[at].date),
            at + 1,
            self.rows.len()
        );
    }

    fn goto_window(&mut self, ctx: &Context) {
        if self.goto.is_none() {
            return;
        }
        let mut done = false;
        let mut go = false;
        Window::new("Go to date")
            .id(Id::new("goto_window"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, -80.0))
            .auto_sized()
            .show(ctx, |ui| {
                ui.horizontal(|ui| self.date_widget(ui, ctx, "go to date"));
                // The calendar takes Enter and Esc while open; otherwise they act on the box.
                let focused = ctx.memory(|m| m.has_focus(self.date_id()));
                if focused && ctx.input(|i| i.key_pressed(Key::Enter)) {
                    go = true;
                }
                if !self.cal_open && ctx.input(|i| i.key_pressed(Key::Escape)) {
                    done = true;
                }
                ui.horizontal(|ui| {
                    let g = ui.button("Go");
                    self.h.track(&g, "go to date: Go");
                    if g.clicked() {
                        go = true;
                    }
                    let c = ui.button("Cancel");
                    self.h.track(&c, "go to date: Cancel");
                    if c.clicked() {
                        done = true;
                    }
                });
            });
        if go {
            if self.date_dirty {
                self.commit_date_text("Go");
            }
            if !self.date_dirty {
                let d = self.date;
                self.close_goto();
                self.go_to_date(d, "Go to date box");
                return;
            }
        }
        if done {
            self.h.event("goto", json!({ "open": false, "via": "cancel" }));
            self.close_goto();
        }
    }

    // -----------------------------------------------------------------------------------------
    // Amount fields and the calculator

    fn open_calculator(&mut self, field: Field, via: &'static str) {
        if !self.variant.field_popup() {
            if self.variant == Variant::C {
                self.open_window(via);
            }
            return;
        }
        let seed = if self.o[FROM_FIELD] {
            self.text_of(field).trim().to_owned()
        } else {
            String::new()
        };
        self.start_popup(field, seed, via);
    }

    fn start_popup(&mut self, field: Field, expr: String, via: &'static str) {
        self.close_calendar("calculator opened");
        self.last_amount = field;
        self.popup = Some(Popup {
            field,
            expr: expr.clone(),
            focus: true,
            via,
        });
        self.h.event(
            "calculator",
            json!({ "open": true, "field": field.name(), "via": via, "starts_with": expr }),
        );
    }

    fn open_window(&mut self, via: &str) {
        if self.window.is_none() {
            self.window = Some(CalcWindow {
                expr: String::new(),
                focus: true,
            });
        }
        self.h
            .event("calculator_window", json!({ "open": true, "via": via }));
    }

    /// The text the field gets for `expr`, when `expr` reads as an amount.
    fn result_of(&self, expr: &str) -> Result<(String, bool), AmountParseError> {
        let c = self.reader.read(expr.trim())?;
        let text = self
            .loc
            .format_amount(c.amount(), self.usd, AmountStyle::Bare);
        Ok((text, c.rounded()))
    }

    fn popup_commit(&mut self, via: &str) {
        let Some(p) = self.popup.take() else {
            return;
        };
        if p.expr.trim().is_empty() {
            self.h.event(
                "calculator",
                json!({ "open": false, "field": p.field.name(), "outcome": "empty", "via": via }),
            );
            self.after_popup(p.field, false);
            return;
        }
        match self.result_of(&p.expr) {
            Ok((text, rounded)) => {
                *self.text_mut(p.field) = text.clone();
                self.entry_error = None;
                self.h.event(
                    "calculator",
                    json!({ "open": false, "field": p.field.name(), "outcome": "committed",
                            "expression": p.expr, "result": text, "rounded": rounded, "via": via }),
                );
                self.emit(
                    &format!("SetAmount({}, {text})", p.field.name()),
                    "calculator hand-back",
                );
                self.after_popup(p.field, true);
            }
            Err(_) => {
                // Stay open: the display shows why.
                self.h.event(
                    "calculator",
                    json!({ "field": p.field.name(), "outcome": "refused", "expression": p.expr, "via": via }),
                );
                self.popup = Some(Popup { focus: true, ..p });
            }
        }
    }

    fn popup_cancel(&mut self, via: &str) {
        let Some(p) = self.popup.take() else {
            return;
        };
        self.h.event(
            "calculator",
            json!({ "open": false, "field": p.field.name(), "outcome": "cancelled",
                    "expression": p.expr, "via": via }),
        );
        self.after_popup(p.field, false);
    }

    /// Where focus goes when the popup closes.
    fn after_popup(&mut self, field: Field, committed: bool) {
        let next = if committed && self.o[FOCUS_NEXT] {
            match field {
                Field::Payment => Field::Deposit.id(),
                Field::Deposit => id_of("enter"),
            }
        } else {
            field.id()
        };
        self.focus_request = Some((next, true));
    }

    fn equals(&mut self) {
        let result = if let Some(p) = &self.popup {
            Some(p.expr.clone())
        } else {
            self.window.as_ref().map(|w| w.expr.clone())
        };
        let Some(expr) = result else {
            return;
        };
        if let Ok((text, _)) = self.result_of(&expr) {
            self.h.event(
                "calculator",
                json!({ "outcome": "equals", "expression": expr, "result": text }),
            );
            if let Some(p) = &mut self.popup {
                p.expr = text;
                p.focus = true;
            } else if let Some(w) = &mut self.window {
                w.expr = text;
                w.focus = true;
            }
        }
    }

    /// The result line under a calculator display.
    fn result_line(&self, expr: &str) -> (String, bool) {
        if expr.trim().is_empty() {
            return (String::new(), false);
        }
        match self.reader.read(expr.trim()) {
            Ok(c) => {
                let amount = self
                    .loc
                    .format_amount(c.amount(), self.usd, AmountStyle::Symbol);
                let mark = if c.rounded() && self.o[ROUNDED_TILDE] {
                    "~"
                } else {
                    "="
                };
                (format!("{mark} {amount}"), false)
            }
            Err(AmountParseError::DivisionByZero) => ("Can't divide by zero".to_owned(), true),
            // Mid-sum (`12+`) is still being typed; anything else can't be read.
            Err(_) if expr.trim_end().ends_with(['+', '-', '*', '/', '×', '÷', '−', '(']) => {
                ("…".to_owned(), false)
            }
            Err(_) => ("Not an amount".to_owned(), true),
        }
    }

    /// The display and the keypad, shared by the popup and the window. Returns what was pressed.
    fn calc_body(
        &mut self,
        ui: &mut Ui,
        ctx: &Context,
        expr: &mut String,
        focus: &mut bool,
        slim: bool,
        scope: &str,
        ok_label: Option<&str>,
    ) -> Pad {
        let mut pad = Pad::None;
        let display_id = Id::new((scope, "display"));
        // Refuse what the reader refuses, and take `=` as a key, only while the display
        // has the keyboard: the F2 note box takes letters while the calculator is open.
        let reader = self.reader.clone();
        let mut equals = false;
        let had_focus = ctx.memory(|m| m.has_focus(display_id));
        ctx.input_mut(|i| {
            if !had_focus {
                return;
            }
            for ev in &mut i.events {
                if let Event::Text(t) | Event::Paste(t) = ev {
                    if t.contains('=') {
                        equals = true;
                    }
                    t.retain(|c| reader.accepts(c));
                }
            }
        });
        let enter = had_focus && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        let esc = had_focus && ctx.input(|i| i.key_pressed(Key::Escape));
        // The keypad's width: four 40-point keys and the three gaps between them.
        let width = if slim { 250.0 } else { 160.0 + 3.0 * ui.spacing().item_spacing.x };
        let r = ui.add(
            TextEdit::singleline(expr)
                .id(display_id)
                .font(TextStyle::Monospace)
                .horizontal_align(Align::RIGHT)
                .desired_width(width)
                .hint_text("0")
                .event_filter(egui::EventFilter {
                    escape: true,
                    ..Default::default()
                }),
        );
        self.h.track(&r, format!("{scope} display"));
        if *focus {
            r.request_focus();
            move_cursor_to_end(ctx, display_id, expr.chars().count());
            *focus = false;
        }
        let (line, is_error) = self.result_line(expr);
        let color = if is_error {
            Color32::from_rgb(0xc0, 0x2b, 0x2b)
        } else {
            ui.visuals().text_color()
        };
        ui.allocate_ui_with_layout(
            vec2(width, 20.0),
            egui::Layout::right_to_left(Align::Center),
            |ui| ui.label(RichText::new(line).color(color)),
        );
        if equals {
            pad = Pad::Equals;
        }
        if enter {
            pad = Pad::Enter;
        }
        if esc {
            pad = Pad::Esc;
        }
        ui.add_space(2.0);
        let dec = self.reader.decimal_mark().to_owned();
        let press =
            |ui: &mut Ui, h: &mut Harness, label: &str, size: egui::Vec2| -> Option<String> {
                let b = ui.add_sized(size, Button::new(label));
                h.track(&b, format!("{scope} key {label}"));
                b.clicked().then(|| label.to_owned())
            };
        let mut pressed: Option<String> = None;
        if slim {
            ui.horizontal(|ui| {
                for label in ["+", "−", "×", "÷", "Clr", "Del"] {
                    if let Some(p) = press(ui, &mut self.h, label, vec2(34.0, 28.0)) {
                        pressed = Some(p);
                    }
                }
                if let Some(ok) = ok_label {
                    if let Some(p) = press(ui, &mut self.h, ok, vec2(40.0, 28.0)) {
                        pressed = Some(p);
                    }
                }
            });
        } else {
            let rows: [[&str; 4]; 4] = [
                ["Clr", "Del", "÷", "×"],
                ["7", "8", "9", "−"],
                ["4", "5", "6", "+"],
                ["1", "2", "3", "="],
            ];
            for row in rows {
                ui.horizontal(|ui| {
                    for label in row {
                        if let Some(p) = press(ui, &mut self.h, label, vec2(40.0, 30.0)) {
                            pressed = Some(p);
                        }
                    }
                });
            }
            ui.horizontal(|ui| {
                if let Some(p) = press(ui, &mut self.h, "0", vec2(40.0, 30.0)) {
                    pressed = Some(p);
                }
                if let Some(p) = press(ui, &mut self.h, &dec, vec2(40.0, 30.0)) {
                    pressed = Some(p);
                }
                if let Some(ok) = ok_label {
                    // Two keys wide plus the gap between them, so it lines up with `−` and `=`.
                    let w = 80.0 + ui.spacing().item_spacing.x;
                    if let Some(p) = press(ui, &mut self.h, ok, vec2(w, 30.0)) {
                        pressed = Some(p);
                    }
                }
            });
        }
        if let Some(label) = pressed {
            match label.as_str() {
                "Clr" => expr.clear(),
                "Del" => {
                    expr.pop();
                }
                "=" => pad = Pad::Equals,
                "OK" | "Paste" => pad = Pad::Enter,
                "÷" => expr.push('/'),
                "×" => expr.push('*'),
                "−" => expr.push('-'),
                other => expr.push_str(other),
            }
            *focus = true;
            if pad == Pad::None {
                pad = Pad::Edited;
            }
        }
        pad
    }

    fn calc_popup(&mut self, ctx: &Context) {
        let Some(mut p) = self.popup.take() else {
            return;
        };
        let anchor = self.rects[p.field as usize].left_top() - vec2(0.0, 4.0);
        let slim = self.variant == Variant::B;
        let mut pad = Pad::None;
        let out = Area::new(Id::new("calc_popup"))
            .order(Order::Foreground)
            .pivot(Align2::LEFT_BOTTOM)
            .fixed_pos(anchor)
            .show(ctx, |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(
                        RichText::new(format!("Calculator · {}", p.field.label()))
                            .small()
                            .weak(),
                    );
                    pad = self.calc_body(
                        ui,
                        ctx,
                        &mut p.expr,
                        &mut p.focus,
                        slim,
                        "calc",
                        Some("OK"),
                    );
                });
            });
        self.popup_rect = out.response.rect;
        self.h.region(self.popup_rect, "calculator popup");
        // Alt+↓ again closes it.
        let alt_close = ctx.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowDown));
        self.popup = Some(p);
        match pad {
            Pad::Enter => return self.popup_commit("Enter"),
            Pad::Esc => return self.popup_cancel("Esc"),
            Pad::Equals => self.equals(),
            _ => {}
        }
        if alt_close {
            return self.popup_cancel("Alt+↓");
        }
        let (pressed, pos) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos()));
        if let (true, Some(pos)) = (pressed, pos) {
            let field = self
                .popup
                .as_ref()
                .map(|p| p.field)
                .unwrap_or(Field::Payment);
            if !self.popup_rect.contains(pos)
                && !self.calc_button_rects[field as usize].contains(pos)
            {
                if self.o[OUTSIDE_COMMITS] {
                    self.popup_commit("click outside");
                } else {
                    self.popup_cancel("click outside");
                }
            }
        }
    }

    fn calc_window(&mut self, ctx: &Context) {
        let Some(mut w) = self.window.take() else {
            return;
        };
        let mut open = true;
        let mut pad = Pad::None;
        let mut paste = None;
        let win = Window::new("Calculator")
            .id(Id::new("calc_window"))
            .open(&mut open)
            .resizable(false)
            .default_pos(pos2(80.0, 140.0))
            .show(ctx, |ui| {
                pad = self.calc_body(ui, ctx, &mut w.expr, &mut w.focus, false, "window", None);
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for f in [Field::Payment, Field::Deposit] {
                        let label = format!("Paste to {}", f.label());
                        let b = ui.add(Button::new(label).selected(self.last_amount == f));
                        self.h.track(&b, format!("window: paste {}", f.name()));
                        if b.clicked() {
                            paste = Some(f);
                        }
                    }
                });
                ui.label(
                    RichText::new(
                        "Enter or = calculates. Paste fills the amount field you used last.",
                    )
                    .small()
                    .weak(),
                );
            });
        if let Some(win) = win {
            self.h.region(win.response.rect, "calculator window");
        }
        if matches!(pad, Pad::Equals | Pad::Enter) {
            self.window = Some(w);
            self.equals();
            w = self.window.take().unwrap();
        }
        if let Some(f) = paste {
            match self.result_of(&w.expr) {
                Ok((text, rounded)) => {
                    *self.text_mut(f) = text.clone();
                    self.last_amount = f;
                    self.h.event(
                        "calculator",
                        json!({ "outcome": "pasted", "field": f.name(), "expression": w.expr,
                                "result": text, "rounded": rounded }),
                    );
                    self.emit(
                        &format!("SetAmount({}, {text})", f.name()),
                        "calculator window Paste",
                    );
                }
                Err(_) => self.h.event(
                    "calculator",
                    json!({ "outcome": "paste_refused", "expression": w.expr }),
                ),
            }
        }
        if open {
            self.window = Some(w);
        } else {
            self.h.event(
                "calculator_window",
                json!({ "open": false, "via": "close button" }),
            );
        }
    }

    // -----------------------------------------------------------------------------------------
    // The entry row

    fn record(&mut self, via: &str) {
        let pay = self.reader.read(self.payment.trim());
        let dep = self.reader.read(self.deposit.trim());
        let to_cents =
            |c: &balanced_i18n::Calculation| (c.amount() * Decimal::from(100)).round().to_i64();
        let cents = match (
            self.payment.trim().is_empty(),
            self.deposit.trim().is_empty(),
        ) {
            (false, true) => pay.ok().and_then(|c| to_cents(&c)).map(|c| -c),
            (true, false) => dep.ok().and_then(|c| to_cents(&c)),
            _ => None,
        };
        let Some(cents) = cents else {
            self.entry_error = Some("Enter a valid Payment or Deposit amount".to_owned());
            return;
        };
        self.entry_error = None;
        let row = Row {
            date: self.date,
            payee: if self.payee.is_empty() {
                "(no payee)".to_owned()
            } else {
                self.payee.clone()
            },
            memo: self.memo.clone(),
            cents,
        };
        self.emit(
            &format!(
                "EnterTransaction({}, {}, {})",
                fmt_day(row.date),
                row.payee,
                fmt_cents(cents)
            ),
            via,
        );
        let at = self
            .rows
            .iter()
            .rposition(|r| r.date <= row.date)
            .map_or(0, |i| i + 1);
        self.rows.insert(at, row);
        self.balances = balances(&self.rows);
        self.scroll_to = Some(at);
        self.payee.clear();
        self.memo.clear();
        self.payment.clear();
        self.deposit.clear();
        self.focus_request = Some((id_of("payee"), true));
    }

    fn amount_field(&mut self, ui: &mut Ui, ctx: &Context, field: Field) {
        let id = field.id();
        let popup_here = self.popup.as_ref().is_some_and(|p| p.field == field);
        let focused = ctx.memory(|m| m.has_focus(id));
        ui.vertical(|ui| {
            ui.label(RichText::new(field.label()).small());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if focused && !popup_here {
                    self.last_amount = field;
                    self.amount_input(ctx, field);
                }
                let r = ui.add(
                    TextEdit::singleline(self.text_mut(field))
                        .id(id)
                        .horizontal_align(Align::RIGHT)
                        .desired_width(AMOUNT_W)
                        .hint_text("0.00"),
                );
                self.h.track(&r, format!("{} field", field.name()));
                self.rects[field as usize] = r.rect;
                if r.has_focus() {
                    self.last_amount = field;
                }
                // As in the built register: leaving the field (Enter, Tab, a click away)
                // replaces typed arithmetic with its result. A press on this field's
                // calculator button leaves it alone, so the popup can start from the text.
                let pressing_button = ctx.input(|i| {
                    i.pointer.any_down()
                        && i.pointer
                            .interact_pos()
                            .is_some_and(|p| self.calc_button_rects[field as usize].contains(p))
                });
                if r.lost_focus() && self.popup.is_none() && !pressing_button {
                    let text = self.text_of(field).trim().to_owned();
                    if self.reader.read(&text).is_ok_and(|c| c.is_arithmetic())
                        && let Ok((result, rounded)) = self.result_of(&text)
                    {
                        *self.text_mut(field) = result.clone();
                        self.h.event(
                            "typed_arithmetic",
                            json!({ "field": field.name(), "expression": text,
                                    "result": result, "rounded": rounded }),
                        );
                        self.emit(&format!("SetAmount({}, {result})", field.name()), "leaving the field");
                    }
                }
                if self.variant.field_popup() && self.o[BUTTON] {
                    let b = calc_icon(ui);
                    self.h
                        .track(&b, format!("{} calculator button", field.name()));
                    self.calc_button_rects[field as usize] = b.rect;
                    if b.clicked() {
                        if popup_here {
                            self.popup_cancel("calculator button");
                        } else {
                            self.open_calculator(field, "button");
                        }
                    }
                } else {
                    self.calc_button_rects[field as usize] = Rect::NOTHING;
                }
            });
            // The typed-arithmetic preview, from #90: `= $33.33`, or `~ $33.33` when rounded.
            let text = self.text_of(field).clone();
            let line = if text.trim().is_empty() {
                None
            } else {
                match self.reader.read(text.trim()) {
                    Ok(c) if c.is_arithmetic() => {
                        let amount =
                            self.loc
                                .format_amount(c.amount(), self.usd, AmountStyle::Symbol);
                        let mark = if c.rounded() { "~" } else { "=" };
                        Some((format!("{mark} {amount}"), false))
                    }
                    Ok(_) => None,
                    Err(AmountParseError::DivisionByZero) => {
                        Some(("Can't divide by zero".to_owned(), true))
                    }
                    Err(_) if focused => None,
                    Err(_) => Some(("Not an amount".to_owned(), true)),
                }
            };
            ui.allocate_ui(vec2(AMOUNT_W + 32.0, 16.0), |ui| match line {
                Some((l, err)) => {
                    let c = if err {
                        Color32::from_rgb(0xc0, 0x2b, 0x2b)
                    } else {
                        ui.visuals().weak_text_color()
                    };
                    ui.label(RichText::new(l).small().color(c));
                }
                None => {
                    ui.label(RichText::new(" ").small());
                }
            });
        });
    }

    /// Keys on a focused amount field: refuse characters, and the ways to open the calculator.
    fn amount_input(&mut self, ctx: &Context, field: Field) {
        let reader = self.reader.clone();
        ctx.input_mut(|i| {
            for ev in &mut i.events {
                if let Event::Text(t) | Event::Paste(t) = ev {
                    t.retain(|c| reader.accepts(c));
                }
            }
        });
        if !self.variant.field_popup() {
            return;
        }
        if self.o[ALT_DOWN] && ctx.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowDown)) {
            self.open_calculator(field, "Alt+↓");
            return;
        }
        if self.o[OPERATOR] {
            let text = self.text_of(field).trim().to_owned();
            let has_number = text.chars().any(|c| c.is_ascii_digit());
            let op = ctx.input_mut(|i| {
                let mut found = None;
                i.events.retain(|ev| {
                    if found.is_none()
                        && let Event::Text(t) = ev
                        && let Some(c) = t.chars().next()
                        && t.chars().count() == 1
                        && (matches!(c, '+' | '*' | '/') || (c == '-' && has_number))
                    {
                        found = Some(c);
                        return false;
                    }
                    true
                });
                found
            });
            if let Some(op) = op {
                // An empty field holds 0, so `+` on it starts `0+`, not a bare `+`.
                let seed = if self.o[FROM_FIELD] && !text.is_empty() {
                    text
                } else {
                    "0".to_owned()
                };
                self.start_popup(field, format!("{seed}{op}"), "operator key");
            }
        }
    }

    fn entry_bar(&mut self, ui: &mut Ui, ctx: &Context) {
        Panel::bottom("entry_bar")
            .frame(Frame::side_top_panel(ui.style()).inner_margin(8.0))
            .show(ui, |ui| {
                let top = ui.cursor().min;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    // Date
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Date").small());
                        ui.horizontal(|ui| {
                            if let Some((_, text, ..)) = &self.parked {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                let mut shown = text.clone();
                                ui.add_enabled(
                                    false,
                                    TextEdit::singleline(&mut shown).desired_width(92.0),
                                );
                                ui.add_enabled(false, Button::new("📅").min_size(vec2(26.0, 0.0)));
                            } else {
                                self.date_widget(ui, ctx, "date");
                            }
                        });
                        ui.label(RichText::new(" ").small());
                    });
                    // Payee, Memo
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Payee").small());
                        let r = ui.add(
                            TextEdit::singleline(&mut self.payee)
                                .id(id_of("payee"))
                                .desired_width(130.0),
                        );
                        self.h.track(&r, "payee field");
                        ui.label(RichText::new(" ").small());
                    });
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Memo").small());
                        let r = ui.add(
                            TextEdit::singleline(&mut self.memo)
                                .id(id_of("memo"))
                                .desired_width(130.0),
                        );
                        self.h.track(&r, "memo field");
                        ui.label(RichText::new(" ").small());
                    });
                    self.amount_field(ui, ctx, Field::Payment);
                    self.amount_field(ui, ctx, Field::Deposit);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(" ").small());
                        let b = ui.add(Button::new("Enter").min_size(vec2(60.0, 0.0)));
                        ui.memory_mut(|_| {});
                        self.h.track(&b, "Enter button");
                        if b.clicked() {
                            self.record("Enter button");
                        }
                        ui.label(RichText::new(" ").small());
                    });
                });
                if let Some(e) = self.entry_error.clone() {
                    ui.colored_label(Color32::from_rgb(0xc0, 0x2b, 0x2b), e);
                } else {
                    ui.label(RichText::new(self.status.clone()).small().weak());
                }
                let bottom = ui.min_rect().bottom();
                let width = ui.available_width();
                self.h.region(
                    Rect::from_min_max(top, pos2(top.x + width, bottom)),
                    "entry bar",
                );
            });
    }

    fn register(&mut self, ui: &mut Ui) {
        let row_h = 22.0;
        let mut table = TableBuilder::new(ui)
            .striped(true)
            .resizable(false)
            .stick_to_bottom(true)
            .column(Column::exact(86.0))
            .column(Column::exact(220.0))
            .column(Column::exact(180.0))
            .column(Column::exact(100.0))
            .column(Column::exact(100.0))
            .column(Column::exact(110.0));
        if let Some(at) = self.scroll_to.take() {
            table = table.scroll_to_row(at, Some(Align::Center));
        }
        let rows = &self.rows;
        let bal = &self.balances;
        let selected = self.selected_row;
        let mut clicked = None;
        table
            .header(22.0, |mut h| {
                for t in ["Date", "Payee", "Memo", "Payment", "Deposit", "Balance"] {
                    h.col(|ui| {
                        ui.label(RichText::new(t).strong());
                    });
                }
            })
            .body(|body| {
                body.rows(row_h, rows.len(), |mut row| {
                    let i = row.index();
                    let r = &rows[i];
                    row.set_selected(selected == Some(i));
                    row.col(|ui| {
                        ui.label(fmt_day(r.date));
                    });
                    row.col(|ui| {
                        ui.label(&r.payee);
                    });
                    row.col(|ui| {
                        ui.label(&r.memo);
                    });
                    row.col(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            if r.cents < 0 {
                                ui.label(fmt_cents(-r.cents));
                            }
                        });
                    });
                    row.col(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            if r.cents > 0 {
                                ui.label(fmt_cents(r.cents));
                            }
                        });
                    });
                    row.col(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            ui.label(fmt_cents(bal[i]));
                        });
                    });
                    if row.response().clicked() {
                        clicked = Some(i);
                    }
                });
            });
        if let Some(i) = clicked {
            self.selected_row = Some(i);
            self.h.event("click", json!({ "widget": format!("register row {}", i + 1) }));
        }
    }

    fn menu_bar(&mut self, ui: &mut Ui) {
        Panel::top("menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    ui.add_enabled(false, Button::new("(not in this prototype)"));
                });
                ui.menu_button("Tools", |ui| {
                    let enabled = self.variant == Variant::C;
                    let b = ui.add_enabled(enabled, Button::new("Calculator"));
                    self.h.track(&b, "Tools: Calculator");
                    if !enabled {
                        b.on_disabled_hover_text("Variant C only");
                    } else if b.clicked() {
                        self.open_window("Tools menu");
                        ui.close();
                    }
                });
                ui.label(
                    RichText::new("Everyday Checking · stub data, today is 10/20/2026").weak(),
                );
            });
        });
    }

    // -----------------------------------------------------------------------------------------
    // Dock

    fn dock(&mut self, ui: &mut Ui) {
        let frame = {
            let v = ui.visuals();
            Frame::side_top_panel(ui.style())
                .fill(v.warn_fg_color.gamma_multiply(0.10).blend(v.panel_fill))
                .stroke(Stroke::new(1.0, v.warn_fg_color))
        };
        if !self.dock_open {
            Panel::right("prototype_dock_edge")
                .exact_size(22.0)
                .resizable(false)
                .frame(frame)
                .show(ui, |ui| {
                    let r = ui
                        .small_button("◀")
                        .on_hover_text("Show prototype controls (F4)");
                    self.h.track(&r, "dock: expand");
                    if r.clicked() {
                        self.set_dock(true, "click");
                    }
                });
            return;
        }
        Panel::right("prototype_dock")
            .exact_size(350.0)
            .resizable(false)
            .frame(frame)
            .show(ui, |ui| {
                Sides::new().show(
                    ui,
                    |ui| ui.label(RichText::new("PROTOTYPE").strong()),
                    |ui| {
                        let r = ui.small_button("▶").on_hover_text("Collapse (F4)");
                        self.h.track(&r, "dock: collapse");
                        if r.clicked() {
                            self.set_dock(false, "click");
                        }
                    },
                );
                ui.separator();
                ui.label(RichText::new("Variant (how the calculator is offered)").strong());
                for (v, (_, hint)) in Variant::ALL.into_iter().zip(VARIANT_KEYS) {
                    let r =
                        ui.selectable_label(self.variant == v, format!("{hint}   {}", v.label()));
                    self.h.track(&r, format!("dock: variant {}", v.key()));
                    if r.clicked() {
                        self.set_variant(v, "click");
                    }
                }
                ui.separator();
                ui.label(RichText::new("Toggles (the variants each applies to)").strong());
                for (i, ((key, label, scope, _), (_, hint))) in
                    TOGGLES.iter().zip(TOGGLE_KEYS).enumerate()
                {
                    let mut value = self.o[i];
                    let r = ui.checkbox(&mut value, format!("{hint}   {label}  [{scope}]"));
                    self.h.track(&r, format!("dock: toggle {key}"));
                    if r.changed() {
                        self.set_toggle(key, Some(value), "click");
                    }
                }
                ui.separator();
                self.h.dock_spotlight(ui);
                self.h.dock_prompt(ui);
                ui.separator();
                ui.label(
                    RichText::new(
                        "A–D or Left/Right variant · 1–9, 0 toggles · F2 note · F3 marker · F4 hide this dock · F5 spotlight · F6 prompt. Letters and numbers type into a focused field: press Esc or click away first.",
                    )
                    .small()
                    .weak(),
                );
            });
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pad {
    None,
    /// A key edited the expression.
    Edited,
    Equals,
    Enter,
    Esc,
}

/// A small calculator icon button, painted (the default fonts have no calculator glyph).
fn calc_icon(ui: &mut Ui) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
    let visuals = ui.style().interact(&response);
    ui.painter().rect(
        rect.shrink(1.0),
        3.0,
        visuals.weak_bg_fill,
        visuals.bg_stroke,
        StrokeKind::Inside,
    );
    let color = visuals.fg_stroke.color;
    let body = Rect::from_center_size(rect.center(), vec2(12.0, 16.0));
    ui.painter()
        .rect_stroke(body, 2.0, Stroke::new(1.2, color), StrokeKind::Inside);
    let screen = Rect::from_min_size(body.min + vec2(2.0, 2.0), vec2(8.0, 3.5));
    ui.painter().rect_filled(screen, 1.0, color);
    for r in 0..3 {
        for c in 0..3 {
            ui.painter().circle_filled(
                pos2(
                    body.min.x + 3.0 + c as f32 * 3.0,
                    body.min.y + 8.5 + r as f32 * 2.8,
                ),
                0.8,
                color,
            );
        }
    }
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Calculator"));
    response.on_hover_text("Calculator")
}

fn move_cursor_to_end(ctx: &Context, id: Id, len: usize) {
    if let Some(mut state) = TextEdit::load_state(ctx, id) {
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(len),
            )));
        state.store(ctx, id);
    }
}

impl eframe::App for Proto {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.h.event("session_end", json!({}));
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(cmd) = self.h.begin_frame(&ctx) {
            self.apply_control(&ctx, &cmd);
        }
        if let Some((id, end)) = self.focus_request.take() {
            ctx.memory_mut(|m| m.request_focus(id));
            if end {
                let len = if id == self.date_id() {
                    self.date_text.chars().count()
                } else if id == Field::Payment.id() {
                    self.payment.chars().count()
                } else if id == Field::Deposit.id() {
                    self.deposit.chars().count()
                } else {
                    0
                };
                move_cursor_to_end(&ctx, id, len);
            }
        }
        if !ctx.egui_wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(Key::ArrowLeft)) {
                self.set_variant(self.variant.step(-1), "key");
            }
            if ctx.input(|i| i.key_pressed(Key::ArrowRight)) {
                self.set_variant(self.variant.step(1), "key");
            }
            for (v, (k, _)) in Variant::ALL.into_iter().zip(VARIANT_KEYS) {
                if ctx.input(|i| i.key_pressed(k) && i.modifiers.is_none()) {
                    self.set_variant(v, "key");
                }
            }
            for (i, (k, _)) in TOGGLE_KEYS.iter().enumerate().take(TOGGLES.len()) {
                if ctx.input(|i| i.key_pressed(*k) && i.modifiers.is_none()) {
                    self.set_toggle(TOGGLES[i].0, None, "key");
                }
            }
        }
        if ctx.input(|i| i.key_pressed(Key::F4)) {
            self.set_dock(!self.dock_open, "key");
        }
        if self.o[GO_TO_DATE] && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::G)) {
            self.open_goto("Ctrl+G");
        }

        self.dock(ui);
        self.h.prompt_panel(ui);
        self.menu_bar(ui);
        self.entry_bar(ui, &ctx);
        CentralPanel::default().show(ui, |ui| {
            let rect = ui.max_rect();
            self.h.region(rect, "register");
            self.register(ui);
        });
        self.calendar_popup(&ctx);
        self.calc_popup(&ctx);
        self.calc_window(&ctx);
        self.goto_window(&ctx);
        self.h.end_frame(&ctx);
    }
}

fn main() -> eframe::Result {
    // Prints the stub figures and exits, so they can be checked without a window.
    if std::env::var_os("CALC_PROTO_DUMP").is_some() {
        let rows = generate();
        let bal = balances(&rows);
        println!(
            "rows {} first {} last {} ending {} min {} max {}",
            rows.len(),
            fmt_day(rows[0].date),
            fmt_day(rows[rows.len() - 1].date),
            fmt_cents(*bal.last().unwrap()),
            fmt_cents(*bal.iter().min().unwrap()),
            fmt_cents(*bal.iter().max().unwrap()),
        );
        return Ok(());
    }
    let args = Args::from_env();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Balanced — PROTOTYPE calculator and calendar popups")
            .with_maximized(true),
        ..Default::default()
    };
    eframe::run_native(
        "balanced_calc_popup_prototype",
        options,
        Box::new(move |cc| {
            let proto = Proto::new(&args);
            let style = proto.theme.style();
            cc.egui_ctx.set_fonts(proto.theme.fonts());
            cc.egui_ctx.set_style_of(egui::Theme::Dark, style.clone());
            cc.egui_ctx.set_style_of(egui::Theme::Light, style);
            cc.egui_ctx.set_theme(egui::Theme::Light);
            Ok(Box::new(proto))
        }),
    )
}

/// A flat square cell, as the built calendar draws its days and ◀ ▶: filled when
/// selected or hovered, outlined when it is today. It never takes focus.
fn flat_cell(ui: &mut Ui, size: egui::Vec2, text: &str, selected: bool, today: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::CLICK);
    let visuals = ui.style().interact(&response);
    if selected {
        ui.painter().rect_filled(rect, 0.0, ui.visuals().selection.bg_fill);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, visuals.weak_bg_fill);
    }
    if today {
        ui.painter().rect_stroke(rect, 0.0, visuals.fg_stroke, egui::StrokeKind::Inside);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        TextStyle::Body.resolve(ui.style()),
        visuals.text_color(),
    );
    response
}
