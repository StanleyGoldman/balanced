//! PROTOTYPE, throwaway: Dialogs and Detached windows.
//!
//! Question (ticket #19): how should Dialogs (a modal confirm, a multi-step New Account wizard,
//! a Manage Accounts list) and Detached windows (a Register, a Reconcile) look and behave, and
//! are real OS windows (egui multi-viewport) reliable enough to keep?
//!
//! Five variants pair a Dialog style with a Detached-window mechanism:
//!   A  egui::Modal in-window        + Detached = deferred viewport (real OS window)
//!   B  egui::Modal in-window        + Detached = immediate viewport (real OS window)
//!   C  Dialog as its own OS window  + Detached = deferred viewport (fake modality: a scrim)
//!   D  non-modal egui::Window       + Detached = embedded egui::Window (in-canvas fallback)
//!   E  egui::Modal in-window        + Detached = a tab in the Main view (fallback)
//!
//! Run:
//!   cargo run -p balanced_ui --example dialogs_detached_prototype -- \
//!     [--variant a|b|c|d|e] [--log <dir>/events.jsonl] [--control <dir>/control.txt]
//!
//! The PROTOTYPE dock on the right holds the variant switcher, toggles and a live list of
//! windows with frames per second; F4 collapses it. ← / → switch variant, F2 writes a note into
//! the log, F3 drops a marker.
//!
//! Control-file verbs (besides the harness's `variant`, `toggle`, `screenshot`, `dock`, `reload`,
//! `quit`): `account <index|name>`, `dialog confirm|wizard|manage`, `ok`, `cancel`,
//! `step next|back` (top Dialog of the Main window), `detach register [account]`,
//! `detach reconcile [account]`, `close detached`, `focus main`.
//!
//! Fixed-seed stub data, English literals, default egui theme. None of this is production code.

#![allow(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::pedantic,
    clippy::too_many_lines,
    clippy::wildcard_enum_match_arm,
    clippy::unwrap_used,
    clippy::str_to_string,
    clippy::let_underscore_must_use,
    clippy::iter_over_hash_type,
    clippy::needless_pass_by_ref_mut,
    clippy::use_self,
    dead_code,
    clippy::needless_range_loop,
    clippy::rc_buffer,
    missing_debug_implementations,
    unreachable_pub
)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use balanced_prototype_harness::{Args, Harness};
use eframe::egui;
use egui::{
    Align, Button, CentralPanel, Context, Id, Key, Layout, Panel, Pos2, Rect, RichText, Sense,
    Sides, Ui, Vec2, ViewportBuilder, ViewportCommand, ViewportId, vec2,
};
use egui_extras::{Column, TableBuilder};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------------------------
// Stub data: fixed seed, realistic volume

const SEED: u64 = 0x00ba_1a9c_ed00;

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

#[derive(Clone, Copy)]
enum Kind {
    Checking,
    Savings,
    Credit,
    Investment,
    Asset,
    Loan,
}

/// (name, group, kind, transactions, opening balance in cents)
const ACCOUNT_SPECS: &[(&str, Group, Kind, usize, i64)] = &[
    (
        "Everyday Checking",
        Group::Banking,
        Kind::Checking,
        5_000,
        250_000,
    ),
    (
        "Joint Checking",
        Group::Banking,
        Kind::Checking,
        3_200,
        400_000,
    ),
    (
        "Bills Checking",
        Group::Banking,
        Kind::Checking,
        1_800,
        150_000,
    ),
    ("Cash", Group::Banking, Kind::Checking, 600, 20_000),
    (
        "High-Yield Savings",
        Group::Banking,
        Kind::Savings,
        900,
        1_000_000,
    ),
    (
        "Emergency Fund",
        Group::Banking,
        Kind::Savings,
        500,
        1_500_000,
    ),
    (
        "Holiday Savings",
        Group::Banking,
        Kind::Savings,
        500,
        50_000,
    ),
    ("Kids' Savings", Group::Banking, Kind::Savings, 500, 30_000),
    ("Visa Rewards", Group::Credit, Kind::Credit, 2_500, 0),
    ("Mastercard", Group::Credit, Kind::Credit, 1_400, 0),
    ("Store Card", Group::Credit, Kind::Credit, 500, 0),
    ("Travel Card", Group::Credit, Kind::Credit, 800, 0),
    (
        "Brokerage",
        Group::Investing,
        Kind::Investment,
        1_200,
        2_000_000,
    ),
    ("401(k)", Group::Investing, Kind::Investment, 900, 3_000_000),
    ("Roth IRA", Group::Investing, Kind::Investment, 600, 800_000),
    (
        "Traditional IRA",
        Group::Investing,
        Kind::Investment,
        500,
        1_200_000,
    ),
    ("HSA", Group::Investing, Kind::Investment, 500, 100_000),
    (
        "529 College Fund",
        Group::Investing,
        Kind::Investment,
        500,
        200_000,
    ),
    ("House", Group::PropertyDebt, Kind::Asset, 500, 35_000_000),
    ("Car", Group::PropertyDebt, Kind::Asset, 500, 2_800_000),
    (
        "Mortgage",
        Group::PropertyDebt,
        Kind::Loan,
        500,
        -28_000_000,
    ),
    ("Car Loan", Group::PropertyDebt, Kind::Loan, 500, -2_400_000),
    (
        "Student Loan",
        Group::PropertyDebt,
        Kind::Loan,
        500,
        -3_500_000,
    ),
    (
        "Personal Loan",
        Group::PropertyDebt,
        Kind::Loan,
        500,
        -600_000,
    ),
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
    num: Option<u32>,
    payee: &'static str,
    category: &'static str,
    amount: i64,
    balance: i64,
    cleared: bool,
}

