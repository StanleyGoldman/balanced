//! PROTOTYPE, throwaway: three radically different layouts of the Main window shell.
//!
//! Question: how should the shell look and behave (Menu bar, toolbar, Account bar, top-level
//! tabs, Main view, Status bar, right-click menus)? Which layout, menu contents and tab set?
//!
//! Run:
//!   cargo run -p balanced_ui --example shell_layout_prototype -- \
//!     [--variant a|b|c] [--log <dir>/events.jsonl] [--control <dir>/control.txt]
//!
//! The PROTOTYPE dock on the right holds the variant switcher and toggles; F4 collapses it.
//! ← / → switch variant, F2 writes a note into the log, F3 drops a marker.
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
    missing_debug_implementations,
    unreachable_pub
)]

use std::collections::HashMap;

use balanced_prototype_harness::{Args, Harness};
use eframe::egui;
use egui::{Align, Button, CentralPanel, Id, Layout, Panel, RichText, Sense, Sides, Ui};
use egui_extras::{Column, TableBuilder};
use serde_json::json;

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
    txns: Vec<Txn>,
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
                txns,
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

/// Quicken 2012-style top-level tabs. Charts/reports are out of scope, so these are placeholders.
const TABS: &[&str] = &[
    "Home",
    "Bills",
    "Spending",
    "Planning",
    "Investing",
    "Property & Debt",
];

// ---------------------------------------------------------------------------------------------
// Variants and toggles

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Tab(usize),
    Account(usize),
}

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
            Self::A => "A · Classic Quicken (toolbar + tab strip)",
            Self::B => "B · Compact (context header, rail)",
            Self::C => "C · Workspace (nav in Account bar, register tabs)",
        }
    }

    fn step(self, delta: isize) -> Self {
        let n = Self::ALL.len() as isize;
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0) as isize;
        Self::ALL[(i + delta).rem_euclid(n) as usize]
    }
}

/// (key, label, applies to)
const TOGGLES: &[(&str, &str, &str)] = &[
    ("dark", "Dark theme", "all"),
    ("status_bar", "Status bar", "all"),
    ("account_bar", "Account bar", "all"),
    ("dense", "Dense rows", "all"),
    ("group_totals", "Group totals in Account bar", "all"),
    (
        "net_worth_footer",
        "Net Worth in Account bar footer (off: Status bar)",
        "all",
    ),
    ("toolbar", "Toolbar", "A"),
    ("toolbar_icons_only", "Toolbar icons only", "A"),
    ("tabs_in_menu_row", "Tabs in menu row (off: own row)", "B"),
    ("register_tabs", "Register tabs (off: one register)", "C"),
];

struct Toggles {
    dark: bool,
    status_bar: bool,
    account_bar: bool,
    dense: bool,
    group_totals: bool,
    net_worth_footer: bool,
    toolbar: bool,
    toolbar_icons_only: bool,
    tabs_in_menu_row: bool,
    register_tabs: bool,
}

