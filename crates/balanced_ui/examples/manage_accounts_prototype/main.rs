//! PROTOTYPE, throwaway: Manage Accounts and the Account bar.
//!
//! Question (ticket #76): how does the Manage Accounts dialog edit, hide and close accounts, and
//! how are accounts arranged in the Account bar?
//!
//! Decided in the session: no user-named sections or separators in v1. The Account bar uses
//! Quicken's fixed groups (Banking, Investing, Property & Debt), each split into fixed
//! sub-groups by account kind, and accounts reorder only within their sub-group.
//!
//! Two variants differ in how Manage Accounts is structured:
//!   A  Account List table: one table, grouped like the Account bar, details inline on each row
//!   B  List + details: the grouped list on the left, the selected account's details on the right
//!
//! Run:
//!   cargo run -p balanced_ui --example manage_accounts_prototype -- \
//!     [--variant a|b] [--log <dir>/events.jsonl] [--control <dir>/control.txt]
//!
//! The PROTOTYPE dock on the right holds the variant switcher and toggles; F4 collapses it.
//! A / B or ← / → switch variant, 1–9 flip toggles, F2 note, F3 marker, F5 spotlight, F6 prompt.
//!
//! Control-file verbs (besides the harness's `prompt` and `spotlight`): `variant <key>`,
//! `toggle <key> [on|off]`, `screenshot [label]`, `dock on|off`, `reload`, `quit`,
//! `account <index|name>`, `tab <index|name>`, `dialog manage`, `ok`, `cancel`,
//! `select <account>` (in the open Manage Accounts), `fold <group|closed>`,
//! `unfold <group|closed>`.
//!
//! The Main window follows the decided shell layout (#15) and the real theme module (#16).
//! Manage Accounts is a heavy Dialog (#19): title bar, movable, resizable, app-modal, dimming,
//! and never bigger than its window.
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
    clippy::type_complexity,
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

