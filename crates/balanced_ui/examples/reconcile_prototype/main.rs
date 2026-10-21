// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! PROTOTYPE, throwaway: the Reconcile window.
//!
//! Question (ticket #374): what shape should the Reconcile window take? Grilling #216 settled the
//! behaviour; this settles only the layout.
//!
//! Applied from earlier decisions: a tick is a real Cleared <-> Uncleared change, closing is
//! "finish later", the statement in progress is remembered per account, Opening is the sum of
//! Reconciled transactions, Difference = ending - Cleared, Finish with a Difference offers a
//! Balance Adjustment, later Cleared transactions are listed, card balances are typed as printed
//! (#216); Dialogs are in-window overlays that dim the window, heavy ones movable, all app-modal,
//! and there is no second OS window in v1 (#19); split Payment/Deposit columns, the Clr column,
//! balances in the context header, Account Actions next to + Transaction (#218); the real theme.
//!
//! Three variants differ in where Reconcile lives:
//!   A  Heavy Dialog with two lists (Quicken Windows' layout), dimming the Register
//!   B  Register mode: the Register filters to the reconcile set under a reconcile bar
//!   C  Docked side panel: one combined list beside the whole Register
//!
//! Toggles (1-6) are listed in `TOGGLES`, each tagged with the variants it applies to.
//!
//! Run:
//!   cargo run -p balanced_ui --example reconcile_prototype -- \
//!     [--variant a|b|c] [--log <dir>/events.jsonl] [--control <dir>/control.txt]
//!
//! Control-file verbs (besides the harness's `prompt` and `spotlight`): `variant <key>`,
//! `toggle <key|number> [on|off|<option number>]`, `screenshot [label]`, `dock on|off`, `reload`,
//! `quit`, `account <checking|visa>`, `reconcile` (opens the Start Dialog), `date <text>`,
//! `ending <text>` (fill the Start Dialog), `ok`, `cancel` (the top Dialog's buttons),
//! `tick <stmt|all|id|payee>`, `untick <all|id|payee>`, `select <id|payee>`, `edit`, `new`,
//! `amount <text>` (the edit stand-in's field), `category <name>` (the adjustment's),
//! `finish`, `later`.
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
    clippy::absolute_paths,
    clippy::single_match_else,
    clippy::manual_let_else,
    clippy::option_if_let_else,
    clippy::unnecessary_wraps,
    clippy::if_not_else,
    dead_code,
    missing_debug_implementations,
    unreachable_pub
)]

use balanced_prototype_harness::{Args, Harness};
use eframe::egui;
use egui::{
    Align, CentralPanel, Color32, Context, Id, Key, Layout, Order, Panel, RichText, Sense, Sides,
    Stroke, TextEdit, Ui, ViewportBuilder, ViewportCommand, vec2,
};
use egui_extras::{Column, TableBuilder};
use serde_json::{Value, json};

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

/// 2026-10-21, as days since 1970-01-01.
const TODAY: i64 = 20_747;
const START: i64 = TODAY - 1_095;
/// Checking's last reconciled statement (9/15/2026) and the one in hand (10/15/2026).
const CK_LAST: i64 = 20_711;
const CK_STMT: i64 = 20_741;
/// Visa's last reconciled statement (9/5/2026) and the one in progress (10/5/2026).
const VS_LAST: i64 = 20_701;
const VS_STMT: i64 = 20_731;

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

/// Howard Hinnant's `days_from_civil`.
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

/// `m/d/yyyy`, `m/d/yy` or `m/d` (this year).
fn parse_day(s: &str) -> Option<i64> {
    let parts: Vec<&str> = s.trim().split('/').collect();
    let num = |p: &str| p.trim().parse::<i64>().ok();
    let (m, d, y) = match parts.as_slice() {
        [m, d] => (num(m)?, num(d)?, civil(TODAY).0),
        [m, d, y] => {
            let y = num(y)?;
            (num(m)?, num(d)?, if y < 100 { 2000 + y } else { y })
        }
        _ => return None,
    };
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

/// `1,234.56`, `-12`, `$5.5`: cents, or `None`.
fn parse_cents(s: &str) -> Option<i64> {
    let t: String = s
        .chars()
        .filter(|c| !matches!(c, ',' | '$' | ' '))
        .collect();
    let (neg, t) = match t.strip_prefix('-') {
        Some(rest) => (true, rest.to_owned()),
        None => (false, t),
    };
    if t.is_empty() {
        return None;
    }
    let (whole, frac) = t.split_once('.').unwrap_or((&t, ""));
    if frac.len() > 2 || !whole.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !frac.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let whole: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let frac: i64 = format!("{frac:0<2}").parse().ok()?;
    let v = whole * 100 + frac;
    Some(if neg { -v } else { v })
}

// ---------------------------------------------------------------------------------------------
// Stub data

const SEED: u64 = 0x00ec_0c11_e374;

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

    /// Roughly `typical` cents, 40% to 160%.
    fn around(&mut self, typical: i64) -> i64 {
        let spread = (typical * 6 / 5).max(1) as u64;
        (typical * 2 / 5 + self.below(spread) as i64).max(1)
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Clr {
    Uncleared,
    Cleared,
    Reconciled,
}

#[derive(Clone, Debug)]
struct Txn {
    id: u32,
    day: i64,
    num: Option<u32>,
    payee: String,
    cat: String,
    /// Signed in the account's own sense: money in is positive; a card charge is negative.
    cents: i64,
    clr: Clr,
    /// The amount the paper statement shows for it, when the statement lists it.
    on_paper: Option<i64>,
}

#[derive(Clone)]
struct Account {
    name: &'static str,
    card: bool,
    txns: Vec<Txn>,
    /// The statement in progress: (date, ending balance as typed: positive owed on a card).
    stmt: Option<(i64, i64)>,
    /// The paper statement the user holds: (date, ending balance as printed).
    paper: (i64, i64),
    paper_note: String,
}

impl Account {
    fn sum(&self, f: impl Fn(&Txn) -> bool) -> i64 {
        self.txns.iter().filter(|t| f(t)).map(|t| t.cents).sum()
    }

    fn opening(&self) -> i64 {
        self.sum(|t| t.clr == Clr::Reconciled)
    }

    fn cleared(&self) -> i64 {
        self.sum(|t| t.clr != Clr::Uncleared)
    }

    fn current(&self) -> i64 {
        self.sum(|t| t.day <= TODAY)
    }

    fn ending(&self) -> i64 {
        self.sum(|_| true)
    }

    /// A balance as the user reads it on a statement: a card's is positive when owed.
    fn shown(&self, cents: i64) -> i64 {
        if self.card { -cents } else { cents }
    }

    fn in_set(&self, t: &Txn, later_uncleared: bool) -> bool {
        let Some((day, _)) = self.stmt else {
            return false;
        };
        t.clr != Clr::Reconciled && (t.day <= day || t.clr == Clr::Cleared || later_uncleared)
    }

    /// Difference = statement ending - Cleared balance, both as the user reads them.
    fn difference(&self) -> Option<i64> {
        self.stmt.map(|(_, end)| end - self.shown(self.cleared()))
    }

    /// (count, total) of ticked transactions in the set, money out or money in.
    fn ticked(&self, later: bool, out: bool) -> (usize, i64) {
        let v: Vec<i64> = self
            .txns
            .iter()
            .filter(|t| self.in_set(t, later) && t.clr == Clr::Cleared && (t.cents < 0) == out)
            .map(|t| t.cents)
            .collect();
        (v.len(), v.iter().sum())
    }

    fn idx(&self, id: u32) -> Option<usize> {
        self.txns.iter().position(|t| t.id == id)
    }

    fn out_label(&self) -> &'static str {
        if self.card { "Charges" } else { "Payments" }
    }

    fn in_label(&self) -> &'static str {
        if self.card { "Payments" } else { "Deposits" }
    }

    fn ending_label(&self) -> &'static str {
        if self.card {
            "Statement balance owed"
        } else {
            "Statement ending balance"
        }
    }

    fn sort(&mut self) {
        self.txns.sort_by_key(|t| (t.day, t.id));
    }
}

