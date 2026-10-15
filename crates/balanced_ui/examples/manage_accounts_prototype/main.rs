//! PROTOTYPE, throwaway: Manage Accounts and the Account bar sections.
//!
//! Question (ticket #76): how does the Manage Accounts dialog edit, hide and close accounts, and
//! with "Group accounts by type" off, how do user-named sections and separators look in the
//! Account bar and how are they arranged?
//!
//! Three variants differ in how Manage Accounts is structured:
//!   A  Account List table: one table holds the whole layout, details inline on each row
//!   B  Tree + details: the layout as a tree on the left, the selected item's details on the right
//!   C  Two tabs: an Accounts tab (details table) and a Layout tab (sections, separators, order)
//!
//! Run:
//!   cargo run -p balanced_ui --example manage_accounts_prototype -- \
//!     [--variant a|b|c] [--log <dir>/events.jsonl] [--control <dir>/control.txt]
//!
//! The PROTOTYPE dock on the right holds the variant switcher and toggles; F4 collapses it.
//! A / B / C or ← / → switch variant, 1–9 and 0 flip toggles, F2 note, F3 marker, F5 spotlight,
//! F6 prompt panel.
//!
//! Control-file verbs (besides the harness's `prompt` and `spotlight`): `variant <key>`,
//! `toggle <key> [on|off]`, `screenshot [label]`, `dock on|off`, `reload`, `quit`,
//! `account <index|name>`, `tab <index|name>`, `dialog manage`, `ok`, `cancel`,
//! `select <account or section name>` (in the open Manage Accounts), `page accounts|layout`
//! (variant C's tabs), `fold <section>`, `unfold <section>`.
//!
//! The Main window follows the decided shell layout (#15) and the real theme module (#16).
//! Manage Accounts is a heavy Dialog (#19): title bar, movable, resizable, app-modal, dimming.
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
    clippy::rc_buffer,
    clippy::print_stdout,
    clippy::disallowed_methods,
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    dead_code,
    missing_debug_implementations,
    unreachable_pub
)]

use std::collections::BTreeSet;
use std::sync::Arc;

use balanced_prototype_harness::{Args, Harness};
use eframe::egui;
use egui::{
    Align, Align2, Button, CentralPanel, Color32, Context, CursorIcon, DragAndDrop, Id, Key,
    Layout, Order, Panel, Rect, RichText, Sense, Shape, Sides, Stroke, Ui, ViewportBuilder,
    ViewportCommand, vec2,
};
use egui_extras::{Column, TableBuilder};
use serde_json::json;

// The real theme module, compiled into the example: the prototype gets the decided look
// without making `theme` public.
#[allow(dead_code)]
#[path = "../../src/theme.rs"]
mod theme;

use theme::{Theme, ThemeSettings};

// ---------------------------------------------------------------------------------------------
// Stub data: fixed seed, realistic volume

const SEED: u64 = 0x00ba_1a9c_ed76;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    Banking,
    Credit,
    Investing,
    PropertyDebt,
}

impl Group {
    const ALL: [Self; 4] = [
        Self::Banking,
        Self::Credit,
        Self::Investing,
        Self::PropertyDebt,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Banking => "Banking",
            Self::Credit => "Credit Cards",
            Self::Investing => "Investing",
            Self::PropertyDebt => "Property & Debt",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Checking,
    Savings,
    Credit,
    Investment,
    Asset,
    Loan,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Checking => "Checking",
            Self::Savings => "Savings",
            Self::Credit => "Credit card",
            Self::Investment => "Investment",
            Self::Asset => "Asset",
            Self::Loan => "Loan",
        }
    }

    fn group(self) -> Group {
        match self {
            Self::Checking | Self::Savings => Group::Banking,
            Self::Credit => Group::Credit,
            Self::Investment => Group::Investing,
            Self::Asset | Self::Loan => Group::PropertyDebt,
        }
    }
}

/// (name, kind, transactions, opening balance in cents)
const ACCOUNT_SPECS: &[(&str, Kind, usize, i64)] = &[
    ("Everyday Checking", Kind::Checking, 5_000, 250_000),
    ("Joint Checking", Kind::Checking, 3_200, 400_000),
    ("Bills Checking", Kind::Checking, 1_800, 150_000),
    ("Cash", Kind::Checking, 600, 20_000),
    ("Old Checking", Kind::Checking, 400, 80_000),
    ("High-Yield Savings", Kind::Savings, 900, 1_000_000),
    ("Emergency Fund", Kind::Savings, 500, 1_500_000),
    ("Holiday Savings", Kind::Savings, 500, 50_000),
    ("Kids' Savings", Kind::Savings, 500, 30_000),
    ("Visa Rewards", Kind::Credit, 2_500, 0),
    ("Mastercard", Kind::Credit, 1_400, 0),
    ("Store Card", Kind::Credit, 500, 0),
    ("Travel Card", Kind::Credit, 800, 0),
    ("Brokerage", Kind::Investment, 1_200, 2_000_000),
    ("HSA", Kind::Investment, 500, 100_000),
    ("529 College Fund", Kind::Investment, 500, 200_000),
    ("401(k)", Kind::Investment, 900, 3_000_000),
    ("Roth IRA", Kind::Investment, 600, 800_000),
    ("Traditional IRA", Kind::Investment, 500, 1_200_000),
    ("House", Kind::Asset, 500, 35_000_000),
    ("Car", Kind::Asset, 500, 2_800_000),
    ("Mortgage", Kind::Loan, 500, -28_000_000),
    ("Car Loan", Kind::Loan, 500, -2_400_000),
    ("Student Loan", Kind::Loan, 500, -3_500_000),
    ("Personal Loan", Kind::Loan, 500, -600_000),
    ("Boat Loan", Kind::Loan, 300, -1_500_000),
];

/// The seeded user layout: `#name` is a section, `---` a separator, anything else an account.
const SEED_LAYOUT: &[&str] = &[
    "#Everyday",
    "Everyday Checking",
    "Joint Checking",
    "Bills Checking",
    "---",
    "Cash",
    "Old Checking",
    "#Savings goals",
    "High-Yield Savings",
    "Emergency Fund",
    "Holiday Savings",
    "Kids' Savings",
    "#Credit",
    "Visa Rewards",
    "Mastercard",
    "Store Card",
    "Travel Card",
    "#Investments",
    "Brokerage",
    "HSA",
    "529 College Fund",
    "#Retirement",
    "401(k)",
    "Roth IRA",
    "Traditional IRA",
    "#Home & loans",
    "House",
    "Car",
    "---",
    "Mortgage",
    "Car Loan",
    "Student Loan",
    "Personal Loan",
    "Boat Loan",
];

const SPENDING: &[(&str, &str, i64)] = &[
    ("Grocery Mart", "Food:Groceries", 18_000),
    ("Corner Bakery", "Food:Groceries", 2_500),
    ("Coffee Corner", "Food:Dining", 1_200),
    ("Noodle House", "Food:Dining", 6_000),
    ("City Power", "Utilities:Electric", 14_000),
    ("Water Works", "Utilities:Water", 6_000),
    ("FiberNet", "Utilities:Internet", 7_000),
    ("Gas & Go", "Auto:Fuel", 6_500),
    ("Quick Lube", "Auto:Service", 9_000),
    ("Book Barn", "Leisure:Books", 3_500),
    ("Cinema 8", "Leisure:Movies", 3_000),
    ("Pharmacy Plus", "Medical:Prescriptions", 4_000),
    ("Hardware Haus", "Home:Repairs", 12_000),
    ("Online Market", "Shopping:General", 8_000),
];

struct Txn {
    day: i64,
    payee: &'static str,
    category: &'static str,
    amount: i64,
    balance: i64,
}

