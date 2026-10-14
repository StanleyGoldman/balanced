//! PROTOTYPE, throwaway: three radically different layouts of the Main window shell.
//!
//! Question: how should the shell look and behave (Menu bar, toolbar, Account bar, top-level
//! tabs, Main view, Status bar, right-click menus)? Which layout, menu contents and tab set?
//!
//! Run: `cargo run -p balanced_ui --example prototype_shell_layout [-- --variant b]`
//! Flip variants with the floating bar at the bottom, or the ← / → keys.
//!
//! Stub data only, English literals, default egui theme. None of this is production code.

#![allow(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::too_many_lines,
    clippy::wildcard_enum_match_arm,
    missing_debug_implementations,
    unreachable_pub
)]

use eframe::egui;
use egui::{Align, Align2, Button, CentralPanel, Id, Layout, Order, Panel, RichText, Sides, Ui};
use egui_extras::{Column, TableBuilder};

// ---------------------------------------------------------------------------------------------
// Stub data

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

    fn total(self) -> (&'static str, bool) {
        match self {
            Self::Banking => ("14,245.67", false),
            Self::Credit => ("-1,512.30", true),
            Self::Investing => ("63,410.00", false),
            Self::PropertyDebt => ("130,000.00", false),
        }
    }
}

struct Account {
    name: &'static str,
    group: Group,
    balance: &'static str,
    negative: bool,
}

const ACCOUNTS: &[Account] = &[
    Account {
        name: "Everyday Checking",
        group: Group::Banking,
        balance: "2,345.67",
        negative: false,
    },
    Account {
        name: "Joint Checking",
        group: Group::Banking,
        balance: "1,900.00",
        negative: false,
    },
    Account {
        name: "Savings",
        group: Group::Banking,
        balance: "10,000.00",
        negative: false,
    },
    Account {
        name: "Visa",
        group: Group::Credit,
        balance: "-512.30",
        negative: true,
    },
    Account {
        name: "Mastercard",
        group: Group::Credit,
        balance: "-1,000.00",
        negative: true,
    },
    Account {
        name: "Brokerage",
        group: Group::Investing,
        balance: "45,210.00",
        negative: false,
    },
    Account {
        name: "401(k)",
        group: Group::Investing,
        balance: "18,200.00",
        negative: false,
    },
    Account {
        name: "House",
        group: Group::PropertyDebt,
        balance: "350,000.00",
        negative: false,
    },
    Account {
        name: "Mortgage",
        group: Group::PropertyDebt,
        balance: "-220,000.00",
        negative: true,
    },
];

const NET_WORTH: &str = "206,143.37";

/// Quicken 2012-style top-level tabs. Charts/reports are out of scope, so these are placeholders.
const TABS: &[&str] = &[
    "Home",
    "Bills",
    "Spending",
    "Planning",
    "Investing",
    "Property & Debt",
];

const PAYEES: &[(&str, &str)] = &[
    ("Grocery Mart", "Food:Groceries"),
    ("City Power", "Utilities:Electric"),
    ("Paycheck", "Income:Salary"),
    ("Coffee Corner", "Food:Dining"),
    ("Gas & Go", "Auto:Fuel"),
    ("Landlord", "Housing:Rent"),
    ("Book Barn", "Leisure:Books"),
];

// ---------------------------------------------------------------------------------------------
// App state

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

    fn label(self) -> &'static str {
        match self {
            Self::A => "A · Classic Quicken (toolbar + tab strip)",
            Self::B => "B · Compact (tabs in menu row, context header, rail)",
            Self::C => "C · Workspace (nav in Account bar, register tabs)",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|v| *v == self).unwrap_or(0)
    }

    fn step(self, delta: isize) -> Self {
        let n = Self::ALL.len().cast_signed();
        let i = (self.index().cast_signed() + delta).rem_euclid(n);
        Self::ALL[i.cast_unsigned()]
    }
}

