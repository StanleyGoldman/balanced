// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! PROTOTYPE, throwaway: row-level Register editing with inline buttons.
//!
//! Question (ticket #356): does editing a row as a unit, with inline Save, Cancel, Split and
//! More actions buttons, feel right, and how should the buttons sit beside the Actions column
//! and the entry bar?
//!
//! Applied from the #212 grilling: a row is edited as a unit, Esc cancels the whole row, one Save
//! sends one Command with every changed field, and leaving a changed, valid row asks Save /
//! Continue editing / Discard (with an "Always save without asking" checkbox). Unchanged rows
//! never prompt, and moving between cells never prompts.
//!
//! Three variants differ in where the buttons (and the edit fields) sit:
//!   A  Actions cell: the row is edited in place; Save, Cancel, Split, More sit in its Actions cell
//!   B  Entry bar: the row is edited in a bar under the Register; the buttons sit in the bar
//!   C  Both: edited in place with A's buttons, and the bar repeats the fields with Save, Cancel
//!
//! Toggles (1-7) switch the keyboard and flow details; see `TOGGLES`.
//!
//! Run:
//!   cargo run -p balanced_ui --example row_edit_prototype -- \
//!     [--variant a|b|c] [--log <dir>/events.jsonl] [--control <dir>/control.txt]
//!
//! Control-file verbs (besides the harness's `prompt` and `spotlight`): `variant <key>`,
//! `toggle <key|number> [on|off]`, `screenshot [label]`, `dock on|off`, `reload`, `quit`,
//! `account <index|name>`, `row <n>` (select and scroll to the nth row, negative counts from the
//! end), `edit <n> [col]` (start editing the nth row), `draft` (edit the blank new transaction),
//! `type <col> <text>` (set a cell of the row being edited, as if typed; empty text clears it),
//! `save`, `cancel`, `split`, `close` (a window close request, which asks when a row is changed).
//! Columns: date, num, payee, category, memo, payment, deposit.
//!
//! Fixed-seed stub data, English literals, en-US dates. None of this is production code.

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

use std::collections::HashSet;

use balanced_prototype_harness::{Args, Harness};
use eframe::egui;
use egui::{
    Align, Button, CentralPanel, Color32, Context, Id, Key, Layout, Panel, Rect, RichText, Sense,
    Sides, Stroke, StrokeKind, TextEdit, Ui, UiBuilder, ViewportBuilder, ViewportCommand, pos2,
    vec2,
};
use egui_extras::{Column, TableBuilder, TableRow};
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
const HISTORY_DAYS: i64 = 3_650;
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