struct Account {
    name: String,
    kind: Kind,
    txns: Arc<Vec<Txn>>,
    balance: i64,
    hidden: bool,
    closed: bool,
    deleted: bool,
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

/// Days since 1970-01-01 of 2016-01-01; the stub history runs about ten years from there.
const FIRST_DAY: i64 = 16_801;
const HISTORY_DAYS: i64 = 3_900;

fn generate() -> Vec<Account> {
    let mut rng = Lcg(SEED);
    ACCOUNT_SPECS
        .iter()
        .map(|&(name, kind, n, opening)| {
            let mut balance = opening;
            let mut txns = Vec::with_capacity(n + 2);
            txns.push(Txn {
                day: FIRST_DAY,
                payee: "Opening Balance",
                category: "[Equity]",
                amount: opening,
                balance,
            });
            // Old Checking stopped being used two years before the end of the history.
            let span = if name == "Old Checking" {
                HISTORY_DAYS - 730
            } else {
                HISTORY_DAYS
            };
            for i in 0..n {
                let day = FIRST_DAY + 1 + (i as i64 * span) / n as i64;
                let roll = rng.below(100);
                let (payee, category, amount) = match kind {
                    // Surplus checking money moves to savings.
                    Kind::Checking if balance > 1_200_000 && name != "Old Checking" => (
                        "Transfer to High-Yield Savings",
                        "[High-Yield Savings]",
                        -rng.around(700_000),
                    ),
                    // Cards are paid in full once the statement grows.
                    Kind::Credit if balance < -120_000 => {
                        ("Payment — Thank You", "[Everyday Checking]", -balance)
                    }
                    Kind::Checking if roll < 5 => {
                        ("Paycheck", "Income:Salary", rng.around(280_000))
                    }
                    Kind::Checking if roll < 8 => {
                        ("Landlord", "Housing:Rent", -rng.around(150_000))
                    }
                    Kind::Checking => {
                        let (p, c, typical) = SPENDING[rng.below(SPENDING.len() as u64) as usize];
                        (p, c, -rng.around(typical))
                    }
                    Kind::Savings if roll < 30 => ("Interest", "Income:Interest", rng.around(800)),
                    Kind::Savings if roll < 85 => (
                        "Transfer from Checking",
                        "[Everyday Checking]",
                        rng.around(30_000),
                    ),
                    Kind::Savings => (
                        "Transfer to Checking",
                        "[Everyday Checking]",
                        -rng.around(40_000),
                    ),
                    Kind::Credit => {
                        let (p, c, typical) = SPENDING[rng.below(SPENDING.len() as u64) as usize];
                        (p, c, -rng.around(typical))
                    }
                    Kind::Investment if roll < 40 => {
                        ("Contribution", "[Everyday Checking]", rng.around(50_000))
                    }
                    Kind::Investment if roll < 75 => {
                        ("Market Gain", "Investments:Unrealized", rng.around(60_000))
                    }
                    Kind::Investment => {
                        ("Market Loss", "Investments:Unrealized", -rng.around(45_000))
                    }
                    Kind::Asset if roll < if name == "Car" { 15 } else { 60 } => {
                        ("Revaluation", "Property:Value Change", rng.around(20_000))
                    }
                    Kind::Asset => (
                        "Depreciation",
                        "Property:Value Change",
                        -rng.around(if name == "Car" { 4_000 } else { 15_000 }),
                    ),
                    Kind::Loan if roll < 90 => (
                        "Principal Payment",
                        "[Everyday Checking]",
                        rng.around(-opening * 8 / (10 * n as i64)),
                    ),
                    Kind::Loan => (
                        "Interest Charged",
                        "Interest:Loans",
                        -rng.around(-opening / (20 * n as i64)),
                    ),
                };
                // Paychecks keep checking accounts from drifting far below zero.
                let amount = if matches!(kind, Kind::Checking) && balance + amount < -50_000 {
                    -amount
                } else {
                    amount
                };
                balance += amount;
                txns.push(Txn {
                    day,
                    payee,
                    category,
                    amount,
                    balance,
                });
            }
            if name == "Old Checking" {
                let amount = -balance;
                balance = 0;
                txns.push(Txn {
                    day: FIRST_DAY + span + 1,
                    payee: "Transfer to Everyday Checking",
                    category: "[Everyday Checking]",
                    amount,
                    balance,
                });
            }
            Account {
                name: name.to_owned(),
                kind,
                txns: Arc::new(txns),
                balance,
                hidden: name == "Store Card",
                closed: name == "Old Checking",
                deleted: false,
            }
        })
        .collect()
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
    format!("{m}/{d}/{y}")
}

// ---------------------------------------------------------------------------------------------
// The Account bar layout: a flat list. A section owns the accounts below it up to the next
// section; a separator is a divider between two accounts.

#[derive(Clone, PartialEq, Eq)]
enum Item {
    Section { id: u64, name: String },
    Separator { id: u64 },
    Account(usize),
}

impl Item {
    fn is_section(&self) -> bool {
        matches!(self, Item::Section { .. })
    }
}

fn seed_layout(accounts: &[Account]) -> Vec<Item> {
    let mut id = 0;
    SEED_LAYOUT
        .iter()
        .map(|&s| {
            id += 1;
            if let Some(name) = s.strip_prefix('#') {
                Item::Section {
                    id,
                    name: name.to_owned(),
                }
            } else if s == "---" {
                Item::Separator { id }
            } else {
                Item::Account(accounts.iter().position(|a| a.name == s).unwrap())
            }
        })
        .collect()
}

/// The layout range `item` moves as: a section takes its accounts with it.
fn block(layout: &[Item], i: usize) -> std::ops::Range<usize> {
    if layout[i].is_section() {
        let end = layout[i + 1..]
            .iter()
            .position(Item::is_section)
            .map_or(layout.len(), |p| i + 1 + p);
        i..end
    } else {
        i..i + 1
    }
}

/// Move the item at `from` (with its block) so it lands before index `to` of the current layout.
/// A section only lands on a section boundary, so it never splits another section.
fn move_item(layout: &mut Vec<Item>, from: usize, mut to: usize) -> bool {
    let b = block(layout, from);
    if layout[from].is_section() {
        while to < layout.len() && !layout[to].is_section() && to > 0 {
            to += 1;
        }
    }
    if to >= b.start && to <= b.end {
        return false;
    }
    let moved: Vec<Item> = layout.drain(b.clone()).collect();
    if to > b.start {
        to -= b.len();
    }
    for (k, it) in moved.into_iter().enumerate() {
        layout.insert(to + k, it);
    }
    true
}

/// One step up or down, as the ↑ / ↓ buttons do. An account steps across a section header into
/// the neighbouring section; a section steps past the whole neighbouring section.
fn step_item(layout: &mut Vec<Item>, i: usize, up: bool) -> bool {
    if layout[i].is_section() {
        if up {
            let Some(prev) = layout[..i].iter().rposition(Item::is_section) else {
                return move_item(layout, i, 0);
            };
            move_item(layout, i, prev)
        } else {
            let b = block(layout, i);
            if b.end >= layout.len() {
                return false;
            }
            let next = block(layout, b.end);
            move_item(layout, i, next.end)
        }
    } else if up {
        i > 0 && {
            layout.swap(i, i - 1);
            true
        }
    } else {
        i + 1 < layout.len() && {
            layout.swap(i, i + 1);
            true
        }
    }
}

/// The section id an account at layout index `i` belongs to.
fn section_of(layout: &[Item], i: usize) -> Option<u64> {
    layout[..i].iter().rev().find_map(|it| match it {
        Item::Section { id, .. } => Some(*id),
        _ => None,
    })
}

/// Move the account at `from` to the end of section `to` (`None`: before the first section).
fn move_to_section(layout: &mut Vec<Item>, from: usize, to: Option<u64>) {
    let item = layout.remove(from);
    let at = match to {
        None => layout
            .iter()
            .position(Item::is_section)
            .unwrap_or(layout.len()),
        Some(sid) => {
            let start = layout
                .iter()
                .position(|it| matches!(it, Item::Section { id, .. } if *id == sid))
                .unwrap();
            block(layout, start).end
        }
    };
    layout.insert(at, item);
}

fn sections(layout: &[Item]) -> Vec<(u64, String)> {
    layout
        .iter()
        .filter_map(|it| match it {
            Item::Section { id, name } => Some((*id, name.clone())),
            _ => None,
        })
        .collect()
}

/// Dragging a layout item, in the Account bar or in Manage Accounts.
#[derive(Clone)]
struct Drag {
    from: usize,
    label: String,
    place: &'static str,
}

/// Call after drawing a row at layout index `index` covering `rect`: while a layout item from
/// `place` is dragged over the row, mark where it would land; on release, report the move.
fn drop_zone(ui: &Ui, place: &str, rect: Rect, index: usize, out: &mut Option<(usize, usize)>) {
    let Some(p) = DragAndDrop::payload::<Drag>(ui.ctx()) else {
        return;
    };
    if p.place != place {
        return;
    }
    let Some(pos) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    if pos.y < rect.top() || pos.y >= rect.bottom() {
        return;
    }
    let before = pos.y < rect.center().y;
    let y = if before { rect.top() } else { rect.bottom() };
    let painter = ui
        .ctx()
        .layer_painter(egui::LayerId::new(Order::Tooltip, Id::new("dnd_line")));
    painter.hline(
        rect.x_range(),
        y,
        Stroke::new(2.0, ui.visuals().selection.stroke.color),
    );
    if ui.input(|i| i.pointer.any_released()) {
        *out = Some((p.from, if before { index } else { index + 1 }));
    }
}

/// A grip that starts a drag of layout item `from`, and the ghost that follows the pointer.
fn start_drag(r: &egui::Response, from: usize, label: &str, place: &'static str) {
    if r.drag_started() {
        DragAndDrop::set_payload(
            &r.ctx,
            Drag {
                from,
                label: label.to_owned(),
                place,
            },
        );
    }
    if r.hovered() {
        r.ctx.set_cursor_icon(CursorIcon::Grab);
    }
}

fn drag_ghost(ctx: &Context) {
    let Some(p) = DragAndDrop::payload::<Drag>(ctx) else {
        return;
    };
    ctx.set_cursor_icon(CursorIcon::Grabbing);
    if let Some(pos) = ctx.pointer_interact_pos() {
        let painter = ctx.layer_painter(egui::LayerId::new(Order::Tooltip, Id::new("dnd_ghost")));
        let font = egui::FontId::proportional(13.0);
        let galley = painter.layout_no_wrap(p.label.clone(), font, Color32::WHITE);
        let rect = Rect::from_min_size(pos + vec2(14.0, 6.0), galley.size() + vec2(12.0, 6.0));
        painter.rect_filled(rect, 3.0, Color32::from_rgba_unmultiplied(40, 60, 90, 220));
        painter.galley(rect.min + vec2(6.0, 3.0), galley, Color32::WHITE);
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
        Self::ALL
            .into_iter()
            .find(|v| v.key().eq_ignore_ascii_case(key))
    }

    fn label(self) -> &'static str {
        match self {
            Self::A => "A · Account List table",
            Self::B => "B · Tree + details",
            Self::C => "C · Two tabs: Accounts | Layout",
        }
    }

    fn step(self, delta: isize) -> Self {
        let n = Self::ALL.len() as isize;
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0) as isize;
        Self::ALL[(i + delta).rem_euclid(n) as usize]
    }
}

#[derive(Clone, Copy)]
struct Opts {
    group_by_type: bool,
    section_totals: bool,
    fold: bool,
    sep_line: bool,
    closed_section: bool,
    hidden_in_nw: bool,
    bar_rearrange: bool,
    dnd: bool,
    caps: bool,
    refuse_close: bool,
}

impl Opts {
    fn get_mut(&mut self, key: &str) -> Option<&mut bool> {
        Some(match key {
            "group_by_type" => &mut self.group_by_type,
            "section_totals" => &mut self.section_totals,
            "fold" => &mut self.fold,
            "sep_line" => &mut self.sep_line,
            "closed_section" => &mut self.closed_section,
            "hidden_in_nw" => &mut self.hidden_in_nw,
            "bar_rearrange" => &mut self.bar_rearrange,
            "dnd" => &mut self.dnd,
            "caps" => &mut self.caps,
            "refuse_close" => &mut self.refuse_close,
            _ => return None,
        })
    }
}

