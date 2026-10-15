//! PROTOTYPE, throwaway: money, date and category input fields.
//!
//! Question: how should money, date and category input fields behave? Locale-aware money entry
//! (parse/format per the money and i18n decisions, plus the calculator question #14 left open),
//! a date field with a picker and keyboard shortcuts, and a type-to-filter category combo box,
//! each tried in `en-US`, `de-DE` and the pseudo-locale `en-XA`.
//!
//! The fields sit in a mock Register: entered rows above, a draft row at the bottom, edited with
//! the keys the Register grid prototype settled (#17): Enter commits and moves right, Tab and
//! Shift+Tab commit and move, Esc cancels the cell, Ctrl+Enter or Enter on Amount adds the row.
//! The variants A–C differ in interaction model: A is typed text, B is assisted (calendar and
//! list buttons while editing, floating hints, a Create row), C is structured (date segments,
//! cents-first money). Every variant's category is the combobox list #17 settled.
//!
//! Run:
//!   cargo run -p balanced_ui --example input_fields_prototype -- \
//!     [--variant a|b|c] [--log <dir>/events.jsonl] [--control <dir>/control.txt]
//!
//! The PROTOTYPE dock on the right holds the variant switcher and toggles; F4 collapses it.
//! A–C or ← / → switch variant (while no field has focus), 1–0 flip toggles, F2 note, F3 marker,
//! F5 spotlight.
//!
//! Money parses and formats through `balanced_i18n::Localizer` and is held as
//! `balanced_core::Money`; dates display through the `Localizer` too. Typed date parsing, month
//! and weekday names and the prototype's own strings are hand-rolled. The look is
//! `balanced_ui::theme`. Fixed-seed stub data, a fixed "today" (2026-10-14). None of this is
//! production code.

#![allow(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::pedantic,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::wildcard_enum_match_arm,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::str_to_string,
    clippy::disallowed_methods,
    clippy::disallowed_macros,
    clippy::cast_precision_loss,
    clippy::let_underscore_must_use,
    clippy::iter_over_hash_type,
    clippy::needless_pass_by_ref_mut,
    clippy::use_self,
    clippy::manual_is_multiple_of,
    clippy::manual_clamp,
    clippy::collapsible_if,
    clippy::arithmetic_side_effects,
    missing_debug_implementations,
    unreachable_pub
)]

// The real theme module, compiled into the example: the prototype gets the decided look
// without making `theme` public.
#[allow(dead_code)]
#[path = "../../src/theme.rs"]
mod theme;

use std::collections::VecDeque;

use balanced_core::{CurrencyCode, Money, RoundingPolicy};
use balanced_i18n::{AmountParseError, AmountStyle, Currency, Localizer};
use balanced_prototype_harness::{Args, Harness};
use eframe::egui;
use egui::{
    Align, CentralPanel, Event, Id, Key, Modifiers, Panel, Rect, RichText, Sense, Sides, Ui, Vec2,
};
use icu_calendar::types::Weekday;
use icu_locale::{langid, locale};
use rust_decimal::Decimal;
use serde_json::json;
use theme::{Theme, ThemeSettings};

// ---------------------------------------------------------------------------------------------
// Dates (proleptic Gregorian, civil-from-days arithmetic)

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Date {
    y: i32,
    m: u32,
    d: u32,
}

/// The prototype's fixed "today", so every session starts from the same calendar.
const TODAY: Date = Date {
    y: 2026,
    m: 10,
    d: 14,
};

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

impl Date {
    fn valid(y: i32, m: u32, d: u32) -> Option<Self> {
        ((1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m) && (1900..=2200).contains(&y))
            .then_some(Self { y, m, d })
    }

    fn to_days(self) -> i64 {
        let y = i64::from(self.y) - i64::from(self.m <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (i64::from(self.m) + 9) % 12;
        let doy = (153 * mp + 2) / 5 + i64::from(self.d) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    fn from_days(z: i64) -> Self {
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        Self {
            y: (y + i64::from(m <= 2)) as i32,
            m,
            d,
        }
    }

    fn add_days(self, n: i64) -> Self {
        Self::from_days(self.to_days() + n)
    }

    /// Monday = 0 … Sunday = 6.
    fn weekday(self) -> usize {
        (self.to_days() + 3).rem_euclid(7) as usize
    }

    fn add_months(self, n: i32) -> Self {
        let total = self.y * 12 + self.m as i32 - 1 + n;
        let (y, m) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
        Self {
            y,
            m,
            d: self.d.min(days_in_month(y, m)),
        }
    }

    fn icu(self) -> icu_calendar::Date<icu_calendar::Iso> {
        icu_calendar::Date::try_new_iso(self.y, self.m as u8, self.d as u8).unwrap()
    }

    /// For the log and the would-emit Commands: locale-free.
    fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.y, self.m, self.d)
    }
}

// ---------------------------------------------------------------------------------------------
// Locales: `balanced_i18n` formats and parses. The prototype keeps only what it has no API
// for: typed date parsing, month and weekday names, and its own strings.

#[derive(Clone, Copy, PartialEq, Eq)]
enum Loc {
    EnUs,
    DeDe,
    /// The pseudo-locale: `en-US` text through a transform, formatting as `de-DE`.
    EnXa,
}

const LOCS: [Loc; 3] = [Loc::EnUs, Loc::DeDe, Loc::EnXa];

impl Loc {
    fn tag(self) -> &'static str {
        match self {
            Self::EnUs => "en-US",
            Self::DeDe => "de-DE",
            Self::EnXa => "en-XA",
        }
    }
}

