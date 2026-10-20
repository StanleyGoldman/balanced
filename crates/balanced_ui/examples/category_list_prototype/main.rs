// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! PROTOTYPE, throwaway: the Category List window.
//!
//! Question (ticket #361): what shape suits a Category List that views, adds, edits, merges and
//! deletes categories with their usage, and what does deleting a used category do?
//!
//! Applied from earlier prototypes: Dialogs are in-window overlays that dim the window behind
//! them, in two weights, and there is no second OS window in v1 (#19); a staged table with a
//! per-row Delete confirm and "nothing is saved until OK" (#76); the real theme (#16/#218).
//!
//! Three variants differ in where the list lives and when an edit lands:
//!   A  Heavy Dialog, immediate: a button column; every action runs at once behind its confirm
//!   B  Register-style tab, immediate: a toolbar and a right-click menu; nothing dims
//!   C  Heavy Dialog, staged: inline rename, per-row actions, nothing changes until OK
//!
//! Toggles (1-6) change the rules every variant applies; see `TOGGLES`. Each cycles through its
//! options on its number key.
//!
//! Run:
//!   cargo run -p balanced_ui --example category_list_prototype -- \
//!     [--variant a|b|c] [--log <dir>/events.jsonl] [--control <dir>/control.txt]
//!
//! Control-file verbs (besides the harness's `prompt` and `spotlight`): `variant <key>`,
//! `toggle <key|number> [on|off|<option number>]`, `screenshot [label]`, `dock on|off`, `reload`,
//! `quit`, `open`, `close`, `select <category path>[; <path>…]`, `search <text>`, `new`,
//! `edit`, `merge`, `delete`, `hide`, `usage` (each acts on the selection, as its button does),
//! `show <all|income|expense|used|unused>`, `hidden on|off`, `ok`, `cancel`.
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
    dead_code,
    missing_debug_implementations,
    unreachable_pub
)]

use std::collections::{BTreeSet, HashMap, HashSet};

use balanced_prototype_harness::{Args, Harness};
use eframe::egui;
use egui::{
    Align, CentralPanel, Color32, Context, Id, Key, Layout, Order, Panel, Rect, RichText, Sense,
    Sides, Stroke, TextEdit, Ui, ViewportBuilder, ViewportCommand, vec2,
};
use egui_extras::{Column, Size, StripBuilder, TableBuilder};
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

/// 2026-10-20, as days since 1970-01-01.
const TODAY: i64 = 20_746;
const HISTORY_DAYS: i64 = 1_095;
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

const SEED: u64 = 0x00ca_7e60_71e5;

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
}

/// A parent and its subcategories.
const TREE: &[(&str, &[&str])] = &[
    ("Auto", &["Fuel", "Insurance", "Service", "Registration"]),
    ("Food", &["Groceries", "Dining", "Coffee"]),
    ("Home", &["Rent", "Maintenance", "Insurance", "Furnishings"]),
    ("Utilities", &["Electric", "Water", "Gas", "Internet", "Phone"]),
    ("Health", &["Doctor", "Pharmacy", "Dental"]),
    ("Entertainment", &["Streaming", "Movies", "Games", "Books"]),
    ("Travel", &["Flights", "Hotels", "Car rental"]),
    ("Kids", &["School", "Activities", "Clothing"]),
    ("Pets", &["Food", "Vet"]),
    ("Taxes", &["Federal", "State", "Property"]),
];
const STANDALONE: &[&str] = &[
    "Charity",
    "Gifts",
    "Bank charges",
    "Miscellaneous",
    "Old subscriptions",
];
const INCOME: &[&str] = &["Salary", "Interest", "Dividends", "Refunds", "Other income"];

/// (category path, payee, typical cents, times a year, fixed amount, only if older than N days).
const SPENDING: &[(&str, &str, i64, u64, bool, i64)] = &[
    ("Auto:Fuel", "Shell", 5_200, 26, false, 0),
    ("Auto:Insurance", "Progressive", 14_200, 12, true, 0),
    ("Auto:Service", "Jiffy Lube", 7_900, 3, false, 0),
    ("Auto:Registration", "State DMV", 18_500, 1, true, 0),
    ("Food:Groceries", "Green Valley Market", 8_200, 85, false, 0),
    ("Food:Groceries", "Trader Joe's", 5_400, 40, false, 0),
    ("Food:Dining", "Luigi's", 4_600, 30, false, 0),
    ("Food:Dining", "Taqueria Azul", 2_400, 24, false, 0),
    ("Food:Coffee", "Blue Door Coffee", 650, 70, false, 0),
    ("Home:Rent", "Maple Court Apartments", 185_000, 12, true, 0),
    ("Home:Maintenance", "Ace Hardware", 3_600, 10, false, 0),
    ("Home:Insurance", "State Farm", 2_900, 12, true, 0),
    ("Home:Furnishings", "IKEA", 12_000, 3, false, 0),
    ("Utilities:Electric", "City Power", 9_400, 12, false, 0),
    ("Utilities:Water", "Metro Water", 4_800, 12, false, 0),
    ("Utilities:Gas", "Northern Gas", 5_200, 12, false, 0),
    ("Utilities:Internet", "Comcast", 7_900, 12, true, 0),
    ("Utilities:Phone", "Verizon", 8_500, 12, true, 0),
    ("Health:Doctor", "Dr. Okafor", 3_000, 6, false, 0),
    ("Health:Pharmacy", "CVS Pharmacy", 1_900, 14, false, 0),
    ("Health:Dental", "Bright Smile Dental", 11_000, 2, false, 0),
    ("Entertainment:Streaming", "Netflix", 1_599, 12, true, 0),
    ("Entertainment:Streaming", "Spotify", 1_099, 12, true, 0),
    ("Entertainment:Movies", "AMC Theatres", 2_800, 5, false, 0),
    ("Entertainment:Books", "Powell's Books", 2_400, 8, false, 0),
    ("Travel:Flights", "Delta Air Lines", 38_000, 3, false, 0),
    ("Travel:Hotels", "Marriott", 29_000, 3, false, 0),
    ("Kids:School", "Lincoln Elementary", 22_000, 4, false, 0),
    ("Kids:Activities", "Youth Soccer League", 6_500, 14, false, 0),
    ("Kids:Clothing", "Old Navy", 4_800, 8, false, 0),
    ("Pets:Food", "Chewy", 5_400, 12, false, 0),
    ("Taxes:Federal", "IRS", 90_000, 4, false, 0),
    ("Taxes:Property", "County Treasurer", 210_000, 2, true, 0),
    ("Charity", "Red Cross", 5_000, 6, false, 0),
    ("Gifts", "Amazon", 4_500, 9, false, 0),
    ("Bank charges", "Checking fee", 1_200, 3, true, 0),
    ("Miscellaneous", "Amazon", 3_800, 20, false, 0),
    ("Old subscriptions", "NY Times", 1_700, 12, true, 420),
];

#[derive(Clone, Debug)]
struct Cat {
    id: u32,
    name: String,
    parent: Option<u32>,
    hidden: bool,
    income: bool,
}

#[derive(Clone, Copy, Debug)]
struct Txn {
    id: u32,
    day: i64,
    payee: &'static str,
    /// `None` is Uncategorized, or a transfer when `transfer` is set.
    cat: Option<u32>,
    transfer: Option<&'static str>,
    cents: i64,
}

#[derive(Clone, Default)]
struct Data {
    cats: Vec<Cat>,
    txns: Vec<Txn>,
    next: u32,
}

#[derive(Clone, Copy, Default)]
struct Usage {
    count: usize,
    total: i64,
    last: i64,
}