/// (key, label, applies to)
const TOGGLES: &[(&str, &str, &str)] = &[
    (
        "group_by_type",
        "Group accounts by type (the Preference)",
        "all",
    ),
    ("section_totals", "Section totals in the Account bar", "all"),
    ("fold", "Sections fold (▸ / ▾)", "all"),
    ("sep_line", "Separator is a line (off: a blank gap)", "all"),
    (
        "closed_section",
        "Closed accounts in a folded Closed section (off: gone from the bar)",
        "all",
    ),
    ("hidden_in_nw", "Hidden accounts count in Net Worth", "all"),
    (
        "bar_rearrange",
        "Rearrange by dragging in the Account bar",
        "all",
    ),
    (
        "dnd",
        "Drag and drop in Manage Accounts (off: ↑ / ↓ buttons)",
        "all",
    ),
    (
        "caps",
        "Section header in small caps (off: bold over a rule)",
        "all",
    ),
    (
        "refuse_close",
        "Refuse to close an account with a balance (off: warn, allow)",
        "all",
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
// Manage Accounts: a draft of the accounts' editable fields and the layout

#[derive(Clone)]
struct AcctEdit {
    name: String,
    hidden: bool,
    closed: bool,
    deleted: bool,
}

/// A light confirm stacked over Manage Accounts.
enum Confirm {
    Delete(usize),
    CloseWithBalance(usize),
    RefuseClose(usize),
}

struct Manage {
    accts: Vec<AcctEdit>,
    layout: Vec<Item>,
    /// Selected layout index.
    sel: Option<usize>,
    /// Variant C's page: false = Accounts, true = Layout.
    layout_page: bool,
    next_id: u64,
    confirm: Option<Confirm>,
    /// A section whose name field should take focus next frame.
    focus_section: Option<u64>,
    fresh: bool,
    opener: Option<Id>,
}

enum Forced {
    Ok,
    Cancel,
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
    layout: Vec<Item>,
    selected: usize,
    selected_row: Option<usize>,
    variant: Variant,
    o: Opts,
    manage: Option<Manage>,
    forced: Option<Forced>,
    folded: BTreeSet<u64>,
    closed_folded: bool,
    tab: usize,
    theme: Theme,
    dock_open: bool,
    status: String,
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
        let accounts = generate();
        let layout = seed_layout(&accounts);
        Self {
            h,
            accounts,
            layout,
            selected: 0,
            selected_row: None,
            variant,
            o: Opts {
                group_by_type: false,
                section_totals: true,
                fold: true,
                sep_line: true,
                closed_section: true,
                hidden_in_nw: true,
                bar_rearrange: false,
                dnd: true,
                caps: true,
                refuse_close: false,
            },
            manage: None,
            forced: None,
            folded: BTreeSet::new(),
            closed_folded: true,
            tab: ACCOUNTS_TAB,
            theme: Theme::new(ThemeSettings::default()),
            dock_open: true,
            status: "Ready".to_owned(),
            new_count: 0,
        }
    }

    fn emit(&mut self, command: &str, source: &str) {
        self.h.event(
            "command",
            json!({ "would_emit": command, "source": source }),
        );
        self.status = format!("{source}: {command}");
    }

    fn set_variant(&mut self, to: Variant, via: &str) {
        if to == self.variant {
            return;
        }
        self.h.event(
            "variant",
            json!({ "from": self.variant.key(), "to": to.key(), "via": via, "manage_open": self.manage.is_some() }),
        );
        self.variant = to;
        self.h.screenshot(format!("variant-{}", to.key()));
    }

    fn set_toggle(&mut self, key: &str, value: Option<bool>, via: &str) {
        let Some(slot) = self.o.get_mut(key) else {
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
                    .position(|a| !a.deleted && a.name.to_lowercase().contains(&needle))
            })
    }

    fn net_worth(&self) -> i64 {
        self.accounts
            .iter()
            .filter(|a| !a.deleted && (self.o.hidden_in_nw || !a.hidden))
            .map(|a| a.balance)
            .sum()
    }

    fn open_manage(&mut self, opener: Option<Id>, select: Option<usize>, via: &str) {
        if self.manage.is_some() {
            return;
        }
        self.emit("AppCommand::OpenManageAccountsDialog", via);
        let accts = self
            .accounts
            .iter()
            .map(|a| AcctEdit {
                name: a.name.clone(),
                hidden: a.hidden,
                closed: a.closed,
                deleted: a.deleted,
            })
            .collect();
        let next_id = self
            .layout
            .iter()
            .filter_map(|it| match it {
                Item::Section { id, .. } | Item::Separator { id } => Some(*id),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            + 1;
        let sel = select.and_then(|a| self.layout.iter().position(|it| *it == Item::Account(a)));
        self.manage = Some(Manage {
            accts,
            layout: self.layout.clone(),
            sel,
            layout_page: false,
            next_id,
            confirm: None,
            focus_section: None,
            fresh: true,
            opener,
        });
        self.h.event(
            "dialog_open",
            json!({ "dialog": "manage", "via": via, "variant": self.variant.key() }),
        );
    }

    /// OK in Manage Accounts: the draft replaces the accounts' fields and the layout.
    fn apply_manage(&mut self, m: Manage) {
        let mut renamed = 0;
        let mut hidden = 0;
        let mut closed = 0;
        let mut deleted = 0;
        for (a, e) in self.accounts.iter_mut().zip(&m.accts) {
            if a.deleted {
                continue;
            }
            if a.name != e.name.trim() && !e.name.trim().is_empty() {
                a.name = e.name.trim().to_owned();
                renamed += 1;
            }
            if a.hidden != e.hidden {
                hidden += 1;
            }
            if a.closed != e.closed {
                closed += 1;
            }
            if e.deleted {
                deleted += 1;
            }
            a.hidden = e.hidden;
            a.closed = e.closed;
            a.deleted = e.deleted;
        }
        let layout: Vec<Item> = m
            .layout
            .into_iter()
            .filter(|it| !matches!(it, Item::Account(i) if self.accounts[*i].deleted))
            .collect();
        let layout_changed = layout != self.layout;
        self.layout = layout;
        if renamed + hidden + closed + deleted > 0 {
            self.emit(
                &format!(
                    "DomainCommand::UpdateAccounts {{ renamed: {renamed}, hidden_changed: {hidden}, closed_changed: {closed}, deleted: {deleted} }}"
                ),
                "Manage Accounts OK",
            );
        }
        if layout_changed {
            self.emit("DomainCommand::SetAccountBarLayout", "Manage Accounts OK");
        }
        if self.accounts[self.selected].deleted {
            self.selected = self.accounts.iter().position(|a| !a.deleted).unwrap_or(0);
        }
    }

    fn close_manage(&mut self, ctx: &Context, reason: &str) {
        let Some(m) = self.manage.take() else { return };
        self.h.event(
            "dialog_close",
            json!({ "dialog": "manage", "reason": reason }),
        );
        let opener = m.opener;
        if reason == "ok" {
            self.apply_manage(m);
        } else {
            self.status = "Manage Accounts cancelled; nothing changed".to_owned();
        }
        if let Some(id) = opener {
            ctx.memory_mut(|mem| mem.request_focus(id));
        }
    }

    /// A new stub account from "+ Add Account": placed after the last account of the same type in
    /// the layout, so it lands beside its siblings.
    fn add_account(&mut self) {
        self.new_count += 1;
        let name = format!("New Savings {}", self.new_count);
        let idx = self.accounts.len();
        self.accounts.push(Account {
            name: name.clone(),
            kind: Kind::Savings,
            txns: Arc::new(vec![Txn {
                day: FIRST_DAY + HISTORY_DAYS,
                payee: "Opening Balance",
                category: "[Equity]",
                amount: 10_000,
                balance: 10_000,
            }]),
            balance: 10_000,
            hidden: false,
            closed: false,
            deleted: false,
        });
        let at = self
            .layout
            .iter()
            .rposition(
                |it| matches!(it, Item::Account(i) if self.accounts[*i].kind == Kind::Savings),
            )
            .map_or(self.layout.len(), |p| p + 1);
        self.layout.insert(at, Item::Account(idx));
        self.selected = idx;
        self.emit(
            &format!("DomainCommand::CreateAccount({name})"),
            "account bar button",
        );
        self.h
            .event("account_added", json!({ "name": name, "layout_index": at }));
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
                Some(i) => {
                    self.selected = i;
                    self.tab = ACCOUNTS_TAB;
                    self.h.event(
                        "navigate",
                        json!({ "to": self.accounts[i].name, "via": "control" }),
                    );
                }
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
            "dialog" if rest == "manage" => self.open_manage(None, None, "control"),
            "ok" => self.forced = Some(Forced::Ok),
            "cancel" => self.forced = Some(Forced::Cancel),
            "select" => {
                let needle = rest.to_lowercase();
                let accounts = &self.accounts;
                if let Some(m) = &mut self.manage {
                    let found = m.layout.iter().position(|it| match it {
                        Item::Section { name, .. } => name.to_lowercase().contains(&needle),
                        Item::Account(i) => accounts[*i].name.to_lowercase().contains(&needle),
                        Item::Separator { .. } => false,
                    });
                    match found {
                        Some(i) => m.sel = Some(i),
                        None => bad(self),
                    }
                } else {
                    bad(self);
                }
            }
            "page" => match &mut self.manage {
                Some(m) => m.layout_page = rest == "layout",
                None => bad(self),
            },
            "fold" | "unfold" => {
                let needle = rest.to_lowercase();
                if needle == "closed" {
                    self.closed_folded = verb == "fold";
                } else {
                    match sections(&self.layout)
                        .into_iter()
                        .find(|(_, n)| n.to_lowercase().contains(&needle))
                    {
                        Some((id, _)) => {
                            if verb == "fold" {
                                self.folded.insert(id);
                            } else {
                                self.folded.remove(&id);
                            }
                        }
                        None => bad(self),
                    }
                }
            }
            "reload" => {
                self.accounts = generate();
                self.layout = seed_layout(&self.accounts);
                self.manage = None;
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
    fn on_exit(&mut self) {
        self.h.event("session_end", json!({}));
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.global_style().interaction.selectable_labels {
            ctx.all_styles_mut(|s| s.interaction.selectable_labels = false);
        }
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
        self.h.prompt_panel(ui);
        self.main_ui(ui, &ctx);
        self.manage_dialog(&ctx);
        drag_ghost(&ctx);
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
        let blocked = self.manage.is_some();
        let menu = Panel::top("menu").show(ui, |ui| {
            ui.add_enabled_ui(!blocked, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if self.menu_item(ui, "File", "Exit") {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                    });
                    ui.menu_button("Tools", |ui| {
                        if self.menu_item(ui, "Tools", "Manage Accounts…") {
                            self.open_manage(None, None, "menu Tools");
                        }
                        if self.menu_item(ui, "Tools", "Preferences…") {
                            self.emit("AppCommand::OpenPreferencesDialog", "menu Tools");
                        }
                    });
                });
            });
        });
        self.h.region(menu.response.rect, "menu bar");
        let tabs = Panel::top("tabs").show(ui, |ui| {
            ui.add_enabled_ui(!blocked, |ui| {
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
        });
        self.h.region(tabs.response.rect, "tab row");
        let status = Panel::bottom("status").show(ui, |ui| {
            ui.label(&self.status);
        });
        self.h.region(status.response.rect, "status bar");
        if self.tab == ACCOUNTS_TAB {
            let bar = Panel::left("accounts").default_size(290.0).show(ui, |ui| {
                ui.add_enabled_ui(!blocked, |ui| self.account_bar(ui));
            });
            self.h.region(bar.response.rect, "account bar");
        }
        CentralPanel::default().show(ui, |ui| {
            if self.tab != ACCOUNTS_TAB {
                ui.heading(TABS[self.tab]);
                ui.label(RichText::new("Not in this prototype. Open the Accounts tab.").weak());
                return;
            }
            ui.add_enabled_ui(!blocked, |ui| self.register_view(ui));
        });
    }

    fn section_header(
        &mut self,
        ui: &mut Ui,
        id: Option<u64>,
        name: &str,
        total: i64,
        count: usize,
        folded: bool,
    ) -> egui::Response {
        let o = self.o;
        let h = self.theme.row_height() + 4.0;
        let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
        let v = ui.visuals().clone();
        if r.hovered() && o.fold {
            ui.painter()
                .rect_filled(rect, 2.0, v.widgets.hovered.weak_bg_fill);
        }
        let mut x = rect.left() + 4.0;
        if o.fold {
            paint_arrow(
                ui.painter(),
                egui::pos2(x + 4.0, rect.center().y),
                if folded { Arrow::Right } else { Arrow::Down },
                v.weak_text_color(),
            );
            x += 14.0;
        }
        let (label, font, color) = if o.caps {
            (
                name.to_uppercase(),
                egui::FontId::proportional(11.0),
                v.weak_text_color(),
            )
        } else {
            (
                name.to_owned(),
                egui::FontId::proportional(13.5),
                v.strong_text_color(),
            )
        };
        let label = if folded {
            format!("{label}  ({count})")
        } else {
            label
        };
        ui.painter().text(
            egui::pos2(x, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            font.clone(),
            color,
        );
        if o.section_totals {
            ui.painter().text(
                rect.right_center() - vec2(12.0, 0.0),
                Align2::RIGHT_CENTER,
                fmt_cents(total),
                font,
                if total < 0 { v.error_fg_color } else { color },
            );
        }
        if !o.caps {
            ui.painter().hline(
                rect.x_range().shrink(2.0),
                rect.bottom() - 0.5,
                Stroke::new(1.0, v.widgets.noninteractive.bg_stroke.color),
            );
        }
        self.h.track(
            &r,
            format!("account bar: section header {}", id.map_or(name, |_| name)),
        );
        r
    }

    fn separator(&self, ui: &mut Ui) {
        if self.o.sep_line {
            ui.add_space(3.0);
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().hline(
                rect.x_range().shrink(14.0),
                rect.center().y,
                Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
            );
            ui.add_space(3.0);
        } else {
            ui.add_space(9.0);
        }
    }

    fn account_bar(&mut self, ui: &mut Ui) {
        let mut add = false;
        let footer = Panel::bottom("account_bar_footer").show(ui, |ui| {
            let total = self.net_worth();
            Sides::new().show(
                ui,
                |ui| ui.strong("Net Worth"),
                |ui| ui.strong(fmt_cents(total)),
            );
            let r = ui.add_sized([ui.available_width(), 22.0], Button::new("+ Add Account"));
            self.h.track(&r, "account bar: + Add Account");
            add = r.clicked();
        });
        self.h.region(footer.response.rect, "account bar footer");
        let mut pick: Option<(usize, &'static str)> = None;
        let mut moved = None;
        let out = egui::ScrollArea::vertical().show(ui, |ui| {
            if self.o.group_by_type {
                self.bar_by_type(ui, &mut pick);
            } else {
                self.bar_by_layout(ui, &mut pick, &mut moved);
            }
            self.closed_section(ui, &mut pick);
        });
        self.h
            .scroll("account bar", out.state.offset.y, self.theme.row_height());
        if add {
            self.add_account();
        }
        if let Some((from, to)) = moved
            && move_item(&mut self.layout, from, to)
        {
            self.h.event(
                "layout_move",
                json!({ "from": from, "to": to, "how": "drag", "where": "account bar" }),
            );
            self.emit("DomainCommand::SetAccountBarLayout", "account bar drag");
        }
        let Some((i, what)) = pick else { return };
        match what {
            "select" => {
                self.selected = i;
                self.selected_row = None;
                self.h.event(
                    "navigate",
                    json!({ "to": self.accounts[i].name, "via": "click" }),
                );
            }
            "tab" => self.emit("AppCommand::OpenRegisterTab", "account menu"),
            "reconcile" => self.emit("AppCommand::OpenReconcileDialog", "account menu"),
            _ => self.open_manage(None, Some(i), "account menu"),
        }
    }

    fn bar_account(
        &mut self,
        ui: &mut Ui,
        i: usize,
        layout_index: Option<usize>,
        pick: &mut Option<(usize, &'static str)>,
    ) -> egui::Response {
        let drag = layout_index.is_some() && self.o.bar_rearrange && !self.o.group_by_type;
        let a = &self.accounts[i];
        let name = a.name.clone();
        let r = account_row(
            ui,
            self.selected == i,
            &name,
            a.balance,
            self.theme.row_height(),
            a.closed,
            drag,
        );
        self.h.track(&r, format!("account bar: {name}"));
        if let Some(li) = layout_index
            && drag
        {
            start_drag(&r, li, &name, "bar");
        }
        if r.clicked() {
            *pick = Some((i, "select"));
        }
        r.context_menu(|ui| {
            for (label, what) in [
                ("Open Register", "select"),
                ("Open in New Tab", "tab"),
                ("Reconcile…", "reconcile"),
                ("Manage Accounts…", "manage"),
            ] {
                if what == "manage" {
                    ui.separator();
                }
                if self.menu_item(ui, "account menu", label) {
                    *pick = Some((i, what));
                }
            }
        });
        r
    }

    fn visible_in_bar(&self, i: usize) -> bool {
        let a = &self.accounts[i];
        !a.deleted && !a.hidden && !a.closed
    }

    fn bar_by_type(&mut self, ui: &mut Ui, pick: &mut Option<(usize, &'static str)>) {
        for group in Group::ALL {
            let members: Vec<usize> = (0..self.accounts.len())
                .filter(|&i| self.accounts[i].kind.group() == group && self.visible_in_bar(i))
                .collect();
            if members.is_empty() {
                continue;
            }
            let total = members.iter().map(|&i| self.accounts[i].balance).sum();
            let gid = 1_000_000 + group as u64;
            let folded = self.o.fold && self.folded.contains(&gid);
            let r = self.section_header(ui, Some(gid), group.name(), total, members.len(), folded);
            if r.clicked() && self.o.fold {
                self.toggle_fold(gid, group.name());
            }
            if folded {
                continue;
            }
            for i in members {
                self.bar_account(ui, i, None, pick);
            }
        }
    }

    fn toggle_fold(&mut self, id: u64, name: &str) {
        let folded = if self.folded.remove(&id) {
            false
        } else {
            self.folded.insert(id);
            true
        };
        self.h
            .event("section_fold", json!({ "section": name, "folded": folded }));
    }

    fn bar_by_layout(
        &mut self,
        ui: &mut Ui,
        pick: &mut Option<(usize, &'static str)>,
        moved: &mut Option<(usize, usize)>,
    ) {
        let layout = self.layout.clone();
        let mut folded_now = false;
        let mut k = 0;
        while k < layout.len() {
            match &layout[k] {
                Item::Section { id, name } => {
                    let b = block(&layout, k);
                    let members: Vec<usize> = layout[b.clone()]
                        .iter()
                        .filter_map(|it| match it {
                            Item::Account(i) if self.visible_in_bar(*i) => Some(*i),
                            _ => None,
                        })
                        .collect();
                    let total = members.iter().map(|&i| self.accounts[i].balance).sum();
                    folded_now = self.o.fold && self.folded.contains(id);
                    ui.add_space(2.0);
                    let r =
                        self.section_header(ui, Some(*id), name, total, members.len(), folded_now);
                    if self.o.bar_rearrange {
                        let r = r.interact(Sense::click_and_drag());
                        start_drag(&r, k, name, "bar");
                        drop_zone(ui, "bar", r.rect, k, moved);
                    }
                    if r.clicked() && self.o.fold {
                        self.toggle_fold(*id, name);
                    }
                }
                Item::Separator { .. } => {
                    if !folded_now {
                        let top = ui.cursor().top();
                        self.separator(ui);
                        let rect =
                            Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.cursor().top());
                        drop_zone(ui, "bar", rect, k, moved);
                    }
                }
                Item::Account(i) => {
                    if !folded_now && self.visible_in_bar(*i) {
                        let r = self.bar_account(ui, *i, Some(k), pick);
                        drop_zone(ui, "bar", r.rect, k, moved);
                    }
                }
            }
            k += 1;
        }
    }

    fn closed_section(&mut self, ui: &mut Ui, pick: &mut Option<(usize, &'static str)>) {
        if !self.o.closed_section {
            return;
        }
        let closed: Vec<usize> = (0..self.accounts.len())
            .filter(|&i| {
                let a = &self.accounts[i];
                !a.deleted && !a.hidden && a.closed
            })
            .collect();
        if closed.is_empty() {
            return;
        }
        ui.add_space(6.0);
        let total = closed.iter().map(|&i| self.accounts[i].balance).sum();
        // The Closed section always folds, whatever the fold toggle says.
        let saved = self.o.fold;
        self.o.fold = true;
        let r = self.section_header(ui, None, "Closed", total, closed.len(), self.closed_folded);
        self.o.fold = saved;
        if r.clicked() {
            self.closed_folded = !self.closed_folded;
            self.h.event(
                "section_fold",
                json!({ "section": "Closed", "folded": self.closed_folded }),
            );
        }
        if !self.closed_folded {
            for i in closed {
                self.bar_account(ui, i, None, pick);
            }
        }
    }

    fn register_view(&mut self, ui: &mut Ui) {
        let sel = self.selected;
        let a = &self.accounts[sel];
        let closed = a.closed;
        let hidden = a.hidden;
        let name = a.name.clone();
        let facts = format!("{} · {} transactions", a.kind.name(), a.txns.len());
        let balance = a.balance;
        let header = ui.horizontal(|ui| {
            ui.heading(name);
            ui.label(RichText::new(fmt_cents(balance)).strong());
            ui.label(RichText::new(facts).weak());
            if closed {
                ui.label(RichText::new("Closed — read-only").color(ui.visuals().warn_fg_color));
            }
            if hidden {
                ui.label(RichText::new("Hidden from the Account bar").weak());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let r = ui.button("Manage Accounts…");
                self.h.track(&r, "context header: Manage Accounts…");
                if r.clicked() {
                    self.open_manage(Some(r.id), Some(sel), "context header");
                }
                let r = ui.add_enabled(!closed, Button::new("+ Transaction"));
                self.h.track(&r, "context header: + Transaction");
                if r.clicked() {
                    self.emit("AppCommand::NewTransaction", "context header");
                }
            });
        });
        self.h.region(header.response.rect, "context header");
        ui.separator();
        let txns = Arc::clone(&self.accounts[sel].txns);
        let n = txns.len();
        let row_h = self.theme.row_height();
        let mut clicked = None;
        let top = ui.cursor().min;
        TableBuilder::new(ui)
            .id_salt(("register", sel))
            .striped(true)
            .resizable(true)
            .sense(Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(88.0))
            .column(Column::initial(210.0).at_least(80.0))
            .column(Column::initial(190.0).at_least(80.0))
            .column(Column::initial(100.0))
            .column(Column::remainder().at_least(100.0))
            .header(row_h + 2.0, |mut h| {
                for t in ["Date", "Payee", "Category", "Amount", "Balance"] {
                    h.col(|ui| {
                        ui.strong(t);
                    });
                }
            })
            .body(|body| {
                body.rows(row_h, n, |mut row| {
                    let r = n - 1 - row.index();
                    let t = &txns[r];
                    row.set_selected(self.selected_row == Some(r));
                    row.col(|ui| {
                        ui.label(fmt_day(t.day));
                    });
                    row.col(|ui| {
                        ui.label(t.payee);
                    });
                    row.col(|ui| {
                        ui.label(t.category);
                    });
                    row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(fmt_cents(t.amount));
                        });
                    });
                    row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(fmt_cents(t.balance));
                        });
                    });
                    if row.response().clicked() {
                        clicked = Some(r);
                    }
                });
            });
        if let Some(r) = clicked {
            self.selected_row = Some(r);
            self.h
                .event("click", json!({ "widget": format!("register row {r}") }));
        }
        self.h
            .region(Rect::from_min_max(top, ui.max_rect().max), "register");
    }
}

