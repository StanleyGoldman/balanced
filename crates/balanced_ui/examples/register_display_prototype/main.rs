//! PROTOTYPE, throwaway: Quicken-style register display and chrome.
//!
//! Question (ticket #218): which of Quicken classic's register display items, the chrome around
//! the register and the Add-account button does Balanced want, and in what form?
//!
//! Applied from the grilling on #218: one Register in the Main view (no Register tabs, #208) and
//! an Account bar that folds only by its own button (no auto-fold, #209).
//!
//! Two variants differ in where the chrome sits:
//!   A  Quicken classic: filter bar on its own row, Account Actions ⚙ in the context header,
//!      balances in a footer strip under the register
//!   B  Compact: search and a Filters button in the context header, one ⚙ in the register
//!      header holding the column chooser and the account actions, balances in the header
//!
//! Toggles (1–9, 0) switch each display item on and off; all start off, which is today's
//! Balanced.
//!
//! Run:
//!   cargo run -p balanced_ui --example register_display_prototype -- \
//!     [--variant a|b] [--log <dir>/events.jsonl] [--control <dir>/control.txt]
//!
//! Control-file verbs (besides the harness's `prompt` and `spotlight`): `variant <key>`,
//! `toggle <key|number> [on|off]`, `screenshot [label]`, `dock on|off`, `reload`, `quit`,
//! `account <index|name>`, `tab <index|name>`, `row <n>` (the nth shown row, negative counts
//! from the end), `today` (scroll to today), `fold <group>`, `unfold <group>`,
//! `bar fold|open`, `search <text>`, `catpick` (open the Category list on the selected row).
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

use std::collections::BTreeSet;

use balanced_prototype_harness::{Args, Harness};
use eframe::egui;
use egui::{
    Align, Align2, Button, CentralPanel, Color32, Context, Id, Key, Layout, Order, Panel, Pos2,
    Rect, RichText, Sense, Shape, Sides, Stroke, Ui, ViewportBuilder, ViewportCommand, pos2, vec2,
};
use egui_extras::{Column, TableBuilder};
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
// Dates

/// 2026-10-18, as days since 1970-01-01.
const TODAY: i64 = 20_744;
const HISTORY_DAYS: i64 = 3_650;
const FUTURE_DAYS: i64 = 35;
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

fn fmt_day(days: i64) -> String {
    let (y, m, d) = civil(days);
    format!("{m}/{d}/{y}")
}

const MONTHS: [&str; 12] = [
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
];

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
// Stub data: fixed seed, realistic volume

const SEED: u64 = 0x00ba_1a9c_ed77;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    Banking,
    Investing,
    PropertyDebt,
}

impl Group {
    const ALL: [Self; 3] = [Self::Banking, Self::Investing, Self::PropertyDebt];