impl Data {
    fn generate() -> Self {
        let mut d = Data {
            next: 1,
            ..Data::default()
        };
        let mut add = |d: &mut Data, name: &str, parent: Option<u32>, income: bool| {
            let id = d.next;
            d.next += 1;
            d.cats.push(Cat {
                id,
                name: name.to_owned(),
                parent,
                hidden: false,
                income,
            });
            id
        };
        for (parent, subs) in TREE {
            let p = add(&mut d, parent, None, false);
            for s in *subs {
                add(&mut d, s, Some(p), false);
            }
        }
        for name in STANDALONE {
            let id = add(&mut d, name, None, false);
            if *name == "Old subscriptions" {
                d.cats.iter_mut().find(|c| c.id == id).unwrap().hidden = true;
            }
        }
        for name in INCOME {
            add(&mut d, name, None, true);
        }
        let mut rng = Lcg(SEED);
        let mut id = 1;
        let mut push = |d: &mut Data, day: i64, payee, cat: Option<&str>, transfer, cents| {
            let cat = cat.and_then(|p| d.find_path(p));
            d.txns.push(Txn {
                id,
                day,
                payee,
                cat,
                transfer,
                cents,
            });
            id += 1;
        };
        for day in START..=TODAY {
            let age = TODAY - day;
            for (cat, payee, typical, per_year, fixed, older_than) in SPENDING {
                if age < *older_than {
                    continue;
                }
                let monthly = *per_year == 12;
                let hit = if monthly {
                    // Once a month, on a day fixed by the payee.
                    let (_, _, dom) = civil(day);
                    dom == 1 + (payee.len() as i64 * 7) % 27
                } else {
                    rng.below(365) < *per_year
                };
                if hit {
                    let cents = if *fixed { *typical } else { rng.around(*typical) };
                    push(&mut d, day, payee, Some(cat), None, -cents);
                }
            }
            // Pay every other Friday.
            if (day - START) % 14 == 4 {
                push(&mut d, day, "Acme Corp", Some("Salary"), None, 285_000);
            }
            let (_, _, dom) = civil(day);
            if dom == 28 {
                push(&mut d, day, "Credit Union", Some("Interest"), None, rng.around(420));
                push(&mut d, day, "Transfer to Savings", None, Some("Savings"), -50_000);
            }
            if rng.below(365) < 5 {
                push(&mut d, day, "Amazon", Some("Refunds"), None, rng.around(2_800));
            }
            if rng.below(365) < 2 {
                push(&mut d, day, "Craigslist sale", Some("Other income"), None, rng.around(15_000));
            }
            // A few that were never categorized.
            if rng.below(365) < 8 {
                push(&mut d, day, "Cash withdrawal", None, None, -(rng.around(4_000) / 100 * 100));
            }
        }
        d
    }

    fn cat(&self, id: u32) -> Option<&Cat> {
        self.cats.iter().find(|c| c.id == id)
    }

    fn find_path(&self, path: &str) -> Option<u32> {
        self.cats.iter().map(|c| c.id).find(|id| self.path(*id) == path)
    }

    fn path(&self, id: u32) -> String {
        match self.cat(id) {
            None => String::new(),
            Some(c) => match c.parent {
                Some(p) => format!("{}:{}", self.path(p), c.name),
                None => c.name.clone(),
            },
        }
    }

    fn children(&self, parent: Option<u32>) -> Vec<u32> {
        let mut ids: Vec<&Cat> = self.cats.iter().filter(|c| c.parent == parent).collect();
        ids.sort_by_key(|c| c.name.to_lowercase());
        ids.into_iter().map(|c| c.id).collect()
    }

    fn descendants(&self, id: u32) -> Vec<u32> {
        let mut out = Vec::new();
        for child in self.children(Some(id)) {
            out.push(child);
            out.extend(self.descendants(child));
        }
        out
    }

    /// Every category in tree order with its depth.
    fn ordered(&self) -> Vec<(u32, usize)> {
        fn walk(d: &Data, parent: Option<u32>, depth: usize, out: &mut Vec<(u32, usize)>) {
            for id in d.children(parent) {
                out.push((id, depth));
                walk(d, Some(id), depth + 1, out);
            }
        }
        let mut out = Vec::new();
        walk(self, None, 0, &mut out);
        out
    }

    fn usage(&self) -> HashMap<u32, Usage> {
        let mut map: HashMap<u32, Usage> = HashMap::new();
        for t in &self.txns {
            if let Some(c) = t.cat {
                let u = map.entry(c).or_default();
                u.count += 1;
                u.total += t.cents;
                u.last = u.last.max(t.day);
            }
        }
        map
    }

    fn transfer_count(&self, account: &str) -> usize {
        self.txns.iter().filter(|t| t.transfer == Some(account)).count()
    }

    /// Paths of every category, for a picker, in alphabetical order.
    fn picker(&self, skip: &[u32]) -> Vec<(u32, String)> {
        let mut v: Vec<(u32, String)> = self
            .cats
            .iter()
            .filter(|c| !skip.contains(&c.id))
            .map(|c| (c.id, self.path(c.id)))
            .collect();
        v.sort_by_key(|(_, p)| p.to_lowercase());
        v
    }
}

/// A change to the categories; what the real app would send as a Command.
#[derive(Clone, Debug)]
enum Op {
    Create {
        name: String,
        parent: Option<u32>,
        income: bool,
    },
    Edit {
        id: u32,
        name: String,
        parent: Option<u32>,
    },
    Hide {
        ids: Vec<u32>,
        hidden: bool,
    },
    Merge {
        from: Vec<u32>,
        into: u32,
        delete_sources: bool,
    },
    Delete {
        ids: Vec<u32>,
        /// Where the transactions of the deleted categories go: `None` is Uncategorized.
        reassign: Option<u32>,
    },
}

impl Data {
    fn apply(&mut self, op: &Op) {
        match op {
            Op::Create { name, parent, income } => {
                let income = parent.and_then(|p| self.cat(p)).map_or(*income, |p| p.income);
                self.cats.push(Cat {
                    id: self.next,
                    name: name.clone(),
                    parent: *parent,
                    hidden: false,
                    income,
                });
                self.next += 1;
            }
            Op::Edit { id, name, parent } => {
                let income = parent.and_then(|p| self.cat(p)).map(|p| p.income);
                if let Some(c) = self.cats.iter_mut().find(|c| c.id == *id) {
                    c.name = name.clone();
                    c.parent = *parent;
                    if let Some(i) = income {
                        c.income = i;
                    }
                }
            }
            Op::Hide { ids, hidden } => {
                for id in ids {
                    let all: Vec<u32> = std::iter::once(*id).chain(self.descendants(*id)).collect();
                    for c in self.cats.iter_mut().filter(|c| all.contains(&c.id)) {
                        c.hidden = *hidden;
                    }
                }
            }
            Op::Merge { from, into, delete_sources } => {
                for t in self.txns.iter_mut().filter(|t| t.cat.is_some_and(|c| from.contains(&c))) {
                    t.cat = Some(*into);
                }
                if *delete_sources {
                    // A source that still has subcategories stays.
                    let keep: Vec<u32> = from
                        .iter()
                        .copied()
                        .filter(|f| self.cats.iter().any(|c| c.parent == Some(*f) && !from.contains(&c.id)))
                        .collect();
                    self.cats.retain(|c| !from.contains(&c.id) || keep.contains(&c.id));
                }
            }
            Op::Delete { ids, reassign } => {
                for t in self.txns.iter_mut().filter(|t| t.cat.is_some_and(|c| ids.contains(&c))) {
                    t.cat = *reassign;
                }
                self.cats.retain(|c| !ids.contains(&c.id));
            }
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
        Self::ALL.into_iter().find(|v| v.key().eq_ignore_ascii_case(key))
    }

    fn label(self) -> &'static str {
        match self {
            Self::A => "A · Heavy Dialog, buttons act at once",
            Self::B => "B · Tab, toolbar and right-click",
            Self::C => "C · Heavy Dialog, staged until OK",
        }
    }

    fn step(self, delta: isize) -> Self {
        let n = Self::ALL.len() as isize;
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0) as isize;
        Self::ALL[(i + delta).rem_euclid(n) as usize]
    }

    fn staged(self) -> bool {
        self == Self::C
    }
}

const DEL_USED: usize = 0;
const DEL_PARENT: usize = 1;
const USAGE: usize = 2;
const HIDE: usize = 3;
const MERGE: usize = 4;
const FILTER: usize = 5;

/// (key, label, options); every toggle applies to all variants.
const TOGGLES: &[(&str, &str, &[&str])] = &[
    (
        "delete_used",
        "Delete a used category",
        &["Blocked", "Reassign", "Uncategorized"],
    ),
    (
        "delete_parent",
        "Delete a parent",
        &["Refused", "Takes its subs"],
    ),
    ("usage", "Usage columns", &["Count", "Count + total", "Off"]),
    ("hide", "Hide and Unhide", &["Offered", "Not offered"]),
    (
        "merge",
        "Merge leaves the sources",
        &["Tick: delete", "Tick: keep", "Always delete"],
    ),
    ("filter", "Filter row", &["Show + Search", "Search only"]),
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

const SHOW: [&str; 5] = ["All", "Income", "Expense", "Used", "Unused"];

// ---------------------------------------------------------------------------------------------
// App state

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Checking,
    Savings,
    Categories,
}

/// A modal opened over the list.
enum Sub {
    New {
        name: String,
        parent: Option<u32>,
        income: bool,
        fresh: bool,
    },
    Edit {
        id: u32,
        name: String,
        parent: Option<u32>,
        fresh: bool,
    },
    Merge {
        from: Vec<u32>,
        into: Option<u32>,
        delete: bool,
    },
    Delete {
        /// The categories the delete takes, subcategories included.
        ids: Vec<u32>,
        /// What the user selected.
        asked: Vec<u32>,
        used: usize,
        total: i64,
        reassign: Option<u32>,
    },
    Message {
        title: String,
        body: String,
        merge_from: Option<Vec<u32>>,
    },
    Usage(u32),
}

