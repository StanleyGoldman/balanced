//! PROTOTYPE, throwaway: three interaction models for the Register grid.
//!
//! Question: how should the Register grid behave, and can egui do it well? Editable transaction
//! grid on stub data: thousands of rows, smooth scrolling, keyboard entry (Tab between fields,
//! Enter to save, Esc to cancel), payee autocomplete, sorting, running balance, a split marker.
//!
//! Run:
//!   cargo run -p balanced_ui --example register_prototype -- \
//!     [--variant a|b|c] [--log <dir>/events.jsonl] [--control <dir>/control.txt] [--stats]
//!
//! The PROTOTYPE dock on the right holds the variant switcher and toggles; F4 collapses it.
//! Keys (no text field focused): A B C / ← → switch variant, 1–9 0 flip toggles, F2 note,
//! F3 marker, F5 spotlight. In variant C the grid owns the keyboard, so hold Alt for those keys.
//!
//! Fixed-seed stub data (money is whole cents in an `i64`), English literals. The look is
//! Balanced's own theme, `balanced_ui::theme`, pulled in by path because it is crate-private;
//! the toggles for gridlines, alternating rows, density and dark set its `ThemeSettings`.
//! None of this is production code.

#![allow(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::pedantic,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::wildcard_enum_match_arm,
    clippy::unwrap_used,
    clippy::str_to_string,
    clippy::let_underscore_must_use,
    clippy::iter_over_hash_type,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::needless_pass_by_ref_mut,
    clippy::needless_range_loop,
    clippy::disallowed_methods,
    missing_debug_implementations,
    unreachable_pub
)]

use balanced_prototype_harness::{Args, Harness};
use eframe::egui;
use egui::{
    Align, CentralPanel, Id, Key, Label, Layout, Margin, Modifiers, Panel, Rect, RichText,
    ScrollArea, Sense, Sides, Ui,
};
use egui_extras::{Column, TableBuilder};
use serde_json::json;

// The real theme module: crate-private, so the example compiles its source in.
#[path = "../../src/theme.rs"]
#[allow(dead_code)]
mod theme;
use theme::{Density, Gridlines, Scheme, Theme, ThemeSettings};

// ---------------------------------------------------------------------------------------------
// Stub data: fixed seed, realistic volume

const SEED: u64 = 0x00ba_1a9c_ed17;
const TXN_COUNT: usize = 20_000;
/// Days since 1970-01-01 of 2016-01-01; the stub history runs about ten years from there.
const FIRST_DAY: i64 = 16_801;
const HISTORY_DAYS: i64 = 3_900;
/// New transactions are dated here.
const TODAY: i64 = FIRST_DAY + HISTORY_DAYS + 1;

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

    /// Roughly `typical` cents, between 40% and 160%.
    fn around(&mut self, typical: i64) -> i64 {
        let spread = (typical * 6 / 5).max(1) as u64;
        (typical * 2 / 5 + self.below(spread) as i64).max(1)
    }
}

/// Category 0 is the reserved marker of a split transaction.
const SPLIT_CATEGORY: u16 = 0;

const CATEGORY_NAMES: &[&str] = &[
    "Split",
    "Uncategorized",
    "Food:Groceries",
    "Food:Dining Out",
    "Food:Coffee",
    "Food:Takeout",
    "Food:Alcohol",
    "Auto:Fuel",
    "Auto:Maintenance",
    "Auto:Insurance",
    "Auto:Parking",
    "Auto:Registration",
    "Home:Rent",
    "Home:Electric",
    "Home:Water",
    "Home:Gas",
    "Home:Internet",
    "Home:Phone",
    "Home:Repairs",
    "Home:Furnishings",
    "Home:Garden",
    "Health:Doctor",
    "Health:Pharmacy",
    "Health:Dental",
    "Health:Vision",
    "Health:Gym",
    "Fun:Streaming",
    "Fun:Movies",
    "Fun:Hobbies",
    "Fun:Books",
    "Fun:Games",
    "Fun:Concerts",
    "Travel:Flights",
    "Travel:Hotels",
    "Travel:Rideshare",
    "Travel:Car Rental",
    "Shopping:Clothing",
    "Shopping:Electronics",
    "Shopping:Gifts",
    "Shopping:Household",
    "Shopping:Personal Care",
    "Shopping:Pets",
    "Shopping:Office",
    "Kids:Activities",
    "Kids:School",
    "Kids:Clothing",
    "Finance:Bank Fees",
    "Finance:Taxes",
    "Finance:Charity",
    "Finance:Insurance",
    "Subscriptions:Software",
    "Subscriptions:News",
    "Subscriptions:Cloud",
    "Income:Salary",
    "Income:Interest",
    "Income:Refunds",
    "Income:Reimbursement",
    "Transfer",
];

/// (name, category, typical dollars, store count)
const CHAINS: &[(&str, &str, i64, u32)] = &[
    ("Safeway", "Food:Groceries", 85, 4),
    ("Costco", "Food:Groceries", 190, 3),
    ("Whole Foods", "Food:Groceries", 70, 4),
    ("Trader Joe's", "Food:Groceries", 55, 4),
    ("Kroger", "Food:Groceries", 78, 3),
    ("Starbucks", "Food:Coffee", 7, 6),
    ("Peet's Coffee", "Food:Coffee", 6, 3),
    ("Chipotle", "Food:Takeout", 14, 4),
    ("Panera Bread", "Food:Dining Out", 18, 3),
    ("Olive Garden", "Food:Dining Out", 62, 3),
    ("Shell", "Auto:Fuel", 48, 5),
    ("Chevron", "Auto:Fuel", 52, 5),
    ("Valvoline", "Auto:Maintenance", 90, 2),
    ("Geico", "Auto:Insurance", 140, 1),
    ("Home Depot", "Home:Repairs", 64, 3),
    ("Lowe's", "Home:Repairs", 58, 3),
    ("IKEA", "Home:Furnishings", 120, 2),
    ("PG&E", "Home:Electric", 110, 1),
    ("Comcast", "Home:Internet", 80, 1),
    ("Verizon", "Home:Phone", 95, 1),
    ("CVS", "Health:Pharmacy", 24, 4),
    ("Walgreens", "Health:Pharmacy", 21, 4),
    ("Netflix", "Fun:Streaming", 16, 1),
    ("Spotify", "Fun:Streaming", 11, 1),
    ("AMC Theatres", "Fun:Movies", 32, 2),
    ("Barnes & Noble", "Fun:Books", 28, 2),
    ("Delta Air Lines", "Travel:Flights", 340, 1),
    ("United Airlines", "Travel:Flights", 380, 1),
    ("Marriott", "Travel:Hotels", 210, 3),
    ("Uber", "Travel:Rideshare", 19, 1),
    ("Lyft", "Travel:Rideshare", 17, 1),
    ("Amazon", "Shopping:Household", 45, 1),
    ("Target", "Shopping:Household", 66, 4),
    ("Walmart", "Shopping:Household", 72, 4),
    ("Best Buy", "Shopping:Electronics", 150, 2),
    ("Apple", "Shopping:Electronics", 120, 2),
    ("Nordstrom", "Shopping:Clothing", 95, 2),
    ("Petco", "Shopping:Pets", 42, 2),
    ("Dropbox", "Subscriptions:Cloud", 12, 1),
    ("Adobe", "Subscriptions:Software", 55, 1),
];

const OWNERS: &[&str] = &[
    "Maria's",
    "Luigi's",
    "Chen's",
    "Patel's",
    "O'Brien's",
    "Kowalski's",
    "Nakamura's",
    "Dubois",
    "Santos",
    "Garcia's",
    "Schmidt's",
    "Ivanov's",
    "Haddad's",
    "Kim's",
    "Rossi's",
    "Murphy's",
    "Tanaka",
    "Silva's",
    "Cohen's",
    "Andersen",
    "Lopez",
    "Baker Street",
    "Elm Street",
    "Harbor",
    "Summit",
    "Riverside",
    "Maple",
    "Oak Hill",
];

const LOCAL_KINDS: &[(&str, &str, i64)] = &[
    ("Bakery", "Food:Dining Out", 16),
    ("Hardware", "Home:Repairs", 38),
    ("Clinic", "Health:Doctor", 85),
    ("Salon", "Shopping:Personal Care", 60),
    ("Garage", "Auto:Maintenance", 210),
];

const MEMOS: &[&str] = &[
    "weekly shop",
    "birthday",
    "reimbursable",
    "trip",
    "monthly",
    "gift",
    "repair",
    "annual fee",
    "split with Sam",
    "returned item",
    "deposit",
    "party",
    "work lunch",
    "school",
    "refill",
];

const INCOME: &[(&str, &str)] = &[
    ("Payroll - Acme Corp", "Income:Salary"),
    ("Payroll - Acme Corp bonus", "Income:Salary"),
    ("Interest Earned", "Income:Interest"),
    ("Tax Refund", "Income:Refunds"),
    ("Venmo cashout", "Income:Reimbursement"),
    ("Employer expense report", "Income:Reimbursement"),
];

#[derive(Clone)]
struct Txn {
    day: i64,
    payee: u16,
    category: u16,
    memo: String,
    /// Cents; negative is a payment.
    amount: i64,
    /// Present on split transactions: (category, cents) summing to `amount`.
    split: Option<Vec<(u16, i64)>>,
    cleared: bool,
    /// Balance after this transaction in date order.
    balance: i64,
}

struct Data {
    payees: Vec<String>,
    payees_lc: Vec<String>,
    /// The category a payee usually lands in.
    payee_category: Vec<u16>,
    categories: Vec<String>,
    txns: Vec<Txn>,
    opening: i64,
}

impl Data {
    fn category_index(&self, name: &str) -> Option<u16> {
        self.categories
            .iter()
            .position(|c| c.eq_ignore_ascii_case(name.trim()))
            .map(|i| i as u16)
    }

    fn payee_index(&self, name: &str) -> Option<u16> {
        self.payees
            .iter()
            .position(|p| p.eq_ignore_ascii_case(name.trim()))
            .map(|i| i as u16)
    }

    fn recompute_balances(&mut self) {
        let mut running = self.opening;
        for t in &mut self.txns {
            running += t.amount;
            t.balance = running;
        }
    }
}

fn generate() -> Data {
    let mut rng = Lcg(SEED);
    let categories: Vec<String> = CATEGORY_NAMES.iter().map(|s| (*s).to_owned()).collect();
    let cat = |name: &str| CATEGORY_NAMES.iter().position(|c| *c == name).unwrap_or(1) as u16;

    let mut payees: Vec<String> = Vec::new();
    let mut payee_category: Vec<u16> = Vec::new();
    let mut typical: Vec<i64> = Vec::new();
    for &(name, category, dollars, stores) in CHAINS {
        for s in 0..stores {
            let label = if stores == 1 {
                name.to_owned()
            } else {
                format!("{name} #{}", 100 + s * 37 + (name.len() as u32 * 11) % 90)
            };
            payees.push(label);
            payee_category.push(cat(category));
            typical.push(dollars * 100);
        }
    }
    for owner in OWNERS {
        for (kind, category, dollars) in LOCAL_KINDS {
            if rng.below(100) < 80 {
                payees.push(format!("{owner} {kind}"));
                payee_category.push(cat(category));
                typical.push(dollars * 100);
            }
        }
    }
    let expense_payees = payees.len();
    for (name, category) in INCOME {
        payees.push((*name).to_owned());
        payee_category.push(cat(category));
        typical.push(0);
    }

    let mut days: Vec<i64> = (0..TXN_COUNT)
        .map(|_| FIRST_DAY + rng.below(HISTORY_DAYS as u64) as i64)
        .collect();
    days.sort_unstable();

    let mut txns: Vec<Txn> = Vec::with_capacity(TXN_COUNT);
    let mut expenses_total = 0_i64;
    let mut income_slots: Vec<usize> = Vec::new();
    for (i, day) in days.into_iter().enumerate() {
        let income = rng.below(100) < 7;
        if income {
            let which = expense_payees + rng.below(INCOME.len() as u64) as usize;
            income_slots.push(i);
            txns.push(Txn {
                day,
                payee: which as u16,
                category: payee_category[which],
                memo: String::new(),
                amount: 0,
                split: None,
                cleared: true,
                balance: 0,
            });
            continue;
        }
        // Squaring the draw makes the early payees (the chains) far more frequent.
        let n = expense_payees as u64;
        let which = (rng.below(n) * rng.below(n) / n) as usize;
        let amount = -rng.around(typical[which]);
        expenses_total += -amount;
        let memo = if rng.below(100) < 25 {
            MEMOS[rng.below(MEMOS.len() as u64) as usize].to_owned()
        } else {
            String::new()
        };
        let mut category = payee_category[which];
        if rng.below(100) < 8 {
            category = 2 + rng.below((CATEGORY_NAMES.len() - 6) as u64) as u16;
        }
        let split = if -amount >= 4_000 && rng.below(100) < 6 {
            let parts = 2 + rng.below(3) as usize;
            let mut lines = Vec::new();
            let mut left = amount;
            for p in 0..parts {
                let c = 2 + rng.below((CATEGORY_NAMES.len() - 6) as u64) as u16;
                let portion = if p + 1 == parts {
                    left
                } else {
                    -(rng.below((-left / 2).max(2) as u64) as i64).max(1)
                };
                left -= portion;
                lines.push((c, portion));
            }
            category = SPLIT_CATEGORY;
            Some(lines)
        } else {
            None
        };
        txns.push(Txn {
            day,
            payee: which as u16,
            category,
            memo,
            amount,
            split,
            cleared: true,
            balance: 0,
        });
    }
    // Income balances the books with a small surplus, so balances drift up from the opening
    // figure instead of running away in either direction.
    let weights: Vec<u64> = income_slots.iter().map(|_| 70 + rng.below(60)).collect();
    let weight_sum: u64 = weights.iter().sum::<u64>().max(1);
    let target = expenses_total + expenses_total / 25;
    for (slot, w) in income_slots.iter().zip(&weights) {
        txns[*slot].amount = target * (*w as i64) / weight_sum as i64;
    }
    let last = txns.len();
    for t in txns.iter_mut().skip(last.saturating_sub(40)) {
        t.cleared = false;
    }

    let mut running = 0_i64;
    let mut lowest = 0_i64;
    for t in &txns {
        running += t.amount;
        lowest = lowest.min(running);
    }
    let opening = 100_000 + (-lowest).max(0);
    let mut data = Data {
        payees_lc: payees.iter().map(|p| p.to_lowercase()).collect(),
        payees,
        payee_category,
        categories,
        txns,
        opening,
    };
    data.recompute_balances();
    data
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

/// The form an amount field edits: signed, no grouping.
fn fmt_plain(cents: i64) -> String {
    format!(
        "{}{}.{:02}",
        if cents < 0 { "-" } else { "" },
        cents.unsigned_abs() / 100,
        cents.unsigned_abs() % 100
    )
}

fn parse_amount(text: &str) -> Option<i64> {
    let cleaned = text.trim().replace([',', '$'], "");
    let (neg, rest) = match cleaned.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, cleaned.as_str()),
    };
    let (whole, frac) = rest.split_once('.').unwrap_or((rest, ""));
    if (whole.is_empty() && frac.is_empty()) || frac.len() > 2 {
        return None;
    }
    let whole: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let frac: i64 = if frac.is_empty() {
        0
    } else {
        format!("{frac:0<2}").parse().ok()?
    };
    let cents = whole.checked_mul(100)?.checked_add(frac)?;
    Some(if neg { -cents } else { cents })
}