/// One account in the Account bar, as the shell layout decided (#15): the whole row is the
/// click target and the highlight, the name left-aligned and the balance right-aligned.
fn account_row(
    ui: &mut Ui,
    selected: bool,
    name: &str,
    balance: i64,
    height: f32,
    closed: bool,
    drag: bool,
) -> egui::Response {
    let sense = if drag {
        Sense::click_and_drag()
    } else {
        Sense::click()
    };
    let (rect, row) = ui.allocate_exact_size(vec2(ui.available_width(), height), sense);
    let v = ui.visuals();
    if selected {
        ui.painter().rect_filled(rect, 2.0, v.selection.bg_fill);
    } else if row.hovered() {
        ui.painter()
            .rect_filled(rect, 2.0, v.widgets.hovered.weak_bg_fill);
    }
    let font = egui::TextStyle::Body.resolve(ui.style());
    let text = if selected {
        v.selection.stroke.color
    } else if closed {
        v.weak_text_color()
    } else {
        v.text_color()
    };
    let money = if balance < 0 && !selected {
        v.error_fg_color
    } else {
        text
    };
    if drag && row.hovered() {
        paint_grip(
            ui.painter(),
            rect.left_center() + vec2(6.0, 0.0),
            v.weak_text_color(),
        );
    }
    ui.painter().text(
        rect.left_center() + vec2(14.0, 0.0),
        Align2::LEFT_CENTER,
        name,
        font.clone(),
        text,
    );
    ui.painter().text(
        rect.right_center() - vec2(12.0, 0.0),
        Align2::RIGHT_CENTER,
        fmt_cents(balance),
        font,
        money,
    );
    row
}