impl Toggles {
    fn get_mut(&mut self, key: &str) -> Option<&mut bool> {
        Some(match key {
            "dark" => &mut self.dark,
            "status_bar" => &mut self.status_bar,
            "account_bar" => &mut self.account_bar,
            "dense" => &mut self.dense,
            "group_totals" => &mut self.group_totals,
            "net_worth_footer" => &mut self.net_worth_footer,
            "toolbar" => &mut self.toolbar,
            "toolbar_icons_only" => &mut self.toolbar_icons_only,
            "tabs_in_menu_row" => &mut self.tabs_in_menu_row,
            "register_tabs" => &mut self.register_tabs,
            _ => return None,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// App state

struct Proto {
    h: Harness,
    accounts: Vec<Account>,
    variant: Variant,
    t: Toggles,
    nav: Nav,
    collapsed: [bool; 4],
    filter: String,
    /// Variant C: open register tabs (account indices), in order.
    open_registers: Vec<usize>,
    /// Selected Register row per account; shared across variants.
    selected_row: HashMap<usize, usize>,
    jump_to_row: Option<usize>,
    /// The docked prototype controls (F4).
    dock_open: bool,
    /// Last action, shown in the Status bar so every click is visible.
    status: String,
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
        Self {
            h,
            accounts: generate(),
            variant,
            t: Toggles {
                dark: false,
                status_bar: true,
                account_bar: true,
                dense: false,
                group_totals: true,
                net_worth_footer: true,
                toolbar: true,
                toolbar_icons_only: false,
                tabs_in_menu_row: true,
                register_tabs: true,
            },
            nav: Nav::Account(0),
            collapsed: [false; 4],
            filter: String::new(),
            open_registers: vec![0, 8],
            selected_row: HashMap::new(),
            jump_to_row: None,
            dock_open: true,
            status: "Ready".to_owned(),
        }
    }

    fn did(&mut self, what: impl Into<String>) {
        self.status = what.into();
    }

    fn emit(&mut self, command: &str, source: &str) {
        self.h.event(
            "command",
            json!({ "would_emit": command, "source": source }),
        );
        self.did(format!("{source} → {command}"));
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
        let Some(slot) = self.t.get_mut(key) else {
            self.h.event("control_unknown", json!({ "toggle": key }));
            return;
        };
        let new = value.unwrap_or(!*slot);
        if *slot == new && via != "click" && via != "menu" {
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

    fn open_account(&mut self, i: usize, via: &str) {
        self.nav = Nav::Account(i);
        if !self.open_registers.contains(&i) {
            self.open_registers.push(i);
        }
        self.h.event(
            "navigate",
            json!({ "to": self.accounts[i].name, "kind": "register", "via": via }),
        );
        self.did(format!("Opened register: {}", self.accounts[i].name));
    }

    fn open_tab(&mut self, t: usize, via: &str) {
        self.nav = Nav::Tab(t);
        self.h.event(
            "navigate",
            json!({ "to": TABS[t], "kind": "tab", "via": via }),
        );
        self.did(format!("Tab → {}", TABS[t]));
    }

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
            "account" => {
                let found = rest
                    .parse::<usize>()
                    .ok()
                    .filter(|i| *i < self.accounts.len())
                    .or_else(|| {
                        let needle = rest.to_lowercase();
                        self.accounts
                            .iter()
                            .position(|a| a.name.to_lowercase().contains(&needle))
                    });
                match found {
                    Some(i) => self.open_account(i, "control"),
                    None => self.h.event("control_unknown", json!({ "command": cmd })),
                }
            }
            "tab" => {
                let found = rest
                    .parse::<usize>()
                    .ok()
                    .filter(|i| *i < TABS.len())
                    .or_else(|| {
                        let needle = rest.to_lowercase();
                        TABS.iter().position(|t| t.to_lowercase().contains(&needle))
                    });
                match found {
                    Some(t) => self.open_tab(t, "control"),
                    None => self.h.event("control_unknown", json!({ "command": cmd })),
                }
            }
            "row" => match (rest.parse::<usize>(), self.nav) {
                (Ok(r), Nav::Account(i)) => {
                    self.jump_to_row = Some(r);
                    self.selected_row.insert(i, r);
                }
                _ => self.h.event("control_unknown", json!({ "command": cmd })),
            },
            "reload" => {
                self.accounts = generate();
                self.h.event("reload", json!({ "seed": SEED }));
            }
            "screenshot" => self
                .h
                .screenshot(if rest.is_empty() { "control" } else { rest }),
            "dock" => self.set_dock(rest != "off", "control"),
            "quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            _ => self.h.event("control_unknown", json!({ "command": cmd })),
        }
    }

    fn group_total(&self, group: Group) -> i64 {
        self.accounts
            .iter()
            .filter(|a| a.group == group)
            .map(|a| a.balance)
            .sum()
    }

    fn net_worth(&self) -> i64 {
        self.accounts.iter().map(|a| a.balance).sum()
    }

    fn nav_title(&self) -> &'static str {
        match self.nav {
            Nav::Account(i) => self.accounts[i].name,
            Nav::Tab(t) => TABS[t],
        }
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
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                self.set_variant(self.variant.step(-1), "key");
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                self.set_variant(self.variant.step(1), "key");
            }
        }
        ctx.set_theme(if self.t.dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });

        if ctx.input(|i| i.key_pressed(egui::Key::F4)) {
            self.set_dock(!self.dock_open, "key");
        }
        self.dock(ui);
        match self.variant {
            Variant::A => self.variant_a(ui),
            Variant::B => self.variant_b(ui),
            Variant::C => self.variant_c(ui),
        }

        self.h.end_frame(&ctx);
    }
}

// ---------------------------------------------------------------------------------------------
// Shared pieces (the menus are part of the question, so all variants share one proposal)

fn command_name(label: &str) -> String {
    label
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect()
}

impl Proto {
    fn item(&mut self, ui: &mut Ui, path: &str, label: &str, shortcut: &str, kind: &str) {
        let button = if shortcut.is_empty() {
            Button::new(label)
        } else {
            Button::new(label).shortcut_text(shortcut)
        };
        let r = ui.add(button);
        self.h.track(&r, format!("menu: {path} → {label}"));
        if r.clicked() {
            self.emit(
                &format!("{kind}::{}", command_name(label)),
                &format!("menu {path}"),
            );
            ui.close();
        }
    }

    fn menu_toggle(&mut self, ui: &mut Ui, key: &str, label: &str) {
        let mut v = *self.t.get_mut(key).unwrap();
        let r = ui.checkbox(&mut v, label);
        self.h.track(&r, format!("menu: View → {label}"));
        if r.changed() {
            self.set_toggle(key, Some(v), "menu");
        }
    }

    fn menus(&mut self, ui: &mut Ui) {
        let app = "AppCommand";
        let dom = "DomainCommand";
        ui.menu_button("File", |ui| {
            self.item(ui, "File", "New Data File…", "", app);
            self.item(ui, "File", "Open Data File…", "Ctrl+O", app);
            ui.menu_button("Open Recent", |ui| {
                self.item(ui, "File ▸ Open Recent", "household.balanced", "", app);
            });
            ui.separator();
            self.item(ui, "File", "Back Up…", "Ctrl+B", app);
            self.item(ui, "File", "Restore Backup…", "", app);
            ui.separator();
            ui.menu_button("Import", |ui| {
                self.item(ui, "File ▸ Import", "QIF File…", "", app);
                self.item(ui, "File ▸ Import", "OFX / QFX File…", "", app);
                self.item(ui, "File ▸ Import", "CSV File…", "", app);
            });
            ui.menu_button("Export", |ui| {
                self.item(ui, "File ▸ Export", "QIF File…", "", app);
                self.item(ui, "File ▸ Export", "CSV File…", "", app);
            });
            ui.separator();
            self.item(ui, "File", "Print Register…", "Ctrl+P", app);
            ui.separator();
            self.item(ui, "File", "Exit", "Alt+F4", app);
        });
        ui.menu_button("Edit", |ui| {
            self.item(ui, "Edit", "Undo", "Ctrl+Z", app);
            self.item(ui, "Edit", "Redo", "Ctrl+Y", app);
            ui.separator();
            self.item(ui, "Edit", "Cut", "Ctrl+X", app);
            self.item(ui, "Edit", "Copy", "Ctrl+C", app);
            self.item(ui, "Edit", "Paste", "Ctrl+V", app);
            ui.separator();
            ui.menu_button("Transaction", |ui| {
                self.item(ui, "Edit ▸ Transaction", "New", "Ctrl+N", app);
                self.item(ui, "Edit ▸ Transaction", "Delete", "Ctrl+D", dom);
                self.item(ui, "Edit ▸ Transaction", "Split…", "Ctrl+S", app);
                self.item(ui, "Edit ▸ Transaction", "Mark Cleared", "Ctrl+M", dom);
            });
            ui.separator();
            self.item(ui, "Edit", "Find…", "Ctrl+F", app);
            self.item(ui, "Edit", "Find and Replace…", "Ctrl+H", app);
        });
        ui.menu_button("View", |ui| {
            self.menu_toggle(ui, "account_bar", "Account Bar");
            self.menu_toggle(ui, "toolbar", "Toolbar");
            self.menu_toggle(ui, "status_bar", "Status Bar");
            ui.separator();
            ui.menu_button("Go To", |ui| {
                for (t, tab) in TABS.iter().enumerate() {
                    let r = ui.button(*tab);
                    self.h.track(&r, format!("menu: View ▸ Go To → {tab}"));
                    if r.clicked() {
                        self.open_tab(t, "menu");
                        ui.close();
                    }
                }
            });
            self.item(ui, "View", "Back", "Alt+←", app);
            self.item(ui, "View", "Forward", "Alt+→", app);
            ui.separator();
            self.menu_toggle(ui, "dark", "Dark Theme");
            self.item(ui, "View", "Zoom In", "Ctrl++", app);
            self.item(ui, "View", "Zoom Out", "Ctrl+-", app);
        });
        ui.menu_button("Tools", |ui| {
            self.item(ui, "Tools", "Account List…", "Ctrl+A", app);
            self.item(ui, "Tools", "Category List…", "Ctrl+Shift+C", app);
            self.item(ui, "Tools", "Payee List…", "", app);
            self.item(ui, "Tools", "Scheduled Transactions…", "Ctrl+J", app);
            ui.separator();
            self.item(ui, "Tools", "Reconcile Account…", "Ctrl+R", app);
            self.item(ui, "Tools", "Calculator", "", app);
            ui.separator();
            self.item(ui, "Tools", "Preferences…", "Ctrl+,", app);
        });
        ui.menu_button("Help", |ui| {
            self.item(ui, "Help", "Balanced Help", "F1", app);
            self.item(ui, "Help", "Keyboard Shortcuts", "", app);
            ui.separator();
            self.item(ui, "Help", "About Balanced", "", app);
        });
    }