/// A locale's `Localizer`, plus what the prototype reads off it (marks, date order) and the
/// names it has no API for.
struct Fmt {
    l10n: Localizer,
    dec: char,
    grp: char,
    mdy: bool,
    sep: char,
    sunday_first: bool,
    pattern: &'static str,
    months: [&'static str; 12],
    days: [&'static str; 7],
    day_names: [&'static str; 7],
}

const EN_MONTHS: [&str; 12] = [
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
const DE_MONTHS: [&str; 12] = [
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
];
const EN_DAYS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
const DE_DAYS: [&str; 7] = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];
const EN_DAY_NAMES: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const DE_DAY_NAMES: [&str; 7] = [
    "Montag",
    "Dienstag",
    "Mittwoch",
    "Donnerstag",
    "Freitag",
    "Samstag",
    "Sonntag",
];

impl Fmt {
    fn new(loc: Loc) -> Self {
        let l10n = match loc {
            Loc::EnUs => Localizer::new(&langid!("en-US"), &locale!("en-US")),
            Loc::DeDe => Localizer::new(&langid!("de-DE"), &locale!("de-DE")),
            Loc::EnXa => Localizer::pseudo(),
        }
        .expect("the shipped locales load");
        // The marks and the date order come from the formatter, not from a table.
        let probe = l10n.format_amount(
            Decimal::new(123_450, 2),
            Currency::new("USD", 2).unwrap(),
            AmountStyle::Bare,
        );
        let grp = probe.chars().nth(1).unwrap();
        let dec = probe.chars().rev().nth(2).unwrap();
        let probe_date = l10n.format_date(
            Date {
                y: 2026,
                m: 10,
                d: 5,
            }
            .icu(),
        );
        let mdy = probe_date.trim_start_matches('0').starts_with("10");
        let sep = probe_date
            .chars()
            .find(|c| !c.is_ascii_digit())
            .unwrap_or('/');
        let sunday_first = l10n.first_weekday() == Weekday::Sunday;
        let english = loc == Loc::EnUs;
        Self {
            l10n,
            dec,
            grp,
            mdy,
            sep,
            sunday_first,
            pattern: if english { "M/D/YYYY" } else { "TT.MM.JJJJ" },
            months: if english { EN_MONTHS } else { DE_MONTHS },
            days: if english { EN_DAYS } else { DE_DAYS },
            day_names: if english { EN_DAY_NAMES } else { DE_DAY_NAMES },
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Currencies and money

const CURRENCIES: [&str; 4] = ["USD", "EUR", "JPY", "BHD"];

fn code_of(i: usize) -> CurrencyCode {
    CurrencyCode::new(CURRENCIES[i]).unwrap()
}

/// The formatter's view of a currency.
fn i18n_currency(code: CurrencyCode) -> Currency {
    Currency::new(code.as_str(), code.decimal_places()).unwrap()
}

/// How a calculator result with more decimal places than the currency is handled.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Rounding {
    Reject,
    Bankers,
    HalfAway,
}

impl Rounding {
    const ALL: [Self; 3] = [Self::Reject, Self::Bankers, Self::HalfAway];

    fn name(self) -> &'static str {
        match self {
            Self::Reject => "reject",
            Self::Bankers => "banker's (half to even)",
            Self::HalfAway => "half away from zero",
        }
    }

    fn policy(self) -> Option<RoundingPolicy> {
        match self {
            Self::Reject => None,
            Self::Bankers => Some(RoundingPolicy::BankersEven),
            Self::HalfAway => Some(RoundingPolicy::HalfAwayFromZero),
        }
    }
}

fn looks_like_expression(s: &str) -> bool {
    let t = s.trim();
    t.contains(['+', '*', '/'])
        || t.char_indices().any(|(i, c)| {
            (c == '-' || c == '−') && i > 0 && !t[i + c.len_utf8()..].trim().is_empty()
        })
}

/// `+ - * /` with the usual precedence and unary minus. Each operand is read by the locale's
/// amount parser, allowing up to ten decimal places (`0.075` is a rate, not money).
fn eval_expression(s: &str, l10n: &Localizer) -> Result<Decimal, AmountParseError> {
    let mut nums: Vec<Decimal> = Vec::new();
    let mut ops: Vec<char> = Vec::new();
    let mut tok = String::new();
    let mut expect_operand = true;
    let flush = |tok: &mut String, nums: &mut Vec<Decimal>| -> Result<(), AmountParseError> {
        if tok.trim().is_empty() {
            return Err(AmountParseError::Invalid);
        }
        nums.push(l10n.parse_amount(tok, 10)?.normalize());
        tok.clear();
        Ok(())
    };
    for c in s.chars() {
        let is_op = matches!(c, '+' | '*' | '/' | '-' | '−');
        if is_op && !(expect_operand && tok.trim().is_empty() && matches!(c, '-' | '−')) {
            flush(&mut tok, &mut nums)?;
            ops.push(if c == '−' { '-' } else { c });
            expect_operand = true;
        } else {
            if !c.is_whitespace() {
                expect_operand = false;
            }
            tok.push(c);
        }
    }
    flush(&mut tok, &mut nums)?;
    // First pass: * and /
    let mut vals = vec![nums[0]];
    let mut adds: Vec<char> = Vec::new();
    for (op, n) in ops.iter().zip(nums.iter().skip(1)) {
        match op {
            '*' => {
                let last = vals.pop().unwrap();
                vals.push(last.checked_mul(*n).ok_or(AmountParseError::OutOfRange)?);
            }
            '/' => {
                let last = vals.pop().unwrap();
                vals.push(last.checked_div(*n).ok_or(AmountParseError::Invalid)?);
            }
            _ => {
                adds.push(*op);
                vals.push(*n);
            }
        }
    }
    let mut total = vals[0];
    for (op, v) in adds.iter().zip(vals.iter().skip(1)) {
        total = if *op == '+' { total + v } else { total - v };
    }
    Ok(total.normalize())
}

// ---------------------------------------------------------------------------------------------
// Date parsing

fn parse_date(input: &str, f: &Fmt, partial: bool, base: Date) -> Result<Option<Date>, ()> {
    let s = input.trim();
    if s.is_empty() {
        return Ok(None);
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        return match s.len() {
            8 => Date::valid(
                s[0..4].parse().map_err(|_| ())?,
                s[4..6].parse().map_err(|_| ())?,
                s[6..8].parse().map_err(|_| ())?,
            )
            .map(Some)
            .ok_or(()),
            1 | 2 if partial => Date::valid(base.y, base.m, s.parse().map_err(|_| ())?)
                .map(Some)
                .ok_or(()),
            _ => Err(()),
        };
    }
    let toks: Vec<&str> = s
        .split(|c: char| !c.is_ascii_digit())
        .filter(|t| !t.is_empty())
        .collect();
    if s.chars()
        .any(|c| !c.is_ascii_digit() && !"/.- ".contains(c))
    {
        return Err(());
    }
    let year = |t: &str| -> Result<i32, ()> {
        match t.len() {
            4 => t.parse().map_err(|_| ()),
            2 if partial => {
                let n: i32 = t.parse().map_err(|_| ())?;
                Ok(if n < 70 { 2000 + n } else { 1900 + n })
            }
            _ => Err(()),
        }
    };
    let num = |t: &str| t.parse::<u32>().map_err(|_| ());
    let date = match toks.len() {
        3 if toks[0].len() == 4 => Date::valid(year(toks[0])?, num(toks[1])?, num(toks[2])?),
        3 => {
            let (m, d) = if f.mdy {
                (num(toks[0])?, num(toks[1])?)
            } else {
                (num(toks[1])?, num(toks[0])?)
            };
            Date::valid(year(toks[2])?, m, d)
        }
        2 if partial => {
            let (m, d) = if f.mdy {
                (num(toks[0])?, num(toks[1])?)
            } else {
                (num(toks[1])?, num(toks[0])?)
            };
            Date::valid(base.y, m, d)
        }
        1 if partial => Date::valid(base.y, base.m, num(toks[0])?),
        _ => None,
    };
    date.map(Some).ok_or(())
}

fn fmt_date(d: Date, f: &Fmt) -> String {
    f.l10n.format_date(d.icu())
}

fn fmt_date_long(d: Date, f: &Fmt) -> String {
    let wd = f.day_names[d.weekday()];
    let month = f.months[d.m as usize - 1];
    if f.mdy {
        format!("{wd}, {month} {}, {}", d.d, d.y)
    } else {
        format!("{wd}, {}. {month} {}", d.d, d.y)
    }
}

// ---------------------------------------------------------------------------------------------
// Text: pseudo-locale transform and a seed set of `de-DE` strings (the rest falls back to English)

fn pseudo(s: &str) -> String {
    let mut out = String::from("[");
    let mut in_brace = false;
    let mut n = 0;
    for c in s.chars() {
        if c == '{' {
            in_brace = true;
        }
        out.push(if in_brace {
            c
        } else {
            match c {
                'a' => 'á',
                'e' => 'é',
                'i' => 'í',
                'o' => 'ó',
                'u' => 'ú',
                'c' => 'ç',
                'n' => 'ñ',
                'y' => 'ý',
                'A' => 'Á',
                'E' => 'É',
                'I' => 'Í',
                'O' => 'Ó',
                'U' => 'Ú',
                _ => c,
            }
        });
        if c == '}' {
            in_brace = false;
        }
        n += 1;
    }
    out.push(' ');
    out.extend(std::iter::repeat_n('~', n * 35 / 100));
    out.push(']');
    out
}

const DE_SEED: &[(&str, &str)] = &[
    ("Date", "Datum"),
    ("Payee", "Empfänger"),
    ("Category", "Kategorie"),
    ("Amount", "Betrag"),
    ("Today", "Heute"),
];

// ---------------------------------------------------------------------------------------------
// Categories: a fixed tree, usage counts from a fixed-seed LCG

const SEED: u64 = 0x00ba_1a9c_ed18;

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
}

const CAT_TREE: &[(&str, &[&str])] = &[
    (
        "Auto",
        &[
            "Fuel",
            "Insurance",
            "Loan Payment",
            "Maintenance",
            "Parking",
            "Registration",
            "Service",
            "Tolls",
        ],
    ),
    (
        "Bills & Utilities",
        &[
            "Electricity",
            "Gas",
            "Internet",
            "Mobile Phone",
            "Water",
            "Trash",
            "Streaming",
        ],
    ),
    (
        "Childcare & Education",
        &[
            "Books",
            "Childcare",
            "School Fees",
            "School Supplies",
            "Tuition",
        ],
    ),
    (
        "Entertainment",
        &[
            "Books",
            "Concerts",
            "Games",
            "Hobbies",
            "Movies",
            "Music",
            "Sports Events",
        ],
    ),
    (
        "Food & Dining",
        &[
            "Alcohol",
            "Coffee Shops",
            "Fast Food",
            "Groceries",
            "Restaurants",
            "Takeaway",
        ],
    ),
    ("Gifts & Donations", &["Charity", "Gifts", "Holiday"]),
    (
        "Health",
        &[
            "Dental",
            "Doctor",
            "Eyecare",
            "Gym",
            "Health Insurance",
            "Pharmacy",
            "Therapy",
        ],
    ),
    (
        "Home",
        &[
            "Furnishings",
            "Garden",
            "Improvement",
            "Mortgage",
            "Property Tax",
            "Rent",
            "Repairs",
            "Security",
        ],
    ),
    (
        "Income",
        &[
            "Bonus",
            "Dividends",
            "Interest",
            "Other Income",
            "Refund",
            "Salary",
            "Side Work",
        ],
    ),
    (
        "Personal Care",
        &["Clothing", "Cosmetics", "Hair", "Laundry"],
    ),
    ("Pets", &["Food", "Grooming", "Supplies", "Vet"]),
    (
        "Shopping",
        &[
            "Electronics",
            "General",
            "Hardware",
            "Office Supplies",
            "Online",
        ],
    ),
    ("Taxes", &["Federal", "Preparation", "Property", "State"]),
    (
        "Transfer",
        &[
            "Emergency Fund",
            "High-Yield Savings",
            "Joint Checking",
            "Visa Rewards",
        ],
    ),
    (
        "Travel",
        &[
            "Airfare",
            "Hotel",
            "Public Transit",
            "Rental Car",
            "Taxi & Rideshare",
            "Vacation",
        ],
    ),
];

#[derive(Clone)]
struct Cat {
    path: String,
    uses: u32,
    last_used: u32,
}

fn generate_cats() -> Vec<Cat> {
    let mut rng = Lcg(SEED);
    let mut cats: Vec<Cat> = Vec::new();
    for (p, _) in CAT_TREE {
        cats.push(Cat {
            path: (*p).to_owned(),
            uses: (rng.next() % 40) as u32,
            last_used: 0,
        });
    }
    for (p, kids) in CAT_TREE {
        for k in *kids {
            cats.push(Cat {
                path: format!("{p}:{k}"),
                uses: (rng.next() % rng.next().max(1).min(400)) as u32 % 400,
                last_used: 0,
            });
        }
    }
    // The most recently used few, so "recent first" differs visibly from A–Z.
    let recents = [
        "Auto:Fuel",
        "Food & Dining:Restaurants",
        "Bills & Utilities:Electricity",
        "Home:Rent",
        "Entertainment:Movies",
    ];
    let mut clock = 0;
    for r in recents.iter().rev() {
        if let Some(c) = cats.iter_mut().find(|c| c.path == *r) {
            clock += 1;
            c.last_used = clock;
            c.uses += 150;
        }
    }
    // Groceries is the most recent of all.
    if let Some(c) = cats
        .iter_mut()
        .find(|c| c.path == "Food & Dining:Groceries")
    {
        c.last_used = clock + 1;
        c.uses += 300;
    }
    cats
}

type Matched = (usize, Vec<usize>);

#[derive(Clone, Copy, PartialEq, Eq)]
enum MatchMode {
    Prefix,
    Substring,
    Fuzzy,
}

impl MatchMode {
    const ALL: [Self; 3] = [Self::Prefix, Self::Substring, Self::Fuzzy];

    fn name(self) -> &'static str {
        match self {
            Self::Prefix => "prefix of any segment",
            Self::Substring => "substring",
            Self::Fuzzy => "fuzzy (in order)",
        }
    }
}

/// Score (lower is better) and matched character indices into `label`.
fn match_label(mode: MatchMode, q: &str, label: &str) -> Option<(u32, Vec<usize>)> {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return Some((0, Vec::new()));
    }
    let chars: Vec<char> = label.to_lowercase().chars().collect();
    let qc: Vec<char> = q.chars().collect();
    match mode {
        MatchMode::Substring => {
            let pos = (0..chars.len()).find(|&i| chars[i..].starts_with(&qc))?;
            Some((pos as u32, (pos..pos + qc.len()).collect()))
        }
        MatchMode::Fuzzy => {
            let mut idx = Vec::new();
            let mut at = 0;
            for c in qc.iter().filter(|c| !c.is_whitespace()) {
                let p = (at..chars.len()).find(|&i| chars[i] == *c)?;
                idx.push(p);
                at = p + 1;
            }
            let span = idx.last()? - idx.first()? + 1;
            Some(((span - idx.len()) as u32 * 2 + idx[0] as u32, idx))
        }
        MatchMode::Prefix => {
            let qs: Vec<Vec<char>> = q.split(':').map(|s| s.trim().chars().collect()).collect();
            let mut starts = vec![0];
            for (i, c) in chars.iter().enumerate() {
                if *c == ':' {
                    starts.push(i + 1);
                }
            }
            'seg: for (j, &start) in starts.iter().enumerate() {
                let mut idx = Vec::new();
                for (k, part) in qs.iter().enumerate() {
                    let Some(&s) = starts.get(j + k) else {
                        continue 'seg;
                    };
                    if !chars[s..].starts_with(part) {
                        continue 'seg;
                    }
                    idx.extend(s..s + part.len());
                }
                return Some((start as u32 / 4 + j as u32, idx));
            }
            None
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Variants and toggles

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
            Self::A => "A · Typed: free text, inline hints",
            Self::B => "B · Assisted: buttons while editing, floating hints, Create row",
            Self::C => "C · Structured: date segments, cents-first money",
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

/// The toggles that cycle through values rather than flip on and off.
const CYCLING: [&str; 4] = ["locale", "currency", "match", "rounding"];

/// (key, label, variants it applies to).
const TOGGLES: &[(&str, &str, &str)] = &[
    ("locale", "Locale", "all"),
    ("currency", "Account currency", "all"),
    ("calc", "Calculator in the amount field", "A B"),
    ("preview", "Live \"reads as\" preview", "A B"),
    ("partial", "Partial dates (5, 3/5, 5.3.)", "A B"),
    (
        "shortcuts",
        "Date shortcuts (+ - t m h y r, PgUp/PgDn)",
        "all",
    ),
    ("picker", "Calendar picker (button, Alt+Down)", "all"),
    ("match", "Category match", "all"),
    ("rank", "Category order: recent & frequent first", "all"),
    ("rounding", "Calculator result rounding", "A B"),
];

struct Toggles {
    locale: usize,
    currency: usize,
    calc: bool,
    preview: bool,
    partial: bool,
    shortcuts: bool,
    picker: bool,
    match_mode: usize,
    rank: bool,
    rounding: usize,
}

impl Toggles {
    fn flag(&mut self, key: &str) -> Option<&mut bool> {
        Some(match key {
            "calc" => &mut self.calc,
            "preview" => &mut self.preview,
            "partial" => &mut self.partial,
            "shortcuts" => &mut self.shortcuts,
            "picker" => &mut self.picker,
            "rank" => &mut self.rank,
            _ => return None,
        })
    }

    fn describe(&mut self, key: &str) -> serde_json::Value {
        match key {
            "locale" => json!(LOCS[self.locale].tag()),
            "currency" => json!(CURRENCIES[self.currency]),
            "match" => json!(MatchMode::ALL[self.match_mode].name()),
            "rounding" => json!(Rounding::ALL[self.rounding].name()),
            _ => json!(self.flag(key).map(|v| *v)),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// App state

/// The draft row's cells, in Tab order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    Date,
    Payee,
    Category,
    Amount,
}

impl Field {
    const ALL: [Self; 4] = [Self::Date, Self::Payee, Self::Category, Self::Amount];

    fn name(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Payee => "payee",
            Self::Category => "category",
            Self::Amount => "amount",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.name() == name)
    }

    /// A fixed id, so `raw_input_hook` can tell that one of the cells has focus.
    fn id(self) -> Id {
        Id::new(("field", self.name()))
    }

    fn step(self, delta: isize) -> Self {
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0) as isize;
        Self::ALL[(i + delta).rem_euclid(Self::ALL.len() as isize) as usize]
    }
}

/// How the user leaves a cell. The keys are the Register's (#17).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Leave {
    /// Commit and move right; on Amount, commit and add the row.
    Enter,
    /// Commit and add the row from any cell.
    CtrlEnter,
    Tab,
    BackTab,
    /// Cancel the cell edit.
    Esc,
    /// Focus went elsewhere (a click, or the control file's `blur`): commit.
    Blur,
}

impl Leave {
    fn name(self) -> &'static str {
        match self {
            Self::Enter => "enter",
            Self::CtrlEnter => "ctrl+enter",
            Self::Tab => "tab",
            Self::BackTab => "shift+tab",
            Self::Esc => "esc",
            Self::Blur => "blur",
        }
    }
}

#[derive(Default)]
struct TextEd {
    buf: String,
    /// The user changed the text since the field gained focus.
    dirty: bool,
    error: Option<String>,
    sel_all: bool,
}

#[derive(Default)]
struct Combo {
    ed: TextEd,
    /// The text the user typed.
    typed: String,
    open: bool,
    /// The list shows every category, whatever was typed (⏷ or Alt+Down).
    all: bool,
    hl: Option<usize>,
    /// The highlight was moved with the arrow keys.
    moved: bool,
    popup: Option<Rect>,
    /// Variant B's ⏷ button, while editing.
    btn: Option<Rect>,
}

enum Outcome {
    Pick(usize),
    Create(String),
    Unknown(String),
    Clear,
}

struct Row {
    date: Option<Date>,
    payee: String,
    cat: Option<usize>,
    amount: Option<Money>,
}

struct Entered {
    date: Date,
    payee: String,
    cat: String,
    amount: Money,
}

#[derive(Clone, Copy)]
enum DateAct {
    Days(i64),
    Months(i32),
    Today,
    MonthStart,
    MonthEnd,
    YearStart,
    YearEnd,
}

struct Proto {
    h: Harness,
    theme: Theme,
    fmts: &'static [Fmt; 3],
    variant: Variant,
    t: Toggles,
    dock_open: bool,
    status: String,
    cats: Vec<Cat>,
    use_clock: u32,
    row: Row,
    /// Oldest first; the draft row sits under the last one.
    entered: Vec<Entered>,
    emitted: VecDeque<String>,
    date_ed: TextEd,
    money_ed: TextEd,
    payee_ed: TextEd,
    combo: Combo,
    /// Variant C date: day, month, year in logical order.
    segs: [Option<u32>; 3],
    seg: usize,
    seg_buf: String,
    seg_error: Option<String>,
    /// Variant C money: digits typed so far (cents-first) and the sign.
    cents: String,
    neg: bool,
    /// Calendar popup: the highlighted day.
    pick: Option<Date>,
    pick_rect: Option<Rect>,
    /// The calendar button, shown while the date cell is edited.
    cal_btn: Option<Rect>,
    pending_focus: Option<Field>,
    pending_blur: bool,
    /// Tab (false) or Shift+Tab (true), taken from the raw input while a cell has focus.
    pending_tab: Option<bool>,
    save_requested: bool,
}

fn select_range(ctx: &egui::Context, id: Id, from: usize, to: usize) {
    if let Some(mut st) = egui::TextEdit::load_state(ctx, id) {
        st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(from),
            egui::text::CCursor::new(to),
        )));
        st.store(ctx, id);
    }
}