/// Howard Hinnant's `civil_from_days`.
fn fmt_day(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y}-{m:02}-{d:02}")
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

/// (year, month, day) of a day number.
fn civil(days: i64) -> (i64, i64, i64) {
    let t = fmt_day(days);
    let mut it = t.split('-').map(|p| p.parse::<i64>().unwrap_or(1));
    (
        it.next().unwrap_or(1970),
        it.next().unwrap_or(1),
        it.next().unwrap_or(1),
    )
}

fn days_in_month(y: i64, m: i64) -> i64 {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    days_from_civil(ny, nm, 1) - days_from_civil(y, m, 1)
}

/// `2026-10-05`, `10/5/2026`, `10/5` (this year) or `t` (today); `None` if it is not a date.
fn parse_date(text: &str) -> Option<i64> {
    let t = text.trim();
    if t.eq_ignore_ascii_case("t") {
        return Some(TODAY);
    }
    let (y, m, d) = if let Some((y, rest)) = t.split_once('-') {
        let (m, d) = rest.split_once('-')?;
        (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?)
    } else {
        let parts: Vec<&str> = t.split('/').collect();
        let year = civil(TODAY).0;
        match parts.as_slice() {
            [m, d] => (year, m.parse().ok()?, d.parse().ok()?),
            [m, d, y] => (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?),
            _ => return None,
        }
    };
    ((1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m)).then(|| days_from_civil(y, m, d))
}

// ---------------------------------------------------------------------------------------------
// Variants, toggles, sorting

#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    A,
    B,
    C,
}

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
            Self::A => "A · Classic rows, edited in place",
            Self::B => "B · Read-only rows + entry bar",
            Self::C => "C · Spreadsheet cells",
        }
    }

    fn step(self, delta: isize) -> Self {
        let n = Self::ALL.len() as isize;
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0) as isize;
        Self::ALL[(i + delta).rem_euclid(n) as usize]
    }
}

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

const VARIANT_KEYS: [(Key, &str); 3] = [(Key::A, "A"), (Key::B, "B"), (Key::C, "C")];

/// (key, label, applies to)
const TOGGLES: &[(&str, &str, &str)] = &[
    (
        "scroll_animated",
        "Key scrolling: egui's, animated a frame late (off: same frame)",
        "C",
    ),
    ("running_balance", "Running balance column", "all"),
    (
        "ac_popup",
        "Autocomplete: list (off: inline ghost text)",
        "all",
    ),
    ("gridlines", "Gridlines (cycles)", "all"),
    ("striping", "Alternating rows", "all"),
    ("dense", "Dense rows", "all"),
    ("entry_bar_top", "Entry bar on top (off: bottom)", "B"),
    ("enter_down", "Enter moves down (off: right)", "C"),
    (
        "balance_follows_sort",
        "Balance follows a non-date sort",
        "all",
    ),
    ("dark", "Dark theme", "all"),
];

struct Toggles {
    /// Variant A's two-line rows; fixed off since S1 ("the two line rows look terrible").
    two_line: bool,
    scroll_animated: bool,
    running_balance: bool,
    ac_popup: bool,
    gridlines: Gridlines,
    striping: bool,
    dense: bool,
    entry_bar_top: bool,
    enter_down: bool,
    balance_follows_sort: bool,
    dark: bool,
}

impl Toggles {
    fn get_mut(&mut self, key: &str) -> Option<&mut bool> {
        Some(match key {
            "scroll_animated" => &mut self.scroll_animated,
            "running_balance" => &mut self.running_balance,
            "ac_popup" => &mut self.ac_popup,
            "striping" => &mut self.striping,
            "dense" => &mut self.dense,
            "entry_bar_top" => &mut self.entry_bar_top,
            "enter_down" => &mut self.enter_down,
            "balance_follows_sort" => &mut self.balance_follows_sort,
            "dark" => &mut self.dark,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortCol {
    Date,
    Payee,
    Category,
    Amount,
}

impl SortCol {
    fn name(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Payee => "payee",
            Self::Category => "category",
            Self::Amount => "amount",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        [Self::Date, Self::Payee, Self::Category, Self::Amount]
            .into_iter()
            .find(|c| c.name() == name)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Sort {
    col: SortCol,
    asc: bool,
}

// ---------------------------------------------------------------------------------------------
// Selection and edit state

/// A Register row: a transaction (by id, its index in `Data::txns`) or the blank new-entry row.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sel {
    None,
    Txn(usize),
    New,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    Payee,
    Amount,
    Category,
    Memo,
    Cell,
}

impl Field {
    /// The column order (S2: "tab focus order should go by order of columns").
    const TAB_ORDER: [Self; 4] = [Self::Payee, Self::Category, Self::Memo, Self::Amount];

    fn name(self) -> &'static str {
        match self {
            Self::Payee => "payee",
            Self::Amount => "amount",
            Self::Category => "category",
            Self::Memo => "memo",
            Self::Cell => "cell",
        }
    }

    fn id(self) -> Id {
        Id::new(("register_field", self.name()))
    }

    fn step(self, delta: isize, skip_category: bool) -> Self {
        let mut i = Self::TAB_ORDER.iter().position(|f| *f == self).unwrap_or(0) as isize;
        loop {
            i = (i + delta).rem_euclid(4);
            let f = Self::TAB_ORDER[i as usize];
            if !(skip_category && f == Self::Category) {
                return f;
            }
        }
    }
}

/// The form behind variant A's edited row, variant B's entry bar and variant C's new-entry row.
#[derive(Clone, Default)]
struct Edit {
    /// `None` while entering a new transaction.
    id: Option<usize>,
    /// Variant C's draft row: the date typed or picked; `None` is today.
    day: Option<i64>,
    payee: String,
    amount: String,
    category: String,
    memo: String,
    /// A field that should take focus when it is next drawn.
    focus: Option<Field>,
    dirty: bool,
}

impl Edit {
    fn blank() -> Self {
        Self {
            focus: Some(Field::Payee),
            ..Self::default()
        }
    }

    fn of(data: &Data, id: usize) -> Self {
        let t = &data.txns[id];
        Self {
            id: Some(id),
            day: Some(t.day),
            payee: data.payees[t.payee as usize].clone(),
            amount: fmt_plain(t.amount),
            category: if t.split.is_some() {
                String::new()
            } else {
                data.categories[t.category as usize].clone()
            },
            memo: t.memo.clone(),
            focus: Some(Field::Payee),
            dirty: false,
        }
    }

    fn text_mut(&mut self, f: Field) -> &mut String {
        match f {
            Field::Payee | Field::Cell => &mut self.payee,
            Field::Amount => &mut self.amount,
            Field::Category => &mut self.category,
            Field::Memo => &mut self.memo,
        }
    }
}

/// Variant C: an in-cell edit.
struct CellEdit {
    row: Sel,
    col: usize,
    buf: String,
    focus: bool,
}

/// Keys taken from the frame before any widget sees them, while a prototype field has focus.
#[derive(Clone, Copy, Default)]
struct Keys {
    enter: bool,
    /// Ctrl+Enter: add the draft row's transaction from any cell.
    ctrl_enter: bool,
    esc: bool,
    tab: bool,
    back_tab: bool,
    down: bool,
    up: bool,
    /// Alt+Down: open a combobox's full list, or the date picker.
    alt_down: bool,
    /// Which field has focus.
    field: Option<Field>,
}

#[derive(Default)]
struct Ac {
    sel: Option<usize>,
    open: bool,
    last: String,
    /// Enter picked a suggestion this frame, so it must not also save.
    enter_used: bool,
    /// The full list is open (dropdown button or Alt+Down), not just the matches while typing.
    all: bool,
    /// Where the list was drawn last frame, so a press on it still counts after the field
    /// loses focus to that press.
    rect: Option<Rect>,
    /// A suggestion was clicked: variant C commits the cell and moves on, as Tab does.
    clicked: bool,
}

/// Matching names, prefix matches first, then substring matches; names before `skip` are never
/// offered (category 0 is the split marker). Up to seven while typing; with `all` (the dropdown
/// button or Alt+Down), every match, and every name when nothing is typed.
fn suggestions(names: &[String], skip: usize, query: &str, all: bool) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    let limit = if all { usize::MAX } else { 7 };
    if q.is_empty() {
        return if all {
            (skip..names.len()).collect()
        } else {
            Vec::new()
        };
    }
    let lc: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
    let prefix = (skip..names.len()).filter(|i| lc[*i].starts_with(&q));
    let inner = (skip..names.len()).filter(|i| !lc[*i].starts_with(&q) && lc[*i].contains(&q));
    prefix.chain(inner).take(limit).collect()
}

// ---------------------------------------------------------------------------------------------
// Widgets

fn gridlines_name(g: Gridlines) -> &'static str {
    match g {
        Gridlines::Off => "off",
        Gridlines::Horizontal => "horizontal",
        Gridlines::Vertical => "vertical",
        Gridlines::Both => "both",
    }
}

/// The grid being drawn this frame: its theme, its scroll viewport (painters that reach into
/// the gaps between cells stop there), and its left and right edges, so a cell can tell it
/// is in the first or last column and take the edge inset.
struct Grid {
    theme: Theme,
    view: Rect,
    left: f32,
    /// The right edge of the last column, as the previous frame drew it.
    right: f32,
    seen_right: f32,
}

thread_local! {
    static GRID: std::cell::RefCell<Option<Grid>> = const { std::cell::RefCell::new(None) };
}

/// Start a grid: call it in the `Ui` the table is built in.
fn set_grid(ui: &Ui, theme: &Theme) {
    GRID.with_borrow_mut(|g| {
        let right = g
            .as_ref()
            .map(|g| g.seen_right)
            .filter(|r| r.is_finite())
            .unwrap_or(ui.max_rect().right());
        *g = Some(Grid {
            theme: theme.clone(),
            view: ui.clip_rect(),
            left: ui.max_rect().left(),
            right,
            seen_right: f32::NEG_INFINITY,
        });
    });
}

fn first_col(ui: &Ui) -> bool {
    GRID.with_borrow(|g| {
        g.as_ref()
            .is_some_and(|g| ui.max_rect().left() <= g.left + 0.5)
    })
}

fn last_col(ui: &Ui) -> bool {
    GRID.with_borrow(|g| {
        g.as_ref()
            .is_some_and(|g| ui.max_rect().right() >= g.right - 0.5)
    })
}

/// The cell's gridlines, from the theme: call it after the cell's content.
fn grid_lines(ui: &Ui) {
    GRID.with_borrow_mut(|g| {
        let Some(g) = g else { return };
        let cell = ui.max_rect();
        g.seen_right = g.seen_right.max(cell.right());
        let column = usize::from(cell.left() > g.left + 0.5);
        let ctx = ui.ctx();
        ctx.layer_painter(ui.layer_id())
            .with_clip_rect(g.theme.rule_clip(ui.clip_rect(), g.view))
            .extend(g.theme.cell_rules(cell, column, ctx.pixels_per_point()));
    });
}

/// The header band behind a header cell and half the gap around it: call it first.
fn header_band(ui: &Ui) {
    GRID.with_borrow_mut(|g| {
        let Some(g) = g else { return };
        let cell = ui.max_rect();
        g.seen_right = g.seen_right.max(cell.right());
        let bg = cell.expand2(ui.spacing().item_spacing / 2.0);
        ui.ctx()
            .layer_painter(ui.layer_id())
            .with_clip_rect(bg.intersect(g.view))
            .rect_filled(bg, 0.0, g.theme.header_fill());
    });
}

/// A right-aligned header (amounts), with the edge inset in the last column.
fn money_header(ui: &mut Ui, title: &str) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if last_col(ui) {
            ui.add_space(Theme::EDGE_INSET);
        }
        ui.strong(title);
    });
}

fn cell_label(ui: &mut Ui, text: impl Into<egui::WidgetText>) {
    if first_col(ui) {
        ui.add_space(Theme::EDGE_INSET);
    }
    ui.add(Label::new(text).truncate());
}

/// An amount, in a right-to-left cell.
fn money_label(ui: &mut Ui, cents: i64) {
    if last_col(ui) {
        ui.add_space(Theme::EDGE_INSET);
    }
    let text = fmt_cents(cents);
    if cents < 0 {
        let c = ui.visuals().error_fg_color;
        ui.add(Label::new(RichText::new(text).color(c)).truncate());
    } else {
        ui.add(Label::new(text).truncate());
    }
}

/// A single-line text field that takes part in the prototype's own Tab/Enter/Esc handling.
fn field(
    ui: &mut Ui,
    h: &mut Harness,
    f: Field,
    text: &mut String,
    focus: &mut Option<Field>,
    name: String,
    right: bool,
) -> egui::Response {
    let mut edit = egui::TextEdit::singleline(text)
        .id(f.id())
        // Keep Esc, Tab and arrows: without `escape`, egui drops the focus on Esc before the
        // prototype sees the key, and the edit never cancels.
        .event_filter(egui::EventFilter {
            tab: true,
            horizontal_arrows: true,
            vertical_arrows: true,
            escape: true,
        })
        .margin(egui::Margin::symmetric(4, 1))
        .desired_width(f32::INFINITY);
    if right {
        edit = edit.horizontal_align(Align::Max);
    }
    let mut out = edit.show(ui);
    let r = out.response.response.clone();
    if *focus == Some(f) {
        r.request_focus();
        *focus = None;
        // Start with the cursor after the text, not before it.
        let end = egui::text::CCursor::new(text.chars().count());
        out.state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(end)));
        out.state.store(ui.ctx(), r.id);
    }
    h.track(&r, name);
    r
}