struct Proto {
    variant: Variant,
    nav: Nav,
    collapsed: [bool; 4],
    account_bar_open: bool,
    show_toolbar: bool,
    show_status_bar: bool,
    dark: bool,
    /// Variant C: open register tabs (account indices), in order.
    open_registers: Vec<usize>,
    /// Last action, shown in the Status bar so every click is visible.
    status: String,
    /// `PROTO_SHOTS=<dir>`: walk through `SHOTS`, save a PNG of each, then quit.
    shots: Option<std::path::PathBuf>,
    shot_index: usize,
    frames: u32,
}

const SHOTS: &[(Variant, bool, &str)] = &[
    (Variant::A, false, "a-classic-light"),
    (Variant::B, false, "b-compact-light"),
    (Variant::C, false, "c-workspace-light"),
    (Variant::A, true, "a-classic-dark"),
];

impl Proto {
    fn new(variant: Variant) -> Self {
        Self {
            variant,
            nav: Nav::Account(0),
            collapsed: [false; 4],
            account_bar_open: true,
            show_toolbar: true,
            show_status_bar: true,
            dark: false,
            open_registers: vec![0, 3],
            status: "Ready".to_owned(),
            shots: std::env::var_os("PROTO_SHOTS").map(Into::into),
            shot_index: 0,
            frames: 0,
        }
    }

    fn did(&mut self, what: impl Into<String>) {
        self.status = what.into();
    }

    fn open_account(&mut self, i: usize) {
        self.nav = Nav::Account(i);
        if !self.open_registers.contains(&i) {
            self.open_registers.push(i);
        }
        self.did(format!("Opened register: {}", ACCOUNTS[i].name));
    }
}

impl eframe::App for Proto {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !ctx.egui_wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                self.variant = self.variant.step(-1);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                self.variant = self.variant.step(1);
            }
        }

        match self.variant {
            Variant::A => self.variant_a(ui),
            Variant::B => self.variant_b(ui),
            Variant::C => self.variant_c(ui),
        }

        self.switcher(&ctx);
        self.screenshots(&ctx);
    }
}

// ---------------------------------------------------------------------------------------------
// Shared pieces (the menus are part of the question, so all variants share one proposal)

impl Proto {
    fn item(&mut self, ui: &mut Ui, path: &str, label: &str, shortcut: &str) {
        let button = if shortcut.is_empty() {
            Button::new(label)
        } else {
            Button::new(label).shortcut_text(shortcut)
        };
        if ui.add(button).clicked() {
            self.did(format!("Menu: {path} → {label}"));
            ui.close();
        }
    }