    fn name(self) -> &'static str {
        match self {
            Self::Banking => "Banking",
            Self::Investing => "Investing",
            Self::PropertyDebt => "Property & Debt",
        }
    }

    fn kinds(self) -> &'static [Kind] {
        match self {
            Self::Banking => &[Kind::Checking, Kind::Savings, Kind::Credit],
            Self::Investing => &[Kind::Investment, Kind::Retirement],
            Self::PropertyDebt => &[Kind::Asset, Kind::Loan],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Checking,
    Savings,
    Credit,
    Investment,
    Retirement,
    Asset,
    Loan,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Checking => "Checking",
            Self::Savings => "Savings",
            Self::Credit => "Credit card",
            Self::Investment => "Brokerage",
            Self::Retirement => "Retirement",
            Self::Asset => "Asset",
            Self::Loan => "Loan",
        }
    }

    /// The (money out, money in) column headings, as Quicken names them for this kind.
    fn out_in(self) -> (&'static str, &'static str) {
        match self {
            Self::Credit => ("Charge", "Payment"),
            Self::Asset | Self::Loan => ("Decrease", "Increase"),
            _ => ("Payment", "Deposit"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Clr {
    None,
    Cleared,
    Reconciled,
}

impl Clr {
    fn text(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Cleared => "c",
            Self::Reconciled => "R",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cat {
    Opening,
    Category(&'static str),
    /// A transfer to or from the account at this index.
    Transfer(usize),
}

#[derive(Clone)]
struct Txn {
    id: u32,
    day: i64,
    num: String,
    payee: String,
    cat: Cat,
    memo: String,
    amount: i64,
    clr: Clr,
    flag: bool,
    /// The id shared with the mirrored transaction in the other account.
    pair: Option<u32>,
}

struct Account {
    name: &'static str,
    kind: Kind,
    closed: bool,
    txns: Vec<Txn>,
    /// The running balance after each transaction, in `txns` order.
    balances: Vec<i64>,
}

impl Account {
    fn recompute(&mut self) {
        self.txns.sort_by_key(|t| (t.day, t.id));
        let mut b = 0;
        self.balances = self
            .txns
            .iter()
            .map(|t| {
                b += t.amount;
                b
            })
            .collect();
    }

    fn ending(&self) -> i64 {
        self.balances.last().copied().unwrap_or(0)
    }

    fn current(&self) -> i64 {
        self.txns
            .iter()
            .zip(&self.balances)
            .rev()
            .find(|(t, _)| t.day <= TODAY)
            .map_or(0, |(_, b)| *b)
    }

    fn cleared(&self) -> i64 {
        self.txns
            .iter()
            .filter(|t| t.clr != Clr::None)
            .map(|t| t.amount)
            .sum()
    }
}

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

    /// Roughly `typical` cents, ±60%.
    fn around(&mut self, typical: i64) -> i64 {
        let spread = (typical * 6 / 5).max(1) as u64;
        (typical * 2 / 5 + self.below(spread) as i64).max(1)
    }
}

const CHECKING: usize = 0;
const SAVINGS: usize = 4;
const VISA: usize = 6;

/// (name, kind, transactions for the simple generator, opening balance in cents, closed)
const ACCOUNT_SPECS: &[(&str, Kind, usize, i64, bool)] = &[
    ("Everyday Checking", Kind::Checking, 0, 320_000, false),
    ("Joint Checking", Kind::Checking, 1_200, 400_000, false),
    ("Cash", Kind::Checking, 400, 20_000, false),
    ("Old Checking", Kind::Checking, 300, 80_000, true),
    ("High-Yield Savings", Kind::Savings, 0, 1_000_000, false),
    ("Emergency Fund", Kind::Savings, 300, 1_500_000, false),
    ("Visa Rewards", Kind::Credit, 0, 0, false),
    ("Mastercard", Kind::Credit, 800, 0, false),
    ("Brokerage", Kind::Investment, 600, 2_000_000, false),
    ("401(k)", Kind::Retirement, 500, 3_000_000, false),
    ("Roth IRA", Kind::Retirement, 300, 800_000, false),
    ("House", Kind::Asset, 200, 35_000_000, false),
    ("Car", Kind::Asset, 200, 2_800_000, false),
    ("Mortgage", Kind::Loan, 400, -28_000_000, false),
    ("Car Loan", Kind::Loan, 300, -2_400_000, false),
    ("Student Loan", Kind::Loan, 300, -3_500_000, false),
];

/// Everyday spending: (payee, category, typical cents).
const SPENDING: &[(&str, &str, i64)] = &[
    ("Grocery Mart", "Food:Groceries", 9_000),
    ("Corner Bakery", "Food:Groceries", 1_800),
    ("Coffee Corner", "Food:Dining", 650),
    ("Noodle House", "Food:Dining", 4_200),
    ("Gas & Go", "Auto:Fuel", 4_800),
    ("Quick Lube", "Auto:Service", 7_500),
    ("Book Barn", "Leisure:Books", 2_400),
    ("Cinema 8", "Leisure:Movies", 2_800),
    ("Pharmacy Plus", "Medical:Prescriptions", 2_600),
    ("Hardware Haus", "Home:Repairs", 6_500),
    ("Online Market", "Shopping:General", 5_500),
];

/// Every category the Category list offers.
const CATEGORIES: &[&str] = &[
    "Auto:Fuel",
    "Auto:Service",
    "Cash",
    "Food:Dining",
    "Food:Groceries",
    "Home:Repairs",
    "Housing:Rent",
    "Insurance:Auto",
    "Income:Interest",
    "Income:Salary",
    "Interest:Loans",
    "Investments:Unrealized",
    "Leisure:Books",
    "Leisure:Movies",
    "Medical:Prescriptions",
    "Property:Value Change",
    "Shopping:General",
    "Utilities:Electric",
    "Utilities:Internet",
    "Utilities:Water",
];

struct Gen {
    rng: Lcg,
    next_id: u32,
    check_no: u32,
}

impl Gen {
    fn id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    fn txn(&mut self, day: i64, num: &str, payee: &str, cat: Cat, memo: &str, amount: i64) -> Txn {
        Txn {
            id: self.id(),
            day,
            num: num.to_owned(),
            payee: payee.to_owned(),
            cat,
            memo: memo.to_owned(),
            amount,
            clr: Clr::None,
            flag: false,
            pair: None,
        }
    }
}

/// Adds a transfer of `amount` out of `from` into `to`, as a mirrored pair.
fn transfer(
    g: &mut Gen,
    accounts: &mut [Account],
    day: i64,
    from: usize,
    to: usize,
    payee: &str,
    memo: &str,
    amount: i64,
) {
    let pair = g.id();
    let mut out = g.txn(day, "TXFR", payee, Cat::Transfer(to), memo, -amount);
    out.pair = Some(pair);
    let mut inn = g.txn(day, "TXFR", payee, Cat::Transfer(from), memo, amount);
    inn.pair = Some(pair);
    accounts[from].txns.push(out);
    accounts[to].txns.push(inn);
}

fn generate() -> Vec<Account> {
    let mut g = Gen {
        rng: Lcg(SEED),
        next_id: 0,
        check_no: 1040,
    };
    let mut accounts: Vec<Account> = ACCOUNT_SPECS
        .iter()
        .map(|&(name, kind, _, opening, closed)| Account {
            name,
            kind,
            closed,
            txns: vec![g.txn(START, "", "Opening Balance", Cat::Opening, "", opening)],
            balances: Vec::new(),
        })
        .collect();

    // Everyday Checking, High-Yield Savings and Visa Rewards, day by day, every transfer
    // between them a mirrored pair.
    let mut chk = ACCOUNT_SPECS[CHECKING].3;
    let mut sav = ACCOUNT_SPECS[SAVINGS].3;
    let mut visa = 0;
    for day in START + 1..=TODAY + FUTURE_DAYS {
        let (_, m, dom) = civil(day);
        let future = day > TODAY;
        let month = MONTHS[(m - 1) as usize];
        if (day - START) % 14 == 3 {
            let pay = 275_000 + g.rng.below(9_000) as i64;
            let t = g.txn(
                day,
                "DEP",
                "Acme Corp",
                Cat::Category("Income:Salary"),
                "Payroll",
                pay,
            );
            accounts[CHECKING].txns.push(t);
            chk += pay;
        }
        if dom == 1 {
            g.check_no += 1;
            let num = g.check_no.to_string();
            let memo = format!("{month} rent");
            let t = g.txn(
                day,
                &num,
                "Maple Street Apartments",
                Cat::Category("Housing:Rent"),
                &memo,
                -240_000,
            );
            accounts[CHECKING].txns.push(t);
            chk -= 240_000;
            transfer(
                &mut g,
                &mut accounts,
                day,
                CHECKING,
                SAVINGS,
                "Monthly savings",
                "",
                50_000,
            );
            chk -= 50_000;
            sav += 50_000;
        }
        for (when, payee, cat, typical) in [
            (5, "City Power", "Utilities:Electric", 11_000),
            (10, "Water Works", "Utilities:Water", 4_500),
            (12, "Northside Insurance", "Insurance:Auto", 24_000),
            (20, "FiberNet", "Utilities:Internet", 6_999),
        ] {
            if dom == when {
                let amount = if payee == "FiberNet" || payee == "Northside Insurance" {
                    typical
                } else {
                    g.rng.around(typical)
                };
                let t = g.txn(day, "EFT", payee, Cat::Category(cat), "", -amount);
                accounts[CHECKING].txns.push(t);
                chk -= amount;
            }
        }
        if (dom == 7 || dom == 21) && !future {
            let t = g.txn(
                day,
                "ATM",
                "Cash withdrawal",
                Cat::Category("Cash"),
                "",
                -6_000,
            );
            accounts[CHECKING].txns.push(t);
            chk -= 6_000;
        }
        if dom == 15 && visa < 0 {
            let memo = format!("{month} statement");
            transfer(
                &mut g,
                &mut accounts,
                day,
                CHECKING,
                VISA,
                "Visa Rewards payment",
                &memo,
                -visa,
            );
            chk += visa;
            visa = 0;
        }
        if dom == 25 && chk > 1_500_000 {
            transfer(
                &mut g,
                &mut accounts,
                day,
                CHECKING,
                SAVINGS,
                "Sweep to savings",
                "",
                600_000,
            );
            chk -= 600_000;
            sav += 600_000;
        }
        if dom == 28 {
            let interest = (sav * 35 / 12_000).max(1);
            let t = g.txn(
                day,
                "",
                "Interest paid",
                Cat::Category("Income:Interest"),
                "",
                interest,
            );
            accounts[SAVINGS].txns.push(t);
            sav += interest;
        }
        if future {
            continue;
        }
        if g.rng.below(100) < 80 {
            let (p, c, typical) = SPENDING[g.rng.below(SPENDING.len() as u64) as usize];
            let amount = g.rng.around(typical);
            let memo = if g.rng.below(100) < 6 {
                "receipt in folder"
            } else {
                ""
            };
            let t = g.txn(day, "", p, Cat::Category(c), memo, -amount);
            accounts[CHECKING].txns.push(t);
            chk -= amount;
        }
        if g.rng.below(100) < 70 {
            let (p, c, typical) = SPENDING[g.rng.below(SPENDING.len() as u64) as usize];
            let amount = g.rng.around(typical);
            let t = g.txn(day, "", p, Cat::Category(c), "", -amount);
            accounts[VISA].txns.push(t);
            visa -= amount;
        }
    }

    // The other accounts: spread evenly over the history; their transfers point at Everyday
    // Checking but aren't mirrored there.
    for (idx, &(name, kind, n, opening, _)) in ACCOUNT_SPECS.iter().enumerate() {
        if n == 0 {
            continue;
        }
        let span = if name == "Old Checking" {
            HISTORY_DAYS - 730
        } else {
            HISTORY_DAYS
        };
        let mut balance = opening;
        for i in 0..n {
            let day = START + 1 + (i as i64 * span) / n as i64;
            let roll = g.rng.below(100);
            let (payee, cat, amount) = match kind {
                Kind::Checking if name == "Cash" && roll < 12 => {
                    ("Cash withdrawal", Cat::Transfer(CHECKING), 6_000)
                }
                Kind::Checking if name == "Cash" => {
                    let (p, c, typical) = SPENDING[g.rng.below(SPENDING.len() as u64) as usize];
                    (p, Cat::Category(c), -g.rng.around(typical / 3))
                }
                Kind::Checking if roll < 5 => (
                    "Paycheck",
                    Cat::Category("Income:Salary"),
                    g.rng.around(180_000),
                ),
                Kind::Checking => {
                    let (p, c, typical) = SPENDING[g.rng.below(SPENDING.len() as u64) as usize];
                    (p, Cat::Category(c), -g.rng.around(typical))
                }
                Kind::Savings if roll < 30 => (
                    "Interest paid",
                    Cat::Category("Income:Interest"),
                    g.rng.around(800),
                ),
                Kind::Savings => (
                    "Transfer from Checking",
                    Cat::Transfer(CHECKING),
                    g.rng.around(20_000),
                ),
                Kind::Credit if balance < -100_000 => {
                    ("Payment — Thank You", Cat::Transfer(CHECKING), -balance)
                }
                Kind::Credit => {
                    let (p, c, typical) = SPENDING[g.rng.below(SPENDING.len() as u64) as usize];
                    (p, Cat::Category(c), -g.rng.around(typical))
                }
                Kind::Investment | Kind::Retirement if roll < 40 => (
                    "Contribution",
                    Cat::Transfer(CHECKING),
                    g.rng.around(50_000),
                ),
                Kind::Investment | Kind::Retirement if roll < 75 => (
                    "Market gain",
                    Cat::Category("Investments:Unrealized"),
                    g.rng.around(60_000),
                ),
                Kind::Investment | Kind::Retirement => (
                    "Market loss",
                    Cat::Category("Investments:Unrealized"),
                    -g.rng.around(45_000),
                ),
                Kind::Asset if roll < if name == "Car" { 15 } else { 60 } => (
                    "Revaluation",
                    Cat::Category("Property:Value Change"),
                    g.rng.around(20_000),
                ),
                Kind::Asset => (
                    "Depreciation",
                    Cat::Category("Property:Value Change"),
                    -g.rng.around(if name == "Car" { 4_000 } else { 15_000 }),
                ),
                Kind::Loan if roll < 90 => (
                    "Principal payment",
                    Cat::Transfer(CHECKING),
                    g.rng.around(-opening * 8 / (10 * n as i64)),
                ),
                Kind::Loan => (
                    "Interest charged",
                    Cat::Category("Interest:Loans"),
                    -g.rng.around(-opening / (20 * n as i64)),
                ),
            };
            let floor = if name == "Cash" { 0 } else { -50_000 };
            let amount = if kind == Kind::Checking && balance + amount < floor {
                -amount
            } else {
                amount
            };
            balance += amount;
            let num = if matches!(cat, Cat::Transfer(_)) {
                "TXFR"
            } else {
                ""
            };
            let t = g.txn(day, num, payee, cat, "", amount);
            accounts[idx].txns.push(t);
        }
        if name == "Old Checking" {
            let t = g.txn(
                START + span + 1,
                "TXFR",
                "Closing transfer",
                Cat::Transfer(CHECKING),
                "",
                -balance,
            );
            accounts[idx].txns.push(t);
        }
    }

    // Cleared status and flags.
    for a in &mut accounts {
        for t in &mut a.txns {
            t.clr = if t.day < TODAY - 40 {
                Clr::Reconciled
            } else if t.day <= TODAY && g.rng.below(100) < 60 {
                Clr::Cleared
            } else {
                Clr::None
            };
            t.flag = t.day <= TODAY && t.cat != Cat::Opening && g.rng.below(1000) < 20;
        }
        a.recompute();
    }
    accounts
}

// ---------------------------------------------------------------------------------------------
// Variants and toggles

#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    A,
    B,
}

const VARIANT_KEYS: [(Key, &str); 2] = [(Key::A, "A"), (Key::B, "B")];

impl Variant {
    const ALL: [Self; 2] = [Self::A, Self::B];

    fn key(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|v| v.key().eq_ignore_ascii_case(key))
    }

    fn label(self) -> &'static str {
        match self {
            Self::A => "A · Quicken classic chrome",
            Self::B => "B · Compact chrome",
        }
    }

    fn step(self, delta: isize) -> Self {
        let n = Self::ALL.len() as isize;
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0) as isize;
        Self::ALL[(i + delta).rem_euclid(n) as usize]
    }
}

#[derive(Clone, Copy, Default)]
struct Opts {
    clr: bool,
    pay_dep: bool,
    transfers: bool,
    balances: bool,
    today_line: bool,
    future_shade: bool,
    flags: bool,
    filter_bar: bool,
    gear: bool,
    add_footer: bool,
}

impl Opts {
    fn get_mut(&mut self, key: &str) -> Option<&mut bool> {
        Some(match key {
            "clr" => &mut self.clr,
            "pay_dep" => &mut self.pay_dep,
            "transfers" => &mut self.transfers,
            "balances" => &mut self.balances,
            "today_line" => &mut self.today_line,
            "future_shade" => &mut self.future_shade,
            "flags" => &mut self.flags,
            "filter_bar" => &mut self.filter_bar,
            "gear" => &mut self.gear,
            "add_footer" => &mut self.add_footer,
            _ => return None,
        })
    }
}

/// (key, label); every toggle applies to both variants.
const TOGGLES: &[(&str, &str)] = &[
    ("clr", "Clr column (blank / c / R)"),
    ("pay_dep", "Payment + Deposit columns (off: Amount)"),
    (
        "transfers",
        "Transfers as [Account], accounts in the Category list",
    ),
    ("balances", "Cleared / Current / Ending balances"),
    ("today_line", "Today divider before future rows"),
    ("future_shade", "Shade future-dated rows"),
    ("flags", "Flag column"),
    ("filter_bar", "Filter bar: search, dates, type"),
    ("gear", "Account Actions ⚙ menu"),
    (
        "add_footer",
        "+ Add Account in the bar's footer (off: header)",
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum DateRange {
    All,
    Last30,
    Last12Months,
    ThisYear,
    LastYear,
}

impl DateRange {
    const ALL: [Self; 5] = [
        Self::All,
        Self::Last30,
        Self::Last12Months,
        Self::ThisYear,
        Self::LastYear,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::All => "All dates",
            Self::Last30 => "Last 30 days",
            Self::Last12Months => "Last 12 months",
            Self::ThisYear => "This year",
            Self::LastYear => "Last year",
        }
    }

    fn keeps(self, day: i64) -> bool {
        let (ty, _, _) = civil(TODAY);
        match self {
            Self::All => true,
            Self::Last30 => day > TODAY - 30 && day <= TODAY,
            Self::Last12Months => day > TODAY - 365 && day <= TODAY,
            Self::ThisYear => civil(day).0 == ty,
            Self::LastYear => civil(day).0 == ty - 1,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TxnType {
    All,
    Payments,
    Deposits,
    Uncleared,
    Flagged,
    Transfers,
}

impl TxnType {
    const ALL: [Self; 6] = [
        Self::All,
        Self::Payments,
        Self::Deposits,
        Self::Uncleared,
        Self::Flagged,
        Self::Transfers,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::All => "All transactions",
            Self::Payments => "Payments",
            Self::Deposits => "Deposits",
            Self::Uncleared => "Uncleared",
            Self::Flagged => "Flagged",
            Self::Transfers => "Transfers",
        }
    }

    fn keeps(self, t: &Txn) -> bool {
        match self {
            Self::All => true,
            Self::Payments => t.amount < 0,
            Self::Deposits => t.amount > 0,
            Self::Uncleared => t.clr == Clr::None,
            Self::Flagged => t.flag,
            Self::Transfers => matches!(t.cat, Cat::Transfer(_)),
        }
    }
}

#[derive(Clone, PartialEq)]
struct Filter {
    search: String,
    dates: DateRange,
    kind: TxnType,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            search: String::new(),
            dates: DateRange::All,
            kind: TxnType::All,
        }
    }
}

impl Filter {
    fn active(&self) -> bool {
        *self != Self::default()
    }
}

/// One shown line of the register.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Line {
    /// The index into the account's `txns`.
    Txn(usize),
    Today,
}

/// Columns the column chooser can hide.
#[derive(Clone, Copy)]
struct Shown {
    num: bool,
    memo: bool,
    balance: bool,
}

/// The open Category list, under one row's Category cell.
struct CatPick {
    account: usize,
    txn: u32,
    pos: Pos2,
    text: String,
    fresh: bool,
}

// ---------------------------------------------------------------------------------------------
// The app

const TABS: [&str; 7] = [
    "Home",
    "Accounts",
    "Bills",
    "Spending",
    "Planning",
    "Investing",
    "Property & Debt",
];
const ACCOUNTS_TAB: usize = 1;

struct Proto {
    h: Harness,
    accounts: Vec<Account>,
    selected: usize,
    /// The selected transaction's id.
    selected_txn: Option<u32>,
    variant: Variant,
    o: Opts,
    filter: Filter,
    shown: Shown,
    folded: BTreeSet<&'static str>,
    bar_folded: bool,
    tab: usize,
    theme: Theme,
    dock_open: bool,
    status: String,
    cat_pick: Option<CatPick>,
    /// The shown lines, and what they were built from.
    lines: Vec<Line>,
    lines_key: Option<(usize, Filter, bool, bool, u64)>,
    data_version: u64,
    scroll_to: Option<usize>,
    new_count: u32,
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
        let mut proto = Self {
            h,
            accounts: generate(),
            selected: CHECKING,
            selected_txn: None,
            variant,
            o: Opts::default(),
            filter: Filter::default(),
            shown: Shown {
                num: true,
                memo: true,
                balance: true,
            },
            folded: BTreeSet::new(),
            bar_folded: false,
            tab: ACCOUNTS_TAB,
            theme: Theme::new(ThemeSettings::default()),
            dock_open: true,
            status: "Ready".to_owned(),
            cat_pick: None,
            lines: Vec::new(),
            lines_key: None,
            data_version: 0,
            scroll_to: None,
            new_count: 0,
        };
        proto.open_account(CHECKING, "start");
        proto
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
        self.h.screenshot(format!("variant-{}", to.key()));
    }

    fn set_toggle(&mut self, key: &str, value: Option<bool>, via: &str) {
        let key = key
            .parse::<usize>()
            .ok()
            .and_then(|n| TOGGLES.get(if n == 0 { 9 } else { n - 1 }))
            .map_or(key, |t| t.0);
        let Some(slot) = self.o.get_mut(key) else {
            self.h.event("control_unknown", json!({ "toggle": key }));
            return;
        };
        let new = value.unwrap_or(!*slot);
        if *slot == new && via != "click" {
            return;
        }
        *slot = new;
        self.h.event(
            "toggle",
            json!({ "key": key, "value": new, "via": via, "applies_to": "all" }),
        );
        // The filter bar going away takes its filter with it.
        if key == "filter_bar" && !new {
            self.filter = Filter::default();
        }
        self.h
            .screenshot(format!("toggle-{key}-{}", if new { "on" } else { "off" }));
    }

    fn set_dock(&mut self, open: bool, via: &str) {
        self.dock_open = open;
        self.h.event("dock", json!({ "open": open, "via": via }));
    }

    fn account_index(&self, rest: &str) -> Option<usize> {
        rest.parse::<usize>()
            .ok()
            .filter(|i| *i < self.accounts.len())
            .or_else(|| {
                let needle = rest.to_lowercase();
                self.accounts
                    .iter()
                    .position(|a| a.name.to_lowercase().contains(&needle))
            })
    }

    /// Opens `i` in the one Register, in place of the account shown, scrolled to today.
    fn open_account(&mut self, i: usize, via: &str) {
        self.selected = i;
        self.selected_txn = None;
        self.cat_pick = None;
        self.tab = ACCOUNTS_TAB;
        self.h.event(
            "navigate",
            json!({ "to": self.accounts[i].name, "via": via }),
        );
        self.lines_key = None;
        self.scroll_to_today();
    }

    fn scroll_to_today(&mut self) {
        self.rebuild_lines();
        let a = &self.accounts[self.selected];
        let at = self
            .lines
            .iter()
            .position(|l| match *l {
                Line::Txn(i) => a.txns[i].day > TODAY,
                Line::Today => true,
            })
            .unwrap_or(self.lines.len());
        self.scroll_to = Some(at.saturating_sub(1));
    }

    fn rebuild_lines(&mut self) {
        let key = (
            self.selected,
            self.filter.clone(),
            self.o.today_line,
            self.o.transfers,
            self.data_version,
        );
        if self.lines_key.as_ref() == Some(&key) {
            return;
        }
        let a = &self.accounts[self.selected];
        let needle = self.filter.search.to_lowercase();
        let mut lines = Vec::with_capacity(a.txns.len() + 1);
        let mut today_placed = false;
        for (i, t) in a.txns.iter().enumerate() {
            if !self.filter.dates.keeps(t.day) || !self.filter.kind.keeps(t) {
                continue;
            }
            if !needle.is_empty() {
                let cat = cat_text(&self.accounts, t.cat, self.o.transfers).to_lowercase();
                if !t.payee.to_lowercase().contains(&needle)
                    && !cat.contains(&needle)
                    && !t.memo.to_lowercase().contains(&needle)
                {
                    continue;
                }
            }
            if self.o.today_line && !today_placed && t.day > TODAY {
                lines.push(Line::Today);
                today_placed = true;
            }
            lines.push(Line::Txn(i));
        }
        self.lines = lines;
        self.lines_key = Some(key);
    }

    fn net_worth(&self) -> i64 {
        self.accounts.iter().map(Account::current).sum()
    }

    fn find_txn(&self, account: usize, id: u32) -> Option<usize> {
        self.accounts[account].txns.iter().position(|t| t.id == id)
    }

    /// Sets a transaction's category or transfer, keeping a transfer's mirror in step.
    fn set_category(&mut self, account: usize, id: u32, cat: Cat) {
        let Some(i) = self.find_txn(account, id) else {
            return;
        };
        let old = self.accounts[account].txns[i].clone();
        if old.cat == cat {
            return;
        }
        if let Some(pair) = old.pair {
            for a in &mut self.accounts {
                a.txns.retain(|t| t.pair != Some(pair) || t.id == id);
            }
        }
        let t = &mut self.accounts[account].txns[i];
        t.cat = cat;
        t.pair = None;
        if let Cat::Transfer(other) = cat {
            let pair = 1_000_000 + id;
            t.pair = Some(pair);
            let mirror = Txn {
                id: 2_000_000 + id,
                day: t.day,
                num: "TXFR".to_owned(),
                payee: t.payee.clone(),
                cat: Cat::Transfer(account),
                memo: t.memo.clone(),
                amount: -t.amount,
                clr: Clr::None,
                flag: false,
                pair: Some(pair),
            };
            self.accounts[other].txns.push(mirror);
        }
        for a in &mut self.accounts {
            a.recompute();
        }
        self.data_version += 1;
        let what = match cat {
            Cat::Transfer(other) => format!(
                "DomainCommand::EditTransaction {{ transfer_to: {:?} }} (mirror written to {})",
                self.accounts[other].name, self.accounts[other].name
            ),
            Cat::Category(c) => format!("DomainCommand::EditTransaction {{ category: {c:?} }}"),
            Cat::Opening => String::new(),
        };
        self.emit(&what, "Category list");
    }

    fn add_account(&mut self, via: &str) {
        self.new_count += 1;
        self.emit(
            &format!("AppCommand::OpenNewAccountDialog (#{})", self.new_count),
            via,
        );
    }

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
            "account" => match self.account_index(rest) {
                Some(i) => self.open_account(i, "control"),
                None => bad(self),
            },
            "tab" => {
                let found = rest
                    .parse::<usize>()
                    .ok()
                    .filter(|i| *i < TABS.len())
                    .or_else(|| {
                        TABS.iter()
                            .position(|t| t.to_lowercase().contains(&rest.to_lowercase()))
                    });
                match found {
                    Some(t) => self.tab = t,
                    None => bad(self),
                }
            }
            "row" => {
                self.rebuild_lines();
                let n = self.lines.len() as i64;
                let at = rest
                    .parse::<i64>()
                    .ok()
                    .map(|r| if r < 0 { n + r } else { r });
                match at
                    .filter(|r| (0..n).contains(r))
                    .map(|r| self.lines[r as usize])
                {
                    Some(Line::Txn(i)) => {
                        self.selected_txn = Some(self.accounts[self.selected].txns[i].id);
                        self.scroll_to = Some(at.unwrap() as usize);
                    }
                    _ => bad(self),
                }
            }
            "today" => self.scroll_to_today(),
            "fold" | "unfold" => {
                let needle = rest.to_lowercase();
                match Group::ALL
                    .iter()
                    .map(|g| g.name())
                    .chain(["Closed"])
                    .find(|n| n.to_lowercase().contains(&needle))
                {
                    Some(n) if verb == "fold" => {
                        self.folded.insert(n);
                    }
                    Some(n) => {
                        self.folded.remove(n);
                    }
                    None => bad(self),
                }
            }
            "bar" => self.bar_folded = rest == "fold",
            "search" => {
                self.filter.search = rest.to_owned();
            }
            "catpick" => {
                if let Some(id) = self.selected_txn {
                    self.cat_pick = Some(CatPick {
                        account: self.selected,
                        txn: id,
                        pos: ctx.content_rect().center() - vec2(120.0, 0.0),
                        text: String::new(),
                        fresh: true,
                    });
                } else {
                    bad(self);
                }
            }
            "reload" => {
                self.accounts = generate();
                self.data_version += 1;
                self.h.event("reload", json!({ "seed": SEED }));
            }
            "screenshot" => self
                .h
                .screenshot(if rest.is_empty() { "control" } else { rest }),
            "dock" => self.set_dock(rest != "off", "control"),
            "quit" => ctx.send_viewport_cmd(ViewportCommand::Close),
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
        if let Some(cmd) = self.h.begin_frame(&ctx) {
            self.apply_control(&ctx, &cmd);
        }
        if !ctx.egui_wants_keyboard_input() && self.cat_pick.is_none() {
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
            for (&(key, _), (k, _)) in TOGGLES.iter().zip(TOGGLE_KEYS) {
                if ctx.input(|i| i.key_pressed(k) && i.modifiers.is_none()) {
                    self.set_toggle(key, None, "key");
                }
            }
        }
        if ctx.input(|i| i.key_pressed(Key::F4)) {
            self.set_dock(!self.dock_open, "key");
        }

        self.dock(ui);
        self.h.prompt_panel(ui);
        self.main_ui(ui, &ctx);
        self.category_list(&ctx);
        self.h.end_frame(&ctx);
    }
}