fn over(ui: &Ui, rect: Option<Rect>) -> bool {
    rect.is_some_and(|r| ui.input(|i| i.pointer.hover_pos().is_some_and(|p| r.contains(p))))
}

fn union(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.union(b)),
        (a, b) => a.or(b),
    }
}

/// Picked on pointer press, not release: the press already moves focus off the field, and a
/// click would land after the list has closed (#17).
fn pressed(ui: &Ui, r: &egui::Response) -> bool {
    r.hovered() && ui.input(|i| i.pointer.primary_pressed())
}

/// Keys a cell keeps for itself: without `escape`, egui drops the focus on Esc before the
/// prototype sees the key (#17).
const CELL_KEYS: egui::EventFilter = egui::EventFilter {
    tab: true,
    horizontal_arrows: true,
    vertical_arrows: true,
    escape: true,
};

struct BoxOut {
    resp: egui::Response,
    lost: bool,
}

const fn draft_row() -> Row {
    Row {
        date: None,
        payee: String::new(),
        cat: None,
        amount: None,
    }
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
        let fmts: &'static [Fmt; 3] = Box::leak(Box::new(LOCS.map(Fmt::new)));
        let mut proto = Self {
            h,
            theme: Theme::new(ThemeSettings::default()),
            fmts,
            variant,
            t: Toggles {
                locale: 0,
                currency: 0,
                calc: false,
                preview: false,
                partial: true,
                shortcuts: true,
                picker: true,
                match_mode: 0,
                rank: false,
                rounding: 0,
            },
            dock_open: true,
            status: "Ready".to_owned(),
            cats: generate_cats(),
            use_clock: 100,
            row: draft_row(),
            entered: Vec::new(),
            emitted: VecDeque::new(),
            date_ed: TextEd::default(),
            money_ed: TextEd::default(),
            payee_ed: TextEd::default(),
            combo: Combo::default(),
            segs: [None; 3],
            seg: 0,
            seg_buf: String::new(),
            seg_error: None,
            cents: String::new(),
            neg: false,
            pick: None,
            pick_rect: None,
            cal_btn: None,
            pending_focus: None,
            pending_blur: false,
            pending_tab: None,
            save_requested: false,
        };
        proto.seed_rows();
        proto.reset_row(false);
        proto
    }

    /// A few entered rows, so the draft row sits under a Register's worth of context.
    fn seed_rows(&mut self) {
        let usd = code_of(0);
        let seeds = [
            (
                (2026, 9, 28),
                "Northside Grocers",
                "Food & Dining:Groceries",
                -84_17,
            ),
            ((2026, 9, 29), "Acme Payroll", "Income:Salary", 341_250),
            (
                (2026, 9, 30),
                "City Power & Light",
                "Bills & Utilities:Electricity",
                -96_40,
            ),
        ];
        for ((y, m, d), payee, cat, cents) in seeds {
            self.entered.push(Entered {
                date: Date { y, m, d },
                payee: payee.to_owned(),
                cat: cat.to_owned(),
                amount: Money::new(Decimal::new(cents, 2), usd).unwrap(),
            });
        }
    }

    /// The starting draft row (Home:Rent, 2026-10-01, -1,850.00), or a blank one.
    fn reset_row(&mut self, blank: bool) {
        self.row = if blank {
            draft_row()
        } else {
            Row {
                date: Some(Date {
                    y: 2026,
                    m: 10,
                    d: 1,
                }),
                payee: "Maple Property Mgmt".to_owned(),
                cat: self.cats.iter().position(|c| c.path == "Home:Rent"),
                amount: Some(Money::new(Decimal::new(-1850, 0), self.code()).unwrap()),
            }
        };
        self.cents.clear();
        self.date_ed = TextEd::default();
        self.money_ed = TextEd::default();
        self.payee_ed = TextEd::default();
        self.combo = Combo::default();
        self.seg_error = None;
    }

    fn loc(&self) -> Loc {
        LOCS[self.t.locale]
    }

    fn f(&self) -> &'static Fmt {
        &self.fmts[self.t.locale]
    }

    fn code(&self) -> CurrencyCode {
        code_of(self.t.currency)
    }

    fn dp(&self) -> u32 {
        self.code().decimal_places()
    }

    /// Translate an English string: `de-DE` has a seed set, `en-XA` is the pseudo transform.
    fn tr(&self, s: &str) -> String {
        match self.loc() {
            Loc::EnUs => s.to_owned(),
            Loc::DeDe => DE_SEED
                .iter()
                .find(|(k, _)| *k == s)
                .map_or_else(|| s.to_owned(), |(_, v)| (*v).to_owned()),
            Loc::EnXa => pseudo(s),
        }
    }

    fn tr1(&self, s: &str, arg: &str) -> String {
        self.tr(s).replacen("{}", arg, 1)
    }

    /// A bare amount, as a Register cell shows it.
    fn amt(&self, v: Option<Money>) -> String {
        v.map_or_else(String::new, |m| {
            self.f()
                .l10n
                .format_amount(m.amount(), i18n_currency(m.currency()), AmountStyle::Bare)
        })
    }

    /// Any decimal at all the places it has: for "rounded from" notes.
    fn exact(&self, v: Decimal) -> String {
        let v = v.round_dp(6).normalize();
        let places = v.scale().max(self.dp());
        self.f().l10n.format_amount(
            v,
            Currency::new(self.code().as_str(), places).unwrap(),
            AmountStyle::Bare,
        )
    }

    fn date_text(&self, d: Option<Date>) -> String {
        d.map_or_else(String::new, |d| fmt_date(d, self.f()))
    }

    fn emit(&mut self, command: &str, source: &str) {
        self.h.event(
            "command",
            json!({ "would_emit": command, "source": source }),
        );
        self.emitted.push_front(format!("{source} → {command}"));
        self.emitted.truncate(8);
        self.status = format!("{source} → {command}");
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
        self.pick = None;
        self.date_ed.error = None;
        self.money_ed.error = None;
        self.seg_error = None;
        self.combo = Combo::default();
        self.h.screenshot(format!("variant-{}", to.key()));
    }

    fn set_toggle(&mut self, key: &str, value: Option<bool>, via: &str) {
        if CYCLING.contains(&key) {
            let (len, slot) = match key {
                "locale" => (LOCS.len(), &mut self.t.locale),
                "currency" => (CURRENCIES.len(), &mut self.t.currency),
                "rounding" => (Rounding::ALL.len(), &mut self.t.rounding),
                _ => (MatchMode::ALL.len(), &mut self.t.match_mode),
            };
            *slot = (*slot + 1) % len;
            if key == "currency" {
                // Stub: the account's currency changed, so the draft amount is relabelled
                // (not converted) at the new currency's decimal places.
                let code = self.code();
                self.row.amount = self.row.amount.map(|m| {
                    Money::rounded(m.amount(), code, RoundingPolicy::HalfAwayFromZero).unwrap()
                });
                self.cents.clear();
                self.money_ed = TextEd::default();
            }
        } else {
            let Some(slot) = self.t.flag(key) else {
                self.h.event("control_unknown", json!({ "toggle": key }));
                return;
            };
            *slot = value.unwrap_or(!*slot);
        }
        let v = self.t.describe(key);
        self.h
            .event("toggle", json!({ "key": key, "value": v, "via": via }));
        self.h
            .screenshot(format!("toggle-{key}-{}", v.to_string().trim_matches('"')));
    }

    /// Step a cycling toggle until it shows `value`.
    fn set_cycling(&mut self, key: &str, value: &str, cmd: &str) {
        for _ in 0..4 {
            if self
                .t
                .describe(key)
                .as_str()
                .is_some_and(|v| v.starts_with(value))
            {
                return;
            }
            self.set_toggle(key, None, "control");
        }
        self.h.event("control_unknown", json!({ "command": cmd }));
    }

    fn set_dock(&mut self, open: bool, via: &str) {
        self.dock_open = open;
        self.h.event("dock", json!({ "open": open, "via": via }));
    }

    // -----------------------------------------------------------------------------------------
    // Control file

    fn apply_control(&mut self, ctx: &egui::Context, cmd: &str) {
        let (verb, rest) = cmd
            .split_once(' ')
            .map_or((cmd, ""), |(v, r)| (v, r.trim()));
        match verb {
            "variant" => match Variant::from_key(rest) {
                Some(v) => self.set_variant(v, "control"),
                None => self.h.event("control_unknown", json!({ "command": cmd })),
            },
            "toggle" => {
                let (key, value) = rest
                    .split_once(' ')
                    .map_or((rest, None), |(k, v)| (k, Some(v.trim())));
                if CYCLING.contains(&key) && value.is_some() {
                    self.set_cycling(key, value.unwrap_or_default(), cmd);
                } else {
                    self.set_toggle(key, value.map(|v| v == "on"), "control");
                }
            }
            "locale" => self.set_cycling("locale", rest, cmd),
            "currency" => self.set_cycling("currency", rest, cmd),
            "rounding" => self.set_cycling("rounding", rest, cmd),
            "match" => self.set_cycling("match", rest, cmd),
            "focus" => match Field::from_name(rest) {
                Some(f) => self.pending_focus = Some(f),
                None => self.h.event("control_unknown", json!({ "command": cmd })),
            },
            "blur" => self.pending_blur = true,
            "set" => {
                let (field, text) = rest.split_once(' ').unwrap_or((rest, ""));
                self.control_set(field, text);
            }
            "picker" => {
                if rest == "open" {
                    self.pick = Some(self.row.date.unwrap_or(TODAY));
                    self.pending_focus = Some(Field::Date);
                } else {
                    self.pick = None;
                }
                self.h.event(
                    "date_popup",
                    json!({ "open": rest == "open", "via": "control" }),
                );
            }
            "reset" => {
                self.reset_row(rest == "blank");
                self.h.event("reset", json!({ "row": rest }));
            }
            "screenshot" => self
                .h
                .screenshot(if rest.is_empty() { "control" } else { rest }),
            "dock" => self.set_dock(rest != "off", "control"),
            "reload" => {
                self.cats = generate_cats();
                self.entered.clear();
                self.seed_rows();
                self.h.event("reload", json!({ "seed": SEED }));
            }
            "quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            _ => self.h.event("control_unknown", json!({ "command": cmd })),
        }
    }

    /// Put `text` into a field as if typed, without committing it.
    fn control_set(&mut self, field: &str, text: &str) {
        let Some(f) = Field::from_name(field) else {
            self.h.event("control_unknown", json!({ "set": field }));
            return;
        };
        self.pending_focus = Some(f);
        match f {
            Field::Date => {
                if self.variant == Variant::C {
                    let base = self.row.date.unwrap_or(TODAY);
                    if let Ok(Some(d)) = parse_date(text, self.f(), true, base) {
                        self.set_segs(Some(d));
                    }
                } else {
                    self.date_ed.buf = text.to_owned();
                    self.date_ed.dirty = true;
                    self.date_ed.error = None;
                }
            }
            Field::Amount => {
                if self.variant == Variant::C {
                    self.cents = text.chars().filter(char::is_ascii_digit).collect();
                    self.neg = text.contains('-');
                } else {
                    self.money_ed.buf = text.to_owned();
                    self.money_ed.dirty = true;
                    self.money_ed.error = None;
                }
            }
            Field::Payee => {
                self.payee_ed.buf = text.to_owned();
                self.payee_ed.dirty = true;
            }
            Field::Category => {
                let st = &mut self.combo;
                st.ed.buf = text.to_owned();
                st.typed = text.to_owned();
                st.ed.dirty = true;
                st.ed.error = None;
                st.open = true;
                st.all = false;
                st.hl = Some(0);
                st.moved = false;
            }
        }
        self.h
            .event("control_set", json!({ "field": field, "text": text }));
    }

    // -----------------------------------------------------------------------------------------
    // Leaving a cell: the Register's keys

    /// Requests focus for `field` when the control file or the last cell asked for it.
    fn take_pending_focus(&mut self, ui: &Ui, field: Field) -> bool {
        if self.pending_focus != Some(field) {
            return false;
        }
        self.pending_focus = None;
        ui.memory_mut(|m| m.request_focus(field.id()));
        true
    }

    /// The key (or the control file's `blur`) that leaves the focused cell this frame.
    /// Call before the cell's widget, after any popup has taken the keys it owns. Modified
    /// keys are matched before their plain forms: `consume_key` ignores unnamed modifiers.
    fn leave_key(&mut self, ui: &Ui, field: Field) -> Option<Leave> {
        if !ui.memory(|m| m.has_focus(field.id())) {
            return None;
        }
        if std::mem::take(&mut self.pending_blur) {
            ui.memory_mut(|m| m.surrender_focus(field.id()));
            return Some(Leave::Blur);
        }
        if let Some(back) = self.pending_tab.take() {
            return Some(if back { Leave::BackTab } else { Leave::Tab });
        }
        ui.input_mut(|i| {
            if i.consume_key(Modifiers::COMMAND, Key::Enter) {
                Some(Leave::CtrlEnter)
            } else if i.consume_key(Modifiers::NONE, Key::Enter) {
                Some(Leave::Enter)
            } else if i.consume_key(Modifiers::NONE, Key::Escape) {
                Some(Leave::Esc)
            } else {
                None
            }
        })
    }

    /// After a cell commits (`ok`) or fails to: move on, or stay with the error.
    fn finish_leave(&mut self, ui: &Ui, field: Field, leave: Leave, ok: bool) {
        self.h.event(
            "cell_leave",
            json!({ "field": field.name(), "via": leave.name(), "ok": ok }),
        );
        if !ok {
            ui.memory_mut(|m| m.request_focus(field.id()));
            return;
        }
        match leave {
            Leave::Enter if field == Field::Amount => self.save_requested = true,
            Leave::Enter | Leave::Tab => self.pending_focus = Some(field.step(1)),
            Leave::BackTab => self.pending_focus = Some(field.step(-1)),
            Leave::CtrlEnter => self.save_requested = true,
            Leave::Esc => ui.memory_mut(|m| m.surrender_focus(field.id())),
            Leave::Blur => {}
        }
    }

    // -----------------------------------------------------------------------------------------
    // Shared text box: select-all on focus, error clearing, blur handling

    fn edit_box(
        &mut self,
        ui: &mut Ui,
        ed: &mut TextEd,
        id: Id,
        name: &str,
        width: f32,
        right: bool,
        hint: &str,
        popup: Option<Rect>,
    ) -> BoxOut {
        let mut te = egui::TextEdit::singleline(&mut ed.buf)
            .id(id)
            .desired_width(width)
            .event_filter(CELL_KEYS)
            .hint_text(hint);
        if right {
            te = te.horizontal_align(Align::RIGHT);
        }
        let mut resp = ui.add(te);
        self.h.track(&resp, name);
        if resp.changed() {
            ed.dirty = true;
            ed.error = None;
        }
        if resp.gained_focus() || ed.sel_all {
            select_range(ui.ctx(), id, 0, ed.buf.chars().count());
            ed.sel_all = false;
        }
        let mut lost = resp.lost_focus();
        if lost && over(ui, popup) {
            ui.memory_mut(|m| m.request_focus(id));
            lost = false;
        }
        resp.id = id;
        BoxOut { resp, lost }
    }

    /// The error or preview line under a field: inline (A, C) or a floating bubble (B).
    fn note(&mut self, ui: &mut Ui, field: &egui::Response, key: &str, text: &str, bad: bool) {
        let colour = if bad {
            ui.visuals().error_fg_color
        } else {
            ui.visuals().weak_text_color()
        };
        if self.variant == Variant::B {
            egui::Area::new(Id::new(("note", key)))
                .order(egui::Order::Tooltip)
                .fixed_pos(field.rect.left_bottom() + Vec2::new(0.0, 2.0))
                .interactable(false)
                .show(ui.ctx(), |ui| {
                    let mut frame = egui::Frame::popup(ui.style());
                    if bad {
                        frame = frame.stroke(egui::Stroke::new(1.0, colour));
                    }
                    frame.show(ui, |ui| {
                        ui.label(RichText::new(text).small().color(colour));
                    });
                });
        } else {
            ui.label(RichText::new(text).small().color(colour));
        }
    }

    // -----------------------------------------------------------------------------------------
    // Payee: a plain text cell, here only so Tab and Enter walk a real row

    fn payee_field(&mut self, ui: &mut Ui) {
        let field = Field::Payee;
        let id = field.id();
        self.take_pending_focus(ui, field);
        let leave = self.leave_key(ui, field);
        let mut ed = std::mem::take(&mut self.payee_ed);
        let focused = ui.memory(|m| m.has_focus(id));
        if !focused && !ed.dirty {
            ed.buf = self.row.payee.clone();
        }
        let out = self.edit_box(ui, &mut ed, id, "payee field", 170.0, false, "", None);
        let leave = leave.or(out.lost.then_some(Leave::Blur));
        match leave {
            Some(Leave::Esc) => {
                ed.buf = self.row.payee.clone();
                ed.dirty = false;
                self.h.event("field_revert", json!({ "field": "payee" }));
                self.finish_leave(ui, field, Leave::Esc, true);
            }
            Some(l) => {
                self.row.payee = ed.buf.trim().to_owned();
                ed.dirty = false;
                self.finish_leave(ui, field, l, true);
            }
            None => {}
        }
        self.payee_ed = ed;
    }

    // -----------------------------------------------------------------------------------------
    // Money

    /// Returns the value and a note (e.g. that a calculator result was rounded).
    fn parse_money(&self, raw: &str) -> Result<(Option<Money>, Option<String>), String> {
        let (f, code) = (self.f(), self.code());
        let bad = |e: AmountParseError| match e {
            AmountParseError::Empty => unreachable!(),
            AmountParseError::Invalid => self
                .tr("“{}” isn't an amount here. Decimal mark: “{}”, group separator: “{}”.")
                .replacen("{}", raw.trim(), 1)
                .replacen("{}", &f.dec.to_string(), 1)
                .replacen("{}", &f.grp.to_string(), 1),
            AmountParseError::TooManyDecimalPlaces { decimal_places } => self
                .tr("{} amounts have {} decimal places")
                .replacen("{}", code.as_str(), 1)
                .replacen("{}", &decimal_places.to_string(), 1),
            AmountParseError::OutOfRange => self.tr("That amount is too large"),
        };
        if raw.trim().is_empty() {
            return Ok((None, None));
        }
        if self.t.calc && self.variant != Variant::C && looks_like_expression(raw) {
            let v = match eval_expression(raw, &f.l10n) {
                Ok(v) => v,
                Err(AmountParseError::Empty) => return Ok((None, None)),
                Err(e) => return Err(bad(e)),
            };
            if let Ok(m) = Money::new(v, code) {
                return Ok((Some(m), None));
            }
            let rounding = Rounding::ALL[self.t.rounding];
            return match rounding.policy() {
                None => Err(self
                    .tr("= {}, but {} amounts have {} decimal places. Not rounded.")
                    .replacen("{}", &self.exact(v), 1)
                    .replacen("{}", code.as_str(), 1)
                    .replacen("{}", &self.dp().to_string(), 1)),
                Some(policy) => {
                    let m = Money::rounded(v, code, policy)
                        .map_err(|_| bad(AmountParseError::OutOfRange))?;
                    let note = self
                        .tr("rounded from {} ({})")
                        .replacen("{}", &self.exact(v), 1)
                        .replacen("{}", rounding.name(), 1);
                    Ok((Some(m), Some(note)))
                }
            };
        }
        match f.l10n.parse_amount(raw, self.dp()) {
            Ok(v) => Ok((Some(Money::new(v, code).unwrap()), None)),
            Err(AmountParseError::Empty) => Ok((None, None)),
            Err(e) => Err(bad(e)),
        }
    }

    fn commit_money(&mut self, ed: &mut TextEd) -> bool {
        let raw = ed.buf.clone();
        match self.parse_money(&raw) {
            Ok((v, note)) => {
                self.row.amount = v;
                ed.error = None;
                ed.dirty = false;
                ed.buf = self.amt(v);
                self.h.event(
                    "field_commit",
                    json!({ "field": "amount", "raw": raw, "outcome": "ok", "value": ed.buf, "note": note }),
                );
                if let Some(n) = note {
                    self.status = n;
                }
                true
            }
            Err(msg) => {
                self.h.event(
                    "field_commit",
                    json!({ "field": "amount", "raw": raw, "outcome": "error", "message": msg }),
                );
                ed.error = Some(msg);
                false
            }
        }
    }

    fn money_field(&mut self, ui: &mut Ui) {
        let field = Field::Amount;
        let id = field.id();
        self.take_pending_focus(ui, field);
        if self.variant == Variant::C {
            self.money_cents(ui, id);
            return;
        }
        let leave = self.leave_key(ui, field);
        let focused = ui.memory(|m| m.has_focus(id));
        let mut ed = std::mem::take(&mut self.money_ed);
        if !focused && ed.error.is_none() && !ed.dirty {
            ed.buf = self.amt(self.row.amount);
        }
        let out = self.edit_box(ui, &mut ed, id, "amount field", 150.0, true, "0", None);
        let leave = leave.or(out.lost.then_some(Leave::Blur));
        match leave {
            Some(Leave::Esc) => {
                ed.buf = self.amt(self.row.amount);
                ed.dirty = false;
                ed.error = None;
                self.h.event("field_revert", json!({ "field": "amount" }));
                self.finish_leave(ui, field, Leave::Esc, true);
            }
            Some(l) => {
                let ok = self.commit_money(&mut ed);
                self.finish_leave(ui, field, l, ok);
            }
            None => {}
        }
        let msg = if let Some(e) = ed.error.clone() {
            Some((e, true))
        } else if self.t.preview && out.resp.has_focus() && ed.dirty {
            Some(match self.parse_money(&ed.buf) {
                Ok((Some(v), note)) => {
                    let mut s = format!(
                        "= {}",
                        self.f().l10n.format_amount(
                            v.amount(),
                            i18n_currency(v.currency()),
                            AmountStyle::Symbol
                        )
                    );
                    if let Some(n) = note {
                        s.push_str(&format!(" ({n})"));
                    }
                    (s, false)
                }
                Ok((None, _)) => (self.tr("empty"), false),
                Err(_) => (self.tr("Not an amount yet"), false),
            })
        } else {
            None
        };
        if let Some((text, bad)) = msg {
            self.note(ui, &out.resp, "amount", &text, bad);
        }
        self.money_ed = ed;
    }

    fn cents_value(&self) -> Option<Money> {
        if self.cents.is_empty() {
            return None;
        }
        let minor: i64 = self.cents.parse().unwrap_or(0);
        let minor = if self.neg { -minor } else { minor };
        Money::new(Decimal::new(minor, self.dp()), self.code()).ok()
    }

    fn money_cents(&mut self, ui: &mut Ui, id: Id) {
        let field = Field::Amount;
        let leave = self.leave_key(ui, field);
        let focused = ui.memory(|m| m.has_focus(id));
        let width = 150.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::click());
        let resp = ui.interact(rect, id, Sense::click());
        self.h.track(&resp, "amount field");
        if resp.clicked() {
            ui.memory_mut(|m| m.request_focus(id));
        }
        let focused_now = ui.memory(|m| m.has_focus(id));
        if focused_now && !focused {
            let a = self.row.amount;
            self.neg = a.is_some_and(|m| m.is_negative());
            self.cents = a.map_or_else(String::new, |m| m.amount().abs().mantissa().to_string());
        }
        if focused_now && leave.is_none() {
            ui.memory_mut(|m| m.set_focus_lock_filter(id, CELL_KEYS));
            let (mut cents, mut neg) = (std::mem::take(&mut self.cents), self.neg);
            ui.input_mut(|i| {
                i.events.retain(|e| match e {
                    Event::Text(s) => {
                        for c in s.chars() {
                            match c {
                                '0'..='9' if cents.len() < 12 => {
                                    cents.push(c);
                                    cents = cents.trim_start_matches('0').to_owned();
                                    if cents.is_empty() {
                                        cents.push('0');
                                    }
                                }
                                '-' | '−' => neg = true,
                                '+' => neg = false,
                                _ => {}
                            }
                        }
                        false
                    }
                    Event::Key {
                        key: Key::Backspace,
                        pressed: true,
                        ..
                    } => {
                        cents.pop();
                        false
                    }
                    Event::Key {
                        key: Key::Delete,
                        pressed: true,
                        ..
                    } => {
                        cents.clear();
                        false
                    }
                    _ => true,
                });
            });
            self.cents = cents;
            self.neg = neg;
        }
        let shown = if focused_now {
            self.cents_value()
        } else {
            self.row.amount
        };
        let text = if focused_now && shown.is_none() {
            String::from("0")
        } else {
            self.amt(shown)
        };
        self.paint_box(ui, rect, &text, focused_now, Align::RIGHT);
        let leave = leave.or((focused && !focused_now).then_some(Leave::Blur));
        match leave {
            Some(Leave::Esc) => {
                self.h.event("field_revert", json!({ "field": "amount" }));
                self.finish_leave(ui, field, Leave::Esc, true);
            }
            Some(l) => {
                let v = self.cents_value();
                self.row.amount = v;
                self.h.event(
                    "field_commit",
                    json!({ "field": "amount", "raw": self.cents, "outcome": "ok", "value": self.amt(v) }),
                );
                self.finish_leave(ui, field, l, true);
            }
            None => {}
        }
    }

    /// A TextEdit-looking box for the custom (C) widgets.
    fn paint_box(&self, ui: &Ui, rect: Rect, text: &str, focused: bool, align: Align) {
        let v = ui.visuals();
        let stroke = if focused {
            v.selection.stroke
        } else {
            v.widgets.inactive.bg_stroke
        };
        ui.painter().rect(
            rect,
            2.0,
            v.extreme_bg_color,
            stroke,
            egui::StrokeKind::Inside,
        );
        let font = egui::TextStyle::Body.resolve(ui.style());
        let galley = ui
            .painter()
            .layout_no_wrap(text.to_owned(), font, v.text_color());
        let x = if align == Align::RIGHT {
            rect.right() - 4.0 - galley.size().x
        } else {
            rect.left() + 4.0
        };
        ui.painter().galley(
            egui::pos2(x, rect.center().y - galley.size().y / 2.0),
            galley,
            v.text_color(),
        );
    }

    // -----------------------------------------------------------------------------------------
    // Date

    fn example_date(&self) -> String {
        fmt_date(
            Date {
                y: 2026,
                m: 10,
                d: 5,
            },
            self.f(),
        )
    }

    fn date_hint(&self) -> String {
        format!("{} = {}", self.f().pattern, self.example_date())
    }

    fn date_error(&self) -> String {
        self.tr("Not a date. Expected {}, for example {}.")
            .replacen("{}", self.f().pattern, 1)
            .replacen("{}", &self.example_date(), 1)
    }

    fn apply_date_act(&self, act: DateAct, base: Date) -> Date {
        match act {
            DateAct::Days(n) => base.add_days(n),
            DateAct::Months(n) => base.add_months(n),
            DateAct::Today => TODAY,
            DateAct::MonthStart => Date { d: 1, ..base },
            DateAct::MonthEnd => Date {
                d: days_in_month(base.y, base.m),
                ..base
            },
            DateAct::YearStart => Date { m: 1, d: 1, ..base },
            DateAct::YearEnd => Date {
                m: 12,
                d: 31,
                ..base
            },
        }
    }

    /// Consume shortcut keys while a date field has focus. `untouched` gates `+` and `-`, which are
    /// also legal characters in typed dates.
    fn date_shortcuts(&self, ui: &Ui, untouched: bool, arrows: bool) -> Option<(DateAct, String)> {
        let mut act = None;
        ui.input_mut(|i| {
            i.events.retain(|e| {
                let Event::Text(s) = e else { return true };
                let a = match s.as_str() {
                    "t" | "T" => DateAct::Today,
                    "m" | "M" => DateAct::MonthStart,
                    "h" | "H" => DateAct::MonthEnd,
                    "y" | "Y" => DateAct::YearStart,
                    "r" | "R" => DateAct::YearEnd,
                    "+" | "=" if untouched => DateAct::Days(1),
                    "-" | "_" if untouched => DateAct::Days(-1),
                    _ => return true,
                };
                act = Some((a, s.clone()));
                false
            });
            if arrows {
                for (key, a) in [
                    (Key::ArrowUp, DateAct::Days(1)),
                    (Key::ArrowDown, DateAct::Days(-1)),
                    (Key::PageUp, DateAct::Months(-1)),
                    (Key::PageDown, DateAct::Months(1)),
                ] {
                    if i.consume_key(Modifiers::NONE, key) {
                        act = Some((a, key.name().to_owned()));
                    }
                }
            }
        });
        act
    }

    fn set_segs(&mut self, d: Option<Date>) {
        let vals = d.map_or([None; 3], |d| [Some(d.d), Some(d.m), Some(d.y as u32)]);
        self.segs = vals;
        self.seg_buf.clear();
        self.seg_error = None;
    }

    /// Logical segment order for the locale: indices into `segs` (0 day, 1 month, 2 year).
    fn seg_order(&self) -> [usize; 3] {
        if self.f().mdy { [1, 0, 2] } else { [0, 1, 2] }
    }

    fn choose_date(&mut self, d: Date, via: &str) {
        self.row.date = Some(d);
        self.date_ed.buf = fmt_date(d, self.f());
        self.date_ed.dirty = false;
        self.date_ed.error = None;
        self.set_segs(Some(d));
        self.pick = None;
        self.h
            .event("date_pick", json!({ "date": d.iso(), "via": via }));
    }

    fn commit_date(&mut self, ed: &mut TextEd) -> bool {
        let raw = ed.buf.clone();
        let base = self.row.date.unwrap_or(TODAY);
        match parse_date(&raw, self.f(), self.t.partial, base) {
            Ok(d) => {
                self.row.date = d;
                ed.error = None;
                ed.dirty = false;
                ed.buf = self.date_text(d);
                self.set_segs(d);
                self.h.event(
                    "field_commit",
                    json!({ "field": "date", "raw": raw, "outcome": "ok", "value": ed.buf }),
                );
                true
            }
            Err(()) => {
                let msg = self.date_error();
                self.h.event(
                    "field_commit",
                    json!({ "field": "date", "raw": raw, "outcome": "error", "message": msg }),
                );
                ed.error = Some(msg);
                false
            }
        }
    }

    /// Keys the open calendar owns: arrows, PgUp/PgDn, Enter picks, Esc closes.
    fn calendar_keys(&mut self, ui: &Ui) {
        let Some(mut cur) = self.pick else { return };
        let mut chosen = None;
        let mut close = false;
        ui.input_mut(|i| {
            for (key, step) in [
                (Key::ArrowLeft, -1),
                (Key::ArrowRight, 1),
                (Key::ArrowUp, -7),
                (Key::ArrowDown, 7),
            ] {
                if i.consume_key(Modifiers::NONE, key) {
                    cur = cur.add_days(step);
                }
            }
            if i.consume_key(Modifiers::NONE, Key::PageUp) {
                cur = cur.add_months(-1);
            }
            if i.consume_key(Modifiers::NONE, Key::PageDown) {
                cur = cur.add_months(1);
            }
            if i.consume_key(Modifiers::NONE, Key::Enter) {
                chosen = Some(cur);
            }
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                close = true;
            }
        });
        if let Some(d) = chosen {
            self.choose_date(d, "key");
        } else if close {
            self.pick = None;
            self.h
                .event("date_popup", json!({ "open": false, "via": "esc" }));
        } else {
            self.pick = Some(cur);
        }
    }

    fn date_field(&mut self, ui: &mut Ui) {
        let field = Field::Date;
        let id = field.id();
        self.take_pending_focus(ui, field);
        let focused = ui.memory(|m| m.has_focus(id));
        // The open calendar takes its keys first, then Alt+Down (before plain Down), then
        // the shortcuts, then the keys that leave the cell.
        if focused && self.pick.is_some() {
            self.calendar_keys(ui);
        }
        if focused
            && self.t.picker
            && self.pick.is_none()
            && ui.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowDown))
        {
            self.pick = Some(self.row.date.unwrap_or(TODAY));
            self.h
                .event("date_popup", json!({ "open": true, "via": "alt+down" }));
        }
        let mut field_rect = Rect::NOTHING;
        let mut field_resp = None;
        ui.horizontal(|ui| {
            if self.variant == Variant::C {
                let r = self.date_segments(ui, id);
                field_rect = r.rect;
                field_resp = Some(r);
            } else {
                let mut ed = std::mem::take(&mut self.date_ed);
                if !focused && ed.error.is_none() && !ed.dirty {
                    ed.buf = self.date_text(self.row.date);
                }
                if focused && self.t.shortcuts {
                    let base = self.row.date.unwrap_or(TODAY);
                    let base = parse_date(&ed.buf, self.f(), self.t.partial, base)
                        .ok()
                        .flatten()
                        .unwrap_or(base);
                    if let Some((act, key)) =
                        self.date_shortcuts(ui, !ed.dirty, self.pick.is_none())
                    {
                        let nd = self.apply_date_act(act, base);
                        self.row.date = Some(nd);
                        ed.buf = fmt_date(nd, self.f());
                        ed.dirty = false;
                        ed.error = None;
                        ed.sel_all = true;
                        self.h.event(
                            "date_shortcut",
                            json!({ "key": key, "from": base.iso(), "to": nd.iso() }),
                        );
                    }
                }
                let leave = self.leave_key(ui, field);
                let hint = self.date_hint();
                let popup = union(self.pick_rect, self.cal_btn);
                let out = self.edit_box(ui, &mut ed, id, "date field", 110.0, false, &hint, popup);
                let leave = leave.or(out.lost.then_some(Leave::Blur));
                match leave {
                    Some(Leave::Esc) => {
                        ed.buf = self.date_text(self.row.date);
                        ed.dirty = false;
                        ed.error = None;
                        self.h.event("field_revert", json!({ "field": "date" }));
                        self.finish_leave(ui, field, Leave::Esc, true);
                    }
                    Some(l) => {
                        self.pick = None;
                        let ok = self.commit_date(&mut ed);
                        self.finish_leave(ui, field, l, ok);
                    }
                    None => {}
                }
                field_rect = out.resp.rect;
                field_resp = Some(out.resp);
                self.date_ed = ed;
            }
            // The calendar button shows only while the cell is edited (#17).
            let editing =
                ui.memory(|m| m.has_focus(id)) || self.pick.is_some() || over(ui, self.cal_btn);
            if self.t.picker && editing {
                let b = ui.small_button("📅").on_hover_text("Calendar (Alt+Down)");
                self.h.track(&b, "date picker button");
                if pressed(ui, &b) {
                    self.pick = if self.pick.is_some() {
                        None
                    } else {
                        Some(self.row.date.unwrap_or(TODAY))
                    };
                    ui.memory_mut(|m| m.request_focus(id));
                    self.h.event(
                        "date_popup",
                        json!({ "open": self.pick.is_some(), "via": "click" }),
                    );
                }
                self.cal_btn = Some(b.rect);
            } else {
                self.cal_btn = None;
            }
        });
        if self.pick.is_some() && self.t.picker {
            self.calendar(ui, field_rect, id);
        } else {
            self.pick = None;
            self.pick_rect = None;
        }
        // Message line.
        let focused = ui.memory(|m| m.has_focus(id));
        let (err, live) = if self.variant == Variant::C {
            (self.seg_error.clone(), None)
        } else {
            (self.date_ed.error.clone(), Some(self.date_ed.buf.clone()))
        };
        if let Some(resp) = field_resp {
            if let Some(e) = err {
                self.note(ui, &resp, "date", &e, true);
            } else if self.t.preview && self.variant != Variant::C && focused && self.date_ed.dirty
            {
                let base = self.row.date.unwrap_or(TODAY);
                let text =
                    match parse_date(&live.unwrap_or_default(), self.f(), self.t.partial, base) {
                        Ok(Some(d)) => format!("= {}", fmt_date_long(d, self.f())),
                        Ok(None) => self.tr("empty"),
                        Err(()) => self.tr1("Not a date yet ({})", self.f().pattern),
                    };
                self.note(ui, &resp, "date", &text, false);
            }
        }
    }

    /// Variant C: day, month and year as separate segments.
    fn date_segments(&mut self, ui: &mut Ui, id: Id) -> egui::Response {
        let field = Field::Date;
        let f = self.f();
        let order = self.seg_order();
        let placeholders = if matches!(self.loc(), Loc::EnUs) {
            ["DD", "MM", "YYYY"]
        } else {
            ["TT", "MM", "JJJJ"]
        };
        let font = egui::TextStyle::Body.resolve(ui.style());
        let text_colour = ui.visuals().text_color();
        let weak = ui.visuals().weak_text_color();
        let widths = [2usize, 2, 4];
        let char_w = ui
            .painter()
            .layout_no_wrap("0".to_owned(), font.clone(), text_colour)
            .size()
            .x;
        let total = order
            .iter()
            .map(|&k| widths[k] as f32 * char_w + 8.0)
            .sum::<f32>()
            + 2.0 * char_w
            + 8.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(total, 24.0), Sense::click());
        let resp = ui.interact(rect, id, Sense::click());
        self.h.track(&resp, "date field");
        let was_focused = ui.memory(|m| m.has_focus(id));
        // Segment rectangles.
        let mut rects = [Rect::NOTHING; 3];
        let mut x = rect.left() + 4.0;
        for (slot, &k) in order.iter().enumerate() {
            let w = widths[k] as f32 * char_w + 4.0;
            rects[slot] = Rect::from_min_size(egui::pos2(x, rect.top() + 2.0), Vec2::new(w, 20.0));
            x += w + char_w + 4.0;
        }
        if resp.clicked() {
            ui.memory_mut(|m| m.request_focus(id));
            if let Some(p) = resp.interact_pointer_pos() {
                if let Some(slot) = rects
                    .iter()
                    .position(|r| r.expand2(Vec2::new(char_w / 2.0, 0.0)).contains(p))
                {
                    self.seg = slot;
                    self.seg_buf.clear();
                }
            }
        }
        let focused = ui.memory(|m| m.has_focus(id));
        if focused && !was_focused {
            self.set_segs(self.row.date);
            self.seg = 0;
        }
        let mut leave = None;
        if focused {
            ui.memory_mut(|m| m.set_focus_lock_filter(id, CELL_KEYS));
            if self.t.shortcuts && self.seg_buf.is_empty() {
                let base = self.row.date.unwrap_or(TODAY);
                if let Some((act, key)) = self.date_shortcuts(ui, true, false) {
                    let nd = self.apply_date_act(act, base);
                    self.row.date = Some(nd);
                    self.set_segs(Some(nd));
                    self.h.event(
                        "date_shortcut",
                        json!({ "key": key, "from": base.iso(), "to": nd.iso() }),
                    );
                }
            }
            leave = self.leave_key(ui, field);
            // Digits, separators, arrows, backspace.
            let mut digits = String::new();
            let mut sep = false;
            let (mut left, mut right, mut up, mut down, mut back) =
                (false, false, false, false, false);
            ui.input_mut(|i| {
                i.events.retain(|e| match e {
                    Event::Text(s) => {
                        for c in s.chars() {
                            if c.is_ascii_digit() {
                                digits.push(c);
                            } else if "/.-".contains(c) {
                                sep = true;
                            }
                        }
                        false
                    }
                    _ => true,
                });
                left = i.consume_key(Modifiers::NONE, Key::ArrowLeft);
                right = i.consume_key(Modifiers::NONE, Key::ArrowRight);
                up = i.consume_key(Modifiers::NONE, Key::ArrowUp);
                down = i.consume_key(Modifiers::NONE, Key::ArrowDown);
                back = i.consume_key(Modifiers::NONE, Key::Backspace);
            });
            if left && self.seg > 0 {
                self.seg -= 1;
                self.seg_buf.clear();
            }
            if (right || sep) && self.seg < 2 {
                self.seg += 1;
                self.seg_buf.clear();
            }
            let k = order[self.seg];
            if back {
                self.segs[k] = None;
                self.seg_buf.clear();
            }
            for c in digits.chars() {
                self.seg_buf.push(c);
                let v: u32 = self.seg_buf.parse().unwrap_or(0);
                self.segs[k] = Some(v);
                let full =
                    self.seg_buf.len() >= widths[k] || (k == 0 && v > 3) || (k == 1 && v > 1);
                if full {
                    self.seg_buf.clear();
                    if self.seg < 2 {
                        self.seg += 1;
                    }
                }
            }
            if up || down {
                let delta: i64 = if up { 1 } else { -1 };
                let today = [TODAY.d, TODAY.m, TODAY.y as u32][k];
                let cur = self.segs[k].unwrap_or(today);
                let (lo, hi) = match k {
                    0 => (
                        1,
                        match (self.segs[1], self.segs[2]) {
                            (Some(m), Some(y)) if (1..=12).contains(&m) => {
                                days_in_month(y as i32, m)
                            }
                            _ => 31,
                        },
                    ),
                    1 => (1, 12),
                    _ => (1900, 2200),
                };
                let span = i64::from(hi - lo + 1);
                let n = (i64::from(cur.max(lo) - lo) + delta).rem_euclid(span) as u32 + lo;
                self.segs[k] = Some(if self.segs[k].is_none() { today } else { n });
                self.seg_buf.clear();
                let seg_name = ["day", "month", "year"][k];
                self.h.event(
                    "date_segment_step",
                    json!({ "segment": seg_name, "to": self.segs[k] }),
                );
            }
        }
        // Paint.
        let stroke = if focused {
            ui.visuals().selection.stroke
        } else {
            ui.visuals().widgets.inactive.bg_stroke
        };
        ui.painter().rect(
            rect,
            2.0,
            ui.visuals().extreme_bg_color,
            stroke,
            egui::StrokeKind::Inside,
        );
        for (slot, &k) in order.iter().enumerate() {
            let (text, colour) = match self.segs[k] {
                Some(v) if k == 2 => (format!("{v:04}"), text_colour),
                Some(v) => (format!("{v:02}"), text_colour),
                None => (placeholders[k].to_owned(), weak),
            };
            if focused && slot == self.seg {
                ui.painter()
                    .rect_filled(rects[slot], 2.0, ui.visuals().selection.bg_fill);
            }
            let g = ui.painter().layout_no_wrap(text, font.clone(), colour);
            let pos = egui::pos2(
                rects[slot].right() - 2.0 - g.size().x,
                rect.center().y - g.size().y / 2.0,
            );
            ui.painter().galley(pos, g, colour);
            if slot < 2 {
                let g = ui
                    .painter()
                    .layout_no_wrap(f.sep.to_string(), font.clone(), weak);
                ui.painter().galley(
                    egui::pos2(
                        rects[slot].right() + 1.0,
                        rect.center().y - g.size().y / 2.0,
                    ),
                    g,
                    weak,
                );
            }
        }
        // Live commit when complete; error when the combination is not a date.
        if self.segs.iter().all(Option::is_some) {
            let [d, m, y] = self.segs.map(Option::unwrap);
            match Date::valid(y as i32, m, d) {
                Some(date) => {
                    self.seg_error = None;
                    if Some(date) != self.row.date && focused {
                        self.row.date = Some(date);
                    }
                }
                None => self.seg_error = Some(self.date_error()),
            }
        }
        let leave = leave.or((was_focused && !focused).then_some(Leave::Blur));
        match leave {
            Some(Leave::Esc) => {
                self.set_segs(self.row.date);
                self.h.event("field_revert", json!({ "field": "date" }));
                self.finish_leave(ui, field, Leave::Esc, true);
            }
            Some(l) => {
                let err = self.seg_error.clone();
                if let Some(msg) = err {
                    self.h.event(
                        "field_commit",
                        json!({ "field": "date", "raw": format!("{:?}", self.segs), "outcome": "error", "message": msg }),
                    );
                    self.finish_leave(ui, field, l, false);
                } else {
                    if self.segs.iter().all(Option::is_none) {
                        self.row.date = None;
                    }
                    self.h.event(
                        "field_commit",
                        json!({ "field": "date", "raw": format!("{:?}", self.segs), "outcome": "ok", "value": self.date_text(self.row.date) }),
                    );
                    self.finish_leave(ui, field, l, true);
                }
            }
            None => {}
        }
        resp
    }

    fn calendar(&mut self, ui: &mut Ui, field_rect: Rect, id: Id) {
        let f = self.f();
        let Some(mut cur) = self.pick else { return };
        let mut chosen = None;
        let first = Date { d: 1, ..cur };
        let lead = if f.sunday_first {
            (first.weekday() + 1) % 7
        } else {
            first.weekday()
        };
        let start = first.add_days(-(lead as i64));
        let today_label = self.tr("Today");
        let selected = self.row.date;
        let mut nav = 0;
        let mut go_today = false;
        let area = egui::Area::new(Id::new("calendar_popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(field_rect.left_bottom() + Vec2::new(0.0, 2.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let b = ui.small_button("⏴");
                        if pressed(ui, &b) {
                            nav = -1;
                        }
                        ui.label(
                            RichText::new(format!("{} {}", f.months[cur.m as usize - 1], cur.y))
                                .strong(),
                        );
                        let b = ui.small_button("⏵");
                        if pressed(ui, &b) {
                            nav = 1;
                        }
                        let b = ui.small_button(&today_label);
                        if pressed(ui, &b) {
                            go_today = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for c in 0..7 {
                            let wd = if f.sunday_first { (c + 6) % 7 } else { c };
                            ui.add_sized(
                                [30.0, 18.0],
                                egui::Label::new(RichText::new(f.days[wd]).small().weak()),
                            );
                        }
                    });
                    for r in 0..6 {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            for c in 0..7 {
                                let d = start.add_days(r * 7 + c);
                                let in_month = d.m == cur.m;
                                let mut text = RichText::new(d.d.to_string());
                                if !in_month {
                                    text = text.weak();
                                }
                                if d == TODAY {
                                    text = text.underline();
                                }
                                let btn = egui::Button::new(text)
                                    .selected(Some(d) == selected)
                                    .stroke(if d == cur {
                                        ui.visuals().selection.stroke
                                    } else {
                                        egui::Stroke::NONE
                                    });
                                let r = ui.add_sized([30.0, 22.0], btn);
                                if pressed(ui, &r) {
                                    chosen = Some((d, "click"));
                                }
                            }
                        });
                    }
                });
            });
        self.pick_rect = Some(area.response.rect);
        self.h.region(area.response.rect, "calendar");
        if nav != 0 {
            cur = cur.add_months(nav);
        }
        if go_today {
            chosen = Some((TODAY, "today"));
        }
        // A press outside the calendar, the field and its button closes it.
        let outside = ui.input(|i| i.pointer.primary_pressed())
            && !over(ui, self.pick_rect)
            && !over(ui, self.cal_btn)
            && !over(ui, Some(field_rect));
        if let Some((d, via)) = chosen {
            self.choose_date(d, via);
            ui.memory_mut(|m| m.request_focus(id));
        } else if outside {
            self.pick = None;
            self.h.event(
                "date_popup",
                json!({ "open": false, "via": "click-outside" }),
            );
        } else {
            self.pick = Some(cur);
        }
    }

    // -----------------------------------------------------------------------------------------
    // Category: the combobox list #17 settled, in every variant

    fn rank_key(&self, cat: usize) -> (i64, String) {
        let c = &self.cats[cat];
        if self.t.rank {
            (
                -(i64::from(c.last_used) * 100_000 + i64::from(c.uses)),
                c.path.to_lowercase(),
            )
        } else {
            (0, c.path.to_lowercase())
        }
    }

    /// Matching categories, best first: (category index, matched character indices).
    fn ranked(&self, q: &str) -> Vec<Matched> {
        let mode = MatchMode::ALL[self.t.match_mode];
        let mut out: Vec<(u32, (i64, String), Matched)> = self
            .cats
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let (score, idx) = match_label(mode, q, &c.path)?;
                Some((score, self.rank_key(i), (i, idx)))
            })
            .collect();
        out.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        out.into_iter().map(|x| x.2).collect()
    }

    fn cat_text(&self) -> String {
        self.row
            .cat
            .map_or_else(String::new, |c| self.cats[c].path.clone())
    }

    fn highlighted(&self, ui: &Ui, label: &str, idx: &[usize]) -> egui::text::LayoutJob {
        let mut job = egui::text::LayoutJob::default();
        let font = egui::TextStyle::Body.resolve(ui.style());
        let normal = ui.visuals().text_color();
        let weak = ui.visuals().weak_text_color();
        let hit = ui.visuals().hyperlink_color;
        let mut seen_colon = false;
        for (i, c) in label.chars().enumerate() {
            let is_hit = idx.contains(&i);
            let colour = if is_hit {
                hit
            } else if !seen_colon && label.contains(':') {
                weak
            } else {
                normal
            };
            if c == ':' {
                seen_colon = true;
            }
            job.append(
                &c.to_string(),
                0.0,
                egui::text::TextFormat {
                    font_id: font.clone(),
                    color: colour,
                    underline: if is_hit {
                        egui::Stroke::new(1.0, hit)
                    } else {
                        egui::Stroke::NONE
                    },
                    ..Default::default()
                },
            );
        }
        job
    }

    /// What leaving the cell means: the highlighted row, else an exact match for the typed text.
    fn resolve(&self, st: &Combo, list: &[Matched], offer_create: bool) -> Option<Outcome> {
        if !(st.ed.dirty || st.moved) {
            return None;
        }
        if let Some(h) = st.hl.filter(|_| st.open) {
            if h < list.len() {
                return Some(Outcome::Pick(list[h].0));
            }
            if offer_create {
                return Some(Outcome::Create(st.ed.buf.trim().to_owned()));
            }
        }
        let text = st.ed.buf.trim();
        if text.is_empty() {
            return Some(Outcome::Clear);
        }
        if let Some(i) = self
            .cats
            .iter()
            .position(|c| c.path.eq_ignore_ascii_case(text))
        {
            return Some(Outcome::Pick(i));
        }
        Some(if offer_create {
            Outcome::Create(text.to_owned())
        } else {
            Outcome::Unknown(text.to_owned())
        })
    }

    /// B offers `Create "x"…` when the typed text names no category.
    fn create_row(&self, st: &Combo, offer_create: bool) -> bool {
        offer_create
            && st.ed.dirty
            && !st.all
            && !st.ed.buf.trim().is_empty()
            && !self
                .cats
                .iter()
                .any(|c| c.path.eq_ignore_ascii_case(st.ed.buf.trim()))
    }

    fn query(st: &Combo) -> String {
        if st.all || !st.ed.dirty {
            String::new()
        } else {
            st.typed.clone()
        }
    }

    fn cat_field(&mut self, ui: &mut Ui) {
        let field = Field::Category;
        let id = field.id();
        let offer_create = self.variant == Variant::B;
        self.take_pending_focus(ui, field);
        let mut st = std::mem::take(&mut self.combo);
        let focused = ui.memory(|m| m.has_focus(id));
        if !focused && st.ed.error.is_none() && !st.ed.dirty {
            st.ed.buf = self.cat_text();
            st.typed = st.ed.buf.clone();
            st.open = false;
            st.all = false;
            st.hl = None;
            st.moved = false;
        }
        let list = self.ranked(&Self::query(&st));
        let rows = list.len() + usize::from(self.create_row(&st, offer_create));
        // The list's keys first, Alt+Down before plain Down; then the keys that leave the cell.
        let mut result = None;
        let mut leave = None;
        if focused {
            let mut picked = None;
            let mut list_event = None;
            ui.input_mut(|i| {
                if i.consume_key(Modifiers::ALT, Key::ArrowDown) {
                    st.open = true;
                    st.all = true;
                    st.hl = None;
                    st.moved = false;
                    list_event = Some("alt+down");
                } else if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                    if st.open && rows > 0 {
                        st.hl = Some(st.hl.map_or(0, |h| (h + 1).min(rows - 1)));
                        st.moved = true;
                    } else {
                        st.open = true;
                        list_event = Some("down");
                    }
                }
                if i.consume_key(Modifiers::NONE, Key::ArrowUp) && st.open {
                    // Up past the top clears the highlight, so Enter keeps the typed text.
                    st.hl = match st.hl {
                        Some(0) | None => None,
                        Some(h) => Some(h - 1),
                    };
                    st.moved = true;
                }
                if st.open && st.hl.is_some() && i.consume_key(Modifiers::NONE, Key::Enter) {
                    picked = st.hl;
                }
                if st.open && i.consume_key(Modifiers::NONE, Key::Escape) {
                    st.open = false;
                    st.all = false;
                    list_event = Some("esc-close");
                }
            });
            if let Some(via) = list_event {
                self.h.event(
                    "category_list",
                    json!({ "open": st.open, "all": st.all, "via": via }),
                );
            }
            if let Some(h) = picked {
                result = Some(if h < list.len() {
                    Outcome::Pick(list[h].0)
                } else {
                    Outcome::Create(st.ed.buf.trim().to_owned())
                });
                leave = Some(Leave::Enter);
            } else {
                leave = self.leave_key(ui, field);
            }
        }
        let mut field_resp = None;
        let mut lost = false;
        ui.horizontal(|ui| {
            let popup = union(st.popup, st.btn);
            let out = self.edit_box(
                ui,
                &mut st.ed,
                id,
                "category field",
                250.0,
                false,
                "",
                popup,
            );
            if out.resp.gained_focus() {
                // B opens the list on focus; A and C open it when typing starts.
                st.open = offer_create;
                st.all = false;
                st.hl = None;
                st.moved = false;
            }
            if out.resp.changed() {
                st.open = true;
                st.all = false;
                st.moved = false;
                st.typed = st.ed.buf.clone();
                // The top match starts highlighted, and Enter takes it.
                st.hl = Some(0);
                ui.ctx().request_repaint();
            }
            // B's ⏷ shows only while the cell is edited (#17).
            let editing = ui.memory(|m| m.has_focus(id)) || over(ui, st.btn) || over(ui, st.popup);
            if offer_create && editing {
                let b = ui
                    .small_button("⏷")
                    .on_hover_text("Every category (Alt+Down)");
                self.h.track(&b, "category list button");
                if pressed(ui, &b) {
                    let show_all = !(st.open && st.all);
                    st.open = show_all;
                    st.all = show_all;
                    st.hl = None;
                    ui.memory_mut(|m| m.request_focus(id));
                    self.h.event(
                        "category_list",
                        json!({ "open": st.open, "all": st.all, "via": "button" }),
                    );
                }
                st.btn = Some(b.rect);
            } else {
                st.btn = None;
            }
            lost = out.lost;
            field_resp = Some(out.resp);
        });
        let resp = field_resp.unwrap();
        let list = self.ranked(&Self::query(&st));
        let create = self.create_row(&st, offer_create);
        if let Some(h) = st.hl {
            let rows = list.len() + usize::from(create);
            st.hl = (rows > 0).then(|| h.min(rows - 1));
        }
        // The list.
        if ui.memory(|m| m.has_focus(id)) && st.open {
            let rect = resp.rect;
            let hl = st.hl;
            let moved = st.moved;
            let mut clicked = None;
            let query = Self::query(&st);
            let typed = st.ed.buf.trim().to_owned();
            let area = egui::Area::new(Id::new("category_list"))
                .order(egui::Order::Foreground)
                .fixed_pos(rect.left_bottom() + Vec2::new(0.0, 2.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_min_width(rect.width().max(280.0));
                        egui::ScrollArea::vertical()
                            .max_height(260.0)
                            .show(ui, |ui| {
                                if list.is_empty() && !create {
                                    ui.label(RichText::new(self.tr("No match")).weak());
                                }
                                for (k, (cat, idx)) in list.iter().enumerate() {
                                    let label = &self.cats[*cat].path;
                                    let job = self.highlighted(ui, label, idx);
                                    let mut r = ui.selectable_label(hl == Some(k), job);
                                    if self.t.rank {
                                        r = r.on_hover_text(format!(
                                            "used {} times",
                                            self.cats[*cat].uses
                                        ));
                                    }
                                    if moved && hl == Some(k) {
                                        r.scroll_to_me(None);
                                    }
                                    if pressed(ui, &r) {
                                        clicked = Some(Outcome::Pick(*cat));
                                    }
                                }
                                if create {
                                    let r = ui.selectable_label(
                                        hl == Some(list.len()),
                                        format!("{} “{typed}”…", self.tr("Create")),
                                    );
                                    if pressed(ui, &r) {
                                        clicked = Some(Outcome::Create(typed.clone()));
                                    }
                                }
                            });
                    });
                });
            st.popup = Some(area.response.rect);
            self.h.region(area.response.rect, "category list");
            if query.is_empty() && st.all {
                self.h.region(area.response.rect, "every category");
            }
            if let Some(o) = clicked {
                result = Some(o);
                leave = None;
                st.open = false;
                st.all = false;
            }
        } else {
            st.popup = None;
        }
        if leave.is_none() && result.is_none() && lost {
            leave = Some(Leave::Blur);
        }
        match leave {
            Some(Leave::Esc) => {
                st.ed.buf = self.cat_text();
                st.ed.dirty = false;
                st.ed.error = None;
                st.moved = false;
                st.open = false;
                self.h.event("field_revert", json!({ "field": "category" }));
                self.finish_leave(ui, field, Leave::Esc, true);
            }
            Some(l) => {
                let outcome = result
                    .take()
                    .or_else(|| self.resolve(&st, &list, offer_create));
                st.open = false;
                st.all = false;
                let ok = outcome.is_none_or(|o| self.apply_cat_outcome(&mut st, o));
                self.finish_leave(ui, field, l, ok);
            }
            None => {
                if let Some(o) = result {
                    // A click in the list commits the cell and leaves the focus in it.
                    self.apply_cat_outcome(&mut st, o);
                    ui.memory_mut(|m| m.request_focus(id));
                }
            }
        }
        if let Some(e) = st.ed.error.clone() {
            self.note(ui, &resp, "category", &e, true);
        }
        self.combo = st;
    }

    fn touch_cat(&mut self, c: usize) {
        self.use_clock += 1;
        self.cats[c].uses += 1;
        self.cats[c].last_used = self.use_clock;
    }

    /// Applies what leaving the cell meant. False keeps the cell open with an error.
    fn apply_cat_outcome(&mut self, st: &mut Combo, out: Outcome) -> bool {
        let desc = match &out {
            Outcome::Pick(c) => json!({ "outcome": "pick", "category": self.cats[*c].path }),
            Outcome::Create(t) => json!({ "outcome": "create", "text": t }),
            Outcome::Unknown(t) => json!({ "outcome": "unknown", "text": t }),
            Outcome::Clear => json!({ "outcome": "clear" }),
        };
        self.h.event(
            "field_commit",
            json!({ "field": "category", "result": desc }),
        );
        st.moved = false;
        st.hl = None;
        match out {
            Outcome::Pick(c) => {
                self.row.cat = Some(c);
                st.ed.buf = self.cats[c].path.clone();
                st.ed.error = None;
                st.ed.dirty = false;
                true
            }
            Outcome::Clear => {
                self.row.cat = None;
                st.ed.dirty = false;
                true
            }
            Outcome::Create(name) => {
                // Real app: opens the New Category Dialog prefilled with the name.
                self.emit(
                    &format!("OpenDialog(NewCategory {{ name: {name:?} }})"),
                    "category combo",
                );
                st.ed.dirty = false;
                true
            }
            Outcome::Unknown(text) => {
                st.ed.error = Some(self.tr1("No category “{}”. Pick one from the list.", &text));
                st.ed.dirty = true;
                false
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // The Register: entered rows, then the draft row

    fn save_row(&mut self) {
        let (Some(date), Some(amount)) = (self.row.date, self.row.amount) else {
            self.status = self.tr("Date and amount are required");
            self.h.event(
                "row_not_saved",
                json!({ "reason": "date or amount missing" }),
            );
            return;
        };
        let cat_path = self.cat_text();
        self.emit(
            &format!(
                "AddTransaction {{ date: {}, payee: {:?}, category: {:?}, amount: {} {} }}",
                date.iso(),
                self.row.payee,
                cat_path,
                amount.amount(),
                amount.currency()
            ),
            "draft row",
        );
        if let Some(c) = self.row.cat {
            self.touch_cat(c);
        }
        self.entered.push(Entered {
            date,
            payee: self.row.payee.clone(),
            cat: cat_path,
            amount,
        });
        // A fresh draft row keeps the date, as a register does for runs of entries.
        self.reset_row(true);
        self.row.date = Some(date);
        self.pending_focus = Some(Field::Date);
    }

    fn main_panel(&mut self, ui: &mut Ui) {
        CentralPanel::default().show(ui, |ui| {
            ui.heading("Register");
            ui.label(
                RichText::new(format!(
                    "{} · {} · {} ({} dp)",
                    self.loc().tag(),
                    self.variant.label(),
                    self.code(),
                    self.dp()
                ))
                .small()
                .weak(),
            );
            ui.add_space(8.0);
            let f = self.f();
            let grid = egui::Grid::new("register")
                .striped(self.theme.alternating_rows())
                .min_row_height(self.theme.row_height())
                .show(ui, |ui| {
                    for h in ["Date", "Payee", "Category", "Amount"] {
                        ui.label(RichText::new(self.tr(h)).strong());
                    }
                    ui.end_row();
                    let start = self.entered.len().saturating_sub(12);
                    for e in &self.entered[start..] {
                        ui.label(fmt_date(e.date, f));
                        ui.label(&e.payee);
                        ui.label(&e.cat);
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            let text = f.l10n.format_amount(
                                e.amount.amount(),
                                i18n_currency(e.amount.currency()),
                                AmountStyle::Bare,
                            );
                            ui.label(self.theme.amount(&e.amount, text));
                        });
                        ui.end_row();
                    }
                    // The draft row.
                    ui.vertical(|ui| self.date_field(ui));
                    ui.vertical(|ui| self.payee_field(ui));
                    ui.vertical(|ui| self.cat_field(ui));
                    ui.vertical(|ui| self.money_field(ui));
                    ui.end_row();
                });
            self.h.region(grid.response.rect, "register");
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Enter: commit, move right (on Amount: add) · Tab / Shift+Tab: commit, next / previous · Esc: cancel the cell · Ctrl+Enter: add the row",
                )
                .small()
                .weak(),
            );
            if self.save_requested {
                self.save_requested = false;
                self.save_row();
            }
            ui.add_space(12.0);
            let tail = ui
                .vertical(|ui| {
                    ui.label(RichText::new("Commands the real app would emit").strong());
                    for line in &self.emitted {
                        ui.label(RichText::new(line).small().monospace());
                    }
                })
                .response;
            self.h.region(tail.rect, "commands");
        });
    }

    fn dock(&mut self, ui: &mut Ui) {
        let frame = {
            let v = ui.visuals();
            egui::Frame::side_top_panel(ui.style())
                .fill(v.warn_fg_color.gamma_multiply(0.10).blend(v.panel_fill))
                .stroke(egui::Stroke::new(1.0, v.warn_fg_color))
        };
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
            .exact_size(320.0)
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
                    let r =
                        ui.selectable_label(self.variant == v, format!("{hint}   {}", v.label()));
                    self.h.track(&r, format!("dock: variant {}", v.key()));
                    if r.clicked() {
                        self.set_variant(v, "click");
                    }
                }
                ui.separator();
                ui.label(RichText::new("Toggles").strong());
                for (&(key, label, scope), (_, hint)) in TOGGLES.iter().zip(TOGGLE_KEYS) {
                    if CYCLING.contains(&key) {
                        let value = self.t.describe(key);
                        let value = value.as_str().unwrap_or_default();
                        let r = ui.button(format!("{hint}   {label}: {value}  [{scope}]"));
                        self.h.track(&r, format!("dock: toggle {key}"));
                        if r.clicked() {
                            self.set_toggle(key, None, "click");
                        }
                        continue;
                    }
                    let mut value = self.t.flag(key).is_some_and(|v| *v);
                    let r = ui.checkbox(&mut value, format!("{hint}   {label}  [{scope}]"));
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
                        "A B C or ⏴/⏵ variant · 1–0 toggles (while no field has focus) · F2 note · F3 marker · F4 hide dock · F5 spotlight",
                    )
                    .small()
                    .weak(),
                );
                ui.add_space(6.0);
                ui.label(RichText::new(&self.status).small().monospace());
            });
    }
}