/// A user request collected while drawing, applied after the frame.
enum Act {
    Select { id: u32, ctrl: bool, shift: bool },
    Fold(u32),
    New,
    Edit(u32),
    Merge(Vec<u32>),
    Delete(Vec<u32>),
    Hide(Vec<u32>, bool),
    Usage(u32),
    Rename(u32, String),
    Close,
    Ok,
    Cancel,
}

struct Staged {
    data: Data,
    pending: Vec<(String, Value, String)>,
}

const RED: Color32 = Color32::from_rgb(0xc0, 0x2b, 0x2b);
const STATUS_H: f32 = 26.0;

struct Proto {
    h: Harness,
    variant: Variant,
    tog: [usize; 6],
    theme: Theme,
    dock_open: bool,
    status: String,
    data: Data,
    staged: Option<Staged>,
    open: bool,
    tab: Tab,
    sel: BTreeSet<u32>,
    anchor: Option<u32>,
    visible: Vec<u32>,
    folded: HashSet<u32>,
    show: usize,
    show_hidden: bool,
    search: String,
    sub: Option<Sub>,
    flash: HashSet<u32>,
    reg_scroll: Option<usize>,
    rename: Option<(u32, String, bool)>,
    acts: Vec<Act>,
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
        let data = Data::generate();
        Self {
            h,
            variant,
            tog: [0; 6],
            theme: Theme::new(ThemeSettings::default()),
            dock_open: true,
            status: "Ready".to_owned(),
            reg_scroll: Some(data.txns.len()),
            data,
            staged: None,
            open: false,
            tab: Tab::Checking,
            sel: BTreeSet::new(),
            anchor: None,
            visible: Vec::new(),
            folded: HashSet::new(),
            show: 0,
            show_hidden: false,
            search: String::new(),
            sub: None,
            flash: HashSet::new(),
            rename: None,
            acts: Vec::new(),
            closing: false,
        }
    }

    fn work(&self) -> &Data {
        self.staged.as_ref().map_or(&self.data, |s| &s.data)
    }

    fn event(&mut self, kind: &str, fields: Value) {
        self.h.event(kind, fields);
    }

    // -----------------------------------------------------------------------------------------
    // Toggles, variants, dock

    fn set_variant(&mut self, to: Variant, via: &str) {
        if to == self.variant {
            return;
        }
        self.event(
            "variant",
            json!({ "from": self.variant.key(), "to": to.key(), "via": via }),
        );
        self.variant = to;
        self.sub = None;
        self.rename = None;
        if let Some(s) = self.staged.take() {
            self.event("staged_dropped", json!({ "pending": s.pending.len() }));
        }
        if self.open {
            self.surface_opened();
        } else if self.tab == Tab::Categories {
            self.tab = Tab::Checking;
        }
        self.h.screenshot(format!("variant-{}", to.key()));
    }

    fn set_toggle(&mut self, key: &str, value: Option<&str>, via: &str) {
        let index = key
            .parse::<usize>()
            .ok()
            .map(|n| if n == 0 { 9 } else { n - 1 })
            .filter(|i| *i < TOGGLES.len())
            .or_else(|| TOGGLES.iter().position(|t| t.0 == key));
        let Some(i) = index else {
            self.event("control_unknown", json!({ "toggle": key }));
            return;
        };
        let n = TOGGLES[i].2.len();
        let new = match value {
            None => (self.tog[i] + 1) % n,
            Some("on") => 1.min(n - 1),
            Some("off") => 0,
            Some(v) => v.parse::<usize>().map_or(self.tog[i], |v| v.saturating_sub(1).min(n - 1)),
        };
        if new == self.tog[i] && via == "control" {
            return;
        }
        self.tog[i] = new;
        self.event(
            "toggle",
            json!({ "key": TOGGLES[i].0, "option": TOGGLES[i].2[new], "index": new + 1, "via": via, "applies_to": "all" }),
        );
        self.h.screenshot(format!("toggle-{}-{}", TOGGLES[i].0, new + 1));
    }

    fn set_dock(&mut self, open: bool, via: &str) {
        self.dock_open = open;
        self.event("dock", json!({ "open": open, "via": via }));
    }

    fn opt(&self, toggle: usize) -> usize {
        self.tog[toggle]
    }

    // -----------------------------------------------------------------------------------------
    // The list opening and closing

    fn surface_opened(&mut self) {
        if self.variant == Variant::B {
            self.tab = Tab::Categories;
            self.staged = None;
        } else if self.variant.staged() {
            self.staged = Some(Staged {
                data: self.data.clone(),
                pending: Vec::new(),
            });
        } else {
            self.staged = None;
        }
    }

    fn open_list(&mut self, source: &str) {
        self.event(
            "command",
            json!({ "would_emit": if self.variant == Variant::B { "OpenTab(CategoryList)" } else { "OpenDialog(CategoryList)" }, "source": source }),
        );
        if self.open {
            if self.variant == Variant::B {
                self.tab = Tab::Categories;
            }
            return;
        }
        self.open = true;
        self.surface_opened();
        self.h.screenshot(format!("open-{}", self.variant.key()));
    }

    fn close_list(&mut self, source: &str) {
        if !self.open {
            return;
        }
        self.event("list_closed", json!({ "source": source, "variant": self.variant.key() }));
        if let Some(s) = self.staged.take() {
            if !s.pending.is_empty() {
                self.event("staged_dropped", json!({ "pending": s.pending.len() }));
            }
        }
        self.open = false;
        self.sub = None;
        self.rename = None;
        if self.tab == Tab::Categories {
            self.tab = Tab::Checking;
        }
    }

    fn commit_staged(&mut self) {
        let Some(s) = self.staged.take() else {
            return;
        };
        for (name, detail, _) in &s.pending {
            self.event("command", json!({ "would_emit": name, "detail": detail, "source": "OK" }));
        }
        let n = s.pending.len();
        self.data = s.data;
        self.status = format!("OK: would emit {n} Command{}", if n == 1 { "" } else { "s" });
        self.close_list("OK");
    }

    // -----------------------------------------------------------------------------------------
    // Operations

    fn describe(&self, op: &Op) -> (String, Value, String) {
        let d = self.work();
        let paths = |ids: &[u32]| ids.iter().map(|i| d.path(*i)).collect::<Vec<_>>();
        match op {
            Op::Create { name, parent, income } => (
                "CreateCategory".to_owned(),
                json!({ "name": name, "parent": parent.map(|p| d.path(p)), "income": income }),
                format!("Created {}", parent.map_or(name.clone(), |p| format!("{}:{name}", d.path(p)))),
            ),
            Op::Edit { id, name, parent } => (
                "EditCategory".to_owned(),
                json!({ "category": d.path(*id), "name": name, "parent": parent.map(|p| d.path(p)) }),
                format!("Edited {}", d.path(*id)),
            ),
            Op::Hide { ids, hidden } => (
                "SetCategoryHidden".to_owned(),
                json!({ "categories": paths(ids), "hidden": hidden }),
                format!("{} {}", if *hidden { "Hid" } else { "Unhid" }, paths(ids).join(", ")),
            ),
            Op::Merge { from, into, delete_sources } => (
                "MergeCategories".to_owned(),
                json!({ "from": paths(from), "into": d.path(*into), "delete_sources": delete_sources }),
                format!("Merged {} into {}", paths(from).join(", "), d.path(*into)),
            ),
            Op::Delete { ids, reassign } => (
                "DeleteCategory".to_owned(),
                json!({ "categories": paths(ids), "reassign_to": reassign.map(|r| d.path(r)) }),
                format!("Deleted {}", paths(ids).join(", ")),
            ),
        }
    }

    /// Applies `op` to the working data: at once, or into the staged copy in variant C.
    fn do_op(&mut self, op: Op, source: &str) {
        let (name, mut detail, mut label) = self.describe(&op);
        let before: Vec<(Option<u32>, String)> = self
            .work()
            .txns
            .iter()
            .map(|t| (t.cat, t.cat.map(|c| self.work().path(c)).unwrap_or_default()))
            .collect();
        match &mut self.staged {
            Some(s) => s.data.apply(&op),
            None => self.data.apply(&op),
        }
        let d = self.work();
        let mut touched = HashSet::new();
        let mut moved = 0;
        for (t, (cat, path)) in d.txns.iter().zip(&before) {
            if t.cat != *cat {
                moved += 1;
            }
            if t.cat != *cat || t.cat.map(|c| d.path(c)).unwrap_or_default() != *path {
                touched.insert(t.id);
            }
        }
        if let Some(last) = d.txns.iter().rposition(|t| touched.contains(&t.id)) {
            self.reg_scroll = Some(last);
        }
        self.flash = touched;
        if moved > 0 {
            detail["txns_moved"] = json!(moved);
            label = format!("{label} — {moved} transactions moved");
        }
        match &mut self.staged {
            Some(s) => {
                self.h.event("staged_op", json!({ "would_emit": name, "detail": detail, "source": source }));
                s.pending.push((name, detail, label.clone()));
            }
            None => self.h.event("command", json!({ "would_emit": name, "detail": detail, "source": source })),
        }
        self.status = label;
        let alive: HashSet<u32> = self.work().cats.iter().map(|c| c.id).collect();
        self.sel.retain(|id| alive.contains(id));
    }

    fn selection(&self) -> Vec<u32> {
        self.sel.iter().copied().filter(|id| self.work().cat(*id).is_some()).collect()
    }

    fn begin_delete(&mut self, asked: Vec<u32>) {
        if asked.is_empty() {
            return;
        }
        let d = self.work().clone();
        let with_subs: Vec<u32> = asked
            .iter()
            .copied()
            .filter(|a| !d.children(Some(*a)).is_empty())
            .collect();
        let mut ids = asked.clone();
        if !with_subs.is_empty() {
            let names = with_subs.iter().map(|i| d.path(*i)).collect::<Vec<_>>().join(", ");
            if self.opt(DEL_PARENT) == 0 {
                self.event("delete_refused", json!({ "why": "has subcategories", "categories": names }));
                self.sub = Some(Sub::Message {
                    title: "Can't delete yet".to_owned(),
                    body: format!(
                        "{names} has subcategories. Delete or move its subcategories first."
                    ),
                    merge_from: None,
                });
                return;
            }
            for a in &asked {
                for x in d.descendants(*a) {
                    if !ids.contains(&x) {
                        ids.push(x);
                    }
                }
            }
        }
        let usage = d.usage();
        let used: usize = ids.iter().map(|i| usage.get(i).map_or(0, |u| u.count)).sum();
        let total: i64 = ids.iter().map(|i| usage.get(i).map_or(0, |u| u.total)).sum();
        if used > 0 && self.opt(DEL_USED) == 0 {
            let names = ids
                .iter()
                .filter(|i| usage.get(i).is_some_and(|u| u.count > 0))
                .map(|i| format!("{} ({})", d.path(*i), usage[i].count))
                .collect::<Vec<_>>()
                .join(", ");
            self.event("delete_refused", json!({ "why": "used", "transactions": used }));
            self.sub = Some(Sub::Message {
                title: "Can't delete a category in use".to_owned(),
                body: format!(
                    "{names} is used by {used} transaction{}. Merge it into another category first, then delete it.",
                    if used == 1 { "" } else { "s" }
                ),
                merge_from: Some(asked),
            });
            return;
        }
        self.sub = Some(Sub::Delete {
            ids,
            asked,
            used,
            total,
            reassign: None,
        });
    }

    fn begin_merge(&mut self, from: Vec<u32>) {
        if from.is_empty() {
            return;
        }
        let delete = self.opt(MERGE) != 1;
        self.sub = Some(Sub::Merge { from, into: None, delete });
    }

    fn apply_acts(&mut self, ctx: &Context) {
        let acts = std::mem::take(&mut self.acts);
        for act in acts {
            match act {
                Act::Select { id, ctrl, shift } => {
                    if shift && let Some(a) = self.anchor {
                        let (i, j) = (
                            self.visible.iter().position(|v| *v == a),
                            self.visible.iter().position(|v| *v == id),
                        );
                        if let (Some(i), Some(j)) = (i, j) {
                            let (lo, hi) = (i.min(j), i.max(j));
                            if !ctrl {
                                self.sel.clear();
                            }
                            self.sel.extend(self.visible[lo..=hi].iter().copied());
                        }
                    } else if ctrl {
                        if !self.sel.remove(&id) {
                            self.sel.insert(id);
                        }
                        self.anchor = Some(id);
                    } else {
                        self.sel.clear();
                        self.sel.insert(id);
                        self.anchor = Some(id);
                    }
                    let path = self.work().path(id);
                    self.event("select", json!({ "category": path, "count": self.sel.len() }));
                }
                Act::Fold(id) => {
                    if !self.folded.remove(&id) {
                        self.folded.insert(id);
                    }
                }
                Act::New => {
                    let parent = self.selection().first().copied().filter(|s| self.work().cat(*s).is_some_and(|c| c.parent.is_none()));
                    self.sub = Some(Sub::New {
                        name: String::new(),
                        parent,
                        income: false,
                        fresh: true,
                    });
                }
                Act::Edit(id) => {
                    if let Some(c) = self.work().cat(id) {
                        self.sub = Some(Sub::Edit {
                            id,
                            name: c.name.clone(),
                            parent: c.parent,
                            fresh: true,
                        });
                    }
                }
                Act::Merge(ids) => self.begin_merge(ids),
                Act::Delete(ids) => self.begin_delete(ids),
                Act::Hide(ids, hidden) => self.do_op(Op::Hide { ids, hidden }, "Hide"),
                Act::Usage(id) => self.sub = Some(Sub::Usage(id)),
                Act::Rename(id, name) => {
                    let name = name.trim().to_owned();
                    if let Some(c) = self.work().cat(id).cloned()
                        && !name.is_empty()
                        && name != c.name
                    {
                        self.do_op(
                            Op::Edit {
                                id,
                                name,
                                parent: c.parent,
                            },
                            "inline rename",
                        );
                    }
                }
                Act::Close => self.close_list("Close"),
                Act::Ok => self.commit_staged(),
                Act::Cancel => self.close_list("Cancel"),
            }
        }
        let _ = ctx;
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
            "open" => self.open_list("control"),
            "close" => self.close_list("control"),
            "select" => {
                self.sel.clear();
                for p in rest.split(';') {
                    match self.work().find_path(p.trim()) {
                        Some(id) => {
                            self.sel.insert(id);
                            self.anchor = Some(id);
                        }
                        None => bad(self),
                    }
                }
            }
            "search" => self.search = rest.to_owned(),
            "show" => match SHOW.iter().position(|s| s.eq_ignore_ascii_case(rest)) {
                Some(i) => self.show = i,
                None => bad(self),
            },
            "hidden" => self.show_hidden = rest != "off",
            "new" => self.acts.push(Act::New),
            "edit" => {
                if let Some(id) = self.selection().first() {
                    self.acts.push(Act::Edit(*id));
                }
            }
            "merge" => self.acts.push(Act::Merge(self.selection())),
            "delete" => self.acts.push(Act::Delete(self.selection())),
            "hide" => {
                let sel = self.selection();
                let hidden = !sel.iter().all(|i| self.work().cat(*i).is_some_and(|c| c.hidden));
                self.acts.push(Act::Hide(sel, hidden));
            }
            "usage" => {
                if let Some(id) = self.selection().first() {
                    self.acts.push(Act::Usage(*id));
                }
            }
            "ok" => self.acts.push(Act::Ok),
            "cancel" => self.acts.push(Act::Cancel),
            "reload" => {
                self.data = Data::generate();
                self.staged = None;
                self.open = false;
                self.sub = None;
                self.sel.clear();
                self.flash.clear();
                self.event("reload", json!({ "seed": SEED }));
            }
            "screenshot" => self.h.screenshot(if rest.is_empty() { "control" } else { rest }),
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
        if let Some(cmd) = self.h.begin_frame(&ctx) {
            self.apply_control(&ctx, &cmd);
        }
        let typing = ctx.egui_wants_keyboard_input();
        if !typing && self.sub.is_none() {
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
        if ctx.input(|i| i.key_pressed(Key::C) && i.modifiers.command && i.modifiers.shift) {
            self.open_list("Ctrl+Shift+C");
        }
        // Esc closes the top Dialog; a modal on top takes it first.
        if !typing
            && self.sub.is_none()
            && self.open
            && self.variant != Variant::B
            && ctx.input(|i| i.key_pressed(Key::Escape))
        {
            self.acts.push(Act::Close);
        }
        // Arrows and Enter in the list, while nothing has text focus.
        if !typing && self.sub.is_none() && self.open && !self.visible.is_empty() {
            let down = ctx.input(|i| i.key_pressed(Key::ArrowDown));
            let up = ctx.input(|i| i.key_pressed(Key::ArrowUp));
            if down || up {
                let at = self.sel.iter().next().and_then(|s| self.visible.iter().position(|v| v == s));
                let next = match (at, down) {
                    (Some(i), true) => (i + 1).min(self.visible.len() - 1),
                    (Some(i), false) => i.saturating_sub(1),
                    (None, _) => 0,
                };
                self.acts.push(Act::Select { id: self.visible[next], ctrl: false, shift: false });
            }
            if ctx.input(|i| i.key_pressed(Key::Enter)) {
                if let Some(id) = self.selection().first() {
                    self.acts.push(if self.variant.staged() { Act::Edit(*id) } else { Act::Edit(*id) });
                }
            }
        }

        self.dock(ui);
        self.h.prompt_panel(ui);
        self.menu_bar(ui);
        self.tab_strip(ui);
        Panel::bottom("status")
            .exact_size(STATUS_H)
            .frame(self.theme.chrome_frame().inner_margin(egui::Margin::symmetric(8, 4)))
            .show(ui, |ui| {
                ui.label(RichText::new(&self.status).weak());
            });
        CentralPanel::default().show(ui, |ui| {
            match self.tab {
                Tab::Checking => self.register(ui),
                Tab::Savings => {
                    ui.label(RichText::new("Savings register: not part of this prototype.").weak());
                }
                Tab::Categories => {
                    let rect = ui.available_rect_before_wrap();
                    self.h.region(rect, "category list");
                    self.list_body(ui);
                }
            }
        });
        self.heavy_window(&ctx);
        self.sub_modal(&ctx);
        self.apply_acts(&ctx);
        self.h.end_frame(&ctx);
    }
}