// ---------------------------------------------------------------------------------------------
// Manage Accounts

/// What a Manage Accounts frame asks the host to do once the draft is released.
#[derive(Default)]
struct Out {
    moved: Option<(usize, usize, &'static str)>,
    step: Option<(usize, bool)>,
    to_section: Option<(usize, Option<u64>)>,
    remove: Option<usize>,
    add_section: bool,
    add_separator: bool,
    confirm: Option<Confirm>,
    events: Vec<(&'static str, serde_json::Value)>,
    tracks: Vec<(egui::Response, String)>,
}

const W_HANDLE: f32 = 46.0;
const W_NAME: f32 = 230.0;
const W_TYPE: f32 = 86.0;
const W_SECTION: f32 = 130.0;
const W_BAL: f32 = 112.0;
const W_FLAG: f32 = 58.0;
const W_ACT: f32 = 76.0;

struct Mcx<'a> {
    accounts: &'a [Account],
    o: Opts,
    row_h: f32,
}

impl Mcx<'_> {
    fn label(&self, m: &Manage, it: &Item) -> String {
        match it {
            Item::Section { name, .. } => name.clone(),
            Item::Separator { .. } => "separator".to_owned(),
            Item::Account(i) => m.accts[*i].name.clone(),
        }
    }

    fn section_total(&self, m: &Manage, k: usize) -> i64 {
        m.layout[block(&m.layout, k)]
            .iter()
            .filter_map(|it| match it {
                Item::Account(i) if !m.accts[*i].deleted => Some(self.accounts[*i].balance),
                _ => None,
            })
            .sum()
    }
}

/// A row with a stripe/selection background painted behind its widgets.
fn table_row(ui: &mut Ui, row_h: f32, k: usize, selected: bool, add: impl FnOnce(&mut Ui)) -> Rect {
    let bg = ui.painter().add(Shape::Noop);
    let r = ui.horizontal(|ui| {
        ui.set_min_height(row_h + 4.0);
        add(ui);
    });
    let rect = Rect::from_x_y_ranges(ui.max_rect().x_range(), r.response.rect.y_range());
    let v = ui.visuals();
    let fill = if selected {
        Some(v.selection.bg_fill.gamma_multiply(0.55))
    } else if k % 2 == 1 {
        Some(v.faint_bg_color)
    } else {
        None
    };
    if let Some(fill) = fill {
        ui.painter().set(bg, Shape::rect_filled(rect, 2.0, fill));
    }
    rect
}

/// The grip or the ↑ / ↓ buttons that move layout item `k`.
fn handle(ui: &mut Ui, cx: &Mcx<'_>, m: &Manage, k: usize, out: &mut Out) {
    let label = cx.label(m, &m.layout[k]);
    ui.allocate_ui_with_layout(
        vec2(W_HANDLE, cx.row_h),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(W_HANDLE);
            if cx.o.dnd {
                let (rect, r) = ui.allocate_exact_size(vec2(20.0, cx.row_h), Sense::drag());
                let c = if r.hovered() {
                    ui.visuals().strong_text_color()
                } else {
                    ui.visuals().weak_text_color()
                };
                paint_grip(ui.painter(), rect.center(), c);
                start_drag(&r, k, &label, "manage");
                out.tracks.push((r, format!("manage: grip {label}")));
            } else {
                let up = arrow_button(ui, Arrow::Up);
                if up.clicked() {
                    out.step = Some((k, true));
                }
                out.tracks.push((up, format!("manage: up {label}")));
                let down = arrow_button(ui, Arrow::Down);
                if down.clicked() {
                    out.step = Some((k, false));
                }
                out.tracks.push((down, format!("manage: down {label}")));
            }
        },
    );
}

#[derive(Clone, Copy)]
enum Arrow {
    Up,
    Down,
    Right,
}