/// A combobox: a text field whose suggestions come from `names` (payees or categories).
/// Typing filters; the top match is highlighted, ↓/↑ move the highlight, Enter or Tab take it,
/// and a click takes the clicked one. ↑ past the top clears the highlight, so Enter keeps what
/// was typed. With `button`, a dropdown button beside the field opens the full list, as Alt+Down
/// does. `active` keeps the list up while the edit is open even if the field lost focus.
/// Returns the response, and the name picked this frame (an index into `names`).
fn combo_edit(
    ui: &mut Ui,
    h: &mut Harness,
    names: &[String],
    skip: usize,
    kind: &str,
    ac: &mut Ac,
    popup_style: bool,
    keys: &Keys,
    text: &mut String,
    focus: &mut Option<Field>,
    name: String,
    id: Field,
    active: bool,
    button: bool,
) -> (egui::Response, Option<usize>) {
    let r = if button {
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 22.0).max(40.0);
            let r = ui
                .scope(|ui| {
                    ui.set_max_width(w);
                    field(ui, h, id, text, focus, name.clone(), false)
                })
                .inner;
            let b = ui.small_button("⏷");
            h.track(&b, format!("{kind} dropdown button"));
            if b.clicked() {
                ac.all = !ac.all;
                *focus = Some(id);
                h.event(
                    "combo_list",
                    json!({ "kind": kind, "open": ac.all, "via": "button" }),
                );
            }
            r
        })
        .inner
    } else {
        field(ui, h, id, text, focus, name.clone(), false)
    };
    if keys.alt_down && keys.field.is_some() {
        ac.all = true;
        h.event(
            "combo_list",
            json!({ "kind": kind, "open": true, "via": "alt+down" }),
        );
    }
    let hovering = ac
        .rect
        .zip(ui.ctx().pointer_hover_pos())
        .is_some_and(|(rect, p)| rect.contains(p));
    let shown = active || r.has_focus() || hovering;
    let sugg = if shown {
        suggestions(names, skip, text, ac.all)
    } else {
        Vec::new()
    };
    let key = sugg
        .first()
        .map(|i| format!("{text}|{}|{}|{}", sugg.len(), names[*i], ac.all))
        .unwrap_or_default();
    if key != ac.last {
        if !sugg.is_empty() {
            h.event(
                "autocomplete_offered",
                json!({ "kind": kind, "typed": text, "count": sugg.len(), "top": names[sugg[0]],
                        "all": ac.all, "style": if popup_style { "list" } else { "inline" } }),
            );
        }
        ac.last = key;
        // Combobox: the top match starts highlighted, so Enter or Tab takes it.
        ac.sel = (!sugg.is_empty()).then_some(0);
    }
    ac.open = (popup_style || ac.all) && !sugg.is_empty();
    ac.rect = None;

    let mut picked: Option<(usize, &'static str)> = None;
    if ac.open {
        if keys.down {
            ac.sel = Some(ac.sel.map_or(0, |s| (s + 1).min(sugg.len() - 1)));
        }
        if keys.up {
            ac.sel = match ac.sel {
                Some(0) | None => None,
                Some(s) => Some(s - 1),
            };
        }
        let row = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
        let max_rows = 12;
        let est = sugg.len().min(max_rows) as f32 * row + 16.0;
        // Open upward when the list would run past the bottom of the window.
        let up = r.rect.bottom() + est > ui.ctx().content_rect().bottom();
        let area = egui::Area::new(f_popup_id(id)).order(egui::Order::Foreground);
        let area = if up {
            area.pivot(egui::Align2::LEFT_BOTTOM)
                .fixed_pos(r.rect.left_top())
        } else {
            area.fixed_pos(r.rect.left_bottom())
        };
        let list = area.show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(r.rect.width().max(220.0));
                ScrollArea::vertical()
                    .max_height(max_rows as f32 * row)
                    .show(ui, |ui| {
                        for (i, s) in sugg.iter().enumerate() {
                            let item = ui.selectable_label(ac.sel == Some(i), &names[*s]);
                            h.track(&item, format!("autocomplete: {}", names[*s]));
                            if (keys.down || keys.up) && ac.sel == Some(i) {
                                item.scroll_to_me(None);
                            }
                            // Take the press, not the release: the press moves focus off the
                            // field, and a list that waits for the release has gone by then.
                            let pressed =
                                item.hovered() && ui.input(|i| i.pointer.primary_pressed());
                            if pressed || item.clicked() {
                                picked = Some((*s, "click"));
                            }
                        }
                    });
            });
        });
        ac.rect = Some(list.response.rect);
        h.region(list.response.rect, "autocomplete list");
        if keys.enter
            && let Some(s) = ac.sel
        {
            picked = Some((sugg[s], "enter"));
            ac.enter_used = true;
        }
        if keys.tab
            && picked.is_none()
            && let Some(s) = ac.sel
            && !names[sugg[s]].eq_ignore_ascii_case(text.as_str())
        {
            picked = Some((sugg[s], "tab"));
        }
    } else if !popup_style && r.has_focus() {
        let q = text.to_lowercase();
        let ghost = sugg
            .iter()
            .find(|i| names[**i].to_lowercase().starts_with(&q) && names[**i].len() > text.len())
            .copied();
        if let Some(g) = ghost {
            let font = egui::TextStyle::Body.resolve(ui.style());
            let color = ui.visuals().text_color();
            let width = ui
                .painter()
                .layout_no_wrap(text.clone(), font.clone(), color)
                .size()
                .x;
            let suffix: String = names[g].chars().skip(text.chars().count()).collect();
            ui.painter().text(
                egui::pos2(r.rect.left() + 4.0 + width, r.rect.center().y),
                egui::Align2::LEFT_CENTER,
                suffix,
                font,
                ui.visuals().weak_text_color(),
            );
            if keys.tab {
                picked = Some((g, "tab"));
            }
        }
    }
    let mut out = None;
    if let Some((i, via)) = picked {
        h.event(
            "autocomplete_picked",
            json!({ "kind": kind, "typed": text, "picked": names[i], "via": via }),
        );
        *text = names[i].clone();
        ac.last.clear();
        ac.sel = None;
        ac.all = false;
        ac.open = false;
        if via == "click" {
            ac.clicked = true;
            *focus = Some(id);
        }
        out = Some(i);
    }
    (r, out)
}

fn f_popup_id(f: Field) -> Id {
    f.id().with("autocomplete")
}

// ---------------------------------------------------------------------------------------------
// App