// ---------------------------------------------------------------------------------------------
// Chrome: dock, menu, tabs

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
                    let r = ui.small_button("◀").on_hover_text("Show prototype controls (F4)");
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
                ui.label(RichText::new("Variant (where it lives, when edits land)").strong());
                for (v, (_, hint)) in Variant::ALL.into_iter().zip(VARIANT_KEYS) {
                    let r = ui.selectable_label(self.variant == v, format!("{hint}   {}", v.label()));
                    self.h.track(&r, format!("dock: variant {}", v.key()));
                    if r.clicked() {
                        self.set_variant(v, "click");
                    }
                }
                ui.separator();
                ui.label(RichText::new("Toggles (all variants; the key cycles)").strong());
                for (i, ((key, label, options), (_, hint))) in TOGGLES.iter().zip(TOGGLE_KEYS).enumerate() {
                    ui.label(format!("{hint}   {label}"));
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
                        "A / B / C or left / right variant · 1–6 toggles · Ctrl+Shift+C opens the list · F2 note · F3 marker · F4 hide this dock · F5 spotlight · F6 prompt",
                    )
                    .small()
                    .weak(),
                );
            });
    }

    fn menu_bar(&mut self, ui: &mut Ui) {
        Panel::top("menu_bar")
            .frame(self.theme.chrome_frame().inner_margin(egui::Margin::symmetric(6, 2)))
            .show(ui, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        ui.add_enabled(false, egui::Button::new("New data file…"));
                        ui.add_enabled(false, egui::Button::new("Open data file…"));
                    });
                    ui.menu_button("Edit", |ui| {
                        ui.add_enabled(false, egui::Button::new("Undo"));
                    });
                    let tools = ui.menu_button("Tools", |ui| {
                        let r = ui.button("Category List…   Ctrl+Shift+C");
                        self.h.track(&r, "menu: Category List");
                        if r.clicked() {
                            ui.close();
                            self.open_list("Tools menu");
                        }
                        ui.add_enabled(false, egui::Button::new("Memorized Payees…"));
                        ui.add_enabled(false, egui::Button::new("Preferences…"));
                    });
                    self.h.track(&tools.response, "menu: Tools");
                });
            });
    }

    fn tab_strip(&mut self, ui: &mut Ui) {
        Panel::top("tab_strip")
            .frame(self.theme.chrome_frame().inner_margin(egui::Margin::symmetric(6, 2)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (tab, name) in [(Tab::Checking, "Checking"), (Tab::Savings, "Savings")] {
                        let r = ui.selectable_label(self.tab == tab, name);
                        self.h.track(&r, format!("tab: {name}"));
                        if r.clicked() {
                            self.tab = tab;
                        }
                    }
                    if self.open && self.variant == Variant::B {
                        let r = ui.selectable_label(self.tab == Tab::Categories, "Category List");
                        self.h.track(&r, "tab: Category List");
                        if r.clicked() {
                            self.tab = Tab::Categories;
                        }
                        let x = ui.small_button("✕").on_hover_text("Close the tab");
                        self.h.track(&x, "tab: close Category List");
                        if x.clicked() {
                            self.close_list("tab ✕");
                        }
                    }
                });
            });
    }

    fn register(&mut self, ui: &mut Ui) {
        let d = &self.data;
        let paths: HashMap<u32, String> = d.cats.iter().map(|c| (c.id, d.path(c.id))).collect();
        let row_h = self.theme.row_height();
        let n = d.txns.len();
        ui.label(RichText::new(format!("Checking · {n} transactions")).weak());
        let mut table = TableBuilder::new(ui)
            .id_salt("register")
            .striped(true)
            .resizable(false)
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(90.0))
            .column(Column::initial(240.0).at_least(80.0))
            .column(Column::initial(260.0).at_least(80.0))
            .column(Column::remainder().at_least(90.0));
        if let Some(r) = self.reg_scroll.take() {
            table = table.scroll_to_row(r.min(n.saturating_sub(1)), Some(Align::Center));
        }
        table
            .header(row_h + 2.0, |mut header| {
                for label in ["Date", "Payee", "Category", "Amount"] {
                    header.col(|ui| {
                        if label == "Amount" {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.strong(label));
                        } else {
                            ui.strong(label);
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(row_h, n, |mut row| {
                    let t = d.txns[row.index()];
                    row.set_selected(self.flash.contains(&t.id));
                    row.col(|ui| {
                        ui.label(fmt_day(t.day));
                    });
                    row.col(|ui| {
                        ui.label(t.payee);
                    });
                    row.col(|ui| match (t.cat, t.transfer) {
                        (Some(c), _) => {
                            ui.label(paths.get(&c).cloned().unwrap_or_default());
                        }
                        (None, Some(a)) => {
                            ui.label(format!("[{a}]"));
                        }
                        (None, None) => {
                            ui.label(RichText::new("Uncategorized").italics().weak());
                        }
                    });
                    row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(self.theme.amount_text(t.cents));
                        });
                    });
                });
            });
    }
}