fn generate() -> Vec<Account> {
    let mut rng = Lcg(SEED);
    let mut id = 0_u32;
    let mut next_id = || {
        id += 1;
        id
    };
    let txn = |id: u32, day: i64, payee: &str, cat: &str, cents: i64| Txn {
        id,
        day,
        num: None,
        payee: payee.to_owned(),
        cat: cat.to_owned(),
        cents,
        clr: Clr::Uncleared,
        on_paper: None,
    };

    // Visa first: Checking pays its statements.
    const MERCHANTS: &[(&str, &str, i64)] = &[
        ("Amazon", "Shopping", 3_800),
        ("Whole Foods", "Groceries", 6_400),
        ("Shell", "Auto:Fuel", 4_500),
        ("Starbucks", "Dining", 650),
        ("Target", "Household", 5_200),
        ("Uber", "Travel", 2_300),
        ("Chipotle", "Dining", 1_400),
        ("Home Depot", "Home:Repairs", 7_800),
        ("CVS Pharmacy", "Medical", 2_100),
    ];
    let mut visa = Vec::new();
    let mut payments = Vec::new();
    let mut bal = 0_i64;
    let mut stmt_bal = 0_i64;
    for day in START..=TODAY {
        let (_, _, d) = civil(day);
        if d == 25 && stmt_bal < 0 {
            let p = -stmt_bal;
            visa.push(txn(next_id(), day, "Payment - Thank You", "[Checking]", p));
            payments.push((day, p));
            bal += p;
        }
        if d == 3 {
            visa.push(txn(next_id(), day, "Netflix", "Entertainment", -1_549));
            bal -= 1_549;
        }
        if d == 11 {
            visa.push(txn(next_id(), day, "Spotify", "Entertainment", -1_199));
            bal -= 1_199;
        }
        for _ in 0..(u64::from(rng.below(100) < 70) + u64::from(rng.below(100) < 25)) {
            let (p, c, typical) = *rng.pick(MERCHANTS);
            let cents = -rng.around(typical);
            visa.push(txn(next_id(), day, p, c, cents));
            bal += cents;
        }
        if d == 5 {
            stmt_bal = bal;
        }
    }

    // Checking.
    const DINING: &[&str] = &["Olive Garden", "Panera Bread", "Thai Basil", "Five Guys"];
    const MISC: &[(&str, &str, i64)] = &[
        ("Walgreens", "Medical", 2_500),
        ("Petco", "Pets", 3_500),
        ("USPS", "Postage", 900),
        ("Barnes & Noble", "Books", 2_800),
        ("Ace Hardware", "Home:Repairs", 3_100),
    ];
    const CHECKS: &[(&str, &str, i64)] = &[
        ("Lincoln Elementary PTA", "Education", 6_000),
        ("Dr. Patel Dental", "Medical:Dental", 15_000),
        ("Green Lawn Care", "Home:Yard", 7_500),
    ];
    let mut ck = vec![txn(
        next_id(),
        START,
        "Opening Balance",
        "Opening Balance",
        420_000,
    )];
    let mut is_check = Vec::new();
    for day in START..=TODAY {
        let (_, _, d) = civil(day);
        if (day - START) % 14 == 3 {
            ck.push(txn(next_id(), day, "Acme Corp Payroll", "Salary", 285_000));
        }
        if d == 1 {
            let t = txn(
                next_id(),
                day,
                "Parkside Apartments",
                "Housing:Rent",
                -195_000,
            );
            is_check.push(t.id);
            ck.push(t);
        }
        if d == 12 {
            let c = -rng.around(11_000);
            ck.push(txn(
                next_id(),
                day,
                "City Power & Light",
                "Utilities:Electric",
                c,
            ));
        }
        if d == 18 {
            ck.push(txn(next_id(), day, "Comcast", "Utilities:Internet", -8_999));
        }
        if d == 22 {
            let c = -rng.around(4_500);
            ck.push(txn(next_id(), day, "City Water Dept", "Utilities:Water", c));
        }
        if (day - START) % 7 == 5 {
            ck.push(txn(next_id(), day, "ATM Withdrawal", "Cash", -6_000));
        }
        if rng.below(7) < 3 {
            let c = -rng.around(8_500);
            ck.push(txn(next_id(), day, "Safeway", "Groceries", c));
        }
        if rng.below(7) == 0 {
            let c = -rng.around(4_800);
            ck.push(txn(next_id(), day, "Chevron", "Auto:Fuel", c));
        }
        if rng.below(7) < 2 {
            let p = *rng.pick(DINING);
            let c = -rng.around(4_000);
            ck.push(txn(next_id(), day, p, "Dining", c));
        }
        if rng.below(10) < 4 {
            let (p, cat, typical) = *rng.pick(MISC);
            let c = -rng.around(typical);
            ck.push(txn(next_id(), day, p, cat, c));
        }
        if rng.below(30) == 0 && !(CK_LAST..=CK_STMT).contains(&day) {
            let (p, cat, typical) = *rng.pick(CHECKS);
            let t = txn(next_id(), day, p, cat, -rng.around(typical));
            is_check.push(t.id);
            ck.push(t);
        }
    }
    for (day, p) in payments {
        ck.push(txn(next_id(), day, "Visa Payment", "[Visa]", -p));
    }
    // The scenario's own rows: an old check never cashed, the transposed check, a later
    // debit already marked Cleared, and next month's rent written early.
    let outstanding = next_id();
    ck.push(txn(
        outstanding,
        20_693,
        "Lincoln Elementary PTA",
        "Education",
        -4_500,
    ));
    is_check.push(outstanding);
    let dental = next_id();
    ck.push(txn(
        dental,
        20_720,
        "Dr. Patel Dental",
        "Medical:Dental",
        -14_500,
    ));
    is_check.push(dental);
    let lawn = next_id();
    ck.push(txn(lawn, 20_736, "Green Lawn Care", "Home:Yard", -7_500));
    is_check.push(lawn);
    let coffee = next_id();
    ck.push(txn(coffee, 20_743, "Blue Bottle Coffee", "Dining", -675));
    let rent = next_id();
    ck.push(txn(
        rent,
        20_758,
        "Parkside Apartments",
        "Housing:Rent",
        -195_000,
    ));
    is_check.push(rent);

    ck.sort_by_key(|t| (t.day, t.id));
    let mut num = 1001;
    for t in &mut ck {
        if is_check.contains(&t.id) {
            t.num = Some(num);
            num += 1;
        }
    }
    let mut dental_num = 0;
    for t in &mut ck {
        if t.day <= CK_LAST && t.id != outstanding {
            t.clr = Clr::Reconciled;
        } else if t.day <= CK_STMT && t.id != outstanding {
            // The bank hasn't seen checks written from 10/8, or anything from 10/13.
            let late = (t.num.is_some() && t.day >= 20_734) || t.day >= 20_739;
            if !late {
                t.on_paper = Some(if t.id == dental { -15_400 } else { t.cents });
            }
        }
        if t.id == coffee {
            t.clr = Clr::Cleared;
        }
        if t.id == dental {
            dental_num = t.num.unwrap_or_default();
        }
    }
    let fee = -1_200;
    let ck_paper = ck
        .iter()
        .filter(|t| t.clr == Clr::Reconciled)
        .map(|t| t.cents)
        .sum::<i64>()
        + ck.iter().filter_map(|t| t.on_paper).sum::<i64>()
        + fee;
    let checking = Account {
        name: "Checking",
        card: false,
        txns: ck,
        stmt: None,
        paper: (CK_STMT, ck_paper),
        paper_note: format!(
            "It lists a 12.00 monthly service fee never entered in Balanced, and check {dental_num} to Dr. Patel Dental at 154.00 (entered as 145.00). Checks written from 10/8 and anything from 10/13 aren't on it."
        ),
    };

    visa.sort_by_key(|t| (t.day, t.id));
    for t in &mut visa {
        if t.day <= VS_LAST {
            t.clr = Clr::Reconciled;
        } else if t.day <= VS_STMT - 2 {
            t.on_paper = Some(t.cents);
            // Started before: the first half is already ticked.
            if t.day <= 20_716 {
                t.clr = Clr::Cleared;
            }
        }
    }
    let vs_paper = -(visa
        .iter()
        .filter(|t| t.clr == Clr::Reconciled)
        .map(|t| t.cents)
        .sum::<i64>()
        + visa.iter().filter_map(|t| t.on_paper).sum::<i64>());
    let visa = Account {
        name: "Visa",
        card: true,
        txns: visa,
        stmt: Some((VS_STMT, vs_paper)),
        paper: (VS_STMT, vs_paper),
        paper_note: "Matches Balanced once every charge and payment it lists is ticked. Charges from 10/4 and 10/5 post on the next statement.".to_owned(),
    };
    vec![checking, visa]
}

// ---------------------------------------------------------------------------------------------
// Variants and toggles

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
            .find(|v| v.key().eq_ignore_ascii_case(key.trim()))
    }

    fn label(self) -> &'static str {
        match self {
            Self::A => "Heavy Dialog, two lists",
            Self::B => "Register mode",
            Self::C => "Docked side panel",
        }
    }

    fn step(self, delta: isize) -> Self {
        let i = Self::ALL.iter().position(|v| *v == self).unwrap() as isize;
        Self::ALL[(i + delta).rem_euclid(3) as usize]
    }
}

const LATER: usize = 0;
const A_LAYOUT: usize = 1;
const B_ROWS: usize = 2;
const FIGS: usize = 3;
const TICK_BY: usize = 4;
const A_EDIT: usize = 5;

/// (key, label, applies to, options).
const TOGGLES: &[(&str, &str, &str, &[&str])] = &[
    (
        "later",
        "Later Uncleared transactions",
        "all",
        &["Hidden", "Shown"],
    ),
    ("lists", "A's two lists", "a", &["Side by side", "Stacked"]),
    (
        "rows",
        "B's Register rows",
        "b",
        &["Only the reconcile set", "All, others dimmed"],
    ),
    ("figures", "B and C's figures", "b,c", &["Above", "Below"]),
    (
        "tick",
        "A and C: ticking",
        "a,c",
        &["Click anywhere on the row", "Click the tick box only"],
    ),
    (
        "edit",
        "A's Edit… and New…",
        "a",
        &[
            "Open on top of the Dialog",
            "Finish later, go to the Register",
        ],
    ),
];

const TOGGLE_KEYS: [(Key, &str); 6] = [
    (Key::Num1, "1"),
    (Key::Num2, "2"),
    (Key::Num3, "3"),
    (Key::Num4, "4"),
    (Key::Num5, "5"),
    (Key::Num6, "6"),
];

const ADJUST_CATEGORIES: &[&str] = &[
    "Bank Fees",
    "Interest Income",
    "Miscellaneous",
    "Uncategorized",
];

/// A light Dialog on top of everything.
enum Sub {
    Start {
        acct: usize,
        date: String,
        ending: String,
        err: Option<String>,
        /// Opened from "Edit statement…" during a reconcile, rather than to begin one.
        editing: bool,
        fresh: bool,
    },
    Adjust {
        cat: Option<usize>,
    },
    Edit {
        id: u32,
        amount: String,
        err: Option<String>,
        fresh: bool,
    },
    New {
        date: String,
        payee: String,
        category: String,
        payment: String,
        deposit: String,
        err: Option<String>,
        fresh: bool,
    },
}

impl Sub {
    fn name(&self) -> &'static str {
        match self {
            Sub::Start { .. } => "start",
            Sub::Adjust { .. } => "adjust",
            Sub::Edit { .. } => "edit",
            Sub::New { .. } => "new",
        }
    }
}