struct Proto {
    h: Harness,
    data: Data,
    variant: Variant,
    t: Toggles,
    sort: Sort,
    /// Transaction ids in display order.
    order: Vec<usize>,
    /// Display position of each transaction id.
    pos_of: Vec<usize>,
    /// The balance shown beside each display position.
    view_bal: Vec<i64>,
    sel: Sel,
    edit: Option<Edit>,
    /// What the entry bar (variant B) was last loaded for.
    bar_for: Option<Sel>,
    ac: Ac,
    /// Variant C's cell edit, cursor column and draft new row.
    cell: Option<CellEdit>,
    col: usize,
    draft: Edit,
    /// The stub split view (a transaction id).
    split_view: Option<usize>,
    /// A display row to scroll to, and whether to centre it.
    jump: Option<(usize, Option<Align>)>,
    /// Automatic scroll test: the current display row.
    auto_scroll: Option<f32>,
    dock_open: bool,
    status: String,
    error: Option<String>,
    /// The keys taken this frame, for variant C's payee cell.
    c_keys: Keys,
    /// Variant C's column order (kinds, see `C_TITLES`); Balance shows with toggle 2.
    cols: Vec<usize>,
    hidden: [bool; 6],
    /// The date picker's month (its first day), while open.
    picker: Option<i64>,
    /// Variant C's body scroll offset and viewport height, from the last frame.
    scroll_y: f32,
    view_h: f32,
    /// A keyboard cursor move to bring into view this frame.
    key_scroll: Option<usize>,
    /// A Tab taken from egui's focus navigation while the grid owns the keyboard (shift?).
    pending_tab: Option<bool>,
    theme: Theme,
    applied: Option<ThemeSettings>,
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
        let data = generate();
        let mut me = Self {
            h,
            data,
            variant,
            t: Toggles {
                two_line: false,
                scroll_animated: false,
                running_balance: true,
                ac_popup: true,
                gridlines: Gridlines::Off,
                striping: true,
                dense: false,
                entry_bar_top: false,
                enter_down: false,
                balance_follows_sort: false,
                dark: false,
            },
            sort: Sort {
                col: SortCol::Date,
                asc: true,
            },
            order: Vec::new(),
            pos_of: Vec::new(),
            view_bal: Vec::new(),
            sel: Sel::None,
            edit: None,
            bar_for: None,
            ac: Ac::default(),
            cell: None,
            col: 1,
            draft: Edit::blank(),
            split_view: None,
            jump: None,
            auto_scroll: None,
            dock_open: true,
            status: "Ready".to_owned(),
            error: None,
            c_keys: Keys::default(),
            cols: (0..6).collect(),
            hidden: [false; 6],
            picker: None,
            scroll_y: 0.0,
            view_h: 0.0,
            key_scroll: None,
            pending_tab: None,
            theme: Theme::default(),
            applied: None,
        };
        me.rebuild_view();
        me.sel = Sel::Txn(me.order.last().copied().unwrap_or(0));
        me.jump = Some((me.order.len(), Some(Align::BOTTOM)));
        let stats = me.stats();
        me.h.event("data", stats);
        me
    }

    fn stats(&self) -> serde_json::Value {
        let splits = self.data.txns.iter().filter(|t| t.split.is_some()).count();
        let min = self.data.txns.iter().map(|t| t.balance).min().unwrap_or(0);
        let max = self.data.txns.iter().map(|t| t.balance).max().unwrap_or(0);
        let last = self.data.txns.last().map_or(0, |t| t.balance);
        let income = self.data.txns.iter().filter(|t| t.amount > 0).count();
        json!({
            "transactions": self.data.txns.len(), "splits": splits, "income_txns": income,
            "payees": self.data.payees.len(), "categories": self.data.categories.len(),
            "opening": fmt_cents(self.data.opening), "final_balance": fmt_cents(last),
            "lowest_balance": fmt_cents(min), "highest_balance": fmt_cents(max),
            "first_day": fmt_day(FIRST_DAY), "last_day": fmt_day(FIRST_DAY + HISTORY_DAYS),
        })
    }

    // -- the sorted view --------------------------------------------------------------------

    fn rebuild_view(&mut self) {
        let d = &self.data;
        let Sort { col, asc } = self.sort;
        let mut order: Vec<usize> = (0..d.txns.len()).collect();
        order.sort_by(|&a, &b| {
            let (x, y) = (&d.txns[a], &d.txns[b]);
            let o = match col {
                SortCol::Date => x.day.cmp(&y.day),
                SortCol::Payee => d.payees_lc[x.payee as usize].cmp(&d.payees_lc[y.payee as usize]),
                SortCol::Category => {
                    d.categories[x.category as usize].cmp(&d.categories[y.category as usize])
                }
                SortCol::Amount => x.amount.cmp(&y.amount),
            }
            .then(a.cmp(&b));
            if asc { o } else { o.reverse() }
        });
        let mut pos_of = vec![0; order.len()];
        for (p, id) in order.iter().enumerate() {
            pos_of[*id] = p;
        }
        let ledger = col == SortCol::Date || !self.t.balance_follows_sort;
        let mut running = d.opening;
        self.view_bal = order
            .iter()
            .map(|&id| {
                if ledger {
                    d.txns[id].balance
                } else {
                    running += d.txns[id].amount;
                    running
                }
            })
            .collect();
        self.order = order;
        self.pos_of = pos_of;
    }

    fn pos(&self, sel: Sel) -> Option<usize> {
        match sel {
            Sel::None => None,
            Sel::Txn(id) => self.pos_of.get(id).copied(),
            Sel::New => Some(self.order.len()),
        }
    }

    fn sel_at(&self, pos: usize) -> Sel {
        self.order.get(pos).map_or(Sel::New, |id| Sel::Txn(*id))
    }

    fn set_sort(&mut self, col: SortCol, asc: Option<bool>, via: &str) {
        let asc = asc.unwrap_or(if self.sort.col == col {
            !self.sort.asc
        } else {
            true
        });
        self.sort = Sort { col, asc };
        self.rebuild_view();
        self.h.event(
            "sort",
            json!({ "column": col.name(), "ascending": asc, "via": via,
                    "balance_follows_sort": self.t.balance_follows_sort }),
        );
        self.jump = Some(match self.pos(self.sel) {
            Some(p) if self.sel != Sel::New => (p, Some(Align::Center)),
            _ if self.sort.col == SortCol::Date && self.sort.asc => {
                (self.order.len(), Some(Align::BOTTOM))
            }
            _ => (0, Some(Align::TOP)),
        });
        self.status = format!("Sorted by {} {}", col.name(), if asc { "⏶" } else { "⏷" });
    }

    // -- editing ------------------------------------------------------------------------------

    fn start_edit(&mut self, sel: Sel, via: &str) {
        let edit = match sel {
            Sel::Txn(id) => Edit::of(&self.data, id),
            Sel::New => Edit::blank(),
            Sel::None => return,
        };
        self.h.event(
            "edit_start",
            json!({ "row": format!("{sel:?}"), "via": via, "variant": self.variant.key() }),
        );
        self.sel = sel;
        self.edit = Some(edit);
        self.error = None;
        if let Some(p) = self.pos(sel) {
            self.jump = Some((p, None));
        }
    }

    fn blur(ctx: &egui::Context) {
        ctx.memory_mut(|m| {
            if let Some(f) = m.focused() {
                m.surrender_focus(f);
            }
        });
    }

    fn cancel_edit(&mut self, ctx: &egui::Context, via: &str) {
        let Some(edit) = self.edit.take() else { return };
        self.h.event(
            "cancel",
            json!({ "via": via, "was_dirty": edit.dirty, "row": edit.id }),
        );
        self.error = None;
        Self::blur(ctx);
        if self.variant == Variant::B {
            self.bar_for = None; // reload the bar from the selection: reverts the changes
        }
        self.status = "Edit cancelled".to_owned();
    }

    /// Validate and apply the form. On success returns the saved transaction's id.
    fn commit_form(&mut self, edit: &Edit, via: &str) -> Result<usize, (Field, String)> {
        let Some(amount) = parse_amount(&edit.amount) else {
            return Err((Field::Amount, format!("“{}” is not an amount", edit.amount)));
        };
        if edit.payee.trim().is_empty() {
            return Err((Field::Payee, "Payee is required".to_owned()));
        }
        let payee = match self.data.payee_index(&edit.payee) {
            Some(p) => p,
            None => {
                self.data.payees.push(edit.payee.trim().to_owned());
                self.data.payees_lc.push(edit.payee.trim().to_lowercase());
                let cat = self.data.category_index(&edit.category).unwrap_or(1);
                self.data.payee_category.push(cat);
                (self.data.payees.len() - 1) as u16
            }
        };
        let split = edit.id.is_some_and(|id| self.data.txns[id].split.is_some());
        let category = if split {
            SPLIT_CATEGORY
        } else if edit.category.trim().is_empty() {
            1
        } else {
            match self.data.category_index(&edit.category) {
                Some(c) => c,
                None => {
                    self.data.categories.push(edit.category.trim().to_owned());
                    (self.data.categories.len() - 1) as u16
                }
            }
        };
        let payee_name = self.data.payees[payee as usize].clone();
        let id = if let Some(id) = edit.id {
            let t = &mut self.data.txns[id];
            t.payee = payee;
            t.category = category;
            t.amount = amount;
            t.memo = edit.memo.clone();
            self.h.event(
                "command",
                json!({ "would_emit": format!(
                    "DomainCommand::EditTransaction {{ id: {id}, payee: {payee_name:?}, amount: {amount}, category: {:?}, memo: {:?} }}",
                    self.data.categories[category as usize], edit.memo), "source": via }),
            );
            id
        } else {
            self.data.txns.push(Txn {
                day: edit.day.unwrap_or(TODAY),
                payee,
                category,
                memo: edit.memo.clone(),
                amount,
                split: None,
                cleared: false,
                balance: 0,
            });
            let id = self.data.txns.len() - 1;
            self.h.event(
                "command",
                json!({ "would_emit": format!(
                    "DomainCommand::AddTransaction {{ payee: {payee_name:?}, amount: {amount}, category: {:?}, memo: {:?} }}",
                    self.data.categories[category as usize], edit.memo), "source": via }),
            );
            id
        };
        self.data.recompute_balances();
        self.rebuild_view();
        self.status = format!("Saved: {payee_name} {}", fmt_cents(amount));
        Ok(id)
    }

    /// Variants A and B: Enter in the form.
    fn save_edit(&mut self, ctx: &egui::Context, via: &str) {
        let Some(mut edit) = self.edit.take() else {
            return;
        };
        let result = match edit.id {
            // A save that changes nothing sends nothing.
            Some(id) if !edit.dirty => {
                self.h
                    .event("save_unchanged", json!({ "id": id, "via": via }));
                Ok(id)
            }
            _ => self.commit_form(&edit, via),
        };
        match result {
            Err((f, msg)) => {
                self.h.event(
                    "save_rejected",
                    json!({ "reason": msg, "field": f.name(), "via": via }),
                );
                self.error = Some(msg);
                edit.focus = Some(f);
                self.edit = Some(edit);
            }
            Ok(id) => {
                self.error = None;
                self.h.event(
                    "save",
                    json!({ "id": id, "new": edit.id.is_none(), "via": via }),
                );
                Self::blur(ctx);
                match (self.variant, edit.id) {
                    // Entering transactions: stay on the blank row for the next one.
                    (Variant::A, None) => {
                        self.sel = Sel::New;
                        self.edit = Some(Edit::blank());
                        self.jump = Some((self.order.len(), None));
                    }
                    (Variant::A, Some(_)) => {
                        let next = self
                            .pos(Sel::Txn(id))
                            .map_or(Sel::New, |p| self.sel_at(p + 1));
                        self.sel = next;
                        if let Some(p) = self.pos(next) {
                            self.jump = Some((p, None));
                        }
                    }
                    (Variant::B, None) => {
                        self.sel = Sel::None;
                        self.bar_for = None;
                        self.edit = Some(Edit::blank());
                    }
                    (Variant::B, Some(_)) => {
                        let next = self
                            .pos(Sel::Txn(id))
                            .map_or(Sel::None, |p| self.sel_at(p + 1));
                        self.sel = if next == Sel::New { Sel::None } else { next };
                        self.bar_for = None;
                        if let Some(p) = self.pos(self.sel) {
                            self.jump = Some((p, None));
                        }
                        let mut e = match self.sel {
                            Sel::Txn(n) => Edit::of(&self.data, n),
                            _ => Edit::blank(),
                        };
                        e.focus = Some(Field::Payee);
                        self.edit = Some(e);
                        self.bar_for = Some(self.sel);
                    }
                    (Variant::C, _) => {}
                }
            }
        }
    }

    /// Variant B: make the entry bar match the selection.
    fn sync_bar(&mut self) {
        if self.bar_for == Some(self.sel) && self.edit.is_some() {
            return;
        }
        if let Some(e) = &self.edit
            && e.dirty
            && self.bar_for.is_some()
        {
            self.h.event(
                "edit_discarded",
                json!({ "row": e.id, "reason": "selection changed" }),
            );
        }
        self.edit = Some(match self.sel {
            Sel::Txn(id) => {
                let mut e = Edit::of(&self.data, id);
                e.focus = None;
                e
            }
            _ => {
                let mut e = Edit::blank();
                e.focus = None;
                e
            }
        });
        self.bar_for = Some(self.sel);
    }

    fn select(&mut self, sel: Sel, via: &str) {
        if sel == self.sel {
            return;
        }
        if self.variant == Variant::A
            && let Some(e) = &self.edit
        {
            self.h.event(
                "edit_discarded",
                json!({ "row": e.id, "was_dirty": e.dirty, "reason": "selected another row" }),
            );
            self.edit = None;
        }
        self.h.event(
            "select",
            json!({ "row": format!("{sel:?}"), "position": self.pos(sel), "via": via }),
        );
        self.sel = sel;
    }

    fn move_selection(&mut self, delta: isize, via: &str) {
        let n = self.order.len();
        let cur = self.pos(self.sel).unwrap_or(if delta > 0 { 0 } else { n });
        let allow_new = self.variant != Variant::B;
        let max = if allow_new { n } else { n.saturating_sub(1) };
        let to = if self.sel == Sel::None && delta > 0 {
            0
        } else {
            (cur as isize + delta).clamp(0, max as isize) as usize
        };
        let sel = self.sel_at(to);
        let sel = if sel == Sel::New && !allow_new {
            Sel::Txn(self.order[n - 1])
        } else {
            sel
        };
        self.select(sel, via);
        self.jump = Some((to, None));
    }

    // -- variant C: cells ---------------------------------------------------------------------

    /// Variant C's visible columns, in order.
    fn vis(&self) -> Vec<usize> {
        self.cols
            .iter()
            .copied()
            .filter(|&k| {
                if k == 5 {
                    self.t.running_balance
                } else {
                    !self.hidden[k]
                }
            })
            .collect()
    }

    /// The visible columns a cell edit can open on, in Tab order.
    fn editable(&self) -> Vec<usize> {
        self.vis().into_iter().filter(|k| *k <= 4).collect()
    }

    fn cell_text(&self, row: Sel, col: usize) -> String {
        match (row, col) {
            (Sel::Txn(id), c) => {
                let t = &self.data.txns[id];
                match c {
                    0 => fmt_day(t.day),
                    1 => self.data.payees[t.payee as usize].clone(),
                    2 if t.split.is_some() => String::new(),
                    2 => self.data.categories[t.category as usize].clone(),
                    3 => t.memo.clone(),
                    4 => fmt_plain(t.amount),
                    _ => String::new(),
                }
            }
            (Sel::New, 0) => fmt_day(self.draft.day.unwrap_or(TODAY)),
            (Sel::New, 1) => self.draft.payee.clone(),
            (Sel::New, 2) => self.draft.category.clone(),
            (Sel::New, 3) => self.draft.memo.clone(),
            (Sel::New, 4) => self.draft.amount.clone(),
            _ => String::new(),
        }
    }

    fn start_cell_edit(&mut self, row: Sel, col: usize, typed: Option<String>, via: &str) {
        if col > 4 || row == Sel::None {
            self.h.event(
                "edit_blocked",
                json!({ "col": col, "via": via, "reason": "read-only column" }),
            );
            return;
        }
        if let Sel::Txn(id) = row
            && col == 2
            && self.data.txns[id].split.is_some()
        {
            self.split_view = Some(id);
            self.h.event("split_open", json!({ "id": id, "via": via, "would_emit": format!("AppCommand::OpenSplit {{ id: {id} }}") }));
            return;
        }
        let buf = typed.clone().unwrap_or_else(|| self.cell_text(row, col));
        self.h.event(
            "edit_start",
            json!({ "row": format!("{row:?}"), "col": col, "via": via, "replaces_content": typed.is_some() }),
        );
        self.cell = Some(CellEdit {
            row,
            col,
            buf,
            focus: true,
        });
        self.ac = Ac::default();
        self.picker = None;
        self.error = None;
    }

    /// Apply the cell edit. Returns whether it was accepted. On the draft row the value only
    /// goes into the draft; `save_draft` adds the transaction.
    fn commit_cell(&mut self, ctx: &egui::Context, via: &str) -> bool {
        let Some(ce) = self.cell.take() else {
            return true;
        };
        self.picker = None;
        let field = ["date", "payee", "category", "memo", "amount"][ce.col];
        let value = ce.buf.trim().to_owned();
        match ce.row {
            Sel::Txn(id) => {
                // A commit that changes nothing sends nothing.
                if value == self.cell_text(ce.row, ce.col).trim() {
                    self.h.event(
                        "save_unchanged",
                        json!({ "id": id, "field": field, "via": via }),
                    );
                    Self::blur(ctx);
                    return true;
                }
                match ce.col {
                    0 => match parse_date(&value) {
                        Some(d) => self.data.txns[id].day = d,
                        None => {
                            let msg = format!("“{value}” is not a date");
                            return self.reject_cell(ce, &msg, via);
                        }
                    },
                    1 if value.is_empty() => return self.reject_cell(ce, "Payee is required", via),
                    1 => {
                        let p = self.data.payee_index(&value).unwrap_or_else(|| {
                            self.data.payees.push(value.clone());
                            self.data.payees_lc.push(value.to_lowercase());
                            self.data.payee_category.push(1);
                            (self.data.payees.len() - 1) as u16
                        });
                        self.data.txns[id].payee = p;
                    }
                    2 => {
                        let c = self.data.category_index(&value).unwrap_or_else(|| {
                            self.data.categories.push(value.clone());
                            (self.data.categories.len() - 1) as u16
                        });
                        self.data.txns[id].category = if value.is_empty() { 1 } else { c };
                    }
                    3 => self.data.txns[id].memo = value.clone(),
                    _ => match parse_amount(&value) {
                        Some(a) => self.data.txns[id].amount = a,
                        None => {
                            let msg = format!("“{value}” is not an amount");
                            return self.reject_cell(ce, &msg, via);
                        }
                    },
                }
                self.h.event(
                    "command",
                    json!({ "would_emit": format!("DomainCommand::EditTransaction {{ id: {id}, {field}: {value:?} }}"), "source": via }),
                );
                self.data.recompute_balances();
                self.rebuild_view();
                self.status = format!("Saved {field}");
            }
            Sel::New => {
                if ce.col == 0 {
                    match parse_date(&value) {
                        Some(d) => self.draft.day = Some(d),
                        None => {
                            let msg = format!("“{value}” is not a date");
                            return self.reject_cell(ce, &msg, via);
                        }
                    }
                } else {
                    *self.draft.text_mut(match ce.col {
                        1 => Field::Payee,
                        2 => Field::Category,
                        3 => Field::Memo,
                        _ => Field::Amount,
                    }) = value;
                }
            }
            Sel::None => {}
        }
        self.error = None;
        Self::blur(ctx);
        true
    }

    /// Add the draft row as a transaction. On a missing payee or bad amount, reopen that cell.
    fn save_draft(&mut self, ctx: &egui::Context, via: &str) {
        let draft = self.draft.clone();
        match self.commit_form(&draft, via) {
            Ok(id) => {
                self.h
                    .event("save", json!({ "id": id, "new": true, "via": via }));
                self.draft = Edit::blank();
                self.sel = Sel::New;
                self.col = self.editable().first().copied().unwrap_or(1);
                self.key_scroll = Some(self.order.len());
                self.jump = Some((self.order.len(), None));
                Self::blur(ctx);
            }
            Err((f, msg)) => {
                let col = match f {
                    Field::Payee => 1,
                    Field::Category => 2,
                    Field::Memo => 3,
                    _ => 4,
                };
                self.h.event(
                    "save_rejected",
                    json!({ "reason": msg, "col": col, "via": via }),
                );
                self.col = col;
                self.start_cell_edit(Sel::New, col, None, "save rejected");
                self.error = Some(msg);
            }
        }
    }

    fn reject_cell(&mut self, mut ce: CellEdit, msg: &str, via: &str) -> bool {
        self.h.event(
            "save_rejected",
            json!({ "reason": msg, "col": ce.col, "via": via }),
        );
        self.error = Some(msg.to_owned());
        ce.focus = true;
        self.cell = Some(ce);
        false
    }

    fn cancel_cell(&mut self, ctx: &egui::Context, via: &str) {
        if let Some(ce) = self.cell.take() {
            self.h.event(
                "cancel",
                json!({ "via": via, "col": ce.col, "row": format!("{:?}", ce.row) }),
            );
            self.error = None;
            Self::blur(ctx);
        }
    }

    fn move_cursor(&mut self, dr: isize, dc: isize, via: &str) {
        let n = self.order.len();
        let cur = self.pos(self.sel).unwrap_or(n);
        let to = (cur as isize + dr).clamp(0, n as isize) as usize;
        let vis = self.vis();
        let at = vis.iter().position(|k| *k == self.col).unwrap_or(0) as isize;
        self.col = vis[(at + dc).clamp(0, vis.len() as isize - 1) as usize];
        self.sel = self.sel_at(to);
        self.h.event(
            "cursor",
            json!({ "position": to, "col": self.col, "via": via }),
        );
        self.jump = Some((to, None));
        self.key_scroll = Some(to);
    }

    // -- control file -------------------------------------------------------------------------

    fn set_variant(&mut self, ctx: &egui::Context, to: Variant, via: &str) {
        if to == self.variant {
            return;
        }
        self.h.event(
            "variant",
            json!({ "from": self.variant.key(), "to": to.key(), "via": via }),
        );
        if let Some(e) = &self.edit
            && e.dirty
        {
            self.h.event(
                "edit_discarded",
                json!({ "row": e.id, "reason": "variant switch" }),
            );
        }
        self.edit = None;
        self.cell = None;
        self.bar_for = None;
        self.error = None;
        Self::blur(ctx);
        self.variant = to;
        if to == Variant::C && self.sel == Sel::None {
            self.sel = self.sel_at(self.order.len().saturating_sub(1));
        }
        if let Some(p) = self.pos(self.sel) {
            self.jump = Some((p, Some(Align::Center)));
        }
        self.h.screenshot(format!("variant-{}", to.key()));
    }

    /// The theme Preferences the toggles stand for.
    fn settings(&self) -> ThemeSettings {
        ThemeSettings {
            scheme: if self.t.dark {
                Scheme::Dark
            } else {
                Scheme::Light
            },
            density: if self.t.dense {
                Density::Dense
            } else {
                Density::Normal
            },
            gridlines: self.t.gridlines,
            alternating_rows: self.t.striping,
            ..ThemeSettings::default()
        }
    }

    fn sync_theme(&mut self, ctx: &egui::Context) {
        let want = self.settings();
        if self.applied == Some(want) {
            return;
        }
        self.applied = Some(want);
        self.theme = Theme::new(want);
        let style = self.theme.style();
        ctx.set_style_of(egui::Theme::Dark, style.clone());
        ctx.set_style_of(egui::Theme::Light, style);
        ctx.set_theme(if self.t.dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });
    }

    /// Gridlines cycle off → horizontal → vertical → both, or take a named value.
    fn set_gridlines(&mut self, value: Option<&str>, via: &str) {
        let next = match value {
            Some("off") => Gridlines::Off,
            Some("horizontal") => Gridlines::Horizontal,
            Some("vertical") => Gridlines::Vertical,
            Some("both") => Gridlines::Both,
            Some(other) => {
                self.h
                    .event("control_unknown", json!({ "gridlines": other }));
                return;
            }
            None => match self.t.gridlines {
                Gridlines::Off => Gridlines::Horizontal,
                Gridlines::Horizontal => Gridlines::Vertical,
                Gridlines::Vertical => Gridlines::Both,
                Gridlines::Both => Gridlines::Off,
            },
        };
        self.t.gridlines = next;
        let name = gridlines_name(next);
        self.h.event(
            "toggle",
            json!({ "key": "gridlines", "value": name, "via": via, "applies_to": "all" }),
        );
        self.h.screenshot(format!("toggle-gridlines-{name}"));
    }

    fn set_toggle(&mut self, key: &str, value: Option<bool>, via: &str) {
        if key == "gridlines" {
            self.set_gridlines(None, via);
            return;
        }
        let Some(slot) = self.t.get_mut(key) else {
            self.h.event("control_unknown", json!({ "toggle": key }));
            return;
        };
        let new = value.unwrap_or(!*slot);
        if *slot == new && via != "click" {
            return;
        }
        *slot = new;
        let applies = TOGGLES.iter().find(|t| t.0 == key).map_or("?", |t| t.2);
        self.h.event(
            "toggle",
            json!({ "key": key, "value": new, "via": via, "applies_to": applies }),
        );
        if key == "balance_follows_sort" {
            self.rebuild_view();
        }
        self.h
            .screenshot(format!("toggle-{key}-{}", if new { "on" } else { "off" }));
    }

    fn open_split(&mut self, id: usize, via: &str) {
        self.split_view = Some(id);
        self.h.event(
            "split_open",
            json!({ "id": id, "via": via, "would_emit": format!("AppCommand::OpenSplit {{ id: {id} }}") }),
        );
    }

    fn apply_control(&mut self, ctx: &egui::Context, cmd: &str) {
        let (verb, rest) = cmd
            .split_once(' ')
            .map_or((cmd, ""), |(v, r)| (v, r.trim()));
        match verb {
            "variant" => match Variant::from_key(rest) {
                Some(v) => self.set_variant(ctx, v, "control"),
                None => self.h.event("control_unknown", json!({ "command": cmd })),
            },
            "toggle" => {
                let (key, value) = rest
                    .split_once(' ')
                    .map_or((rest, None), |(k, v)| (k, Some(v.trim() == "on")));
                if key == "gridlines" {
                    let value = rest.split_once(' ').map(|(_, v)| v.trim());
                    self.set_gridlines(value, "control");
                } else {
                    self.set_toggle(key, value, "control");
                }
            }
            "row" => {
                let target = if rest == "split" {
                    let from = self.pos(self.sel).map_or(0, |p| p + 1);
                    (from..self.order.len())
                        .chain(0..from.min(self.order.len()))
                        .find(|p| self.data.txns[self.order[*p]].split.is_some())
                } else if rest == "end" {
                    Some(self.order.len().saturating_sub(1))
                } else {
                    rest.parse::<usize>().ok().filter(|p| *p < self.order.len())
                };
                match target {
                    Some(p) => {
                        self.edit = if self.variant == Variant::A {
                            None
                        } else {
                            self.edit.take()
                        };
                        self.sel = self.sel_at(p);
                        self.jump = Some((p, Some(Align::Center)));
                        self.h
                            .event("select", json!({ "position": p, "via": "control" }));
                    }
                    None => self.h.event("control_unknown", json!({ "command": cmd })),
                }
            }
            "sort" => {
                let (name, dir) = rest
                    .split_once(' ')
                    .map_or((rest, None), |(n, d)| (n, Some(d.trim())));
                match SortCol::from_name(name) {
                    Some(c) => self.set_sort(c, dir.map(|d| d != "desc"), "control"),
                    None => self.h.event("control_unknown", json!({ "command": cmd })),
                }
            }
            "edit" => match self.variant {
                Variant::A => {
                    let s = if self.sel == Sel::None {
                        Sel::New
                    } else {
                        self.sel
                    };
                    self.start_edit(s, "control");
                }
                Variant::B => {
                    if let Some(e) = &mut self.edit {
                        e.focus = Some(Field::Payee);
                    }
                }
                Variant::C => self.start_cell_edit(self.sel, self.col, None, "control"),
            },
            "new" => match self.variant {
                Variant::A => self.start_edit(Sel::New, "control"),
                Variant::B => {
                    self.sel = Sel::None;
                    self.bar_for = None;
                    self.sync_bar();
                    if let Some(e) = &mut self.edit {
                        e.focus = Some(Field::Payee);
                    }
                }
                Variant::C => {
                    self.sel = Sel::New;
                    self.col = 1;
                    self.jump = Some((self.order.len(), None));
                }
            },
            "split" => {
                if let Sel::Txn(id) = self.sel {
                    self.open_split(id, "control");
                }
            }
            "autoscroll" => {
                self.auto_scroll = (rest != "off").then_some(0.0);
                self.h.event("autoscroll", json!({ "on": rest != "off" }));
            }
            "reload" => {
                self.data = generate();
                self.rebuild_view();
                self.sel = Sel::Txn(self.order.last().copied().unwrap_or(0));
                self.edit = None;
                self.cell = None;
                self.bar_for = None;
                self.jump = Some((self.order.len(), Some(Align::BOTTOM)));
                let stats = self.stats();
                self.h.event("reload", stats);
            }
            "screenshot" => self
                .h
                .screenshot(if rest.is_empty() { "control" } else { rest }),
            "dock" => self.set_dock(rest != "off", "control"),
            "quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            _ => self.h.event("control_unknown", json!({ "command": cmd })),
        }
    }

    // -- keys ---------------------------------------------------------------------------------

    fn take_keys(&mut self, ctx: &egui::Context) -> Keys {
        let focused = ctx.memory(egui::Memory::focused);
        let field = [
            Field::Payee,
            Field::Amount,
            Field::Category,
            Field::Memo,
            Field::Cell,
        ]
        .into_iter()
        .find(|f| Some(f.id()) == focused);
        if field.is_none() {
            return Keys::default();
        }
        let ac_open = self.ac.open;
        // `consume_key` ignores Shift and Alt the pattern leaves out, so a plain-key pattern
        // also eats Shift+Tab and Alt+Down: take the modified keys first.
        ctx.input_mut(|i| {
            let ctrl_enter = i.consume_key(Modifiers::COMMAND, Key::Enter);
            let back_tab = i.consume_key(Modifiers::SHIFT, Key::Tab);
            let alt_down = i.consume_key(Modifiers::ALT, Key::ArrowDown);
            Keys {
                enter: i.consume_key(Modifiers::NONE, Key::Enter),
                ctrl_enter,
                esc: i.consume_key(Modifiers::NONE, Key::Escape),
                tab: i.consume_key(Modifiers::NONE, Key::Tab),
                back_tab,
                down: ac_open && i.consume_key(Modifiers::NONE, Key::ArrowDown),
                up: ac_open && i.consume_key(Modifiers::NONE, Key::ArrowUp),
                alt_down,
                field,
            }
        })
    }

    fn hotkeys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        // Variant C's grid owns plain keys (typing starts an edit), so it needs Alt.
        let alt = self.variant == Variant::C;
        let pressed = |k: Key| {
            ctx.input(|i| {
                i.key_pressed(k)
                    && if alt {
                        i.modifiers.alt && !i.modifiers.command
                    } else {
                        i.modifiers.is_none()
                    }
            })
        };
        if self.variant != Variant::C || ctx.input(|i| i.modifiers.alt) {
            if pressed(Key::ArrowLeft) {
                self.set_variant(ctx, self.variant.step(-1), "key");
            }
            if pressed(Key::ArrowRight) {
                self.set_variant(ctx, self.variant.step(1), "key");
            }
        }
        for (v, (k, _)) in Variant::ALL.into_iter().zip(VARIANT_KEYS) {
            if pressed(k) {
                self.set_variant(ctx, v, "key");
            }
        }
        for (&(key, _, _), (k, _)) in TOGGLES.iter().zip(TOGGLE_KEYS) {
            if pressed(k) {
                self.set_toggle(key, None, "key");
            }
        }
    }

    /// Row navigation shared by A and B (C has its own).
    fn grid_keys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() || self.split_view.is_some() {
            return;
        }
        let (up, down, pgup, pgdn, home, end, enter, newtxn, delete) = ctx.input(|i| {
            (
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::PageUp),
                i.key_pressed(Key::PageDown),
                i.key_pressed(Key::Home),
                i.key_pressed(Key::End),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::N) && i.modifiers.command,
                i.key_pressed(Key::Delete),
            )
        });
        if up {
            self.move_selection(-1, "key");
        }
        if down {
            self.move_selection(1, "key");
        }
        if pgup {
            self.move_selection(-20, "key");
        }
        if pgdn {
            self.move_selection(20, "key");
        }
        if home {
            self.move_selection(-(self.order.len() as isize), "key");
        }
        if end {
            self.move_selection(self.order.len() as isize, "key");
        }
        if newtxn {
            self.apply_control(ctx, "new");
        }
        if delete && let Sel::Txn(id) = self.sel {
            self.h.event(
                "command",
                json!({ "would_emit": format!("DomainCommand::DeleteTransaction {{ id: {id} }}"), "source": "Delete key" }),
            );
            self.status = format!("Would delete transaction {id}");
        }
        if enter {
            match self.variant {
                Variant::A => {
                    let s = if self.sel == Sel::None {
                        Sel::New
                    } else {
                        self.sel
                    };
                    self.start_edit(s, "enter");
                }
                Variant::B => {
                    if let Some(e) = &mut self.edit {
                        e.focus = Some(Field::Payee);
                    }
                }
                Variant::C => {}
            }
        }
    }

    /// Variant C, while a cell is being edited: Esc, Enter and Tab, after the editor has drawn
    /// (so a combobox has already put its pick into the cell).
    fn cell_edit_keys(&mut self, ctx: &egui::Context, keys: Keys) {
        let Some(ce) = &self.cell else {
            return;
        };
        let (col, row) = (ce.col, ce.row);
        let clicked = std::mem::take(&mut self.ac.clicked);
        self.ac.enter_used = false;
        if keys.esc {
            self.cancel_cell(ctx, "esc");
            self.picker = None;
            return;
        }
        if keys.ctrl_enter {
            if !self.commit_cell(ctx, "ctrl+enter") {
                return;
            }
            if row == Sel::New {
                self.save_draft(ctx, "ctrl+enter");
            }
        } else if keys.enter {
            if !self.commit_cell(ctx, "enter") {
                return;
            }
            if self.t.enter_down {
                if row == Sel::New {
                    self.save_draft(ctx, "enter");
                } else {
                    self.move_cursor(1, 0, "enter");
                }
            } else {
                self.enter_right(ctx, row, col);
            }
        } else if keys.tab || keys.back_tab || clicked {
            let back = keys.back_tab;
            let via = if clicked { "click pick" } else { "tab" };
            if !self.commit_cell(ctx, via) {
                return;
            }
            // Tab walks the visible editable columns in order and keeps editing.
            let ed = self.editable();
            let i = ed.iter().position(|k| *k == col).unwrap_or(0);
            let next = if back {
                i.checked_sub(1)
            } else {
                (i + 1 < ed.len()).then_some(i + 1)
            };
            match next {
                Some(j) => {
                    self.col = ed[j];
                    self.h.event(
                        "cursor",
                        json!({ "row": format!("{row:?}"), "col": self.col, "via": via }),
                    );
                    self.start_cell_edit(row, self.col, None, via);
                }
                // Tab past the last column of the draft row adds the transaction.
                None if row == Sel::New && !back => self.save_draft(ctx, via),
                None => {
                    let edge = if back { ed.last() } else { ed.first() };
                    self.move_cursor(if back { -1 } else { 1 }, 0, via);
                    self.col = edge.copied().unwrap_or(1);
                }
            }
        }
    }

    /// Enter moving right: the cursor goes to the next editable column without editing it; past
    /// the last it wraps to the next row's first, and on the draft row it adds the transaction.
    fn enter_right(&mut self, ctx: &egui::Context, row: Sel, col: usize) {
        let ed = self.editable();
        let i = ed.iter().position(|k| *k == col).unwrap_or(0);
        match ed.get(i + 1) {
            Some(&next) => {
                self.col = next;
                self.h.event(
                    "cursor",
                    json!({ "row": format!("{row:?}"), "col": next, "via": "enter" }),
                );
            }
            None if row == Sel::New => self.save_draft(ctx, "enter"),
            None => {
                self.move_cursor(1, 0, "enter");
                self.col = ed.first().copied().unwrap_or(1);
            }
        }
    }

    /// Variant C, while no cell is being edited: move the cursor, or start an edit.
    fn cell_keys(&mut self, ctx: &egui::Context, _keys: Keys) {
        if self.cell.is_some() {
            return;
        }
        if ctx.egui_wants_keyboard_input() || self.split_view.is_some() {
            return;
        }
        let (up, down, left, right, pgup, pgdn, home, end, enter, f2, tab, back_tab) = ctx
            .input_mut(|i| {
                (
                    i.key_pressed(Key::ArrowUp),
                    i.key_pressed(Key::ArrowDown),
                    i.key_pressed(Key::ArrowLeft) && !i.modifiers.alt,
                    i.key_pressed(Key::ArrowRight) && !i.modifiers.alt,
                    i.key_pressed(Key::PageUp),
                    i.key_pressed(Key::PageDown),
                    i.key_pressed(Key::Home),
                    i.key_pressed(Key::End),
                    i.key_pressed(Key::Enter),
                    i.key_pressed(Key::F2) && false,
                    false,
                    false,
                )
            });
        let (tab, back_tab) = match self.pending_tab.take() {
            Some(shift) => (!shift, shift),
            None => (tab, back_tab),
        };
        if up {
            self.move_cursor(-1, 0, "key");
        }
        if down {
            self.move_cursor(1, 0, "key");
        }
        if left || back_tab {
            self.move_cursor(0, -1, "key");
        }
        if right || tab {
            self.move_cursor(0, 1, "key");
        }
        if pgup {
            self.move_cursor(-20, 0, "key");
        }
        if pgdn {
            self.move_cursor(20, 0, "key");
        }
        if home {
            self.move_cursor(-(self.order.len() as isize), 0, "key");
        }
        if end {
            self.move_cursor(self.order.len() as isize, 0, "key");
        }
        if enter && ctx.input(|i| i.modifiers.command) {
            if self.sel == Sel::New {
                self.save_draft(ctx, "ctrl+enter");
            }
        } else if enter || f2 {
            self.start_cell_edit(self.sel, self.col, None, "enter");
        }
        let typed: Option<String> = ctx.input(|i| {
            if i.modifiers.alt || i.modifiers.command {
                return None;
            }
            i.events.iter().find_map(|e| match e {
                egui::Event::Text(t) if !t.chars().all(char::is_control) => Some(t.clone()),
                _ => None,
            })
        });
        if let Some(t) = typed {
            self.start_cell_edit(self.sel, self.col, Some(t), "typing");
        }
    }

    // -- chrome -------------------------------------------------------------------------------

    fn status_bar(&mut self, ui: &mut Ui) {
        let frame = self
            .theme
            .chrome_frame()
            .inner_margin(Margin::symmetric(8, 3));
        let r = Panel::bottom("status").frame(frame).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Everyday Checking · {} transactions · sorted by {} {}",
                    self.order.len(),
                    self.sort.col.name(),
                    if self.sort.asc { "⏶" } else { "⏷" }
                ));
                ui.separator();
                if let Some(err) = &self.error {
                    let c = ui.visuals().error_fg_color;
                    ui.label(RichText::new(err).color(c));
                } else {
                    ui.label(RichText::new(&self.status).weak());
                }
            });
        });
        self.h.region(r.response.rect, "status bar");
    }

    fn row_height(&self, lines: u32) -> f32 {
        self.theme.row_height() * lines as f32
    }

    fn split_window(&mut self, ui: &mut Ui) {
        let Some(id) = self.split_view else { return };
        let ctx = ui.ctx().clone();
        let t = &self.data.txns[id];
        let mut open = true;
        let mut close = false;
        let shown = egui::Window::new("Split transaction")
            .open(&mut open)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(&ctx, |ui| {
                ui.strong(format!(
                    "{} · {} · {}",
                    fmt_day(t.day),
                    self.data.payees[t.payee as usize],
                    fmt_cents(t.amount)
                ));
                ui.separator();
                egui::Grid::new("split_lines")
                    .num_columns(2)
                    .show(ui, |ui| {
                        for (c, amt) in t.split.as_deref().unwrap_or(&[]) {
                            ui.label(&self.data.categories[*c as usize]);
                            money_label(ui, *amt);
                            ui.end_row();
                        }
                    });
                let left = t.amount
                    - t.split
                        .as_deref()
                        .unwrap_or(&[])
                        .iter()
                        .map(|l| l.1)
                        .sum::<i64>();
                ui.label(format!("Unassigned: {}", fmt_cents(left)));
                ui.separator();
                ui.label(RichText::new("Stub: the real split editor is a Dialog.").weak());
                if ui.button("Close (Esc)").clicked() {
                    close = true;
                }
            });
        if let Some(shown) = shown {
            self.h.region(shown.response.rect, "split view");
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            close = true;
        }
        if !open || close {
            self.split_view = None;
            self.h.event("split_close", json!({ "id": id }));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The app loop

impl eframe::App for Proto {
    /// In C the grid owns Tab while no field has focus; egui would otherwise move focus to the
    /// next widget (the dock) before the prototype sees the key.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if self.variant != Variant::C
            || self.split_view.is_some()
            || ctx.memory(|m| m.focused().is_some())
        {
            return;
        }
        let mut tab = None;
        raw.events.retain(|e| match e {
            egui::Event::Key {
                key: Key::Tab,
                pressed,
                modifiers,
                ..
            } => {
                if *pressed {
                    tab = Some(modifiers.shift);
                }
                false
            }
            _ => true,
        });
        if tab.is_some() {
            self.pending_tab = tab;
        }
    }

    fn on_exit(&mut self) {
        self.h.event("session_end", json!({}));
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(cmd) = self.h.begin_frame(&ctx) {
            self.apply_control(&ctx, &cmd);
        }
        self.sync_theme(&ctx);
        if ctx.input(|i| i.key_pressed(Key::F4)) {
            self.set_dock(!self.dock_open, "key");
        }
        if let Some(row) = &mut self.auto_scroll {
            *row += 60.0;
            let to = *row as usize;
            if to >= self.order.len() {
                self.auto_scroll = None;
                self.h.event("autoscroll", json!({ "done": true }));
            } else {
                self.jump = Some((to, Some(Align::TOP)));
            }
        }

        let keys = self.take_keys(&ctx);
        self.ac.enter_used = false;
        self.c_keys = keys;
        self.hotkeys(&ctx);
        if self.split_view.is_none() {
            match self.variant {
                Variant::A | Variant::B => self.grid_keys(&ctx),
                Variant::C => self.cell_keys(&ctx, keys),
            }
        }

        self.dock(ui);
        match self.variant {
            Variant::A => self.variant_a(ui, keys),
            Variant::B => self.variant_b(ui, keys),
            Variant::C => self.variant_c(ui, keys),
        }
        self.split_window(ui);

        if self.variant == Variant::C {
            self.cell_edit_keys(&ctx, keys);
        }
        // Enter / Esc / Tab in the form fields (A and B), after the fields have drawn.
        if self.variant != Variant::C && self.edit.is_some() {
            if keys.esc {
                self.cancel_edit(&ctx, "esc");
            } else if keys.enter && !self.ac.enter_used {
                self.save_edit(&ctx, "enter");
            } else if (keys.tab || keys.back_tab)
                && let (Some(cur), Some(edit)) = (keys.field, &mut self.edit)
            {
                let skip = edit.id.is_some_and(|id| self.data.txns[id].split.is_some());
                edit.focus = Some(cur.step(if keys.back_tab { -1 } else { 1 }, skip));
            }
        }
        self.h.end_frame(&ctx);
    }
}

// ---------------------------------------------------------------------------------------------
// Shared grid pieces

enum Act {
    Select(Sel),
    StartEdit(Sel),
    Sort(SortCol),
    Split(usize),
    Cell(Sel, usize),
    CellEdit(Sel, usize),
}

fn sort_header(
    ui: &mut Ui,
    h: &mut Harness,
    sort: Sort,
    col: SortCol,
    title: &str,
    acts: &mut Vec<Act>,
) {
    let active = sort.col == col;
    let arrow = if !active {
        ""
    } else if sort.asc {
        " ⏶"
    } else {
        " ⏷"
    };
    let text = RichText::new(format!("{title}{arrow}")).strong();
    let label = Label::new(text).sense(Sense::click());
    let r = if col == SortCol::Amount {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if last_col(ui) {
                ui.add_space(Theme::EDGE_INSET);
            }
            ui.add(label)
        })
        .inner
    } else {
        if first_col(ui) {
            ui.add_space(Theme::EDGE_INSET);
        }
        ui.add(label)
    };
    h.track(&r, format!("header: {title}"));
    if r.clicked() {
        acts.push(Act::Sort(col));
    }
}