impl Theme {
    fn amount_text(&self, cents: i64) -> RichText {
        let text = RichText::new(fmt_cents(cents));
        if cents < 0 { text.color(RED) } else { text }
    }
}

// ---------------------------------------------------------------------------------------------
// The Category List

impl Proto {
    /// The heavy Dialog of variants A and C.
    fn heavy_window(&mut self, ctx: &Context) {
        if !self.open || self.variant == Variant::B {
            return;
        }
        let screen = ctx.content_rect();
        let backdrop = self.theme.backdrop();
        egui::Area::new(Id::new("cl_backdrop"))
            .order(Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
                ui.painter().rect_filled(rect, 0.0, backdrop);
            });
        let max = screen.size() - vec2(Theme::DIALOG_MARGIN * 2.0, Theme::DIALOG_MARGIN * 2.0);
        let size = vec2(860.0_f32.min(max.x), 600.0_f32.min(max.y));
        let mut open = true;
        let title = if self.variant.staged() {
            "Category List"
        } else {
            "Category List"
        };
        egui::Window::new(title)
            .id(Id::new("cl_window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(size)
            .max_size(max)
            .default_pos(screen.center() - size / 2.0)
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                self.h.region(rect, "category list");
                ui.set_min_size(vec2(size.x - 24.0, size.y - 48.0));
                self.list_body(ui);
            });
        ctx.move_to_top(egui::LayerId::new(Order::Middle, Id::new("cl_window")));
        if !open {
            self.acts.push(if self.variant.staged() { Act::Cancel } else { Act::Close });
        }
    }

    /// Which categories the table shows, in order: (id, depth).
    fn rows(&self, d: &Data, usage: &HashMap<u32, Usage>) -> Vec<(u32, usize)> {
        let q = self.search.trim().to_lowercase();
        let matches = |id: u32| {
            let c = d.cat(id).unwrap();
            let by_show = match self.show {
                1 => c.income,
                2 => !c.income,
                3 => usage.get(&id).is_some_and(|u| u.count > 0),
                4 => usage.get(&id).is_none_or(|u| u.count == 0),
                _ => true,
            };
            let by_hidden = self.show_hidden || !c.hidden;
            let by_search = q.is_empty() || d.path(id).to_lowercase().contains(&q);
            by_show && by_hidden && by_search
        };
        let ordered = d.ordered();
        let searching = !q.is_empty() || self.show != 0;
        let mut out = Vec::new();
        for (id, depth) in &ordered {
            let c = d.cat(*id).unwrap();
            // A folded ancestor hides the row, unless a search or filter is looking for it.
            let mut hidden_by_fold = false;
            let mut p = c.parent;
            while let Some(pid) = p {
                if self.folded.contains(&pid) && !searching {
                    hidden_by_fold = true;
                }
                p = d.cat(pid).and_then(|x| x.parent);
            }
            if hidden_by_fold {
                continue;
            }
            // A parent shows when it matches or one of its descendants does.
            let shown = matches(*id)
                || (d.descendants(*id).iter().any(|x| matches(*x))
                    && (self.show_hidden || !c.hidden));
            if shown {
                out.push((*id, *depth));
            }
        }
        out
    }

    fn list_body(&mut self, ui: &mut Ui) {
        let d = self.work().clone();
        let usage = d.usage();
        let rows = self.rows(&d, &usage);
        self.visible = rows.iter().map(|(id, _)| *id).collect();
        let row_h = self.theme.row_height();
        let variant = self.variant;
        let footer = match variant {
            Variant::B => 0.0,
            _ => row_h + 14.0,
        };
        let toolbar = if variant == Variant::B { row_h + 12.0 } else { 0.0 };
        StripBuilder::new(ui)
            .size(Size::exact(toolbar))
            .size(Size::exact(row_h + 10.0))
            .size(Size::remainder())
            .size(Size::exact(footer))
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    if variant == Variant::B {
                        self.toolbar(ui, &d);
                    }
                });
                strip.cell(|ui| self.filter_row(ui));
                strip.cell(|ui| {
                    if variant == Variant::A {
                        StripBuilder::new(ui)
                            .size(Size::remainder())
                            .size(Size::exact(132.0))
                            .horizontal(|mut strip| {
                                strip.cell(|ui| self.table(ui, &d, &usage, &rows));
                                strip.cell(|ui| self.button_column(ui, &d));
                            });
                    } else {
                        self.table(ui, &d, &usage, &rows);
                    }
                });
                strip.cell(|ui| {
                    if variant != Variant::B {
                        self.footer(ui);
                    }
                });
            });
    }

    fn filter_row(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            if self.opt(FILTER) == 0 {
                ui.label("Show:");
                let before = self.show;
                egui::ComboBox::from_id_salt("cl_show")
                    .selected_text(SHOW[self.show])
                    .width(90.0)
                    .show_ui(ui, |ui| {
                        for (i, s) in SHOW.iter().enumerate() {
                            ui.selectable_value(&mut self.show, i, *s);
                        }
                    });
                if self.show != before {
                    self.h.event("filter", json!({ "show": SHOW[self.show] }));
                }
                ui.add_space(10.0);
            }
            ui.label("Search:");
            let r = ui.add(
                TextEdit::singleline(&mut self.search)
                    .desired_width(Theme::SEARCH_BOX_WIDTH)
                    .hint_text("Category name"),
            );
            self.h.track(&r, "search field");
            if r.changed() {
                self.h.event("search", json!({ "text": self.search }));
            }
            if self.opt(HIDE) == 0 {
                ui.add_space(10.0);
                let before = self.show_hidden;
                let r = ui.checkbox(&mut self.show_hidden, "Show hidden categories");
                self.h.track(&r, "show hidden checkbox");
                if self.show_hidden != before {
                    self.h.event("filter", json!({ "show_hidden": self.show_hidden }));
                }
            }
        });
    }

    fn toolbar(&mut self, ui: &mut Ui, d: &Data) {
        let sel = self.selection();
        let one = sel.len() == 1;
        ui.horizontal_centered(|ui| {
            if self.tbtn(ui, "New…", true, "toolbar: New") {
                self.acts.push(Act::New);
            }
            if self.tbtn(ui, "Edit…", one, "toolbar: Edit") {
                self.acts.push(Act::Edit(sel[0]));
            }
            if self.tbtn(ui, "Merge…", !sel.is_empty(), "toolbar: Merge") {
                self.acts.push(Act::Merge(sel.clone()));
            }
            if self.opt(HIDE) == 0 {
                let all_hidden = !sel.is_empty() && sel.iter().all(|i| d.cat(*i).is_some_and(|c| c.hidden));
                let label = if all_hidden { "Unhide" } else { "Hide" };
                if self.tbtn(ui, label, !sel.is_empty(), "toolbar: Hide") {
                    self.acts.push(Act::Hide(sel.clone(), !all_hidden));
                }
            }
            if self.tbtn(ui, "Delete…", !sel.is_empty(), "toolbar: Delete") {
                self.acts.push(Act::Delete(sel.clone()));
            }
            if self.opt(USAGE) == 2 && self.tbtn(ui, "Usage…", one, "toolbar: Usage") {
                self.acts.push(Act::Usage(sel[0]));
            }
        });
    }

    fn tbtn(&mut self, ui: &mut Ui, label: &str, enabled: bool, name: &str) -> bool {
        let r = ui.add_enabled(enabled, egui::Button::new(label));
        self.h.track(&r, name);
        r.clicked()
    }

    fn button_column(&mut self, ui: &mut Ui, d: &Data) {
        let sel = self.selection();
        let one = sel.len() == 1;
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            let w = ui.available_width() - 8.0;
            let b = |s: &mut Self, ui: &mut Ui, label: &str, enabled: bool, name: &str| -> bool {
                let r = ui.add_enabled(enabled, egui::Button::new(label).min_size(vec2(w, 0.0)));
                s.h.track(&r, name);
                r.clicked()
            };
            if b(self, ui, "New…", true, "button: New") {
                self.acts.push(Act::New);
            }
            if b(self, ui, "Edit…", one, "button: Edit") {
                self.acts.push(Act::Edit(sel[0]));
            }
            if b(self, ui, "Merge…", !sel.is_empty(), "button: Merge") {
                self.acts.push(Act::Merge(sel.clone()));
            }
            if self.opt(HIDE) == 0 {
                let all_hidden = !sel.is_empty() && sel.iter().all(|i| d.cat(*i).is_some_and(|c| c.hidden));
                let label = if all_hidden { "Unhide" } else { "Hide" };
                if b(self, ui, label, !sel.is_empty(), "button: Hide") {
                    self.acts.push(Act::Hide(sel.clone(), !all_hidden));
                }
            }
            if b(self, ui, "Delete…", !sel.is_empty(), "button: Delete") {
                self.acts.push(Act::Delete(sel.clone()));
            }
            if self.opt(USAGE) == 2 && b(self, ui, "Usage…", one, "button: Usage") {
                self.acts.push(Act::Usage(sel[0]));
            }
        });
    }

    fn footer(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            if self.variant.staged() {
                let n = self.staged.as_ref().map_or(0, |s| s.pending.len());
                ui.label(RichText::new(if n == 0 {
                    "Nothing is saved until OK.".to_owned()
                } else {
                    format!("{n} change{} waiting. Nothing is saved until OK.", if n == 1 { "" } else { "s" })
                }).weak());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let r = ui.button("Cancel");
                    self.h.track(&r, "footer: Cancel");
                    if r.clicked() {
                        self.acts.push(Act::Cancel);
                    }
                    let r = ui.add(egui::Button::new("OK").min_size(vec2(70.0, 0.0)));
                    self.h.track(&r, "footer: OK");
                    if r.clicked() {
                        self.acts.push(Act::Ok);
                    }
                });
            } else {
                ui.label(RichText::new("Changes apply as soon as you confirm them.").weak());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let r = ui.add(egui::Button::new("Close").min_size(vec2(70.0, 0.0)));
                    self.h.track(&r, "footer: Close");
                    if r.clicked() {
                        self.acts.push(Act::Close);
                    }
                });
            }
        });
    }

    fn table(&mut self, ui: &mut Ui, d: &Data, usage: &HashMap<u32, Usage>, rows: &[(u32, usize)]) {
        let row_h = self.theme.row_height();
        let usage_on = self.opt(USAGE) != 2;
        let totals = self.opt(USAGE) == 1;
        let staged = self.variant.staged();
        let transfers = ["Checking", "Savings"];
        let n = rows.len() + transfers.len();
        let selected_all = self.selection();
        let mut table = TableBuilder::new(ui)
            .id_salt("cl_table")
            .striped(true)
            .resizable(false)
            .sense(Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::remainder().at_least(160.0).clip(true))
            .column(Column::exact(68.0));
        if usage_on {
            table = table.column(Column::exact(64.0));
        }
        if totals {
            table = table.column(Column::exact(100.0));
        }
        if staged {
            if self.opt(HIDE) == 0 {
                table = table.column(Column::exact(56.0));
            }
            table = table.column(Column::exact(158.0));
        }
        table
            .header(row_h + 2.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Category");
                });
                header.col(|ui| {
                    ui.strong("Type");
                });
                if usage_on {
                    header.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.strong("Txns"));
                    });
                }
                if totals {
                    header.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.strong("Total"));
                    });
                }
                if staged {
                    if self.opt(HIDE) == 0 {
                        header.col(|ui| {
                            ui.strong("Hidden");
                        });
                    }
                    header.col(|ui| {
                        ui.strong("");
                    });
                }
            })
            .body(|body| {
                body.rows(row_h, n, |mut row| {
                    let i = row.index();
                    if i >= rows.len() {
                        let name = transfers[i - rows.len()];
                        row.col(|ui| {
                            ui.label(RichText::new(format!("[{name}]")).weak());
                        });
                        row.col(|ui| {
                            ui.label(RichText::new("Transfer").weak());
                        });
                        if usage_on {
                            row.col(|ui| {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(RichText::new(d.transfer_count(name).to_string()).weak());
                                });
                            });
                        }
                        return;
                    }
                    let (id, depth) = rows[i];
                    let c = d.cat(id).unwrap();
                    let u = usage.get(&id).copied().unwrap_or_default();
                    let is_sel = selected_all.contains(&id);
                    row.set_selected(is_sel);
                    let has_kids = !d.children(Some(id)).is_empty();
                    row.col(|ui| {
                        ui.add_space(depth as f32 * 18.0 + 2.0);
                        if has_kids {
                            let glyph = if self.folded.contains(&id) { "⏵" } else { "⏷" };
                            let r = ui.add(egui::Button::new(glyph).frame(false).small());
                            self.h.track(&r, format!("fold, {}", c.name));
                            if r.clicked() {
                                self.acts.push(Act::Fold(id));
                            }
                        } else {
                            ui.add_space(20.0);
                        }
                        let renaming = matches!(&self.rename, Some((rid, _, _)) if *rid == id);
                        if renaming {
                            let (_, buf, fresh) = self.rename.as_mut().unwrap();
                            let r = ui.add(TextEdit::singleline(buf).desired_width(180.0));
                            if *fresh {
                                r.request_focus();
                                *fresh = false;
                            }
                            self.h.track(&r, format!("rename field, {}", c.name));
                            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                            let esc = ui.input(|i| i.key_pressed(Key::Escape));
                            if esc {
                                self.rename = None;
                            } else if enter || r.lost_focus() {
                                let (rid, text, _) = self.rename.take().unwrap();
                                self.acts.push(Act::Rename(rid, text));
                            }
                        } else {
                            let mut text = RichText::new(&c.name);
                            if c.hidden {
                                text = text.italics().weak();
                            }
                            ui.label(text);
                            if c.hidden {
                                ui.label(RichText::new("hidden").small().weak());
                            }
                        }
                    });
                    row.col(|ui| {
                        ui.label(if c.income { "Income" } else { "Expense" });
                    });
                    if usage_on {
                        row.col(|ui| {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if u.count == 0 {
                                    ui.label(RichText::new("0").weak());
                                } else {
                                    ui.label(u.count.to_string());
                                }
                            });
                        });
                    }
                    if totals {
                        row.col(|ui| {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if u.count > 0 {
                                    ui.label(self.theme.amount_text(u.total));
                                }
                            });
                        });
                    }
                    if staged {
                        if self.opt(HIDE) == 0 {
                            row.col(|ui| {
                                let mut hidden = c.hidden;
                                let r = ui.checkbox(&mut hidden, "");
                                self.h.track(&r, format!("hidden checkbox, {}", c.name));
                                if r.changed() {
                                    self.acts.push(Act::Hide(vec![id], hidden));
                                }
                            });
                        }
                        row.col(|ui| {
                            let r = ui.small_button("Merge…");
                            self.h.track(&r, format!("row Merge, {}", c.name));
                            if r.clicked() {
                                self.acts.push(Act::Merge(vec![id]));
                            }
                            let r = ui.small_button("Delete…");
                            self.h.track(&r, format!("row Delete, {}", c.name));
                            if r.clicked() {
                                self.acts.push(Act::Delete(vec![id]));
                            }
                            if !usage_on {
                                let r = ui.small_button("Usage…");
                                self.h.track(&r, format!("row Usage, {}", c.name));
                                if r.clicked() {
                                    self.acts.push(Act::Usage(id));
                                }
                            }
                        });
                    }
                    let r = row.response();
                    self.h.track(&r, format!("category row, {}", d.path(id)));
                    if r.clicked() {
                        let m = r.ctx.input(|i| i.modifiers);
                        self.acts.push(Act::Select { id, ctrl: m.command, shift: m.shift });
                    }
                    if r.double_clicked() {
                        if staged {
                            self.rename = Some((id, c.name.clone(), true));
                        } else {
                            self.acts.push(Act::Edit(id));
                        }
                    }
                    if self.variant == Variant::B {
                        r.context_menu(|ui| {
                            if !self.sel.contains(&id) {
                                self.acts.push(Act::Select { id, ctrl: false, shift: false });
                            }
                            let target = if self.sel.contains(&id) {
                                self.selection()
                            } else {
                                vec![id]
                            };
                            if ui.button("New…").clicked() {
                                self.acts.push(Act::New);
                                ui.close();
                            }
                            if ui.add_enabled(target.len() == 1, egui::Button::new("Edit…")).clicked() {
                                self.acts.push(Act::Edit(id));
                                ui.close();
                            }
                            if ui.button("Merge…").clicked() {
                                self.acts.push(Act::Merge(target.clone()));
                                ui.close();
                            }
                            if self.opt(HIDE) == 0 {
                                let label = if c.hidden { "Unhide" } else { "Hide" };
                                if ui.button(label).clicked() {
                                    self.acts.push(Act::Hide(target.clone(), !c.hidden));
                                    ui.close();
                                }
                            }
                            if ui.button("Delete…").clicked() {
                                self.acts.push(Act::Delete(target.clone()));
                                ui.close();
                            }
                            if ui.add_enabled(target.len() == 1, egui::Button::new("Usage…")).clicked() {
                                self.acts.push(Act::Usage(id));
                                ui.close();
                            }
                        });
                    }
                });
            });
    }
}