    fn account_context_menu(&mut self, ui: &mut Ui, i: usize) {
        let name = self.accounts[i].name;
        let path = format!("account menu ({name})");
        for (label, cmd) in [
            ("Open Register", "AppCommand::OpenRegister"),
            (
                "Open in Detached Window",
                "AppCommand::OpenDetachedRegister",
            ),
            ("Reconcile…", "AppCommand::OpenReconcileDialog"),
            ("Edit Account Details…", "AppCommand::OpenEditAccountDialog"),
            ("Hide from Account Bar", "DomainCommand::HideAccount"),
            ("Delete Account…", "AppCommand::OpenDeleteAccountDialog"),
        ] {
            if label == "Delete Account…" {
                ui.separator();
            }
            let r = ui.button(label);
            self.h.track(&r, format!("{path} → {label}"));
            if r.clicked() {
                if label == "Open Register" {
                    self.open_account(i, "context menu");
                }
                self.emit(&format!("{cmd} {{ account: {name:?} }}"), &path);
                ui.close();
            }
        }
    }

    fn money_label(ui: &mut Ui, cents: i64) -> egui::Response {
        let rt = RichText::new(fmt_cents(cents));
        if cents < 0 {
            let c = ui.visuals().error_fg_color;
            ui.label(rt.color(c))
        } else {
            ui.label(rt)
        }
    }