impl Proto {
    fn apply_acts(&mut self, acts: Vec<Act>) {
        for act in acts {
            match act {
                Act::Select(sel) => self.select(sel, "click"),
                Act::StartEdit(sel) => self.start_edit(sel, "click"),
                Act::Sort(col) => self.set_sort(col, None, "click"),
                Act::Split(id) => self.open_split(id, "click"),
                Act::Cell(sel, col) => {
                    if self.cell.is_some() {
                        let ctx = egui::Context::default();
                        self.cancel_cell(&ctx, "click elsewhere");
                    }
                    self.sel = sel;
                    self.col = col;
                    self.h.event(
                        "cursor",
                        json!({ "row": format!("{sel:?}"), "col": col, "via": "click" }),
                    );
                }
                Act::CellEdit(sel, col) => {
                    self.sel = sel;
                    self.col = col;
                    self.start_cell_edit(sel, col, None, "double click");
                }
            }
        }
    }

    fn table<'a>(&self, ui: &'a mut Ui, id: &str) -> TableBuilder<'a> {
        let mut b = TableBuilder::new(ui)
            .id_salt(id)
            .striped(self.t.striping)
            // Off: its drag handles draw a line at every column boundary, read as gridlines.
            .resizable(false)
            .sense(Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center));
        if let Some((row, align)) = self.jump_clone() {
            b = b.scroll_to_row(row, align);
        }
        b
    }

    fn jump_clone(&self) -> Option<(usize, Option<Align>)> {
        self.jump
    }
}