struct Account {
    name: &'static str,
    group: Group,
    txns: Arc<Vec<Txn>>,
    hidden: bool,
    closed: bool,
    balance: i64,
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
        .map(|&(name, group, kind, n, opening)| {
            let mut balance = opening;
            let mut check = 1000;
            let mut txns = Vec::with_capacity(n + 1);
            txns.push(Txn {
                day: FIRST_DAY,
                num: None,
                payee: "Opening Balance",
                category: "[Equity]",
                amount: opening,
                balance,
                cleared: true,
            });
            for i in 0..n {
                let day = FIRST_DAY + 1 + (i as i64 * HISTORY_DAYS) / n as i64;
                let roll = rng.below(100);
                let (payee, category, amount) = match kind {
                    Kind::Checking if roll < 5 => {
                        ("Paycheck", "Income:Salary", rng.around(280_000))
                    }
                    Kind::Checking if roll < 8 => {
                        ("Landlord", "Housing:Rent", -rng.around(150_000))
                    }
                    Kind::Checking if roll < 11 => (
                        "Transfer to Visa Rewards",
                        "[Visa Rewards]",
                        -rng.around(60_000),
                    ),
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
                    Kind::Credit if roll < 8 => (
                        "Payment — Thank You",
                        "[Everyday Checking]",
                        rng.around(80_000),
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
                    Kind::Asset if roll < 60 => {
                        ("Revaluation", "Property:Value Change", rng.around(20_000))
                    }
                    Kind::Asset => ("Depreciation", "Property:Value Change", -rng.around(15_000)),
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
                balance += amount;
                let num = if matches!(kind, Kind::Checking) && amount < 0 && rng.below(6) == 0 {
                    check += 1;
                    Some(check)
                } else {
                    None
                };
                txns.push(Txn {
                    day,
                    num,
                    payee,
                    category,
                    amount,
                    balance,
                    cleared: day < FIRST_DAY + HISTORY_DAYS - 30,
                });
            }
            Account {
                name,
                group,
                txns: Arc::new(txns),
                hidden: false,
                closed: false,
                balance,
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
    format!("{y}-{m:02}-{d:02}")
}

// ---------------------------------------------------------------------------------------------
// Plumbing shared by every window: options, event sink, per-window frame statistics

#[derive(Clone, Copy, PartialEq, Eq)]
enum DlgStyle {
    /// `egui::Modal`: an overlay inside the window that opened it, with a backdrop.
    Modal,
    /// A real OS window per Dialog, plus a scrim over the opener for fake modality.
    OsWindow,
    /// A non-modal, movable `egui::Window` inside the window that opened it.
    Window,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mech {
    Deferred,
    Immediate,
    Embedded,
    Tab,
}

#[derive(Clone, Copy)]
struct Opts {
    animate: bool,
    on_top: bool,
    esc: bool,
    enter: bool,
    return_focus: bool,
    confirm_exit: bool,
    dim: bool,
    cascade: bool,
    diag: bool,
    style: DlgStyle,
    mech: Mech,
}

impl Opts {
    fn get_mut(&mut self, key: &str) -> Option<&mut bool> {
        Some(match key {
            "animate" => &mut self.animate,
            "on_top" => &mut self.on_top,
            "esc" => &mut self.esc,
            "enter" => &mut self.enter,
            "return_focus" => &mut self.return_focus,
            "confirm_exit" => &mut self.confirm_exit,
            "dim" => &mut self.dim,
            "cascade" => &mut self.cascade,
            "diag" => &mut self.diag,
            _ => return None,
        })
    }
}

/// (key, label, applies to)
const TOGGLES: &[(&str, &str, &str)] = &[
    (
        "animate",
        "Repaint continuously in every window (stress)",
        "all",
    ),
    (
        "on_top",
        "Detached windows and OS Dialogs always on top",
        "A B C",
    ),
    ("esc", "Esc closes the top Dialog", "all"),
    ("enter", "Enter presses the default button", "all"),
    (
        "return_focus",
        "Return focus to what opened the Dialog",
        "all",
    ),
    (
        "confirm_exit",
        "Confirm before closing the Main window",
        "all",
    ),
    ("dim", "Dim the window behind a Dialog", "A B C E"),
    (
        "cascade",
        "Place Detached windows beside the Main window",
        "A B C",
    ),
    ("diag", "Window diagnostics strip", "all"),
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
enum Variant {
    A,
    B,
    C,
    D,
    E,
}

const VARIANT_KEYS: [(Key, &str); 5] = [
    (Key::A, "A"),
    (Key::B, "B"),
    (Key::C, "C"),
    (Key::D, "D"),
    (Key::E, "E"),
];

impl Variant {
    const ALL: [Self; 5] = [Self::A, Self::B, Self::C, Self::D, Self::E];

    fn key(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
            Self::D => "d",
            Self::E => "e",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|v| v.key().eq_ignore_ascii_case(key))
    }

    fn label(self) -> &'static str {
        match self {
            Self::A => "A · Modal + OS window (deferred)",
            Self::B => "B · Modal + OS window (immediate)",
            Self::C => "C · OS-window Dialog + OS window (deferred)",
            Self::D => "D · In-canvas windows only",
            Self::E => "E · Modal + Detached as Main view tab",
        }
    }

    fn style(self) -> DlgStyle {
        match self {
            Self::A | Self::B | Self::E => DlgStyle::Modal,
            Self::C => DlgStyle::OsWindow,
            Self::D => DlgStyle::Window,
        }
    }

    fn mech(self) -> Mech {
        match self {
            Self::A | Self::C => Mech::Deferred,
            Self::B => Mech::Immediate,
            Self::D => Mech::Embedded,
            Self::E => Mech::Tab,
        }
    }

    fn step(self, delta: isize) -> Self {
        let n = Self::ALL.len() as isize;
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0) as isize;
        Self::ALL[(i + delta).rem_euclid(n) as usize]
    }
}

/// Events from any window, drained into the harness log by the Main window each frame.
#[derive(Clone)]
struct Sink {
    events: Arc<Mutex<Vec<(String, Value)>>>,
    /// Responses from the root viewport, handed to `Harness::track` so it can name the widget in
    /// its focus, key and typing events.
    tracks: Arc<Mutex<Vec<(egui::Response, String)>>>,
    start: Instant,
}

impl Sink {
    fn new() -> Self {
        Self {
            events: Arc::default(),
            tracks: Arc::default(),
            start: Instant::now(),
        }
    }

    fn event(&self, kind: &str, mut fields: Value) {
        if let Value::Object(map) = &mut fields {
            let at = u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX);
            map.insert("at_ms".to_owned(), json!(at));
        }
        self.events.lock().unwrap().push((kind.to_owned(), fields));
    }

    /// Name a widget; log its clicks.
    fn click(&self, r: &egui::Response, name: &str) {
        if r.ctx.viewport_id() == ViewportId::ROOT {
            self.tracks
                .lock()
                .unwrap()
                .push((r.clone(), name.to_owned()));
        } else {
            if r.clicked() {
                self.event("click", json!({ "widget": name }));
            }
            if r.secondary_clicked() {
                self.event("right_click", json!({ "widget": name }));
            }
        }
    }

    /// Log an edit of a text field (the harness only sees typing in the root viewport).
    fn edited(&self, r: &egui::Response, field: &str, text: &str) {
        self.click(r, field);
        if r.changed() {
            self.event("field_edit", json!({ "field": field, "len": text.len() }));
        }
    }
}

struct ViewStats {
    frames: u64,
    last: Instant,
    win_start: Instant,
    win_frames: u32,
    fps: f32,
    focused: Option<bool>,
    occluded: Option<bool>,
    outer: Option<Rect>,
    inner: Option<Rect>,
}

struct Shared {
    opts: Mutex<Opts>,
    sink: Sink,
    tx: Sender<Action>,
    stats: Mutex<BTreeMap<String, ViewStats>>,
    forced: Mutex<Option<Forced>>,
    uid: AtomicU64,
}

impl Shared {
    fn opts(&self) -> Opts {
        *self.opts.lock().unwrap()
    }

    fn next_uid(&self) -> u64 {
        self.uid.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a command to the Main window and wake it.
    fn send(&self, ctx: &Context, action: Action) {
        self.sink
            .event("command", json!({ "would_emit": action.name() }));
        let _ = self.tx.send(action);
        ctx.request_repaint_of(ViewportId::ROOT);
    }

    /// Call first in every window's frame: frame statistics, focus and occlusion changes, and the
    /// continuous-repaint stress.
    fn frame_tick(&self, ctx: &Context, name: &str) {
        let now = Instant::now();
        let (focused, occluded, outer, inner) = ctx.input(|i| {
            let v = i.viewport();
            (v.focused, v.occluded, v.outer_rect, v.inner_rect)
        });
        {
            let mut stats = self.stats.lock().unwrap();
            let s = stats.entry(name.to_owned()).or_insert_with(|| ViewStats {
                frames: 0,
                last: now,
                win_start: now,
                win_frames: 0,
                fps: 0.0,
                focused: None,
                occluded: None,
                outer: None,
                inner: None,
            });
            let gap = now.duration_since(s.last);
            if s.frames > 0 && gap > Duration::from_millis(750) {
                self.sink.event(
                    "frame_gap",
                    json!({ "window": name, "gap_ms": gap.as_millis() }),
                );
            }
            s.last = now;
            s.frames += 1;
            s.win_frames += 1;
            let span = now.duration_since(s.win_start);
            if span >= Duration::from_secs(1) {
                s.fps = s.win_frames as f32 / span.as_secs_f32();
                s.win_frames = 0;
                s.win_start = now;
                self.sink.event(
                    "viewport_perf",
                    json!({ "window": name, "fps": (s.fps * 10.0).round() / 10.0, "frames": s.frames }),
                );
            }
            if focused != s.focused {
                self.sink.event(
                    "viewport_focus",
                    json!({ "window": name, "focused": focused }),
                );
                s.focused = focused;
            }
            if occluded != s.occluded {
                self.sink.event(
                    "viewport_occluded",
                    json!({ "window": name, "occluded": occluded }),
                );
                s.occluded = occluded;
            }
            s.outer = outer;
            s.inner = inner;
        }
        if self.opts().animate {
            ctx.request_repaint();
        }
    }

    fn forget(&self, name: &str) {
        self.stats.lock().unwrap().remove(name);
    }
}

/// A one-line strip of facts about the window it sits in, and a spinner while stressing.
fn diag_strip(ui: &mut Ui, shared: &Shared, name: &str) {
    let opts = shared.opts();
    if !opts.diag && !opts.animate {
        return;
    }
    let text = {
        let stats = shared.stats.lock().unwrap();
        stats.get(name).map_or_else(String::new, |s| {
            let pos = s.outer.map_or("pos ?".to_owned(), |r| {
                format!("pos {:.0},{:.0}", r.min.x, r.min.y)
            });
            let size = s.inner.map_or("size ?".to_owned(), |r| {
                format!("{:.0}×{:.0}", r.width(), r.height())
            });
            format!(
                "{name} · {:.0} fps · frame {} · {pos} · {size} · {}{}",
                s.fps,
                s.frames,
                if s.focused == Some(true) {
                    "focused"
                } else {
                    "unfocused"
                },
                if s.occluded == Some(true) {
                    " · OCCLUDED"
                } else {
                    ""
                },
            )
        })
    };
    ui.horizontal(|ui| {
        if opts.animate {
            ui.add(egui::Spinner::new());
        }
        if opts.diag {
            ui.label(RichText::new(text).small().monospace().weak());
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Commands the windows send to the Main window

struct NewAccount {
    name: String,
    kind: usize,
    institution: String,
    opening_cents: i64,
    date: String,
}

#[derive(Clone)]
struct ManageRow {
    orig: String,
    name: String,
    hidden: bool,
    closed: bool,
    deleted: bool,
}

enum Action {
    CreateAccount(NewAccount),
    ApplyManage(Vec<ManageRow>),
    DeleteAccount(String),
    CloseDetached(u64),
    Exit,
    /// Handled by the host that shows the Dialogs: mark a Manage Accounts row deleted.
    DeleteManageRow(usize),
    /// Handled by the host that shows the Dialogs: close the Dialog under the one answering.
    DiscardBelow,
}

impl Action {
    fn name(&self) -> String {
        match self {
            Self::CreateAccount(a) => format!("DomainCommand::CreateAccount({})", a.name),
            Self::ApplyManage(rows) => {
                format!("DomainCommand::ApplyAccountEdits({} rows)", rows.len())
            }
            Self::DeleteAccount(n) => format!("DomainCommand::DeleteAccount({n})"),
            Self::CloseDetached(id) => format!("AppCommand::CloseDetachedWindow({id})"),
            Self::Exit => "AppCommand::Exit".to_owned(),
            Self::DeleteManageRow(i) => format!("(dialog-local) DeleteRow({i})"),
            Self::DiscardBelow => "(dialog-local) DiscardDialog".to_owned(),
        }
    }
}

/// A control-file command aimed at the top Dialog of the Main window.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Forced {
    Ok,
    Cancel,
    Next,
    Back,
}

// ---------------------------------------------------------------------------------------------
// Dialogs

const WIZARD_STEPS: [&str; 4] = ["Type", "Details", "Opening balance", "Review"];
const KIND_NAMES: [&str; 6] = [
    "Checking",
    "Savings",
    "Credit card",
    "Cash",
    "Investment",
    "Loan",
];

struct Wizard {
    step: usize,
    kind: usize,
    name: String,
    institution: String,
    opening: String,
    date: String,
    error: Option<String>,
    focus_pending: bool,
}

impl Wizard {
    fn new() -> Self {
        Self {
            step: 0,
            kind: 0,
            name: String::new(),
            institution: String::new(),
            opening: "0.00".to_owned(),
            date: "2026-10-14".to_owned(),
            error: None,
            focus_pending: true,
        }
    }

    fn dirty(&self) -> bool {
        self.step > 0
            || self.kind != 0
            || !self.name.is_empty()
            || !self.institution.is_empty()
            || self.opening != "0.00"
    }

    fn opening_cents(&self) -> Option<i64> {
        let t = self.opening.trim().replace(',', "");
        let (neg, t) = t
            .strip_prefix('-')
            .map_or((false, t.as_str()), |r| (true, r));
        let (whole, frac) = t.split_once('.').unwrap_or((t, ""));
        if whole.is_empty() && frac.is_empty() || frac.len() > 2 {
            return None;
        }
        let w: i64 = if whole.is_empty() {
            0
        } else {
            whole.parse().ok()?
        };
        let f: i64 = format!("{frac:0<2}").parse().ok()?;
        Some(if neg { -(w * 100 + f) } else { w * 100 + f })
    }

    fn validate(&self) -> Result<(), String> {
        match self.step {
            1 if self.name.trim().is_empty() => Err("Enter a name for the account.".to_owned()),
            2 if self.opening_cents().is_none() => {
                Err("Opening balance must be a number with at most two decimals.".to_owned())
            }
            2 if !valid_date(&self.date) => {
                Err("Opening date must look like 2026-10-14.".to_owned())
            }
            _ => Ok(()),
        }
    }
}

fn valid_date(s: &str) -> bool {
    let parts: Vec<&str> = s.trim().split('-').collect();
    matches!(parts.as_slice(), [y, m, d]
        if y.len() == 4
            && y.parse::<u32>().is_ok()
            && m.parse::<u32>().is_ok_and(|m| (1..=12).contains(&m))
            && d.parse::<u32>().is_ok_and(|d| (1..=31).contains(&d)))
}

struct Confirm {
    title: String,
    text: String,
    ok: String,
    on_ok: Option<Action>,
}

struct Manage {
    rows: Vec<ManageRow>,
}

enum DialogKind {
    Confirm(Confirm),
    Wizard(Wizard),
    Manage(Manage),
}

struct Dialog {
    uid: u64,
    opener: Option<Id>,
    fresh: bool,
    pos: Option<Pos2>,
    kind: DialogKind,
}

impl Dialog {
    fn title(&self) -> String {
        match &self.kind {
            DialogKind::Confirm(c) => c.title.clone(),
            DialogKind::Wizard(_) => "New Account".to_owned(),
            DialogKind::Manage(_) => "Manage Accounts".to_owned(),
        }
    }

    fn short(&self) -> &'static str {
        match &self.kind {
            DialogKind::Confirm(_) => "confirm",
            DialogKind::Wizard(_) => "wizard",
            DialogKind::Manage(_) => "manage",
        }
    }

    fn size(&self) -> Vec2 {
        match &self.kind {
            DialogKind::Confirm(_) => vec2(420.0, 170.0),
            DialogKind::Wizard(_) => vec2(520.0, 400.0),
            DialogKind::Manage(_) => vec2(600.0, 440.0),
        }
    }
}

enum Done {
    Ok(Option<Action>),
    Cancel(&'static str),
    Push(DialogKind),
}

struct DlgCx<'a> {
    shared: &'a Shared,
    opts: Opts,
    top: bool,
    forced: Option<Forced>,
}

fn primary_button(ui: &mut Ui, label: &str) -> egui::Response {
    let fill = ui.visuals().selection.bg_fill;
    ui.add(Button::new(RichText::new(label).strong()).fill(fill))
}

fn dialog_ui(ui: &mut Ui, d: &mut Dialog, cx: &DlgCx<'_>) -> Option<Done> {
    let esc = cx.top && cx.opts.esc && ui.input(|i| i.key_pressed(Key::Escape));
    let enter = cx.top && cx.opts.enter && ui.input(|i| i.key_pressed(Key::Enter));
    let sink = &cx.shared.sink;
    let fresh = std::mem::take(&mut d.fresh);
    let tag = d.short();
    if esc {
        sink.event("dialog_key", json!({ "dialog": tag, "key": "Escape" }));
    }
    if enter {
        sink.event("dialog_key", json!({ "dialog": tag, "key": "Enter" }));
    }
    match &mut d.kind {
        DialogKind::Confirm(c) => {
            ui.set_min_width(380.0);
            ui.heading(&c.title);
            ui.add_space(6.0);
            ui.label(&c.text);
            ui.add_space(12.0);
            ui.separator();
            let mut out = None;
            Sides::new().show(
                ui,
                |_| {},
                |ui| {
                    let ok = primary_button(ui, &c.ok);
                    sink.click(&ok, &format!("confirm: {}", c.ok));
                    if fresh {
                        ok.request_focus();
                    }
                    let cancel = ui.button("Cancel");
                    sink.click(&cancel, "confirm: Cancel");
                    if ok.clicked()
                        || (enter && cx.forced.is_none())
                        || cx.forced == Some(Forced::Ok)
                    {
                        out = Some(Done::Ok(c.on_ok.take()));
                    } else if cancel.clicked() || esc || cx.forced == Some(Forced::Cancel) {
                        out = Some(Done::Cancel(if esc { "esc" } else { "cancel" }));
                    }
                },
            );
            out
        }
        DialogKind::Wizard(w) => wizard_ui(ui, w, cx, enter, esc),
        DialogKind::Manage(m) => {
            ui.set_min_width(560.0);
            ui.heading("Manage Accounts");
            ui.label(
                RichText::new("Rename, hide, close or delete accounts. Nothing is saved until OK.")
                    .small()
                    .weak(),
            );
            ui.separator();
            let mut push = None;
            egui::ScrollArea::vertical()
                .max_height(280.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("manage_grid").num_columns(4).striped(true).show(ui, |ui| {
                        ui.strong("Name");
                        ui.strong("Hidden");
                        ui.strong("Closed");
                        ui.strong("");
                        ui.end_row();
                        for (i, row) in m.rows.iter_mut().enumerate() {
                            if row.deleted {
                                ui.label(RichText::new(&row.name).strikethrough().weak());
                                ui.label("");
                                ui.label("");
                                ui.label(RichText::new("will be deleted").small());
                                ui.end_row();
                                continue;
                            }
                            let r = ui.add(egui::TextEdit::singleline(&mut row.name).desired_width(240.0));
                            sink.edited(&r, &format!("manage: name row {i}"), &row.name);
                            if fresh && i == 0 {
                                r.request_focus();
                            }
                            let h = ui.checkbox(&mut row.hidden, "");
                            sink.click(&h, &format!("manage: hidden row {i}"));
                            let c = ui.checkbox(&mut row.closed, "");
                            sink.click(&c, &format!("manage: closed row {i}"));
                            let del = ui.small_button("Delete…");
                            sink.click(&del, &format!("manage: delete row {i}"));
                            if del.clicked() {
                                push = Some(DialogKind::Confirm(Confirm {
                                    title: "Delete account?".to_owned(),
                                    text: format!(
                                        "Delete “{}” and all of its transactions? This can't be undone.",
                                        row.orig
                                    ),
                                    ok: "Delete".to_owned(),
                                    on_ok: Some(Action::DeleteManageRow(i)),
                                }));
                            }
                            ui.end_row();
                        }
                    });
                });
            ui.separator();
            let mut out = push.map(Done::Push);
            Sides::new().show(
                ui,
                |_| {},
                |ui| {
                    let ok = primary_button(ui, "OK");
                    sink.click(&ok, "manage: OK");
                    let cancel = ui.button("Cancel");
                    sink.click(&cancel, "manage: Cancel");
                    if out.is_none() {
                        if ok.clicked() || cx.forced == Some(Forced::Ok) {
                            out = Some(Done::Ok(Some(Action::ApplyManage(m.rows.clone()))));
                        } else if cancel.clicked() || esc || cx.forced == Some(Forced::Cancel) {
                            out = Some(Done::Cancel(if esc { "esc" } else { "cancel" }));
                        }
                    }
                },
            );
            out
        }
    }
}

fn wizard_ui(ui: &mut Ui, w: &mut Wizard, cx: &DlgCx<'_>, enter: bool, esc: bool) -> Option<Done> {
    let sink = &cx.shared.sink;
    ui.set_min_width(480.0);
    ui.heading("New Account");
    ui.horizontal(|ui| {
        for (i, label) in WIZARD_STEPS.iter().enumerate() {
            let text = RichText::new(format!("{}. {label}", i + 1));
            ui.label(if i == w.step {
                text.strong()
            } else if i < w.step {
                text
            } else {
                text.weak()
            });
            if i + 1 < WIZARD_STEPS.len() {
                ui.label(RichText::new("›").weak());
            }
        }
    });
    ui.separator();
    let pending = std::mem::take(&mut w.focus_pending);
    ui.scope(|ui| {
        ui.set_min_height(190.0);
        match w.step {
            0 => {
                ui.label("What kind of account is it?");
                for (i, name) in KIND_NAMES.iter().enumerate() {
                    let r = ui.radio_value(&mut w.kind, i, *name);
                    sink.click(&r, &format!("wizard: type {name}"));
                    if pending && i == w.kind {
                        r.request_focus();
                    }
                }
            }
            1 => {
                ui.label("Name and institution");
                let r = ui.add(
                    egui::TextEdit::singleline(&mut w.name)
                        .hint_text("Account name")
                        .desired_width(300.0),
                );
                sink.edited(&r, "wizard: name", &w.name);
                if pending {
                    r.request_focus();
                }
                let r = ui.add(
                    egui::TextEdit::singleline(&mut w.institution)
                        .hint_text("Institution (optional)")
                        .desired_width(300.0),
                );
                sink.edited(&r, "wizard: institution", &w.institution);
            }
            2 => {
                ui.label("Opening balance and date");
                let r = ui.add(egui::TextEdit::singleline(&mut w.opening).desired_width(140.0));
                sink.edited(&r, "wizard: opening balance", &w.opening);
                if pending {
                    r.request_focus();
                }
                let r = ui.add(egui::TextEdit::singleline(&mut w.date).desired_width(140.0));
                sink.edited(&r, "wizard: opening date", &w.date);
            }
            _ => {
                ui.label("Review");
                egui::Grid::new("wizard_review")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Type");
                        ui.label(KIND_NAMES[w.kind]);
                        ui.end_row();
                        ui.label("Name");
                        ui.label(&w.name);
                        ui.end_row();
                        ui.label("Institution");
                        ui.label(if w.institution.is_empty() {
                            "—"
                        } else {
                            &w.institution
                        });
                        ui.end_row();
                        ui.label("Opening balance");
                        ui.label(fmt_cents(w.opening_cents().unwrap_or(0)));
                        ui.end_row();
                        ui.label("As of");
                        ui.label(&w.date);
                        ui.end_row();
                    });
            }
        }
    });
    match &w.error {
        Some(e) => ui.colored_label(ui.visuals().error_fg_color, e),
        None => ui.label(" "),
    };
    ui.separator();

    let (mut next, mut back, mut cancel) = (false, false, false);
    Sides::new().show(
        ui,
        |ui| {
            let r = ui.button("Cancel");
            sink.click(&r, "wizard: Cancel");
            cancel = r.clicked();
        },
        |ui| {
            let last = w.step + 1 == WIZARD_STEPS.len();
            let r = primary_button(ui, if last { "Finish" } else { "Next ›" });
            sink.click(
                &r,
                if last {
                    "wizard: Finish"
                } else {
                    "wizard: Next"
                },
            );
            next = r.clicked();
            if w.step > 0 {
                let r = ui.button("‹ Back");
                sink.click(&r, "wizard: Back");
                back = r.clicked();
            }
        },
    );
    match cx.forced {
        Some(Forced::Next | Forced::Ok) => next = true,
        Some(Forced::Back) => back = true,
        Some(Forced::Cancel) => cancel = true,
        None => {}
    }
    if !next && !back && !cancel {
        next = enter;
        cancel = esc;
    }

    if back && w.step > 0 {
        w.step -= 1;
        w.error = None;
        w.focus_pending = true;
        sink.event("wizard_step", json!({ "to": w.step, "via": "back" }));
    } else if next {
        match w.validate() {
            Err(e) => {
                sink.event("wizard_invalid", json!({ "step": w.step, "error": e }));
                w.error = Some(e);
            }
            Ok(()) if w.step + 1 == WIZARD_STEPS.len() => {
                return Some(Done::Ok(Some(Action::CreateAccount(NewAccount {
                    name: w.name.trim().to_owned(),
                    kind: w.kind,
                    institution: w.institution.clone(),
                    opening_cents: w.opening_cents().unwrap_or(0),
                    date: w.date.clone(),
                }))));
            }
            Ok(()) => {
                w.step += 1;
                w.error = None;
                w.focus_pending = true;
                sink.event("wizard_step", json!({ "to": w.step, "via": "next" }));
            }
        }
    } else if cancel {
        if w.dirty() {
            return Some(Done::Push(DialogKind::Confirm(Confirm {
                title: "Discard new account?".to_owned(),
                text: "The account you were setting up hasn't been created. Discard it?".to_owned(),
                ok: "Discard".to_owned(),
                on_ok: Some(Action::DiscardBelow),
            })));
        }
        return Some(Done::Cancel(if esc { "esc" } else { "cancel" }));
    }
    None
}

fn open_dialog(
    ctx: &Context,
    shared: &Shared,
    host: &str,
    stack: &mut Vec<Dialog>,
    kind: DialogKind,
    opener: Option<Id>,
    via: &str,
) {
    let opts = shared.opts();
    let mut d = Dialog {
        uid: shared.next_uid(),
        opener,
        fresh: true,
        pos: None,
        kind,
    };
    if opts.style == DlgStyle::OsWindow
        && let Some(outer) = ctx.input(|i| i.viewport().outer_rect)
    {
        d.pos = Some(outer.center() - d.size() / 2.0);
    }
    shared.sink.event(
        "dialog_open",
        json!({ "dialog": d.short(), "host": host, "via": via, "depth": stack.len() + 1,
                "style": match opts.style { DlgStyle::Modal => "modal", DlgStyle::OsWindow => "os_window", DlgStyle::Window => "window" } }),
    );
    stack.push(d);
}

fn confirm(title: &str, text: &str, ok: &str, on_ok: Action) -> DialogKind {
    DialogKind::Confirm(Confirm {
        title: title.to_owned(),
        text: text.to_owned(),
        ok: ok.to_owned(),
        on_ok: Some(on_ok),
    })
}

/// Show every Dialog hosted by a window, in the style of the current variant, and handle at most
/// one answer. Call inside the host window's frame.
fn show_dialogs(
    ctx: &Context,
    shared: &Shared,
    host: &str,
    stack: &mut Vec<Dialog>,
    is_main: bool,
) {
    if stack.is_empty() {
        return;
    }
    let opts = shared.opts();
    let host_vid = ctx.viewport_id();
    let mut forced = if is_main {
        shared.forced.lock().unwrap().take()
    } else {
        None
    };
    let n = stack.len();
    let mut answer: Option<(usize, Done)> = None;
    let mut top_vid = None;
    let backdrop = egui::Color32::from_black_alpha(if opts.dim { 110 } else { 0 });

    for i in 0..n {
        let top = i == n - 1;
        let cx = DlgCx {
            shared,
            opts,
            top,
            forced: if top { forced.take() } else { None },
        };
        let uid = stack[i].uid;
        let title = stack[i].title();
        let vid = ViewportId::from_hash_of(("dialog", uid));
        let res = match opts.style {
            DlgStyle::Modal => {
                let m = egui::Modal::new(Id::new(("dlg", uid)))
                    .backdrop_color(backdrop)
                    .show(ctx, |ui| dialog_ui(ui, &mut stack[i], &cx));
                if m.backdrop_response.clicked() && top {
                    shared.sink.event(
                        "backdrop_click",
                        json!({ "host": host, "dialog": stack[i].short() }),
                    );
                }
                m.inner
            }
            DlgStyle::Window => egui::Window::new(&title)
                .id(Id::new(("dlgwin", uid)))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| dialog_ui(ui, &mut stack[i], &cx))
                .and_then(|r| r.inner.flatten()),
            DlgStyle::OsWindow => {
                let size = stack[i].size();
                let mut b = ViewportBuilder::default()
                    .with_title(&title)
                    .with_inner_size(size)
                    .with_resizable(false)
                    .with_minimize_button(false)
                    .with_maximize_button(false)
                    .with_window_type(egui::X11WindowType::Dialog);
                if opts.on_top {
                    b = b.with_always_on_top();
                }
                if let Some(p) = stack[i].pos {
                    b = b.with_position(p);
                }
                if top {
                    top_vid = Some(vid);
                }
                let name = format!("dialog#{uid}");
                ctx.show_viewport_immediate(vid, b, |ui, _| {
                    let ctx = ui.ctx().clone();
                    shared.frame_tick(&ctx, &name);
                    let mut out = None;
                    CentralPanel::default().show(ui, |ui| {
                        diag_strip(ui, shared, &name);
                        out = dialog_ui(ui, &mut stack[i], &cx);
                    });
                    if ctx.input(|i| i.viewport().close_requested()) {
                        out = Some(Done::Cancel("window_close"));
                    }
                    out
                })
            }
        };
        if let Some(r) = res
            && answer.is_none()
        {
            answer = Some((i, r));
        }
    }

    if opts.style == DlgStyle::OsWindow {
        let rect = ctx.content_rect();
        egui::Area::new(Id::new(("scrim", host)))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                let (rect, r) = ui.allocate_exact_size(rect.size(), Sense::click_and_drag());
                ui.painter().rect_filled(rect, 0.0, backdrop);
                if r.is_pointer_button_down_on()
                    && ctx.input(|i| i.pointer.any_pressed())
                    && let Some(v) = top_vid
                {
                    shared.sink.event(
                        "scrim_click",
                        json!({ "host": host, "action": "focus the Dialog window" }),
                    );
                    ctx.send_viewport_cmd_to(v, ViewportCommand::Focus);
                }
            });
    }