/// A small filled triangle; the theme's font has no arrow glyphs.
fn paint_arrow(painter: &egui::Painter, c: egui::Pos2, dir: Arrow, color: Color32) {
    let s = 4.0;
    let pts = match dir {
        Arrow::Up => vec![
            c + vec2(-s, s * 0.6),
            c + vec2(s, s * 0.6),
            c + vec2(0.0, -s * 0.8),
        ],
        Arrow::Down => vec![
            c + vec2(-s, -s * 0.6),
            c + vec2(s, -s * 0.6),
            c + vec2(0.0, s * 0.8),
        ],
        Arrow::Right => vec![
            c + vec2(-s * 0.6, -s),
            c + vec2(-s * 0.6, s),
            c + vec2(s * 0.8, 0.0),
        ],
    };
    painter.add(Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// Six dots, the drag grip.
fn paint_grip(painter: &egui::Painter, c: egui::Pos2, color: Color32) {
    for dx in [-2.5, 2.5] {
        for dy in [-5.0, 0.0, 5.0] {
            painter.circle_filled(c + vec2(dx, dy), 1.3, color);
        }
    }
}

fn arrow_button(ui: &mut Ui, dir: Arrow) -> egui::Response {
    let r = ui.add(Button::new("").min_size(vec2(18.0, 18.0)));
    let c = ui.visuals().widgets.style(&r).fg_stroke.color;
    paint_arrow(ui.painter(), r.rect.center(), dir, c);
    r
}

fn separator_line(ui: &mut Ui, w: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(w, 10.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.5, ui.visuals().weak_text_color()),
    );
}

fn fixed(ui: &mut Ui, w: f32, h: f32, right: bool, add: impl FnOnce(&mut Ui)) {
    let layout = if right {
        Layout::right_to_left(Align::Center)
    } else {
        Layout::left_to_right(Align::Center)
    };
    ui.allocate_ui_with_layout(vec2(w, h), layout, |ui| {
        ui.set_min_width(w);
        ui.set_max_width(w);
        add(ui);
    });
}

fn money_label(ui: &mut Ui, cents: i64) {
    let c = if cents < 0 {
        ui.visuals().error_fg_color
    } else {
        ui.visuals().text_color()
    };
    ui.label(RichText::new(fmt_cents(cents)).color(c));
}

/// Hidden and Closed checkboxes; ticking Closed on an account with a balance asks first.
fn flags(ui: &mut Ui, cx: &Mcx<'_>, m: &mut Manage, i: usize, out: &mut Out, labelled: bool) {
    let name = m.accts[i].name.clone();
    let mut hidden = m.accts[i].hidden;
    let r = ui.checkbox(
        &mut hidden,
        if labelled { "Hide in Account bar" } else { "" },
    );
    if r.changed() {
        m.accts[i].hidden = hidden;
        out.events
            .push(("manage_edit", json!({ "account": name, "hidden": hidden })));
    }
    out.tracks.push((r, format!("manage: hidden {name}")));
    if !labelled {
        ui.add_space(W_FLAG - 24.0);
    }
    let mut closed = m.accts[i].closed;
    let r = ui.checkbox(&mut closed, if labelled { "Closed" } else { "" });
    if r.changed() {
        if closed && cx.accounts[i].balance != 0 {
            out.confirm = Some(if cx.o.refuse_close {
                Confirm::RefuseClose(i)
            } else {
                Confirm::CloseWithBalance(i)
            });
            out.events.push((
                "manage_close_balance",
                json!({ "account": name, "balance": fmt_cents(cx.accounts[i].balance), "refuse": cx.o.refuse_close }),
            ));
        } else {
            m.accts[i].closed = closed;
            out.events
                .push(("manage_edit", json!({ "account": name, "closed": closed })));
        }
    }
    out.tracks.push((r, format!("manage: closed {name}")));
}

fn delete_button(ui: &mut Ui, m: &Manage, i: usize, out: &mut Out) {
    let r = ui.small_button("Delete…");
    if r.clicked() {
        out.confirm = Some(Confirm::Delete(i));
    }
    out.tracks
        .push((r, format!("manage: delete {}", m.accts[i].name)));
}

fn section_name_edit(ui: &mut Ui, m: &mut Manage, k: usize, w: f32, out: &mut Out) {
    let focus = m.focus_section;
    if let Item::Section { id, name } = &mut m.layout[k] {
        let r = ui.add(
            egui::TextEdit::singleline(name)
                .desired_width(w)
                .font(egui::TextStyle::Body)
                .text_color(ui.visuals().strong_text_color()),
        );
        if focus == Some(*id) {
            r.request_focus();
            m.focus_section = None;
        }
        if r.changed() {
            out.events.push((
                "manage_edit",
                json!({ "section": name.clone(), "renamed": true }),
            ));
        }
        if r.gained_focus() {
            m.sel = Some(k);
        }
        out.tracks.push((r, format!("manage: section name {name}")));
    }
}

/// Variant A: one table holds the whole layout, details inline.
fn variant_a(ui: &mut Ui, cx: &Mcx<'_>, m: &mut Manage, out: &mut Out) {
    let h = cx.row_h;
    ui.horizontal(|ui| {
        ui.add_space(W_HANDLE);
        for (t, w) in [
            ("Name", W_NAME + 14.0),
            ("Type", W_TYPE),
            ("Balance", W_BAL),
            ("Hidden", W_FLAG),
            ("Closed", W_FLAG),
        ] {
            fixed(ui, w, h, t == "Balance", |ui| {
                ui.strong(t);
            });
        }
    });
    ui.separator();
    let mut drop = None;
    rows_scroll(ui, |ui| {
        let mut in_section = false;
        for k in 0..m.layout.len() {
            let sel = m.sel == Some(k);
            let item = m.layout[k].clone();
            let rect = table_row(ui, h, k, sel, |ui| {
                handle(ui, cx, m, k, out);
                match &item {
                    Item::Section { .. } => {
                        in_section = true;
                        fixed(ui, W_NAME + 14.0, h, false, |ui| {
                            section_name_edit(ui, m, k, W_NAME, out);
                        });
                        fixed(ui, W_TYPE, h, false, |ui| {
                            ui.label(RichText::new("Section").weak().italics());
                        });
                        let total = cx.section_total(m, k);
                        fixed(ui, W_BAL, h, true, |ui| money_label(ui, total));
                        ui.add_space(W_FLAG * 2.0);
                        let r = ui.small_button("Remove");
                        if r.clicked() {
                            out.remove = Some(k);
                        }
                        out.tracks
                            .push((r, format!("manage: remove section {}", cx.label(m, &item))));
                    }
                    Item::Separator { .. } => {
                        fixed(
                            ui,
                            W_NAME + 14.0 + W_TYPE + W_BAL + W_FLAG * 2.0,
                            h,
                            false,
                            |ui| {
                                if in_section {
                                    ui.add_space(14.0);
                                }
                                separator_line(ui, 260.0);
                                ui.label(RichText::new("separator").weak().italics());
                            },
                        );
                        let r = ui.small_button("Remove");
                        if r.clicked() {
                            out.remove = Some(k);
                        }
                        out.tracks
                            .push((r, format!("manage: remove separator row {k}")));
                    }
                    Item::Account(i) => {
                        let i = *i;
                        fixed(ui, W_NAME + 14.0, h, false, |ui| {
                            if in_section {
                                ui.add_space(14.0);
                            }
                            if m.accts[i].deleted {
                                ui.label(RichText::new(&m.accts[i].name).strikethrough().weak());
                            } else {
                                let r = ui.add(
                                    egui::TextEdit::singleline(&mut m.accts[i].name).desired_width(
                                        W_NAME - if in_section { 14.0 } else { 0.0 },
                                    ),
                                );
                                if r.gained_focus() {
                                    m.sel = Some(k);
                                }
                                if m.fresh && m.sel == Some(k) {
                                    r.request_focus();
                                }
                                if r.changed() {
                                    out.events.push((
                                        "manage_edit",
                                        json!({ "account": cx.accounts[i].name, "renamed": true }),
                                    ));
                                }
                                out.tracks
                                    .push((r, format!("manage: name {}", cx.accounts[i].name)));
                            }
                        });
                        fixed(ui, W_TYPE, h, false, |ui| {
                            ui.label(RichText::new(cx.accounts[i].kind.name()).weak());
                        });
                        fixed(ui, W_BAL, h, true, |ui| {
                            money_label(ui, cx.accounts[i].balance)
                        });
                        if m.accts[i].deleted {
                            ui.add_space(W_FLAG * 2.0);
                            ui.label(RichText::new("will be deleted").small());
                        } else {
                            ui.add_space(14.0);
                            flags(ui, cx, m, i, out, false);
                            ui.add_space(8.0);
                            delete_button(ui, m, i, out);
                        }
                    }
                }
            });
            drop_zone(ui, "manage", rect, k, &mut drop);
        }
    });
    if let Some((from, to)) = drop {
        out.moved = Some((from, to, "drag"));
    }
}

fn rows_scroll(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let h = (ui.available_height() - 44.0).max(120.0);
    egui::ScrollArea::vertical()
        .id_salt("manage_rows")
        .max_height(h)
        .auto_shrink([false, false])
        .show(ui, add);
}

/// The layout as a tree of selectable rows: variant B's left side and variant C's Layout tab.
fn tree(ui: &mut Ui, cx: &Mcx<'_>, m: &mut Manage, out: &mut Out, inline_remove: bool) {
    let h = cx.row_h;
    let mut drop = None;
    let mut in_section = false;
    for k in 0..m.layout.len() {
        let item = m.layout[k].clone();
        let sel = m.sel == Some(k);
        let label = cx.label(m, &item);
        let rect = table_row(ui, h, k, sel, |ui| {
            handle(ui, cx, m, k, out);
            let (text, indent) = match &item {
                Item::Section { .. } => {
                    in_section = true;
                    (RichText::new(&label).strong(), 0.0)
                }
                Item::Separator { .. } => (
                    RichText::new("separator").weak().italics(),
                    if in_section { 14.0 } else { 0.0 },
                ),
                Item::Account(i) => {
                    let e = &m.accts[*i];
                    let mut t = RichText::new(&e.name);
                    if e.deleted {
                        t = t.strikethrough().weak();
                    } else if e.hidden || e.closed {
                        t = t.weak();
                    }
                    (t, if in_section { 14.0 } else { 0.0 })
                }
            };
            ui.add_space(indent);
            let w = ui.available_width() - if inline_remove { 150.0 } else { 0.0 };
            let r = ui.add_sized(
                [w.max(80.0), h],
                egui::Button::selectable(sel, text).frame_when_inactive(false),
            );
            if r.clicked() {
                m.sel = Some(k);
                out.events.push(("manage_select", json!({ "item": label })));
            }
            out.tracks.push((r, format!("manage: tree {label}")));
            if let Item::Account(i) = item {
                let e = &m.accts[i];
                let tag = if e.deleted {
                    "will be deleted"
                } else if e.closed && e.hidden {
                    "closed · hidden"
                } else if e.closed {
                    "closed"
                } else if e.hidden {
                    "hidden"
                } else {
                    ""
                };
                if inline_remove {
                    ui.label(RichText::new(tag).small().weak());
                }
            }
            if inline_remove && !matches!(item, Item::Account(_)) {
                let r = ui.small_button("Remove");
                if r.clicked() {
                    out.remove = Some(k);
                }
                out.tracks.push((r, format!("manage: remove {label}")));
            }
        });
        drop_zone(ui, "manage", rect, k, &mut drop);
    }
    if let Some((from, to)) = drop {
        out.moved = Some((from, to, "drag"));
    }
}

fn section_combo(ui: &mut Ui, m: &Manage, k: usize, w: f32, out: &mut Out) {
    let current = section_of(&m.layout, k);
    let secs = sections(&m.layout);
    let shown = current
        .and_then(|c| secs.iter().find(|(id, _)| *id == c).map(|(_, n)| n.clone()))
        .unwrap_or_else(|| "(no section)".to_owned());
    let Item::Account(i) = m.layout[k] else {
        return;
    };
    let name = m.accts[i].name.clone();
    let mut pick = current;
    let r = egui::ComboBox::from_id_salt(("section_combo", k))
        .selected_text(shown)
        .width(w)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut pick, None, "(no section)");
            for (id, n) in &secs {
                ui.selectable_value(&mut pick, Some(*id), n);
            }
        });
    out.tracks
        .push((r.response, format!("manage: section of {name}")));
    if pick != current {
        out.to_section = Some((k, pick));
    }
}