// ---------------------------------------------------------------------------------------------
// Variant A: classic two-line rows, edited in place. The blank entry row sits at the end.

impl Proto {
    fn variant_a(&mut self, ui: &mut Ui, keys: Keys) {
        self.status_bar(ui);
        let r = CentralPanel::default().show(ui, |ui| {
            ScrollArea::horizontal().show(ui, |ui| self.grid_a(ui, keys))
        });
        self.h.region(r.response.rect, "register");
    }

    fn grid_a(&mut self, ui: &mut Ui, keys: Keys) {
        let two = self.t.two_line;
        let row_h = self.row_height(if two { 2 } else { 1 });
        let n = self.order.len();
        set_grid(ui, &self.theme);
        let balance = self.t.running_balance;
        let sort = self.sort;
        let popup = self.t.ac_popup;
        let mut acts: Vec<Act> = Vec::new();
        let mut builder = self.table(ui, "register_a");
        builder = builder.column(Column::exact(90.0));
        builder = if two {
            builder
                .column(Column::remainder().at_least(160.0).clip(true))
                .column(Column::initial(240.0).at_least(100.0))
        } else {
            builder
                .column(Column::remainder().at_least(120.0).clip(true))
                .column(Column::remainder().at_least(100.0).clip(true))
                .column(Column::initial(160.0).at_least(80.0))
                .column(Column::exact(100.0))
        };
        if balance {
            builder = builder.column(Column::exact(100.0));
        }
        self.jump = None;
        let out = builder
            .header(self.row_height(1) + 2.0, |mut hd| {
                hd.col(|ui| {
                    header_band(ui);
                    sort_header(ui, &mut self.h, sort, SortCol::Date, "Date", &mut acts);
                });
                hd.col(|ui| {
                    header_band(ui);
                    sort_header(ui, &mut self.h, sort, SortCol::Payee, "Payee", &mut acts);
                    if two {
                        ui.label("/");
                        sort_header(ui, &mut self.h, sort, SortCol::Category, "Category", &mut acts);
                    }
                });
                if two {
                    hd.col(|ui| {
                    header_band(ui);
                        sort_header(ui, &mut self.h, sort, SortCol::Amount, "Amount", &mut acts);
                        ui.label("/ Memo");
                    });
                } else {
                    hd.col(|ui| {
                    header_band(ui);
                        sort_header(ui, &mut self.h, sort, SortCol::Category, "Category", &mut acts);
                    });
                    hd.col(|ui| {
                    header_band(ui);
                        ui.strong("Memo");
                    });
                    hd.col(|ui| {
                    header_band(ui);
                        sort_header(ui, &mut self.h, sort, SortCol::Amount, "Amount", &mut acts);
                    });
                }
                if balance {
                    hd.col(|ui| {
                    header_band(ui);
                        money_header(ui, "Balance");
                    });
                }
            })
            .body(|body| {
                body.rows(row_h, n + 1, |mut row| {
                    let pos = row.index();
                    let this = if pos < n { Sel::Txn(self.order[pos]) } else { Sel::New };
                    row.set_selected(self.sel == this);
                    let editing = self.sel == this
                        && self.edit.as_ref().is_some_and(|e| {
                            e.id == if let Sel::Txn(id) = this { Some(id) } else { None }
                        });
                    if editing {
                        let edit = self.edit.as_mut().unwrap();
                        let skip_cat = matches!(this, Sel::Txn(id) if self.data.txns[id].split.is_some());
                        let tag = format!("row {pos}");
                        row.col(|ui| {
                            cell_label(ui, if let Sel::Txn(id) = this { fmt_day(self.data.txns[id].day) } else { fmt_day(TODAY) });
                        });
                        let ac_keys = keys;
                        let mut picked = None;
                        let mut field_responses = Vec::new();
                        row.col(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.vertical(|ui| {
                                let (r, p) = combo_edit(ui, &mut self.h, &self.data.payees, 0, "payee", &mut self.ac, popup, &ac_keys,
                                    &mut edit.payee, &mut edit.focus, format!("payee field, {tag}"), Field::Payee, false, false);
                                field_responses.push(r.changed());
                                picked = p;
                                if two {
                                    if skip_cat {
                                        if ui.button("Split…").clicked() && let Sel::Txn(id) = this {
                                            acts.push(Act::Split(id));
                                        }
                                    } else {
                                        let r = field(ui, &mut self.h, Field::Category, &mut edit.category, &mut edit.focus, format!("category field, {tag}"), false);
                                        field_responses.push(r.changed());
                                    }
                                }
                            });
                        });
                        if two {
                            row.col(|ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                ui.vertical(|ui| {
                                    let r = field(ui, &mut self.h, Field::Amount, &mut edit.amount, &mut edit.focus, format!("amount field, {tag}"), true);
                                    field_responses.push(r.changed());
                                    let r = field(ui, &mut self.h, Field::Memo, &mut edit.memo, &mut edit.focus, format!("memo field, {tag}"), false);
                                    field_responses.push(r.changed());
                                });
                            });
                        } else {
                            row.col(|ui| {
                                if skip_cat {
                                    if ui.button("Split…").clicked() && let Sel::Txn(id) = this {
                                        acts.push(Act::Split(id));
                                    }
                                } else {
                                    let r = field(ui, &mut self.h, Field::Category, &mut edit.category, &mut edit.focus, format!("category field, {tag}"), false);
                                    field_responses.push(r.changed());
                                }
                            });
                            row.col(|ui| {
                                let r = field(ui, &mut self.h, Field::Memo, &mut edit.memo, &mut edit.focus, format!("memo field, {tag}"), false);
                                field_responses.push(r.changed());
                            });
                            row.col(|ui| {
                                let r = field(ui, &mut self.h, Field::Amount, &mut edit.amount, &mut edit.focus, format!("amount field, {tag}"), true);
                                field_responses.push(r.changed());
                            });
                        }
                        if balance {
                            row.col(|ui| {
                                if let Sel::Txn(_) = this {
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| money_label(ui, self.view_bal[pos]));
                                }
                            });
                        }
                        edit.dirty |= field_responses.iter().any(|c| *c);
                        if let Some(p) = picked
                            && edit.category.trim().is_empty()
                        {
                            edit.category = self.data.categories[self.data.payee_category[p] as usize].clone();
                        }
                        return;
                    }

                    // A read-only row (or the idle blank row).
                    let mut clicked = false;
                    let mut double = false;
                    let mut note = |resp: &egui::Response, h: &mut Harness, what: &str| {
                        h.track(resp, format!("{what} cell, row {pos}"));
                        clicked |= resp.clicked();
                        double |= resp.double_clicked();
                    };
                    if let Sel::Txn(id) = this {
                        let t = &self.data.txns[id];
                        let (_, r) = row.col(|ui| {
                            cell_label(ui, fmt_day(t.day));
                            grid_lines(ui);
                        });
                        note(&r, &mut self.h, "date");
                        let (_, r) = row.col(|ui| {
                            if two {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    cell_label(ui, &self.data.payees[t.payee as usize]);
                                    if t.split.is_some() {
                                        if ui.link(RichText::new("[Split] click to view").small()).clicked() {
                                            acts.push(Act::Split(id));
                                        }
                                    } else {
                                        cell_label(ui, RichText::new(&self.data.categories[t.category as usize]).weak());
                                    }
                                });
                            } else {
                                cell_label(ui, &self.data.payees[t.payee as usize]);
                            }
                            grid_lines(ui);
                        });
                        note(&r, &mut self.h, "payee");
                        if two {
                            let (_, r) = row.col(|ui| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| money_label(ui, t.amount));
                                    cell_label(ui, RichText::new(&t.memo).weak());
                                });
                                grid_lines(ui);
                            });
                            note(&r, &mut self.h, "amount");
                        } else {
                            let (_, r) = row.col(|ui| {
                                if t.split.is_some() {
                                    if ui.link("[Split]").clicked() {
                                        acts.push(Act::Split(id));
                                    }
                                } else {
                                    cell_label(ui, &self.data.categories[t.category as usize]);
                                }
                                grid_lines(ui);
                            });
                            note(&r, &mut self.h, "category");
                            let (_, r) = row.col(|ui| {
                                cell_label(ui, RichText::new(&t.memo).weak());
                                grid_lines(ui);
                            });
                            note(&r, &mut self.h, "memo");
                            let (_, r) = row.col(|ui| {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| money_label(ui, t.amount));
                                grid_lines(ui);
                            });
                            note(&r, &mut self.h, "amount");
                        }
                        if balance {
                            let (_, r) = row.col(|ui| {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| money_label(ui, self.view_bal[pos]));
                                grid_lines(ui);
                            });
                            note(&r, &mut self.h, "balance");
                        }
                    } else {
                        let cols = if two { 3 } else { 5 } + usize::from(balance);
                        for c in 0..cols {
                            let (_, r) = row.col(|ui| {
                                if c == 0 {
                                    cell_label(ui, RichText::new(fmt_day(TODAY)).weak());
                                } else if c == 1 {
                                    ui.label(RichText::new("New transaction — click or press Enter").weak().italics());
                                }
                                grid_lines(ui);
                            });
                            note(&r, &mut self.h, "new row");
                        }
                    }
                    let resp_row = row.response();
                    resp_row.context_menu(|ui| {
                        if let Sel::Txn(id) = this {
                            for label in ["Edit", "Split…", "Mark Cleared", "Duplicate", "Delete"] {
                                if ui.button(label).clicked() {
                                    self.h.event("command", json!({ "would_emit": format!("DomainCommand::{}{{ id: {id} }}", label.replace(['…', ' '], "")), "source": "register row menu" }));
                                    if label == "Edit" {
                                        acts.push(Act::StartEdit(this));
                                    }
                                    if label == "Split…" {
                                        acts.push(Act::Split(id));
                                    }
                                    ui.close();
                                }
                            }
                        }
                    });
                    if this == Sel::New && clicked {
                        acts.push(Act::StartEdit(Sel::New));
                    } else if double {
                        acts.push(Act::StartEdit(this));
                    } else if clicked || resp_row.clicked() {
                        acts.push(Act::Select(this));
                    }
                });
            });
        self.h.scroll("register", out.state.offset.y, row_h + 3.0);
        self.apply_acts(acts);
    }
}