    let Some((i, done)) = answer else { return };
    match done {
        Done::Push(kind) => open_dialog(ctx, shared, host, stack, kind, None, "dialog"),
        Done::Ok(action) => {
            let mut removed = vec![stack.remove(i)];
            let reason = "ok";
            match action {
                Some(Action::DeleteManageRow(row)) => {
                    for d in stack.iter_mut() {
                        if let DialogKind::Manage(m) = &mut d.kind
                            && let Some(r) = m.rows.get_mut(row)
                        {
                            r.deleted = true;
                        }
                    }
                }
                Some(Action::DiscardBelow) => {
                    if i > 0 && i - 1 < stack.len() {
                        removed.push(stack.remove(i - 1));
                    }
                }
                Some(other) => shared.send(ctx, other),
                None => {}
            }
            close_dialogs(ctx, shared, host, host_vid, &removed, reason);
        }
        Done::Cancel(reason) => {
            let removed = vec![stack.remove(i)];
            close_dialogs(ctx, shared, host, host_vid, &removed, reason);
        }
    }
}

fn close_dialogs(
    ctx: &Context,
    shared: &Shared,
    host: &str,
    host_vid: ViewportId,
    removed: &[Dialog],
    reason: &str,
) {
    let opts = shared.opts();
    for d in removed {
        shared.sink.event(
            "dialog_close",
            json!({ "dialog": d.short(), "host": host, "reason": reason }),
        );
        shared.forget(&format!("dialog#{}", d.uid));
    }
    let opener = removed.iter().rev().find_map(|d| d.opener);
    if opts.return_focus {
        if let Some(id) = opener {
            ctx.memory_mut(|m| m.request_focus(id));
            shared.sink.event(
                "focus_return",
                json!({ "host": host, "to": "opener widget" }),
            );
        } else {
            shared.sink.event(
                "focus_return",
                json!({ "host": host, "to": "nothing (opened from a menu or a Dialog)" }),
            );
        }
    }
    if opts.style == DlgStyle::OsWindow && host_vid != ViewportId::ROOT
        || opts.style == DlgStyle::OsWindow
    {
        ctx.send_viewport_cmd_to(host_vid, ViewportCommand::Focus);
    }
}