enum Act {
    Tick(u32, &'static str),
    Select(u32, &'static str),
    OpenStart(&'static str),
    Finish(&'static str),
    Later(&'static str),
    Edit(&'static str),
    New(&'static str),
    Account(usize, &'static str),
    MarkAll(bool),
}

const RED: Color32 = Color32::from_rgb(0xc0, 0x2b, 0x2b);
const GREEN: Color32 = Color32::from_rgb(0x2e, 0x7d, 0x32);
const STATUS_H: f32 = 26.0;

struct Proto {
    h: Harness,
    variant: Variant,
    tog: [usize; 6],
    theme: Theme,
    dock_open: bool,
    maximize_sent: bool,
    status: String,
    accts: Vec<Account>,
    cur: usize,
    /// The account being reconciled.
    recon: Option<usize>,
    sel: Option<u32>,
    sub: Option<Sub>,
    acts: Vec<Act>,
    reg_scroll: Option<usize>,
    /// Lists drawn last frame, in order, for the arrow keys.
    lists: Vec<Vec<u32>>,
    reg_rows: Vec<u32>,
    closing: bool,
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
        let accts = generate();
        let n = accts[0].txns.len();
        Self {
            h,
            variant,
            tog: [0; 6],
            theme: Theme::new(ThemeSettings::default()),
            dock_open: true,
            maximize_sent: false,
            status: "Ready".to_owned(),
            accts,
            cur: 0,
            recon: None,
            sel: None,
            sub: None,
            acts: Vec::new(),
            reg_scroll: Some(n),
            lists: Vec::new(),
            reg_rows: Vec::new(),
            closing: false,
        }
    }

    fn event(&mut self, kind: &str, fields: Value) {
        self.h.event(kind, fields);
    }

    fn opt(&self, t: usize) -> usize {
        self.tog[t]
    }

    fn later(&self) -> bool {
        self.opt(LATER) == 1
    }

    fn acct(&self) -> &Account {
        &self.accts[self.cur]
    }

    /// Whether the open account is being reconciled.
    fn reconciling(&self) -> bool {
        self.recon == Some(self.cur)
    }

    fn describe(&self, a: usize, id: u32) -> Value {
        let acct = &self.accts[a];
        acct.idx(id).map_or(json!({ "id": id }), |i| {
            let t = &acct.txns[i];
            json!({ "id": id, "account": acct.name, "date": fmt_day(t.day), "num": t.num, "payee": t.payee, "amount": fmt_cents(t.cents) })
        })
    }

    fn figures(&self) -> Value {
        let a = self.acct();
        json!({
            "account": a.name,
            "cleared": fmt_cents(a.shown(a.cleared())),
            "difference": a.difference().map(fmt_cents),
        })
    }

    // -----------------------------------------------------------------------------------------
    // Toggles, variants, dock

    fn set_variant(&mut self, to: Variant, via: &str) {
        if to == self.variant {
            return;
        }
        self.event(
            "variant",
            json!({ "from": self.variant.key(), "to": to.key(), "via": via, "reconciling": self.recon.map(|a| self.accts[a].name) }),
        );
        self.variant = to;
        self.scroll_to_set();
        self.h.screenshot(format!("variant-{}", to.key()));
    }

    fn set_toggle(&mut self, key: &str, value: Option<&str>, via: &str) {
        let index = key
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1))
            .filter(|i| *i < TOGGLES.len())
            .or_else(|| TOGGLES.iter().position(|t| t.0 == key));
        let Some(i) = index else {
            self.event("control_unknown", json!({ "toggle": key }));
            return;
        };
        let n = TOGGLES[i].3.len();
        let new = match value {
            None => (self.tog[i] + 1) % n,
            Some("on") => 1.min(n - 1),
            Some("off") => 0,
            Some(v) => v
                .parse::<usize>()
                .map_or(self.tog[i], |v| v.saturating_sub(1).min(n - 1)),
        };
        if new == self.tog[i] && via == "control" {
            return;
        }
        self.tog[i] = new;
        self.event(
            "toggle",
            json!({ "key": TOGGLES[i].0, "option": TOGGLES[i].3[new], "index": new + 1, "via": via, "applies_to": TOGGLES[i].2 }),
        );
        if i == B_ROWS || i == LATER {
            self.scroll_to_set();
        }
        self.h
            .screenshot(format!("toggle-{}-{}", TOGGLES[i].0, new + 1));
    }

    fn set_dock(&mut self, open: bool, via: &str) {
        self.dock_open = open;
        self.event("dock", json!({ "open": open, "via": via }));
    }

    /// Scrolls the Register to the first row of the reconcile set, or to the end.
    fn scroll_to_set(&mut self) {
        let a = self.acct();
        let later = self.later();
        let first = a.txns.iter().position(|t| a.in_set(t, later));
        self.reg_scroll = Some(if self.reconciling() && self.variant == Variant::B {
            0
        } else {
            first.unwrap_or(a.txns.len())
        });
        if self.reconciling() && self.variant == Variant::B && self.opt(B_ROWS) == 1 {
            self.reg_scroll = first;
        }
    }

    // -----------------------------------------------------------------------------------------
    // Reconcile actions

    fn open_start(&mut self, source: &str, editing: bool) {
        let a = self.acct();
        let (date, ending) = match a.stmt {
            Some((d, e)) => (fmt_day(d), fmt_cents(e)),
            None => (fmt_day(TODAY), String::new()),
        };
        self.event(
            "command",
            json!({ "would_emit": "OpenDialog(ReconcileStart)", "source": source, "account": a.name, "refilled": a.stmt.is_some(), "editing": editing }),
        );
        self.sub = Some(Sub::Start {
            acct: self.cur,
            date,
            ending,
            err: None,
            editing,
            fresh: true,
        });
    }

    fn tick(&mut self, id: u32, via: &str) {
        let a = self.cur;
        let Some(i) = self.accts[a].idx(id) else {
            return;
        };
        let t = &mut self.accts[a].txns[i];
        let to = match t.clr {
            Clr::Uncleared => Clr::Cleared,
            Clr::Cleared => Clr::Uncleared,
            Clr::Reconciled => {
                self.event(
                    "tick_refused",
                    json!({ "why": "Reconciled; the real Register asks before un-reconciling", "via": via }),
                );
                return;
            }
        };
        t.clr = to;
        let mut f = self.describe(a, id);
        f["would_emit"] = json!(format!("EditTransaction {{ ClearedStatus: {to:?} }}"));
        f["via"] = json!(via);
        f["figures"] = self.figures();
        self.event("command", f);
    }

    fn mark_all(&mut self, on: bool) {
        let a = self.cur;
        let later = self.later();
        let acct = &self.accts[a];
        let ids: Vec<u32> = acct
            .txns
            .iter()
            .filter(|t| {
                acct.in_set(t, later) && t.clr == if on { Clr::Uncleared } else { Clr::Cleared }
            })
            .map(|t| t.id)
            .collect();
        for t in &mut self.accts[a].txns {
            if ids.contains(&t.id) {
                t.clr = if on { Clr::Cleared } else { Clr::Uncleared };
            }
        }
        self.event(
            "command",
            json!({ "would_emit": format!("Compound[EditTransaction {{ ClearedStatus: {} }} x{}]", if on { "Cleared" } else { "Uncleared" }, ids.len()), "via": if on { "Mark all" } else { "Unmark all" }, "figures": self.figures() }),
        );
    }

    fn begin(&mut self, acct: usize, day: i64, ending: i64, editing: bool) {
        self.accts[acct].stmt = Some((day, ending));
        self.event(
            "command",
            json!({ "would_emit": format!("SetStatement {{ account: {}, date: {}, ending: {} }}", self.accts[acct].name, fmt_day(day), fmt_cents(ending)), "editing": editing }),
        );
        if !editing {
            self.recon = Some(acct);
            self.sel = None;
            self.event(
                "reconcile_open",
                json!({ "variant": self.variant.key(), "account": self.accts[acct].name, "figures": self.figures() }),
            );
            self.status = format!(
                "Reconciling {} to the statement of {}.",
                self.accts[acct].name,
                fmt_day(day)
            );
            self.scroll_to_set();
            self.h
                .screenshot(format!("reconcile-{}", self.variant.key()));
        }
    }

    fn finish_later(&mut self, via: &str) {
        let Some(a) = self.recon.take() else {
            return;
        };
        self.event(
            "finish_later",
            json!({ "would_emit": "none: the ticks are already saved and the statement stays on the account", "via": via, "account": self.accts[a].name, "difference": self.accts[a].difference().map(fmt_cents) }),
        );
        self.status = format!(
            "Reconcile of {} saved for later. The ticks stay as Cleared.",
            self.accts[a].name
        );
        self.scroll_to_set();
    }

    fn finish(&mut self, adjust: Option<usize>, via: &str) {
        let Some(a) = self.recon else {
            return;
        };
        let diff = self.accts[a].difference().unwrap_or(0);
        if diff != 0 && adjust.is_none() {
            self.event(
                "finish_difference",
                json!({ "difference": fmt_cents(diff), "via": via }),
            );
            self.sub = Some(Sub::Adjust { cat: None });
            return;
        }
        let acct = &mut self.accts[a];
        let (day, _) = acct.stmt.unwrap();
        let mut parts = Vec::new();
        if let Some(c) = adjust {
            let cents = acct.shown(diff);
            let id = acct.txns.iter().map(|t| t.id).max().unwrap_or(0) + 1;
            acct.txns.push(Txn {
                id,
                day,
                num: None,
                payee: "Balance Adjustment".to_owned(),
                cat: ADJUST_CATEGORIES[c].to_owned(),
                cents,
                clr: Clr::Cleared,
                on_paper: None,
            });
            acct.sort();
            parts.push(format!(
                "AddTransaction(Balance Adjustment, {}, {}, {})",
                fmt_day(day),
                ADJUST_CATEGORIES[c],
                fmt_cents(cents)
            ));
        }
        let n = acct.txns.iter().filter(|t| t.clr == Clr::Cleared).count();
        for t in &mut acct.txns {
            if t.clr == Clr::Cleared {
                t.clr = Clr::Reconciled;
            }
        }
        acct.stmt = None;
        parts.push(format!("EditTransaction(ClearedStatus: Reconciled) x{n}"));
        parts.push("ClearStatement".to_owned());
        let name = acct.name;
        self.recon = None;
        self.event(
            "command",
            json!({ "would_emit": format!("Compound[{}]", parts.join(", ")), "via": via, "account": name }),
        );
        self.status = format!("Reconciled {name} to the statement of {}.", fmt_day(day));
        self.scroll_to_set();
        self.h.screenshot("finished");
    }

    fn switch_account(&mut self, i: usize, via: &str) {
        if i == self.cur {
            return;
        }
        // Leaving a Register in reconcile mode, or with its panel open, is finishing later.
        if self.reconciling() && self.variant != Variant::A {
            self.finish_later(&format!("switched account ({via})"));
        }
        self.cur = i;
        self.sel = None;
        self.event("account", json!({ "to": self.accts[i].name, "via": via }));
        self.scroll_to_set();
    }

    fn edit_selected(&mut self, via: &str) {
        let Some(id) = self.sel else {
            self.event(
                "edit_refused",
                json!({ "why": "nothing selected", "via": via }),
            );
            return;
        };
        if self.variant == Variant::A && self.reconciling() && self.opt(A_EDIT) == 1 {
            self.finish_later("Edit… (goes to the Register)");
            let at = self.acct().idx(id).unwrap_or(0);
            self.reg_scroll = Some(at);
            self.event(
                "command",
                json!({ "would_emit": "Navigate(Register row)", "via": via, "txn": self.describe(self.cur, id) }),
            );
            return;
        }
        let amount = self
            .acct()
            .idx(id)
            .map(|i| fmt_cents(self.acct().txns[i].cents))
            .unwrap_or_default();
        self.event(
            "edit_open",
            json!({ "via": via, "txn": self.describe(self.cur, id) }),
        );
        self.sub = Some(Sub::Edit {
            id,
            amount,
            err: None,
            fresh: true,
        });
    }

    fn new_txn(&mut self, via: &str) {
        if self.variant == Variant::A && self.reconciling() && self.opt(A_EDIT) == 1 {
            self.finish_later("New… (goes to the Register)");
            self.reg_scroll = Some(self.acct().txns.len());
            self.event(
                "command",
                json!({ "would_emit": "GoToDraftRow", "via": via }),
            );
            return;
        }
        let day = self.acct().stmt.map_or(TODAY, |s| s.0);
        self.event("new_open", json!({ "via": via }));
        self.sub = Some(Sub::New {
            date: fmt_day(day),
            payee: String::new(),
            category: String::new(),
            payment: String::new(),
            deposit: String::new(),
            err: None,
            fresh: true,
        });
    }

    fn find(&self, what: &str) -> Option<u32> {
        let a = self.acct();
        if let Ok(id) = what.parse::<u32>() {
            if a.idx(id).is_some() {
                return Some(id);
            }
            if let Some(t) = a.txns.iter().find(|t| t.num == Some(id)) {
                return Some(t.id);
            }
        }
        let later = self.later();
        let q = what.to_lowercase();
        // Prefer the reconcile set, then the newest match.
        a.txns
            .iter()
            .filter(|t| t.payee.to_lowercase().contains(&q))
            .max_by_key(|t| (a.in_set(t, later), t.day))
            .map(|t| t.id)
    }

    fn apply_acts(&mut self) {
        for act in std::mem::take(&mut self.acts) {
            match act {
                Act::Tick(id, via) => {
                    self.sel = Some(id);
                    self.tick(id, via);
                }
                Act::Select(id, via) => {
                    self.sel = Some(id);
                    if via == "panel row" {
                        self.reg_scroll = self.reg_rows.iter().position(|r| *r == id);
                    }
                }
                Act::OpenStart(source) => {
                    if self.reconciling() {
                        // Already open: bring it forward.
                        self.event("reconcile_already_open", json!({ "source": source }));
                    } else {
                        self.open_start(source, false);
                    }
                }
                Act::Finish(via) => self.finish(None, via),
                Act::Later(via) => self.finish_later(via),
                Act::Edit(via) => self.edit_selected(via),
                Act::New(via) => self.new_txn(via),
                Act::Account(i, via) => self.switch_account(i, via),
                Act::MarkAll(on) => self.mark_all(on),
            }
        }
    }

    fn apply_control(&mut self, ctx: &Context, cmd: &str) {
        let (verb, rest) = cmd
            .split_once(' ')
            .map_or((cmd, ""), |(v, r)| (v, r.trim()));
        let bad = |s: &mut Self| s.event("control_unknown", json!({ "command": cmd }));
        match verb {
            "variant" => match Variant::from_key(rest) {
                Some(v) => self.set_variant(v, "control"),
                None => bad(self),
            },
            "toggle" => {
                let (key, value) = rest
                    .split_once(' ')
                    .map_or((rest, None), |(k, v)| (k, Some(v.trim())));
                self.set_toggle(key, value, "control");
            }
            "account" => match self
                .accts
                .iter()
                .position(|a| a.name.eq_ignore_ascii_case(rest))
            {
                Some(i) => self.switch_account(i, "control"),
                None => bad(self),
            },
            "reconcile" => self.acts.push(Act::OpenStart("control")),
            "date" | "ending" => match &mut self.sub {
                Some(Sub::Start { date, ending, .. }) => {
                    *(if verb == "date" { date } else { ending }) = rest.to_owned();
                }
                _ => bad(self),
            },
            "amount" => match &mut self.sub {
                Some(Sub::Edit { amount, .. }) => *amount = rest.to_owned(),
                _ => bad(self),
            },
            "category" => match &mut self.sub {
                Some(Sub::Adjust { cat }) => {
                    *cat = ADJUST_CATEGORIES
                        .iter()
                        .position(|c| c.eq_ignore_ascii_case(rest));
                }
                _ => bad(self),
            },
            "ok" => self.h.event("control_ok", json!({})),
            "cancel" => {
                if let Some(sub) = self.sub.take() {
                    self.event(
                        "sub_closed",
                        json!({ "dialog": sub.name(), "via": "control" }),
                    );
                } else {
                    self.finish_later("control cancel");
                }
            }
            "tick" | "untick" => {
                let on = verb == "tick";
                if !self.reconciling() {
                    bad(self);
                } else if rest == "all" {
                    self.mark_all(on);
                } else if rest == "stmt" {
                    let a = self.acct();
                    let ids: Vec<u32> = a
                        .txns
                        .iter()
                        .filter(|t| t.on_paper.is_some() && t.clr == Clr::Uncleared)
                        .map(|t| t.id)
                        .collect();
                    for id in ids {
                        self.tick(id, "control");
                    }
                } else if let Some(id) = self.find(rest) {
                    let clr = self.acct().idx(id).map(|i| self.acct().txns[i].clr);
                    if (clr == Some(Clr::Uncleared)) == on {
                        self.tick(id, "control");
                    }
                    self.sel = Some(id);
                } else {
                    bad(self);
                }
            }
            "select" => match self.find(rest) {
                Some(id) => {
                    self.sel = Some(id);
                    self.reg_scroll = self.reg_rows.iter().position(|r| *r == id);
                }
                None => bad(self),
            },
            "edit" => self.edit_selected("control"),
            "new" => self.new_txn("control"),
            "finish" => self.finish(None, "control"),
            "later" => self.finish_later("control"),
            "reload" => {
                self.accts = generate();
                self.recon = None;
                self.sub = None;
                self.sel = None;
                self.status = "Ready".to_owned();
                self.scroll_to_set();
                self.event("reload", json!({ "seed": SEED }));
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

// ---------------------------------------------------------------------------------------------
// Frame

impl eframe::App for Proto {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.h.event("session_end", json!({}));
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.maximize_sent {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(true));
            self.maximize_sent = true;
        }
        let mut control_ok = false;
        if let Some(cmd) = self.h.begin_frame(&ctx) {
            control_ok = cmd.trim() == "ok";
            self.apply_control(&ctx, &cmd);
        }
        let typing = ctx.egui_wants_keyboard_input();
        let free = !typing && self.sub.is_none();
        if free {
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
            let down = ctx.input(|i| i.key_pressed(Key::ArrowDown));
            let up = ctx.input(|i| i.key_pressed(Key::ArrowUp));
            if down || up {
                let list = self
                    .sel
                    .and_then(|s| self.lists.iter().find(|l| l.contains(&s)))
                    .or_else(|| self.lists.first())
                    .cloned()
                    .unwrap_or_default();
                if !list.is_empty() {
                    let at = self.sel.and_then(|s| list.iter().position(|v| *v == s));
                    let next = match (at, down) {
                        (Some(i), true) => (i + 1).min(list.len() - 1),
                        (Some(i), false) => i.saturating_sub(1),
                        (None, _) => 0,
                    };
                    self.sel = Some(list[next]);
                    self.event(
                        "key",
                        json!({ "key": if down { "Down" } else { "Up" }, "selected": self.describe(self.cur, list[next]) }),
                    );
                }
            }
            if self.reconciling() && ctx.input(|i| i.key_pressed(Key::Space)) {
                if let Some(id) = self.sel {
                    let later = self.later();
                    let a = self.acct();
                    if a.idx(id).is_some_and(|i| a.in_set(&a.txns[i], later)) {
                        self.acts.push(Act::Tick(id, "Space"));
                    }
                }
            }
            if self.reconciling() && ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.acts.push(Act::Later("Esc"));
            }
            if self.reconciling()
                && self.variant == Variant::A
                && ctx.input(|i| i.key_pressed(Key::Enter))
            {
                self.acts.push(Act::Finish("Enter"));
            }
        }
        if ctx.input(|i| i.key_pressed(Key::F4)) {
            self.set_dock(!self.dock_open, "key");
        }

        self.dock(ui);
        self.h.prompt_panel(ui);
        self.menu_bar(ui);
        Panel::bottom("status")
            .exact_size(STATUS_H)
            .frame(
                self.theme
                    .chrome_frame()
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(ui, |ui| {
                ui.label(RichText::new(&self.status).weak());
            });
        self.account_bar(ui);
        self.lists.clear();
        if self.reconciling() && self.variant == Variant::C {
            self.side_panel(ui);
        }
        CentralPanel::default().show(ui, |ui| {
            self.context_header(ui);
            ui.add_space(4.0);
            let b = self.reconciling() && self.variant == Variant::B;
            if b && self.opt(FIGS) == 0 {
                self.reconcile_bar(ui);
            }
            if b && self.opt(FIGS) == 1 {
                let h = ui.available_height() - 92.0;
                ui.allocate_ui(vec2(ui.available_width(), h), |ui| self.register(ui));
                self.reconcile_bar(ui);
            } else {
                self.register(ui);
            }
        });
        if self.reconciling() && self.variant == Variant::A {
            self.heavy_window(&ctx);
        }
        self.sub_modal(&ctx, control_ok);
        self.apply_acts();
        self.h.end_frame(&ctx);
    }
}

// ---------------------------------------------------------------------------------------------
// Chrome: dock, menu, Account bar, context header, Register

impl Proto {
    /// A button that logs its click and gives up focus, so Space ticks a row afterwards
    /// rather than pressing the button again.
    fn btn(&mut self, ui: &mut Ui, label: &str, name: &str, enabled: bool) -> bool {
        let r = ui.add_enabled(enabled, egui::Button::new(label));
        self.h.track(&r, name);
        if r.clicked() {
            r.surrender_focus();
            return true;
        }
        false
    }

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
            .exact_size(340.0)
            .resizable(false)
            .frame(frame)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    Sides::new().show(
                        ui,
                        |ui| ui.label(RichText::new("PROTOTYPE · Reconcile").strong()),
                        |ui| {
                            let r = ui.small_button("▶").on_hover_text("Collapse (F4)");
                            self.h.track(&r, "dock: collapse");
                            if r.clicked() {
                                self.set_dock(false, "click");
                            }
                        },
                    );
                    ui.separator();
                    ui.label(RichText::new("Variant (where Reconcile lives)").strong());
                    for (v, (_, hint)) in Variant::ALL.into_iter().zip(VARIANT_KEYS) {
                        let r = ui
                            .selectable_label(self.variant == v, format!("{hint}   {}", v.label()));
                        self.h.track(&r, format!("dock: variant {}", v.key()));
                        if r.clicked() {
                            self.set_variant(v, "click");
                        }
                    }
                    ui.separator();
                    ui.label(RichText::new("Toggles (the key cycles)").strong());
                    for (i, ((key, label, applies, options), (_, hint))) in
                        TOGGLES.iter().zip(TOGGLE_KEYS).enumerate()
                    {
                        let applies = if *applies == "all" {
                            "all".to_owned()
                        } else {
                            applies.to_uppercase().replace(',', ", ")
                        };
                        ui.label(format!("{hint}   {label}  ({applies})"));
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(14.0);
                            for (n, option) in options.iter().enumerate() {
                                let r = ui.selectable_label(self.tog[i] == n, *option);
                                self.h.track(&r, format!("dock: toggle {key} {option}"));
                                if r.clicked() && self.tog[i] != n {
                                    self.set_toggle(key, Some(&(n + 1).to_string()), "click");
                                }
                            }
                        });
                    }
                    ui.separator();
                    ui.label(RichText::new("The paper statement in your hand").strong());
                    let a = self.acct();
                    let (day, end) = a.paper;
                    ui.label(format!(
                        "{}: {} {}, {} {}.",
                        a.name,
                        if a.card { "statement closing" } else { "statement of" },
                        fmt_day(day),
                        if a.card { "balance owed" } else { "ending balance" },
                        fmt_cents(end)
                    ));
                    ui.label(RichText::new(a.paper_note.clone()).small());
                    ui.separator();
                    self.h.dock_spotlight(ui);
                    self.h.dock_prompt(ui);
                    ui.separator();
                    let r = ui.button("Reset the data");
                    self.h.track(&r, "dock: reset");
                    if r.clicked() {
                        self.apply_control(&ui.ctx().clone(), "reload");
                    }
                    ui.separator();
                    ui.label(
                        RichText::new(
                            "A / B / C or left / right: variant · 1–6 toggles · up / down select · Space ticks · Esc finishes later · F2 note · F3 marker · F4 hide this dock · F5 spotlight · F6 prompt",
                        )
                        .small()
                        .weak(),
                    );
                });
            });
    }