impl eframe::App for Proto {
    /// The draft row owns Tab while one of its cells has focus; egui would otherwise move
    /// focus to the next widget (a button, the dock) before the prototype sees the key.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        let Some(focused) = ctx.memory(|m| m.focused()) else {
            return;
        };
        if !Field::ALL.iter().any(|f| f.id() == focused) {
            return;
        }
        raw.events.retain(|e| match e {
            Event::Key {
                key: Key::Tab,
                pressed,
                modifiers,
                ..
            } => {
                if *pressed {
                    self.pending_tab = Some(modifiers.shift);
                }
                false
            }
            _ => true,
        });
    }

    fn on_exit(&mut self) {
        self.h.event("session_end", json!({}));
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(cmd) = self.h.begin_frame(&ctx) {
            self.apply_control(&ctx, &cmd);
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
            for (&(key, _, _), (k, _)) in TOGGLES.iter().zip(TOGGLE_KEYS) {
                if ctx.input(|i| i.key_pressed(k) && i.modifiers.is_none()) {
                    self.set_toggle(key, None, "key");
                }
            }
        }
        if ctx.input(|i| i.key_pressed(Key::F4)) {
            self.set_dock(!self.dock_open, "key");
        }
        self.dock(ui);
        self.main_panel(ui);
        self.h.end_frame(&ctx);
    }
}

fn main() -> eframe::Result {
    let args = Args::from_env();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Balanced — PROTOTYPE input fields")
            .with_inner_size([1400.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "balanced_input_fields_prototype",
        options,
        Box::new(move |cc| {
            let proto = Proto::new(&args);
            let style = proto.theme.style();
            cc.egui_ctx.set_style_of(egui::Theme::Dark, style.clone());
            cc.egui_ctx.set_style_of(egui::Theme::Light, style);
            cc.egui_ctx.set_theme(egui::Theme::Light);
            Ok(Box::new(proto))
        }),
    )
}