// ---------------------------------------------------------------------------------------------
// Detached windows

enum DetContent {
    Register {
        account: String,
        txns: Arc<Vec<Txn>>,
        selected: Option<usize>,
    },
    Reconcile {
        account: String,
        rows: Vec<(String, i64, bool)>,
        statement_end: String,
    },
}

struct Detached {
    id: u64,
    title: String,
    pos: Option<Pos2>,
    content: DetContent,
    dialogs: Vec<Dialog>,
}

impl Detached {
    fn name(&self) -> String {
        format!("detached#{}", self.id)
    }

    fn dirty(&self) -> bool {
        match &self.content {
            DetContent::Reconcile { rows, .. } => rows.iter().any(|r| r.2),
            DetContent::Register { .. } => false,
        }
    }

    fn account(&self) -> &str {
        match &self.content {
            DetContent::Register { account, .. } | DetContent::Reconcile { account, .. } => account,
        }
    }
}

/// Ask a Detached window to close. Returns true when it can close now; otherwise a confirm
/// Dialog is up inside the window.
fn request_close(ctx: &Context, shared: &Shared, d: &mut Detached, via: &str) -> bool {
    shared.sink.event(
        "detached_close_requested",
        json!({ "window": d.name(), "via": via }),
    );
    if d.dirty() {
        if d.dialogs.is_empty() {
            let id = d.id;
            let name = d.name();
            open_dialog(
                ctx,
                shared,
                &name,
                &mut d.dialogs,
                confirm(
                    "Discard reconcile progress?",
                    "Some transactions are ticked but the reconcile isn't finished. Close anyway?",
                    "Discard",
                    Action::CloseDetached(id),
                ),
                None,
                "close request",
            );
        }
        return false;
    }
    true
}