    fn menu_bar(&mut self, ui: &mut Ui) {
        Panel::top("menu_bar")
            .frame(
                self.theme
                    .chrome_frame()
                    .inner_margin(egui::Margin::symmetric(6, 2)),
            )
            .show(ui, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        ui.add_enabled(false, egui::Button::new("New data file…"));
                        ui.add_enabled(false, egui::Button::new("Open data file…"));
                    });
                    ui.menu_button("Edit", |ui| {
                        ui.add_enabled(false, egui::Button::new("Undo"));
                    });
                    ui.menu_button("View", |ui| {
                        ui.add_enabled(false, egui::Button::new("Switch pane"));
                    });
                    ui.menu_button("Tools", |ui| {
                        ui.add_enabled(false, egui::Button::new("Manage Accounts…"));
                        ui.add_enabled(false, egui::Button::new("Category List…"));
                    });
                });
            });
    }

    fn account_bar(&mut self, ui: &mut Ui) {
        Panel::left("account_bar")
            .exact_size(230.0)
            .resizable(false)
            .frame(
                self.theme
                    .chrome_frame()
                    .inner_margin(egui::Margin::symmetric(8, 6)),
            )
            .show(ui, |ui| {
                let rect = ui.max_rect();
                self.h.region(rect, "account bar");
                ui.label(RichText::new("Accounts").strong());
                ui.add_space(4.0);
                ui.label(RichText::new("Banking").weak());
                for i in 0..self.accts.len() {
                    let a = &self.accts[i];
                    let name = a.name;
                    let bal = a.current();
                    let selected = i == self.cur;
                    let r = ui
                        .horizontal(|ui| {
                            let r = ui.selectable_label(selected, name);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(self.amount_text(bal));
                            });
                            r
                        })
                        .inner;
                    self.h.track(&r, format!("account bar: {name}"));
                    if r.clicked() {
                        self.acts.push(Act::Account(i, "account bar"));
                    }
                    r.context_menu(|ui| {
                        if ui.button("Open Register").clicked() {
                            self.acts.push(Act::Account(i, "account bar menu"));
                            ui.close();
                        }
                        let rr = ui.button("Reconcile");
                        self.h
                            .track(&rr, format!("account bar menu: Reconcile {name}"));
                        if rr.clicked() {
                            self.acts.push(Act::Account(i, "account bar menu"));
                            self.acts.push(Act::OpenStart("account bar menu"));
                            ui.close();
                        }
                        ui.separator();
                        ui.add_enabled(false, egui::Button::new("Manage Accounts…"));
                    });
                }
                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    ui.add_enabled(false, egui::Button::new("+ Add Account"));
                });
            });
    }

    fn amount_text(&self, cents: i64) -> RichText {
        let text = RichText::new(fmt_cents(cents));
        if cents < 0 { text.color(RED) } else { text }
    }

    fn context_header(&mut self, ui: &mut Ui) {
        let a = self.acct();
        let (name, current, ending, cleared) = (a.name, a.current(), a.ending(), a.cleared());
        let r = Sides::new().shrink_left().truncate().show(
            ui,
            |ui| {
                ui.label(RichText::new(name).heading());
            },
            |ui| {
                let menu = ui.menu_button("Account Actions ⚙", |ui| {
                    let r = ui.button("Reconcile…");
                    self.h.track(&r, "Account Actions: Reconcile");
                    if r.clicked() {
                        self.acts.push(Act::OpenStart("Account Actions"));
                        ui.close();
                    }
                    for item in [
                        "Edit account details…",
                        "Register preferences…",
                        "Print Register…",
                        "Export to CSV…",
                    ] {
                        ui.add_enabled(false, egui::Button::new(item));
                    }
                });
                self.h.track(&menu.response, "Account Actions");
                if self.btn(ui, "+ Transaction", "+ Transaction", true) {
                    self.acts.push(Act::New("+ Transaction"));
                }
                for (label, v) in [
                    ("Cleared", cleared),
                    ("Ending", ending),
                    ("Current", current),
                ] {
                    ui.label(
                        RichText::new(format!("{label} {}", fmt_cents(v)))
                            .small()
                            .color(if v < 0 {
                                RED
                            } else {
                                ui.visuals().weak_text_color()
                            }),
                    );
                }
            },
        );
        let _ = r;
        let rect = ui.min_rect();
        self.h.region(rect, "context header");
    }

    fn register(&mut self, ui: &mut Ui) {
        let row_h = self.theme.row_height();
        let b_mode = self.reconciling() && self.variant == Variant::B;
        let only_set = b_mode && self.opt(B_ROWS) == 0;
        let dim_rest = b_mode && self.opt(B_ROWS) == 1;
        let later = self.later();
        let a = &self.accts[self.cur];
        let card = a.card;
        let rows: Vec<usize> = (0..a.txns.len())
            .filter(|i| !only_set || a.in_set(&a.txns[*i], later))
            .collect();
        // Running balance over every row, so a filtered view still shows the true balance.
        let mut bal = Vec::with_capacity(a.txns.len());
        let mut run = 0;
        for t in &a.txns {
            run += t.cents;
            bal.push(run);
        }
        let ids: Vec<u32> = rows.iter().map(|i| a.txns[*i].id).collect();
        let n = rows.len();
        let rect = ui.available_rect_before_wrap();
        self.h.region(rect, "register");
        ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
        let mut table = TableBuilder::new(ui)
            .id_salt(("register", self.cur, only_set))
            .striped(true)
            .resizable(false)
            .auto_shrink(false)
            .sense(Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(88.0))
            .column(Column::exact(54.0))
            .column(Column::remainder().at_least(140.0).clip(true))
            .column(Column::exact(170.0).clip(true))
            .column(Column::exact(2.0 * row_h))
            .column(Column::exact(100.0))
            .column(Column::exact(100.0))
            .column(Column::exact(110.0));
        if let Some(r) = self.reg_scroll.take() {
            table = table.scroll_to_row(r.min(n.saturating_sub(1)), Some(Align::Center));
        }
        let (out_h, in_h) = if card {
            ("Charge", "Payment")
        } else {
            ("Payment", "Deposit")
        };
        let mut acts = Vec::new();
        let mut tracked = Vec::new();
        let sel = self.sel;
        table
            .header(row_h + 2.0, |mut header| {
                for (label, right) in [
                    ("Date", false),
                    ("Num", false),
                    ("Payee", false),
                    ("Category", false),
                    ("Clr", false),
                    (out_h, true),
                    (in_h, true),
                    ("Balance", true),
                ] {
                    header.col(|ui| {
                        if right {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.strong(label)
                            });
                        } else if label == "Clr" {
                            ui.centered_and_justified(|ui| ui.strong(label));
                        } else {
                            ui.strong(label);
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(row_h, n, |mut row| {
                    let i = rows[row.index()];
                    let t = &a.txns[i];
                    let dim = dim_rest && !a.in_set(t, later);
                    let txt = |s: String| {
                        let r = RichText::new(s);
                        if dim { r.weak() } else { r }
                    };
                    row.set_selected(sel == Some(t.id));
                    row.col(|ui| {
                        ui.label(txt(fmt_day(t.day)));
                    });
                    row.col(|ui| {
                        ui.label(txt(t.num.map(|n| n.to_string()).unwrap_or_default()));
                    });
                    row.col(|ui| {
                        ui.label(txt(t.payee.clone()));
                    });
                    row.col(|ui| {
                        ui.label(txt(t.cat.clone()));
                    });
                    row.col(|ui| {
                        let (rect, r) = ui.allocate_exact_size(ui.available_size(), Sense::click());
                        let glyph = match t.clr {
                            Clr::Uncleared => "",
                            Clr::Cleared => "c",
                            Clr::Reconciled => "R",
                        };
                        let color = if dim {
                            ui.visuals().weak_text_color()
                        } else {
                            ui.visuals().text_color()
                        };
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            glyph,
                            egui::TextStyle::Body.resolve(ui.style()),
                            color,
                        );
                        if r.hovered() && t.clr != Clr::Reconciled {
                            ui.painter().rect_stroke(
                                rect.shrink(2.0),
                                2.0,
                                Stroke::new(1.0, ui.visuals().weak_text_color()),
                                egui::StrokeKind::Inside,
                            );
                        }
                        tracked.push((
                            r.clone(),
                            format!("Clr cell, {} {}", t.payee, fmt_day(t.day)),
                        ));
                        if r.clicked() {
                            acts.push(Act::Tick(t.id, "Clr cell"));
                        }
                    });
                    row.col(|ui| {
                        if t.cents < 0 {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(txt(fmt_cents(-t.cents)));
                            });
                        }
                    });
                    row.col(|ui| {
                        if t.cents >= 0 {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(txt(fmt_cents(t.cents)));
                            });
                        }
                    });
                    row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let b = bal[i];
                            let r = RichText::new(fmt_cents(b));
                            ui.label(if dim {
                                r.weak()
                            } else if b < 0 {
                                r.color(RED)
                            } else {
                                r
                            });
                        });
                    });
                    let r = row.response();
                    tracked.push((
                        r.clone(),
                        format!("register row, {} {}", t.payee, fmt_day(t.day)),
                    ));
                    if r.clicked() {
                        acts.push(Act::Select(t.id, "register row"));
                    }
                    if r.double_clicked() {
                        acts.push(Act::Edit("register row double-click"));
                    }
                });
            });
        for (r, name) in tracked {
            self.h.track(&r, name);
        }
        self.acts.extend(acts);
        self.reg_rows = ids.clone();
        // The arrow keys walk the Register unless A's or C's list is the reconcile surface.
        if !(self.reconciling() && self.variant != Variant::B) {
            self.lists.push(ids);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Reconcile surfaces

/// A tick box painted by hand: the default fonts lack a reliable check mark.
fn tick_box(ui: &mut Ui, ticked: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
    let v = ui.visuals();
    let stroke = if r.hovered() {
        Stroke::new(1.5, v.strong_text_color())
    } else {
        Stroke::new(1.0, v.text_color())
    };
    ui.painter()
        .rect_stroke(rect.shrink(1.0), 2.0, stroke, egui::StrokeKind::Inside);
    if ticked {
        let c = rect.center();
        ui.painter().line(
            vec![
                c + vec2(-4.5, 0.0),
                c + vec2(-1.2, 3.5),
                c + vec2(4.5, -4.0),
            ],
            Stroke::new(2.0, GREEN),
        );
    }
    r
}

impl Proto {
    /// The reconcile figures, as label and value rows.
    fn figure_rows(&self) -> Vec<(String, i64, bool)> {
        let a = self.acct();
        let later = self.later();
        let (on, ot) = a.ticked(later, true);
        let (inn, it) = a.ticked(later, false);
        let (_, end) = a.stmt.unwrap_or((TODAY, 0));
        let diff = a.difference().unwrap_or(0);
        vec![
            (
                if a.card {
                    "Opening balance owed".to_owned()
                } else {
                    "Opening balance".to_owned()
                },
                a.shown(a.opening()),
                false,
            ),
            (format!("{} ticked ({on})", a.out_label()), -ot, false),
            (format!("{} ticked ({inn})", a.in_label()), it, false),
            (
                if a.card {
                    "Cleared balance owed".to_owned()
                } else {
                    "Cleared balance".to_owned()
                },
                a.shown(a.cleared()),
                false,
            ),
            (a.ending_label().to_owned(), end, false),
            ("Difference".to_owned(), diff, true),
        ]
    }

    fn diff_text(&self, diff: i64) -> RichText {
        let t = RichText::new(fmt_cents(diff)).strong();
        if diff == 0 {
            t.color(GREEN)
        } else {
            t.color(RED)
        }
    }

    /// The statement line with Edit statement…, shared by every variant.
    fn statement_line(&mut self, ui: &mut Ui) {
        let a = self.acct();
        let (day, end) = a.stmt.unwrap_or((TODAY, 0));
        let label = a.ending_label();
        ui.label(format!(
            "Statement date {}  ·  {label} {}",
            fmt_day(day),
            fmt_cents(end)
        ));
        if self.btn(ui, "Edit statement…", "Edit statement", true) {
            self.open_start("Edit statement…", true);
        }
    }

    fn finish_buttons(&mut self, ui: &mut Ui) {
        let diff = self.acct().difference().unwrap_or(0);
        let finish = egui::Button::new(RichText::new("Finish").strong()).min_size(vec2(80.0, 0.0));
        let r = ui.add(finish);
        self.h.track(&r, "Finish");
        if r.clicked() {
            r.surrender_focus();
            self.acts.push(Act::Finish("Finish button"));
        }
        if self.btn(ui, "Finish later", "Finish later", true) {
            self.acts.push(Act::Later("Finish later button"));
        }
        let _ = diff;
    }

    /// A's heavy Dialog.
    fn heavy_window(&mut self, ctx: &Context) {
        let screen = ctx.content_rect();
        let backdrop = self.theme.backdrop();
        egui::Area::new(Id::new("rc_backdrop"))
            .order(Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, r) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
                ui.painter().rect_filled(rect, 0.0, backdrop);
                if r.clicked() {
                    self.h.event("backdrop_click", json!({}));
                }
            });
        let max = screen.size() - vec2(Theme::DIALOG_MARGIN * 2.0, Theme::DIALOG_MARGIN * 2.0);
        let size = vec2(1120.0_f32.min(max.x), 680.0_f32.min(max.y));
        let mut open = true;
        let name = self.acct().name;
        egui::Window::new(format!("Reconcile {name}"))
            .id(Id::new("rc_window"))
            .open(&mut open)
            .collapsible(false)
            // Fixed: the lists' tables don't shrink, so a resizable Window grew to the
            // whole screen. Movable still; its size isn't this prototype's question.
            .resizable(false)
            .fixed_size(size)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(screen.center())
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                self.h.region(rect, "reconcile dialog");
                self.dialog_body(ui);
            });
        ctx.move_to_top(egui::LayerId::new(Order::Middle, Id::new("rc_window")));
        if !open {
            self.acts.push(Act::Later("Dialog ×"));
        }
    }

    fn dialog_body(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            self.statement_line(ui);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.btn(ui, "Unmark all", "Unmark all", true) {
                    self.acts.push(Act::MarkAll(false));
                }
                if self.btn(ui, "Mark all", "Mark all", true) {
                    self.acts.push(Act::MarkAll(true));
                }
                ui.separator();
                let has_sel = self.sel.is_some();
                if self.btn(ui, "Edit…", "dialog Edit", has_sel) {
                    self.acts.push(Act::Edit("dialog Edit…"));
                }
                if self.btn(ui, "New…", "dialog New", true) {
                    self.acts.push(Act::New("dialog New…"));
                }
            });
        });
        ui.separator();
        let footer_h = 128.0;
        let lists_h = (ui.available_height() - footer_h).max(120.0);
        let w = ui.available_width();
        let a = self.acct();
        let later = self.later();
        let set: Vec<u32> = a
            .txns
            .iter()
            .filter(|t| a.in_set(t, later))
            .map(|t| t.id)
            .collect();
        let out: Vec<u32> = set
            .iter()
            .copied()
            .filter(|id| a.txns[a.idx(*id).unwrap()].cents < 0)
            .collect();
        let inn: Vec<u32> = set
            .iter()
            .copied()
            .filter(|id| a.txns[a.idx(*id).unwrap()].cents >= 0)
            .collect();
        let (out_l, in_l) = (a.out_label(), a.in_label());
        let stacked = self.opt(A_LAYOUT) == 1;
        ui.allocate_ui(vec2(w, lists_h), |ui| {
            if stacked {
                let h = (lists_h - 8.0) / 2.0;
                ui.allocate_ui(vec2(w, h), |ui| {
                    self.list(ui, "out", out_l, &out, h, false);
                });
                ui.allocate_ui(vec2(w, h), |ui| {
                    self.list(ui, "in", in_l, &inn, h, false);
                });
            } else {
                ui.columns(2, |cols| {
                    self.list(&mut cols[0], "out", out_l, &out, lists_h, false);
                    self.list(&mut cols[1], "in", in_l, &inn, lists_h, false);
                });
            }
        });
        ui.separator();
        let rows = self.figure_rows();
        let rect = ui.available_rect_before_wrap();
        self.h.region(rect, "reconcile figures");
        ui.horizontal(|ui| {
            // Painted at fixed places: right-to-left cells in a Grid stretched the Dialog.
            let row_h = self.theme.row_height();
            let (label_w, value_w, gap) = (200.0, 110.0, 32.0);
            let (rect, _) = ui.allocate_exact_size(
                vec2(2.0 * (label_w + value_w) + gap, 3.0 * row_h),
                Sense::hover(),
            );
            let font = egui::TextStyle::Body.resolve(ui.style());
            let text = ui.visuals().text_color();
            for (n, i) in [0, 1, 2, 3, 4, 5].into_iter().enumerate() {
                let (label, v, is_diff) = &rows[i];
                let col = (n / 3) as f32;
                let y = rect.top() + (n % 3) as f32 * row_h + row_h / 2.0;
                let x = rect.left() + col * (label_w + value_w + gap);
                ui.painter().text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    font.clone(),
                    text,
                );
                let color = match (*is_diff, *v == 0) {
                    (true, true) => GREEN,
                    (true, false) => RED,
                    _ => text,
                };
                ui.painter().text(
                    egui::pos2(x + label_w + value_w, y),
                    egui::Align2::RIGHT_CENTER,
                    fmt_cents(*v),
                    font.clone(),
                    color,
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Max), |ui| {
                self.finish_buttons(ui);
            });
        });
    }

    /// One reconcile list: A's two, or C's combined one (signed amounts).
    fn list(
        &mut self,
        ui: &mut Ui,
        key: &str,
        title: &str,
        ids: &[u32],
        height: f32,
        signed: bool,
    ) {
        let row_h = self.theme.row_height();
        let a = &self.accts[self.cur];
        let ticked: i64 = ids
            .iter()
            .filter_map(|id| a.idx(*id))
            .filter(|i| a.txns[*i].clr == Clr::Cleared)
            .map(|i| a.txns[i].cents)
            .sum();
        ui.horizontal(|ui| {
            ui.strong(format!("{title} ({})", ids.len()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "ticked {}",
                        fmt_cents(if signed { ticked } else { ticked.abs() })
                    ))
                    .weak(),
                );
            });
        });
        let rect = ui.available_rect_before_wrap();
        self.h.region(
            egui::Rect::from_min_size(rect.min, vec2(rect.width(), height - 24.0)),
            format!("{} list", title.to_lowercase()),
        );
        let tick_anywhere = self.opt(TICK_BY) == 0;
        let sel = self.sel;
        let mut acts = Vec::new();
        let mut tracked = Vec::new();
        ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
        TableBuilder::new(ui)
            .id_salt(("rc_list", key))
            .striped(true)
            .resizable(false)
            .auto_shrink(false)
            .max_scroll_height(height - 24.0 - row_h)
            .min_scrolled_height(height - 24.0 - row_h)
            .sense(Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(26.0))
            .column(Column::exact(80.0))
            .column(Column::exact(44.0))
            .column(Column::remainder().at_least(80.0).clip(true))
            .column(Column::exact(92.0))
            .header(row_h, |mut header| {
                for (label, right) in [
                    ("", false),
                    ("Date", false),
                    ("Num", false),
                    ("Payee", false),
                    ("Amount", true),
                ] {
                    header.col(|ui| {
                        if right {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.strong(label)
                            });
                        } else {
                            ui.strong(label);
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(row_h, ids.len(), |mut row| {
                    let id = ids[row.index()];
                    let t = &a.txns[a.idx(id).unwrap()];
                    row.set_selected(sel == Some(id));
                    let label = format!("{} {} {}", t.payee, fmt_day(t.day), fmt_cents(t.cents));
                    row.col(|ui| {
                        let r = tick_box(ui, t.clr == Clr::Cleared);
                        tracked.push((r.clone(), format!("tick box, {label}")));
                        if r.clicked() {
                            acts.push(Act::Tick(id, "tick box"));
                        }
                    });
                    row.col(|ui| {
                        ui.label(fmt_day(t.day));
                    });
                    row.col(|ui| {
                        ui.label(t.num.map(|n| n.to_string()).unwrap_or_default());
                    });
                    row.col(|ui| {
                        ui.label(&t.payee);
                    });
                    row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if signed {
                                ui.label(if t.cents < 0 {
                                    RichText::new(fmt_cents(t.cents)).color(RED)
                                } else {
                                    RichText::new(fmt_cents(t.cents))
                                });
                            } else {
                                ui.label(fmt_cents(t.cents.abs()));
                            }
                        });
                    });
                    let r = row.response();
                    tracked.push((r.clone(), format!("reconcile row, {label}")));
                    if r.double_clicked() {
                        acts.push(Act::Edit("reconcile row double-click"));
                    } else if r.clicked() {
                        if tick_anywhere {
                            acts.push(Act::Tick(id, "row click"));
                        } else {
                            acts.push(Act::Select(
                                id,
                                if signed { "panel row" } else { "list row" },
                            ));
                        }
                    }
                });
            });
        for (r, name) in tracked {
            self.h.track(&r, name);
        }
        // A tick by row click in C also brings the Register to the row.
        for act in &acts {
            if let (true, Act::Tick(id, _)) = (signed, act) {
                self.reg_scroll = self.reg_rows.iter().position(|r| r == id);
            }
        }
        self.acts.extend(acts);
        self.lists.push(ids.to_vec());
    }

    /// B's reconcile bar, above or below the Register.
    fn reconcile_bar(&mut self, ui: &mut Ui) {
        let frame = egui::Frame::new()
            .fill(self.theme.header_fill())
            .stroke(Stroke::new(
                1.0,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .inner_margin(egui::Margin::symmetric(10, 6))
            .corner_radius(4.0);
        let r = frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Reconciling {}", self.acct().name)).strong());
                ui.separator();
                self.statement_line(ui);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    self.finish_buttons(ui);
                    if self.btn(ui, "Unmark all", "Unmark all", true) {
                        self.acts.push(Act::MarkAll(false));
                    }
                    if self.btn(ui, "Mark all", "Mark all", true) {
                        self.acts.push(Act::MarkAll(true));
                    }
                });
            });
            ui.add_space(2.0);
            let rows = self.figure_rows();
            ui.horizontal_wrapped(|ui| {
                for (i, (label, v, is_diff)) in rows.iter().enumerate() {
                    if i > 0 {
                        ui.label(RichText::new("·").weak());
                    }
                    ui.label(RichText::new(label.as_str()).weak());
                    ui.label(if *is_diff {
                        self.diff_text(*v)
                    } else {
                        RichText::new(fmt_cents(*v))
                    });
                }
            });
            ui.label(
                RichText::new("Click a Clr cell, or select a row and press Space, to tick it.")
                    .small()
                    .weak(),
            );
        });
        self.h.region(r.response.rect, "reconcile bar");
    }

    /// C's docked side panel.
    fn side_panel(&mut self, ui: &mut Ui) {
        Panel::right("reconcile_panel")
            .default_size(470.0)
            .resizable(true)
            .frame(
                self.theme
                    .chrome_frame()
                    .inner_margin(egui::Margin::symmetric(10, 8)),
            )
            .show(ui, |ui| {
                let rect = ui.max_rect();
                self.h.region(rect, "reconcile panel");
                let name = self.acct().name;
                Sides::new().show(
                    ui,
                    |ui| {
                        ui.label(RichText::new(format!("Reconcile {name}")).heading());
                    },
                    |ui| {
                        let r = ui.small_button("Close").on_hover_text("Finish later");
                        self.h.track(&r, "panel close");
                        if r.clicked() {
                            self.acts.push(Act::Later("panel Close"));
                        }
                    },
                );
                ui.horizontal_wrapped(|ui| self.statement_line(ui));
                ui.separator();
                let figs_h = 170.0;
                let above = self.opt(FIGS) == 0;
                if above {
                    self.panel_figures(ui);
                    ui.separator();
                }
                let a = self.acct();
                let later = self.later();
                let set: Vec<u32> = a
                    .txns
                    .iter()
                    .filter(|t| a.in_set(t, later))
                    .map(|t| t.id)
                    .collect();
                let h = if above {
                    ui.available_height()
                } else {
                    (ui.available_height() - figs_h).max(120.0)
                };
                let w = ui.available_width();
                ui.allocate_ui(vec2(w, h), |ui| {
                    self.list(ui, "panel", "To reconcile", &set, h, true);
                });
                if !above {
                    ui.separator();
                    self.panel_figures(ui);
                }
            });
    }

    fn panel_figures(&mut self, ui: &mut Ui) {
        let rows = self.figure_rows();
        let r = ui.vertical(|ui| {
            egui::Grid::new("rc_panel_figs")
                .num_columns(2)
                .spacing(vec2(24.0, 2.0))
                .show(ui, |ui| {
                    for (label, v, is_diff) in &rows {
                        ui.label(label.as_str());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(if *is_diff {
                                self.diff_text(*v)
                            } else {
                                RichText::new(fmt_cents(*v))
                            });
                        });
                        ui.end_row();
                    }
                });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if self.btn(ui, "Mark all", "Mark all", true) {
                    self.acts.push(Act::MarkAll(true));
                }
                if self.btn(ui, "Unmark all", "Unmark all", true) {
                    self.acts.push(Act::MarkAll(false));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    self.finish_buttons(ui);
                });
            });
        });
        self.h.region(r.response.rect, "reconcile figures");
    }
}