/// Variant B: tree on the left, details of the selected item on the right.
fn variant_b(ui: &mut Ui, cx: &Mcx<'_>, m: &mut Manage, out: &mut Out) {
    let h = (ui.available_height() - 44.0).max(120.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(320.0);
            egui::ScrollArea::vertical()
                .id_salt("manage_tree")
                .max_height(h)
                .auto_shrink([false, false])
                .show(ui, |ui| tree(ui, cx, m, out, false));
        });
        ui.separator();
        ui.vertical(|ui| {
            let Some(k) = m.sel.filter(|k| *k < m.layout.len()) else {
                ui.label(RichText::new("Select an account, section or separator.").weak());
                return;
            };
            match m.layout[k].clone() {
                Item::Account(i) => {
                    let a = &cx.accounts[i];
                    ui.heading(&m.accts[i].name);
                    ui.label(
                        RichText::new(format!("{} · balance {}", a.kind.name(), fmt_cents(a.balance)))
                            .weak(),
                    );
                    ui.add_space(8.0);
                    if m.accts[i].deleted {
                        ui.label("This account will be deleted when you press OK.");
                        return;
                    }
                    egui::Grid::new("details").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                        ui.label("Name");
                        let r = ui.add(egui::TextEdit::singleline(&mut m.accts[i].name).desired_width(220.0));
                        if m.fresh {
                            r.request_focus();
                        }
                        if r.changed() {
                            out.events.push(("manage_edit", json!({ "account": a.name, "renamed": true })));
                        }
                        out.tracks.push((r, format!("manage: name {}", a.name)));
                        ui.end_row();
                        ui.label("Section");
                        section_combo(ui, m, k, 220.0, out);
                        ui.end_row();
                        ui.label("Show");
                        ui.vertical(|ui| flags(ui, cx, m, i, out, true));
                        ui.end_row();
                    });
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(
                            "Hidden: left out of the Account bar, still in reports and Net Worth.\nClosed: read-only, moves to the Closed section.",
                        )
                        .small()
                        .weak(),
                    );
                    ui.add_space(10.0);
                    delete_button(ui, m, i, out);
                }
                Item::Section { .. } => {
                    ui.label(RichText::new("Section").weak());
                    section_name_edit(ui, m, k, 220.0, out);
                    let b = block(&m.layout, k);
                    let n = m.layout[b].iter().filter(|it| matches!(it, Item::Account(_))).count();
                    ui.label(
                        RichText::new(format!("{n} accounts · total {}", fmt_cents(cx.section_total(m, k))))
                            .weak(),
                    );
                    ui.add_space(10.0);
                    let r = ui.button("Remove section");
                    if r.clicked() {
                        out.remove = Some(k);
                    }
                    out.tracks.push((r, "manage: remove section (details)".to_owned()));
                    ui.label(
                        RichText::new("Its accounts join the section above.").small().weak(),
                    );
                }
                Item::Separator { .. } => {
                    ui.label(RichText::new("Separator").weak());
                    let r = ui.button("Remove separator");
                    if r.clicked() {
                        out.remove = Some(k);
                    }
                    out.tracks.push((r, "manage: remove separator (details)".to_owned()));
                }
            }
        });
    });
}

/// Variant C: an Accounts tab (details table) and a Layout tab.
fn variant_c(ui: &mut Ui, cx: &Mcx<'_>, m: &mut Manage, out: &mut Out) {
    ui.horizontal(|ui| {
        for (page, label) in [(false, "Accounts"), (true, "Layout")] {
            let r = ui.selectable_label(m.layout_page == page, label);
            if r.clicked() {
                m.layout_page = page;
                out.events.push(("manage_page", json!({ "page": label })));
            }
            out.tracks.push((r, format!("manage: page {label}")));
        }
    });
    ui.separator();
    if m.layout_page {
        rows_scroll(ui, |ui| tree(ui, cx, m, out, true));
        return;
    }
    let h = cx.row_h;
    ui.horizontal(|ui| {
        for (t, w) in [
            ("Name", W_NAME),
            ("Type", W_TYPE),
            ("Section", W_SECTION + 8.0),
            ("Balance", W_BAL),
            ("Hidden", W_FLAG),
            ("Closed", W_FLAG),
        ] {
            fixed(ui, w, h, t == "Balance", |ui| {
                ui.strong(t);
            });
        }
    });
    ui.separator();
    rows_scroll(ui, |ui| {
        let order: Vec<(usize, usize)> = m
            .layout
            .iter()
            .enumerate()
            .filter_map(|(k, it)| match it {
                Item::Account(i) => Some((k, *i)),
                _ => None,
            })
            .collect();
        for (row, (k, i)) in order.into_iter().enumerate() {
            table_row(ui, h, row, m.sel == Some(k), |ui| {
                fixed(ui, W_NAME, h, false, |ui| {
                    if m.accts[i].deleted {
                        ui.label(RichText::new(&m.accts[i].name).strikethrough().weak());
                    } else {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut m.accts[i].name)
                                .desired_width(W_NAME - 6.0),
                        );
                        if r.gained_focus() {
                            m.sel = Some(k);
                        }
                        if m.fresh && m.sel == Some(k) {
                            r.request_focus();
                        }
                        if r.changed() {
                            out.events.push((
                                "manage_edit",
                                json!({ "account": cx.accounts[i].name, "renamed": true }),
                            ));
                        }
                        out.tracks
                            .push((r, format!("manage: name {}", cx.accounts[i].name)));
                    }
                });
                fixed(ui, W_TYPE, h, false, |ui| {
                    ui.label(RichText::new(cx.accounts[i].kind.name()).weak());
                });
                fixed(ui, W_SECTION + 8.0, h, false, |ui| {
                    section_combo(ui, m, k, W_SECTION, out)
                });
                fixed(ui, W_BAL, h, true, |ui| {
                    money_label(ui, cx.accounts[i].balance)
                });
                if m.accts[i].deleted {
                    ui.label(RichText::new("will be deleted").small());
                } else {
                    ui.add_space(14.0);
                    flags(ui, cx, m, i, out, false);
                    ui.add_space(8.0);
                    delete_button(ui, m, i, out);
                }
            });
        }
    });
}