fn register_table(
    ui: &mut Ui,
    txns: &[Txn],
    salt: &str,
    selected: &mut Option<usize>,
    shared: &Shared,
) {
    let n = txns.len();
    let mut clicked = None;
    TableBuilder::new(ui)
        .id_salt(("register", salt))
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::exact(88.0))
        .column(Column::initial(190.0).at_least(80.0))
        .column(Column::initial(170.0).at_least(80.0))
        .column(Column::initial(96.0))
        .column(Column::remainder().at_least(96.0))
        .header(22.0, |mut h| {
            for t in ["Date", "Payee", "Category", "Amount", "Balance"] {
                h.col(|ui| {
                    ui.strong(t);
                });
            }
        })
        .body(|body| {
            body.rows(22.0, n, |mut row| {
                let r = n - 1 - row.index();
                let t = &txns[r];
                row.set_selected(*selected == Some(r));
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
                    ui.label(fmt_cents(t.amount));
                });
                row.col(|ui| {
                    ui.label(fmt_cents(t.balance));
                });
                if row.response().clicked() {
                    clicked = Some(r);
                }
            });
        });
    if let Some(r) = clicked {
        *selected = Some(r);
        shared
            .sink
            .event("click", json!({ "widget": format!("{salt}: row {r}") }));
    }
}

/// The body of a Detached window, wherever it is drawn: an OS window, an embedded window or a tab.
fn detached_content(ui: &mut Ui, shared: &Shared, d: &mut Detached) {
    let ctx = ui.ctx().clone();
    let name = d.name();
    diag_strip(ui, shared, &name);
    let mut want_new_account = None;
    let mut want_close = false;
    let mut want_delete: Option<String> = None;
    let mut want_finish = false;
    ui.horizontal(|ui| {
        ui.strong(&d.title);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let r = ui.button("Close");
            shared.sink.click(&r, &format!("{name}: Close"));
            want_close = r.clicked();
            let r = ui.button("New Account…");
            shared.sink.click(&r, &format!("{name}: New Account…"));
            if r.clicked() {
                want_new_account = Some(r.id);
            }
            match &d.content {
                DetContent::Register { account, .. } => {
                    let r = ui.button("Delete Account…");
                    shared.sink.click(&r, &format!("{name}: Delete Account…"));
                    if r.clicked() {
                        want_delete = Some(account.clone());
                    }
                }
                DetContent::Reconcile { .. } => {
                    let r = ui.button("Finish…");
                    shared.sink.click(&r, &format!("{name}: Finish…"));
                    want_finish = r.clicked();
                }
            }
        });
    });
    ui.separator();
    let blocked = !d.dialogs.is_empty() && shared.opts().style != DlgStyle::Window;
    ui.add_enabled_ui(
        !blocked || shared.opts().style == DlgStyle::OsWindow,
        |ui| match &mut d.content {
            DetContent::Register { txns, selected, .. } => {
                let txns = Arc::clone(txns);
                register_table(ui, &txns, &name, selected, shared);
            }
            DetContent::Reconcile {
                rows,
                statement_end,
                ..
            } => {
                ui.horizontal(|ui| {
                    ui.label("Statement ending balance");
                    let r = ui.add(egui::TextEdit::singleline(statement_end).desired_width(110.0));
                    shared
                        .sink
                        .edited(&r, &format!("{name}: statement balance"), statement_end);
                });
                let ticked: i64 = rows.iter().filter(|r| r.2).map(|r| r.1).sum();
                ui.label(format!("Ticked: {}", fmt_cents(ticked)));
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (i, (payee, amount, ticked)) in rows.iter_mut().enumerate() {
                            let r =
                                ui.checkbox(ticked, format!("{payee}   {}", fmt_cents(*amount)));
                            shared.sink.click(&r, &format!("{name}: tick row {i}"));
                        }
                    });
            }
        },
    );

    let host = d.name();
    let account = d.account().to_owned();
    if let Some(id) = want_new_account {
        open_dialog(
            &ctx,
            shared,
            &host,
            &mut d.dialogs,
            DialogKind::Wizard(Wizard::new()),
            Some(id),
            "button",
        );
    }
    if let Some(a) = want_delete {
        open_dialog(
            &ctx,
            shared,
            &host,
            &mut d.dialogs,
            confirm(
                "Delete account?",
                &format!("Delete “{a}” and all of its transactions? This can't be undone."),
                "Delete",
                Action::DeleteAccount(a.clone()),
            ),
            None,
            "button",
        );
    }
    if want_finish {
        open_dialog(
            &ctx,
            shared,
            &host,
            &mut d.dialogs,
            confirm(
                "Finish reconcile?",
                &format!(
                    "Mark the ticked transactions in “{account}” as reconciled and close this window?"
                ),
                "Finish",
                Action::CloseDetached(d.id),
            ),
            None,
            "button",
        );
    }
    if want_close && request_close(&ctx, shared, d, "Close button") {
        shared.send(&ctx, Action::CloseDetached(d.id));
    }
}