// ---------------------------------------------------------------------------------------------
// Light Dialogs

impl Proto {
    fn sub_modal(&mut self, ctx: &Context, control_ok: bool) {
        let Some(mut sub) = self.sub.take() else {
            return;
        };
        let enter = control_ok || ctx.input(|i| i.key_pressed(Key::Enter));
        let mut done = false;
        let mut after: Option<Box<dyn FnOnce(&mut Self)>> = None;
        let accts = &self.accts;
        let cur = self.cur;
        let recon = self.recon;
        let mut tracked: Vec<(egui::Response, String)> = Vec::new();
        let modal = egui::Modal::new(Id::new(("rc_sub", sub.name()))).show(ctx, |ui| {
            ui.set_width(Theme::LIGHT_DIALOG_WIDTH + 60.0);
            match &mut sub {
                Sub::Start {
                    acct,
                    date,
                    ending,
                    err,
                    editing,
                    fresh,
                } => {
                    let a = &accts[*acct];
                    ui.heading(if *editing {
                        format!("Edit statement · {}", a.name)
                    } else {
                        format!("Reconcile {}", a.name)
                    });
                    ui.add_space(4.0);
                    if a.stmt.is_some() && !*editing {
                        ui.label(
                            RichText::new("Continuing the statement you started earlier.").weak(),
                        );
                    }
                    ui.label(
                        RichText::new(format!(
                            "Opening balance{} (from reconciled transactions): {}",
                            if a.card { " owed" } else { "" },
                            fmt_cents(a.shown(a.opening()))
                        ))
                        .weak(),
                    );
                    ui.add_space(6.0);
                    ui.label("Statement date");
                    let r = ui.add(TextEdit::singleline(date).desired_width(140.0));
                    tracked.push((r.clone(), "start: statement date".to_owned()));
                    if *fresh {
                        r.request_focus();
                        *fresh = false;
                    }
                    ui.add_space(4.0);
                    ui.label(if a.card {
                        "Statement balance owed (as printed)"
                    } else {
                        "Statement ending balance"
                    });
                    let r = ui.add(TextEdit::singleline(ending).desired_width(140.0));
                    tracked.push((r, "start: ending balance".to_owned()));
                    if let Some(e) = err {
                        ui.colored_label(RED, e.as_str());
                    }
                    ui.add_space(8.0);
                    let mut go = enter;
                    ui.horizontal(|ui| {
                        let r = ui.add(
                            egui::Button::new(if *editing { "OK" } else { "Start" })
                                .min_size(vec2(70.0, 0.0)),
                        );
                        tracked.push((r.clone(), "start: Start".to_owned()));
                        go |= r.clicked();
                        let r = ui.button("Cancel");
                        tracked.push((r.clone(), "start: Cancel".to_owned()));
                        if r.clicked() {
                            done = true;
                        }
                    });
                    if go && !done {
                        match (parse_day(date), parse_cents(ending)) {
                            (None, _) => *err = Some("Enter the statement date, like 10/15/2026.".to_owned()),
                            (_, None) => {
                                *err = Some("Enter the ending balance from your statement.".to_owned());
                            }
                            (Some(day), Some(end)) => {
                                let (acct, editing) = (*acct, *editing);
                                after = Some(Box::new(move |s: &mut Self| s.begin(acct, day, end, editing)));
                                done = true;
                            }
                        }
                    }
                }
                Sub::Adjust { cat } => {
                    let a = &accts[recon.unwrap_or(cur)];
                    let diff = a.difference().unwrap_or(0);
                    let (day, _) = a.stmt.unwrap_or((TODAY, 0));
                    ui.heading("The statement doesn't balance");
                    ui.add_space(4.0);
                    ui.label(format!(
                        "The cleared transactions differ from the statement by {}.",
                        fmt_cents(diff)
                    ));
                    ui.label(format!(
                        "Balanced can add a Balance Adjustment of {} dated {}, marked Reconciled, and finish.",
                        fmt_cents(diff),
                        fmt_day(day)
                    ));
                    ui.add_space(6.0);
                    ui.label("Category");
                    egui::ComboBox::from_id_salt("adjust_cat")
                        .selected_text(cat.map_or("Pick a category", |c| ADJUST_CATEGORIES[c]))
                        .width(240.0)
                        .show_ui(ui, |ui| {
                            for (i, c) in ADJUST_CATEGORIES.iter().enumerate() {
                                ui.selectable_value(cat, Some(i), *c);
                            }
                        });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let ok = cat.is_some();
                        let r = ui.add_enabled(ok, egui::Button::new("Add Adjustment and Finish"));
                        tracked.push((r.clone(), "adjust: Add Adjustment".to_owned()));
                        if r.clicked() || (enter && ok) {
                            let c = cat.unwrap();
                            after = Some(Box::new(move |s: &mut Self| s.finish(Some(c), "adjustment")));
                            done = true;
                        }
                        let r = ui.button("Go Back");
                        tracked.push((r.clone(), "adjust: Go Back".to_owned()));
                        if r.clicked() {
                            done = true;
                        }
                    });
                }
                Sub::Edit {
                    id,
                    amount,
                    err,
                    fresh,
                } => {
                    let a = &accts[cur];
                    let t = a.idx(*id).map(|i| a.txns[i].clone());
                    ui.heading("Edit Transaction");
                    ui.label(
                        RichText::new("Stand-in for the Register's row edit.").small().weak(),
                    );
                    if let Some(t) = &t {
                        ui.add_space(4.0);
                        ui.label(format!(
                            "{}  {}  {}",
                            fmt_day(t.day),
                            t.num.map(|n| format!("#{n}")).unwrap_or_default(),
                            t.payee
                        ));
                    }
                    ui.add_space(4.0);
                    ui.label("Amount (negative for money out)");
                    let r = ui.add(TextEdit::singleline(amount).desired_width(140.0));
                    tracked.push((r.clone(), "edit: amount".to_owned()));
                    if *fresh {
                        r.request_focus();
                        *fresh = false;
                    }
                    if let Some(e) = err {
                        ui.colored_label(RED, e.as_str());
                    }
                    ui.add_space(8.0);
                    let mut go = enter;
                    ui.horizontal(|ui| {
                        let r = ui.add(egui::Button::new("Save").min_size(vec2(70.0, 0.0)));
                        tracked.push((r.clone(), "edit: Save".to_owned()));
                        go |= r.clicked();
                        let r = ui.button("Cancel");
                        tracked.push((r.clone(), "edit: Cancel".to_owned()));
                        if r.clicked() {
                            done = true;
                        }
                    });
                    if go && !done {
                        match parse_cents(amount) {
                            None => *err = Some("Enter an amount.".to_owned()),
                            Some(cents) => {
                                let id = *id;
                                after = Some(Box::new(move |s: &mut Self| {
                                    let a = s.cur;
                                    if let Some(i) = s.accts[a].idx(id) {
                                        s.accts[a].txns[i].cents = cents;
                                    }
                                    let mut f = s.describe(a, id);
                                    f["would_emit"] = json!(format!("EditTransaction {{ Amount: {} }}", fmt_cents(cents)));
                                    f["figures"] = s.figures();
                                    s.event("command", f);
                                }));
                                done = true;
                            }
                        }
                    }
                }
                Sub::New {
                    date,
                    payee,
                    category,
                    payment,
                    deposit,
                    err,
                    fresh,
                } => {
                    let card = accts[cur].card;
                    ui.heading("New Transaction");
                    ui.label(
                        RichText::new("Stand-in for the Register's draft row.").small().weak(),
                    );
                    ui.add_space(4.0);
                    egui::Grid::new("new_txn").num_columns(2).show(ui, |ui| {
                        ui.label("Date");
                        let r = ui.add(TextEdit::singleline(date).desired_width(140.0));
                        tracked.push((r.clone(), "new: date".to_owned()));
                        ui.end_row();
                        ui.label("Payee");
                        let r2 = ui.add(TextEdit::singleline(payee).desired_width(220.0));
                        tracked.push((r2.clone(), "new: payee".to_owned()));
                        if *fresh {
                            r2.request_focus();
                            *fresh = false;
                        }
                        ui.end_row();
                        ui.label("Category");
                        let r = ui.add(TextEdit::singleline(category).desired_width(220.0));
                        tracked.push((r, "new: category".to_owned()));
                        ui.end_row();
                        ui.label(if card { "Charge" } else { "Payment" });
                        let r = ui.add(TextEdit::singleline(payment).desired_width(140.0));
                        tracked.push((r, "new: payment".to_owned()));
                        ui.end_row();
                        ui.label(if card { "Payment" } else { "Deposit" });
                        let r = ui.add(TextEdit::singleline(deposit).desired_width(140.0));
                        tracked.push((r, "new: deposit".to_owned()));
                        ui.end_row();
                    });
                    if let Some(e) = err {
                        ui.colored_label(RED, e.as_str());
                    }
                    ui.add_space(8.0);
                    let mut go = enter;
                    ui.horizontal(|ui| {
                        let r = ui.add(egui::Button::new("Save").min_size(vec2(70.0, 0.0)));
                        tracked.push((r.clone(), "new: Save".to_owned()));
                        go |= r.clicked();
                        let r = ui.button("Cancel");
                        tracked.push((r.clone(), "new: Cancel".to_owned()));
                        if r.clicked() {
                            done = true;
                        }
                    });
                    if go && !done {
                        let out = parse_cents(payment).unwrap_or(0);
                        let inn = parse_cents(deposit).unwrap_or(0);
                        match parse_day(date) {
                            None => *err = Some("Enter a date, like 10/15/2026.".to_owned()),
                            Some(day) => {
                                let (p, c) = (payee.trim().to_owned(), category.trim().to_owned());
                                after = Some(Box::new(move |s: &mut Self| {
                                    let a = s.cur;
                                    let acct = &mut s.accts[a];
                                    let id = acct.txns.iter().map(|t| t.id).max().unwrap_or(0) + 1;
                                    acct.txns.push(Txn {
                                        id,
                                        day,
                                        num: None,
                                        payee: p,
                                        cat: if c.is_empty() { "Uncategorized".to_owned() } else { c },
                                        cents: inn - out,
                                        clr: Clr::Uncleared,
                                        on_paper: None,
                                    });
                                    acct.sort();
                                    s.sel = Some(id);
                                    let mut f = s.describe(a, id);
                                    f["would_emit"] = json!("AddTransaction");
                                    s.event("command", f);
                                }));
                                done = true;
                            }
                        }
                    }
                }
            }
        });
        for (r, name) in tracked {
            self.h.track(&r, name);
        }
        if modal.should_close() && !done {
            done = true;
            self.event(
                "sub_closed",
                json!({ "dialog": sub.name(), "via": "Esc or backdrop" }),
            );
        }
        if !done {
            self.sub = Some(sub);
        } else if let Some(f) = after {
            f(self);
        } else {
            self.event(
                "sub_closed",
                json!({ "dialog": sub.name(), "via": "button" }),
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------

fn main() -> eframe::Result {
    // Prints the stub figures and exits, so they can be checked without a window.
    if std::env::var_os("RECONCILE_PROTO_DUMP").is_some() {
        let mut accts = generate();
        for a in &mut accts {
            println!(
                "{}: {} txns, first {} last {}; current {} ending {} cleared {} opening {}",
                a.name,
                a.txns.len(),
                fmt_day(a.txns[0].day),
                fmt_day(a.txns.last().unwrap().day),
                fmt_cents(a.current()),
                fmt_cents(a.ending()),
                fmt_cents(a.cleared()),
                fmt_cents(a.opening())
            );
            let min = a
                .txns
                .iter()
                .scan(0, |b, t| {
                    *b += t.cents;
                    Some(*b)
                })
                .min()
                .unwrap();
            println!("  lowest running balance {}", fmt_cents(min));
            println!(
                "  paper {} {} ; remembered {:?}",
                fmt_day(a.paper.0),
                fmt_cents(a.paper.1),
                a.stmt.map(|(d, e)| (fmt_day(d), fmt_cents(e)))
            );
            a.stmt = Some(a.paper);
            let set: Vec<&Txn> = a.txns.iter().filter(|t| a.in_set(t, false)).collect();
            let all: usize = a.txns.iter().filter(|t| a.in_set(t, true)).count();
            println!(
                "  set {} (with later uncleared {}), on paper {}, cleared now {}",
                set.len(),
                all,
                a.txns.iter().filter(|t| t.on_paper.is_some()).count(),
                set.iter().filter(|t| t.clr == Clr::Cleared).count()
            );
            println!("  difference now {}", fmt_cents(a.difference().unwrap()));
            for t in &mut a.txns {
                if t.on_paper.is_some() {
                    t.clr = Clr::Cleared;
                }
            }
            println!(
                "  difference with every paper item ticked {}",
                fmt_cents(a.difference().unwrap())
            );
            for t in a.txns.iter().filter(|t| t.day > a.paper.0 - 3) {
                println!(
                    "    {} {:>5} {:<24} {:>10} {:?} {:?}",
                    fmt_day(t.day),
                    t.num.map(|n| n.to_string()).unwrap_or_default(),
                    t.payee,
                    fmt_cents(t.cents),
                    t.clr,
                    t.on_paper.map(fmt_cents)
                );
            }
        }
        println!("today {} start {}", fmt_day(TODAY), fmt_day(START));
        return Ok(());
    }
    let args = Args::from_env();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Balanced — PROTOTYPE Reconcile")
            .with_maximized(true)
            .with_inner_size([1600.0, 1000.0]),
        ..Default::default()
    };
    eframe::run_native(
        "balanced_reconcile_prototype",
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