impl Proto {
    fn manage_dialog(&mut self, ctx: &Context) {
        let Some(mut m) = self.manage.take() else {
            return;
        };
        let forced = self.forced.take();
        // Keys the Dialog acts on only while no field is being edited.
        let free = !ctx.egui_wants_keyboard_input();
        let esc = ctx.input(|i| i.key_pressed(Key::Escape));
        let enter = free && ctx.input(|i| i.key_pressed(Key::Enter));

        let cx = Mcx {
            accounts: &self.accounts,
            o: self.o,
            row_h: self.theme.row_height(),
        };
        let mut out = Out::default();

        // Backdrop: dims the Main window and swallows its clicks.
        let screen = ctx.content_rect();
        egui::Area::new(Id::new("manage_backdrop"))
            .order(Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, r) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
                ui.painter()
                    .rect_filled(rect, 0.0, Color32::from_black_alpha(110));
                if r.clicked() {
                    out.events
                        .push(("backdrop_click", json!({ "dialog": "manage" })));
                }
            });

        let mut open = true;
        let mut ok = false;
        let mut cancel = false;
        let variant = self.variant;
        let shown = egui::Window::new("Manage Accounts")
            .id(Id::new("manage_window"))
            .order(Order::Foreground)
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(vec2(820.0, 560.0))
            .min_size(vec2(640.0, 380.0))
            .pivot(Align2::CENTER_CENTER)
            .default_pos(screen.center())
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(if cx.o.group_by_type {
                        "Rename, hide, close or delete accounts. The Account bar groups by type now (Tools → Preferences); your sections apply when grouping is off. Nothing is saved until OK."
                    } else {
                        "Rename, hide, close or delete accounts, and arrange the Account bar. Nothing is saved until OK."
                    })
                    .small()
                    .weak(),
                );
                ui.separator();
                match variant {
                    Variant::A => variant_a(ui, &cx, &mut m, &mut out),
                    Variant::B => variant_b(ui, &cx, &mut m, &mut out),
                    Variant::C => variant_c(ui, &cx, &mut m, &mut out),
                }
                ui.separator();
                let layout_controls = variant != Variant::C || m.layout_page;
                let mut left = Vec::new();
                let mut right = Vec::new();
                Sides::new().show(
                    ui,
                    |ui| {
                        if layout_controls {
                            left.push((ui.button("+ Section"), "manage: + Section".to_owned()));
                            left.push((ui.button("+ Separator"), "manage: + Separator".to_owned()));
                        }
                    },
                    |ui| {
                        right.push((ui.button("Cancel"), "manage: Cancel".to_owned()));
                        let fill = ui.visuals().selection.bg_fill;
                        let r = ui.add(Button::new(RichText::new("OK").strong()).fill(fill));
                        right.push((r, "manage: OK".to_owned()));
                    },
                );
                for (r, name) in left.into_iter().chain(right) {
                    match name.as_str() {
                        "manage: + Section" => out.add_section = r.clicked(),
                        "manage: + Separator" => out.add_separator = r.clicked(),
                        "manage: Cancel" => cancel = r.clicked(),
                        _ => ok = r.clicked(),
                    }
                    out.tracks.push((r, name));
                }
            });
        if let Some(r) = &shown {
            self.h.region(r.response.rect, "dialog: manage accounts");
        }
        ctx.move_to_top(egui::LayerId::new(
            Order::Foreground,
            Id::new("manage_backdrop"),
        ));
        ctx.move_to_top(egui::LayerId::new(
            Order::Foreground,
            Id::new("manage_window"),
        ));
        m.fresh = false;

        // The light confirm on top, as an egui::Modal.
        let mut confirm_done = None;
        if let Some(c) = &m.confirm {
            let (title, text, ok_label) = match c {
                Confirm::Delete(i) => (
                    "Delete account?",
                    format!(
                        "Delete “{}” and all of its {} transactions? This can't be undone.",
                        m.accts[*i].name,
                        self.accounts[*i].txns.len()
                    ),
                    Some("Delete"),
                ),
                Confirm::CloseWithBalance(i) => (
                    "Close account with a balance?",
                    format!(
                        "“{}” still has a balance of {}. A closed account is read-only and its balance still counts in Net Worth. Close it anyway?",
                        m.accts[*i].name,
                        fmt_cents(self.accounts[*i].balance)
                    ),
                    Some("Close Account"),
                ),
                Confirm::RefuseClose(i) => (
                    "Can't close this account",
                    format!(
                        "“{}” has a balance of {}. Transfer the balance out to zero first, then close it.",
                        m.accts[*i].name,
                        fmt_cents(self.accounts[*i].balance)
                    ),
                    None,
                ),
            };
            let resp = egui::Modal::new(Id::new("manage_confirm"))
                .backdrop_color(Color32::from_black_alpha(70))
                .show(ctx, |ui| {
                    ui.set_max_width(420.0);
                    ui.heading(title);
                    ui.add_space(6.0);
                    ui.label(text);
                    ui.add_space(12.0);
                    ui.separator();
                    let mut res = None;
                    Sides::new().show(
                        ui,
                        |_| {},
                        |ui| {
                            if let Some(ok_label) = ok_label {
                                let fill = ui.visuals().selection.bg_fill;
                                let r = ui
                                    .add(Button::new(RichText::new(ok_label).strong()).fill(fill));
                                out.tracks.push((r.clone(), format!("confirm: {ok_label}")));
                                let c = ui.button("Cancel");
                                out.tracks.push((c.clone(), "confirm: Cancel".to_owned()));
                                if r.clicked() || ctx.input(|i| i.key_pressed(Key::Enter)) {
                                    res = Some(true);
                                } else if c.clicked() || esc {
                                    res = Some(false);
                                }
                            } else {
                                let r = ui.button("OK");
                                out.tracks.push((r.clone(), "confirm: OK".to_owned()));
                                if r.clicked() || esc || ctx.input(|i| i.key_pressed(Key::Enter)) {
                                    res = Some(false);
                                }
                            }
                        },
                    );
                    res
                });
            confirm_done = resp.inner;
        }

        // Release `cx` before changing the draft structure.
        let Out {
            moved,
            step,
            to_section,
            remove,
            add_section,
            add_separator,
            confirm,
            events,
            tracks,
        } = out;
        for (r, name) in tracks {
            self.h.track(&r, name);
        }
        for (kind, fields) in events {
            self.h.event(kind, fields);
        }
        if let Some(done) = confirm_done {
            let c = m.confirm.take().unwrap();
            let (what, i) = match c {
                Confirm::Delete(i) => {
                    if done {
                        m.accts[i].deleted = true;
                    }
                    ("delete", i)
                }
                Confirm::CloseWithBalance(i) => {
                    if done {
                        m.accts[i].closed = true;
                    }
                    ("close_with_balance", i)
                }
                Confirm::RefuseClose(i) => ("refuse_close", i),
            };
            self.h.event(
                "confirm",
                json!({ "what": what, "account": self.accounts[i].name, "ok": done }),
            );
        } else if m.confirm.is_none() {
            if let Some(c) = confirm {
                m.confirm = Some(c);
            }
        }
        let confirm_open = m.confirm.is_some() || confirm_done.is_some();

        if let Some((from, to, how)) = moved
            && move_item(&mut m.layout, from, to)
        {
            self.h.event(
                "layout_move",
                json!({ "from": from, "to": to, "how": how, "where": "manage" }),
            );
            m.sel = None;
        }
        if let Some((k, up)) = step {
            let item = m.layout[k].clone();
            if step_item(&mut m.layout, k, up) {
                m.sel = m.layout.iter().position(|it| *it == item);
                self.h.event(
                    "layout_move",
                    json!({ "from": k, "to": m.sel, "how": if up { "up" } else { "down" }, "where": "manage" }),
                );
            }
        }
        if let Some((k, sec)) = to_section {
            let item = m.layout[k].clone();
            move_to_section(&mut m.layout, k, sec);
            m.sel = m.layout.iter().position(|it| *it == item);
            self.h.event(
                "layout_move",
                json!({ "from": k, "to": m.sel, "how": "section combo", "where": "manage" }),
            );
        }
        if let Some(k) = remove {
            let it = m.layout.remove(k);
            m.sel = None;
            self.h.event(
                "layout_remove",
                json!({ "what": if it.is_section() { "section" } else { "separator" } }),
            );
        }
        if add_section {
            let id = m.next_id;
            m.next_id += 1;
            m.layout.push(Item::Section {
                id,
                name: "New section".to_owned(),
            });
            m.sel = Some(m.layout.len() - 1);
            m.focus_section = Some(id);
            if variant == Variant::C {
                m.layout_page = true;
            }
            self.h.event("layout_add", json!({ "what": "section" }));
        }
        if add_separator {
            let id = m.next_id;
            m.next_id += 1;
            let at = m.sel.map_or(m.layout.len(), |k| k + 1).min(m.layout.len());
            m.layout.insert(at, Item::Separator { id });
            m.sel = Some(at);
            self.h
                .event("layout_add", json!({ "what": "separator", "at": at }));
        }

        self.manage = Some(m);
        let reason = if !open {
            Some("close_button")
        } else if ok || (forced.is_some() && matches!(forced, Some(Forced::Ok))) {
            Some("ok")
        } else if cancel || matches!(forced, Some(Forced::Cancel)) {
            Some("cancel")
        } else if !confirm_open && esc {
            Some("esc")
        } else if !confirm_open && enter {
            Some("ok")
        } else {
            None
        };
        if let Some(reason) = reason {
            self.close_manage(ctx, reason);
        }
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
                ui.label(RichText::new("Variant (Manage Accounts)").strong());
                for (v, (_, hint)) in Variant::ALL.into_iter().zip(VARIANT_KEYS) {
                    let r = ui.selectable_label(self.variant == v, format!("{hint}   {}", v.label()));
                    self.h.track(&r, format!("dock: variant {}", v.key()));
                    if r.clicked() {
                        self.set_variant(v, "click");
                    }
                }
                ui.separator();
                ui.label(RichText::new("Toggles").strong());
                for (&(key, label, applies), (_, hint)) in TOGGLES.iter().zip(TOGGLE_KEYS) {
                    let mut value = *self.o.get_mut(key).unwrap();
                    let r = ui.checkbox(&mut value, format!("{hint}   [{applies}] {label}"));
                    self.h.track(&r, format!("dock: toggle {key}"));
                    if r.changed() {
                        self.set_toggle(key, Some(value), "click");
                    }
                }
                ui.separator();
                self.h.dock_spotlight(ui);
                self.h.dock_prompt(ui);
                ui.separator();
                let r = ui.button("Open Manage Accounts…");
                self.h.track(&r, "dock: open manage");
                if r.clicked() {
                    self.open_manage(Some(r.id), None, "dock");
                }
                ui.separator();
                ui.label(
                    RichText::new(
                        "A–C or ◀ / ▶ variant · 1–9, 0 toggles · F2 note · F3 marker · F4 hide this dock · F5 spotlight · F6 prompt",
                    )
                    .small()
                    .weak(),
                );
            });
    }
}

fn main() -> eframe::Result {
    // `PROTO_DUMP=1` prints the stub balances and exits, to check them without a window.
    if std::env::var_os("PROTO_DUMP").is_some() {
        let accounts = generate();
        for a in &accounts {
            let min = a.txns.iter().map(|t| t.balance).min().unwrap_or(0);
            println!(
                "{:<20} {:>16} min {:>16} txns {}",
                a.name,
                fmt_cents(a.balance),
                fmt_cents(min),
                a.txns.len()
            );
        }
        let nw: i64 = accounts.iter().map(|a| a.balance).sum();
        println!("net worth {}", fmt_cents(nw));
        return Ok(());
    }
    let args = Args::from_env();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Balanced — PROTOTYPE Manage Accounts and Account bar sections")
            .with_inner_size([1400.0, 860.0]),
        ..Default::default()
    };
    eframe::run_native(
        "balanced_manage_accounts_prototype",
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