    fn menus(&mut self, ui: &mut Ui) {
        ui.menu_button("File", |ui| {
            self.item(ui, "File", "New Data File…", "");
            self.item(ui, "File", "Open Data File…", "Ctrl+O");
            ui.menu_button("Open Recent", |ui| {
                self.item(ui, "File ▸ Open Recent", "household.balanced", "");
            });
            ui.separator();
            self.item(ui, "File", "Back Up…", "Ctrl+B");
            self.item(ui, "File", "Restore Backup…", "");
            ui.separator();
            ui.menu_button("Import", |ui| {
                self.item(ui, "File ▸ Import", "QIF File…", "");
                self.item(ui, "File ▸ Import", "OFX / QFX File…", "");
                self.item(ui, "File ▸ Import", "CSV File…", "");
            });
            ui.menu_button("Export", |ui| {
                self.item(ui, "File ▸ Export", "QIF File…", "");
                self.item(ui, "File ▸ Export", "CSV File…", "");
            });
            ui.separator();
            self.item(ui, "File", "Print Register…", "Ctrl+P");
            ui.separator();
            self.item(ui, "File", "Exit", "Alt+F4");
        });
        ui.menu_button("Edit", |ui| {
            self.item(ui, "Edit", "Undo", "Ctrl+Z");
            self.item(ui, "Edit", "Redo", "Ctrl+Y");
            ui.separator();
            self.item(ui, "Edit", "Cut", "Ctrl+X");
            self.item(ui, "Edit", "Copy", "Ctrl+C");
            self.item(ui, "Edit", "Paste", "Ctrl+V");
            ui.separator();
            ui.menu_button("Transaction", |ui| {
                self.item(ui, "Edit ▸ Transaction", "New", "Ctrl+N");
                self.item(ui, "Edit ▸ Transaction", "Delete", "Ctrl+D");
                self.item(ui, "Edit ▸ Transaction", "Split…", "Ctrl+S");
                self.item(ui, "Edit ▸ Transaction", "Mark Cleared", "Ctrl+M");
            });
            ui.separator();
            self.item(ui, "Edit", "Find…", "Ctrl+F");
            self.item(ui, "Edit", "Find and Replace…", "Ctrl+H");
        });
        ui.menu_button("View", |ui| {
            if ui
                .checkbox(&mut self.account_bar_open, "Account Bar")
                .changed()
            {
                self.did("View → Account Bar toggled");
            }
            if ui.checkbox(&mut self.show_toolbar, "Toolbar").changed() {
                self.did("View → Toolbar toggled");
            }
            if ui
                .checkbox(&mut self.show_status_bar, "Status Bar")
                .changed()
            {
                self.did("View → Status Bar toggled");
            }
            ui.separator();
            ui.menu_button("Go To", |ui| {
                for (i, tab) in TABS.iter().enumerate() {
                    if ui.button(*tab).clicked() {
                        self.nav = Nav::Tab(i);
                        self.did(format!("View ▸ Go To → {tab}"));
                        ui.close();
                    }
                }
            });
            self.item(ui, "View", "Back", "Alt+←");
            self.item(ui, "View", "Forward", "Alt+→");
            ui.separator();
            if ui.checkbox(&mut self.dark, "Dark Theme").changed() {
                self.did("View → Dark Theme toggled");
            }
            self.item(ui, "View", "Zoom In", "Ctrl++");
            self.item(ui, "View", "Zoom Out", "Ctrl+-");
        });
        ui.menu_button("Tools", |ui| {
            self.item(ui, "Tools", "Account List…", "Ctrl+A");
            self.item(ui, "Tools", "Category List…", "Ctrl+Shift+C");
            self.item(ui, "Tools", "Payee List…", "");
            self.item(ui, "Tools", "Scheduled Transactions…", "Ctrl+J");
            ui.separator();
            self.item(ui, "Tools", "Reconcile Account…", "Ctrl+R");
            self.item(ui, "Tools", "Calculator", "");
            ui.separator();
            self.item(ui, "Tools", "Preferences…", "Ctrl+,");
        });
        ui.menu_button("Help", |ui| {
            self.item(ui, "Help", "Balanced Help", "F1");
            self.item(ui, "Help", "Keyboard Shortcuts", "");
            ui.separator();
            self.item(ui, "Help", "About Balanced", "");
        });
    }