/// Quicken's fixed Account bar groups. Each has fixed sub-groups, one per account kind, which
/// the Account bar marks off with a thin line.
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

    /// The sub-group's heading, where one is shown.
    fn plural(self) -> &'static str {
        match self {
            Self::Checking => "Checking",
            Self::Savings => "Savings",
            Self::Credit => "Credit cards",
            Self::Investment => "Investments",
            Self::Retirement => "Retirement",
            Self::Asset => "Assets",
            Self::Loan => "Liabilities",
        }
    }

    fn group(self) -> Group {
        match self {
            Self::Checking | Self::Savings | Self::Credit => Group::Banking,
            Self::Investment | Self::Retirement => Group::Investing,
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
    ("401(k)", Kind::Retirement, 900, 3_000_000),
    ("Roth IRA", Kind::Retirement, 600, 800_000),
    ("Traditional IRA", Kind::Retirement, 500, 1_200_000),
    ("House", Kind::Asset, 500, 35_000_000),
    ("Car", Kind::Asset, 500, 2_800_000),
    ("Mortgage", Kind::Loan, 500, -28_000_000),
    ("Car Loan", Kind::Loan, 500, -2_400_000),
    ("Student Loan", Kind::Loan, 500, -3_500_000),
    ("Personal Loan", Kind::Loan, 500, -600_000),
    ("Boat Loan", Kind::Loan, 300, -1_500_000),
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
                    Kind::Investment | Kind::Retirement if roll < 40 => {
                        ("Contribution", "[Everyday Checking]", rng.around(50_000))
                    }
                    Kind::Investment | Kind::Retirement if roll < 75 => {
                        ("Market Gain", "Investments:Unrealized", rng.around(60_000))
                    }
                    Kind::Investment | Kind::Retirement => {
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
// Account order: one list of account ids. Within a sub-group (one kind) accounts show in this
// order; nothing moves across sub-groups.

/// The accounts of `kind` in `order`, filtered by `keep`.
fn members(
    order: &[usize],
    accounts: &[Account],
    kind: Kind,
    keep: impl Fn(usize) -> bool,
) -> Vec<usize> {
    order
        .iter()
        .copied()
        .filter(|&i| accounts[i].kind == kind && keep(i))
        .collect()
}

/// Move account `id` so it sits just before (or after) account `target`.
fn move_in_order(order: &mut Vec<usize>, id: usize, target: usize, before: bool) -> bool {
    if id == target {
        return false;
    }
    let from = order.iter().position(|&i| i == id).unwrap();
    order.remove(from);
    let at = order.iter().position(|&i| i == target).unwrap();
    order.insert(if before { at } else { at + 1 }, id);
    true
}

/// Swap account `id` with its neighbour of the same kind among `visible`, as ↑ / ↓ do.
fn step_in_order(order: &mut [usize], visible: &[usize], id: usize, up: bool) -> bool {
    let Some(p) = visible.iter().position(|&i| i == id) else {
        return false;
    };
    let other = if up {
        p.checked_sub(1).map(|q| visible[q])
    } else {
        visible.get(p + 1).copied()
    };
    let Some(other) = other else { return false };
    let a = order.iter().position(|&i| i == id).unwrap();
    let b = order.iter().position(|&i| i == other).unwrap();
    order.swap(a, b);
    true
}

/// Dragging an account, in the Account bar or in Manage Accounts.
#[derive(Clone)]
struct Drag {
    id: usize,
    kind: Kind,
    label: String,
    place: &'static str,
}

/// A drop: (dragged account, target account, before the target).
type Drop = Option<(usize, usize, bool)>;

/// Call after drawing account `target`'s row covering `rect`: while an account of the same kind
/// from `place` is dragged over it, mark where it would land; on release, report the drop. A drag
/// over another sub-group shows nothing, because accounts move only within their sub-group.
fn drop_zone(ui: &Ui, place: &str, rect: Rect, target: usize, kind: Kind, out: &mut Drop) {
    let Some(p) = DragAndDrop::payload::<Drag>(ui.ctx()) else {
        return;
    };
    if p.place != place || p.kind != kind {
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
        *out = Some((p.id, target, before));
    }
}

fn start_drag(r: &egui::Response, id: usize, kind: Kind, label: &str, place: &'static str) {
    if r.drag_started() {
        DragAndDrop::set_payload(
            &r.ctx,
            Drag {
                id,
                kind,
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
            Self::A => "A · Account List table",
            Self::B => "B · List + details",
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
    group_totals: bool,
    fold: bool,
    subgroup_lines: bool,
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
            "group_totals" => &mut self.group_totals,
            "fold" => &mut self.fold,
            "subgroup_lines" => &mut self.subgroup_lines,
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
        "group_totals",
        "Group totals in the Account bar (a Preference, default off)",
        "all",
    ),
    ("fold", "Groups fold (arrow on the header)", "all"),
    (
        "subgroup_lines",
        "Sub-groups split by a line (off: small sub-group headings)",
        "all",
    ),
    (
        "closed_section",
        "Closed accounts in a folded Closed group (off: gone from the bar)",
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
        "Drag in Manage Accounts (off: up / down buttons)",
        "all",
    ),
    (
        "caps",
        "Group header in small caps (off: bold over a rule)",
        "all",
    ),
    (
        "refuse_close",
        "Refuse to close an account with a balance (off: warn, allow)",
        "all",
    ),
];

const TOGGLE_KEYS: [(Key, &str); 9] = [
    (Key::Num1, "1"),
    (Key::Num2, "2"),
    (Key::Num3, "3"),
    (Key::Num4, "4"),
    (Key::Num5, "5"),
    (Key::Num6, "6"),
    (Key::Num7, "7"),
    (Key::Num8, "8"),
    (Key::Num9, "9"),
];

// ---------------------------------------------------------------------------------------------
// Manage Accounts: a draft of the accounts' editable fields and their order

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
    order: Vec<usize>,
    /// Selected account id.
    sel: Option<usize>,
    confirm: Option<Confirm>,
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
    order: Vec<usize>,
    selected: usize,
    selected_row: Option<usize>,
    variant: Variant,
    o: Opts,
    manage: Option<Manage>,
    forced: Option<Forced>,
    /// Folded groups by name, plus "Closed".
    folded: BTreeSet<&'static str>,
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
        let order = (0..accounts.len()).collect();
        Self {
            h,
            accounts,
            order,
            selected: 0,
            selected_row: None,
            variant,
            o: Opts {
                group_totals: false,
                fold: true,
                subgroup_lines: true,
                closed_section: true,
                hidden_in_nw: true,
                bar_rearrange: false,
                dnd: true,
                caps: true,
                refuse_close: false,
            },
            manage: None,
            forced: None,
            folded: BTreeSet::from(["Closed"]),
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
        self.manage = Some(Manage {
            accts,
            order: self.order.clone(),
            sel: select,
            confirm: None,
            fresh: true,
            opener,
        });
        self.h.event(
            "dialog_open",
            json!({ "dialog": "manage", "via": via, "variant": self.variant.key() }),
        );
    }

    /// OK in Manage Accounts: the draft replaces the accounts' fields and their order.
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
        if renamed + hidden + closed + deleted > 0 {
            self.emit(
                &format!(
                    "DomainCommand::UpdateAccounts {{ renamed: {renamed}, hidden_changed: {hidden}, closed_changed: {closed}, deleted: {deleted} }}"
                ),
                "Manage Accounts OK",
            );
        }
        if m.order != self.order {
            self.order = m.order;
            self.emit("DomainCommand::ReorderAccounts", "Manage Accounts OK");
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

    /// A new stub savings account from "+ Add Account": it lands at the end of its sub-group.
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
        self.order.push(idx);
        self.selected = idx;
        self.emit(
            &format!("DomainCommand::CreateAccount({name})"),
            "account bar button",
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
            "select" => match (self.account_index(rest), self.manage.is_some()) {
                (Some(i), true) => self.manage.as_mut().unwrap().sel = Some(i),
                _ => bad(self),
            },
            "fold" | "unfold" => {
                let needle = rest.to_lowercase();
                let name = Group::ALL
                    .iter()
                    .map(|g| g.name())
                    .chain(["Closed"])
                    .find(|n| n.to_lowercase().contains(&needle));
                match name {
                    Some(n) if verb == "fold" => {
                        self.folded.insert(n);
                    }
                    Some(n) => {
                        self.folded.remove(n);
                    }
                    None => bad(self),
                }
            }
            "reload" => {
                self.accounts = generate();
                self.order = (0..self.accounts.len()).collect();
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

    /// A group header: fold arrow, name, and the group total when that Preference is on.
    fn group_header(
        &mut self,
        ui: &mut Ui,
        name: &'static str,
        total: i64,
        count: usize,
        foldable: bool,
    ) -> egui::Response {
        let o = self.o;
        let folded = foldable && self.folded.contains(name);
        let h = self.theme.row_height() + 4.0;
        let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
        let v = ui.visuals().clone();
        if r.hovered() && foldable {
            ui.painter()
                .rect_filled(rect, 2.0, v.widgets.hovered.weak_bg_fill);
        }
        let mut x = rect.left() + 4.0;
        if foldable {
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
        if o.group_totals {
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
        self.h
            .track(&r, format!("account bar: group header {name}"));
        if r.clicked() && foldable {
            let now = if self.folded.remove(name) {
                false
            } else {
                self.folded.insert(name);
                true
            };
            self.h
                .event("group_fold", json!({ "group": name, "folded": now }));
        }
        r
    }

    /// Between two sub-groups: a thin line, or with toggle 3 off a small heading.
    fn subgroup_mark(&self, ui: &mut Ui, kind: Kind, first: bool) {
        if self.o.subgroup_lines {
            if first {
                return;
            }
            ui.add_space(3.0);
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().hline(
                rect.x_range().shrink(14.0),
                rect.center().y,
                Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
            );
            ui.add_space(3.0);
        } else {
            ui.add_space(if first { 0.0 } else { 4.0 });
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(
                    RichText::new(kind.plural())
                        .size(11.0)
                        .italics()
                        .color(ui.visuals().weak_text_color()),
                );
            });
        }
    }

    fn visible_in_bar(&self, i: usize) -> bool {
        let a = &self.accounts[i];
        !a.deleted && !a.hidden && !a.closed
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
        let mut dropped: Drop = None;
        let out = egui::ScrollArea::vertical().show(ui, |ui| {
            for group in Group::ALL {
                let subs: Vec<(Kind, Vec<usize>)> = group
                    .kinds()
                    .iter()
                    .map(|&k| {
                        (
                            k,
                            members(&self.order, &self.accounts, k, |i| self.visible_in_bar(i)),
                        )
                    })
                    .filter(|(_, ids)| !ids.is_empty())
                    .collect();
                let count: usize = subs.iter().map(|(_, ids)| ids.len()).sum();
                if count == 0 {
                    continue;
                }
                let total = subs
                    .iter()
                    .flat_map(|(_, ids)| ids)
                    .map(|&i| self.accounts[i].balance)
                    .sum();
                ui.add_space(2.0);
                self.group_header(ui, group.name(), total, count, self.o.fold);
                if self.o.fold && self.folded.contains(group.name()) {
                    continue;
                }
                for (n, (kind, ids)) in subs.into_iter().enumerate() {
                    self.subgroup_mark(ui, kind, n == 0);
                    for i in ids {
                        let r = self.bar_account(ui, i, &mut pick);
                        drop_zone(ui, "bar", r.rect, i, kind, &mut dropped);
                    }
                }
            }
            self.closed_group(ui, &mut pick);
        });
        self.h
            .scroll("account bar", out.state.offset.y, self.theme.row_height());
        if add {
            self.add_account();
        }
        if let Some((id, target, before)) = dropped
            && move_in_order(&mut self.order, id, target, before)
        {
            self.h.event(
                "reorder",
                json!({ "account": self.accounts[id].name, "target": self.accounts[target].name, "before": before, "how": "drag", "where": "account bar" }),
            );
            self.emit("DomainCommand::ReorderAccounts", "account bar drag");
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
        pick: &mut Option<(usize, &'static str)>,
    ) -> egui::Response {
        let drag = self.o.bar_rearrange && !self.accounts[i].closed;
        let a = &self.accounts[i];
        let name = a.name.clone();
        let kind = a.kind;
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
        if drag {
            start_drag(&r, i, kind, &name, "bar");
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

    fn closed_group(&mut self, ui: &mut Ui, pick: &mut Option<(usize, &'static str)>) {
        if !self.o.closed_section {
            return;
        }
        let closed: Vec<usize> = self
            .order
            .iter()
            .copied()
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
        // The Closed group always folds, whatever toggle 2 says.
        self.group_header(ui, "Closed", total, closed.len(), true);
        if !self.folded.contains("Closed") {
            for i in closed {
                self.bar_account(ui, i, pick);
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

/// What a Manage Accounts frame asks for once the draft is released.
#[derive(Default)]
struct Out {
    dropped: Drop,
    step: Option<(usize, bool)>,
    confirm: Option<Confirm>,
    events: Vec<(&'static str, serde_json::Value)>,
    tracks: Vec<(egui::Response, String)>,
}

const W_HANDLE: f32 = 46.0;
const W_NAME: f32 = 230.0;
const W_TYPE: f32 = 86.0;
const W_BAL: f32 = 112.0;
const W_FLAG: f32 = 58.0;

struct Mcx<'a> {
    accounts: &'a [Account],
    o: Opts,
    row_h: f32,
    /// The tallest the Dialog's list may be, so the Dialog fits inside its window.
    list_h: f32,
}

/// Every account the Dialog lists, grouped like the Account bar: deleted-in-draft accounts stay
/// (struck through) until OK; hidden and closed ones stay in their sub-group.
fn grouped(cx: &Mcx<'_>, m: &Manage) -> Vec<(Group, Vec<(Kind, Vec<usize>)>)> {
    Group::ALL
        .iter()
        .map(|&g| {
            let subs = g
                .kinds()
                .iter()
                .map(|&k| {
                    (
                        k,
                        members(&m.order, cx.accounts, k, |i| !cx.accounts[i].deleted),
                    )
                })
                .filter(|(_, ids)| !ids.is_empty())
                .collect();
            (g, subs)
        })
        .collect()
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

fn group_row(ui: &mut Ui, name: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(name).strong().size(14.0));
}

fn subgroup_row(ui: &mut Ui, kind: Kind) {
    ui.horizontal(|ui| {
        ui.add_space(W_HANDLE);
        ui.label(
            RichText::new(kind.plural())
                .italics()
                .color(ui.visuals().weak_text_color()),
        );
    });
}

/// The grip or the ↑ / ↓ buttons that move account `i` within its sub-group.
fn handle(ui: &mut Ui, cx: &Mcx<'_>, m: &Manage, i: usize, out: &mut Out) {
    let label = m.accts[i].name.clone();
    let kind = cx.accounts[i].kind;
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
                start_drag(&r, i, kind, &label, "manage");
                out.tracks.push((r, format!("manage: grip {label}")));
            } else {
                let up = arrow_button(ui, Arrow::Up);
                if up.clicked() {
                    out.step = Some((i, true));
                }
                out.tracks.push((up, format!("manage: up {label}")));
                let down = arrow_button(ui, Arrow::Down);
                if down.clicked() {
                    out.step = Some((i, false));
                }
                out.tracks.push((down, format!("manage: down {label}")));
            }
        },
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

fn rows_scroll(ui: &mut Ui, cx: &Mcx<'_>, add: impl FnOnce(&mut Ui)) {
    egui::ScrollArea::vertical()
        .id_salt("manage_rows")
        .max_height(cx.list_h)
        .auto_shrink([false, false])
        .show(ui, add);
}

/// Variant A: one table grouped like the Account bar, details inline on each row.
fn variant_a(ui: &mut Ui, cx: &Mcx<'_>, m: &mut Manage, out: &mut Out) {
    let h = cx.row_h;
    ui.horizontal(|ui| {
        ui.add_space(W_HANDLE);
        for (t, w) in [
            ("Name", W_NAME + 8.0),
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
    let groups = grouped(cx, m);
    let mut dropped = None;
    rows_scroll(ui, cx, |ui| {
        let mut k = 0;
        for (g, subs) in groups {
            if subs.is_empty() {
                continue;
            }
            group_row(ui, g.name());
            for (kind, ids) in subs {
                subgroup_row(ui, kind);
                for i in ids {
                    k += 1;
                    let sel = m.sel == Some(i);
                    let rect = table_row(ui, h, k, sel, |ui| {
                        handle(ui, cx, m, i, out);
                        fixed(ui, W_NAME + 8.0, h, false, |ui| {
                            if m.accts[i].deleted {
                                ui.label(RichText::new(&m.accts[i].name).strikethrough().weak());
                            } else {
                                let r = ui.add(
                                    egui::TextEdit::singleline(&mut m.accts[i].name)
                                        .desired_width(W_NAME),
                                );
                                if r.gained_focus() {
                                    m.sel = Some(i);
                                }
                                if m.fresh && sel {
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
                    });
                    drop_zone(ui, "manage", rect, i, kind, &mut dropped);
                }
            }
        }
    });
    if dropped.is_some() {
        out.dropped = dropped;
    }
}

/// Variant B: the grouped list on the left, the selected account's details on the right.
fn variant_b(ui: &mut Ui, cx: &Mcx<'_>, m: &mut Manage, out: &mut Out) {
    let h = cx.row_h;
    let groups = grouped(cx, m);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(340.0);
            let mut dropped = None;
            egui::ScrollArea::vertical()
                .id_salt("manage_list")
                .max_height(cx.list_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let mut k = 0;
                    for (g, subs) in groups {
                        if subs.is_empty() {
                            continue;
                        }
                        group_row(ui, g.name());
                        for (kind, ids) in subs {
                            subgroup_row(ui, kind);
                            for i in ids {
                                k += 1;
                                let sel = m.sel == Some(i);
                                let rect = table_row(ui, h, k, sel, |ui| {
                                    handle(ui, cx, m, i, out);
                                    let e = &m.accts[i];
                                    let mut t = RichText::new(&e.name);
                                    if e.deleted {
                                        t = t.strikethrough().weak();
                                    } else if e.hidden || e.closed {
                                        t = t.weak();
                                    }
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
                                    let w = ui.available_width() - 90.0;
                                    let (rect, r) =
                                        ui.allocate_exact_size(vec2(w.max(80.0), h), Sense::click());
                                    if r.hovered() && !sel {
                                        ui.painter().rect_filled(
                                            rect,
                                            2.0,
                                            ui.visuals().widgets.hovered.weak_bg_fill,
                                        );
                                    }
                                    let galley = egui::WidgetText::from(t).into_galley(
                                        ui,
                                        Some(egui::TextWrapMode::Truncate),
                                        rect.width() - 8.0,
                                        egui::TextStyle::Body,
                                    );
                                    let pos = egui::pos2(
                                        rect.left() + 4.0,
                                        rect.center().y - galley.size().y / 2.0,
                                    );
                                    ui.painter().galley(pos, galley, ui.visuals().text_color());
                                    if r.clicked() {
                                        m.sel = Some(i);
                                        out.events.push((
                                            "manage_select",
                                            json!({ "account": cx.accounts[i].name }),
                                        ));
                                    }
                                    out.tracks
                                        .push((r, format!("manage: list {}", cx.accounts[i].name)));
                                    ui.label(RichText::new(tag).small().weak());
                                });
                                drop_zone(ui, "manage", rect, i, kind, &mut dropped);
                            }
                        }
                    }
                });
            if dropped.is_some() {
                out.dropped = dropped;
            }
        });
        ui.separator();
        ui.vertical(|ui| {
            let Some(i) = m.sel else {
                ui.label(RichText::new("Select an account.").weak());
                return;
            };
            let a = &cx.accounts[i];
            ui.heading(&m.accts[i].name);
            ui.label(
                RichText::new(format!(
                    "{} · {} · balance {}",
                    a.kind.group().name(),
                    a.kind.name(),
                    fmt_cents(a.balance)
                ))
                .weak(),
            );
            ui.add_space(8.0);
            if m.accts[i].deleted {
                ui.label("This account will be deleted when you press OK.");
                return;
            }
            egui::Grid::new("details")
                .num_columns(2)
                .spacing([10.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Name");
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut m.accts[i].name).desired_width(220.0),
                    );
                    if m.fresh {
                        r.request_focus();
                    }
                    if r.changed() {
                        out.events
                            .push(("manage_edit", json!({ "account": a.name, "renamed": true })));
                    }
                    out.tracks.push((r, format!("manage: name {}", a.name)));
                    ui.end_row();
                    ui.label("Show");
                    ui.vertical(|ui| flags(ui, cx, m, i, out, true));
                    ui.end_row();
                });
            ui.add_space(10.0);
            ui.label(
                RichText::new(
                    "Hidden: left out of the Account bar, still in reports and Net Worth.\nClosed: read-only, moves to the Closed group.",
                )
                .small()
                .weak(),
            );
            ui.add_space(10.0);
            delete_button(ui, m, i, out);
        });
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
        let screen = ctx.content_rect();

        let cx = Mcx {
            accounts: &self.accounts,
            o: self.o,
            row_h: self.theme.row_height(),
            // The window minus the Dialog's title, description and footer, and a margin.
            list_h: (screen.height() - 260.0).clamp(160.0, 470.0),
        };
        let mut out = Out::default();

        // Backdrop: dims the Main window and swallows its clicks.
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
            .min_size(vec2(640.0, 300.0))
            // A Dialog never outgrows the window it opens in.
            .max_size(screen.size() - vec2(48.0, 48.0))
            .constrain(true)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(screen.center())
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Rename, hide, close, delete or reorder accounts. Accounts move only within their sub-group. Nothing is saved until OK.",
                    )
                    .small()
                    .weak(),
                );
                ui.separator();
                match variant {
                    Variant::A => variant_a(ui, &cx, &mut m, &mut out),
                    Variant::B => variant_b(ui, &cx, &mut m, &mut out),
                }
                ui.separator();
                let mut buttons = Vec::new();
                Sides::new().show(
                    ui,
                    |_| {},
                    |ui| {
                        buttons.push((ui.button("Cancel"), "manage: Cancel"));
                        let fill = ui.visuals().selection.bg_fill;
                        let r = ui.add(Button::new(RichText::new("OK").strong()).fill(fill));
                        buttons.push((r, "manage: OK"));
                    },
                );
                for (r, name) in buttons {
                    if name == "manage: OK" {
                        ok = r.clicked();
                    } else {
                        cancel = r.clicked();
                    }
                    out.tracks.push((r, name.to_owned()));
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
                    let mut buttons = Vec::new();
                    Sides::new().show(
                        ui,
                        |_| {},
                        |ui| {
                            if let Some(ok_label) = ok_label {
                                let fill = ui.visuals().selection.bg_fill;
                                let r = ui
                                    .add(Button::new(RichText::new(ok_label).strong()).fill(fill));
                                buttons.push((r, format!("confirm: {ok_label}"), true));
                                buttons.push((
                                    ui.button("Cancel"),
                                    "confirm: Cancel".to_owned(),
                                    false,
                                ));
                            } else {
                                buttons.push((ui.button("OK"), "confirm: OK".to_owned(), false));
                            }
                        },
                    );
                    for (r, name, yes) in buttons {
                        if r.clicked() {
                            res = Some(yes);
                        }
                        out.tracks.push((r, name));
                    }
                    if res.is_none() {
                        if ok_label.is_some() && ctx.input(|i| i.key_pressed(Key::Enter)) {
                            res = Some(true);
                        } else if esc
                            || (ok_label.is_none() && ctx.input(|i| i.key_pressed(Key::Enter)))
                        {
                            res = Some(false);
                        }
                    }
                    res
                });
            confirm_done = resp.inner;
            // The heavy Dialog is raised every frame, so raise the confirm above it.
            ctx.move_to_top(egui::LayerId::new(
                Order::Foreground,
                Id::new("manage_confirm"),
            ));
        }

        let Out {
            dropped,
            step,
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
        } else if m.confirm.is_none()
            && let Some(c) = confirm
        {
            m.confirm = Some(c);
        }
        let confirm_open = m.confirm.is_some() || confirm_done.is_some();

        if let Some((id, target, before)) = dropped
            && move_in_order(&mut m.order, id, target, before)
        {
            self.h.event(
                "reorder",
                json!({ "account": self.accounts[id].name, "target": self.accounts[target].name, "before": before, "how": "drag", "where": "manage" }),
            );
            m.sel = Some(id);
        }
        if let Some((id, up)) = step {
            let kind = self.accounts[id].kind;
            let visible = members(&m.order, &self.accounts, kind, |i| {
                !self.accounts[i].deleted
            });
            if step_in_order(&mut m.order, &visible, id, up) {
                self.h.event(
                    "reorder",
                    json!({ "account": self.accounts[id].name, "how": if up { "up" } else { "down" }, "where": "manage" }),
                );
                m.sel = Some(id);
            }
        }

        self.manage = Some(m);
        let reason = if !open {
            Some("close_button")
        } else if ok || matches!(forced, Some(Forced::Ok)) {
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
                    let r =
                        ui.selectable_label(self.variant == v, format!("{hint}   {}", v.label()));
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
                        "A / B or left / right variant · 1–9 toggles · F2 note · F3 marker · F4 hide this dock · F5 spotlight · F6 prompt",
                    )
                    .small()
                    .weak(),
                );
            });
    }
}

fn main() -> eframe::Result {
    let args = Args::from_env();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Balanced — PROTOTYPE Manage Accounts and the Account bar")
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