/// The frame of a Detached window that is a real OS window.
fn detached_viewport_ui(ui: &mut Ui, shared: &Shared, det: &Mutex<Detached>) {
    let ctx = ui.ctx().clone();
    let mut d = det.lock().unwrap();
    let name = d.name();
    shared.frame_tick(&ctx, &name);
    if ctx.input(|i| i.viewport().close_requested()) {
        if request_close(&ctx, shared, &mut d, "window close button") {
            shared.send(&ctx, Action::CloseDetached(d.id));
        } else {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        }
    }
    CentralPanel::default().show(ui, |ui| detached_content(ui, shared, &mut d));
    let mut dialogs = std::mem::take(&mut d.dialogs);
    show_dialogs(&ctx, shared, &name, &mut dialogs, false);
    // A Dialog answered this frame may have been replaced by a request made in the content.
    dialogs.append(&mut d.dialogs);
    d.dialogs = dialogs;
}

fn detached_builder(d: &Detached, opts: Opts) -> ViewportBuilder {
    let mut b = ViewportBuilder::default()
        .with_title(&d.title)
        .with_inner_size([700.0, 480.0])
        .with_min_inner_size([420.0, 280.0]);
    if let Some(p) = d.pos {
        b = b.with_position(p);
    }
    if opts.on_top {
        b = b.with_always_on_top();
    }
    b
}

// ---------------------------------------------------------------------------------------------
// The Main window

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Register,
    Det(u64),
}

struct Proto {
    h: Harness,
    shared: Arc<Shared>,
    rx: Receiver<Action>,
    accounts: Vec<Account>,
    selected: usize,
    selected_row: Option<usize>,
    variant: Variant,
    o: Opts,
    main_dialogs: Vec<Dialog>,
    detached: Vec<Arc<Mutex<Detached>>>,
    next_det: u64,
    nav: Nav,
    exit_confirmed: bool,
    dock_open: bool,
    status: String,
    last_frame: Instant,
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
        let o = Opts {
            animate: false,
            on_top: false,
            esc: true,
            enter: true,
            return_focus: true,
            confirm_exit: false,
            dim: true,
            cascade: true,
            diag: true,
            style: variant.style(),
            mech: variant.mech(),
        };
        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Shared {
            opts: Mutex::new(o),
            sink: Sink::new(),
            tx,
            stats: Mutex::default(),
            forced: Mutex::default(),
            uid: AtomicU64::new(1),
        });
        Self {
            h,
            shared,
            rx,
            accounts: generate(),
            selected: 0,
            selected_row: None,
            variant,
            o,
            main_dialogs: Vec::new(),
            detached: Vec::new(),
            next_det: 1,
            nav: Nav::Register,
            exit_confirmed: false,
            dock_open: true,
            status: "Ready".to_owned(),
            last_frame: Instant::now(),
        }
    }

    fn emit(&mut self, command: &str, source: &str) {
        self.h.event(
            "command",
            json!({ "would_emit": command, "source": source }),
        );
        self.status = format!("{source} → {command}");
    }

    fn set_variant(&mut self, to: Variant, via: &str) {
        if to == self.variant {
            return;
        }
        self.h.event(
            "variant",
            json!({ "from": self.variant.key(), "to": to.key(), "via": via,
                    "open_dialogs": self.main_dialogs.len(), "open_detached": self.detached.len() }),
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
                    .position(|a| a.name.to_lowercase().contains(&needle))
            })
    }

    fn open_main_dialog(&mut self, ctx: &Context, kind: DialogKind, opener: Option<Id>, via: &str) {
        open_dialog(
            ctx,
            &self.shared,
            "main",
            &mut self.main_dialogs,
            kind,
            opener,
            via,
        );
    }

    fn open_wizard(&mut self, ctx: &Context, opener: Option<Id>, via: &str) {
        self.emit("AppCommand::OpenNewAccountDialog", via);
        self.open_main_dialog(ctx, DialogKind::Wizard(Wizard::new()), opener, via);
    }

    fn open_manage(&mut self, ctx: &Context, opener: Option<Id>, via: &str) {
        self.emit("AppCommand::OpenManageAccountsDialog", via);
        let rows = self
            .accounts
            .iter()
            .map(|a| ManageRow {
                orig: a.name.to_owned(),
                name: a.name.to_owned(),
                hidden: a.hidden,
                closed: a.closed,
                deleted: false,
            })
            .collect();
        self.open_main_dialog(ctx, DialogKind::Manage(Manage { rows }), opener, via);
    }

    fn open_delete(&mut self, ctx: &Context, account: usize, opener: Option<Id>, via: &str) {
        let name = self.accounts[account].name;
        self.emit("AppCommand::OpenDeleteAccountConfirm", via);
        self.open_main_dialog(
            ctx,
            confirm(
                "Delete account?",
                &format!("Delete “{name}” and all of its transactions? This can't be undone."),
                "Delete",
                Action::DeleteAccount(name.to_owned()),
            ),
            opener,
            via,
        );
    }

    fn open_detached(&mut self, ctx: &Context, account: usize, reconcile: bool, via: &str) {
        let acct = &self.accounts[account];
        let id = self.next_det;
        self.next_det += 1;
        let (title, content) = if reconcile {
            let rows = acct
                .txns
                .iter()
                .rev()
                .take(14)
                .map(|t| (t.payee.to_owned(), t.amount, false))
                .collect();
            (
                format!("Reconcile — {}", acct.name),
                DetContent::Reconcile {
                    account: acct.name.to_owned(),
                    rows,
                    statement_end: fmt_cents(acct.balance),
                },
            )
        } else {
            (
                format!("Register — {}", acct.name),
                DetContent::Register {
                    account: acct.name.to_owned(),
                    txns: Arc::clone(&acct.txns),
                    selected: None,
                },
            )
        };
        let main_outer = ctx.input(|i| i.viewport().outer_rect);
        let pos = if self.o.cascade && matches!(self.o.mech, Mech::Deferred | Mech::Immediate) {
            main_outer.map(|r| r.min + vec2(80.0, 60.0) * (1 + self.detached.len() % 5) as f32)
        } else {
            None
        };
        self.h.event(
            "detached_open",
            json!({ "id": id, "title": title, "via": via,
                    "mechanism": match self.o.mech { Mech::Deferred => "deferred", Mech::Immediate => "immediate", Mech::Embedded => "embedded window", Mech::Tab => "tab" },
                    "requested_pos": pos.map(|p| json!([p.x, p.y])),
                    "main_outer_known": main_outer.is_some() }),
        );
        self.emit("AppCommand::OpenDetachedWindow", via);
        self.detached.push(Arc::new(Mutex::new(Detached {
            id,
            title,
            pos,
            content,
            dialogs: Vec::new(),
        })));
        if self.o.mech == Mech::Tab {
            self.nav = Nav::Det(id);
        }
    }

    fn close_detached(&mut self, id: u64, via: &str) {
        if let Some(i) = self
            .detached
            .iter()
            .position(|d| d.lock().unwrap().id == id)
        {
            let d = self.detached.remove(i);
            let name = d.lock().unwrap().name();
            self.shared.forget(&name);
            self.h.event(
                "detached_close",
                json!({ "id": id, "via": via, "remaining": self.detached.len() }),
            );
        }
        if self.nav == Nav::Det(id) {
            self.nav = Nav::Register;
        }
    }

    fn apply(&mut self, ctx: &Context, action: Action) {
        match action {
            Action::CreateAccount(a) => {
                let group = match a.kind {
                    0 | 1 | 3 => Group::Banking,
                    2 => Group::Credit,
                    4 => Group::Investing,
                    _ => Group::PropertyDebt,
                };
                let name: &'static str = Box::leak(a.name.clone().into_boxed_str());
                let day = FIRST_DAY + HISTORY_DAYS;
                self.accounts.push(Account {
                    name,
                    group,
                    txns: Arc::new(vec![Txn {
                        day,
                        num: None,
                        payee: "Opening Balance",
                        category: "[Equity]",
                        amount: a.opening_cents,
                        balance: a.opening_cents,
                        cleared: true,
                    }]),
                    balance: a.opening_cents,
                    hidden: false,
                    closed: false,
                });
                self.selected = self.accounts.len() - 1;
                self.nav = Nav::Register;
                self.status = format!("Created “{name}” ({})", KIND_NAMES[a.kind]);
            }
            Action::ApplyManage(rows) => {
                let mut keep = Vec::new();
                for acct in std::mem::take(&mut self.accounts) {
                    match rows.iter().find(|r| r.orig == acct.name) {
                        Some(r) if r.deleted => {}
                        Some(r) => keep.push(Account {
                            name: if r.name == acct.name {
                                acct.name
                            } else {
                                Box::leak(r.name.clone().into_boxed_str())
                            },
                            hidden: r.hidden,
                            closed: r.closed,
                            ..acct
                        }),
                        None => keep.push(acct),
                    }
                }
                self.accounts = keep;
                self.selected = self.selected.min(self.accounts.len().saturating_sub(1));
                self.status = "Account edits applied".to_owned();
            }
            Action::DeleteAccount(name) => {
                self.accounts.retain(|a| a.name != name);
                self.selected = self.selected.min(self.accounts.len().saturating_sub(1));
                self.status = format!("Deleted “{name}”");
            }
            Action::CloseDetached(id) => self.close_detached(id, "command"),
            Action::Exit => {
                self.exit_confirmed = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            Action::DeleteManageRow(_) | Action::DiscardBelow => {}
        }
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
                    self.nav = Nav::Register;
                    self.h.event(
                        "navigate",
                        json!({ "to": self.accounts[i].name, "via": "control" }),
                    );
                }
                None => bad(self),
            },
            "dialog" => match rest {
                "confirm" => self.open_delete(ctx, self.selected, None, "control"),
                "wizard" => self.open_wizard(ctx, None, "control"),
                "manage" => self.open_manage(ctx, None, "control"),
                _ => bad(self),
            },
            "ok" => *self.shared.forced.lock().unwrap() = Some(Forced::Ok),
            "cancel" => *self.shared.forced.lock().unwrap() = Some(Forced::Cancel),
            "step" => match rest {
                "next" => *self.shared.forced.lock().unwrap() = Some(Forced::Next),
                "back" => *self.shared.forced.lock().unwrap() = Some(Forced::Back),
                _ => bad(self),
            },
            "detach" => {
                let (what, who) = rest
                    .split_once(' ')
                    .map_or((rest, ""), |(w, a)| (w, a.trim()));
                let acct = if who.is_empty() {
                    Some(self.selected)
                } else {
                    self.account_index(who)
                };
                match (what, acct) {
                    ("register", Some(i)) => self.open_detached(ctx, i, false, "control"),
                    ("reconcile", Some(i)) => self.open_detached(ctx, i, true, "control"),
                    _ => bad(self),
                }
            }
            "close" if rest == "detached" => {
                let ids: Vec<u64> = self.detached.iter().map(|d| d.lock().unwrap().id).collect();
                for id in ids {
                    self.close_detached(id, "control");
                }
            }
            "focus" if rest == "main" => {
                ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Focus)
            }
            "reload" => {
                self.accounts = generate();
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

    fn drain_sink(&mut self) {
        let events = std::mem::take(&mut *self.shared.sink.events.lock().unwrap());
        for (kind, fields) in events {
            self.h.event(&kind, fields);
        }
        let tracks = std::mem::take(&mut *self.shared.sink.tracks.lock().unwrap());
        for (r, name) in tracks {
            self.h.track(&r, name);
        }
    }
}