// ---------------------------------------------------------------------------------------------
// Main window

impl Proto {
    fn menu_item(&mut self, ui: &mut Ui, path: &str, label: &str) -> bool {
        let r = ui.add(Button::new(label));
        self.h.track(&r, format!("menu: {path} → {label}"));
        if r.clicked() {
            ui.close();
            return true;
        }
        false
    }

    fn main_ui(&mut self, ui: &mut Ui, ctx: &Context) {
        let menu = Panel::top("menu")
            .frame(self.theme.chrome_frame())
            .show(ui, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if self.menu_item(ui, "File", "New Account…") {
                            self.add_account("menu File");
                        }
                        ui.separator();
                        if self.menu_item(ui, "File", "Quit") {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                    });
                    ui.menu_button("Edit", |ui| {
                        for item in ["Undo", "Redo"] {
                            if self.menu_item(ui, "Edit", item) {
                                self.emit(&format!("AppCommand::{item}"), "menu Edit");
                            }
                        }
                    });
                    ui.menu_button("View", |ui| {
                        let label = if self.bar_folded {
                            "Unfold Account Bar"
                        } else {
                            "Fold Account Bar"
                        };
                        if self.menu_item(ui, "View", label) {
                            self.bar_folded = !self.bar_folded;
                            self.h.event(
                                "bar_fold",
                                json!({ "folded": self.bar_folded, "via": "menu" }),
                            );
                        }
                        if self.menu_item(ui, "View", "Switch Pane") {
                            self.emit("AppCommand::SwitchPane", "menu View");
                        }
                    });
                    ui.menu_button("Tools", |ui| {
                        for item in ["Manage Accounts…", "Preferences…"] {
                            if self.menu_item(ui, "Tools", item) {
                                self.emit(&format!("AppCommand::Open {item}"), "menu Tools");
                            }
                        }
                    });
                    ui.menu_button("Help", |ui| {
                        if self.menu_item(ui, "Help", "About Balanced") {
                            self.emit("AppCommand::About", "menu Help");
                        }
                    });
                });
            });
        self.h.region(menu.response.rect, "menu bar");
        let tabs = Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (t, name) in TABS.iter().enumerate() {
                    let r = ui.selectable_label(self.tab == t, *name);
                    self.h.track(&r, format!("tab: {name}"));
                    if r.clicked() {
                        self.tab = t;
                    }
                }
            });
        });
        self.h.region(tabs.response.rect, "tab row");
        let status = Panel::bottom("status")
            .frame(self.theme.chrome_frame().inner_margin(4))
            .show(ui, |ui| {
                ui.label(&self.status);
            });
        self.h.region(status.response.rect, "status bar");
        // The Account bar stays docked on every tab (#209): it folds only by its own button.
        if self.bar_folded {
            let rail = Panel::left("account_rail")
                .exact_size(26.0)
                .resizable(false)
                .frame(self.theme.chrome_frame().inner_margin(2))
                .show(ui, |ui| {
                    let r = ui.small_button("▶").on_hover_text("Unfold the Account bar");
                    self.h.track(&r, "account bar: unfold");
                    if r.clicked() {
                        self.bar_folded = false;
                        self.h
                            .event("bar_fold", json!({ "folded": false, "via": "click" }));
                    }
                });
            self.h.region(rail.response.rect, "account bar rail");
        } else {
            let bar = Panel::left("accounts")
                .default_size(270.0)
                .frame(self.theme.chrome_frame().inner_margin(4))
                .show(ui, |ui| self.account_bar(ui));
            self.h.region(bar.response.rect, "account bar");
        }
        CentralPanel::default().show(ui, |ui| {
            if self.tab != ACCOUNTS_TAB {
                ui.heading(TABS[self.tab]);
                ui.label(
                    RichText::new("Not in this prototype. The Account bar stays docked here.")
                        .weak(),
                );
                return;
            }
            self.register_view(ui);
        });
    }

    fn group_header(&mut self, ui: &mut Ui, name: &'static str, count: usize) {
        let folded = self.folded.contains(name);
        let h = self.theme.row_height() + 4.0;
        let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
        let v = ui.visuals().clone();
        if r.hovered() {
            ui.painter()
                .rect_filled(rect, 0.0, v.widgets.hovered.weak_bg_fill);
        }
        paint_triangle(
            ui.painter(),
            pos2(rect.left() + 8.0, rect.center().y),
            if folded { Dir::Right } else { Dir::Down },
            v.weak_text_color(),
        );
        let label = if folded {
            format!("{}  ({count})", name.to_uppercase())
        } else {
            name.to_uppercase()
        };
        ui.painter().text(
            pos2(rect.left() + 18.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(11.0),
            v.weak_text_color(),
        );
        self.h
            .track(&r, format!("account bar: group header {name}"));
        if r.clicked() {
            let now = if self.folded.remove(name) {
                false
            } else {
                self.folded.insert(name);
                true
            };
            self.h
                .event("group_fold", json!({ "group": name, "folded": now }));
        }
    }

    fn account_bar(&mut self, ui: &mut Ui) {
        let header = Panel::top("account_bar_header")
            .frame(egui::Frame::NONE)
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let r = ui.small_button("◀").on_hover_text("Fold the Account bar");
                    self.h.track(&r, "account bar: fold");
                    if r.clicked() {
                        self.bar_folded = true;
                        self.h
                            .event("bar_fold", json!({ "folded": true, "via": "click" }));
                    }
                    if !self.o.add_footer {
                        let r = ui.small_button("+ Account");
                        self.h.track(&r, "account bar: + Account (header)");
                        if r.clicked() {
                            self.add_account("account bar header");
                        }
                    }
                });
            });
        self.h.region(header.response.rect, "account bar header");
        let footer = Panel::bottom("account_bar_footer")
            .frame(self.theme.footer_frame())
            .show(ui, |ui| {
                let total = self.net_worth();
                Sides::new().show(
                    ui,
                    |ui| ui.strong("Net Worth"),
                    |ui| ui.label(money(total, true)),
                );
                if self.o.add_footer {
                    let r = ui.add_sized(
                        [ui.available_width(), self.theme.row_height() + 2.0],
                        Button::new("+ Add Account"),
                    );
                    self.h.track(&r, "account bar: + Add Account (footer)");
                    if r.clicked() {
                        self.add_account("account bar footer");
                    }
                }
            });
        self.h.region(footer.response.rect, "account bar footer");
        let mut pick: Option<(usize, &'static str)> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for group in Group::ALL {
                let ids: Vec<Vec<usize>> = group
                    .kinds()
                    .iter()
                    .map(|&k| {
                        (0..self.accounts.len())
                            .filter(|&i| self.accounts[i].kind == k && !self.accounts[i].closed)
                            .collect::<Vec<_>>()
                    })
                    .filter(|v| !v.is_empty())
                    .collect();
                let count = ids.iter().map(Vec::len).sum();
                ui.add_space(2.0);
                self.group_header(ui, group.name(), count);
                if self.folded.contains(group.name()) {
                    continue;
                }
                for (n, sub) in ids.into_iter().enumerate() {
                    if n > 0 {
                        let (rect, _) =
                            ui.allocate_exact_size(vec2(ui.available_width(), 5.0), Sense::hover());
                        ui.painter().hline(
                            rect.x_range().shrink(14.0),
                            rect.center().y,
                            Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                        );
                    }
                    for i in sub {
                        self.bar_account(ui, i, &mut pick);
                    }
                }
            }
            let closed: Vec<usize> = (0..self.accounts.len())
                .filter(|&i| self.accounts[i].closed)
                .collect();
            ui.add_space(6.0);
            self.group_header(ui, "Closed", closed.len());
            if !self.folded.contains("Closed") {
                for i in closed {
                    self.bar_account(ui, i, &mut pick);
                }
            }
        });
        let Some((i, what)) = pick else { return };
        match what {
            "select" => self.open_account(i, "click"),
            "reconcile" => self.emit("AppCommand::OpenReconcile", "account menu"),
            _ => self.emit("AppCommand::OpenManageAccountsDialog", "account menu"),
        }
    }

    fn bar_account(&mut self, ui: &mut Ui, i: usize, pick: &mut Option<(usize, &'static str)>) {
        let a = &self.accounts[i];
        let name = a.name;
        let balance = a.current();
        let selected = self.selected == i;
        let (rect, r) = ui.allocate_exact_size(
            vec2(ui.available_width(), self.theme.row_height()),
            Sense::click(),
        );
        let v = ui.visuals();
        if selected {
            ui.painter().rect_filled(rect, 0.0, v.selection.bg_fill);
        } else if r.hovered() {
            ui.painter()
                .rect_filled(rect, 0.0, v.widgets.hovered.weak_bg_fill);
        }
        let font = egui::TextStyle::Body.resolve(ui.style());
        ui.painter().text(
            pos2(rect.left() + 18.0, rect.center().y),
            Align2::LEFT_CENTER,
            name,
            font.clone(),
            v.text_color(),
        );
        ui.painter().text(
            pos2(rect.right() - 6.0, rect.center().y),
            Align2::RIGHT_CENTER,
            fmt_cents(balance),
            font,
            if balance < 0 {
                v.error_fg_color
            } else {
                v.text_color()
            },
        );
        self.h.track(&r, format!("account bar: {name}"));
        if r.clicked() {
            *pick = Some((i, "select"));
        }
        r.context_menu(|ui| {
            for (label, what) in [
                ("Open Register", "select"),
                ("Reconcile…", "reconcile"),
                ("Manage Accounts…", "manage"),
            ] {
                if self.menu_item(ui, "account menu", label) {
                    *pick = Some((i, what));
                }
            }
        });
    }

    /// The Account Actions menu: Quicken's ⚙ at the register's top right.
    fn account_actions(&mut self, ui: &mut Ui, place: &str) {
        for item in [
            "Reconcile…",
            "Edit account details…",
            "Register preferences…",
            "Print register…",
            "Export to CSV…",
        ] {
            if self.menu_item(ui, place, item) {
                self.emit(&format!("AppCommand::{item}"), place);
            }
        }
    }

    fn column_chooser(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Columns").weak());
        for (label, slot) in [
            ("Num", &mut self.shown.num),
            ("Memo", &mut self.shown.memo),
            ("Balance", &mut self.shown.balance),
        ] {
            let r = ui.checkbox(slot, label);
            if r.changed() {
                self.h
                    .event("column", json!({ "column": label, "shown": *slot }));
            }
            self.h.track(&r, format!("column chooser: {label}"));
        }
    }

    fn balances_text(&self) -> [(String, i64); 3] {
        let a = &self.accounts[self.selected];
        [
            ("Cleared".to_owned(), a.cleared()),
            ("Current".to_owned(), a.current()),
            ("Ending".to_owned(), a.ending()),
        ]
    }

    fn search_box(&mut self, ui: &mut Ui, width: f32, place: &str) {
        let r = ui.add(
            egui::TextEdit::singleline(&mut self.filter.search)
                .hint_text("Search payee, category, memo")
                .desired_width(width),
        );
        self.h.track(&r, format!("{place}: search"));
        if r.lost_focus() && !self.filter.search.is_empty() {
            self.h.event(
                "filter",
                json!({ "search": self.filter.search, "place": place }),
            );
        }
    }

    fn filter_combos(&mut self, ui: &mut Ui, place: &str) {
        let before = self.filter.clone();
        egui::ComboBox::from_id_salt(("dates", place))
            .selected_text(self.filter.dates.label())
            .show_ui(ui, |ui| {
                for d in DateRange::ALL {
                    ui.selectable_value(&mut self.filter.dates, d, d.label());
                }
            });
        egui::ComboBox::from_id_salt(("type", place))
            .selected_text(self.filter.kind.label())
            .show_ui(ui, |ui| {
                for k in TxnType::ALL {
                    ui.selectable_value(&mut self.filter.kind, k, k.label());
                }
            });
        let r = ui.add_enabled(self.filter.active(), Button::new("Reset"));
        self.h.track(&r, format!("{place}: Reset"));
        if r.clicked() {
            self.filter = Filter::default();
        }
        if before.dates != self.filter.dates || before.kind != self.filter.kind {
            self.h.event(
                "filter",
                json!({ "dates": self.filter.dates.label(), "type": self.filter.kind.label(), "place": place }),
            );
        }
    }

    fn register_view(&mut self, ui: &mut Ui) {
        let sel = self.selected;
        let closed = self.accounts[sel].closed;
        let name = self.accounts[sel].name;
        let current = self.accounts[sel].current();
        let variant = self.variant;
        let o = self.o;

        // Context header.
        let header = ui.horizontal(|ui| {
            ui.heading(name);
            ui.label(money(current, true).heading());
            if closed {
                ui.label(RichText::new("Closed — read-only").color(ui.visuals().warn_fg_color));
            }
            if variant == Variant::B && o.filter_bar {
                ui.add_space(12.0);
                self.search_box(ui, 200.0, "context header");
                let label =
                    if self.filter.dates != DateRange::All || self.filter.kind != TxnType::All {
                        "Filters • ⏷"
                    } else {
                        "Filters ⏷"
                    };
                let r = ui.menu_button(label, |ui| {
                    ui.set_min_width(180.0);
                    ui.label(RichText::new("Dates").weak());
                    for d in DateRange::ALL {
                        if ui
                            .selectable_value(&mut self.filter.dates, d, d.label())
                            .clicked()
                        {
                            self.h.event(
                                "filter",
                                json!({ "dates": d.label(), "place": "filters menu" }),
                            );
                        }
                    }
                    ui.separator();
                    ui.label(RichText::new("Type").weak());
                    for k in TxnType::ALL {
                        if ui
                            .selectable_value(&mut self.filter.kind, k, k.label())
                            .clicked()
                        {
                            self.h.event(
                                "filter",
                                json!({ "type": k.label(), "place": "filters menu" }),
                            );
                        }
                    }
                    ui.separator();
                    if ui.button("Reset").clicked() {
                        self.filter = Filter::default();
                        ui.close();
                    }
                });
                self.h.track(&r.response, "context header: Filters");
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if variant == Variant::A && o.gear {
                    let r = ui.menu_button("⚙", |ui| self.account_actions(ui, "account actions"));
                    self.h
                        .track(&r.response, "context header: account actions ⚙");
                }
                let r = ui.add_enabled(!closed, Button::new("+ Transaction"));
                self.h.track(&r, "context header: + Transaction");
                if r.clicked() {
                    self.emit("AppCommand::GoToDraftRow", "context header");
                }
                // Step 2 picked header balances for both variants.
                if o.balances {
                    for (label, value) in self.balances_text().into_iter().rev() {
                        ui.label(money(value, false).small());
                        ui.label(RichText::new(label).small().weak());
                        ui.add_space(6.0);
                    }
                }
            });
        });
        self.h.region(header.response.rect, "context header");

        // Variant A's filter bar.
        if variant == Variant::A && o.filter_bar {
            let bar = ui.horizontal(|ui| {
                self.search_box(ui, 240.0, "filter bar");
                self.filter_combos(ui, "filter bar");
                if self.filter.active() {
                    ui.label(
                        RichText::new(format!(
                            "{} of {} shown",
                            self.lines
                                .iter()
                                .filter(|l| matches!(l, Line::Txn(_)))
                                .count(),
                            self.accounts[sel].txns.len()
                        ))
                        .weak(),
                    );
                }
            });
            self.h.region(bar.response.rect, "filter bar");
        }
        ui.separator();


        self.rebuild_lines();
        let mut out = Out::default();
        let top = ui.cursor().min;
        {
            let cx = Rcx {
                accounts: &self.accounts,
                account: &self.accounts[sel],
                lines: &self.lines,
                o,
                shown: self.shown,
                theme: &self.theme,
                selected_txn: self.selected_txn,
                closed,
                variant,
            };
            register_table(ui, &mut self.h, &cx, self.scroll_to.take(), &mut out);
        }
        if out.chooser {
            // The ⚙ in the header was clicked; its menu is drawn by the table.
        }
        if let Some((menu_place, kind)) = out.menu_item.take() {
            self.emit(&format!("AppCommand::{kind}"), menu_place);
        }
        if let Some((label, shown)) = out.column.take() {
            match label {
                "Num" => self.shown.num = shown,
                "Memo" => self.shown.memo = shown,
                _ => self.shown.balance = shown,
            }
            self.h
                .event("column", json!({ "column": label, "shown": shown }));
        }
        if let Some(id) = out.clicked {
            self.selected_txn = Some(id);
        }
        if let Some(id) = out.clr {
            let a = &mut self.accounts[sel];
            if let Some(t) = a.txns.iter_mut().find(|t| t.id == id) {
                t.clr = match t.clr {
                    Clr::None => Clr::Cleared,
                    Clr::Cleared => Clr::None,
                    Clr::Reconciled => Clr::Reconciled,
                };
                let state = t.clr.text();
                self.data_version += 1;
                self.emit(
                    &format!("DomainCommand::SetCleared {{ to: {state:?} }}"),
                    "Clr cell",
                );
            }
        }
        if let Some(id) = out.flag {
            let a = &mut self.accounts[sel];
            if let Some(t) = a.txns.iter_mut().find(|t| t.id == id) {
                t.flag = !t.flag;
                let on = t.flag;
                self.data_version += 1;
                self.emit(
                    &format!("DomainCommand::SetFlag {{ on: {on} }}"),
                    "flag cell",
                );
            }
        }
        if let Some((id, pos)) = out.category {
            self.selected_txn = Some(id);
            self.cat_pick = Some(CatPick {
                account: sel,
                txn: id,
                pos,
                text: String::new(),
                fresh: true,
            });
            self.h.event("category_list_open", json!({ "txn": id }));
        }
        if let Some(pair_account) = out.goto_transfer {
            self.open_account(pair_account, "transfer jump");
        }
        self.h
            .region(Rect::from_min_max(top, ui.max_rect().max), "register");
    }

    /// The Category list under a Category cell: categories, then (toggle 3) the accounts in
    /// brackets, filtered by what's typed.
    fn category_list(&mut self, ctx: &Context) {
        let Some(mut pick) = self.cat_pick.take() else {
            return;
        };
        let mut chosen: Option<Cat> = None;
        let mut close = ctx.input(|i| i.key_pressed(Key::Escape));
        let area = egui::Area::new(Id::new("category_list"))
            .order(Order::Foreground)
            .fixed_pos(pick.pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(260.0);
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut pick.text)
                            .hint_text("Type to filter")
                            .desired_width(250.0),
                    );
                    if pick.fresh {
                        r.request_focus();
                        pick.fresh = false;
                    }
                    let needle = pick.text.to_lowercase();
                    egui::ScrollArea::vertical()
                        .max_height(320.0)
                        .show(ui, |ui| {
                            for c in CATEGORIES {
                                if !c.to_lowercase().contains(&needle) {
                                    continue;
                                }
                                let r = ui.selectable_label(false, *c);
                                self.h.track(&r, format!("category list: {c}"));
                                if r.clicked() {
                                    chosen = Some(Cat::Category(c));
                                }
                            }
                            if self.o.transfers {
                                let accounts: Vec<usize> = (0..self.accounts.len())
                                    .filter(|&i| {
                                        i != pick.account
                                            && !self.accounts[i].closed
                                            && format!("[{}]", self.accounts[i].name)
                                                .to_lowercase()
                                                .contains(&needle)
                                    })
                                    .collect();
                                if !accounts.is_empty() {
                                    ui.separator();
                                    ui.label(RichText::new("Transfer to or from").small().weak());
                                }
                                for i in accounts {
                                    let label = format!("[{}]", self.accounts[i].name);
                                    let r = ui.selectable_label(false, &label);
                                    self.h.track(&r, format!("category list: {label}"));
                                    if r.clicked() {
                                        chosen = Some(Cat::Transfer(i));
                                    }
                                }
                            }
                        });
                });
            });
        let clicked_outside = ctx.input(|i| i.pointer.any_pressed())
            && !ctx
                .pointer_interact_pos()
                .is_some_and(|p| area.response.rect.contains(p));
        if clicked_outside && !pick.fresh {
            close = true;
        }
        if let Some(cat) = chosen {
            self.h.event(
                "category_pick",
                json!({ "transfer": matches!(cat, Cat::Transfer(_)), "typed": pick.text }),
            );
            self.set_category(pick.account, pick.txn, cat);
            return;
        }
        if close {
            self.h.event("category_list_close", json!({}));
            return;
        }
        self.cat_pick = Some(pick);
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
                ui.label(RichText::new("Variant (where the chrome sits)").strong());
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
                for (&(key, label), (_, hint)) in TOGGLES.iter().zip(TOGGLE_KEYS) {
                    let mut value = *self.o.get_mut(key).unwrap();
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
                let r = ui.button("Scroll to today");
                self.h.track(&r, "dock: scroll to today");
                if r.clicked() {
                    self.scroll_to_today();
                }
                ui.separator();
                ui.label(
                    RichText::new(
                        "A / B or left / right variant · 1–9, 0 toggles · F2 note · F3 marker · F4 hide this dock · F5 spotlight · F6 prompt",
                    )
                    .small()
                    .weak(),
                );
            });
    }
}