// ---------------------------------------------------------------------------------------------
// Variant B: dense one-line read-only rows, with a fixed entry bar that edits the selection.

impl Proto {
    fn variant_b(&mut self, ui: &mut Ui, keys: Keys) {
        self.status_bar(ui);
        self.sync_bar();
        let panel = if self.t.entry_bar_top {
            Panel::top("entry_bar")
        } else {
            Panel::bottom("entry_bar")
        };
        let frame = self
            .theme
            .chrome_frame()
            .inner_margin(Margin::symmetric(8, 6));
        let r = panel.frame(frame).show(ui, |ui| self.entry_bar(ui, keys));
        self.h.region(r.response.rect, "entry bar");
        let r = CentralPanel::default().show(ui, |ui| {
            ScrollArea::horizontal().show(ui, |ui| self.grid_b(ui))
        });
        self.h.region(r.response.rect, "register");
    }

    fn entry_bar(&mut self, ui: &mut Ui, keys: Keys) {
        let popup = self.t.ac_popup;
        let Some(mut edit) = self.edit.take() else {
            return;
        };
        let mut save = false;
        let mut cancel = false;
        let mut new = false;
        let mut split = None;
        let mut picked = None;
        let is_split = edit.id.is_some_and(|id| self.data.txns[id].split.is_some());
        ui.horizontal(|ui| {
            let title = match edit.id {
                Some(id) => format!(
                    "Editing: {} {}",
                    fmt_day(self.data.txns[id].day),
                    if edit.dirty { "(changed)" } else { "" }
                ),
                None => "New transaction".to_owned(),
            };
            ui.strong(title);
            if let Some(err) = &self.error {
                let c = ui.visuals().error_fg_color;
                ui.label(RichText::new(err).color(c));
            }
        });
        ui.horizontal_wrapped(|ui| {
            let mut changed = false;
            ui.label("Payee");
            ui.scope(|ui| {
                ui.set_width(260.0);
                let (r, p) = combo_edit(
                    ui,
                    &mut self.h,
                    &self.data.payees,
                    0,
                    "payee",
                    &mut self.ac,
                    popup,
                    &keys,
                    &mut edit.payee,
                    &mut edit.focus,
                    "entry bar: payee".to_owned(),
                    Field::Payee,
                    false,
                    false,
                );
                changed |= r.changed();
                picked = p;
            });
            ui.label("Category");
            ui.scope(|ui| {
                ui.set_width(200.0);
                if is_split {
                    if ui.button("Split…").clicked() {
                        split = edit.id;
                    }
                } else {
                    let r = field(
                        ui,
                        &mut self.h,
                        Field::Category,
                        &mut edit.category,
                        &mut edit.focus,
                        "entry bar: category".to_owned(),
                        false,
                    );
                    changed |= r.changed();
                }
            });
            ui.label("Memo");
            ui.scope(|ui| {
                ui.set_width(200.0);
                let r = field(
                    ui,
                    &mut self.h,
                    Field::Memo,
                    &mut edit.memo,
                    &mut edit.focus,
                    "entry bar: memo".to_owned(),
                    false,
                );
                changed |= r.changed();
            });
            ui.label("Amount");
            ui.scope(|ui| {
                ui.set_width(100.0);
                let r = field(
                    ui,
                    &mut self.h,
                    Field::Amount,
                    &mut edit.amount,
                    &mut edit.focus,
                    "entry bar: amount".to_owned(),
                    true,
                );
                changed |= r.changed();
            });
            edit.dirty |= changed;
            let r = ui.button("Save (Enter)");
            self.h.track(&r, "entry bar: save");
            save = r.clicked();
            let r = ui.button("Cancel (Esc)");
            self.h.track(&r, "entry bar: cancel");
            cancel = r.clicked();
            let r = ui.button("New (Ctrl+N)");
            self.h.track(&r, "entry bar: new");
            new = r.clicked();
        });
        if let Some(p) = picked
            && edit.category.trim().is_empty()
        {
            edit.category = self.data.categories[self.data.payee_category[p] as usize].clone();
        }
        self.edit = Some(edit);
        let ctx = ui.ctx().clone();
        if let Some(id) = split {
            self.open_split(id, "click");
        }
        if save {
            self.save_edit(&ctx, "save button");
        }
        if cancel {
            self.cancel_edit(&ctx, "cancel button");
        }
        if new {
            self.apply_control(&ctx, "new");
        }
    }

    fn grid_b(&mut self, ui: &mut Ui) {
        let row_h = self.row_height(1);
        let n = self.order.len();
        set_grid(ui, &self.theme);
        let balance = self.t.running_balance;
        let sort = self.sort;
        let mut acts: Vec<Act> = Vec::new();
        let mut builder = self.table(ui, "register_b");
        builder = builder
            .column(Column::exact(88.0))
            .column(Column::remainder().at_least(120.0).clip(true))
            .column(Column::remainder().at_least(100.0).clip(true))
            .column(Column::initial(170.0).at_least(80.0))
            .column(Column::exact(100.0));
        if balance {
            builder = builder.column(Column::exact(100.0));
        }
        self.jump = None;
        let out = builder
            .header(self.row_height(1) + 2.0, |mut hd| {
                for (title, col) in [
                    ("Date", Some(SortCol::Date)),
                    ("Payee", Some(SortCol::Payee)),
                    ("Category", Some(SortCol::Category)),
                    ("Memo", None),
                    ("Amount", Some(SortCol::Amount)),
                ] {
                    hd.col(|ui| {
                        header_band(ui);
                        match col {
                            Some(c) => sort_header(ui, &mut self.h, sort, c, title, &mut acts),
                            None => {
                                ui.strong(title);
                            }
                        }
                    });
                }
                if balance {
                    hd.col(|ui| {
                    header_band(ui);
                        money_header(ui, "Balance");
                    });
                }
            })
            .body(|body| {
                body.rows(row_h, n, |mut row| {
                    let pos = row.index();
                    let id = self.order[pos];
                    let this = Sel::Txn(id);
                    row.set_selected(self.sel == this);
                    let t = &self.data.txns[id];
                    let mut clicked = false;
                    let mut cells = |row: &mut egui_extras::TableRow<'_, '_>, h: &mut Harness, name: &str, f: &mut dyn FnMut(&mut Ui)| {
                        let (_, r) = row.col(|ui| {
                            f(ui);
                            grid_lines(ui);
                        });
                        h.track(&r, format!("{name} cell, row {pos}"));
                        clicked |= r.clicked();
                    };
                    cells(&mut row, &mut self.h, "date", &mut |ui| cell_label(ui, fmt_day(t.day)));
                    cells(&mut row, &mut self.h, "payee", &mut |ui| cell_label(ui, &self.data.payees[t.payee as usize]));
                    cells(&mut row, &mut self.h, "category", &mut |ui| {
                        if t.split.is_some() {
                            if ui.link("[Split]").clicked() {
                                acts.push(Act::Split(id));
                            }
                        } else {
                            cell_label(ui, &self.data.categories[t.category as usize]);
                        }
                    });
                    cells(&mut row, &mut self.h, "memo", &mut |ui| cell_label(ui, RichText::new(&t.memo).weak()));
                    cells(&mut row, &mut self.h, "amount", &mut |ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| money_label(ui, t.amount));
                    });
                    if balance {
                        let bal = self.view_bal[pos];
                        cells(&mut row, &mut self.h, "balance", &mut |ui| {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| money_label(ui, bal));
                        });
                    }
                    let r = row.response();
                    r.context_menu(|ui| {
                        for label in ["Edit", "Split…", "Mark Cleared", "Duplicate", "Delete"] {
                            if ui.button(label).clicked() {
                                self.h.event("command", json!({ "would_emit": format!("DomainCommand::{}{{ id: {id} }}", label.replace(['…', ' '], "")), "source": "register row menu" }));
                                if label == "Edit" {
                                    acts.push(Act::Select(this));
                                }
                                if label == "Split…" {
                                    acts.push(Act::Split(id));
                                }
                                ui.close();
                            }
                        }
                    });
                    if clicked || r.clicked() {
                        acts.push(Act::Select(this));
                    }
                });
            });
        self.h.scroll("register", out.state.offset.y, row_h + 3.0);
        self.apply_acts(acts);
    }
}

// ---------------------------------------------------------------------------------------------
// Variant C: spreadsheet cells. Arrows move, typing starts an edit that replaces the cell.

const C_TITLES: [&str; 6] = ["Date", "Payee", "Category", "Memo", "Amount", "Balance"];

/// The untitled last column: an Actions menu button per row, the column chooser in its header.
const ACTIONS_WIDTH: f32 = 88.0;

const ROW_ACTIONS: &[&str] = &[
    "Edit",
    "Split…",
    "Mark Cleared",
    "Memorize Payee",
    "Copy Transaction",
    "Duplicate",
    "Void",
    "Delete",
];

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

fn c_column(kind: usize) -> Column {
    match kind {
        // Room for the date and, while editing, the picker button.
        0 => Column::exact(112.0),
        1 => Column::remainder().at_least(120.0).clip(true),
        2 => Column::remainder().at_least(100.0).clip(true),
        3 => Column::initial(170.0).at_least(80.0).clip(true),
        _ => Column::exact(100.0),
    }
}

/// The column chooser: show or hide each column (Date and Amount stay) and move it up or down.
/// Returns the new (order, hidden, balance shown) when something changed.
fn column_chooser(
    ui: &mut Ui,
    h: &mut Harness,
    cols: &[usize],
    hidden: &[bool; 6],
    balance: bool,
) -> Option<(Vec<usize>, [bool; 6], bool)> {
    ui.set_min_width(220.0);
    let mut order = cols.to_vec();
    let mut hid = *hidden;
    let mut bal = balance;
    let mut changed = false;
    let mut swap = None;
    ui.label(RichText::new("Columns").strong());
    ui.label(
        RichText::new("One layout for every register, for now")
            .small()
            .weak(),
    );
    for (i, &k) in cols.iter().enumerate() {
        ui.horizontal(|ui| {
            let required = k == 0 || k == 4;
            let mut shown = if k == 5 { bal } else { !hid[k] };
            let r = ui.add_enabled(!required, egui::Checkbox::new(&mut shown, C_TITLES[k]));
            h.track(&r, format!("columns: {}", C_TITLES[k]));
            if r.changed() {
                if k == 5 {
                    bal = shown;
                } else {
                    hid[k] = !shown;
                }
                changed = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let down = ui.add_enabled(i + 1 < cols.len(), egui::Button::new("⏷").small());
                h.track(&down, format!("columns: move {} down", C_TITLES[k]));
                if down.clicked() {
                    swap = Some((i, i + 1));
                }
                let up = ui.add_enabled(i > 0, egui::Button::new("⏶").small());
                h.track(&up, format!("columns: move {} up", C_TITLES[k]));
                if up.clicked() {
                    swap = Some((i - 1, i));
                }
            });
        });
    }
    if let Some((a, b)) = swap {
        order.swap(a, b);
        changed = true;
    }
    changed.then_some((order, hid, bal))
}

/// A month calendar under (or above) the date cell. Returns the day clicked, and whether the
/// month changed (so the cell editor takes the focus back).
fn date_picker(
    ui: &Ui,
    h: &mut Harness,
    anchor: Rect,
    month: &mut i64,
    selected: Option<i64>,
) -> (Option<i64>, bool) {
    let (y, m, _) = civil(*month);
    let mut picked = None;
    let mut nav = false;
    let row = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
    let est = 9.0 * row + 24.0;
    let up = anchor.bottom() + est > ui.ctx().content_rect().bottom();
    let area = egui::Area::new(Id::new("date_picker")).order(egui::Order::Foreground);
    let area = if up {
        area.pivot(egui::Align2::LEFT_BOTTOM)
            .fixed_pos(anchor.left_top())
    } else {
        area.fixed_pos(anchor.left_bottom())
    };
    let shown = area.show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                let prev = ui.small_button("⏴");
                h.track(&prev, "date picker: previous month");
                if prev.clicked() {
                    let (py, pm) = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
                    *month = days_from_civil(py, pm, 1);
                    nav = true;
                }
                ui.strong(format!("{} {y}", MONTHS[(m - 1) as usize]));
                let next = ui.small_button("⏵");
                h.track(&next, "date picker: next month");
                if next.clicked() {
                    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
                    *month = days_from_civil(ny, nm, 1);
                    nav = true;
                }
            });
            egui::Grid::new("date_picker_days")
                .spacing([2.0, 2.0])
                .show(ui, |ui| {
                    for d in ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"] {
                        ui.label(RichText::new(d).small().weak());
                    }
                    ui.end_row();
                    let first = days_from_civil(y, m, 1);
                    // 1970-01-01 was a Thursday; Sunday is column 0.
                    let lead = (first + 4).rem_euclid(7);
                    for _ in 0..lead {
                        ui.label("");
                    }
                    let mut col = lead;
                    for d in 1..=days_in_month(y, m) {
                        let day = first + d - 1;
                        let mut text = RichText::new(format!("{d:>2}"));
                        if day == TODAY {
                            text = text.strong();
                        }
                        let b = ui.selectable_label(selected == Some(day), text);
                        h.track(&b, format!("date picker: {}", fmt_day(day)));
                        let pressed = b.hovered() && ui.input(|i| i.pointer.primary_pressed());
                        if pressed || b.clicked() {
                            picked = Some(day);
                        }
                        col += 1;
                        if col == 7 {
                            ui.end_row();
                            col = 0;
                        }
                    }
                });
            let t = ui.small_button("Today");
            h.track(&t, "date picker: today");
            if t.hovered() && ui.input(|i| i.pointer.primary_pressed()) {
                picked = Some(TODAY);
            }
        });
    });
    h.region(shown.response.rect, "date picker");
    (picked, nav)
}

impl Proto {
    fn variant_c(&mut self, ui: &mut Ui, _keys: Keys) {
        self.status_bar(ui);
        let r = CentralPanel::default().show(ui, |ui| {
            ScrollArea::horizontal().show(ui, |ui| self.grid_c(ui))
        });
        self.h.region(r.response.rect, "register");
    }