    /// The Account bar body: groups with totals, accounts with balances, right-click menus.
    fn account_groups(&mut self, ui: &mut Ui) {
        let gap = if self.t.dense { 2.0 } else { 8.0 };
        if self.t.dense {
            ui.spacing_mut().item_spacing.y = 1.0;
        }
        let needle = self.filter.to_lowercase();
        for (g, group) in Group::ALL.into_iter().enumerate() {
            let total = self.group_total(group);
            let show_total = self.t.group_totals;
            let arrow = if self.collapsed[g] { "⏵" } else { "⏷" };
            let (header, ()) = Sides::new().show(
                ui,
                |ui| {
                    ui.add(
                        Button::new(RichText::new(format!("{arrow} {}", group.name())).strong())
                            .frame(false),
                    )
                },
                |ui| {
                    if show_total {
                        Self::money_label(ui, total);
                    }
                },
            );
            self.h
                .track(&header, format!("account group: {}", group.name()));
            if header.clicked() {
                self.collapsed[g] = !self.collapsed[g];
                self.h.event(
                    "group_collapse",
                    json!({ "group": group.name(), "collapsed": self.collapsed[g] }),
                );
            }
            header.context_menu(|ui| {
                for label in [
                    "Add Account to Group…",
                    "Collapse All Groups",
                    "Expand All Groups",
                ] {
                    let r = ui.button(label);
                    self.h
                        .track(&r, format!("group menu ({}) → {label}", group.name()));
                    if r.clicked() {
                        match label {
                            "Collapse All Groups" => self.collapsed = [true; 4],
                            "Expand All Groups" => self.collapsed = [false; 4],
                            _ => self.emit("AppCommand::OpenNewAccountDialog", "group menu"),
                        }
                        ui.close();
                    }
                }
            });
            if self.collapsed[g] {
                ui.add_space(gap);
                continue;
            }
            let rows: Vec<(usize, &'static str, i64)> = self
                .accounts
                .iter()
                .enumerate()
                .filter(|(_, a)| {
                    a.group == group
                        && (needle.is_empty() || a.name.to_lowercase().contains(&needle))
                })
                .map(|(i, a)| (i, a.name, a.balance))
                .collect();
            for (i, name, balance) in rows {
                let selected = self.nav == Nav::Account(i);
                let (row, _) = Sides::new().show(
                    ui,
                    |ui| {
                        ui.add_space(14.0);
                        ui.selectable_label(selected, name)
                    },
                    |ui| Self::money_label(ui, balance),
                );
                self.h.track(&row, format!("account bar: {name}"));
                if row.clicked() {
                    self.open_account(i, "account bar");
                }
                row.context_menu(|ui| self.account_context_menu(ui, i));
            }
            ui.add_space(gap);
        }
    }

    fn net_worth_row(&mut self, ui: &mut Ui) {
        let nw = self.net_worth();
        Sides::new().show(
            ui,
            |ui| ui.label(RichText::new("Net Worth").strong()),
            |ui| ui.label(RichText::new(fmt_cents(nw)).strong()),
        );
        let r = ui.button("➕ Add Account…");
        self.h.track(&r, "account bar footer: Add Account");
        if r.clicked() {
            self.emit("AppCommand::OpenNewAccountDialog", "account bar footer");
        }
    }

    /// The Account bar as a resizable left panel, with the optional Net Worth footer.
    fn account_bar(&mut self, ui: &mut Ui, id: &str, top: impl FnOnce(&mut Self, &mut Ui)) {
        Panel::left(Id::new(id))
            .resizable(true)
            .default_size(270.0)
            .min_size(180.0)
            .show(ui, |ui| {
                if self.t.net_worth_footer {
                    Panel::bottom(Id::new((id, "footer"))).show(ui, |ui| self.net_worth_row(ui));
                }
                CentralPanel::default_margins().show(ui, |ui| {
                    top(self, ui);
                    egui::ScrollArea::vertical().show(ui, |ui| self.account_groups(ui));
                });
            });
    }

    fn tab_strip(&mut self, ui: &mut Ui, size: f32) {
        for (t, tab) in TABS.iter().enumerate() {
            let r = ui.selectable_label(self.nav == Nav::Tab(t), RichText::new(*tab).size(size));
            self.h.track(&r, format!("tab: {tab}"));
            if r.clicked() {
                self.open_tab(t, "tab");
            }
        }
    }

    fn status_bar(&self, ui: &mut Ui) {
        if !self.t.status_bar {
            return;
        }
        let right = if self.t.net_worth_footer {
            format!("{} accounts", self.accounts.len())
        } else {
            format!(
                "Net Worth {}   ·   {} accounts",
                fmt_cents(self.net_worth()),
                self.accounts.len()
            )
        };
        let status = self.status.clone();
        Panel::bottom("status_bar").show(ui, |ui| {
            Sides::new().show(
                ui,
                |ui| ui.label(status),
                |ui| ui.label(RichText::new(right).weak()),
            );
        });
    }

    /// What the Main view shows for the current navigation.
    fn main_view_body(&mut self, ui: &mut Ui) {
        match self.nav {
            Nav::Account(i) => self.register(ui, i),
            Nav::Tab(t) => {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.heading(TABS[t]);
                    ui.label(
                        RichText::new("Placeholder: the tab's own view is a later feature map.")
                            .weak(),
                    );
                });
            }
        }
    }

    /// A rough Register stub; the real grid is "Prototype the Register grid".
    fn register(&mut self, ui: &mut Ui, account: usize) {
        let row_h = if self.t.dense { 18.0 } else { 24.0 };
        let n = self.accounts[account].txns.len();
        let selected = self.selected_row.get(&account).copied();
        let mut builder = TableBuilder::new(ui)
            .id_salt(("register", account))
            .striped(true)
            .resizable(true)
            .sense(Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(88.0))
            .column(Column::exact(48.0))
            .column(Column::initial(200.0).at_least(80.0))
            .column(Column::initial(180.0).at_least(80.0))
            .column(Column::exact(24.0))
            .column(Column::initial(96.0))
            .column(Column::initial(96.0))
            .column(Column::remainder().at_least(96.0));
        if let Some(r) = self.jump_to_row.take() {
            builder = builder.scroll_to_row(r.min(n.saturating_sub(1)), Some(Align::Center));
        }
        let mut clicked: Option<(usize, &'static str)> = None;
        let mut names: Vec<(egui::Response, String)> = Vec::new();
        let out = builder
            .header(22.0, |mut h| {
                for title in [
                    "Date", "Num", "Payee", "Category", "Clr", "Payment", "Deposit", "Balance",
                ] {
                    h.col(|ui| {
                        ui.strong(title);
                    });
                }
            })
            .body(|body| {
                let txns = &self.accounts[account].txns;
                body.rows(row_h, n, |mut row| {
                    let r = row.index();
                    let t = &txns[r];
                    row.set_selected(selected == Some(r));
                    let cells: [(&'static str, String, bool); 8] = [
                        ("date", fmt_day(t.day), false),
                        (
                            "num",
                            t.num.map(|n| n.to_string()).unwrap_or_default(),
                            false,
                        ),
                        ("payee", t.payee.to_owned(), false),
                        ("category", t.category.to_owned(), false),
                        (
                            "cleared",
                            if t.cleared {
                                "c".to_owned()
                            } else {
                                String::new()
                            },
                            false,
                        ),
                        (
                            "payment",
                            if t.amount < 0 {
                                fmt_cents(-t.amount)
                            } else {
                                String::new()
                            },
                            false,
                        ),
                        (
                            "deposit",
                            if t.amount >= 0 {
                                fmt_cents(t.amount)
                            } else {
                                String::new()
                            },
                            false,
                        ),
                        ("balance", fmt_cents(t.balance), t.balance < 0),
                    ];
                    for (col, text, negative) in cells {
                        let (_, resp) = row.col(|ui| {
                            if negative {
                                let c = ui.visuals().error_fg_color;
                                ui.label(RichText::new(&text).color(c));
                            } else {
                                ui.label(&text);
                            }
                        });
                        if resp.clicked() {
                            clicked = Some((r, col));
                        }
                        names.push((resp, format!("{col} cell, row {r}")));
                    }
                    row.response().context_menu(|ui| {
                        for label in [
                            "Edit",
                            "Split…",
                            "Mark Cleared",
                            "Duplicate",
                            "Go to Transfer",
                            "Delete",
                        ] {
                            if label == "Delete" {
                                ui.separator();
                            }
                            if ui.button(label).clicked() {
                                clicked = Some((r, label));
                                ui.close();
                            }
                        }
                    });
                });
            });
        for (resp, name) in &names {
            self.h.track(resp, name.clone());
        }
        self.h.scroll("register", out.state.offset.y, row_h + 3.0);
        if let Some((r, what)) = clicked {
            self.selected_row.insert(account, r);
            if what.chars().next().is_some_and(char::is_uppercase) {
                let cmd = format!(
                    "{}::{} {{ row: {r} }}",
                    if what == "Delete" || what == "Mark Cleared" {
                        "DomainCommand"
                    } else {
                        "AppCommand"
                    },
                    command_name(what)
                );
                self.emit(&cmd, "register row menu");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Variant A: Classic Quicken. Menu bar, icon toolbar, a full-width tab strip, Account bar on
// the left, Status bar.

impl Proto {
    fn variant_a(&mut self, ui: &mut Ui) {
        Panel::top("a_menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| self.menus(ui));
        });
        if self.t.toolbar {
            let icons_only = self.t.toolbar_icons_only;
            Panel::top("a_toolbar").show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (icon, label) in [
                        ("◀", "Back"),
                        ("▶", "Forward"),
                        ("➕", "Add Account"),
                        ("🔍", "Find"),
                        ("✔", "Reconcile"),
                        ("🖶", "Print"),
                        ("⚙", "Preferences"),
                    ] {
                        let text = if icons_only {
                            icon.to_owned()
                        } else {
                            format!("{icon} {label}")
                        };
                        let r = ui.button(text).on_hover_text(label);
                        self.h.track(&r, format!("toolbar: {label}"));
                        if r.clicked() {
                            self.emit(&format!("AppCommand::{}", command_name(label)), "toolbar");
                        }
                    }
                });
            });
        }
        Panel::top("a_tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                self.tab_strip(ui, 15.0);
            });
        });
        self.status_bar(ui);
        if self.t.account_bar {
            self.account_bar(ui, "a_account_bar", |_, _| {});
        }
        CentralPanel::default().show(ui, |ui| {
            if let Nav::Account(i) = self.nav {
                let (name, balance, n) = (
                    self.accounts[i].name,
                    self.accounts[i].balance,
                    self.accounts[i].txns.len(),
                );
                Sides::new().show(
                    ui,
                    |ui| {
                        ui.heading(name);
                        ui.label(RichText::new(format!("{n} transactions")).weak());
                    },
                    |ui| {
                        Self::money_label(ui, balance);
                        ui.label("Ending balance:");
                    },
                );
                ui.separator();
            }
            self.main_view_body(ui);
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Variant B: Compact. No global toolbar. Tabs share the menu row (toggle: own row). The Main
// view gets a context header whose actions depend on what is shown. The Account bar collapses
// to a thin rail.

impl Proto {
    fn variant_b(&mut self, ui: &mut Ui) {
        let tabs_in_menu = self.t.tabs_in_menu_row;
        Panel::top("b_menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                self.menus(ui);
                if tabs_in_menu {
                    ui.separator();
                    self.tab_strip(ui, 14.0);
                }
            });
        });
        if !tabs_in_menu {
            Panel::top("b_tabs").show(ui, |ui| {
                ui.horizontal(|ui| self.tab_strip(ui, 14.0));
            });
        }
        self.status_bar(ui);

        if self.t.account_bar {
            self.account_bar(ui, "b_account_bar", |this, ui| {
                Sides::new().show(
                    ui,
                    |ui| ui.label(RichText::new("ACCOUNTS").small().weak()),
                    |ui| {
                        let r = ui.small_button("«").on_hover_text("Collapse Account bar");
                        this.h.track(&r, "account bar: collapse «");
                        if r.clicked() {
                            this.set_toggle("account_bar", Some(false), "click");
                        }
                    },
                );
            });
        } else {
            Panel::left("b_rail")
                .exact_size(28.0)
                .resizable(false)
                .show(ui, |ui| {
                    let r = ui.small_button("»").on_hover_text("Show Account bar");
                    self.h.track(&r, "account bar rail: expand »");
                    if r.clicked() {
                        self.set_toggle("account_bar", Some(true), "click");
                    }
                });
        }

        CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(self.nav_title());
                if let Nav::Account(i) = self.nav {
                    Self::money_label(ui, self.accounts[i].balance);
                    ui.label(
                        RichText::new(format!("{} transactions", self.accounts[i].txns.len()))
                            .weak(),
                    );
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let actions: &[&str] = match self.nav {
                        Nav::Account(_) => &["More…", "Reconcile", "Filter", "+ Transaction"],
                        Nav::Tab(_) => &["More…", "Customize"],
                    };
                    for label in actions {
                        let r = ui.button(*label);
                        self.h.track(&r, format!("context header: {label}"));
                        if r.clicked() {
                            self.emit(
                                &format!("AppCommand::{}", command_name(label)),
                                "context header",
                            );
                        }
                    }
                });
            });
            ui.separator();
            self.main_view_body(ui);
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Variant C: Workspace. The top-level "tabs" become a nav list at the top of the Account bar.
// The Main view holds closeable register tabs (like browser tabs) for each opened account.
// No toolbar; actions live in menus and right-click menus.