/// Parses `m/d/yyyy`.
fn parse_day(s: &str) -> Option<i64> {
    let mut parts = s.trim().split('/');
    let m: i64 = parts.next()?.trim().parse().ok()?;
    let d: i64 = parts.next()?.trim().parse().ok()?;
    let y: i64 = parts.next()?.trim().parse().ok()?;
    if parts.next().is_some() || !(1900..=2200).contains(&y) {
        return None;
    }
    let days = days_from_civil(y, m, d);
    (civil(days) == (y, m, d)).then_some(days)
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

/// Parses an unsigned amount; empty is zero.
fn parse_cents(s: &str) -> Option<i64> {
    let s = s.trim().replace(',', "");
    if s.is_empty() {
        return Some(0);
    }
    let (whole, frac) = s.split_once('.').unwrap_or((&s, ""));
    if whole.is_empty() && frac.is_empty() {
        return None;
    }
    if frac.len() > 2
        || !whole.chars().all(|c| c.is_ascii_digit())
        || !frac.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let whole: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let frac: i64 = match frac.len() {
        0 => 0,
        1 => frac.parse::<i64>().ok()? * 10,
        _ => frac.parse().ok()?,
    };
    Some(whole * 100 + frac)
}

// ---------------------------------------------------------------------------------------------
// The transaction model: committed rows, and the pending edit of one row

const NAMES: [&str; 7] = [
    "date", "num", "payee", "category", "memo", "payment", "deposit",
];
const LABELS: [&str; 7] = [
    "Date", "Num", "Payee", "Category", "Memo", "Payment", "Deposit",
];
const LAST: usize = 6;
const SPLIT_LABEL: &str = "--Split--";

#[derive(Clone, PartialEq, Default)]
struct SplitLine {
    cat: String,
    memo: String,
    /// Unsigned, in the direction of the transaction.
    amount: String,
}

/// Everything an edit changes: the seven cells and the split lines. A row is edited as a unit, so
/// this is also what one Save sends.
#[derive(Clone, PartialEq, Default)]
struct Fields {
    cells: [String; 7],
    splits: Option<Vec<SplitLine>>,
}

impl Fields {
    fn out(&self) -> i64 {
        parse_cents(&self.cells[5]).unwrap_or(0)
    }

    fn inn(&self) -> i64 {
        parse_cents(&self.cells[6]).unwrap_or(0)
    }

    fn total(&self) -> i64 {
        self.inn() - self.out()
    }

    fn blank() -> Self {
        let mut f = Self::default();
        f.cells[0] = fmt_day(TODAY);
        f
    }
}

fn lines_total(lines: &[SplitLine]) -> i64 {
    lines
        .iter()
        .map(|l| parse_cents(&l.amount).unwrap_or(0))
        .sum()
}

/// Why a row can't be saved, and the cell to put the cursor in.
fn validate(f: &Fields) -> Result<(), (usize, String)> {
    if parse_day(&f.cells[0]).is_none() {
        return Err((
            0,
            "The date isn't a valid date (month/day/year).".to_owned(),
        ));
    }
    if parse_cents(&f.cells[5]).is_none() {
        return Err((5, "The payment isn't an amount.".to_owned()));
    }
    if parse_cents(&f.cells[6]).is_none() {
        return Err((6, "The deposit isn't an amount.".to_owned()));
    }
    if f.out() != 0 && f.inn() != 0 {
        return Err((5, "Enter a payment or a deposit, not both.".to_owned()));
    }
    if f.total() == 0 {
        return Err((5, "Enter a payment or deposit amount.".to_owned()));
    }
    if let Some(lines) = &f.splits {
        if lines
            .iter()
            .any(|l| parse_cents(&l.amount).is_none() || l.amount.trim().starts_with('-'))
        {
            return Err((5, "A split line's amount isn't an amount.".to_owned()));
        }
        let sum = lines_total(lines);
        if sum != f.total().abs() {
            return Err((
                5,
                format!(
                    "The split lines add up to {}, but the transaction is {}.",
                    fmt_cents(sum),
                    fmt_cents(f.total().abs())
                ),
            ));
        }
    }
    Ok(())
}

#[derive(Clone)]
struct Txn {
    id: u32,
    day: i64,
    amount: i64,
    f: Fields,
}

impl Txn {
    /// A committed row from edited fields, with the cells in their tidy form.
    fn commit(id: u32, mut f: Fields) -> Self {
        let day = parse_day(&f.cells[0]).unwrap_or(START);
        let total = f.total();
        f.cells[0] = fmt_day(day);
        f.cells[5] = if total < 0 {
            fmt_cents(-total)
        } else {
            String::new()
        };
        f.cells[6] = if total > 0 {
            fmt_cents(total)
        } else {
            String::new()
        };
        if let Some(lines) = &mut f.splits {
            for l in lines.iter_mut() {
                l.amount = fmt_cents(parse_cents(&l.amount).unwrap_or(0));
            }
            f.cells[3] = SPLIT_LABEL.to_owned();
        }
        Self {
            id,
            day,
            amount: total,
            f,
        }
    }
}

struct Account {
    name: &'static str,
    txns: Vec<Txn>,
    balances: Vec<i64>,
    next_id: u32,
}

impl Account {
    fn recompute(&mut self) {
        self.txns.sort_by_key(|t| (t.day, t.id));
        let mut sum = 0;
        self.balances = self
            .txns
            .iter()
            .map(|t| {
                sum += t.amount;
                sum
            })
            .collect();
    }

    fn position(&self, id: u32) -> Option<usize> {
        self.txns.iter().position(|t| t.id == id)
    }
}

// ---------------------------------------------------------------------------------------------
// Stub data: fixed seed, realistic volume

const SEED: u64 = 0x00ba_1a9c_ed77;

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

/// (payee, category, typical cents).
const SPENDING: &[(&str, &str, i64)] = &[
    ("Green Valley Market", "Groceries", 7_800),
    ("Shell", "Auto:Fuel", 4_800),
    ("Netflix", "Entertainment", 1_599),
    ("Blue Bottle Coffee", "Dining", 650),
    ("City Water & Power", "Utilities", 9_500),
    ("Amazon", "Shopping", 3_900),
    ("Trattoria Roma", "Dining", 5_200),
    ("Pharmacy Plus", "Health", 2_300),
    ("Hardware Depot", "Home", 6_400),
    ("Metro Transit", "Transportation", 2_750),
    ("Spotify", "Entertainment", 1_099),
    ("Gym Monthly", "Health", 3_500),
    ("Book Nook", "Shopping", 2_400),
    ("Corner Bakery", "Dining", 900),
];

const MEMOS: &[&str] = &[
    "",
    "",
    "",
    "",
    "weekly",
    "gift",
    "for the trip",
    "reimbursable",
];

fn row_fields(
    day: i64,
    num: &str,
    payee: &str,
    cat: &str,
    memo: &str,
    cents: i64,
    splits: Option<Vec<SplitLine>>,
) -> Fields {
    let mut f = Fields::default();
    f.cells[0] = fmt_day(day);
    f.cells[1] = num.to_owned();
    f.cells[2] = payee.to_owned();
    f.cells[3] = if splits.is_some() {
        SPLIT_LABEL.to_owned()
    } else {
        cat.to_owned()
    };
    f.cells[4] = memo.to_owned();
    if cents < 0 {
        f.cells[5] = fmt_cents(-cents);
    } else {
        f.cells[6] = fmt_cents(cents);
    }
    f.splits = splits;
    f
}

fn push(a: &mut Account, f: Fields) {
    let id = a.next_id;
    a.next_id += 1;
    a.txns.push(Txn::commit(id, f));
}

fn generate() -> Vec<Account> {
    let mut g = Lcg(SEED);
    let mut checking = Account {
        name: "Checking",
        txns: Vec::new(),
        balances: Vec::new(),
        next_id: 1,
    };
    let mut savings = Account {
        name: "Savings",
        txns: Vec::new(),
        balances: Vec::new(),
        next_id: 1,
    };
    push(
        &mut checking,
        row_fields(
            START,
            "",
            "Opening Balance",
            "[Opening Balance]",
            "",
            500_000,
            None,
        ),
    );
    push(
        &mut savings,
        row_fields(
            START,
            "",
            "Opening Balance",
            "[Opening Balance]",
            "",
            1_000_000,
            None,
        ),
    );
    let mut check_no = 1000;
    for day in START + 1..=TODAY {
        let dom = civil(day).2;
        if dom == 1 {
            check_no += 1;
            push(
                &mut checking,
                row_fields(
                    day,
                    &check_no.to_string(),
                    "Pinewood Apartments",
                    "Housing:Rent",
                    "",
                    -145_000,
                    None,
                ),
            );
        }
        if dom == 1 || dom == 15 {
            let pay = g.around(235_000) / 2 + 117_500;
            push(
                &mut checking,
                row_fields(day, "DEP", "Northwind Payroll", "Salary", "", pay, None),
            );
        }
        if dom == 3 {
            push(
                &mut checking,
                row_fields(
                    day,
                    "TXFR",
                    "Transfer to Savings",
                    "[Savings]",
                    "",
                    -70_000,
                    None,
                ),
            );
            push(
                &mut savings,
                row_fields(
                    day,
                    "TXFR",
                    "Transfer from Checking",
                    "[Checking]",
                    "",
                    70_000,
                    None,
                ),
            );
        }
        if dom == 28 {
            let interest = g.around(2_000);
            push(
                &mut savings,
                row_fields(
                    day,
                    "",
                    "Bank Interest",
                    "Interest Income",
                    "",
                    interest,
                    None,
                ),
            );
        }
        if dom == 10 && g.below(100) < 12 {
            let repair = g.around(90_000);
            push(
                &mut savings,
                row_fields(day, "", "Emergency repair", "Home", "", -repair, None),
            );
        }
        let spends = usize::from(g.below(100) < 85) + usize::from(g.below(100) < 25);
        for _ in 0..spends {
            let (payee, cat, typical) = SPENDING[g.below(SPENDING.len() as u64) as usize];
            let cents = g.around(typical);
            let memo = MEMOS[g.below(MEMOS.len() as u64) as usize];
            let num = if g.below(100) < 8 {
                check_no += 1;
                check_no.to_string()
            } else {
                String::new()
            };
            let splits = (g.below(100) < 4 && cents > 2_000).then(|| {
                let first = cents * 6 / 10;
                let rest = cents - first;
                let mut lines = vec![
                    SplitLine {
                        cat: cat.to_owned(),
                        memo: String::new(),
                        amount: fmt_cents(first),
                    },
                    SplitLine {
                        cat: "Household".to_owned(),
                        memo: "other items".to_owned(),
                        amount: fmt_cents(rest),
                    },
                ];
                if g.below(100) < 40 {
                    let third = rest / 2;
                    lines[1].amount = fmt_cents(rest - third);
                    lines.push(SplitLine {
                        cat: "Gifts".to_owned(),
                        memo: String::new(),
                        amount: fmt_cents(third),
                    });
                }
                lines
            });
            push(
                &mut checking,
                row_fields(day, &num, payee, cat, memo, -cents, splits),
            );
        }
    }
    checking.recompute();
    savings.recompute();
    vec![checking, savings]
}

// ---------------------------------------------------------------------------------------------
// Variants and toggles

#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    A,
    B,
    C,
}

const VARIANT_KEYS: [(Key, &str); 3] = [(Key::A, "A"), (Key::B, "B"), (Key::C, "C")];

impl Variant {
    const ALL: [Self; 3] = [Self::A, Self::B, Self::C];

    fn key(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|v| v.key().eq_ignore_ascii_case(key))
    }

    fn label(self) -> &'static str {
        match self {
            Self::A => "A · Buttons in the Actions cell",
            Self::B => "B · Entry bar",
            Self::C => "C · Both",
        }
    }

    fn step(self, delta: isize) -> Self {
        let n = Self::ALL.len() as isize;
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0) as isize;
        Self::ALL[(i + delta).rem_euclid(n) as usize]
    }
}

const ENTER_ANY: usize = 0;
const TAB_BUTTONS: usize = 1;
const NEXT_ROW: usize = 2;
const ALWAYS_SAVE: usize = 3;
const TRIM_MORE: usize = 4;
const SPLIT_INLINE: usize = 5;
const PROMPT_CONTINUE: usize = 6;