// ---------------------------------------------------------------------------------------------
// The register table

/// What the register asks for this frame.
#[derive(Default)]
struct Out {
    clicked: Option<u32>,
    clr: Option<u32>,
    flag: Option<u32>,
    category: Option<(u32, Pos2)>,
    goto_transfer: Option<usize>,
    chooser: bool,
    menu_item: Option<(&'static str, &'static str)>,
    column: Option<(&'static str, bool)>,
}

struct Rcx<'a> {
    accounts: &'a [Account],
    account: &'a Account,
    lines: &'a [Line],
    o: Opts,
    shown: Shown,
    theme: &'a Theme,
    selected_txn: Option<u32>,
    closed: bool,
    variant: Variant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Col {
    Date,
    Num,
    Payee,
    Clr,
    Category,
    Memo,
    Amount,
    Out,
    In,
    Balance,
    Flag,
    Actions,
}

fn cat_text(accounts: &[Account], cat: Cat, transfers: bool) -> String {
    match cat {
        Cat::Opening => "[Opening Balance]".to_owned(),
        Cat::Category(c) => c.to_owned(),
        Cat::Transfer(i) if transfers => format!("[{}]", accounts[i].name),
        Cat::Transfer(_) => "Transfer".to_owned(),
    }
}

fn money(cents: i64, colour_negative: bool) -> RichText {
    let text = RichText::new(fmt_cents(cents));
    if colour_negative && cents < 0 {
        text.color(Color32::from_rgb(0xc0, 0x2b, 0x2b))
    } else {
        text
    }
}

fn register_table(
    ui: &mut Ui,
    h: &mut Harness,
    cx: &Rcx<'_>,
    scroll_to: Option<usize>,
    out: &mut Out,
) {
    let o = cx.o;
    let mut cols = vec![Col::Date];
    if cx.shown.num {
        cols.push(Col::Num);
    }
    cols.push(Col::Payee);
    if o.clr {
        cols.push(Col::Clr);
    }
    cols.push(Col::Category);
    if cx.shown.memo {
        cols.push(Col::Memo);
    }
    if o.pay_dep {
        cols.push(Col::Out);
        cols.push(Col::In);
    } else {
        cols.push(Col::Amount);
    }
    if cx.shown.balance {
        cols.push(Col::Balance);
    }
    if o.flags {
        cols.push(Col::Flag);
    }
    cols.push(Col::Actions);

    let (out_label, in_label) = cx.account.kind.out_in();
    let row_h = cx.theme.row_height();
    let shade = ui.visuals().hyperlink_color.gamma_multiply(0.10);
    let divider = ui.visuals().hyperlink_color;
    let mut table = TableBuilder::new(ui)
        .id_salt(("register", cx.account.name))
        .striped(cx.theme.alternating_rows())
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(Layout::left_to_right(Align::Center));
    for c in &cols {
        table = table.column(match c {
            Col::Date => Column::exact(78.0),
            Col::Num => Column::exact(44.0),
            Col::Payee => Column::initial(200.0).at_least(80.0),
            Col::Clr => Column::exact(22.0),
            Col::Category => Column::initial(170.0).at_least(80.0),
            Col::Memo => Column::remainder().at_least(80.0),
            Col::Amount | Col::Out | Col::In => Column::exact(88.0),
            Col::Balance => Column::exact(96.0),
            Col::Flag => Column::exact(20.0),
            Col::Actions => Column::exact(28.0),
        });
    }
    if let Some(row) = scroll_to {
        table = table.scroll_to_row(row, Some(Align::Center));
    }
    table
        .header(row_h + 2.0, |mut header| {
            for c in &cols {
                header.col(|ui| {
                    let right = |ui: &mut Ui, t: &str| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.strong(t);
                        });
                    };
                    match c {
                        Col::Date => {
                            ui.strong("Date");
                        }
                        Col::Num => {
                            ui.strong("Num");
                        }
                        Col::Payee => {
                            ui.strong("Payee");
                        }
                        Col::Clr => {
                            ui.strong("Clr").on_hover_text("Cleared status: c cleared, R reconciled");
                        }
                        Col::Category => {
                            ui.strong("Category");
                        }
                        Col::Memo => {
                            ui.strong("Memo");
                        }
                        Col::Amount => right(ui, "Amount"),
                        Col::Out => right(ui, out_label),
                        Col::In => right(ui, in_label),
                        Col::Balance => right(ui, "Balance"),
                        Col::Flag => {
                            let r = ui.allocate_response(vec2(14.0, row_h), Sense::hover());
                            paint_flag(ui.painter(), r.rect.center(), ui.visuals().weak_text_color(), false);
                        }
                        Col::Actions => {
                            // The ⚙ at the header's right end: the column chooser, and in
                            // variant B the account actions too.
                            let r = ui.menu_button("⚙", |ui| {
                                ui.set_min_width(190.0);
                                if cx.variant == Variant::B && o.gear {
                                    ui.label(RichText::new("Account").weak());
                                    for item in [
                                        "Reconcile…",
                                        "Edit account details…",
                                        "Register preferences…",
                                        "Print register…",
                                        "Export to CSV…",
                                    ] {
                                        let r = ui.button(item);
                                        h.track(&r, format!("header ⚙: {item}"));
                                        if r.clicked() {
                                            out.menu_item = Some(("header ⚙", item));
                                            ui.close();
                                        }
                                    }
                                    ui.separator();
                                }
                                ui.label(RichText::new("Columns").weak());
                                for (label, shown) in [
                                    ("Num", cx.shown.num),
                                    ("Memo", cx.shown.memo),
                                    ("Balance", cx.shown.balance),
                                ] {
                                    let mut v = shown;
                                    let r = ui.checkbox(&mut v, label);
                                    h.track(&r, format!("column chooser: {label}"));
                                    if r.changed() {
                                        out.column = Some((label, v));
                                    }
                                }
                            });
                            h.track(&r.response, "register header: ⚙");
                        }
                    }
                });
            }
        })
        .body(|body| {
            body.rows(row_h, cx.lines.len(), |mut row| {
                let line = cx.lines[row.index()];
                let Line::Txn(i) = line else {
                    // The Today divider: a rule across every cell, labelled in the Payee cell.
                    for c in &cols {
                        row.col(|ui| {
                            let rect = ui.max_rect().expand2(vec2(4.0, 0.0));
                            ui.painter().hline(
                                rect.x_range(),
                                rect.center().y,
                                Stroke::new(1.5, divider),
                            );
                            if *c == Col::Payee {
                                let text = format!("Today · {} · scheduled below", fmt_day(TODAY));
                                let galley = ui.painter().layout_no_wrap(
                                    text,
                                    egui::FontId::proportional(11.0),
                                    divider,
                                );
                                let bg = ui.visuals().panel_fill;
                                let p = pos2(rect.left() + 4.0, rect.center().y - galley.size().y / 2.0);
                                ui.painter().rect_filled(
                                    Rect::from_min_size(p, galley.size()).expand(2.0),
                                    0.0,
                                    bg,
                                );
                                ui.painter().galley(p, galley, divider);
                            }
                        });
                    }
                    return;
                };
                let t = &cx.account.txns[i];
                let balance = cx.account.balances[i];
                let future = t.day > TODAY;
                row.set_selected(cx.selected_txn == Some(t.id));
                for c in &cols {
                    let (_, resp) = row.col(|ui| {
                        if future && o.future_shade {
                            ui.painter().rect_filled(
                                ui.max_rect().expand2(vec2(4.0, 0.0)),
                                0.0,
                                shade,
                            );
                        }
                        let right = |ui: &mut Ui, text: RichText| {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(text);
                            });
                        };
                        match c {
                            Col::Date => {
                                ui.label(fmt_day(t.day));
                            }
                            Col::Num => {
                                ui.label(&t.num);
                            }
                            Col::Payee => {
                                ui.label(&t.payee);
                            }
                            Col::Clr => {
                                ui.with_layout(
                                    Layout::centered_and_justified(egui::Direction::LeftToRight),
                                    |ui| {
                                        let text = RichText::new(t.clr.text());
                                        let text = if t.clr == Clr::Reconciled {
                                            text.weak()
                                        } else {
                                            text.strong()
                                        };
                                        let r = ui.add(
                                            egui::Label::new(text).sense(Sense::click()),
                                        );
                                        h.track(&r, format!("clr cell, txn {}", t.id));
                                        if r.clicked() && !cx.closed {
                                            out.clr = Some(t.id);
                                        }
                                    },
                                );
                            }
                            Col::Category => {
                                let text = cat_text(cx.accounts, t.cat, o.transfers);
                                let r = ui.add(
                                    egui::Label::new(text).truncate().sense(Sense::click()),
                                );
                                h.track(&r, format!("category cell, txn {}", t.id));
                                if r.clicked() && !cx.closed && t.cat != Cat::Opening {
                                    if cx.selected_txn == Some(t.id) {
                                        out.category = Some((t.id, r.rect.left_bottom()));
                                    } else {
                                        out.clicked = Some(t.id);
                                    }
                                }
                                if let Cat::Transfer(other) = t.cat
                                    && o.transfers
                                    && r.double_clicked()
                                {
                                    out.goto_transfer = Some(other);
                                }
                                if let Cat::Transfer(_) = t.cat
                                    && o.transfers
                                {
                                    r.on_hover_text("Click the selected row's category to change it; double-click to go to the other account");
                                }
                            }
                            Col::Memo => {
                                ui.add(egui::Label::new(RichText::new(&t.memo).weak()).truncate());
                            }
                            Col::Amount => right(ui, money(t.amount, true)),
                            Col::Out => {
                                if t.amount < 0 {
                                    right(ui, money(-t.amount, false));
                                }
                            }
                            Col::In => {
                                if t.amount > 0 {
                                    right(ui, money(t.amount, false));
                                }
                            }
                            Col::Balance => right(ui, money(balance, true)),
                            Col::Flag => {
                                let r = ui.allocate_response(vec2(14.0, row_h), Sense::click());
                                if t.flag {
                                    paint_flag(ui.painter(), r.rect.center(), Color32::from_rgb(0xd9, 0x6b, 0x00), true);
                                } else if r.hovered() {
                                    paint_flag(ui.painter(), r.rect.center(), ui.visuals().weak_text_color(), false);
                                }
                                h.track(&r, format!("flag cell, txn {}", t.id));
                                if r.clicked() {
                                    out.flag = Some(t.id);
                                }
                            }
                            Col::Actions => {
                                if cx.selected_txn == Some(t.id) {
                                    let r = ui.menu_button("…", |ui| {
                                        for item in ["Split", "Void", "Delete"] {
                                            let r = ui.button(item);
                                            h.track(&r, format!("row menu: {item}"));
                                            if r.clicked() {
                                                out.menu_item = Some(("row menu", item));
                                                ui.close();
                                            }
                                        }
                                    });
                                    h.track(&r.response, format!("actions cell, txn {}", t.id));
                                }
                            }
                        }
                    });
                    let _ = resp;
                }
                if row.response().clicked() && out.clicked.is_none() && out.category.is_none() {
                    out.clicked = Some(t.id);
                }
            });
        });
}