    fn grid_c(&mut self, ui: &mut Ui) {
        let row_h = self.row_height(1);
        let n = self.order.len();
        set_grid(ui, &self.theme);
        let vis = self.vis();
        if !vis.contains(&self.col) {
            self.col = vis[0];
        }
        let sort = self.sort;
        let popup = self.t.ac_popup;
        let keys = self.take_keys_snapshot();
        let mut acts: Vec<Act> = Vec::new();
        let mut col_change = None;
        let mut row_action: Option<(usize, &'static str)> = None;
        // A keyboard move scrolls in the same frame, with no animation, unless toggle 1 asks for
        // egui's own scroll_to_row (animated, applied a frame late) to compare.
        let pitch = row_h + ui.spacing().item_spacing.y;
        let same_frame = !self.t.scroll_animated && self.key_scroll.is_some();
        if same_frame {
            self.jump = None;
        }
        // The id follows the column set, so egui_extras doesn't reuse another layout's widths.
        let mut builder = self
            .table(ui, &format!("register_c_{vis:?}"))
            .animate_scrolling(self.t.scroll_animated);
        if let Some(p) = self.key_scroll.take()
            && same_frame
        {
            let top = p as f32 * pitch;
            let bottom = top + pitch;
            if top < self.scroll_y {
                self.scroll_y = top;
            } else if self.view_h > 0.0 && bottom > self.scroll_y + self.view_h {
                self.scroll_y = bottom - self.view_h;
            }
            builder = builder.vertical_scroll_offset(self.scroll_y);
        }
        for &k in &vis {
            builder = builder.column(c_column(k));
        }
        builder = builder.column(Column::exact(ACTIONS_WIDTH));
        self.jump = None;
        let out = builder
            .header(row_h + 2.0, |mut hd| {
                for &k in &vis {
                    hd.col(|ui| {
                        header_band(ui);
                        let title = C_TITLES[k];
                        match k {
                            0 => sort_header(ui, &mut self.h, sort, SortCol::Date, title, &mut acts),
                            1 => sort_header(ui, &mut self.h, sort, SortCol::Payee, title, &mut acts),
                            2 => sort_header(ui, &mut self.h, sort, SortCol::Category, title, &mut acts),
                            4 => sort_header(ui, &mut self.h, sort, SortCol::Amount, title, &mut acts),
                            5 => money_header(ui, title),
                            _ => {
                                ui.strong(title);
                            }
                        }
                    });
                }
                // The Actions column has no title; its header holds the column chooser.
                hd.col(|ui| {
                    header_band(ui);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(Theme::EDGE_INSET);
                        let b = ui.small_button("⚙").on_hover_text("Show, hide and order columns");
                        self.h.track(&b, "header: columns button");
                        let (cols, hidden, bal) = (self.cols.clone(), self.hidden, self.t.running_balance);
                        egui::Popup::menu(&b)
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .show(|ui| {
                                if let Some(c) = column_chooser(ui, &mut self.h, &cols, &hidden, bal) {
                                    col_change = Some(c);
                                }
                            });
                    });
                });
            })
            .body(|body| {
                body.rows(row_h, n + 1, |mut row| {
                    let pos = row.index();
                    let this = if pos < n {
                        Sel::Txn(self.order[pos])
                    } else {
                        Sel::New
                    };
                    row.set_selected(self.sel == this);
                    for &col in &vis {
                        let cursor_here = self.sel == this && self.col == col;
                        let editing_here = self
                            .cell
                            .as_ref()
                            .is_some_and(|c| c.row == this && c.col == col);
                        let (_, r) = row.col(|ui| {
                            if editing_here {
                                let ce = self.cell.as_mut().unwrap();
                                let focus_req = std::mem::take(&mut ce.focus);
                                let mut focus = focus_req.then_some(Field::Cell);
                                let name = format!(
                                    "{} cell editor, row {pos}",
                                    C_TITLES[col].to_lowercase()
                                );
                                match col {
                                    0 => {
                                        let anchor = ui.max_rect();
                                        let button = ui
                                            .horizontal(|ui| {
                                                let w = (ui.available_width() - 22.0).max(40.0);
                                                ui.scope(|ui| {
                                                    ui.set_max_width(w);
                                                    field(ui, &mut self.h, Field::Cell, &mut ce.buf, &mut focus, name, false);
                                                });
                                                let b = ui.small_button("📅");
                                                self.h.track(&b, format!("date picker button, row {pos}"));
                                                b.clicked()
                                            })
                                            .inner;
                                        if button || keys.alt_down {
                                            self.picker = if button && self.picker.is_some() {
                                                None
                                            } else {
                                                let (y, m, _) = civil(parse_date(&ce.buf).unwrap_or(TODAY));
                                                Some(days_from_civil(y, m, 1))
                                            };
                                            self.h.event(
                                                "date_popup",
                                                json!({ "open": self.picker.is_some(), "via": if button { "button" } else { "alt+down" } }),
                                            );
                                            focus = Some(Field::Cell);
                                        }
                                        if let Some(month) = &mut self.picker {
                                            let (picked, nav) = date_picker(ui, &mut self.h, anchor, month, parse_date(&ce.buf));
                                            if nav {
                                                focus = Some(Field::Cell);
                                            }
                                            if let Some(d) = picked {
                                                self.h.event("date_pick", json!({ "day": fmt_day(d), "row": pos }));
                                                ce.buf = fmt_day(d);
                                                self.picker = None;
                                                // Picking commits the cell and moves on, as Tab does.
                                                self.ac.clicked = true;
                                            }
                                        }
                                    }
                                    1 => {
                                        let (_, picked) = combo_edit(
                                            ui, &mut self.h, &self.data.payees, 0, "payee", &mut self.ac, popup,
                                            &keys, &mut ce.buf, &mut focus, name, Field::Cell, true, false,
                                        );
                                        // Picking a payee on the draft row fills its usual category.
                                        if let Some(p) = picked
                                            && this == Sel::New
                                            && self.draft.category.trim().is_empty()
                                        {
                                            self.draft.category = self.data.categories
                                                [self.data.payee_category[p] as usize]
                                                .clone();
                                        }
                                    }
                                    2 => {
                                        combo_edit(
                                            ui, &mut self.h, &self.data.categories, 1, "category", &mut self.ac,
                                            popup, &keys, &mut ce.buf, &mut focus, name, Field::Cell, true, true,
                                        );
                                    }
                                    _ => {
                                        field(ui, &mut self.h, Field::Cell, &mut ce.buf, &mut focus, name, col == 4);
                                    }
                                }
                                if focus.is_some() {
                                    ce.focus = true;
                                }
                            } else {
                                let mut drawn = |ui: &mut Ui| match (this, col) {
                                    (Sel::Txn(id), c) => {
                                        let t = &self.data.txns[id];
                                        match c {
                                            0 => cell_label(ui, fmt_day(t.day)),
                                            1 => cell_label(ui, &self.data.payees[t.payee as usize]),
                                            2 if t.split.is_some() => {
                                                if ui.link("[Split]").clicked() {
                                                    acts.push(Act::Split(id));
                                                }
                                            }
                                            2 => cell_label(ui, &self.data.categories[t.category as usize]),
                                            3 => cell_label(ui, RichText::new(&t.memo).weak()),
                                            4 => {
                                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                    money_label(ui, t.amount)
                                                });
                                            }
                                            _ => {
                                                let b = self.view_bal[pos];
                                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                    money_label(ui, b)
                                                });
                                            }
                                        }
                                    }
                                    (_, c) => {
                                        let text = self.cell_text(this, c);
                                        if c == 1 && self.draft.payee.is_empty() {
                                            ui.label(RichText::new("type to add a transaction").weak().italics());
                                        } else if c == 4 || c == 5 {
                                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                ui.label(RichText::new(text).weak());
                                            });
                                        } else {
                                            cell_label(ui, RichText::new(text).weak());
                                        }
                                    }
                                };
                                drawn(ui);
                            }
                            grid_lines(ui);
                            if cursor_here {
                                let stroke = egui::Stroke::new(2.0, ui.visuals().selection.stroke.color);
                                ui.painter().rect_stroke(
                                    ui.max_rect().shrink(0.5),
                                    0.0,
                                    stroke,
                                    egui::StrokeKind::Inside,
                                );
                            }
                        });
                        self.h.track(&r, format!("{} cell, row {pos}", C_TITLES[col].to_lowercase()));
                        if !editing_here {
                            if r.double_clicked() {
                                acts.push(Act::CellEdit(this, col));
                            } else if r.clicked() {
                                acts.push(Act::Cell(this, col));
                            }
                        }
                    }
                    // Actions: a menu button on the selected row and the row under the pointer.
                    row.col(|ui| {
                        grid_lines(ui);
                        let Sel::Txn(id) = this else { return };
                        let rect = ui.max_rect();
                        let hovered = ui
                            .ctx()
                            .pointer_hover_pos()
                            .is_some_and(|p| rect.y_range().contains(p.y) && p.x <= ui.clip_rect().right());
                        if self.sel != this && !hovered {
                            return;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add_space(Theme::EDGE_INSET);
                            let m = ui.menu_button("Actions ⏷", |ui| {
                                for &label in ROW_ACTIONS {
                                    let b = ui.button(label);
                                    self.h.track(&b, format!("actions: {label}, row {pos}"));
                                    if b.clicked() {
                                        row_action = Some((id, label));
                                        ui.close();
                                    }
                                }
                            });
                            self.h.track(&m.response, format!("actions button, row {pos}"));
                        });
                    });
                });
            });
        self.h.scroll("register", out.state.offset.y, pitch);
        self.scroll_y = out.state.offset.y;
        self.view_h = out.inner_rect.height();
        self.apply_acts(acts);
        if let Some((order, hidden, bal)) = col_change {
            self.cols = order;
            self.hidden = hidden;
            self.t.running_balance = bal;
            let names: Vec<&str> = self.vis().iter().map(|k| C_TITLES[*k]).collect();
            let hid: Vec<&str> = self
                .cols
                .iter()
                .filter(|k| !self.vis().contains(k))
                .map(|k| C_TITLES[*k])
                .collect();
            self.h.event(
                "columns",
                json!({ "shown": names, "hidden": hid,
                        "would_emit": format!("AppCommand::SetPreference(RegisterColumns {{ shown: {names:?}, hidden: {hid:?} }})") }),
            );
            if let Some(p) = self.pos(self.sel) {
                self.jump = Some((p, Some(Align::Center)));
            }
            self.h.screenshot("columns");
        }
        if let Some((id, label)) = row_action {
            let would = match label {
                "Edit" => format!("start a cell edit on transaction {id}"),
                "Split…" => format!("AppCommand::OpenSplit {{ id: {id} }}"),
                "Copy Transaction" => format!("AppCommand::CopyTransaction {{ id: {id} }}"),
                other => format!("DomainCommand::{} {{ id: {id} }}", other.replace(' ', "")),
            };
            self.h.event(
                "command",
                json!({ "would_emit": would, "source": "actions menu", "action": label }),
            );
            match label {
                "Edit" => {
                    let first = self.editable().into_iter().find(|k| *k == 1).unwrap_or(1);
                    self.sel = Sel::Txn(id);
                    self.col = first;
                    self.start_cell_edit(Sel::Txn(id), first, None, "actions menu");
                }
                "Split…" => self.open_split(id, "actions menu"),
                _ => self.status = format!("Would {label}: transaction {id}"),
            }
        }
    }

    /// The keys consumed at the top of the frame are not kept; the payee field only needs the
    /// autocomplete ones, which it reads from this snapshot of the focused-field state.
    fn take_keys_snapshot(&self) -> Keys {
        self.c_keys
    }
}

// ---------------------------------------------------------------------------------------------
// Prototype tooling is docked as the outermost right-hand panel: each variant lays out in the
// rest of the window, so the dock never covers the design. F4 collapses it to a thin edge.

impl Proto {
    fn set_dock(&mut self, open: bool, via: &str) {
        self.dock_open = open;
        self.h.event("dock", json!({ "open": open, "via": via }));
    }

    fn dock(&mut self, ui: &mut Ui) {
        let frame = {
            let v = ui.visuals();
            egui::Frame::side_top_panel(ui.style())
                .fill(v.warn_fg_color.gamma_multiply(0.10).blend(v.panel_fill))
                .stroke(egui::Stroke::new(1.0, v.warn_fg_color))
        };
        let ctx = ui.ctx().clone();
        if !self.dock_open {
            Panel::right("prototype_dock_edge")
                .exact_size(22.0)
                .resizable(false)
                .frame(frame)
                .show(ui, |ui| {
                    let r = ui
                        .small_button("⏴")
                        .on_hover_text("Show prototype controls (F4)");
                    self.h.track(&r, "dock: expand");
                    if r.clicked() {
                        self.set_dock(true, "click");
                    }
                });
            return;
        }
        Panel::right("prototype_dock")
            .exact_size(290.0)
            .resizable(false)
            .frame(frame)
            .show(ui, |ui| {
                Sides::new().show(
                    ui,
                    |ui| ui.label(RichText::new("PROTOTYPE").strong()),
                    |ui| {
                        let r = ui.small_button("⏵").on_hover_text("Collapse (F4)");
                        self.h.track(&r, "dock: collapse");
                        if r.clicked() {
                            self.set_dock(false, "click");
                        }
                    },
                );
                ui.separator();
                ui.label(RichText::new("Variant").strong());
                for (v, (_, hint)) in Variant::ALL.into_iter().zip(VARIANT_KEYS) {
                    let r = ui.selectable_label(self.variant == v, format!("{hint}   {}", v.label()));
                    self.h.track(&r, format!("dock: variant {}", v.key()));
                    if r.clicked() {
                        self.set_variant(&ctx, v, "click");
                    }
                }
                ui.separator();
                ui.label(RichText::new("Toggles").strong());
                let current = self.variant.key().to_uppercase();
                for (&(key, label, applies), (_, hint)) in TOGGLES.iter().zip(TOGGLE_KEYS) {
                    let relevant = applies == "all" || applies.split(' ').any(|a| a == current);
                    if key == "gridlines" {
                        let text = format!("{hint}   [{applies}] Gridlines: {}", gridlines_name(self.t.gridlines));
                        let r = ui.selectable_label(false, text);
                        self.h.track(&r, "dock: toggle gridlines");
                        if r.clicked() {
                            self.set_gridlines(None, "click");
                        }
                        continue;
                    }
                    let mut value = *self.t.get_mut(key).unwrap();
                    let text = RichText::new(format!("{hint}   [{applies}] {label}"));
                    let r = ui.checkbox(&mut value, if relevant { text } else { text.weak() });
                    self.h.track(&r, format!("dock: toggle {key}"));
                    if r.changed() {
                        self.set_toggle(key, Some(value), "click");
                    }
                }
                ui.separator();
                self.h.dock_spotlight(ui);
                ui.separator();
                ui.label(
                    RichText::new(
                        "A B C or ◀ / ▶ variant · 1–0 toggles · F2 note · F3 marker · F4 hide this dock · F5 spotlight.\nIn C the grid owns the keyboard: hold Alt for the variant and toggle keys.",
                    )
                    .small()
                    .weak(),
                );
            });
    }
}

fn main() -> eframe::Result {
    if std::env::args().any(|a| a == "--stats") {
        let data = generate();
        let splits = data.txns.iter().filter(|t| t.split.is_some()).count();
        let min = data.txns.iter().map(|t| t.balance).min().unwrap_or(0);
        let max = data.txns.iter().map(|t| t.balance).max().unwrap_or(0);
        println!(
            "{} txns, {} split, {} payees, {} categories; opening {}, final {}, low {}, high {}",
            data.txns.len(),
            splits,
            data.payees.len(),
            data.categories.len(),
            fmt_cents(data.opening),
            fmt_cents(data.txns.last().map_or(0, |t| t.balance)),
            fmt_cents(min),
            fmt_cents(max)
        );
        for t in data.txns.iter().filter(|t| t.split.is_some()).take(3) {
            let sum: i64 = t.split.as_ref().map_or(0, |s| s.iter().map(|l| l.1).sum());
            println!(
                "split {} lines sum {} vs {}",
                t.split.as_ref().map_or(0, Vec::len),
                sum,
                t.amount
            );
        }
        return Ok(());
    }
    let args = Args::from_env();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Balanced — PROTOTYPE Register grid")
            .with_inner_size([1360.0, 820.0]),
        ..Default::default()
    };
    eframe::run_native(
        "balanced_register_prototype",
        options,
        Box::new(move |_cc| Ok(Box::new(Proto::new(&args)))),
    )
}
