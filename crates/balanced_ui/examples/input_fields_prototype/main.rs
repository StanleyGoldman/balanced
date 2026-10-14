//! PROTOTYPE, throwaway: money, date and category input fields.
//!
//! Question: how should money, date and category input fields behave? Locale-aware money entry
//! (parse/format per the money and i18n decisions, plus the calculator question #14 left open),
//! a date field with a picker and keyboard shortcuts, and a type-to-filter category combo box,
//! each tried in `en-US`, `de-DE` and the pseudo-locale `en-XA`.
//!
//! The three fields sit in a mock Register edit row. The variants A–C differ in interaction model:
//! A is typed text with quick-fill, B is dropdown-assisted, C is structured (segments, cents-first
//! money, two-level category).
//!
//! Run:
//!   cargo run -p balanced_ui --example input_fields_prototype -- \
//!     [--variant a|b|c] [--log <dir>/events.jsonl] [--control <dir>/control.txt]
//!
//! The PROTOTYPE dock on the right holds the variant switcher and toggles; F4 collapses it.
//! A–C or ← / → switch variant (while no field has focus), 1–0 flip toggles, F2 note, F3 marker.
//!
//! Fixed-seed stub data, a fixed "today" (2026-10-14), hand-rolled locale tables (ICU4X and
//! `balanced_i18n` are not wired yet). None of this is production code.

#![allow(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::pedantic,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::wildcard_enum_match_arm,
    clippy::unwrap_used,
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
    missing_debug_implementations,
    unreachable_pub
)]

use std::collections::{HashMap, VecDeque};

use balanced_prototype_harness::{Args, Harness};
use eframe::egui;
use egui::{
    Align, CentralPanel, Event, Id, Key, Modifiers, Panel, Rect, RichText, Sense, Sides, Ui, Vec2,
};
use serde_json::json;

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
}

// ---------------------------------------------------------------------------------------------
// Locales and currencies (hand-rolled stand-ins for ICU4X)

#[derive(Clone, Copy, PartialEq, Eq)]
enum Loc {
    EnUs,
    DeDe,
    /// The pseudo-locale: `en-US` text through a transform, formatting as `de-DE` (the default).
    EnXa,
}

const LOCS: [Loc; 3] = [Loc::EnUs, Loc::DeDe, Loc::EnXa];

struct Fmt {
    dec: char,
    grp: char,
    mdy: bool,
    sep: char,
    sunday_first: bool,
    sym_after: bool,
    pattern: &'static str,
    months: [&'static str; 12],
    days: [&'static str; 7],
    day_names: [&'static str; 7],
}

const EN: Fmt = Fmt {
    dec: '.',
    grp: ',',
    mdy: true,
    sep: '/',
    sunday_first: true,
    sym_after: false,
    pattern: "MM/DD/YYYY",
    months: [
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
    ],
    days: ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
    day_names: [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ],
};

const DE: Fmt = Fmt {
    dec: ',',
    grp: '.',
    mdy: false,
    sep: '.',
    sunday_first: false,
    sym_after: true,
    pattern: "TT.MM.JJJJ",
    months: [
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
    ],
    days: ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"],
    day_names: [
        "Montag",
        "Dienstag",
        "Mittwoch",
        "Donnerstag",
        "Freitag",
        "Samstag",
        "Sonntag",
    ],
};

impl Loc {
    fn tag(self) -> &'static str {
        match self {
            Self::EnUs => "en-US",
            Self::DeDe => "de-DE",
            Self::EnXa => "en-XA",
        }
    }

    fn fmt(self) -> &'static Fmt {
        match self {
            Self::EnUs => &EN,
            Self::DeDe | Self::EnXa => &DE,
        }
    }
}

struct Cur {
    code: &'static str,
    sym: &'static str,
    dp: u32,
}

const CURS: [Cur; 4] = [
    Cur {
        code: "USD",
        sym: "$",
        dp: 2,
    },
    Cur {
        code: "EUR",
        sym: "€",
        dp: 2,
    },
    Cur {
        code: "JPY",
        sym: "¥",
        dp: 0,
    },
    Cur {
        code: "BHD",
        sym: "BD",
        dp: 3,
    },
];

/// Amounts are held as micro-units (6 decimals) so the calculator has room; committed amounts are
/// always rounded to the currency's decimal places.
const MICRO: i128 = 1_000_000;

fn round_dp(v: i128, dp: u32) -> i128 {
    let unit = 10_i128.pow(6 - dp);
    let q = v.div_euclid(unit);
    let r = v.rem_euclid(unit);
    let q = match (r * 2).cmp(&unit) {
        std::cmp::Ordering::Less => q,
        std::cmp::Ordering::Greater => q + 1,
        std::cmp::Ordering::Equal => q + (q & 1),
    };
    q * unit
}

fn group_digits(digits: &str, sep: char) -> String {
    let chars: Vec<char> = digits.chars().collect();
    let mut out = String::new();
    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            out.push(sep);
        }
        out.push(*c);
    }
    out
}

/// A bare number at the currency's decimal places (what a Register cell shows).
fn fmt_amount(v: i128, dp: u32, f: &Fmt) -> String {
    let v = round_dp(v, dp);
    let minor = v.abs() / 10_i128.pow(6 - dp);
    let scale = 10_i128.pow(dp);
    let int = group_digits(&(minor / scale).to_string(), f.grp);
    let mut out = String::new();
    if v < 0 {
        out.push('-');
    }
    out.push_str(&int);
    if dp > 0 {
        out.push(f.dec);
        out.push_str(&format!("{:0width$}", minor % scale, width = dp as usize));
    }
    out
}

/// The amount with its currency symbol, placed per locale (what the Account bar would show).
fn fmt_with_symbol(v: i128, cur: &Cur, f: &Fmt) -> String {
    let n = fmt_amount(v, cur.dp, f);
    if f.sym_after {
        format!("{n}\u{a0}{}", cur.sym)
    } else if let Some(rest) = n.strip_prefix('-') {
        format!("-{}{rest}", cur.sym)
    } else {
        format!("{}{n}", cur.sym)
    }
}

enum PErr {
    Empty,
    Invalid,
    Decimals(u32),
}