    fn account_context_menu(&mut self, ui: &mut Ui, i: usize) {
        let name = ACCOUNTS[i].name;
        for label in [
            "Open Register",
            "Open in Detached Window",
            "Reconcile…",
            "Edit Account Details…",
            "Hide from Account Bar",
        ] {
            if ui.button(label).clicked() {
                self.did(format!("Account menu ({name}) → {label}"));
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Delete Account…").clicked() {
            self.did(format!("Account menu ({name}) → Delete Account…"));
            ui.close();
        }
    }

    fn money_label(ui: &mut Ui, text: &str, negative: bool) {
        let rt = RichText::new(text);
        if negative {
            let c = ui.visuals().error_fg_color;
            ui.label(rt.color(c));
        } else {
            ui.label(rt);
        }
    }

    /// The Account bar body: groups with totals, accounts with balances, right-click menus.
    fn account_groups(&mut self, ui: &mut Ui, dense: bool) {
        for (g, group) in Group::ALL.into_iter().enumerate() {
            let (total, neg) = group.total();
            let arrow = if self.collapsed[g] { "⏵" } else { "⏷" };
            let header = Sides::new().show(
                ui,
                |ui| {
                    ui.add(
                        Button::new(RichText::new(format!("{arrow} {}", group.name())).strong())
                            .frame(false),
                    )
                },
                |ui| Self::money_label(ui, total, neg),
            );
            if header.0.clicked() {
                self.collapsed[g] = !self.collapsed[g];
            }
            header.0.context_menu(|ui| {
                for label in [
                    "Add Account to Group…",
                    "Collapse All Groups",
                    "Expand All Groups",
                ] {
                    if ui.button(label).clicked() {
                        match label {
                            "Collapse All Groups" => self.collapsed = [true; 4],
                            "Expand All Groups" => self.collapsed = [false; 4],
                            _ => {}
                        }
                        self.did(format!("Group menu ({}) → {label}", group.name()));
                        ui.close();
                    }
                }
            });
            if self.collapsed[g] {
                continue;
            }
            for (i, acct) in ACCOUNTS
                .iter()
                .enumerate()
                .filter(|(_, a)| a.group == group)
            {
                let selected = self.nav == Nav::Account(i);
                let row = Sides::new().show(
                    ui,
                    |ui| {
                        ui.add_space(12.0);
                        ui.selectable_label(selected, acct.name)
                    },
                    |ui| Self::money_label(ui, acct.balance, acct.negative),
                );
                if row.0.clicked() {
                    self.open_account(i);
                }
                row.0.context_menu(|ui| self.account_context_menu(ui, i));
            }
            ui.add_space(if dense { 2.0 } else { 6.0 });
        }
    }

    fn net_worth(ui: &mut Ui) {
        Sides::new().show(
            ui,
            |ui| ui.label(RichText::new("Net Worth").strong()),
            |ui| ui.label(RichText::new(NET_WORTH).strong()),
        );
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
        let rows = 60;
        TableBuilder::new(ui)
            .id_salt(("register", account))
            .striped(true)
            .resizable(true)
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(80.0))
            .column(Column::exact(48.0))
            .column(Column::initial(160.0).at_least(80.0))
            .column(Column::initial(160.0).at_least(80.0))
            .column(Column::initial(84.0))
            .column(Column::initial(84.0))
            .column(Column::remainder().at_least(84.0))
            .header(22.0, |mut h| {
                for title in [
                    "Date", "Num", "Payee", "Category", "Payment", "Deposit", "Balance",
                ] {
                    h.col(|ui| {
                        ui.strong(title);
                    });
                }
            })
            .body(|body| {
                body.rows(20.0, rows, |mut row| {
                    let r = row.index();
                    let (payee, category) = PAYEES[r % PAYEES.len()];
                    let deposit = payee == "Paycheck";
                    let amount = format!("{}.{:02}", 10 + (r * 37) % 400, (r * 13) % 100);
                    row.col(|ui| {
                        ui.label(format!("2026-{:02}-{:02}", 1 + r / 28 % 12, 1 + r % 28));
                    });
                    row.col(|ui| {
                        if r % 5 == 0 {
                            ui.label(format!("{}", 1000 + r));
                        }
                    });
                    row.col(|ui| {
                        ui.label(payee);
                    });
                    row.col(|ui| {
                        ui.label(category);
                    });
                    row.col(|ui| {
                        if !deposit {
                            ui.label(&amount);
                        }
                    });
                    row.col(|ui| {
                        if deposit {
                            ui.label(&amount);
                        }
                    });
                    row.col(|ui| {
                        ui.label(format!(
                            "{},{:03}.{:02}",
                            2 + r / 10,
                            (r * 71) % 1000,
                            (r * 7) % 100
                        ));
                    });
                    row.response().context_menu(|ui| {
                        for label in [
                            "Edit",
                            "Split…",
                            "Mark Cleared",
                            "Duplicate",
                            "Go to Transfer",
                        ] {
                            if ui.button(label).clicked() {
                                self.did(format!("Register row {r} → {label}"));
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ui.button("Delete").clicked() {
                            self.did(format!("Register row {r} → Delete"));
                            ui.close();
                        }
                    });
                });
            });
    }

    fn nav_title(&self) -> &'static str {
        match self.nav {
            Nav::Account(i) => ACCOUNTS[i].name,
            Nav::Tab(t) => TABS[t],
        }
    }

    fn status_bar(&self, ui: &mut Ui, right: &str) {
        if !self.show_status_bar {
            return;
        }
        Panel::bottom("status_bar").show(ui, |ui| {
            Sides::new().show(
                ui,
                |ui| ui.label(&self.status),
                |ui| ui.label(RichText::new(right).weak()),
            );
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Variant A: Classic Quicken. Menu bar, icon toolbar, a full-width tab strip, Account bar on
// the left with group totals and Net Worth pinned at the bottom, Status bar.

impl Proto {
    fn variant_a(&mut self, ui: &mut Ui) {
        Panel::top("a_menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| self.menus(ui));
        });
        if self.show_toolbar {
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
                        if ui.button(format!("{icon} {label}")).clicked() {
                            self.did(format!("Toolbar → {label}"));
                        }
                    }
                });
            });
        }
        Panel::top("a_tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (t, tab) in TABS.iter().enumerate() {
                    let selected = self.nav == Nav::Tab(t);
                    if ui
                        .selectable_label(selected, RichText::new(*tab).size(15.0))
                        .clicked()
                    {
                        self.nav = Nav::Tab(t);
                        self.did(format!("Tab → {tab}"));
                    }
                    ui.add_space(8.0);
                }
            });
        });
        self.status_bar(ui, "Last backup: never");
        if self.account_bar_open {
            Panel::left("a_account_bar")
                .resizable(true)
                .default_size(260.0)
                .min_size(180.0)
                .show(ui, |ui| {
                    Panel::bottom("a_account_bar_footer").show(ui, |ui| {
                        Self::net_worth(ui);
                        if ui.button("➕ Add Account…").clicked() {
                            self.did("Account bar → Add Account…");
                        }
                    });
                    CentralPanel::default_margins().show(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| self.account_groups(ui, false));
                    });
                });
        }
        CentralPanel::default().show(ui, |ui| {
            if let Nav::Account(i) = self.nav {
                let a = &ACCOUNTS[i];
                Sides::new().show(
                    ui,
                    |ui| ui.heading(a.name),
                    |ui| {
                        Self::money_label(ui, a.balance, a.negative);
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
// Variant B: Compact. No global toolbar. Tabs share the menu row. The Main view gets a context
// header whose actions depend on what is shown. The Account bar collapses to a thin rail.
// Net Worth moves to the Status bar.

impl Proto {
    fn variant_b(&mut self, ui: &mut Ui) {
        Panel::top("b_menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                self.menus(ui);
                ui.separator();
                for (t, tab) in TABS.iter().enumerate() {
                    if ui.selectable_label(self.nav == Nav::Tab(t), *tab).clicked() {
                        self.nav = Nav::Tab(t);
                        self.did(format!("Tab → {tab}"));
                    }
                }
            });
        });
        self.status_bar(ui, &format!("Net Worth {NET_WORTH}   ·   9 accounts"));

        let mut open = self.account_bar_open;
        Panel::left("b_account_bar")
            .resizable(true)
            .default_size(230.0)
            .min_size(160.0)
            .show_collapsible(ui, &mut open, |ui| {
                Sides::new().show(
                    ui,
                    |ui| ui.label(RichText::new("ACCOUNTS").small().weak()),
                    |ui| {
                        if ui
                            .small_button("«")
                            .on_hover_text("Collapse Account bar")
                            .clicked()
                        {
                            self.account_bar_open = false;
                        }
                    },
                );
                egui::ScrollArea::vertical().show(ui, |ui| self.account_groups(ui, true));
            });
        if self.account_bar_open {
            self.account_bar_open = open;
        }
        if !self.account_bar_open {
            Panel::left("b_rail")
                .exact_size(28.0)
                .resizable(false)
                .show(ui, |ui| {
                    if ui
                        .small_button("»")
                        .on_hover_text("Show Account bar")
                        .clicked()
                    {
                        self.account_bar_open = true;
                    }
                });
        }

        CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(self.nav_title());
                if let Nav::Account(i) = self.nav {
                    let a = &ACCOUNTS[i];
                    Self::money_label(ui, a.balance, a.negative);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let actions: &[&str] = match self.nav {
                        Nav::Account(_) => &["More…", "Reconcile", "Filter", "+ Transaction"],
                        Nav::Tab(_) => &["More…", "Customize"],
                    };
                    for label in actions {
                        if ui.button(*label).clicked() {
                            self.did(format!("Context header → {label}"));
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
        self.status_bar(ui, &format!("Net Worth {NET_WORTH}"));
        if self.account_bar_open {
            Panel::left("c_account_bar")
                .resizable(true)
                .default_size(250.0)
                .min_size(180.0)
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    for (t, tab) in TABS.iter().enumerate() {
                        if ui
                            .selectable_label(
                                self.nav == Nav::Tab(t),
                                RichText::new(*tab).size(14.0),
                            )
                            .clicked()
                        {
                            self.nav = Nav::Tab(t);
                            self.did(format!("Nav → {tab}"));
                        }
                    }
                    ui.separator();
                    let mut filter = String::new();
                    ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Filter accounts…"));
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical().show(ui, |ui| self.account_groups(ui, true));
                    ui.separator();
                    Self::net_worth(ui);
                });
        }
        CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut close = None;
                for (pos, &i) in self.open_registers.iter().enumerate() {
                    let selected = self.nav == Nav::Account(i);
                    let r = ui.selectable_label(selected, ACCOUNTS[i].name);
                    if r.clicked() {
                        self.nav = Nav::Account(i);
                    }
                    if r.middle_clicked() || ui.small_button("×").clicked() {
                        close = Some(pos);
                    }
                    ui.separator();
                }
                if let Some(pos) = close {
                    let i = self.open_registers.remove(pos);
                    self.did(format!("Closed register tab: {}", ACCOUNTS[i].name));
                    if self.nav == Nav::Account(i) {
                        self.nav = self
                            .open_registers
                            .last()
                            .map_or(Nav::Tab(0), |&j| Nav::Account(j));
                    }
                }
            });
            ui.separator();
            self.main_view_body(ui);
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Prototype switcher: floating pill at the bottom centre, ← / → keys. Not part of any design.

impl Proto {
    fn switcher(&mut self, ctx: &egui::Context) {
        ctx.set_theme(if self.dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });
        egui::Area::new(Id::new("prototype_switcher"))
            .anchor(Align2::CENTER_BOTTOM, [0.0, -36.0])
            .order(Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("PROTOTYPE").small().strong());
                        if ui.button("◀").clicked() {
                            self.variant = self.variant.step(-1);
                        }
                        ui.label(RichText::new(self.variant.label()).strong());
                        if ui.button("▶").clicked() {
                            self.variant = self.variant.step(1);
                        }
                        ui.separator();
                        ui.checkbox(&mut self.dark, "Dark");
                    });
                });
            });
    }
}

impl Proto {
    fn screenshots(&mut self, ctx: &egui::Context) {
        let Some(dir) = self.shots.clone() else {
            return;
        };
        let Some(&(variant, dark, name)) = SHOTS.get(self.shot_index) else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        };
        self.variant = variant;
        self.dark = dark;
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            let [w, h] = image.size;
            let path = dir.join(format!("{name}.png"));
            image::save_buffer(
                &path,
                &bytes,
                w as u32,
                h as u32,
                image::ExtendedColorType::Rgba8,
            )
            .expect("save screenshot");
            self.shot_index += 1;
            self.frames = 0;
        } else {
            self.frames += 1;
            if self.frames == 20 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
        }
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let variant = match args
        .iter()
        .position(|a| a == "--variant")
        .and_then(|p| args.get(p + 1))
    {
        Some(v) if v.eq_ignore_ascii_case("b") => Variant::B,
        Some(v) if v.eq_ignore_ascii_case("c") => Variant::C,
        _ => Variant::A,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Balanced — PROTOTYPE shell layout")
            .with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "balanced_prototype_shell_layout",
        options,
        Box::new(move |_cc| Ok(Box::new(Proto::new(variant)))),
    )
}