// ---------------------------------------------------------------------------------------------
// Modals over the list

impl Proto {
    fn sub_modal(&mut self, ctx: &Context) {
        let Some(mut sub) = self.sub.take() else {
            return;
        };
        let d = self.work().clone();
        let usage = d.usage();
        let mut done = false;
        let mut op: Option<Op> = None;
        let mut next: Option<Sub> = None;
        let enter = ctx.input(|i| i.key_pressed(Key::Enter));
        let modal = egui::Modal::new(Id::new("cl_sub")).show(ctx, |ui| {
            ui.set_width(420.0);
            match &mut sub {
                Sub::New { name, parent, income, fresh } => {
                    ui.heading("New Category");
                    ui.add_space(6.0);
                    ui.label("Name");
                    let r = ui.add(TextEdit::singleline(name).desired_width(f32::INFINITY));
                    if *fresh {
                        r.request_focus();
                        *fresh = false;
                    }
                    self.h.track(&r, "new category: name");
                    ui.add_space(4.0);
                    ui.label("Subcategory of");
                    let tops = d.children(None);
                    egui::ComboBox::from_id_salt("new_parent")
                        .selected_text(parent.map_or("(none: a top-level category)".to_owned(), |p| d.path(p)))
                        .width(260.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(parent, None, "(none: a top-level category)");
                            for t in tops {
                                ui.selectable_value(parent, Some(t), d.path(t));
                            }
                        });
                    if parent.is_none() {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.radio_value(income, false, "Expense");
                            ui.radio_value(income, true, "Income");
                        });
                    }
                    let taken = d.children(*parent).iter().any(|c| d.cat(*c).is_some_and(|c| c.name.eq_ignore_ascii_case(name.trim())));
                    if taken {
                        ui.colored_label(RED, "A category with that name is already there.");
                    }
                    ui.add_space(8.0);
                    let ok = !name.trim().is_empty() && !taken;
                    ui.horizontal(|ui| {
                        let r = ui.add_enabled(ok, egui::Button::new("OK").min_size(vec2(70.0, 0.0)));
                        self.h.track(&r, "new category: OK");
                        if r.clicked() || (enter && ok) {
                            op = Some(Op::Create { name: name.trim().to_owned(), parent: *parent, income: *income });
                            done = true;
                        }
                        let r = ui.button("Cancel");
                        self.h.track(&r, "new category: Cancel");
                        if r.clicked() {
                            done = true;
                        }
                    });
                }
                Sub::Edit { id, name, parent, fresh } => {
                    let c = d.cat(*id).cloned().unwrap();
                    ui.heading("Edit Category");
                    ui.add_space(6.0);
                    ui.label("Name");
                    let r = ui.add(TextEdit::singleline(name).desired_width(f32::INFINITY));
                    if *fresh {
                        r.request_focus();
                        *fresh = false;
                    }
                    self.h.track(&r, "edit category: name");
                    ui.add_space(4.0);
                    ui.label("Subcategory of");
                    let own: Vec<u32> = std::iter::once(*id).chain(d.descendants(*id)).collect();
                    let tops: Vec<u32> = d.children(None).into_iter().filter(|t| !own.contains(t)).collect();
                    egui::ComboBox::from_id_salt("edit_parent")
                        .selected_text(parent.map_or("(none: a top-level category)".to_owned(), |p| d.path(p)))
                        .width(260.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(parent, None, "(none: a top-level category)");
                            for t in tops {
                                ui.selectable_value(parent, Some(t), d.path(t));
                            }
                        });
                    let u = usage.get(id).map_or(0, |u| u.count);
                    if u > 0 && name.trim() != c.name {
                        ui.add_space(4.0);
                        ui.label(RichText::new(format!("Renaming updates {u} transactions.")).weak());
                    }
                    let taken = d.children(*parent).iter().any(|s| *s != *id && d.cat(*s).is_some_and(|x| x.name.eq_ignore_ascii_case(name.trim())));
                    if taken {
                        ui.colored_label(RED, "A category with that name is already there.");
                    }
                    ui.add_space(8.0);
                    let ok = !name.trim().is_empty() && !taken;
                    ui.horizontal(|ui| {
                        let r = ui.add_enabled(ok, egui::Button::new("OK").min_size(vec2(70.0, 0.0)));
                        self.h.track(&r, "edit category: OK");
                        if r.clicked() || (enter && ok) {
                            if name.trim() != c.name || *parent != c.parent {
                                op = Some(Op::Edit { id: *id, name: name.trim().to_owned(), parent: *parent });
                            }
                            done = true;
                        }
                        let r = ui.button("Cancel");
                        self.h.track(&r, "edit category: Cancel");
                        if r.clicked() {
                            done = true;
                        }
                    });
                }
                Sub::Merge { from, into, delete } => {
                    ui.heading("Merge Categories");
                    ui.add_space(6.0);
                    let n: usize = from.iter().map(|f| usage.get(f).map_or(0, |u| u.count)).sum();
                    ui.label(format!(
                        "Merge {} into one category. Its {n} transaction{} move there.",
                        from.iter().map(|f| d.path(*f)).collect::<Vec<_>>().join(", "),
                        if n == 1 { "" } else { "s" },
                    ));
                    ui.add_space(6.0);
                    ui.label("Merge transactions into");
                    egui::ComboBox::from_id_salt("merge_into")
                        .selected_text(into.map_or("Choose a category".to_owned(), |p| d.path(p)))
                        .width(300.0)
                        .show_ui(ui, |ui| {
                            for (id, path) in d.picker(from) {
                                ui.selectable_value(into, Some(id), path);
                            }
                        });
                    ui.add_space(6.0);
                    match self.tog[MERGE] {
                        2 => {
                            ui.label(RichText::new("The merged categories are deleted.").weak());
                        }
                        _ => {
                            let r = ui.checkbox(delete, "Delete unused selected categories");
                            self.h.track(&r, "merge: delete checkbox");
                        }
                    }
                    if *delete || self.tog[MERGE] == 2 {
                        let kept: Vec<String> = from
                            .iter()
                            .filter(|f| d.children(Some(**f)).iter().any(|c| !from.contains(c)))
                            .map(|f| d.path(*f))
                            .collect();
                        if !kept.is_empty() {
                            ui.label(RichText::new(format!("{} keeps its subcategories, so it stays.", kept.join(", "))).weak());
                        }
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let r = ui.add_enabled(into.is_some(), egui::Button::new("Merge").min_size(vec2(70.0, 0.0)));
                        self.h.track(&r, "merge: Merge");
                        if r.clicked() || (enter && into.is_some()) {
                            op = Some(Op::Merge { from: from.clone(), into: into.unwrap(), delete_sources: *delete || self.tog[MERGE] == 2 });
                            done = true;
                        }
                        let r = ui.button("Cancel");
                        self.h.track(&r, "merge: Cancel");
                        if r.clicked() {
                            done = true;
                        }
                    });
                }
                Sub::Delete { ids, asked, used, total, reassign } => {
                    ui.heading("Delete Category");
                    ui.add_space(6.0);
                    let names = asked.iter().map(|i| d.path(*i)).collect::<Vec<_>>().join(", ");
                    let subs = ids.len() - asked.len();
                    if *used == 0 && subs == 0 {
                        ui.label(format!("Delete {names}? No transaction uses it."));
                    } else if *used == 0 {
                        ui.label(format!("Delete {names} and its {subs} subcategor{}? No transaction uses them.", if subs == 1 { "y" } else { "ies" }));
                    } else {
                        let what = if subs > 0 {
                            format!("{names} and its {subs} subcategor{}", if subs == 1 { "y" } else { "ies" })
                        } else {
                            names.clone()
                        };
                        ui.label(format!(
                            "{what} {} used by {used} transaction{} totalling {}.",
                            if subs > 0 { "are" } else { "is" },
                            if *used == 1 { "" } else { "s" },
                            fmt_cents(*total),
                        ));
                        ui.add_space(6.0);
                        if self.tog[DEL_USED] == 1 {
                            ui.label("Move those transactions to");
                            egui::ComboBox::from_id_salt("delete_reassign")
                                .selected_text(reassign.map_or("(Uncategorized)".to_owned(), |p| d.path(p)))
                                .width(300.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(reassign, None, "(Uncategorized)");
                                    for (id, path) in d.picker(ids) {
                                        ui.selectable_value(reassign, Some(id), path);
                                    }
                                });
                        } else {
                            ui.label("They will become Uncategorized.");
                        }
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let r = ui.add(egui::Button::new("Delete").min_size(vec2(70.0, 0.0)));
                        self.h.track(&r, "delete: Delete");
                        r.request_focus();
                        if r.clicked() || enter {
                            op = Some(Op::Delete { ids: ids.clone(), reassign: *reassign });
                            done = true;
                        }
                        let r = ui.button("Cancel");
                        self.h.track(&r, "delete: Cancel");
                        if r.clicked() {
                            done = true;
                        }
                    });
                }
                Sub::Message { title, body, merge_from } => {
                    ui.heading(title.as_str());
                    ui.add_space(6.0);
                    ui.label(body.as_str());
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let r = ui.add(egui::Button::new("OK").min_size(vec2(70.0, 0.0)));
                        self.h.track(&r, "message: OK");
                        if r.clicked() || enter {
                            done = true;
                        }
                        if let Some(from) = merge_from.clone() {
                            let r = ui.button("Merge…");
                            self.h.track(&r, "message: Merge");
                            if r.clicked() {
                                next = Some(Sub::Merge { from, into: None, delete: self.tog[MERGE] != 1 });
                                done = true;
                            }
                        }
                    });
                }
                Sub::Usage(id) => {
                    let u = usage.get(id).copied().unwrap_or_default();
                    ui.heading(format!("Usage: {}", d.path(*id)));
                    ui.add_space(6.0);
                    if u.count == 0 {
                        ui.label("No transaction uses this category.");
                    } else {
                        ui.label(format!(
                            "{} transactions totalling {}, last used {}.",
                            u.count,
                            fmt_cents(u.total),
                            fmt_day(u.last)
                        ));
                        ui.add_space(6.0);
                        for t in d.txns.iter().rev().filter(|t| t.cat == Some(*id)).take(6) {
                            ui.horizontal(|ui| {
                                ui.label(fmt_day(t.day));
                                ui.label(t.payee);
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(self.theme.amount_text(t.cents));
                                });
                            });
                        }
                    }
                    ui.add_space(8.0);
                    let r = ui.add(egui::Button::new("Close").min_size(vec2(70.0, 0.0)));
                    self.h.track(&r, "usage: Close");
                    if r.clicked() || enter {
                        done = true;
                    }
                }
            }
        });
        if modal.should_close() {
            done = true;
        }
        if let Some(op) = op {
            self.do_op(op, "dialog");
        }
        if let Some(s) = next {
            self.sub = Some(s);
        } else if !done {
            self.sub = Some(sub);
        } else {
            self.event("sub_closed", json!({}));
        }
    }
}

fn main() -> eframe::Result {
    // Prints the stub figures and exits, so they can be checked without a window.
    if std::env::var_os("CATEGORY_LIST_PROTO_DUMP").is_some() {
        let d = Data::generate();
        let usage = d.usage();
        println!("categories {} transactions {}", d.cats.len(), d.txns.len());
        for (id, depth) in d.ordered() {
            let u = usage.get(&id).copied().unwrap_or_default();
            println!(
                "{}{:<22} {:>5} {:>13}",
                "  ".repeat(depth),
                d.cat(id).unwrap().name,
                u.count,
                fmt_cents(u.total)
            );
        }
        let net: i64 = d.txns.iter().map(|t| t.cents).sum();
        let uncat = d.txns.iter().filter(|t| t.cat.is_none() && t.transfer.is_none()).count();
        println!("net {} uncategorized {uncat}", fmt_cents(net));
        return Ok(());
    }
    let args = Args::from_env();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Balanced — PROTOTYPE Category List")
            .with_maximized(true),
        ..Default::default()
    };
    eframe::run_native(
        "balanced_category_list_prototype",
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