impl eframe::App for Proto {
    fn on_exit(&mut self) {
        self.drain_sink();
        self.h.event("session_end", json!({}));
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.global_style().interaction.selectable_labels {
            ctx.all_styles_mut(|s| s.interaction.selectable_labels = false);
        }
        self.o.style = self.variant.style();
        self.o.mech = self.variant.mech();
        *self.shared.opts.lock().unwrap() = self.o;
        self.shared.frame_tick(&ctx, "main");
        let gap = self.last_frame.elapsed();
        self.last_frame = Instant::now();
        let _ = gap;

        if let Some(cmd) = self.h.begin_frame(&ctx) {
            self.apply_control(&ctx, &cmd);
        }
        let no_dialog = self.main_dialogs.is_empty();
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
        if no_dialog && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::N)) {
            self.open_wizard(&ctx, None, "shortcut Ctrl+N");
        }

        // Closing the Main window.
        if ctx.input(|i| i.viewport().close_requested()) && !self.exit_confirmed {
            let any_dirty = self.detached.iter().any(|d| d.lock().unwrap().dirty());
            self.h.event(
                "main_close_requested",
                json!({ "confirm_exit": self.o.confirm_exit, "detached_open": self.detached.len(), "dirty": any_dirty }),
            );
            if self.o.confirm_exit || any_dirty {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                if self.main_dialogs.is_empty() {
                    let text = if self.detached.is_empty() {
                        "Close Balanced?".to_owned()
                    } else {
                        format!(
                            "Close Balanced? {} Detached window(s) will close with it.",
                            self.detached.len()
                        )
                    };
                    self.open_main_dialog(
                        &ctx,
                        confirm("Exit Balanced?", &text, "Exit", Action::Exit),
                        None,
                        "close request",
                    );
                }
            }
        }

        self.dock(ui);
        self.main_ui(ui, &ctx);
        self.show_detached(&ctx);

        let mut dialogs = std::mem::take(&mut self.main_dialogs);
        show_dialogs(&ctx, &self.shared, "main", &mut dialogs, true);
        dialogs.append(&mut self.main_dialogs);
        self.main_dialogs = dialogs;

        // Dialogs hosted by a Detached window that is a tab or an embedded window.
        if matches!(self.o.mech, Mech::Tab | Mech::Embedded) {
            for det in self.detached.clone() {
                let mut d = det.lock().unwrap();
                let name = d.name();
                let mut stack = std::mem::take(&mut d.dialogs);
                drop(d);
                show_dialogs(&ctx, &self.shared, &name, &mut stack, false);
                let mut d = det.lock().unwrap();
                stack.append(&mut d.dialogs);
                d.dialogs = stack;
            }
        }

        while let Ok(action) = self.rx.try_recv() {
            self.apply(&ctx, action);
        }
        self.drain_sink();
        self.h.end_frame(&ctx);
        self.drain_sink();
    }
}

impl Proto {
    /// Detached windows that are OS windows or embedded windows. Tabs are drawn in the Main view.
    fn show_detached(&mut self, ctx: &Context) {
        let opts = self.o;
        let list = self.detached.clone();
        for det in list {
            let (id, builder, title) = {
                let d = det.lock().unwrap();
                (d.id, detached_builder(&d, opts), d.title.clone())
            };
            let vid = ViewportId::from_hash_of(("detached", id));
            match opts.mech {
                Mech::Deferred => {
                    let shared = Arc::clone(&self.shared);
                    ctx.show_viewport_deferred(vid, builder, move |ui, _| {
                        detached_viewport_ui(ui, &shared, &det);
                    });
                }
                Mech::Immediate => {
                    let shared = Arc::clone(&self.shared);
                    ctx.show_viewport_immediate(vid, builder, |ui, _| {
                        detached_viewport_ui(ui, &shared, &det);
                    });
                }
                Mech::Embedded => {
                    let mut open = true;
                    egui::Window::new(&title)
                        .id(Id::new(("detwin", id)))
                        .default_size([640.0, 420.0])
                        .open(&mut open)
                        .show(ctx, |ui| {
                            let mut d = det.lock().unwrap();
                            self.shared.frame_tick(ui.ctx(), &d.name());
                            detached_content(ui, &self.shared, &mut d);
                        });
                    if !open {
                        let ok = {
                            let mut d = det.lock().unwrap();
                            request_close(ctx, &self.shared, &mut d, "window close button")
                        };
                        if ok {
                            self.close_detached(id, "window close button");
                        }
                    }
                }
                Mech::Tab => {}
            }
        }
    }