// ---------------------------------------------------------------------------------------------
// Painted glyphs the default fonts lack

#[derive(Clone, Copy)]
enum Dir {
    Right,
    Down,
}

fn paint_triangle(painter: &egui::Painter, c: Pos2, dir: Dir, color: Color32) {
    let s = 3.5;
    let points = match dir {
        Dir::Right => vec![
            pos2(c.x - s * 0.6, c.y - s),
            pos2(c.x + s * 0.8, c.y),
            pos2(c.x - s * 0.6, c.y + s),
        ],
        Dir::Down => vec![
            pos2(c.x - s, c.y - s * 0.6),
            pos2(c.x + s, c.y - s * 0.6),
            pos2(c.x, c.y + s * 0.8),
        ],
    };
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
}

/// A small flag: a pole and a pennant, filled when set.
fn paint_flag(painter: &egui::Painter, c: Pos2, color: Color32, filled: bool) {
    let pole_x = c.x - 4.0;
    painter.line_segment(
        [pos2(pole_x, c.y - 6.0), pos2(pole_x, c.y + 6.0)],
        Stroke::new(1.2, color),
    );
    let pennant = vec![
        pos2(pole_x, c.y - 6.0),
        pos2(c.x + 5.0, c.y - 3.0),
        pos2(pole_x, c.y),
    ];
    if filled {
        painter.add(Shape::convex_polygon(pennant, color, Stroke::NONE));
    } else {
        painter.add(Shape::closed_line(pennant, Stroke::new(1.0, color)));
    }
}

fn main() -> eframe::Result {
    // Prints the stub figures and exits, so they can be checked without a window.
    if std::env::var_os("REGISTER_PROTO_DUMP").is_some() {
        for a in generate() {
            let min = a.balances.iter().min().copied().unwrap_or(0);
            let max = a.balances.iter().max().copied().unwrap_or(0);
            let future = a.txns.iter().filter(|t| t.day > TODAY).count();
            println!(
                "{:<24} rows {:>5} future {:>3} current {:>12} ending {:>12} cleared {:>12} min {:>12} max {:>12}",
                a.name,
                a.txns.len(),
                future,
                fmt_cents(a.current()),
                fmt_cents(a.ending()),
                fmt_cents(a.cleared()),
                fmt_cents(min),
                fmt_cents(max),
            );
        }
        return Ok(());
    }
    let args = Args::from_env();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Balanced — PROTOTYPE register display and chrome")
            .with_inner_size([1500.0, 900.0]),
        ..Default::default()
    };
    eframe::run_native(
        "balanced_register_display_prototype",
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