/// Strict, locale-aware: the locale's decimal mark and group separator, a leading or trailing
/// minus, a surrounding currency symbol or code (ignored). More decimals than `max_dp` fail.
fn parse_plain(input: &str, f: &Fmt, cur: &Cur, max_dp: u32) -> Result<i128, PErr> {
    let mut s = input.trim().to_owned();
    if s.is_empty() {
        return Err(PErr::Empty);
    }
    let mut neg = false;
    for _ in 0..2 {
        for tok in [cur.sym, cur.code] {
            s = s.trim_start_matches(tok).trim().to_owned();
            s = s.trim_end_matches(tok).trim().to_owned();
        }
        if let Some(rest) = s.strip_prefix(['-', '−']) {
            neg = true;
            s = rest.trim().to_owned();
        } else if let Some(rest) = s.strip_prefix('+') {
            s = rest.trim().to_owned();
        }
        if let Some(rest) = s.strip_suffix(['-', '−']) {
            neg = true;
            s = rest.trim().to_owned();
        }
    }
    let parts: Vec<&str> = s.split(f.dec).collect();
    if parts.len() > 2 {
        return Err(PErr::Invalid);
    }
    let int = parts[0];
    let frac = parts.get(1).copied().unwrap_or("");
    if int.is_empty() && frac.is_empty() {
        return Err(PErr::Invalid);
    }
    let groups: Vec<&str> = int.split(f.grp).collect();
    let group_ok = groups.iter().enumerate().all(|(i, g)| {
        g.chars().all(|c| c.is_ascii_digit())
            && if groups.len() == 1 {
                true
            } else if i == 0 {
                (1..=3).contains(&g.len())
            } else {
                g.len() == 3
            }
    });
    if !group_ok || !frac.chars().all(|c| c.is_ascii_digit()) {
        return Err(PErr::Invalid);
    }
    if frac.len() as u32 > max_dp {
        return Err(PErr::Decimals(max_dp));
    }
    let digits: String = groups.concat();
    let int_v: i128 = if digits.is_empty() {
        0
    } else {
        digits.parse().map_err(|_| PErr::Invalid)?
    };
    let frac_v: i128 = if frac.is_empty() {
        0
    } else {
        format!("{frac:0<6}").parse().map_err(|_| PErr::Invalid)?
    };
    let v = int_v * MICRO + frac_v;
    Ok(if neg { -v } else { v })
}

fn looks_like_expression(s: &str) -> bool {
    let t = s.trim();
    t.contains(['+', '*', '/'])
        || t.char_indices().any(|(i, c)| {
            (c == '-' || c == '−') && i > 0 && !t[i + c.len_utf8()..].trim().is_empty()
        })
}

/// `+ - * /` with the usual precedence and unary minus. Operands may carry up to six decimals.
fn eval_expression(s: &str, f: &Fmt, cur: &Cur) -> Result<i128, PErr> {
    let mut nums: Vec<i128> = Vec::new();
    let mut ops: Vec<char> = Vec::new();
    let mut tok = String::new();
    let mut expect_operand = true;
    let flush = |tok: &mut String, nums: &mut Vec<i128>| -> Result<(), PErr> {
        if tok.trim().is_empty() {
            return Err(PErr::Invalid);
        }
        nums.push(parse_plain(tok, f, cur, 6)?);
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
                vals.push(last * n / MICRO);
            }
            '/' => {
                if *n == 0 {
                    return Err(PErr::Invalid);
                }
                let last = vals.pop().unwrap();
                vals.push(last * MICRO / n);
            }
            _ => {
                adds.push(*op);
                vals.push(*n);
            }
        }
    }
    let mut total = vals[0];
    for (op, v) in adds.iter().zip(vals.iter().skip(1)) {
        if *op == '+' {
            total += v;
        } else {
            total -= v;
        }
    }
    Ok(total)
}

fn trim_micro(v: i128, f: &Fmt) -> String {
    let s = fmt_amount(v, 6, f);
    let s = s.trim_end_matches('0');
    s.trim_end_matches(f.dec).to_owned()
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
    if f.mdy {
        format!("{:02}{}{:02}{}{}", d.m, f.sep, d.d, f.sep, d.y)
    } else {
        format!("{:02}{}{:02}{}{}", d.d, f.sep, d.m, f.sep, d.y)
    }
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
    ("Parent", "Oberkategorie"),
    ("Subcategory", "Unterkategorie"),
    ("Amount", "Betrag"),
    ("Today", "Heute"),
    ("Save row", "Zeile speichern"),
    ("Entered rows", "Erfasste Zeilen"),
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
    name: String,
    parent: Option<usize>,
    uses: u32,
    last_used: u32,
}