    fn menus(&mut self, ui: &mut Ui, ctx: &Context) {
        let sel = self.selected;
        ui.menu_button("File", |ui| {
            if self.menu_item(ui, "File", "New Account…", "Ctrl+N") {
                self.open_wizard(ctx, None, "menu File");
            }
            if self.menu_item(ui, "File", "Manage Accounts…", "") {
                self.open_manage(ctx, None, "menu File");
            }
            ui.separator();
            if self.menu_item(ui, "File", "Exit", "") {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        });
        ui.menu_button("Account", |ui| {
            if self.menu_item(ui, "Account", "Open in Detached Window", "") {
                self.open_detached(ctx, sel, false, "menu Account");
            }
            if self.menu_item(ui, "Account", "Reconcile…", "") {
                self.open_detached(ctx, sel, true, "menu Account");
            }
            ui.separator();
            if self.menu_item(ui, "Account", "Delete Account…", "") {
                self.open_delete(ctx, sel, None, "menu Account");
            }
        });
        ui.menu_button("Window", |ui| {
            let list: Vec<(u64, String)> = self
                .detached
                .iter()
                .map(|d| {
                    let d = d.lock().unwrap();
                    (d.id, d.title.clone())
                })
                .collect();
            if list.is_empty() {
                ui.label(RichText::new("No Detached windows").weak());
            }
            for (id, title) in list {
                if self.menu_item(ui, "Window", &title, "") {
                    if self.o.mech == Mech::Tab {
                        self.nav = Nav::Det(id);
                    } else {
                        ctx.send_viewport_cmd_to(
                            ViewportId::from_hash_of(("detached", id)),
                            ViewportCommand::Focus,
                        );
                    }
                }
            }
            ui.separator();
            if self.menu_item(ui, "Window", "Close All Detached Windows", "") {
                self.apply_control(ctx, "close detached");
            }
        });
    }

    fn menu_item(&mut self, ui: &mut Ui, path: &str, label: &str, shortcut: &str) -> bool {
        let button = if shortcut.is_empty() {
            Button::new(label)
        } else {
            Button::new(label).shortcut_text(shortcut)
        };
        let r = ui.add(button);
        self.h.track(&r, format!("menu: {path} → {label}"));
        if r.clicked() {
            ui.close();
            return true;
        }
        false
    }

    fn main_ui(&mut self, ui: &mut Ui, ctx: &Context) {
        let blocked = !self.main_dialogs.is_empty() && self.o.style != DlgStyle::Window;

        Panel::top("menu").show(ui, |ui| {
            ui.add_enabled_ui(!blocked, |ui| {
                egui::MenuBar::new().ui(ui, |ui| self.menus(ui, ctx));
            });
        });
        Panel::bottom("status").show(ui, |ui| {
            Sides::new().show(
                ui,
                |ui| ui.label(&self.status),
                |ui| {
                    let stats = self.shared.stats.lock().unwrap();
                    let text: Vec<String> = stats
                        .iter()
                        .filter(|(k, _)| !k.starts_with("dialog#"))
                        .map(|(k, s)| format!("{k} {:.0} fps", s.fps))
                        .collect();
                    ui.label(RichText::new(text.join(" · ")).small().monospace());
                },
            );
        });
        Panel::left("accounts").default_size(210.0).show(ui, |ui| {
            ui.add_enabled_ui(!blocked, |ui| self.account_bar(ui, ctx));
        });
        CentralPanel::default().show(ui, |ui| {
            diag_strip(ui, &self.shared, "main");
            if self.o.mech == Mech::Tab {
                self.tab_strip(ui, ctx);
            }
            ui.add_enabled_ui(!blocked, |ui| match self.nav {
                Nav::Register => self.register_view(ui, ctx),
                Nav::Det(id) => {
                    if let Some(det) = self
                        .detached
                        .iter()
                        .find(|d| d.lock().unwrap().id == id)
                        .cloned()
                    {
                        let mut d = det.lock().unwrap();
                        detached_content(ui, &self.shared, &mut d);
                    }
                }
            });
        });
        // A scrim over the Main window is part of the OS-window Dialog style, drawn by show_dialogs.
        if blocked && self.o.style == DlgStyle::Modal {
            // egui::Modal supplies its own backdrop; nothing to add.
        }
    }

    fn tab_strip(&mut self, ui: &mut Ui, ctx: &Context) {
        let tabs: Vec<(u64, String)> = self
            .detached
            .iter()
            .map(|d| {
                let d = d.lock().unwrap();
                (d.id, d.title.clone())
            })
            .collect();
        ui.horizontal(|ui| {
            let r = ui.selectable_label(
                self.nav == Nav::Register,
                format!("Register — {}", self.accounts[self.selected].name),
            );
            self.h.track(&r, "tab: Register");
            if r.clicked() {
                self.nav = Nav::Register;
            }
            for (id, title) in tabs {
                let r = ui.selectable_label(self.nav == Nav::Det(id), &title);
                self.h.track(&r, format!("tab: {title}"));
                if r.clicked() {
                    self.nav = Nav::Det(id);
                }
                let x = ui.small_button("✕");
                self.h.track(&x, format!("tab close: {title}"));
                if x.clicked()
                    && let Some(det) = self
                        .detached
                        .iter()
                        .find(|d| d.lock().unwrap().id == id)
                        .cloned()
                {
                    let ok = request_close(
                        ctx,
                        &self.shared,
                        &mut det.lock().unwrap(),
                        "tab close button",
                    );
                    if ok {
                        self.close_detached(id, "tab close button");
                    }
                }
            }
        });
        ui.separator();
    }

    fn account_bar(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.heading("Accounts");
        let mut open_detached = None;
        let mut reconcile = None;
        let mut delete = None;
        let mut select = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for group in Group::ALL {
                let members: Vec<usize> = self
                    .accounts
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| a.group == group && !a.hidden)
                    .map(|(i, _)| i)
                    .collect();
                if members.is_empty() {
                    continue;
                }
                ui.label(RichText::new(group.name()).small().strong());
                for i in members {
                    let a = &self.accounts[i];
                    let text = if a.closed {
                        RichText::new(format!("{} (closed)", a.name)).weak()
                    } else {
                        RichText::new(a.name)
                    };
                    let r = ui.add_sized(
                        [ui.available_width(), 22.0],
                        egui::Button::selectable(
                            self.selected == i && self.nav == Nav::Register,
                            text,
                        ),
                    );
                    self.h.track(&r, format!("account bar: {}", a.name));
                    if r.clicked() {
                        select = Some(i);
                    }
                    r.context_menu(|ui| {
                        if self.menu_item(ui, "Account bar context", "Open in Detached Window", "")
                        {
                            open_detached = Some(i);
                        }
                        if self.menu_item(ui, "Account bar context", "Reconcile…", "") {
                            reconcile = Some(i);
                        }
                        if self.menu_item(ui, "Account bar context", "Delete Account…", "") {
                            delete = Some(i);
                        }
                    });
                }
            }
        });
        if let Some(i) = select {
            self.selected = i;
            self.selected_row = None;
            self.nav = Nav::Register;
            self.h.event(
                "navigate",
                json!({ "to": self.accounts[i].name, "via": "click" }),
            );
        }
        if let Some(i) = open_detached {
            self.selected = i;
            self.open_detached(ctx, i, false, "account context menu");
        }
        if let Some(i) = reconcile {
            self.selected = i;
            self.open_detached(ctx, i, true, "account context menu");
        }
        if let Some(i) = delete {
            self.selected = i;
            self.open_delete(ctx, i, None, "account context menu");
        }
    }

    fn register_view(&mut self, ui: &mut Ui, ctx: &Context) {
        if self.accounts.is_empty() {
            ui.label("No accounts. Use File ▸ New Account…");
            return;
        }
        self.selected = self.selected.min(self.accounts.len() - 1);
        let sel = self.selected;
        ui.horizontal(|ui| {
            ui.heading(self.accounts[sel].name);
            ui.label(RichText::new(fmt_cents(self.accounts[sel].balance)).strong());
        });
        ui.horizontal(|ui| {
            let r = ui.button("New Account…");
            self.h.track(&r, "toolbar: New Account…");
            if r.clicked() {
                self.open_wizard(ctx, Some(r.id), "toolbar");
            }
            let r = ui.button("Manage Accounts…");
            self.h.track(&r, "toolbar: Manage Accounts…");
            if r.clicked() {
                self.open_manage(ctx, Some(r.id), "toolbar");
            }
            let r = ui.button("Delete Account…");
            self.h.track(&r, "toolbar: Delete Account…");
            if r.clicked() {
                self.open_delete(ctx, sel, Some(r.id), "toolbar");
            }
            ui.separator();
            let r = ui.button("Reconcile…");
            self.h.track(&r, "toolbar: Reconcile…");
            if r.clicked() {
                self.open_detached(ctx, sel, true, "toolbar");
            }
            let r = ui.button("Open in Detached Window");
            self.h.track(&r, "toolbar: Open in Detached Window");
            if r.clicked() {
                self.open_detached(ctx, sel, false, "toolbar");
            }
        });
        ui.separator();
        let txns = Arc::clone(&self.accounts[sel].txns);
        let shared = Arc::clone(&self.shared);
        register_table(
            ui,
            &txns,
            &format!("main {sel}"),
            &mut self.selected_row,
            &shared,
        );
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
            .exact_size(300.0)
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
                        self.set_variant(v, "click");
                    }
                }
                ui.separator();
                ui.label(RichText::new("Toggles").strong());
                let current = self.variant.key().to_uppercase();
                for (&(key, label, applies), (_, hint)) in TOGGLES.iter().zip(TOGGLE_KEYS) {
                    let relevant = applies == "all" || applies.split(' ').any(|a| a == current);
                    let mut value = *self.o.get_mut(key).unwrap();
                    let text = RichText::new(format!("{hint}   [{applies}] {label}"));
                    let r = ui.checkbox(&mut value, if relevant { text } else { text.weak() });
                    self.h.track(&r, format!("dock: toggle {key}"));
                    if r.changed() {
                        self.set_toggle(key, Some(value), "click");
                    }
                }
                ui.separator();
                ui.label(RichText::new("Windows").strong());
                {
                    let stats = self.shared.stats.lock().unwrap();
                    for (name, s) in stats.iter() {
                        let idle = s.last.elapsed().as_secs_f32();
                        let flags = format!(
                            "{}{}",
                            if s.focused == Some(true) { " focus" } else { "" },
                            if s.occluded == Some(true) { " OCCLUDED" } else { "" }
                        );
                        let text = format!("{name}: {:.0} fps, idle {idle:.1}s{flags}", s.fps);
                        ui.label(RichText::new(text).small().monospace());
                    }
                }
                ui.separator();
                ui.label(
                    RichText::new(
                        "A–E or ◀ / ▶ variant · 1–9 toggles · Ctrl+N New Account · F2 note · F3 marker · F4 hide this dock",
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
            .with_title("Balanced — PROTOTYPE dialogs and detached windows")
            .with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "balanced_dialogs_detached_prototype",
        options,
        Box::new(move |_cc| Ok(Box::new(Proto::new(&args)))),
    )
}