impl Proto {
    fn variant_c(&mut self, ui: &mut Ui) {
        Panel::top("c_menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| self.menus(ui));
        });
        self.status_bar(ui);
        if self.t.account_bar {
            self.account_bar(ui, "c_account_bar", |this, ui| {
                ui.vertical(|ui| this.tab_strip(ui, 14.0));
                ui.separator();
                let r = ui.add(
                    egui::TextEdit::singleline(&mut this.filter).hint_text("Filter accounts…"),
                );
                this.h.track(&r, "account filter");
                if r.changed() {
                    this.h.event("filter", json!({ "text": this.filter }));
                }
                ui.add_space(4.0);
            });
        }
        CentralPanel::default().show(ui, |ui| {
            if self.t.register_tabs {
                ui.horizontal(|ui| {
                    let mut close = None;
                    for (pos, &i) in self.open_registers.clone().iter().enumerate() {
                        let name = self.accounts[i].name;
                        let r = ui.selectable_label(self.nav == Nav::Account(i), name);
                        self.h.track(&r, format!("register tab: {name}"));
                        if r.clicked() {
                            self.open_account(i, "register tab");
                        }
                        let x = ui.small_button("×");
                        self.h.track(&x, format!("register tab close: {name}"));
                        if r.middle_clicked() || x.clicked() {
                            close = Some(pos);
                        }
                        ui.separator();
                    }
                    if let Some(pos) = close {
                        let i = self.open_registers.remove(pos);
                        self.h.event(
                            "register_tab_closed",
                            json!({ "account": self.accounts[i].name }),
                        );
                        if self.nav == Nav::Account(i) {
                            self.nav = self
                                .open_registers
                                .last()
                                .map_or(Nav::Tab(0), |&j| Nav::Account(j));
                        }
                    }
                });
            } else {
                ui.heading(self.nav_title());
            }
            ui.separator();
            self.main_view_body(ui);
        });
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
            .exact_size(270.0)
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
                for v in Variant::ALL {
                    let r = ui.selectable_label(self.variant == v, v.label());
                    self.h.track(&r, format!("dock: variant {}", v.key()));
                    if r.clicked() {
                        self.set_variant(v, "click");
                    }
                }
                ui.separator();
                ui.label(RichText::new("Toggles").strong());
                let current = self.variant.key().to_uppercase();
                for &(key, label, applies) in TOGGLES {
                    let relevant = applies == "all" || applies == current;
                    let mut value = *self.t.get_mut(key).unwrap();
                    let text = RichText::new(format!("[{applies}] {label}"));
                    let r = ui.checkbox(&mut value, if relevant { text } else { text.weak() });
                    self.h.track(&r, format!("dock: toggle {key}"));
                    if r.changed() {
                        self.set_toggle(key, Some(value), "click");
                    }
                }
                ui.separator();
                ui.label(
                    RichText::new("◀ / ▶ variant · F2 note · F3 marker · F4 hide this dock")
                        .small()
                        .weak(),
                );
            });
    }
}

fn main() -> eframe::Result {
    let args = Args::from_env();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Balanced — PROTOTYPE shell layout")
            .with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "balanced_shell_layout_prototype",
        options,
        Box::new(move |_cc| Ok(Box::new(Proto::new(&args)))),
    )
}