/// (key, label, starting value); every toggle applies to all variants.
const TOGGLES: &[(&str, &str, bool)] = &[
    (
        "enter_any",
        "Enter saves from any cell (off: only the last)",
        false,
    ),
    (
        "tab_buttons",
        "Tab leaves the last cell into the buttons (off: wraps)",
        true,
    ),
    (
        "next_row",
        "After Save, edit the next row (off: stay on it)",
        false,
    ),
    (
        "always_save",
        "Always save without asking (the Preference)",
        false,
    ),
    (
        "trim_more",
        "More actions: trimmed menu (off: same as Actions)",
        false,
    ),
    (
        "split_inline",
        "Split lines under the row (off: in a dialog)",
        true,
    ),
    (
        "prompt_continue",
        "Leave prompt: Enter means Continue editing (GnuCash); off: Save",
        false,
    ),
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

// ---------------------------------------------------------------------------------------------
// The edit in progress

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Target {
    Txn(u32),
    Draft,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Surface {
    Row,
    Bar,
}

struct Edit {
    target: Target,
    fields: Fields,
    original: Fields,
    split_open: bool,
    /// The cell to put the cursor in on the next frame it is drawn.
    focus: Option<usize>,
    last_col: usize,
    error: Option<(usize, String)>,
}

impl Edit {
    fn new(target: Target, fields: Fields) -> Self {
        Self {
            target,
            original: fields.clone(),
            fields,
            split_open: false,
            focus: None,
            last_col: 0,
            error: None,
        }
    }

    fn dirty(&self) -> bool {
        self.fields != self.original
    }

    fn changed(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = (0..7)
            .filter(|c| self.fields.cells[*c] != self.original.cells[*c])
            .map(|c| NAMES[c])
            .collect();
        if self.fields.splits != self.original.splits {
            out.push("splits");
        }
        out
    }
}

/// Where the user was going when a changed row stopped them.
#[derive(Clone, Copy)]
enum Leave {
    Row(Target, usize),
    Account(usize),
    Close,
}

impl Leave {
    fn text(self) -> String {
        match self {
            Self::Row(t, c) => format!("row {t:?} cell {c}"),
            Self::Account(i) => format!("account {i}"),
            Self::Close => "close the window".to_owned(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Which {
    Full,
    Short,
}

/// What a click or key asked for, applied once the frame has drawn.
enum Act {
    Save(&'static str),
    Cancel(&'static str),
    ToggleSplit,
    Menu(&'static str),
    Begin(Target, usize),
    Select(Target),
}

#[derive(Clone, Copy)]
enum Vis {
    Txn(usize),
    Draft,
    Split(usize),
}

enum SplitView {
    Table,
    Bar,
    Dialog,
}

const RED: Color32 = Color32::from_rgb(0xc0, 0x2b, 0x2b);
const BAR_BASE: f32 = 86.0;
const STATUS_H: f32 = 26.0;

struct Proto {
    h: Harness,
    accounts: Vec<Account>,
    current: usize,
    variant: Variant,
    o: [bool; 7],
    theme: Theme,
    dock_open: bool,
    status: String,
    selected: Option<Target>,
    edit: Option<Edit>,
    leave_prompt: Option<Leave>,
    prompt_always: bool,
    closing: bool,
    /// XWayland ignores `with_maximized` at creation, so maximize again once the window is up.
    maximize_sent: bool,
    scroll_to: Option<usize>,
    /// The ids of the edit's widgets drawn last frame and this frame.
    edit_ids: HashSet<Id>,
    edit_ids_now: HashSet<Id>,
    acts: Vec<Act>,
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
        let mut o = [false; 7];
        for (i, t) in TOGGLES.iter().enumerate() {
            o[i] = t.2;
        }
        Self {
            h,
            accounts: generate(),
            current: 0,
            variant,
            o,
            theme: Theme::new(ThemeSettings::default()),
            dock_open: true,
            status: "Ready".to_owned(),
            selected: None,
            edit: None,
            leave_prompt: None,
            prompt_always: false,
            closing: false,
            maximize_sent: false,
            scroll_to: Some(usize::MAX),
            edit_ids: HashSet::new(),
            edit_ids_now: HashSet::new(),
            acts: Vec::new(),
        }
    }

    fn emit(&mut self, command: &str, source: &str) {
        self.h.event(
            "command",
            json!({ "would_emit": command, "source": source }),
        );
        self.status = format!("{source}: would emit {command}");
    }

    fn active_surface(&self) -> Surface {
        if self.variant == Variant::B {
            Surface::Bar
        } else {
            Surface::Row
        }
    }

    fn has_bar(&self) -> bool {
        self.variant != Variant::A
    }

    fn split_view(&self) -> SplitView {
        if !self.o[SPLIT_INLINE] {
            SplitView::Dialog
        } else if self.variant == Variant::B {
            SplitView::Bar
        } else {
            SplitView::Table
        }
    }

    fn split_is_open(&self) -> bool {
        self.edit
            .as_ref()
            .is_some_and(|e| e.split_open && e.fields.splits.is_some())
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
        if let Some(e) = &mut self.edit {
            e.focus = Some(e.last_col);
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
            json!({ "key": TOGGLES[i].0, "value": new, "via": via, "applies_to": "all" }),
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

    // -----------------------------------------------------------------------------------------
    // Rows

    fn base_index(&self, t: Target) -> Option<usize> {
        let a = &self.accounts[self.current];
        match t {
            Target::Txn(id) => a.position(id),
            Target::Draft => (self.variant != Variant::B).then_some(a.txns.len()),
        }
    }

    fn base_count(&self) -> usize {
        self.accounts[self.current].txns.len() + usize::from(self.variant != Variant::B)
    }

    /// The target of the nth base row, negative counting from the end.
    fn target_at(&self, n: i64) -> Option<Target> {
        let a = &self.accounts[self.current];
        let len = a.txns.len() as i64;
        let i = if n < 0 { len + n } else { n };
        match i {
            i if (0..len).contains(&i) => Some(Target::Txn(a.txns[i as usize].id)),
            i if i == len => Some(Target::Draft),
            _ => None,
        }
    }

    fn target_fields(&self, t: Target) -> Option<Fields> {
        match t {
            Target::Draft => Some(Fields::blank()),
            Target::Txn(id) => {
                let a = &self.accounts[self.current];
                a.position(id).map(|i| a.txns[i].f.clone())
            }
        }
    }

    /// Starts editing `t` with the cursor in `col`; the caller has already dealt with any edit.
    fn start_edit(&mut self, t: Target, col: usize, via: &str) {
        let Some(fields) = self.target_fields(t) else {
            return;
        };
        let mut e = Edit::new(t, fields);
        e.focus = Some(col);
        e.last_col = col;
        self.edit = Some(e);
        self.selected = Some(t);
        self.scroll_to = self.base_index(t);
        self.h.event(
            "edit_start",
            json!({ "target": format!("{t:?}"), "col": NAMES[col], "via": via }),
        );
    }

    /// Begins editing `t` at `col`, asking first when a changed row would be left behind.
    fn begin_edit(&mut self, ctx: &Context, t: Target, col: usize, via: &str) {
        if let Some(e) = &mut self.edit {
            if e.target == t {
                e.focus = Some(col);
                return;
            }
            self.request_leave(ctx, Leave::Row(t, col));
            return;
        }
        self.start_edit(t, col, via);
    }

    fn request_leave(&mut self, ctx: &Context, leave: Leave) {
        let Some(e) = &self.edit else {
            self.perform(ctx, leave);
            return;
        };
        if !e.dirty() {
            self.h.event(
                "leave_row",
                json!({ "to": leave.text(), "outcome": "unchanged, no prompt" }),
            );
            self.edit = None;
            self.perform(ctx, leave);
            return;
        }
        if let Err((col, msg)) = validate(&e.fields) {
            self.h.event(
                "leave_row",
                json!({ "to": leave.text(), "outcome": "refused", "message": msg }),
            );
            let e = self.edit.as_mut().unwrap();
            e.error = Some((col, msg));
            e.focus = Some(col);
            return;
        }
        if self.o[ALWAYS_SAVE] {
            self.h.event(
                "leave_row",
                json!({ "to": leave.text(), "outcome": "saved without asking" }),
            );
            if self.try_save("leave row (always save)", false) {
                self.perform(ctx, leave);
            }
            return;
        }
        self.h.event(
            "leave_row",
            json!({ "to": leave.text(), "outcome": "prompt", "changed": e.changed() }),
        );
        self.prompt_always = false;
        self.leave_prompt = Some(leave);
        self.h.screenshot("leave-prompt");
    }

    fn perform(&mut self, ctx: &Context, leave: Leave) {
        match leave {
            Leave::Row(t, c) => {
                self.edit = None;
                self.start_edit(t, c, "click");
            }
            Leave::Account(i) => {
                self.edit = None;
                self.open_account(i, "click");
            }
            Leave::Close => {
                self.closing = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
        ctx.request_repaint();
    }

    fn open_account(&mut self, i: usize, via: &str) {
        if i >= self.accounts.len() {
            return;
        }
        self.current = i;
        self.selected = None;
        self.scroll_to = Some(usize::MAX);
        self.h.event(
            "navigate",
            json!({ "to": self.accounts[i].name, "via": via }),
        );
    }

    /// Saves the row being edited as one Command. Returns whether the row is closed (saved, or
    /// nothing to save); a refused row stays open with its message.
    fn try_save(&mut self, via: &str, follow: bool) -> bool {
        let Some(e) = &mut self.edit else {
            return true;
        };
        let col = e.last_col;
        // A click on Save is deliberate, so it saves and stays; only keys move on to the
        // next row. Add still opens a fresh draft row.
        let next_row = follow && via != "button";
        if !e.dirty() {
            let target = e.target;
            self.h.event(
                "save",
                json!({ "target": format!("{target:?}"), "via": via, "unchanged": true }),
            );
            self.edit = None;
            // An unchanged save still closes the row the way a changed one does.
            if let Target::Txn(id) = target {
                self.selected = Some(target);
                if next_row {
                    self.follow_save(id, col);
                }
            }
            return true;
        }
        if let Err((col, msg)) = validate(&e.fields) {
            e.error = Some((col, msg.clone()));
            e.focus = Some(col);
            let target = e.target;
            self.h.event(
                "refused",
                json!({ "target": format!("{target:?}"), "via": via, "message": msg }),
            );
            self.status = msg;
            return false;
        }
        let e = self.edit.take().unwrap();
        let changed = e.changed();
        let a = &mut self.accounts[self.current];
        let payee = e.fields.cells[2].clone();
        let (id, command) = match e.target {
            Target::Draft => {
                let id = a.next_id;
                a.next_id += 1;
                a.txns.push(Txn::commit(id, e.fields.clone()));
                (
                    id,
                    format!(
                        "DomainCommand::AddTransaction {{ payee: {payee:?}, amount: {}, split_lines: {} }}",
                        fmt_cents(e.fields.total()),
                        e.fields.splits.as_ref().map_or(0, Vec::len)
                    ),
                )
            }
            Target::Txn(id) => {
                if let Some(i) = a.position(id) {
                    a.txns[i] = Txn::commit(id, e.fields.clone());
                }
                (
                    id,
                    format!(
                        "DomainCommand::EditTransaction {{ id: {id}, changed: {changed:?}, split_lines: {} }}",
                        e.fields.splits.as_ref().map_or(0, Vec::len)
                    ),
                )
            }
        };
        a.recompute();
        self.emit(&command, via);
        self.h.event(
            "save",
            json!({ "target": format!("{:?}", e.target), "via": via, "changed": changed }),
        );
        self.selected = Some(Target::Txn(id));
        if follow {
            if e.target == Target::Draft {
                self.start_edit(Target::Draft, 0, "after add");
                self.scroll_to = Some(usize::MAX);
            } else if next_row {
                self.follow_save(id, col);
            }
        }
        true
    }

    /// With "After Save, edit the next row" on, opens the row below saved row `id` in
    /// column `col`, the one the save came from.
    fn follow_save(&mut self, id: u32, col: usize) {
        if !self.o[NEXT_ROW] {
            return;
        }
        let a = &self.accounts[self.current];
        let next = a.position(id).map(|i| i + 1).unwrap_or(0);
        let t = if next < a.txns.len() {
            Target::Txn(a.txns[next].id)
        } else {
            Target::Draft
        };
        self.start_edit(t, col, "after save");
    }

    fn cancel(&mut self, via: &str) {
        // The row the prompt asks about is gone, so the prompt goes with it.
        if self.leave_prompt.take().is_some() {
            self.h.event("leave_prompt", json!({ "choice": "dismissed", "via": via }));
        }
        if let Some(e) = self.edit.take() {
            self.h.event(
                "cancel",
                json!({ "target": format!("{:?}", e.target), "via": via, "was_changed": e.dirty() }),
            );
            self.selected = Some(e.target)
                .filter(|t| *t != Target::Draft)
                .or(self.selected);
            self.status = "Row edit cancelled".to_owned();
        }
    }

    fn toggle_split(&mut self, via: &str) {
        let Some(e) = &mut self.edit else {
            return;
        };
        if e.fields.splits.is_none() {
            let total = e.fields.total().abs();
            let first = SplitLine {
                cat: e.fields.cells[3].clone(),
                memo: e.fields.cells[4].clone(),
                amount: if total == 0 {
                    String::new()
                } else {
                    fmt_cents(total)
                },
            };
            e.fields.splits = Some(vec![first, SplitLine::default()]);
            e.fields.cells[3] = SPLIT_LABEL.to_owned();
            e.split_open = true;
        } else {
            e.split_open = !e.split_open;
        }
        let open = e.split_open;
        self.h.event("split", json!({ "open": open, "via": via }));
        self.h
            .screenshot(if open { "split-open" } else { "split-closed" });
    }

    fn remove_split(&mut self) {
        if let Some(e) = &mut self.edit {
            if let Some(lines) = e.fields.splits.take() {
                e.fields.cells[3] = lines.first().map(|l| l.cat.clone()).unwrap_or_default();
            }
            e.split_open = false;
            self.h.event("split_removed", json!({}));
        }
    }

    fn apply_acts(&mut self, ctx: &Context) {
        for act in std::mem::take(&mut self.acts) {
            match act {
                Act::Save(via) => {
                    self.try_save(via, true);
                }
                Act::Cancel(via) => self.cancel(via),
                Act::ToggleSplit => self.toggle_split("button"),
                Act::Menu(item) => self.menu_item(ctx, item),
                Act::Begin(t, c) => self.begin_edit(ctx, t, c, "click"),
                Act::Select(t) => self.selected = Some(t),
            }
            ctx.request_repaint();
        }
    }

    fn menu_item(&mut self, ctx: &Context, item: &str) {
        let target = self.edit.as_ref().map(|e| e.target).or(self.selected);
        match item {
            "Edit" => {
                if let Some(t) = target {
                    self.begin_edit(ctx, t, 2, "menu");
                }
            }
            "Split" => {
                if let Some(t) = target {
                    if self.edit.as_ref().map(|e| e.target) != Some(t) {
                        self.begin_edit(ctx, t, 2, "menu");
                    }
                    self.toggle_split("menu");
                }
            }
            other => {
                let id = match target {
                    Some(Target::Txn(id)) => id.to_string(),
                    _ => "draft".to_owned(),
                };
                self.emit(
                    &format!("DomainCommand::{other}Transaction {{ id: {id} }}"),
                    "More actions",
                );
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Control file

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
            "account" => {
                let found = rest
                    .parse::<usize>()
                    .ok()
                    .filter(|i| *i < self.accounts.len())
                    .or_else(|| {
                        self.accounts
                            .iter()
                            .position(|a| a.name.to_lowercase().contains(&rest.to_lowercase()))
                    });
                match found {
                    Some(i) if i != self.current => self.request_leave(ctx, Leave::Account(i)),
                    Some(_) => {}
                    None => bad(self),
                }
            }
            "row" => match rest.parse::<i64>().ok().and_then(|n| self.target_at(n)) {
                Some(t) => {
                    self.selected = Some(t);
                    self.scroll_to = self.base_index(t);
                }
                None => bad(self),
            },
            "edit" => {
                let (n, col) = rest.split_once(' ').unwrap_or((rest, "payee"));
                let col = NAMES.iter().position(|c| *c == col.trim()).unwrap_or(2);
                match n.parse::<i64>().ok().and_then(|n| self.target_at(n)) {
                    Some(t) => self.begin_edit(ctx, t, col, "control"),
                    None => bad(self),
                }
            }
            "draft" => {
                self.begin_edit(ctx, Target::Draft, 0, "control");
                self.scroll_to = Some(usize::MAX);
            }
            "type" => {
                let (col, text) = rest.split_once(' ').unwrap_or((rest, ""));
                match (NAMES.iter().position(|c| *c == col), &mut self.edit) {
                    (Some(c), Some(e)) => {
                        e.fields.cells[c] = text.to_owned();
                        e.error = None;
                    }
                    _ => bad(self),
                }
            }
            "save" => {
                self.try_save("control", true);
            }
            "cancel" => self.cancel("control"),
            "split" => self.toggle_split("control"),
            "close" => self.request_leave(ctx, Leave::Close),
            "reload" => {
                self.edit = None;
                self.accounts = generate();
                self.h.event("reload", json!({ "seed": SEED }));
            }
            "screenshot" => self
                .h
                .screenshot(if rest.is_empty() { "control" } else { rest }),
            "dock" => self.set_dock(rest != "off", "control"),
            "quit" => {
                self.closing = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            _ => bad(self),
        }
    }
}

impl eframe::App for Proto {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.h.event("session_end", json!({}));
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.maximize_sent {
            self.maximize_sent = true;
            ctx.send_viewport_cmd(ViewportCommand::Maximized(true));
        }
        if let Some(cmd) = self.h.begin_frame(&ctx) {
            self.apply_control(&ctx, &cmd);
        }
        let focused = ctx.memory(|m| m.focused());
        let in_edit = focused.is_some_and(|id| self.edit_ids.contains(&id));
        if !ctx.egui_wants_keyboard_input() && self.leave_prompt.is_none() {
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

        // Ctrl+S opens Split from the editing row (Quicken's shortcut).
        if self.leave_prompt.is_none()
            && self.edit.is_some()
            && ctx.input(|i| i.modifiers.command && i.key_pressed(Key::S))
        {
            self.toggle_split("Ctrl+S");
        }

        // Esc cancels the whole row, from any of its widgets; with the Split dialog open it only
        // closes the dialog.
        if self.leave_prompt.is_none()
            && ctx.input(|i| i.key_pressed(Key::Escape))
            && self.edit.is_some()
        {
            let dialog = self.split_is_open() && matches!(self.split_view(), SplitView::Dialog);
            if dialog {
                if let Some(e) = &mut self.edit {
                    e.split_open = false;
                }
            } else if in_edit || focused.is_none() {
                // egui surrenders a text field's focus on Esc before `ui` runs, so a field's Esc
                // arrives here with nothing focused.
                self.cancel("Esc");
            }
        }
        // With nothing editing and nothing focused, the arrows move the selected row, and Enter
        // edits it.
        if self.edit.is_none() && focused.is_none() && self.leave_prompt.is_none() {
            let down = ctx.input(|i| i.key_pressed(Key::ArrowDown));
            let up = ctx.input(|i| i.key_pressed(Key::ArrowUp));
            if down || up {
                let at = self.selected.and_then(|t| self.base_index(t));
                let n = self.base_count();
                let next = match (at, down) {
                    (Some(i), true) => (i + 1).min(n - 1),
                    (Some(i), false) => i.saturating_sub(1),
                    (None, _) => n - 1,
                };
                let a = &self.accounts[self.current];
                self.selected = Some(if next < a.txns.len() {
                    Target::Txn(a.txns[next].id)
                } else {
                    Target::Draft
                });
                self.scroll_to = Some(next);
            }
            if ctx.input(|i| i.key_pressed(Key::Enter))
                && let Some(t) = self.selected
            {
                self.start_edit(t, 0, "Enter");
            }
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.closing
            && self.edit.as_ref().is_some_and(Edit::dirty)
        {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.request_leave(&ctx, Leave::Close);
        }

        self.edit_ids_now.clear();
        self.dock(ui);
        self.h.prompt_panel(ui);
        self.account_header(ui);
        self.main_ui(ui, &ctx);
        self.leave_dialog(&ctx);
        self.split_dialog(&ctx);
        self.edit_ids = std::mem::take(&mut self.edit_ids_now);
        self.apply_acts(&ctx);
        self.h.end_frame(&ctx);
    }
}

// ---------------------------------------------------------------------------------------------
// Chrome: dock, account header, dialogs

impl Proto {
    fn dock(&mut self, ui: &mut Ui) {
        let frame = {
            let v = ui.visuals();
            egui::Frame::side_top_panel(ui.style())
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
            .exact_size(330.0)
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
                ui.label(RichText::new("Variant (where the buttons sit)").strong());
                for (v, (_, hint)) in Variant::ALL.into_iter().zip(VARIANT_KEYS) {
                    let r =
                        ui.selectable_label(self.variant == v, format!("{hint}   {}", v.label()));
                    self.h.track(&r, format!("dock: variant {}", v.key()));
                    if r.clicked() {
                        self.set_variant(v, "click");
                    }
                }
                ui.separator();
                ui.label(RichText::new("Toggles (all variants)").strong());
                for (i, ((key, label, _), (_, hint))) in
                    TOGGLES.iter().zip(TOGGLE_KEYS).enumerate()
                {
                    let mut value = self.o[i];
                    let r = ui.checkbox(&mut value, format!("{hint}   {label}"));
                    self.h.track(&r, format!("dock: toggle {key}"));
                    if r.changed() {
                        self.set_toggle(key, Some(value), "click");
                    }
                }
                ui.separator();
                self.h.dock_spotlight(ui);
                self.h.dock_prompt(ui);
                ui.separator();
                let r = ui.button("Ask to close the window");
                self.h.track(&r, "dock: close window");
                if r.clicked() {
                    let ctx = ui.ctx().clone();
                    self.request_leave(&ctx, Leave::Close);
                }
                ui.separator();
                ui.label(
                    RichText::new(
                        "A / B / C or left / right variant · 1–7 toggles · F2 note · F3 marker · F4 hide this dock · F5 spotlight · F6 prompt",
                    )
                    .small()
                    .weak(),
                );
            });
    }

    fn account_header(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        Panel::top("account_header").show(ui, |ui| {
            ui.horizontal(|ui| {
                for i in 0..self.accounts.len() {
                    let name = self.accounts[i].name;
                    let r = ui.selectable_label(self.current == i, name);
                    self.h.track(&r, format!("account: {name}"));
                    if r.clicked() && self.current != i {
                        self.request_leave(&ctx, Leave::Account(i));
                    }
                }
                ui.separator();
                let a = &self.accounts[self.current];
                let ending = a.balances.last().copied().unwrap_or(0);
                ui.label(format!("{}   Ending balance {}", a.name, fmt_cents(ending)));
            });
        });
    }

    fn leave_dialog(&mut self, ctx: &Context) {
        let Some(leave) = self.leave_prompt else {
            return;
        };
        if self.edit.is_none() {
            self.leave_prompt = None;
            return;
        }
        enum Choice {
            Save,
            Continue,
            Discard,
        }
        let (payee, changed) = self
            .edit
            .as_ref()
            .map(|e| (e.fields.cells[2].clone(), e.changed().join(", ")))
            .unwrap_or_default();
        let mut choice = None;
        let mut always = self.prompt_always;
        let modal = egui::Modal::new(Id::new("leave_prompt")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading("Save this transaction?");
            ui.label(format!(
                "You changed {changed} on “{payee}”. Save the row before you leave it?"
            ));
            ui.add_space(6.0);
            let r = ui.checkbox(&mut always, "Always save without asking");
            self.h.track(&r, "leave prompt: always save");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let r = ui.add(Button::new("Save").min_size(vec2(90.0, 0.0)));
                self.h.track(&r, "leave prompt: Save");
                let enter = ctx.input(|i| i.key_pressed(Key::Enter));
                let to_continue = self.o[PROMPT_CONTINUE];
                if r.clicked() || (enter && !to_continue) {
                    choice = Some(Choice::Save);
                }
                let r = ui.button("Continue editing");
                self.h.track(&r, "leave prompt: Continue editing");
                if r.clicked() || (enter && to_continue) {
                    choice = Some(Choice::Continue);
                }
                let r = ui.button("Discard");
                self.h.track(&r, "leave prompt: Discard");
                if r.clicked() {
                    choice = Some(Choice::Discard);
                }
            });
        });
        self.prompt_always = always;
        if choice.is_none() && modal.should_close() {
            choice = Some(Choice::Continue);
        }
        let Some(choice) = choice else {
            return;
        };
        self.leave_prompt = None;
        match choice {
            Choice::Save => {
                self.h.event(
                    "leave_prompt",
                    json!({ "choice": "save", "always_save_checked": always }),
                );
                if always {
                    self.o[ALWAYS_SAVE] = true;
                    self.h.event(
                        "toggle",
                        json!({ "key": "always_save", "value": true, "via": "leave prompt checkbox" }),
                    );
                }
                if self.try_save("leave prompt", false) {
                    self.perform(ctx, leave);
                }
            }
            Choice::Discard => {
                self.h.event("leave_prompt", json!({ "choice": "discard" }));
                self.edit = None;
                self.perform(ctx, leave);
            }
            Choice::Continue => {
                self.h
                    .event("leave_prompt", json!({ "choice": "continue editing" }));
                if let Some(e) = &mut self.edit {
                    e.focus = Some(e.last_col);
                }
            }
        }
    }

    /// The Split lines in a window, when they aren't shown under the row or in the bar.
    fn split_dialog(&mut self, ctx: &Context) {
        if !self.split_is_open() || !matches!(self.split_view(), SplitView::Dialog) {
            return;
        }
        egui::Window::new("Split transaction")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                self.split_grid(ui, "dialog");
            });
    }
}

// ---------------------------------------------------------------------------------------------
// Main area: register, status strip, entry bar

impl Proto {
    fn bar_height(&self) -> f32 {
        if !self.has_bar() {
            return 0.0;
        }
        let mut h = BAR_BASE;
        if self.split_is_open() && matches!(self.split_view(), SplitView::Bar) {
            let n = self
                .edit
                .as_ref()
                .and_then(|e| e.fields.splits.as_ref())
                .map_or(0, Vec::len);
            h += 30.0 * (n as f32 + 2.0);
        }
        h
    }

    fn main_ui(&mut self, ui: &mut Ui, ctx: &Context) {
        CentralPanel::default().show(ui, |ui| {
            let full = ui.available_rect_before_wrap();
            let bar_h = self.bar_height();
            let table_rect =
                Rect::from_min_max(full.min, pos2(full.max.x, full.max.y - bar_h - STATUS_H));
            let status_rect = Rect::from_min_max(
                pos2(full.min.x, table_rect.max.y),
                pos2(full.max.x, table_rect.max.y + STATUS_H),
            );
            let bar_rect = Rect::from_min_max(pos2(full.min.x, status_rect.max.y), full.max);
            self.h.region(table_rect, "register");
            ui.scope_builder(UiBuilder::new().max_rect(table_rect), |ui| {
                self.table(ui, ctx);
            });
            ui.scope_builder(UiBuilder::new().max_rect(status_rect), |ui| {
                self.status_strip(ui);
            });
            if self.has_bar() {
                self.h.region(bar_rect, "entry bar");
                ui.scope_builder(UiBuilder::new().max_rect(bar_rect), |ui| {
                    self.entry_bar(ui, bar_rect);
                });
            }
        });
    }

    fn status_strip(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            let error = self.edit.as_ref().and_then(|e| e.error.clone());
            if let Some((_, msg)) = error {
                ui.label(
                    RichText::new(format!("Can't save this row: {msg}"))
                        .color(RED)
                        .strong(),
                );
            } else {
                ui.label(RichText::new(&self.status).weak());
            }
        });
    }

    fn inline_split_rows(&self) -> usize {
        if self.variant != Variant::B
            && self.o[SPLIT_INLINE]
            && let Some(e) = &self.edit
            && e.split_open
            && let Some(lines) = &e.fields.splits
        {
            lines.len() + 1
        } else {
            0
        }
    }

    fn table(&mut self, ui: &mut Ui, _ctx: &Context) {
        let row_h = self.theme.row_height();
        let n = self.accounts[self.current].txns.len();
        let has_draft = self.variant != Variant::B;
        let k = self.inline_split_rows();
        let er = self
            .edit
            .as_ref()
            .and_then(|e| self.base_index(e.target))
            .filter(|_| k > 0);
        let total = n + usize::from(has_draft) + k;
        let actions_w = if self.variant == Variant::B {
            80.0
        } else {
            300.0
        };
        let mut table = TableBuilder::new(ui)
            .id_salt(("register", self.current))
            .striped(self.theme.settings().alternating_rows)
            .resizable(false)
            .sense(Sense::CLICK) // clickable, not a Tab stop between the editing row's fields
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(84.0))
            .column(Column::exact(54.0))
            .column(Column::initial(200.0).at_least(80.0))
            .column(Column::initial(170.0).at_least(80.0))
            .column(Column::remainder().at_least(80.0))
            .column(Column::exact(92.0))
            .column(Column::exact(92.0))
            .column(Column::exact(104.0))
            .column(Column::exact(actions_w));
        if let Some(r) = self.scroll_to.take() {
            table = table.scroll_to_row(r.min(total.saturating_sub(1)), Some(Align::Center));
        }
        table
            .header(row_h + 2.0, |mut header| {
                for label in LABELS {
                    header.col(|ui| {
                        if label == "Payment" || label == "Deposit" {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.strong(label);
                            });
                        } else {
                            ui.strong(label);
                        }
                    });
                }
                header.col(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.strong("Balance");
                    });
                });
                // Quicken leaves the Actions column's header blank.
                header.col(|_| {});
            })
            .body(|body| {
                body.rows(row_h, total, |mut row| {
                    let v = row.index();
                    let base = |i: usize| if i < n { Vis::Txn(i) } else { Vis::Draft };
                    let vis = match er {
                        Some(er) if v > er => {
                            if v <= er + k {
                                Vis::Split(v - er - 1)
                            } else {
                                base(v - k)
                            }
                        }
                        _ => base(v),
                    };
                    match vis {
                        Vis::Split(j) => self.split_row(&mut row, j),
                        Vis::Txn(_) | Vis::Draft => self.txn_row(&mut row, vis),
                    }
                });
            });
    }

    fn txn_row(&mut self, row: &mut TableRow<'_, '_>, vis: Vis) {
        let a = &self.accounts[self.current];
        let (target, cells, balance, index) = match vis {
            Vis::Txn(i) => {
                let t = &a.txns[i];
                (Target::Txn(t.id), t.f.cells.clone(), Some(a.balances[i]), i)
            }
            _ => (Target::Draft, Fields::default().cells, None, a.txns.len()),
        };
        let editing = self.edit.as_ref().is_some_and(|e| e.target == target);
        let in_place = editing && self.variant != Variant::B;
        let selected = self.selected == Some(target);
        row.set_selected(selected && !editing);
        let highlight = self.theme_highlight();
        let mut clicked = None;
        for c in 0..7 {
            let (_, resp) = row.col(|ui| {
                if editing {
                    ui.painter()
                        .rect_filled(ui.max_rect().expand2(vec2(4.0, 0.0)), 0.0, highlight);
                }
                if in_place {
                    let splitting = self
                        .edit
                        .as_ref()
                        .is_some_and(|e| e.fields.splits.is_some());
                    if c == 3 && splitting {
                        ui.label(RichText::new(SPLIT_LABEL).italics());
                    } else {
                        let w = ui.available_width();
                        self.field(
                            ui,
                            Surface::Row,
                            c,
                            w,
                            format!("{} cell, row {index} (editing)", NAMES[c]),
                        );
                    }
                } else {
                    let text = if target == Target::Draft && c == 2 {
                        RichText::new("New transaction…").weak().italics()
                    } else {
                        RichText::new(&cells[c])
                    };
                    if c >= 5 {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(text);
                        });
                    } else {
                        ui.add(egui::Label::new(text).truncate());
                    }
                }
            });
            if !in_place {
                self.h
                    .track(&resp, format!("{} cell, row {index}", NAMES[c]));
                if resp.clicked() {
                    clicked = Some(c);
                }
            }
        }
        let (_, resp) = row.col(|ui| {
            if editing {
                ui.painter()
                    .rect_filled(ui.max_rect().expand2(vec2(4.0, 0.0)), 0.0, highlight);
            }
            if let Some(b) = balance {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(fmt_cents(b));
                });
            }
        });
        if !in_place && resp.clicked() {
            clicked = Some(2);
        }
        let (_, resp) = row.col(|ui| {
            if editing {
                ui.painter()
                    .rect_filled(ui.max_rect().expand2(vec2(4.0, 0.0)), 0.0, highlight);
            }
            if in_place {
                // Leave room inside the row for the focus outline, which the cell clips.
                ui.spacing_mut().button_padding.y = 1.0;
                ui.spacing_mut().interact_size.y = self.theme.row_height() - 6.0;
                self.edit_buttons(ui, Surface::Row, Which::Full);
            } else if selected && target != Target::Draft {
                let r = ui.menu_button("Actions…", |ui| {
                    for item in ["Edit", "Split", "Duplicate", "Void", "Delete"] {
                        let r = ui.button(item);
                        self.h.track(&r, format!("actions menu: {item}"));
                        if r.clicked() {
                            self.acts.push(Act::Menu(item));
                            ui.close();
                        }
                    }
                });
                self.h
                    .track(&r.response, format!("actions cell, row {index}"));
            }
        });
        if !in_place && resp.clicked() && !selected {
            self.acts.push(Act::Select(target));
        }
        if let Some(c) = clicked {
            self.acts.push(Act::Begin(target, c));
        }
    }

    fn theme_highlight(&self) -> Color32 {
        egui::Color32::from_rgba_unmultiplied(0x3a, 0x7b, 0xd5, 38)
    }

    /// A sub-row under the editing row: one split line, or the footer after the last.
    fn split_row(&mut self, row: &mut TableRow<'_, '_>, j: usize) {
        let lines = self
            .edit
            .as_ref()
            .and_then(|e| e.fields.splits.as_ref())
            .map_or(0, Vec::len);
        let pay = self.edit.as_ref().is_some_and(|e| e.fields.inn() == 0);
        let highlight = self.theme_highlight();
        let foot = j >= lines;
        for c in 0..9 {
            row.col(|ui| {
                ui.painter()
                    .rect_filled(ui.max_rect().expand2(vec2(4.0, 0.0)), 0.0, highlight);
                match (c, foot) {
                    (3, false) => self.split_cell(ui, j, 0, "table"),
                    (4, false) => self.split_cell(ui, j, 1, "table"),
                    (5, false) if pay => self.split_cell(ui, j, 2, "table"),
                    (6, false) if !pay => self.split_cell(ui, j, 2, "table"),
                    (8, false) => {
                        let r = ui.small_button("Remove line");
                        self.h.track(&r, format!("split line {j}: remove"));
                        self.edit_ids_now.insert(r.id);
                        if r.clicked() {
                            if let Some(l) =
                                self.edit.as_mut().and_then(|e| e.fields.splits.as_mut())
                            {
                                l.remove(j);
                            }
                        }
                    }
                    (3, true) => {
                        let r = ui.button("Add line");
                        self.h.track(&r, "split: add line");
                        self.edit_ids_now.insert(r.id);
                        if r.clicked() {
                            if let Some(l) =
                                self.edit.as_mut().and_then(|e| e.fields.splits.as_mut())
                            {
                                l.push(SplitLine::default());
                            }
                        }
                    }
                    (4, true) => {
                        ui.label(self.remaining_text());
                    }
                    (8, true) => {
                        ui.horizontal(|ui| {
                            let r = ui.small_button("Remove split");
                            self.h.track(&r, "split: remove split");
                            self.edit_ids_now.insert(r.id);
                            if r.clicked() {
                                self.remove_split();
                            }
                            let r = ui.small_button("Hide");
                            self.h.track(&r, "split: hide lines");
                            self.edit_ids_now.insert(r.id);
                            if r.clicked() {
                                if let Some(e) = &mut self.edit {
                                    e.split_open = false;
                                }
                            }
                        });
                    }
                    _ => {}
                }
            });
        }
    }

    fn remaining_text(&self) -> RichText {
        let Some(e) = &self.edit else {
            return RichText::new("");
        };
        let total = e.fields.total().abs();
        let sum = e.fields.splits.as_deref().map_or(0, lines_total);
        let rem = total - sum;
        let text = RichText::new(format!("Remaining {}", fmt_cents(rem)));
        if rem == 0 {
            text.weak()
        } else {
            text.color(RED)
        }
    }

    /// The split lines as a grid, for the dialog and for the entry bar.
    fn split_grid(&mut self, ui: &mut Ui, tag: &str) {
        let pay = self.edit.as_ref().is_some_and(|e| e.fields.inn() == 0);
        let n = self
            .edit
            .as_ref()
            .and_then(|e| e.fields.splits.as_ref())
            .map_or(0, Vec::len);
        egui::Grid::new(("split_grid", tag))
            .num_columns(4)
            .show(ui, |ui| {
                ui.strong("Category");
                ui.strong("Memo");
                ui.strong(if pay { "Payment" } else { "Deposit" });
                ui.label("");
                ui.end_row();
                for j in 0..n {
                    self.split_cell(ui, j, 0, tag);
                    self.split_cell(ui, j, 1, tag);
                    self.split_cell(ui, j, 2, tag);
                    let r = ui.small_button("Remove line");
                    self.h.track(&r, format!("split line {j}: remove"));
                    self.edit_ids_now.insert(r.id);
                    if r.clicked() {
                        if let Some(l) = self.edit.as_mut().and_then(|e| e.fields.splits.as_mut()) {
                            l.remove(j);
                        }
                        break;
                    }
                    ui.end_row();
                }
            });
        ui.horizontal(|ui| {
            let r = ui.button("Add line");
            self.h.track(&r, "split: add line");
            self.edit_ids_now.insert(r.id);
            if r.clicked() {
                if let Some(l) = self.edit.as_mut().and_then(|e| e.fields.splits.as_mut()) {
                    l.push(SplitLine::default());
                }
            }
            ui.label(self.remaining_text());
            let r = ui.button("Remove split");
            self.h.track(&r, "split: remove split");
            self.edit_ids_now.insert(r.id);
            if r.clicked() {
                self.remove_split();
            }
            let r = ui.button("Done");
            self.h.track(&r, "split: done");
            self.edit_ids_now.insert(r.id);
            if r.clicked() {
                if let Some(e) = &mut self.edit {
                    e.split_open = false;
                }
            }
        });
    }

    fn split_cell(&mut self, ui: &mut Ui, j: usize, k: usize, tag: &str) {
        let id = Id::new(("split_cell", tag, j, k));
        let Some(mut text) = self
            .edit
            .as_ref()
            .and_then(|e| e.fields.splits.as_ref())
            .and_then(|s| s.get(j))
            .map(|l| match k {
                0 => l.cat.clone(),
                1 => l.memo.clone(),
                _ => l.amount.clone(),
            })
        else {
            return;
        };
        let mut te = TextEdit::singleline(&mut text)
            .id(id)
            .desired_width(if tag == "table" {
                ui.available_width()
            } else {
                150.0
            });
        if k == 2 {
            te = te.horizontal_align(Align::Max);
        }
        // A Grid in an auto-sized Window shrinks its fields below `desired_width`, so the
        // dialog and the bar give each column a fixed width.
        let r = if tag == "table" {
            ui.add(te)
        } else {
            ui.add_sized([[200.0, 240.0, 100.0][k], ui.spacing().interact_size.y], te)
        };
        self.edit_ids_now.insert(id);
        self.h.track(
            &r,
            format!("split line {j} {}", ["category", "memo", "amount"][k]),
        );
        if r.changed() {
            if let Some(l) = self
                .edit
                .as_mut()
                .and_then(|e| e.fields.splits.as_mut())
                .and_then(|s| s.get_mut(j))
            {
                match k {
                    0 => l.cat = text,
                    1 => l.memo = text,
                    _ => l.amount = text,
                }
                if let Some(e) = &mut self.edit {
                    e.error = None;
                }
            }
        }
    }

    /// One of the row's seven cells as a text field, on `surface`, bound to the pending edit.
    fn field(&mut self, ui: &mut Ui, surface: Surface, c: usize, width: f32, name: String) {
        let ctx = ui.ctx().clone();
        let id = Id::new(("edit_field", surface as u8, c));
        let mut text = self
            .edit
            .as_ref()
            .map_or_else(String::new, |e| e.fields.cells[c].clone());
        let focus_wanted =
            surface == self.active_surface() && self.edit.as_ref().and_then(|e| e.focus) == Some(c);
        if focus_wanted {
            ctx.memory_mut(|m| m.request_focus(id));
            if let Some(e) = &mut self.edit {
                e.focus = None;
            }
        }
        let mut te = TextEdit::singleline(&mut text).id(id).desired_width(width);
        if c >= 5 {
            te = te.horizontal_align(Align::Max);
        }
        let out = te.show(ui);
        let r = out.response;
        if focus_wanted {
            let mut state = out.state;
            let len = text.chars().count();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                )));
            state.store(&ctx, id);
        }
        self.edit_ids_now.insert(id);
        self.h.track(&r, name);
        if (r.gained_focus() || r.changed()) && self.edit.is_none() {
            self.start_edit(Target::Draft, c, "bar");
            if let Some(e) = &mut self.edit {
                e.focus = None;
            }
        }
        if r.changed() {
            if let Some(e) = &mut self.edit {
                e.fields.cells[c] = text;
                e.error = None;
            }
        }
        if r.has_focus() {
            if let Some(e) = &mut self.edit {
                e.last_col = c;
            }
        }
        let errored = self
            .edit
            .as_ref()
            .and_then(|e| e.error.as_ref())
            .is_some_and(|(col, _)| *col == c);
        if errored {
            ui.painter()
                .rect_stroke(r.rect, 2.0, Stroke::new(1.5, RED), StrokeKind::Outside);
        }
        if r.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter)) && self.leave_prompt.is_none()
        {
            let ctrl = ctx.input(|i| i.modifiers.command);
            if self.o[ENTER_ANY] || c == LAST || ctrl {
                self.acts.push(Act::Save("Enter"));
            } else if let Some(e) = &mut self.edit {
                e.focus = Some(c + 1);
            }
        }
        if !self.o[TAB_BUTTONS] && (c == 0 || c == LAST) && r.has_focus() {
            ctx.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: false,
                        vertical_arrows: false,
                        escape: false,
                    },
                );
            });
            let (tab, shift) = ctx.input(|i| (i.key_pressed(Key::Tab), i.modifiers.shift));
            if tab && let Some(e) = &mut self.edit {
                e.focus = Some(match (c, shift) {
                    (0, false) => 1,
                    (0, true) => LAST,
                    (_, false) => 0,
                    (_, true) => LAST - 1,
                });
                ctx.request_repaint();
            }
        }
    }

    fn edit_buttons(&mut self, ui: &mut Ui, surface: Surface, which: Which) {
        let draft = self
            .edit
            .as_ref()
            .is_some_and(|e| e.target == Target::Draft);
        let tag = match surface {
            Surface::Row => "actions cell",
            Surface::Bar => "entry bar",
        };
        let save = if draft { "Add" } else { "Save" };
        let r = ui.button(save);
        self.edit_ids_now.insert(r.id);
        self.h
            .track(&r, format!("{} button ({tag})", save.to_lowercase()));
        if r.clicked() {
            self.acts.push(Act::Save("button"));
        }
        let r = ui.button("Cancel");
        self.edit_ids_now.insert(r.id);
        self.h.track(&r, format!("cancel button ({tag})"));
        if r.clicked() {
            self.acts.push(Act::Cancel("button"));
        }
        if which == Which::Full {
            let r = ui.button("Split");
            self.edit_ids_now.insert(r.id);
            self.h.track(&r, format!("split button ({tag})"));
            if r.clicked() {
                self.acts.push(Act::ToggleSplit);
            }
            let items: &[&'static str] = if self.o[TRIM_MORE] {
                &["Duplicate", "Void", "Delete"]
            } else {
                &["Edit", "Split", "Duplicate", "Void", "Delete"]
            };
            let r = ui.menu_button("More ⏷", |ui| {
                for item in items {
                    let r = ui.button(*item);
                    self.h.track(&r, format!("more actions menu: {item}"));
                    if r.clicked() {
                        self.acts.push(Act::Menu(item));
                        ui.close();
                    }
                }
            });
            self.edit_ids_now.insert(r.response.id);
            self.h
                .track(&r.response, format!("more actions button ({tag})"));
        }
    }

    fn entry_bar(&mut self, ui: &mut Ui, rect: Rect) {
        ui.painter()
            .rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        ui.painter().hline(
            rect.x_range(),
            rect.top(),
            Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        );
        let title = match &self.edit {
            None => "New transaction".to_owned(),
            Some(e) if e.target == Target::Draft => "New transaction".to_owned(),
            Some(e) => format!("Editing “{}”", e.fields.cells[2]),
        };
        let dirty = self.edit.as_ref().is_some_and(Edit::dirty);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.label(RichText::new(title).strong());
            if dirty {
                ui.label(RichText::new("· changed").weak());
            }
        });
        let buttons_w = if self.variant == Variant::B {
            330.0
        } else {
            150.0
        };
        let fixed = [84.0, 54.0, 190.0, 160.0, 0.0, 92.0, 92.0];
        let gaps = 8.0 * 7.0;
        let memo_w = (rect.width() - 12.0 - fixed.iter().sum::<f32>() - gaps - buttons_w).max(80.0);
        let splitting = self
            .edit
            .as_ref()
            .is_some_and(|e| e.fields.splits.is_some());
        ui.horizontal_top(|ui| {
            ui.add_space(6.0);
            for c in 0..7 {
                let w = if c == 4 { memo_w } else { fixed[c] };
                ui.vertical(|ui| {
                    ui.set_width(w);
                    ui.label(RichText::new(LABELS[c]).small().weak());
                    if c == 3 && splitting {
                        ui.label(RichText::new(SPLIT_LABEL).italics());
                    } else {
                        self.field(ui, Surface::Bar, c, w, format!("entry bar: {}", NAMES[c]));
                    }
                });
            }
            ui.vertical(|ui| {
                ui.label("");
                ui.horizontal(|ui| {
                    let which = if self.variant == Variant::B {
                        Which::Full
                    } else {
                        Which::Short
                    };
                    self.edit_buttons(ui, Surface::Bar, which);
                });
            });
        });
        if self.split_is_open() && matches!(self.split_view(), SplitView::Bar) {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.vertical(|ui| self.split_grid(ui, "bar"));
            });
        }
    }
}

fn main() -> eframe::Result {
    // Prints the stub figures and exits, so they can be checked without a window.
    if std::env::var_os("ROW_EDIT_PROTO_DUMP").is_some() {
        for a in generate() {
            let min = a.balances.iter().min().copied().unwrap_or(0);
            let max = a.balances.iter().max().copied().unwrap_or(0);
            let splits = a.txns.iter().filter(|t| t.f.splits.is_some()).count();
            println!(
                "{:<10} rows {:>5} splits {:>3} ending {:>12} min {:>12} max {:>12}",
                a.name,
                a.txns.len(),
                splits,
                fmt_cents(a.balances.last().copied().unwrap_or(0)),
                fmt_cents(min),
                fmt_cents(max),
            );
        }
        return Ok(());
    }
    let args = Args::from_env();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Balanced — PROTOTYPE row-level editing")
            .with_inner_size([1600.0, 1000.0])
            .with_maximized(true),
        ..Default::default()
    };
    eframe::run_native(
        "balanced_row_edit_prototype",
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