fn generate_cats() -> Vec<Cat> {
    let mut rng = Lcg(SEED);
    let mut cats: Vec<Cat> = Vec::new();
    let mut parents = Vec::new();
    for (p, _) in CAT_TREE {
        parents.push(cats.len());
        cats.push(Cat {
            path: (*p).to_owned(),
            name: (*p).to_owned(),
            parent: None,
            uses: (rng.next() % 40) as u32,
            last_used: 0,
        });
    }
    for ((p, kids), pi) in CAT_TREE.iter().zip(&parents) {
        for k in *kids {
            cats.push(Cat {
                path: format!("{p}:{k}"),
                name: (*k).to_owned(),
                parent: Some(*pi),
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

struct Item {
    cat: usize,
    label: String,
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
            Self::A => "A · Typed: free text, quick-fill, shortcuts",
            Self::B => "B · Assisted: dropdowns, calendar, floating hints",
            Self::C => "C · Structured: segments, cents-first, two-level",
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

/// (key, label, variants it applies to). `locale`, `currency` and `match` cycle.
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
    ("preselect", "Preselect the top category match", "B C"),
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
    preselect: bool,
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
            "preselect" => &mut self.preselect,
            _ => return None,
        })
    }

    fn describe(&mut self, key: &str) -> serde_json::Value {
        match key {
            "locale" => json!(LOCS[self.locale].tag()),
            "currency" => json!(CURS[self.currency].code),
            "match" => json!(MatchMode::ALL[self.match_mode].name()),
            _ => json!(self.flag(key).map(|v| *v)),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// App state

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
    /// Text the user typed, without any quick-fill remainder (variant A).
    typed: String,
    open: bool,
    hl: Option<usize>,
    /// The highlight was moved with the arrow keys.
    moved: bool,
    popup: Option<Rect>,
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
    amount: Option<i128>,
}

struct Entered {
    date: Date,
    payee: String,
    cat: String,
    amount: i128,
    cur: usize,
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
    variant: Variant,
    t: Toggles,
    dock_open: bool,
    status: String,
    cats: Vec<Cat>,
    use_clock: u32,
    row: Row,
    entered: Vec<Entered>,
    emitted: VecDeque<String>,
    date_ed: TextEd,
    money_ed: TextEd,
    combos: HashMap<&'static str, Combo>,
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
    pending_focus: Option<String>,
    pending_blur: bool,
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

struct BoxOut {
    resp: egui::Response,
    lost: bool,
    enter: bool,
    esc: bool,
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
        let cats = generate_cats();
        let rent = cats.iter().position(|c| c.path == "Home:Rent");
        Self {
            h,
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
                preselect: true,
            },
            dock_open: true,
            status: "Ready".to_owned(),
            cats,
            use_clock: 100,
            row: Row {
                date: Some(Date {
                    y: 2026,
                    m: 10,
                    d: 1,
                }),
                payee: "Maple Property Mgmt".to_owned(),
                cat: rent,
                amount: Some(-1_850 * MICRO),
            },
            entered: Vec::new(),
            emitted: VecDeque::new(),
            date_ed: TextEd::default(),
            money_ed: TextEd::default(),
            combos: HashMap::new(),
            segs: [None; 3],
            seg: 0,
            seg_buf: String::new(),
            seg_error: None,
            cents: String::new(),
            neg: false,
            pick: None,
            pick_rect: None,
            pending_focus: None,
            pending_blur: false,
            save_requested: false,
        }
    }

    fn loc(&self) -> Loc {
        LOCS[self.t.locale]
    }

    fn f(&self) -> &'static Fmt {
        self.loc().fmt()
    }

    fn cur(&self) -> &'static Cur {
        &CURS[self.t.currency]
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

    fn amt(&self, v: Option<i128>) -> String {
        v.map_or_else(String::new, |v| fmt_amount(v, self.cur().dp, self.f()))
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
        self.combos.clear();
        self.h.screenshot(format!("variant-{}", to.key()));
    }

    fn set_toggle(&mut self, key: &str, value: Option<bool>, via: &str) {
        match key {
            "locale" | "currency" | "match" => {
                let (len, slot) = match key {
                    "locale" => (LOCS.len(), &mut self.t.locale),
                    "currency" => (CURS.len(), &mut self.t.currency),
                    _ => (MatchMode::ALL.len(), &mut self.t.match_mode),
                };
                *slot = (*slot + 1) % len;
                if key == "currency" {
                    // Committed amounts always carry exactly the currency's decimal places.
                    let dp = CURS[self.t.currency].dp;
                    self.row.amount = self.row.amount.map(|v| round_dp(v, dp));
                    self.cents.clear();
                }
            }
            _ => {
                let Some(slot) = self.t.flag(key) else {
                    self.h.event("control_unknown", json!({ "toggle": key }));
                    return;
                };
                *slot = value.unwrap_or(!*slot);
            }
        }
        let v = self.t.describe(key);
        self.h
            .event("toggle", json!({ "key": key, "value": v, "via": via }));
        self.h
            .screenshot(format!("toggle-{key}-{}", v.to_string().trim_matches('"')));
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
                    .map_or((rest, None), |(k, v)| (k, Some(v.trim() == "on")));
                self.set_toggle(key, value, "control");
            }
            "locale" => match LOCS.iter().position(|l| l.tag() == rest) {
                Some(i) => {
                    self.t.locale = i;
                    self.h.event(
                        "toggle",
                        json!({ "key": "locale", "value": rest, "via": "control" }),
                    );
                    self.h.screenshot(format!("toggle-locale-{rest}"));
                }
                None => self.h.event("control_unknown", json!({ "command": cmd })),
            },
            "currency" => match CURS.iter().position(|c| c.code == rest) {
                Some(i) => {
                    self.t.currency = (i + CURS.len() - 1) % CURS.len();
                    self.set_toggle("currency", None, "control");
                }
                None => self.h.event("control_unknown", json!({ "command": cmd })),
            },
            "focus" => self.pending_focus = Some(rest.to_owned()),
            "blur" => self.pending_blur = true,
            "set" => {
                let (field, text) = rest.split_once(' ').unwrap_or((rest, ""));
                self.control_set(field, text);
            }
            "picker" => {
                if rest == "open" {
                    self.pick = Some(self.row.date.unwrap_or(TODAY));
                } else {
                    self.pick = None;
                }
                self.h.event(
                    "date_popup",
                    json!({ "open": rest == "open", "via": "control" }),
                );
            }
            "reset" => {
                self.row = if rest == "blank" {
                    Row {
                        date: None,
                        payee: String::new(),
                        cat: None,
                        amount: None,
                    }
                } else {
                    Row {
                        date: Some(Date {
                            y: 2026,
                            m: 10,
                            d: 1,
                        }),
                        payee: "Maple Property Mgmt".to_owned(),
                        cat: self.cats.iter().position(|c| c.path == "Home:Rent"),
                        amount: Some(-1_850 * MICRO),
                    }
                };
                self.cents.clear();
                self.h.event("reset", json!({ "row": rest }));
            }
            "screenshot" => self
                .h
                .screenshot(if rest.is_empty() { "control" } else { rest }),
            "dock" => self.set_dock(rest != "off", "control"),
            "reload" => {
                self.cats = generate_cats();
                self.entered.clear();
                self.h.event("reload", json!({ "seed": SEED }));
            }
            "quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            _ => self.h.event("control_unknown", json!({ "command": cmd })),
        }
    }

    /// Put `text` into a field as if typed, without committing it.
    fn control_set(&mut self, field: &str, text: &str) {
        self.pending_focus = Some(field.to_owned());
        match field {
            "date" => {
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
            "amount" => {
                if self.variant == Variant::C {
                    self.cents = text.chars().filter(char::is_ascii_digit).collect();
                    self.neg = text.contains('-');
                } else {
                    self.money_ed.buf = text.to_owned();
                    self.money_ed.dirty = true;
                    self.money_ed.error = None;
                }
            }
            "payee" => self.row.payee = text.to_owned(),
            "category" | "child" => {
                let key = match (self.variant, field) {
                    (Variant::A, _) => "cat_field",
                    (Variant::B, _) => "cat_field",
                    (Variant::C, "child") => "cat_child",
                    (Variant::C, _) => "cat_parent",
                };
                let st = self.combos.entry(key).or_default();
                st.ed.buf = text.to_owned();
                st.typed = text.to_owned();
                st.ed.dirty = true;
                st.ed.error = None;
                st.open = true;
                st.hl = None;
            }
            _ => self.h.event("control_unknown", json!({ "set": field })),
        }
        self.h
            .event("control_set", json!({ "field": field, "text": text }));
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
        let focused = ui.memory(|m| m.has_focus(id));
        let enter = focused && ui.input(|i| i.key_pressed(Key::Enter));
        let esc = focused && ui.input(|i| i.key_pressed(Key::Escape));
        let mut te = egui::TextEdit::singleline(&mut ed.buf)
            .id(id)
            .desired_width(width)
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
        BoxOut {
            resp,
            lost,
            enter,
            esc,
        }
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
    // Money

    /// Returns the value and a note (e.g. that a calculator result was rounded).
    fn parse_money(&self, raw: &str) -> Result<(Option<i128>, Option<String>), String> {
        let (f, cur) = (self.f(), self.cur());
        let bad = |e: PErr| match e {
            PErr::Empty => unreachable!(),
            PErr::Invalid => self
                .tr("“{}” isn't an amount here. Decimal mark: “{}”, group separator: “{}”.")
                .replacen("{}", raw.trim(), 1)
                .replacen("{}", &f.dec.to_string(), 1)
                .replacen("{}", &f.grp.to_string(), 1),
            PErr::Decimals(dp) => self
                .tr("{} amounts have {} decimal places")
                .replacen("{}", cur.code, 1)
                .replacen("{}", &dp.to_string(), 1),
        };
        if raw.trim().is_empty() {
            return Ok((None, None));
        }
        if self.t.calc && self.variant != Variant::C && looks_like_expression(raw) {
            return match eval_expression(raw, f, cur) {
                Ok(v) => {
                    let r = round_dp(v, cur.dp);
                    let note = (r != v).then(|| self.tr1("rounded from {}", &trim_micro(v, f)));
                    Ok((Some(r), note))
                }
                Err(PErr::Empty) => Ok((None, None)),
                Err(e) => Err(bad(e)),
            };
        }
        match parse_plain(raw, f, cur, cur.dp) {
            Ok(v) => Ok((Some(v), None)),
            Err(PErr::Empty) => Ok((None, None)),
            Err(e) => Err(bad(e)),
        }
    }

    fn commit_money(&mut self, ed: &mut TextEd, id: Id, ui: &mut Ui) -> bool {
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
                true
            }
            Err(msg) => {
                self.h.event(
                    "field_commit",
                    json!({ "field": "amount", "raw": raw, "outcome": "error", "message": msg }),
                );
                ed.error = Some(msg);
                ui.memory_mut(|m| m.request_focus(id));
                false
            }
        }
    }

    fn money_field(&mut self, ui: &mut Ui) {
        let id = ui.make_persistent_id("amount_field");
        if self.pending_focus.as_deref() == Some("amount") {
            ui.memory_mut(|m| m.request_focus(id));
            self.pending_focus = None;
        }
        if self.pending_blur && ui.memory(|m| m.has_focus(id)) {
            ui.memory_mut(|m| m.surrender_focus(id));
            self.pending_blur = false;
        }
        if self.variant == Variant::C {
            self.money_cents(ui, id);
            return;
        }
        let focused = ui.memory(|m| m.has_focus(id));
        let mut ed = std::mem::take(&mut self.money_ed);
        if !focused && ed.error.is_none() {
            ed.buf = self.amt(self.row.amount);
            ed.dirty = false;
        }
        let out = self.edit_box(ui, &mut ed, id, "amount field", 150.0, true, "0", None);
        let mut saved = false;
        if out.lost {
            if out.esc {
                ed.buf = self.amt(self.row.amount);
                ed.dirty = false;
                ed.error = None;
                self.h.event("field_revert", json!({ "field": "amount" }));
                if !ed.dirty {
                    self.emit("CancelRowEdit", "Esc in amount");
                }
            } else {
                saved = self.commit_money(&mut ed, id, ui) && out.enter;
            }
        }
        let msg = if let Some(e) = ed.error.clone() {
            Some((e, true))
        } else if self.t.preview && out.resp.has_focus() && ed.dirty {
            Some(match self.parse_money(&ed.buf) {
                Ok((Some(v), note)) => {
                    let mut s = format!(
                        "= {} {}",
                        fmt_with_symbol(v, self.cur(), self.f()),
                        self.cur().code
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
        self.save_requested |= saved;
    }

    fn cents_value(&self) -> Option<i128> {
        if self.cents.is_empty() {
            return None;
        }
        let minor: i128 = self.cents.parse().unwrap_or(0);
        let v = minor * 10_i128.pow(6 - self.cur().dp);
        Some(if self.neg { -v } else { v })
    }

    fn money_cents(&mut self, ui: &mut Ui, id: Id) {
        let focused = ui.memory(|m| m.has_focus(id));
        let width = 150.0;
        let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::click());
        resp.id = id;
        let resp = ui.interact(rect, id, Sense::click());
        self.h.track(&resp, "amount field");
        if resp.clicked() {
            ui.memory_mut(|m| m.request_focus(id));
        }
        let focused_now = ui.memory(|m| m.has_focus(id));
        if focused_now && !focused {
            let a = self.row.amount.unwrap_or(0);
            self.neg = a < 0;
            self.cents = self.row.amount.map_or_else(String::new, |v| {
                (v.abs() / 10_i128.pow(6 - self.cur().dp)).to_string()
            });
        }
        let mut esc = false;
        let mut enter = false;
        if focused_now {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    egui::EventFilter {
                        tab: false,
                        horizontal_arrows: false,
                        vertical_arrows: false,
                        escape: false,
                    },
                );
            });
            let (mut cents, mut neg) = (std::mem::take(&mut self.cents), self.neg);
            ui.input_mut(|i| {
                i.events.retain(|e| match e {
                    Event::Text(s) => {
                        for c in s.chars() {
                            match c {
                                '0'..='9' if cents.len() < 12 => {
                                    if !(cents.is_empty() && c == '0') {
                                        cents.push(c);
                                    } else {
                                        cents.push('0');
                                        cents = cents.trim_start_matches('0').to_owned();
                                        if cents.is_empty() {
                                            cents.push('0');
                                        }
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
                esc = i.key_pressed(Key::Escape);
                enter = i.key_pressed(Key::Enter);
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
        if focused_now && esc {
            ui.memory_mut(|m| m.surrender_focus(id));
            self.h.event("field_revert", json!({ "field": "amount" }));
        } else if focused && !focused_now || (focused_now && enter) {
            let v = self.cents_value();
            self.row.amount = v;
            self.h.event(
                "field_commit",
                json!({ "field": "amount", "raw": self.cents, "outcome": "ok", "value": self.amt(v) }),
            );
            if enter {
                self.save_requested = true;
            }
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

    fn date_hint(&self) -> String {
        format!(
            "{} = {}",
            self.f().pattern,
            fmt_date(
                Date {
                    y: 2026,
                    m: 10,
                    d: 5
                },
                self.f()
            )
        )
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
            .event("date_pick", json!({ "date": fmt_date(d, &EN), "via": via }));
    }

    fn commit_date(&mut self, ed: &mut TextEd, id: Id, ui: &mut Ui) -> bool {
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
                let msg = self
                    .tr("Not a date. Expected {}, for example {}.")
                    .replacen("{}", self.f().pattern, 1)
                    .replacen(
                        "{}",
                        &fmt_date(
                            Date {
                                y: 2026,
                                m: 10,
                                d: 5,
                            },
                            self.f(),
                        ),
                        1,
                    );
                self.h.event(
                    "field_commit",
                    json!({ "field": "date", "raw": raw, "outcome": "error", "message": msg }),
                );
                ed.error = Some(msg);
                ui.memory_mut(|m| m.request_focus(id));
                false
            }
        }
    }

    fn date_field(&mut self, ui: &mut Ui) {
        let id = ui.make_persistent_id("date_field");
        if self.pending_focus.as_deref() == Some("date") {
            ui.memory_mut(|m| m.request_focus(id));
            self.pending_focus = None;
        }
        if self.pending_blur && ui.memory(|m| m.has_focus(id)) {
            ui.memory_mut(|m| m.surrender_focus(id));
            self.pending_blur = false;
        }
        let mut field_rect = Rect::NOTHING;
        let mut saved = false;
        let mut field_resp = None;
        ui.horizontal(|ui| {
            if self.variant == Variant::C {
                let r = self.date_segments(ui, id);
                field_rect = r.rect;
                field_resp = Some(r);
            } else {
                let focused = ui.memory(|m| m.has_focus(id));
                let mut ed = std::mem::take(&mut self.date_ed);
                if !focused && ed.error.is_none() {
                    ed.buf = self.date_text(self.row.date);
                    ed.dirty = false;
                }
                if focused && self.t.shortcuts {
                    let base = self
                        .row
                        .date
                        .unwrap_or(TODAY);
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
                            json!({ "key": key, "from": fmt_date(base, &EN), "to": fmt_date(nd, &EN) }),
                        );
                    }
                }
                let hint = self.date_hint();
                let out = self.edit_box(
                    ui,
                    &mut ed,
                    id,
                    "date field",
                    120.0,
                    false,
                    &hint,
                    self.pick_rect,
                );
                if out.lost {
                    if out.esc {
                        ed.buf = self.date_text(self.row.date);
                        ed.dirty = false;
                        ed.error = None;
                        self.pick = None;
                        self.h.event("field_revert", json!({ "field": "date" }));
                    } else {
                        saved = self.commit_date(&mut ed, id, ui) && out.enter;
                    }
                }
                field_rect = out.resp.rect;
                field_resp = Some(out.resp);
                self.date_ed = ed;
            }
            if self.t.picker {
                let b = ui.small_button(self.tr("Cal"));
                self.h.track(&b, "date picker button");
                if b.clicked() {
                    self.pick = if self.pick.is_some() {
                        None
                    } else {
                        Some(self.row.date.unwrap_or(TODAY))
                    };
                    self.h
                        .event("date_popup", json!({ "open": self.pick.is_some(), "via": "click" }));
                }
            }
        });
        // Alt+Down opens the picker from the field.
        let focused = ui.memory(|m| m.has_focus(id));
        if focused
            && self.t.picker
            && self.pick.is_none()
            && ui.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowDown))
        {
            self.pick = Some(self.row.date.unwrap_or(TODAY));
            self.h
                .event("date_popup", json!({ "open": true, "via": "alt+down" }));
        }
        if self.pick.is_some() && self.t.picker {
            self.calendar(ui, field_rect, id);
        } else {
            self.pick = None;
            self.pick_rect = None;
        }
        // Message line.
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
        self.save_requested |= saved;
    }

    /// Variant C: day, month and year as separate segments.
    fn date_segments(&mut self, ui: &mut Ui, id: Id) -> egui::Response {
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
        let mut esc = false;
        let mut enter = false;
        if focused {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    egui::EventFilter {
                        tab: false,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: false,
                    },
                );
            });
            esc = ui.input(|i| i.key_pressed(Key::Escape));
            enter = ui.input(|i| i.key_pressed(Key::Enter));
            if self.t.shortcuts && self.seg_buf.is_empty() {
                let base = self.row.date.unwrap_or(TODAY);
                if let Some((act, key)) = self.date_shortcuts(ui, true, false) {
                    let nd = self.apply_date_act(act, base);
                    self.row.date = Some(nd);
                    self.set_segs(Some(nd));
                    self.h.event(
                        "date_shortcut",
                        json!({ "key": key, "from": fmt_date(base, &EN), "to": fmt_date(nd, &EN) }),
                    );
                }
            }
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
                let n = (i64::from(cur - lo) + delta).rem_euclid(span) as u32 + lo;
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
                None => {
                    self.seg_error = Some(
                        self.tr("Not a date. Expected {}, for example {}.")
                            .replacen("{}", f.pattern, 1)
                            .replacen(
                                "{}",
                                &fmt_date(
                                    Date {
                                        y: 2026,
                                        m: 10,
                                        d: 5,
                                    },
                                    f,
                                ),
                                1,
                            ),
                    );
                }
            }
        }
        if focused && esc {
            self.set_segs(self.row.date);
            ui.memory_mut(|m| m.surrender_focus(id));
            self.h.event("field_revert", json!({ "field": "date" }));
        } else if (was_focused && !focused) || (focused && enter) {
            let err = self.seg_error.clone();
            if let Some(msg) = err {
                self.h.event(
                    "field_commit",
                    json!({ "field": "date", "raw": format!("{:?}", self.segs), "outcome": "error", "message": msg }),
                );
                ui.memory_mut(|m| m.request_focus(id));
            } else {
                if self.segs.iter().all(Option::is_none) {
                    self.row.date = None;
                }
                self.h.event(
                    "field_commit",
                    json!({ "field": "date", "raw": format!("{:?}", self.segs), "outcome": "ok", "value": self.date_text(self.row.date) }),
                );
                if enter {
                    self.save_requested = true;
                }
            }
        }
        resp
    }

    fn calendar(&mut self, ui: &mut Ui, field_rect: Rect, id: Id) {
        let f = self.f();
        let Some(cursor) = self.pick else { return };
        // Keys go to the calendar while it is open.
        let mut chosen = None;
        let mut close = false;
        let mut cur = cursor;
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
                chosen = Some((cur, "key"));
            }
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                close = true;
            }
        });
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
                        if ui.small_button("<").clicked() {
                            nav = -1;
                        }
                        ui.label(
                            RichText::new(format!("{} {}", f.months[cur.m as usize - 1], cur.y))
                                .strong(),
                        );
                        if ui.small_button(">").clicked() {
                            nav = 1;
                        }
                        if ui.small_button(&today_label).clicked() {
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
                                if r.clicked() {
                                    chosen = Some((d, "click"));
                                }
                            }
                        });
                    }
                });
            });
        self.pick_rect = Some(area.response.rect);
        if nav != 0 {
            cur = cur.add_months(nav);
        }
        if go_today {
            chosen = Some((TODAY, "today"));
        }
        // Click outside the calendar and the field closes it.
        let pressed = ui.input(|i| i.pointer.primary_pressed());
        if pressed
            && !over(ui, self.pick_rect)
            && !over(ui, Some(field_rect.expand2(Vec2::new(60.0, 0.0))))
        {
            close = true;
            self.h.event(
                "date_popup",
                json!({ "open": false, "via": "click-outside" }),
            );
        }
        if let Some((d, via)) = chosen {
            self.choose_date(d, via);
            ui.memory_mut(|m| m.request_focus(id));
        } else if close {
            self.pick = None;
        } else {
            self.pick = Some(cur);
        }
    }

    // -----------------------------------------------------------------------------------------
    // Category

    fn items_all(&self) -> Vec<Item> {
        self.cats
            .iter()
            .enumerate()
            .map(|(i, c)| Item {
                cat: i,
                label: c.path.clone(),
            })
            .collect()
    }

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

    fn ranked(&self, items: &[Item], q: &str) -> Vec<Matched> {
        let mode = MatchMode::ALL[self.t.match_mode];
        let mut out: Vec<(u32, (i64, String), Matched)> = items
            .iter()
            .enumerate()
            .filter_map(|(i, it)| {
                let (score, idx) = match_label(mode, q, &it.label)?;
                Some((score, self.rank_key(it.cat), (i, idx)))
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

    /// Decide what leaving the field means.
    fn resolve(
        &self,
        st: &Combo,
        list: &[Matched],
        items: &[Item],
        offer_create: bool,
    ) -> Option<Outcome> {
        let intent = st.ed.dirty || st.moved;
        if !intent {
            return None;
        }
        if let Some(h) = st.hl {
            if h < list.len() {
                return Some(Outcome::Pick(items[list[h].0].cat));
            }
            if offer_create {
                return Some(Outcome::Create(st.ed.buf.trim().to_owned()));
            }
        }
        let text = st.ed.buf.trim();
        if text.is_empty() {
            return Some(Outcome::Clear);
        }
        if let Some(it) = items.iter().find(|it| it.label.eq_ignore_ascii_case(text)) {
            return Some(Outcome::Pick(it.cat));
        }
        Some(if offer_create {
            Outcome::Create(text.to_owned())
        } else {
            Outcome::Unknown(text.to_owned())
        })
    }

    /// Type-to-filter combo: used by B (all categories) and twice by C (parent, child).
    /// `inline` is variant A: the first path-prefix match is completed in the field.
    fn combo(
        &mut self,
        ui: &mut Ui,
        key: &'static str,
        name: &str,
        items: &[Item],
        current: String,
        width: f32,
        offer_create: bool,
        inline: bool,
    ) -> Option<Outcome> {
        let id = ui.make_persistent_id(key);
        let mut st = self.combos.remove(key).unwrap_or_default();
        let mut result = None;
        let focused = ui.memory(|m| m.has_focus(id));
        let want_focus = self.pending_focus.as_deref().is_some_and(|f| {
            f == "category" && matches!(key, "cat_field" | "cat_parent")
                || (f == "child" && key == "cat_child")
        });
        if want_focus {
            ui.memory_mut(|m| m.request_focus(id));
            self.pending_focus = None;
        }
        if self.pending_blur && focused {
            ui.memory_mut(|m| m.surrender_focus(id));
            self.pending_blur = false;
        }
        if !focused && st.ed.error.is_none() && !st.ed.dirty {
            st.ed.buf = current;
            st.typed = st.ed.buf.clone();
            st.open = false;
            st.hl = None;
            st.moved = false;
        }
        let query = if st.ed.dirty {
            st.typed.clone()
        } else {
            String::new()
        };
        let mut list = self.ranked(items, &query);
        let create_row = offer_create
            && st.ed.dirty
            && !st.ed.buf.trim().is_empty()
            && !items
                .iter()
                .any(|it| it.label.eq_ignore_ascii_case(st.ed.buf.trim()));
        let rows = list.len() + usize::from(create_row);
        // Keys that steer the list; consumed before the text box sees them.
        let mut picked_by_key = None;
        if focused {
            ui.input_mut(|i| {
                if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                    if st.open && rows > 0 {
                        st.hl = Some(st.hl.map_or(0, |h| (h + 1).min(rows - 1)));
                        st.moved = true;
                    } else {
                        st.open = true;
                    }
                }
                if i.consume_key(Modifiers::NONE, Key::ArrowUp) && st.open && rows > 0 {
                    st.hl = Some(st.hl.map_or(0, |h| h.saturating_sub(1)));
                    st.moved = true;
                }
                if st.open
                    && st.hl.is_some()
                    && (st.ed.dirty || st.moved)
                    && i.consume_key(Modifiers::NONE, Key::Enter)
                {
                    picked_by_key = st.hl;
                }
                if st.open && i.consume_key(Modifiers::NONE, Key::Escape) {
                    st.open = false;
                }
            });
        }
        let out = self.edit_box(ui, &mut st.ed, id, name, width, false, "", st.popup);
        if out.resp.gained_focus() {
            st.open = !inline;
            st.hl = None;
            st.moved = false;
        }
        if out.resp.changed() {
            st.open = true;
            st.moved = false;
            let now = st.ed.buf.clone();
            let grew = now.chars().count() > st.typed.chars().count();
            st.typed = now.clone();
            st.hl = (self.t.preselect || inline)
                .then_some(0)
                .filter(|_| !inline);
            if inline && grew && !now.is_empty() {
                // Quick-fill: complete with the best path that starts with what was typed.
                let lower = now.to_lowercase();
                let mut cands: Vec<&Item> = items
                    .iter()
                    .filter(|it| it.label.to_lowercase().starts_with(&lower))
                    .collect();
                cands.sort_by_key(|it| self.rank_key(it.cat));
                if let Some(best) = cands.first() {
                    let typed_len = now.chars().count();
                    let full: String = best.label.chars().skip(typed_len).collect();
                    st.ed.buf = format!("{now}{full}");
                    select_range(ui.ctx(), id, typed_len, st.ed.buf.chars().count());
                    st.open = false;
                    self.h
                        .event("quick_fill", json!({ "typed": now, "offered": best.label }));
                } else {
                    st.open = true;
                }
            }
            ui.ctx().request_repaint();
        }
        let query = if st.ed.dirty {
            st.typed.clone()
        } else {
            String::new()
        };
        list = self.ranked(items, &query);
        // The list.
        if focused || out.resp.has_focus() {
            let show = st.open && (rows > 0 || !query.is_empty());
            if show {
                let rect = out.resp.rect;
                let hl = st.hl;
                let mut clicked = None;
                let shown: Vec<(usize, Vec<usize>)> = list.iter().take(10).cloned().collect();
                let more = list.len().saturating_sub(10);
                let area = egui::Area::new(Id::new((key, "list")))
                    .order(egui::Order::Foreground)
                    .fixed_pos(rect.left_bottom() + Vec2::new(0.0, 2.0))
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.set_min_width(rect.width().max(260.0));
                            if shown.is_empty() && !create_row {
                                ui.label(RichText::new(self.tr("No match")).weak());
                            }
                            for (k, (i, idx)) in shown.iter().enumerate() {
                                let it = &items[*i];
                                let job = self.highlighted(ui, &it.label, idx);
                                let r = ui.selectable_label(hl == Some(k), job);
                                if self.t.rank {
                                    r.clone().on_hover_text(format!(
                                        "used {} times",
                                        self.cats[it.cat].uses
                                    ));
                                }
                                if r.clicked() {
                                    clicked = Some(Outcome::Pick(it.cat));
                                }
                            }
                            if more > 0 {
                                ui.label(RichText::new(format!("… {more} more")).small().weak());
                            }
                            if create_row {
                                let r = ui.selectable_label(
                                    hl == Some(list.len()),
                                    format!("{} “{}”…", self.tr("Create"), st.ed.buf.trim()),
                                );
                                if r.clicked() {
                                    clicked = Some(Outcome::Create(st.ed.buf.trim().to_owned()));
                                }
                            }
                        });
                    });
                st.popup = Some(area.response.rect);
                if let Some(o) = clicked {
                    result = Some(o);
                    st.open = false;
                }
            } else {
                st.popup = None;
            }
        } else {
            st.popup = None;
        }
        if let Some(h) = picked_by_key {
            result = if h < list.len() {
                Some(Outcome::Pick(items[list[h].0].cat))
            } else {
                Some(Outcome::Create(st.ed.buf.trim().to_owned()))
            };
            st.open = false;
        }
        let mut saved = false;
        if result.is_none() && out.lost {
            if out.esc {
                st.ed.dirty = false;
                st.ed.error = None;
                st.moved = false;
                st.open = false;
                self.h.event("field_revert", json!({ "field": key }));
            } else {
                result = self.resolve(&st, &list, items, offer_create);
                st.open = false;
                saved = out.enter;
                if result.is_none() {
                    st.ed.dirty = false;
                    st.moved = false;
                }
            }
        }
        // Errors are applied by the caller through `set_error`; keep the state otherwise.
        if let Some(e) = &st.ed.error {
            let e = e.clone();
            self.note(ui, &out.resp, key, &e, true);
        } else if self.t.preview && false {
            // Preview applies to money and dates only.
        }
        self.combos.insert(key, st);
        if let Some(r) = &result {
            // Reset editing state; the caller sets the committed value.
            if let Some(st) = self.combos.get_mut(key) {
                st.ed.dirty = false;
                st.moved = false;
                st.hl = None;
            }
            let desc = match r {
                Outcome::Pick(c) => json!({ "outcome": "pick", "category": self.cats[*c].path }),
                Outcome::Create(t) => json!({ "outcome": "create", "text": t }),
                Outcome::Unknown(t) => json!({ "outcome": "unknown", "text": t }),
                Outcome::Clear => json!({ "outcome": "clear" }),
            };
            self.h
                .event("field_commit", json!({ "field": key, "result": desc }));
        }
        if saved {
            self.save_requested = true;
        }
        result
    }

    fn set_combo_error(&mut self, key: &'static str, msg: String, ui: &Ui) {
        let id = ui.make_persistent_id(key);
        if let Some(st) = self.combos.get_mut(key) {
            st.ed.error = Some(msg);
            st.ed.dirty = true;
        }
        ui.memory_mut(|m| m.request_focus(id));
    }

    fn touch_cat(&mut self, c: usize) {
        self.use_clock += 1;
        self.cats[c].uses += 1;
        self.cats[c].last_used = self.use_clock;
    }

    fn apply_cat_outcome(&mut self, key: &'static str, out: Outcome, ui: &Ui) {
        match out {
            Outcome::Pick(c) => {
                self.row.cat = Some(c);
                if let Some(st) = self.combos.get_mut(key) {
                    st.ed.error = None;
                    st.ed.dirty = false;
                }
            }
            Outcome::Clear => {
                self.row.cat = None;
            }
            Outcome::Create(name) => {
                // Real app: opens the New Category Dialog prefilled with the name.
                self.emit(
                    &format!("OpenDialog(NewCategory {{ name: {name:?} }})"),
                    "category combo",
                );
            }
            Outcome::Unknown(text) => {
                let msg = self.tr1("No category “{}”. Pick one from the list.", &text);
                self.set_combo_error(key, msg, ui);
            }
        }
    }

    fn cat_field(&mut self, ui: &mut Ui) {
        match self.variant {
            Variant::A | Variant::B => {
                let items = self.items_all();
                let current = self.cat_text();
                let inline = self.variant == Variant::A;
                let create = self.variant == Variant::B;
                if let Some(out) = self.combo(
                    ui,
                    "cat_field",
                    "category field",
                    &items,
                    current,
                    300.0,
                    create,
                    inline,
                ) {
                    self.apply_cat_outcome("cat_field", out, ui);
                }
            }
            Variant::C => {
                let parents: Vec<Item> = self
                    .cats
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.parent.is_none())
                    .map(|(i, c)| Item {
                        cat: i,
                        label: c.name.clone(),
                    })
                    .collect();
                let cur_parent = self.row.cat.map(|c| self.cats[c].parent.unwrap_or(c));
                let cur_child = self
                    .row
                    .cat
                    .filter(|c| self.cats[*c].parent.is_some())
                    .map(|c| self.cats[c].name.clone())
                    .unwrap_or_default();
                let parent_text =
                    cur_parent.map_or_else(String::new, |p| self.cats[p].name.clone());
                let mut parent_changed = None;
                let mut child_out = None;
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(self.tr("Parent")).small().weak());
                        if let Some(out) = self.combo(
                            ui,
                            "cat_parent",
                            "category parent field",
                            &parents,
                            parent_text,
                            150.0,
                            false,
                            false,
                        ) {
                            parent_changed = Some(out);
                        }
                    });
                    let kids: Vec<Item> = self
                        .cats
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| c.parent.is_some() && c.parent == cur_parent)
                        .map(|(i, c)| Item {
                            cat: i,
                            label: c.name.clone(),
                        })
                        .collect();
                    ui.vertical(|ui| {
                        ui.label(RichText::new(self.tr("Subcategory")).small().weak());
                        if let Some(out) = self.combo(
                            ui,
                            "cat_child",
                            "category child field",
                            &kids,
                            cur_child,
                            170.0,
                            false,
                            false,
                        ) {
                            child_out = Some(out);
                        }
                    });
                });
                if let Some(out) = parent_changed {
                    match out {
                        Outcome::Pick(p) => {
                            self.row.cat = Some(p);
                            if let Some(st) = self.combos.get_mut("cat_parent") {
                                st.ed.error = None;
                            }
                            // Moving on: the subcategory is the next thing to choose.
                            self.pending_focus = Some("child".to_owned());
                        }
                        other => self.apply_cat_outcome("cat_parent", other, ui),
                    }
                }
                if let Some(out) = child_out {
                    match out {
                        Outcome::Clear => {
                            self.row.cat = cur_parent;
                        }
                        other => self.apply_cat_outcome("cat_child", other, ui),
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // The row

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
                fmt_date(date, &EN),
                self.row.payee,
                cat_path,
                fmt_amount(amount, self.cur().dp, &EN),
                self.cur().code
            ),
            "Enter",
        );
        if let Some(c) = self.row.cat {
            self.touch_cat(c);
        }
        self.entered.insert(
            0,
            Entered {
                date,
                payee: self.row.payee.clone(),
                cat: cat_path,
                amount,
                cur: self.t.currency,
            },
        );
        self.entered.truncate(14);
        self.row = Row {
            date: Some(date),
            payee: String::new(),
            cat: None,
            amount: None,
        };
        self.cents.clear();
        self.pending_focus = Some("date".to_owned());
    }

    fn main_panel(&mut self, ui: &mut Ui) {
        CentralPanel::default().show(ui, |ui| {
            ui.heading(self.tr("Edit row"));
            ui.label(
                RichText::new(format!(
                    "{} · {} · {} {}",
                    self.loc().tag(),
                    self.variant.label(),
                    self.cur().code,
                    self.cur().dp
                ))
                .small()
                .weak(),
            );
            ui.add_space(8.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_min_width(190.0);
                        ui.label(self.tr("Date"));
                        self.date_field(ui);
                    });
                    ui.vertical(|ui| {
                        ui.set_min_width(190.0);
                        ui.label(self.tr("Payee"));
                        let id = ui.make_persistent_id("payee_field");
                        if self.pending_focus.as_deref() == Some("payee") {
                            ui.memory_mut(|m| m.request_focus(id));
                            self.pending_focus = None;
                        }
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut self.row.payee)
                                .id(id)
                                .desired_width(170.0),
                        );
                        self.h.track(&r, "payee field");
                    });
                    ui.vertical(|ui| {
                        ui.set_min_width(if self.variant == Variant::C { 340.0 } else { 320.0 });
                        ui.label(self.tr("Category"));
                        self.cat_field(ui);
                    });
                    ui.vertical(|ui| {
                        ui.label(self.tr("Amount"));
                        self.money_field(ui);
                    });
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let r = ui.button(format!("{} (Enter)", self.tr("Save row")));
                    self.h.track(&r, "save row button");
                    if r.clicked() {
                        self.save_requested = true;
                    }
                    ui.label(
                        RichText::new("Tab: next field · Enter: pick / commit, then save · Esc: revert, then cancel")
                            .small()
                            .weak(),
                    );
                });
            });
            if self.save_requested {
                self.save_requested = false;
                self.save_row();
            }
            ui.add_space(12.0);
            ui.label(RichText::new(self.tr("Entered rows")).strong());
            let f = self.f();
            egui::Grid::new("entered").striped(true).show(ui, |ui| {
                for h in ["Date", "Payee", "Category", "Amount"] {
                    ui.label(RichText::new(self.tr(h)).small().weak());
                }
                ui.end_row();
                for e in &self.entered {
                    ui.label(fmt_date(e.date, f));
                    ui.label(&e.payee);
                    ui.label(&e.cat);
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.label(fmt_amount(e.amount, CURS[e.cur].dp, f));
                    });
                    ui.end_row();
                }
            });
            ui.add_space(12.0);
            ui.label(RichText::new("Commands the real app would emit").strong());
            for line in &self.emitted {
                ui.label(RichText::new(line).small().monospace());
            }
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
                        .small_button("<")
                        .on_hover_text("Show prototype controls (F4)");
                    self.h.track(&r, "dock: expand");
                    if r.clicked() {
                        self.set_dock(true, "click");
                    }
                });
            return;
        }
        Panel::right("prototype_dock")
            .exact_size(300.0)
            .resizable(false)
            .frame(frame)
            .show(ui, |ui| {
                Sides::new().show(
                    ui,
                    |ui| ui.label(RichText::new("PROTOTYPE").strong()),
                    |ui| {
                        let r = ui.small_button(">").on_hover_text("Collapse (F4)");
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
                    if matches!(key, "locale" | "currency" | "match") {
                        let value = match key {
                            "locale" => self.loc().tag().to_owned(),
                            "currency" => self.cur().code.to_owned(),
                            _ => MatchMode::ALL[self.t.match_mode].name().to_owned(),
                        };
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
                ui.label(
                    RichText::new(
                        "A B C or Left/Right variant · 1–0 toggles (while no field has focus) · F2 note · F3 marker · F4 hide dock",
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
        Box::new(move |_cc| Ok(Box::new(Proto::new(&args)))),
    )
}
