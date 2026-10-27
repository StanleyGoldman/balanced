//! PROTOTYPE (#374): the Reconcile window in three shapes, drawn into the real Main
//! window by the patched `main_window.rs`. Throwaway.
//!
//!   A  Heavy Dialog with two lists (Quicken Windows' layout), over the dimmed Register
//!   B  Register mode: the real Register filters to the reconcile set under a bar, and
//!      its Clr column does the ticking
//!   C  Docked side panel: one combined list beside the whole real Register
//!
//! Every tick, Mark all, Finish and edit is a real Command on the real ledger. The
//! statement in progress lives here, in memory, as no domain code holds it yet.

use std::cell::RefCell;
use std::collections::HashMap;

use balanced_core::{
    AccountId, AccountKind, AccountSummary, ClearedStatus, DomainCommand, LedgerView,
    NewTransaction, TransactionEdit, TransactionId,
};
use balanced_i18n::{AmountReader, AmountStyle, Localizer};
use eframe::egui::{
    self, Align, Color32, Id, Key, Layout, Modifiers, Panel, RichText, Sense, Sides, Stroke,
    TextEdit, Ui, vec2,
};
use egui_extras::{Column, TableBuilder};
use icu_calendar::{Date, Iso};
use serde_json::json;

use crate::command::{AppCommand, CommandSender};
use crate::data::{self, Day, Paper, cents, day_of, iso, money};
use crate::date_field::{self, DateEdit};
use crate::dialog::{DialogOutcome, HeavyDialog, LightDialog, OpenDialog};
use crate::formatted_amount::{FormattedAmount, i18n_currency};
use crate::i18n::fl;
use crate::proto;
use crate::register_model::RegisterRow;
use crate::route::Route;
use crate::theme::Theme;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    A,
    B,
    C,
}

impl Variant {
    pub const ALL: [Self; 3] = [Self::A, Self::B, Self::C];

    pub fn key(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|v| v.key().eq_ignore_ascii_case(key.trim()))
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::A => "Heavy Dialog, two lists",
            Self::B => "Register mode",
            Self::C => "Docked side panel",
        }
    }
}

pub const LATER: usize = 0;
pub const A_LAYOUT: usize = 1;
pub const B_ROWS: usize = 2;
pub const FIGS: usize = 3;
pub const TICK_BY: usize = 4;
pub const A_EDIT: usize = 5;

/// (key, label, applies to, options).
pub const TOGGLES: &[(&str, &str, &str, &[&str])] = &[
    (
        "later",
        "Later Uncleared transactions",
        "all",
        &["Hidden", "Shown"],
    ),
    ("lists", "A's two lists", "a", &["Side by side", "Stacked"]),
    (
        "rows",
        "B's Register rows",
        "b",
        &["Only the reconcile set", "All, others dimmed"],
    ),
    ("figures", "B and C's figures", "b,c", &["Above", "Below"]),
    (
        "tick",
        "A and C: ticking",
        "a,c",
        &["Click anywhere on the row", "Click the tick box only"],
    ),
    (
        "edit",
        "A's Edit… and New…",
        "a",
        &[
            "Open on top of the Dialog",
            "Finish later, go to the Register",
        ],
    ),
];

/// Which of the open Register's rows B shows, as `register_filter.rs` and
/// `register.rs` read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetRule {
    pub statement: Day,
    pub later: bool,
    /// Only the reconcile set, rather than every row with the others dimmed.
    pub only: bool,
}

impl SetRule {
    pub fn contains(&self, row: &RegisterRow) -> bool {
        let status = row.cleared_status();
        status != ClearedStatus::Reconciled
            && row.saved_date().is_some_and(|date| {
                day_of(date) <= self.statement || status == ClearedStatus::Cleared || self.later
            })
    }
}

thread_local! {
    static RULE: RefCell<Option<SetRule>> = const { RefCell::new(None) };
    static REQUEST: RefCell<Option<(Option<AccountId>, String)>> = const { RefCell::new(None) };
}

/// The rule B's Register filters or dims by this frame, if B is showing.
pub fn rule() -> Option<SetRule> {
    RULE.with(|r| *r.borrow())
}

/// Asks to open the Start Dialog for `account`, or the open account.
pub fn request(account: Option<AccountId>, source: &str) {
    proto::log(
        "reconcile_requested",
        json!({ "source": source, "account": account.map(|a| format!("{a:?}")) }),
    );
    REQUEST.with(|r| *r.borrow_mut() = Some((account, source.to_owned())));
}

#[derive(Debug)]
struct StartForm {
    account: AccountId,
    date: DateEdit,
    saved: Day,
    ending: String,
    error: Option<String>,
    editing: bool,
    focus: bool,
}

#[derive(Debug)]
struct AdjustForm {
    category: Option<balanced_core::CategoryId>,
}

#[derive(Debug)]
struct EditForm {
    transaction: TransactionId,
    amount: String,
    error: Option<String>,
    focus: bool,
}

#[derive(Debug)]
struct NewForm {
    payee: String,
    category: Option<balanced_core::CategoryId>,
    payment: String,
    deposit: String,
    error: Option<String>,
    focus: bool,
}

/// One row of a reconcile list.
#[derive(Debug, Clone)]
struct Item {
    id: TransactionId,
    day: Day,
    date: String,
    num: String,
    payee: String,
    cents: i64,
    cleared: bool,
}

#[derive(Debug, Default)]
pub struct Recon {
    pub variant: Option<Variant>,
    pub tog: [usize; 6],
    /// The statement in progress per account: (date, ending as typed).
    pub statements: HashMap<AccountId, (Day, i64)>,
    pub paper: HashMap<AccountId, Paper>,
    /// The account being reconciled.
    pub active: Option<AccountId>,
    pub sel: Option<TransactionId>,
    start: Option<OpenDialog<StartForm>>,
    heavy: Option<OpenDialog<()>>,
    adjust: Option<OpenDialog<AdjustForm>>,
    edit: Option<OpenDialog<EditForm>>,
    new: Option<OpenDialog<NewForm>>,
    /// A request waiting for its account's Register to open.
    waiting: Option<(AccountId, String)>,
    /// The Balance Adjustment Finish posted, still to be marked Reconciled.
    pending_adjust: Option<(AccountId, Day)>,
    /// The C panel has the keyboard: the last press landed in it.
    panel_keys: bool,
    /// The lists drawn last frame, in order, for the arrow keys.
    lists: Vec<Vec<TransactionId>>,
    /// Set by `Finish`: the A Dialog closes next frame.
    finish_requested: Option<String>,
    pub open_account: Option<AccountId>,
    /// What B or C last brought the Register to the set for.
    register_key: Option<(Variant, Option<AccountId>, Option<SetRule>, bool)>,
}

impl Recon {
    pub fn variant(&self) -> Variant {
        self.variant.unwrap_or(Variant::A)
    }

    pub fn later(&self) -> bool {
        self.tog[LATER] == 1
    }

    /// Whether the open account is being reconciled.
    fn showing(&self) -> bool {
        self.active.is_some() && self.active == self.open_account
    }

    pub fn showing_variant(&self, v: Variant) -> bool {
        self.showing() && self.variant() == v
    }

    /// Whether one of the prototype's Dialogs is open over the window.
    pub fn dialog_open(&self) -> bool {
        self.start.is_some()
            || self.heavy.is_some()
            || self.adjust.is_some()
            || self.edit.is_some()
            || self.new.is_some()
    }

    pub fn reset(&mut self, scenario: &data::Scenario) {
        let (variant, tog) = (self.variant, self.tog);
        *self = Self {
            variant,
            tog,
            statements: scenario.statements.clone(),
            paper: scenario.paper.clone(),
            ..Self::default()
        };
    }

    // -----------------------------------------------------------------------------------
    // Each frame

    /// Runs first in the Main window's frame: takes Reconcile requests, keeps the
    /// variant's surface open, publishes B's rule and takes the list keys.
    pub fn begin_frame(
        &mut self,
        ctx: &egui::Context,
        commands: &CommandSender,
        view: &LedgerView,
        open: Option<AccountId>,
        today: Date<Iso>,
    ) {
        self.open_account = open;
        if let Some((account, source)) = REQUEST.with(|r| r.borrow_mut().take()) {
            match account.or(open) {
                Some(account) if Some(account) == open => {
                    self.open_start(ctx, view, account, &source, false, today);
                }
                Some(account) => {
                    commands.send(AppCommand::Navigate(Route::Account(account)));
                    self.waiting = Some((account, source));
                }
                None => proto::log("reconcile_refused", json!({ "why": "no account open" })),
            }
        } else if let Some((account, source)) = self.waiting.take() {
            if Some(account) == open {
                self.open_start(ctx, view, account, &source, false, today);
            } else {
                self.waiting = Some((account, source));
            }
        }
        // Leaving the account in B or C is finishing later.
        if let Some(active) = self.active
            && Some(active) != open
            && self.variant() != Variant::A
        {
            self.finish_later(view, "switched account");
        }
        // A's Dialog is open exactly while A reconciles the open account.
        let want_heavy = self.showing_variant(Variant::A);
        if want_heavy && self.heavy.is_none() {
            self.heavy = Some(OpenDialog::new(ctx, ()));
        } else if !want_heavy && self.heavy.is_some() {
            self.heavy = None;
        }
        if let Some((account, day)) = self.pending_adjust.take() {
            let found = view.account(account).and_then(|s| {
                s.register()
                    .iter()
                    .map(|e| e.transaction())
                    .find(|t| {
                        t.payee() == "Balance Adjustment"
                            && day_of(t.date()) == day
                            && t.cleared_status() != ClearedStatus::Reconciled
                    })
                    .map(|t| t.id())
            });
            match found {
                Some(id) => commands.send(AppCommand::Domain(DomainCommand::EditTransaction {
                    transaction: id,
                    edit: TransactionEdit::ClearedStatus(ClearedStatus::Reconciled),
                })),
                None => self.pending_adjust = Some((account, day)),
            }
        }
        let rule = (self.showing_variant(Variant::B))
            .then(|| self.active.and_then(|a| self.statements.get(&a)))
            .flatten()
            .map(|(day, _)| SetRule {
                statement: *day,
                later: self.later(),
                only: self.tog[B_ROWS] == 0,
            });
        RULE.with(|r| *r.borrow_mut() = rule);
        // B and C: starting, or a new rule, brings the Register to the set's first row.
        let key = (self.showing_variant(Variant::B) || self.showing_variant(Variant::C))
            .then(|| (self.variant(), self.active, rule, self.later()));
        let before = std::mem::replace(&mut self.register_key, key);
        if key.is_some()
            && key != before
            && let Some(account) = self.active
            && let Some(first) = items(view, self, account).first()
        {
            commands.send(AppCommand::GoToTransaction {
                account,
                transaction: first.id,
            });
        }
        self.list_keys(ctx, commands, view);
        // The lists draw again this frame.
        self.lists.clear();
    }

    /// ↑/↓ and Space in A's lists, and in C's while the panel has the keyboard. They
    /// run before the Register, so it doesn't see them.
    fn list_keys(&mut self, ctx: &egui::Context, commands: &CommandSender, view: &LedgerView) {
        let a = self.showing_variant(Variant::A)
            && self.start.is_none()
            && self.adjust.is_none()
            && self.edit.is_none()
            && self.new.is_none();
        let c = self.showing_variant(Variant::C) && self.panel_keys && !self.dialog_open();
        if !(a || c) || ctx.text_edit_focused() {
            return;
        }
        let down = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
        let up = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
        if down || up {
            let list = self
                .sel
                .and_then(|s| self.lists.iter().find(|l| l.contains(&s)))
                .or_else(|| self.lists.first())
                .cloned()
                .unwrap_or_default();
            if !list.is_empty() {
                let at = self.sel.and_then(|s| list.iter().position(|v| *v == s));
                let next = match (at, down) {
                    (Some(i), true) => (i + 1).min(list.len() - 1),
                    (Some(i), false) => i.saturating_sub(1),
                    (None, _) => 0,
                };
                self.sel = Some(list[next]);
                proto::log(
                    "key",
                    json!({ "key": if down { "Down" } else { "Up" }, "selected": describe(view, list[next]) }),
                );
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Space)) {
            // A real Space also types a space, which a focused button would take.
            ctx.input_mut(|i| {
                i.events
                    .retain(|e| !matches!(e, egui::Event::Text(t) if t == " "))
            });
            if let Some(id) = self.sel {
                self.tick(view, commands, id, "Space");
            }
        }
    }

    // -----------------------------------------------------------------------------------
    // Actions

    fn open_start(
        &mut self,
        ctx: &egui::Context,
        view: &LedgerView,
        account: AccountId,
        source: &str,
        editing: bool,
        today: Date<Iso>,
    ) {
        let Some(summary) = view.account(account) else {
            return;
        };
        if summary.account().is_closed() {
            proto::log("reconcile_refused", json!({ "why": "closed account" }));
            return;
        }
        if !editing && self.active == Some(account) {
            proto::log("reconcile_already_open", json!({ "source": source }));
            return;
        }
        if !editing && let Some(other) = self.active {
            if other != account {
                self.finish_later(view, "started another account's reconcile");
            }
        }
        let remembered = self.statements.get(&account).copied();
        let saved = remembered.map_or(day_of(today), |(d, _)| d);
        let ending = remembered.map_or_else(String::new, |(_, e)| data_amount_text(e));
        proto::would_emit(
            "OpenDialog(ReconcileStart)",
            json!({ "source": source, "account": summary.account().name(), "refilled": remembered.is_some(), "editing": editing }),
        );
        self.start = Some(OpenDialog::new(
            ctx,
            StartForm {
                account,
                date: DateEdit::new(Some(iso(saved))),
                saved,
                ending,
                error: None,
                editing,
                focus: true,
            },
        ));
    }

    fn begin(
        &mut self,
        view: &LedgerView,
        account: AccountId,
        day: Day,
        ending: i64,
        editing: bool,
    ) {
        self.statements.insert(account, (day, ending));
        let name = account_name(view, account);
        proto::would_emit(
            format!(
                "SetStatement {{ account: {name}, date: {}, ending: {} }}",
                fmt_day(day),
                data_amount_text(ending)
            ),
            json!({ "editing": editing }),
        );
        if !editing {
            self.active = Some(account);
            self.sel = None;
            self.panel_keys = false;
            proto::log(
                "reconcile_open",
                json!({ "variant": self.variant().key(), "account": name, "figures": figures_json(view, self, account) }),
            );
            proto::screenshot(format!("reconcile-{}", self.variant().key()));
        }
    }

    pub fn tick(
        &mut self,
        view: &LedgerView,
        commands: &CommandSender,
        id: TransactionId,
        via: &str,
    ) {
        self.sel = Some(id);
        let Some(status) = status_of(view, self.active, id) else {
            return;
        };
        let to = match status {
            ClearedStatus::Uncleared => ClearedStatus::Cleared,
            ClearedStatus::Cleared => ClearedStatus::Uncleared,
            ClearedStatus::Reconciled => {
                proto::log("tick_refused", json!({ "why": "Reconciled", "via": via }));
                return;
            }
        };
        commands.send(AppCommand::Domain(DomainCommand::EditTransaction {
            transaction: id,
            edit: TransactionEdit::ClearedStatus(to),
        }));
        let mut f = describe(view, id);
        f["via"] = json!(via);
        proto::would_emit(format!("EditTransaction {{ ClearedStatus: {to:?} }}"), f);
    }

    pub fn mark_all(&mut self, view: &LedgerView, commands: &CommandSender, on: bool) {
        let Some(account) = self.active else {
            return;
        };
        let from = if on {
            ClearedStatus::Uncleared
        } else {
            ClearedStatus::Cleared
        };
        let to = if on {
            ClearedStatus::Cleared
        } else {
            ClearedStatus::Uncleared
        };
        let edits: Vec<DomainCommand> = items(view, self, account)
            .into_iter()
            .filter(|i| status_of(view, Some(account), i.id) == Some(from))
            .map(|i| DomainCommand::EditTransaction {
                transaction: i.id,
                edit: TransactionEdit::ClearedStatus(to),
            })
            .collect();
        let n = edits.len();
        if n > 0 {
            commands.send(AppCommand::Domain(DomainCommand::Compound(edits)));
        }
        proto::would_emit(
            format!("Compound[EditTransaction {{ ClearedStatus: {to:?} }} x{n}]"),
            json!({ "via": if on { "Mark all" } else { "Unmark all" } }),
        );
    }

    pub fn finish_later(&mut self, view: &LedgerView, via: &str) {
        let Some(account) = self.active.take() else {
            return;
        };
        self.heavy = None;
        proto::log(
            "finish_later",
            json!({ "would_emit": "none: the ticks are already saved and the statement stays on the account", "via": via, "account": account_name(view, account), "difference": difference(view, self, account).map(data_amount_text) }),
        );
        proto::notice(&format!(
            "Reconcile of {} saved for later. The ticks stay as Cleared.",
            account_name(view, account)
        ));
    }

    /// Finish: with a Difference, the Balance Adjustment Dialog first.
    pub fn finish(
        &mut self,
        ctx: &egui::Context,
        view: &LedgerView,
        commands: &CommandSender,
        adjust: Option<balanced_core::CategoryId>,
        via: &str,
    ) {
        let Some(account) = self.active else {
            return;
        };
        let diff = difference(view, self, account).unwrap_or(0);
        if diff != 0 && adjust.is_none() {
            proto::log(
                "finish_difference",
                json!({ "difference": data_amount_text(diff), "via": via }),
            );
            self.adjust = Some(OpenDialog::new(ctx, AdjustForm { category: None }));
            return;
        }
        let card = is_card(view, account);
        let (day, _) = self.statements[&account];
        let mut cmds = Vec::new();
        let mut parts = Vec::new();
        if let Some(c) = adjust {
            let amount = if card { -diff } else { diff };
            cmds.push(DomainCommand::PostTransaction {
                account,
                transaction: NewTransaction::new(iso(day), "Balance Adjustment", c, money(amount)),
            });
            parts.push(format!(
                "PostTransaction(Balance Adjustment, {}, {}, Reconciled)",
                fmt_day(day),
                data_amount_text(amount)
            ));
            self.pending_adjust = Some((account, day));
        }
        let cleared: Vec<TransactionId> = view
            .account(account)
            .map(|s| {
                s.register()
                    .iter()
                    .map(|e| e.transaction())
                    .filter(|t| t.cleared_status() == ClearedStatus::Cleared)
                    .map(|t| t.id())
                    .collect()
            })
            .unwrap_or_default();
        parts.push(format!(
            "EditTransaction(ClearedStatus: Reconciled) x{}",
            cleared.len()
        ));
        parts.push("ClearStatement".to_owned());
        for id in cleared {
            cmds.push(DomainCommand::EditTransaction {
                transaction: id,
                edit: TransactionEdit::ClearedStatus(ClearedStatus::Reconciled),
            });
        }
        commands.send(AppCommand::Domain(DomainCommand::Compound(cmds)));
        self.statements.remove(&account);
        self.active = None;
        self.heavy = None;
        let name = account_name(view, account);
        proto::would_emit(
            format!("Compound[{}]", parts.join(", ")),
            json!({ "via": via, "account": name, "note": "PROTOTYPE sends the adjustment's Reconciled mark as a second Command, so Undo takes two steps here" }),
        );
        proto::notice(&format!(
            "Reconciled {name} to the statement of {}.",
            fmt_day(day)
        ));
        proto::screenshot("finished");
    }

    fn edit_selected(
        &mut self,
        ctx: &egui::Context,
        view: &LedgerView,
        commands: &CommandSender,
        via: &str,
    ) {
        let Some(id) = self.sel else {
            proto::log(
                "edit_refused",
                json!({ "why": "nothing selected", "via": via }),
            );
            return;
        };
        let Some(account) = self.active else {
            return;
        };
        if self.variant() == Variant::A && self.tog[A_EDIT] == 1 {
            self.finish_later(view, "Edit… (goes to the Register)");
            commands.send(AppCommand::GoToTransaction {
                account,
                transaction: id,
            });
            proto::would_emit("GoToTransaction", describe(view, id));
            return;
        }
        let amount = view
            .account(account)
            .and_then(|s| s.register().iter().find(|e| e.transaction().id() == id))
            .map(|e| data_amount_text(cents(e.transaction().amount())))
            .unwrap_or_default();
        proto::log(
            "edit_open",
            json!({ "via": via, "txn": describe(view, id) }),
        );
        self.edit = Some(OpenDialog::new(
            ctx,
            EditForm {
                transaction: id,
                amount,
                error: None,
                focus: true,
            },
        ));
    }

    fn new_txn(
        &mut self,
        ctx: &egui::Context,
        view: &LedgerView,
        commands: &CommandSender,
        via: &str,
    ) {
        if self.variant() == Variant::A && self.tog[A_EDIT] == 1 {
            self.finish_later(view, "New… (goes to the Register)");
            commands.send(AppCommand::GoToDraftRow);
            proto::would_emit("GoToDraftRow", json!({ "via": via }));
            return;
        }
        proto::log("new_open", json!({ "via": via }));
        self.new = Some(OpenDialog::new(
            ctx,
            NewForm {
                payee: String::new(),
                category: None,
                payment: String::new(),
                deposit: String::new(),
                error: None,
                focus: true,
            },
        ));
    }

    // -----------------------------------------------------------------------------------
    // Control verbs

    /// Applies a control verb this module owns; returns false for one it doesn't know.
    pub fn control(
        &mut self,
        ctx: &egui::Context,
        view: &LedgerView,
        commands: &CommandSender,
        verb: &str,
        rest: &str,
        today: Date<Iso>,
    ) -> bool {
        match verb {
            "reconcile" => request(None, "control"),
            "date" => match (&mut self.start, parse_day(rest)) {
                (Some(d), Some(day)) => {
                    d.form.date = DateEdit::new(Some(iso(day)));
                    d.form.saved = day;
                }
                _ => return false,
            },
            "ending" => match &mut self.start {
                Some(d) => d.form.ending = rest.to_owned(),
                None => return false,
            },
            "amount" => match &mut self.edit {
                Some(d) => d.form.amount = rest.to_owned(),
                None => return false,
            },
            "category" => match &mut self.adjust {
                Some(d) => {
                    d.form.category = view
                        .categories()
                        .iter()
                        .find(|c| c.name().eq_ignore_ascii_case(rest))
                        .map(balanced_core::Category::id);
                }
                None => return false,
            },
            "tick" | "untick" => {
                let on = verb == "tick";
                let Some(account) = self.active else {
                    return false;
                };
                if rest == "all" {
                    self.mark_all(view, commands, on);
                } else if rest == "stmt" {
                    let ids: Vec<TransactionId> = self
                        .paper
                        .get(&account)
                        .map(|p| p.items.iter().map(|(id, _)| *id).collect())
                        .unwrap_or_default();
                    let edits: Vec<DomainCommand> = ids
                        .into_iter()
                        .filter(|id| {
                            status_of(view, Some(account), *id) == Some(ClearedStatus::Uncleared)
                        })
                        .map(|id| DomainCommand::EditTransaction {
                            transaction: id,
                            edit: TransactionEdit::ClearedStatus(ClearedStatus::Cleared),
                        })
                        .collect();
                    proto::would_emit(format!("control tick stmt x{}", edits.len()), json!({}));
                    commands.send(AppCommand::Domain(DomainCommand::Compound(edits)));
                } else if let Some(id) = find(view, account, rest, self) {
                    let status = status_of(view, Some(account), id);
                    if (status == Some(ClearedStatus::Uncleared)) == on {
                        self.tick(view, commands, id, "control");
                    }
                    self.sel = Some(id);
                } else {
                    return false;
                }
            }
            "select" => match self.active.and_then(|a| find(view, a, rest, self)) {
                Some(id) => {
                    self.sel = Some(id);
                    if let Some(account) = self.active
                        && self.variant() != Variant::A
                    {
                        commands.send(AppCommand::GoToTransaction {
                            account,
                            transaction: id,
                        });
                    }
                }
                None => return false,
            },
            "edit" => self.edit_selected(ctx, view, commands, "control"),
            "new" => self.new_txn(ctx, view, commands, "control"),
            "finish" => self.finish(ctx, view, commands, None, "control"),
            "later" => self.finish_later(view, "control"),
            "editstatement" => {
                if let Some(a) = self.active {
                    self.open_start(ctx, view, a, "control", true, today);
                }
            }
            _ => return false,
        }
        true
    }

    /// `ok` and `cancel` for the top prototype Dialog; false when none is open.
    pub fn answer_top(
        &mut self,
        ctx: &egui::Context,
        view: &LedgerView,
        commands: &CommandSender,
        ok: bool,
        today: Date<Iso>,
    ) -> bool {
        let _ = today;
        if self.adjust.is_some() {
            if ok {
                let c = self.adjust.as_ref().and_then(|d| d.form.category);
                if c.is_some() {
                    self.adjust = None;
                    self.finish(ctx, view, commands, c, "control ok");
                }
            } else {
                self.adjust = None;
            }
            return true;
        }
        if let Some(mut d) = self.start.take() {
            if ok {
                if self.submit_start(view, &mut d.form) {
                    return true;
                }
                self.start = Some(d);
            }
            return true;
        }
        if let Some(d) = self.edit.take() {
            if ok {
                self.submit_edit(view, commands, d.form);
            }
            return true;
        }
        if let Some(d) = self.new.take() {
            if ok {
                let mut form = d.form;
                if !self.submit_new(view, commands, &mut form) {
                    self.new = Some(OpenDialog::new(ctx, form));
                }
            }
            return true;
        }
        if self.heavy.is_some() {
            if ok {
                self.finish(ctx, view, commands, None, "control ok");
            } else {
                self.finish_later(view, "control cancel");
            }
            return true;
        }
        false
    }

    fn submit_start(&mut self, view: &LedgerView, form: &mut StartForm) -> bool {
        let day = form.saved;
        let i18n_free = form.ending.trim().replace(',', "");
        match i18n_free.parse::<f64>() {
            Ok(v) => {
                let ending = (v * 100.0).round() as i64;
                self.begin(view, form.account, day, ending, form.editing);
                true
            }
            Err(_) => {
                form.error = Some("Enter the ending balance from your statement.".to_owned());
                false
            }
        }
    }

    fn submit_edit(&mut self, view: &LedgerView, commands: &CommandSender, form: EditForm) {
        let text = form.amount.trim().replace(',', "");
        if let Ok(v) = text.parse::<f64>() {
            let c = (v * 100.0).round() as i64;
            commands.send(AppCommand::Domain(DomainCommand::EditTransaction {
                transaction: form.transaction,
                edit: TransactionEdit::Amount(money(c)),
            }));
            let mut f = describe(view, form.transaction);
            f["via"] = json!("Edit stand-in");
            proto::would_emit(
                format!("EditTransaction {{ Amount: {} }}", data_amount_text(c)),
                f,
            );
        }
    }

    fn submit_new(
        &mut self,
        view: &LedgerView,
        commands: &CommandSender,
        form: &mut NewForm,
    ) -> bool {
        let Some(account) = self.active else {
            return true;
        };
        let read = |s: &str| {
            s.trim()
                .replace(',', "")
                .parse::<f64>()
                .ok()
                .map(|v| (v * 100.0).round() as i64)
        };
        let out = read(&form.payment).unwrap_or(0);
        let inn = read(&form.deposit).unwrap_or(0);
        let Some(category) = form.category.or_else(|| {
            view.categories()
                .iter()
                .find(|c| c.name() == "Uncategorized")
                .map(balanced_core::Category::id)
        }) else {
            form.error = Some("Pick a category.".to_owned());
            return false;
        };
        let day = self.statements.get(&account).map_or(data::today(), |s| s.0);
        let mut signed = inn - out;
        if is_card(view, account) {
            // On a card, the first column is a charge (money out of the card's sense).
            signed = inn - out;
        }
        commands.send(AppCommand::Domain(DomainCommand::PostTransaction {
            account,
            transaction: NewTransaction::new(iso(day), form.payee.trim(), category, money(signed)),
        }));
        proto::would_emit(
            "PostTransaction",
            json!({ "via": "New stand-in", "payee": form.payee, "amount": data_amount_text(signed) }),
        );
        true
    }

    // -----------------------------------------------------------------------------------
    // Drawing

    /// C's panel, a right-hand Panel drawn before the Main view.
    pub fn side_panel(
        &mut self,
        ui: &mut Ui,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
    ) {
        if !self.showing_variant(Variant::C) {
            return;
        }
        let Some(account) = self.active else { return };
        let ctx = ui.ctx().clone();
        let r = Panel::right("reconcile_panel")
            .default_size(470.0)
            .resizable(true)
            .frame(
                theme
                    .chrome_frame()
                    .inner_margin(egui::Margin::symmetric(10, 8)),
            )
            .show(ui, |ui| {
                proto::region(ui.max_rect(), "reconcile panel");
                let name = account_name(view, account);
                Sides::new().show(
                    ui,
                    |ui| {
                        ui.label(RichText::new(format!("Reconcile {name}")).heading());
                    },
                    |ui| {
                        let r = ui.small_button("Close").on_hover_text("Finish later");
                        proto::track(&r, "panel close");
                        if r.clicked() {
                            self.finish_later(view, "panel Close");
                        }
                    },
                );
                ui.horizontal_wrapped(|ui| self.statement_line(ui, i18n, view, account));
                ui.separator();
                let above = self.tog[FIGS] == 0;
                let figs_h = 190.0;
                if above {
                    self.panel_figures(ui, &ctx, i18n, theme, commands, view, account);
                    ui.separator();
                }
                let list = items(view, self, account);
                let h = if above {
                    ui.available_height()
                } else {
                    (ui.available_height() - figs_h).max(120.0)
                };
                let w = ui.available_width();
                ui.allocate_ui(vec2(w, h), |ui| {
                    self.list(
                        ui,
                        i18n,
                        theme,
                        commands,
                        view,
                        "panel",
                        "To reconcile",
                        &list,
                        h,
                        true,
                    );
                });
                if !above {
                    ui.separator();
                    self.panel_figures(ui, &ctx, i18n, theme, commands, view, account);
                }
            });
        // A press in the panel gives it the arrow keys and Space; one elsewhere gives
        // them back to the Register.
        if ctx.input(|i| i.pointer.any_pressed())
            && let Some(pos) = ctx.input(|i| i.pointer.interact_pos())
        {
            let inside = r.response.rect.contains(pos);
            if inside != self.panel_keys {
                self.panel_keys = inside;
                proto::log("panel_keys", json!({ "panel": inside }));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn panel_figures(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
        account: AccountId,
    ) {
        let rows = figure_rows(view, self, account);
        let r = ui.vertical(|ui| {
            egui::Grid::new("rc_panel_figs")
                .num_columns(2)
                .spacing(vec2(24.0, 2.0))
                .show(ui, |ui| {
                    for (label, v, is_diff) in &rows {
                        ui.label(label.as_str());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(amount_text(i18n, theme, *v, *is_diff));
                        });
                        ui.end_row();
                    }
                });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if btn(ui, "Mark all") {
                    self.mark_all(view, commands, true);
                }
                if btn(ui, "Unmark all") {
                    self.mark_all(view, commands, false);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    self.finish_buttons(ui, ctx, view, commands);
                });
            });
        });
        proto::region(r.response.rect, "reconcile figures");
    }

    fn finish_buttons(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        view: &LedgerView,
        commands: &CommandSender,
    ) {
        let r =
            ui.add(egui::Button::new(RichText::new("Finish").strong()).min_size(vec2(80.0, 0.0)));
        proto::track(&r, "Finish");
        if r.clicked() {
            r.surrender_focus();
            self.finish(ctx, view, commands, None, "Finish button");
        }
        if btn(ui, "Finish later") {
            self.finish_later(view, "Finish later button");
        }
    }

    /// The statement line with Edit statement…, shared by B and C (A has it in its
    /// Dialog).
    fn statement_line(
        &mut self,
        ui: &mut Ui,
        i18n: &Localizer,
        view: &LedgerView,
        account: AccountId,
    ) {
        let (day, end) = self
            .statements
            .get(&account)
            .copied()
            .unwrap_or((data::today(), 0));
        let label = ending_label(view, account);
        ui.label(format!(
            "Statement date {}  ·  {label} {}",
            i18n.format_date(iso(day)),
            FormattedAmount::new(i18n, money(end), AmountStyle::Bare).text()
        ));
        if btn(ui, "Edit statement…") {
            request_edit_statement();
        }
    }

    /// B's reconcile bar, above or below the Register.
    pub fn reconcile_bar(
        &mut self,
        ui: &mut Ui,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
    ) {
        let Some(account) = self.active else { return };
        let ctx = ui.ctx().clone();
        let frame = egui::Frame::new()
            .fill(theme.header_fill())
            .stroke(Stroke::new(
                1.0,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .inner_margin(egui::Margin::symmetric(10, 6))
            .corner_radius(4.0);
        let r = frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Reconciling {}", account_name(view, account))).strong());
                ui.separator();
                self.statement_line(ui, i18n, view, account);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    self.finish_buttons(ui, &ctx, view, commands);
                    if btn(ui, "Unmark all") {
                        self.mark_all(view, commands, false);
                    }
                    if btn(ui, "Mark all") {
                        self.mark_all(view, commands, true);
                    }
                });
            });
            ui.add_space(2.0);
            let rows = figure_rows(view, self, account);
            ui.horizontal_wrapped(|ui| {
                for (i, (label, v, is_diff)) in rows.iter().enumerate() {
                    if i > 0 {
                        ui.label(RichText::new("·").weak());
                    }
                    ui.label(RichText::new(label.as_str()).weak());
                    ui.label(amount_text(i18n, theme, *v, *is_diff));
                }
            });
            ui.label(
                RichText::new(
                    "Click a Clr cell, or put the cell cursor on Clr and press Space, to tick it. Edit a row in place as usual.",
                )
                .small()
                .weak(),
            );
        });
        proto::region(r.response.rect, "reconcile bar");
    }

    /// One reconcile list: A's two, or C's combined one (signed amounts).
    #[allow(clippy::too_many_arguments)]
    fn list(
        &mut self,
        ui: &mut Ui,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
        key: &str,
        title: &str,
        list: &[Item],
        height: f32,
        signed: bool,
    ) {
        let row_h = theme.row_height();
        let ticked: i64 = list.iter().filter(|i| i.cleared).map(|i| i.cents).sum();
        ui.horizontal(|ui| {
            ui.strong(format!("{title} ({})", list.len()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let t = if signed { ticked } else { ticked.abs() };
                ui.label(
                    RichText::new(format!(
                        "ticked {}",
                        FormattedAmount::new(i18n, money(t), AmountStyle::Bare).text()
                    ))
                    .weak(),
                );
            });
        });
        let rect = ui.available_rect_before_wrap();
        proto::region(
            egui::Rect::from_min_size(rect.min, vec2(rect.width(), height - 24.0)),
            format!("{} list", title.to_lowercase()),
        );
        let tick_anywhere = self.tog[TICK_BY] == 0;
        let sel = self.sel;
        let mut ticks = Vec::new();
        let mut select = None;
        let mut edit = false;
        ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
        TableBuilder::new(ui)
            .id_salt(("rc_list", key))
            .striped(true)
            .resizable(false)
            .auto_shrink(false)
            .max_scroll_height(height - 24.0 - row_h)
            .min_scrolled_height(height - 24.0 - row_h)
            .sense(Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(26.0))
            .column(Column::exact(84.0))
            .column(Column::exact(44.0))
            .column(Column::remainder().at_least(80.0).clip(true))
            .column(Column::exact(96.0))
            .header(row_h, |mut header| {
                for (label, right) in [
                    ("", false),
                    ("Date", false),
                    ("Num", false),
                    ("Payee", false),
                    ("Amount", true),
                ] {
                    header.col(|ui| {
                        if right {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.strong(label)
                            });
                        } else {
                            ui.strong(label);
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(row_h, list.len(), |mut row| {
                    let item = &list[row.index()];
                    row.set_selected(sel == Some(item.id));
                    let amount = FormattedAmount::new(
                        i18n,
                        money(if signed { item.cents } else { item.cents.abs() }),
                        AmountStyle::Bare,
                    );
                    let label = format!("{} {} {}", item.payee, item.date, amount.text());
                    row.col(|ui| {
                        let r = tick_box(ui, item.cleared);
                        proto::track(&r, format!("tick box, {label}"));
                        if r.clicked() {
                            ticks.push((item.id, "tick box"));
                        }
                    });
                    row.col(|ui| {
                        ui.label(&item.date);
                    });
                    row.col(|ui| {
                        ui.label(&item.num);
                    });
                    row.col(|ui| {
                        ui.label(&item.payee);
                    });
                    row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(amount.styled(theme));
                        });
                    });
                    let r = row.response();
                    proto::track(&r, format!("reconcile row, {label}"));
                    if r.double_clicked() {
                        select = Some(item.id);
                        edit = true;
                    } else if r.clicked() {
                        if tick_anywhere {
                            ticks.push((item.id, "row click"));
                        } else {
                            select = Some(item.id);
                        }
                    }
                });
            });
        for (id, via) in ticks {
            self.tick(view, commands, id, via);
            if signed {
                self.reveal(commands, id);
            }
        }
        if let Some(id) = select {
            self.sel = Some(id);
            if signed {
                self.reveal(commands, id);
            }
            if edit {
                self.edit_selected(
                    &ui.ctx().clone(),
                    view,
                    commands,
                    "reconcile row double-click",
                );
            }
        }
        self.lists.push(list.iter().map(|i| i.id).collect());
    }

    /// C: a panel row clicked brings the Register to it.
    fn reveal(&self, commands: &CommandSender, id: TransactionId) {
        if let Some(account) = self.active {
            commands.send(AppCommand::GoToTransaction {
                account,
                transaction: id,
            });
        }
    }

    /// The prototype's Dialogs: A's heavy one, then the light ones over it.
    pub fn dialogs(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
        today: Date<Iso>,
    ) {
        if EDIT_STATEMENT.with(|e| std::mem::take(&mut *e.borrow_mut()))
            && let Some(a) = self.active
        {
            self.open_start(ctx, view, a, "Edit statement…", true, today);
        }
        self.heavy_dialog(ctx, i18n, theme, commands, view);
        self.start_dialog(ctx, i18n, theme, view, today);
        self.adjust_dialog(ctx, i18n, theme, commands, view);
        self.edit_dialog(ctx, i18n, theme, commands, view);
        self.new_dialog(ctx, i18n, theme, commands, view);
    }

    fn heavy_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
    ) {
        let Some(mut dialog) = self.heavy.take() else {
            return;
        };
        let Some(account) = self.active else { return };
        let name = account_name(view, account);
        let max = ctx.content_rect().size()
            - vec2(2.0 * Theme::DIALOG_MARGIN, 2.0 * Theme::DIALOG_MARGIN);
        let size = vec2(1120.0_f32.min(max.x), 680.0_f32.min(max.y));
        let outcome = HeavyDialog::new(
            Id::new("reconcile_dialog"),
            format!("Reconcile {name}"),
            "Finish".to_owned(),
            "Finish later".to_owned(),
            fl!(i18n, "dialog-close"),
            theme,
        )
        .with_size(size)
        .body_fills()
        .show(ctx, &mut dialog, |ui, _| {
            proto::region(ui.max_rect(), "reconcile dialog");
            self.dialog_body(ui, i18n, theme, commands, view, account);
        });
        // A light Dialog opened over it takes Enter and Esc this frame.
        let over = self.start.is_some()
            || self.adjust.is_some()
            || self.edit.is_some()
            || self.new.is_some();
        match outcome {
            DialogOutcome::Accepted if !over => {
                self.heavy = Some(dialog);
                self.finish(ctx, view, commands, None, "Dialog Finish / Enter");
                return;
            }
            DialogOutcome::Cancelled if !over => {
                self.finish_later(view, "Dialog Finish later / Esc / ×");
                return;
            }
            _ => {}
        }
        if self.active.is_some() && self.variant() == Variant::A {
            self.heavy = Some(dialog);
        }
    }

    fn dialog_body(
        &mut self,
        ui: &mut Ui,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
        account: AccountId,
    ) {
        let ctx = ui.ctx().clone();
        ui.horizontal(|ui| {
            let (day, end) = self
                .statements
                .get(&account)
                .copied()
                .unwrap_or((data::today(), 0));
            ui.label(format!(
                "Statement date {}  ·  {} {}",
                i18n.format_date(iso(day)),
                ending_label(view, account),
                FormattedAmount::new(i18n, money(end), AmountStyle::Bare).text()
            ));
            if btn(ui, "Edit statement…") {
                self.open_start(
                    &ctx,
                    view,
                    account,
                    "Edit statement…",
                    true,
                    iso(data::today()),
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if btn(ui, "Unmark all") {
                    self.mark_all(view, commands, false);
                }
                if btn(ui, "Mark all") {
                    self.mark_all(view, commands, true);
                }
                ui.separator();
                let has_sel = self.sel.is_some();
                let r = ui.add_enabled(has_sel, egui::Button::new("Edit…"));
                proto::track(&r, "dialog Edit");
                if r.clicked() {
                    self.edit_selected(&ctx, view, commands, "dialog Edit…");
                }
                if btn(ui, "New…") {
                    self.new_txn(&ctx, view, commands, "dialog New…");
                }
            });
        });
        ui.separator();
        let footer_h = 3.0 * theme.row_height() + 24.0;
        let lists_h = (ui.available_height() - footer_h).max(120.0);
        let w = ui.available_width();
        let all = items(view, self, account);
        let (out, inn): (Vec<Item>, Vec<Item>) = all.into_iter().partition(|i| i.cents < 0);
        let card = is_card(view, account);
        let (out_l, in_l) = if card {
            ("Charges", "Payments")
        } else {
            ("Payments", "Deposits")
        };
        let stacked = self.tog[A_LAYOUT] == 1;
        ui.allocate_ui(vec2(w, lists_h), |ui| {
            if stacked {
                let h = (lists_h - 8.0) / 2.0;
                ui.allocate_ui(vec2(w, h), |ui| {
                    self.list(
                        ui, i18n, theme, commands, view, "out", out_l, &out, h, false,
                    );
                });
                ui.allocate_ui(vec2(w, h), |ui| {
                    self.list(ui, i18n, theme, commands, view, "in", in_l, &inn, h, false);
                });
            } else {
                ui.columns(2, |cols| {
                    let (l, r) = cols.split_at_mut(1);
                    self.list(
                        &mut l[0], i18n, theme, commands, view, "out", out_l, &out, lists_h, false,
                    );
                    self.list(
                        &mut r[0], i18n, theme, commands, view, "in", in_l, &inn, lists_h, false,
                    );
                });
            }
        });
        ui.separator();
        let rows = figure_rows(view, self, account);
        let rect = ui.available_rect_before_wrap();
        proto::region(rect, "reconcile figures");
        // Painted at fixed places: right-to-left cells in a Grid stretched the Dialog.
        let row_h = theme.row_height();
        let (label_w, value_w, gap) = (210.0, 110.0, 32.0);
        let (rect, _) = ui.allocate_exact_size(
            vec2(2.0 * (label_w + value_w) + gap, 3.0 * row_h),
            Sense::hover(),
        );
        let font = egui::TextStyle::Body.resolve(ui.style());
        let text = ui.visuals().text_color();
        for (n, (label, v, is_diff)) in rows.iter().enumerate() {
            let col = (n / 3) as f32;
            let y = rect.top() + (n % 3) as f32 * row_h + row_h / 2.0;
            let x = rect.left() + col * (label_w + value_w + gap);
            ui.painter().text(
                egui::pos2(x, y),
                egui::Align2::LEFT_CENTER,
                label,
                font.clone(),
                text,
            );
            let color = match (*is_diff, *v == 0) {
                (true, true) => GREEN,
                (true, false) => ui.visuals().error_fg_color,
                _ => text,
            };
            ui.painter().text(
                egui::pos2(x + label_w + value_w, y),
                egui::Align2::RIGHT_CENTER,
                FormattedAmount::new(i18n, money(*v), AmountStyle::Bare).text(),
                font.clone(),
                color,
            );
        }
    }

    fn start_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        view: &LedgerView,
        today: Date<Iso>,
    ) {
        let Some(mut dialog) = self.start.take() else {
            return;
        };
        let account = dialog.form.account;
        let card = is_card(view, account);
        let name = account_name(view, account);
        let editing = dialog.form.editing;
        let remembered = self.statements.contains_key(&account);
        let opening = opening(view, account);
        let outcome = LightDialog::new(
            Id::new("reconcile_start"),
            if editing {
                "OK".to_owned()
            } else {
                "Start".to_owned()
            },
            fl!(i18n, "dialog-cancel"),
            theme,
        )
        .show(ctx, &mut dialog, |ui, form| {
            ui.label(
                RichText::new(if editing {
                    format!("Edit statement · {name}")
                } else {
                    format!("Reconcile {name}")
                })
                .strong(),
            );
            ui.add_space(4.0);
            if remembered && !editing {
                ui.label(RichText::new("Continuing the statement you started earlier.").weak());
            }
            ui.label(
                RichText::new(format!(
                    "Opening balance{} (from reconciled transactions): {}",
                    if card { " owed" } else { "" },
                    FormattedAmount::new(
                        i18n,
                        money(if card { -opening } else { opening }),
                        AmountStyle::Bare
                    )
                    .text()
                ))
                .weak(),
            );
            ui.add_space(6.0);
            let label = ui.label("Statement date");
            let (rect, _) = ui.allocate_exact_size(
                vec2(ui.available_width(), theme.row_height()),
                Sense::hover(),
            );
            let date_id = Id::new("reconcile_start_date");
            let r = form
                .date
                .show(ui, i18n, theme, today, date_id, rect)
                .labelled_by(label.id);
            proto::track(&r, "start: statement date");
            ui.add_space(4.0);
            let label = ui.label(if card {
                "Statement balance owed (as printed)"
            } else {
                "Statement ending balance"
            });
            let r = ui
                .add(
                    TextEdit::singleline(&mut form.ending)
                        .id(Id::new("reconcile_start_ending"))
                        .desired_width(f32::INFINITY),
                )
                .labelled_by(label.id);
            proto::track(&r, "start: ending balance");
            if form.focus {
                if editing || remembered {
                    r.request_focus();
                } else {
                    ui.memory_mut(|m| m.request_focus(date_id));
                }
                form.focus = false;
            }
            if r.changed() {
                form.error = None;
            }
            if let Some(e) = &form.error {
                ui.label(RichText::new(e.as_str()).color(ui.visuals().error_fg_color));
            }
        });
        match outcome {
            DialogOutcome::Open => self.start = Some(dialog),
            DialogOutcome::Accepted => {
                match dialog.form.date.commit(i18n) {
                    date_field::Commit::Unchanged => {}
                    date_field::Commit::Changed(date) => dialog.form.saved = day_of(date),
                    date_field::Commit::Rejected => {
                        self.start = Some(dialog);
                        return;
                    }
                }
                let mut form = dialog.form;
                let reader = i18n.amount_reader(i18n_currency(data::usd()));
                match read_amount(&reader, &form.ending) {
                    Some(ending) => {
                        self.begin(view, form.account, form.saved, ending, form.editing);
                    }
                    None => {
                        form.error =
                            Some("Enter the ending balance from your statement.".to_owned());
                        form.focus = true;
                        self.start = Some(OpenDialog::new(ctx, form));
                    }
                }
            }
            DialogOutcome::Cancelled | DialogOutcome::Back | DialogOutcome::Discarded => {
                proto::log("start_cancelled", json!({ "editing": editing }));
            }
        }
    }

    fn adjust_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
    ) {
        let Some(mut dialog) = self.adjust.take() else {
            return;
        };
        let Some(account) = self.active else { return };
        let diff = difference(view, self, account).unwrap_or(0);
        let (day, _) = self
            .statements
            .get(&account)
            .copied()
            .unwrap_or((data::today(), 0));
        let cats: Vec<(balanced_core::CategoryId, String)> = view
            .categories()
            .iter()
            .filter(|c| c.parent().is_none())
            .map(|c| {
                (
                    c.id(),
                    view.category_path(c.id())
                        .map(|p| p.join(":"))
                        .unwrap_or_default(),
                )
            })
            .collect();
        let ok = dialog.form.category.is_some();
        let outcome = LightDialog::new(
            Id::new("reconcile_adjust"),
            "Add Adjustment and Finish".to_owned(),
            "Go Back".to_owned(),
            theme,
        )
        .with_accept_enabled(ok)
        .show(ctx, &mut dialog, |ui, form| {
            ui.label(RichText::new("The statement doesn't balance").strong());
            ui.add_space(4.0);
            let d = FormattedAmount::new(i18n, money(diff), AmountStyle::Bare);
            ui.label(format!("The cleared transactions differ from the statement by {}.", d.text()));
            ui.label(format!(
                "Balanced can add a Balance Adjustment of {} dated {}, marked Reconciled, and finish.",
                d.text(),
                i18n.format_date(iso(day))
            ));
            ui.add_space(6.0);
            ui.label("Category");
            let selected = form
                .category
                .and_then(|c| cats.iter().find(|(id, _)| *id == c))
                .map_or("Pick a category".to_owned(), |(_, n)| n.clone());
            let r = egui::ComboBox::from_id_salt("adjust_cat")
                .selected_text(selected)
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for (id, n) in &cats {
                        ui.selectable_value(&mut form.category, Some(*id), n);
                    }
                });
            proto::track(&r.response, "adjust: category");
        });
        match outcome {
            DialogOutcome::Open => self.adjust = Some(dialog),
            DialogOutcome::Accepted => {
                let c = dialog.form.category;
                self.finish(ctx, view, commands, c, "adjustment");
            }
            _ => proto::log("adjust_go_back", json!({})),
        }
    }

    fn edit_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
    ) {
        let Some(mut dialog) = self.edit.take() else {
            return;
        };
        let id = dialog.form.transaction;
        let d = describe(view, id);
        let outcome = LightDialog::new(
            Id::new("reconcile_edit"),
            "Save".to_owned(),
            fl!(i18n, "dialog-cancel"),
            theme,
        )
        .show(ctx, &mut dialog, |ui, form| {
            ui.label(RichText::new("Edit Transaction").strong());
            ui.label(
                RichText::new(
                    "PROTOTYPE stand-in: the Register's row edit can't open over the Dialog.",
                )
                .small()
                .weak(),
            );
            ui.add_space(4.0);
            ui.label(format!(
                "{}  {}  {}",
                d["date"].as_str().unwrap_or(""),
                d["num"]
                    .as_str()
                    .map(|n| format!("#{n}"))
                    .unwrap_or_default(),
                d["payee"].as_str().unwrap_or("")
            ));
            let label = ui.label("Amount (negative for money out)");
            let r = ui
                .add(TextEdit::singleline(&mut form.amount).desired_width(f32::INFINITY))
                .labelled_by(label.id);
            proto::track(&r, "edit: amount");
            if form.focus {
                r.request_focus();
                form.focus = false;
            }
        });
        match outcome {
            DialogOutcome::Open => self.edit = Some(dialog),
            DialogOutcome::Accepted => self.submit_edit(view, commands, dialog.form),
            _ => proto::log("edit_cancelled", json!({})),
        }
    }

    fn new_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
    ) {
        let Some(mut dialog) = self.new.take() else {
            return;
        };
        let Some(account) = self.active else { return };
        let card = is_card(view, account);
        let cats: Vec<(balanced_core::CategoryId, String)> = view
            .categories()
            .iter()
            .map(|c| {
                (
                    c.id(),
                    view.category_path(c.id())
                        .map(|p| p.join(":"))
                        .unwrap_or_default(),
                )
            })
            .collect();
        let outcome = LightDialog::new(
            Id::new("reconcile_new"),
            "Save".to_owned(),
            fl!(i18n, "dialog-cancel"),
            theme,
        )
        .show(ctx, &mut dialog, |ui, form| {
            ui.label(RichText::new("New Transaction").strong());
            ui.label(
                RichText::new(
                    "PROTOTYPE stand-in for the Register's draft row, dated the statement date.",
                )
                .small()
                .weak(),
            );
            ui.label("Payee");
            let r = ui.add(TextEdit::singleline(&mut form.payee).desired_width(f32::INFINITY));
            proto::track(&r, "new: payee");
            if form.focus {
                r.request_focus();
                form.focus = false;
            }
            ui.label("Category");
            let selected = form
                .category
                .and_then(|c| cats.iter().find(|(id, _)| *id == c))
                .map_or("Uncategorized".to_owned(), |(_, n)| n.clone());
            egui::ComboBox::from_id_salt("new_cat")
                .selected_text(selected)
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for (id, n) in &cats {
                        ui.selectable_value(&mut form.category, Some(*id), n);
                    }
                });
            ui.label(if card { "Charge" } else { "Payment" });
            let r = ui.add(TextEdit::singleline(&mut form.payment).desired_width(f32::INFINITY));
            proto::track(&r, "new: payment");
            ui.label(if card { "Payment" } else { "Deposit" });
            let r = ui.add(TextEdit::singleline(&mut form.deposit).desired_width(f32::INFINITY));
            proto::track(&r, "new: deposit");
            if let Some(e) = &form.error {
                ui.label(RichText::new(e.as_str()).color(ui.visuals().error_fg_color));
            }
        });
        match outcome {
            DialogOutcome::Open => self.new = Some(dialog),
            DialogOutcome::Accepted => {
                let mut form = dialog.form;
                if !self.submit_new(view, commands, &mut form) {
                    self.new = Some(OpenDialog::new(ctx, form));
                }
            }
            _ => proto::log("new_cancelled", json!({})),
        }
    }
}

thread_local! {
    static EDIT_STATEMENT: RefCell<bool> = const { RefCell::new(false) };
}

fn request_edit_statement() {
    EDIT_STATEMENT.with(|e| *e.borrow_mut() = true);
}

const GREEN: Color32 = Color32::from_rgb(0x2e, 0x7d, 0x32);

/// A button that gives up focus after its click, so Space ticks a row afterwards
/// rather than pressing the button again.
fn btn(ui: &mut Ui, label: &str) -> bool {
    let r = ui.button(label);
    proto::track(&r, label);
    if r.clicked() {
        r.surrender_focus();
        return true;
    }
    false
}

/// A tick box painted by hand: the default fonts lack a reliable check mark.
fn tick_box(ui: &mut Ui, ticked: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
    let v = ui.visuals();
    let stroke = if r.hovered() {
        Stroke::new(1.5, v.strong_text_color())
    } else {
        Stroke::new(1.0, v.text_color())
    };
    ui.painter()
        .rect_stroke(rect.shrink(1.0), 2.0, stroke, egui::StrokeKind::Inside);
    if ticked {
        let c = rect.center();
        ui.painter().line(
            vec![
                c + vec2(-4.5, 0.0),
                c + vec2(-1.2, 3.5),
                c + vec2(4.5, -4.0),
            ],
            Stroke::new(2.0, GREEN),
        );
    }
    r
}

fn amount_text(i18n: &Localizer, theme: &Theme, cents: i64, is_diff: bool) -> RichText {
    let f = FormattedAmount::new(i18n, money(cents), AmountStyle::Bare);
    if is_diff {
        let t = RichText::new(f.text()).strong();
        if cents == 0 {
            t.color(GREEN)
        } else {
            t.color(Color32::from_rgb(0xc0, 0x2b, 0x2b))
        }
    } else {
        f.styled(theme)
    }
}

fn fmt_day(day: Day) -> String {
    let (y, m, d) = data::civil(day);
    format!("{m}/{d}/{y}")
}

fn parse_day(s: &str) -> Option<Day> {
    let mut it = s.trim().split('/');
    let m = it.next()?.parse().ok()?;
    let d = it.next()?.parse().ok()?;
    let y = it.next()?.parse().ok()?;
    Some(data::days_from_civil(y, m, d))
}

/// Cents as an en-US plain amount, for the log and the refill.
fn data_amount_text(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let a = cents.abs();
    let whole = (a / 100).to_string();
    let mut grouped = String::new();
    for (i, ch) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{sign}{grouped}.{:02}", a % 100)
}

fn read_amount(reader: &AmountReader, text: &str) -> Option<i64> {
    let calc = reader.read(text.trim()).ok()?;
    Some(cents(money_of(calc.amount())))
}

fn money_of(amount: rust_decimal::Decimal) -> balanced_core::Money {
    balanced_core::Money::new(amount, data::usd()).unwrap()
}

fn summary(view: &LedgerView, account: AccountId) -> Option<&AccountSummary> {
    view.account(account)
}

fn account_name(view: &LedgerView, account: AccountId) -> String {
    summary(view, account).map_or_else(String::new, |s| s.account().name().to_owned())
}

fn is_card(view: &LedgerView, account: AccountId) -> bool {
    summary(view, account).is_some_and(|s| {
        matches!(
            s.account().kind(),
            AccountKind::CreditCard | AccountKind::Liability
        )
    })
}

fn ending_label(view: &LedgerView, account: AccountId) -> &'static str {
    if is_card(view, account) {
        "Statement balance owed"
    } else {
        "Statement ending balance"
    }
}

fn status_of(
    view: &LedgerView,
    account: Option<AccountId>,
    id: TransactionId,
) -> Option<ClearedStatus> {
    let s = summary(view, account?)?;
    s.register()
        .iter()
        .find(|e| e.transaction().id() == id)
        .map(|e| e.transaction().cleared_status())
}

fn opening(view: &LedgerView, account: AccountId) -> i64 {
    summary(view, account).map_or(0, |s| {
        s.register()
            .iter()
            .filter(|e| e.transaction().cleared_status() == ClearedStatus::Reconciled)
            .map(|e| cents(e.transaction().amount()))
            .sum()
    })
}

fn cleared(view: &LedgerView, account: AccountId) -> i64 {
    summary(view, account).map_or(0, |s| cents(s.cleared_balance()))
}

/// Difference = statement ending − Cleared balance, both as the user reads them.
fn difference(view: &LedgerView, recon: &Recon, account: AccountId) -> Option<i64> {
    let (_, end) = recon.statements.get(&account)?;
    let c = cleared(view, account);
    Some(end - if is_card(view, account) { -c } else { c })
}

/// The reconcile set's rows, in date order.
fn items(view: &LedgerView, recon: &Recon, account: AccountId) -> Vec<Item> {
    let Some(s) = summary(view, account) else {
        return Vec::new();
    };
    let Some((stmt, _)) = recon.statements.get(&account).copied() else {
        return Vec::new();
    };
    let later = recon.later();
    s.register()
        .iter()
        .map(|e| e.transaction())
        .filter(|t| {
            let st = t.cleared_status();
            st != ClearedStatus::Reconciled
                && (day_of(t.date()) <= stmt || st == ClearedStatus::Cleared || later)
        })
        .map(|t| Item {
            id: t.id(),
            day: day_of(t.date()),
            date: fmt_day(day_of(t.date())),
            num: t.number().unwrap_or("").to_owned(),
            payee: t.payee().to_owned(),
            cents: cents(t.amount()),
            cleared: t.cleared_status() == ClearedStatus::Cleared,
        })
        .collect()
}

/// The figures, as label, value and whether it is the Difference.
fn figure_rows(view: &LedgerView, recon: &Recon, account: AccountId) -> Vec<(String, i64, bool)> {
    let card = is_card(view, account);
    let shown = |c: i64| if card { -c } else { c };
    let list = items(view, recon, account);
    let (on, ot): (usize, i64) = list
        .iter()
        .filter(|i| i.cleared && i.cents < 0)
        .fold((0, 0), |(n, t), i| (n + 1, t + i.cents));
    let (inn, it): (usize, i64) = list
        .iter()
        .filter(|i| i.cleared && i.cents >= 0)
        .fold((0, 0), |(n, t), i| (n + 1, t + i.cents));
    let (_, end) = recon.statements.get(&account).copied().unwrap_or((0, 0));
    let (out_l, in_l) = if card {
        ("Charges", "Payments")
    } else {
        ("Payments", "Deposits")
    };
    vec![
        (
            if card {
                "Opening balance owed"
            } else {
                "Opening balance"
            }
            .to_owned(),
            shown(opening(view, account)),
            false,
        ),
        (format!("{out_l} ticked ({on})"), -ot, false),
        (format!("{in_l} ticked ({inn})"), it, false),
        (
            if card {
                "Cleared balance owed"
            } else {
                "Cleared balance"
            }
            .to_owned(),
            shown(cleared(view, account)),
            false,
        ),
        (ending_label(view, account).to_owned(), end, false),
        (
            "Difference".to_owned(),
            difference(view, recon, account).unwrap_or(0),
            true,
        ),
    ]
}

pub fn figures_json(view: &LedgerView, recon: &Recon, account: AccountId) -> serde_json::Value {
    let rows = figure_rows(view, recon, account);
    serde_json::Value::Object(
        rows.into_iter()
            .map(|(l, v, _)| (l, json!(data_amount_text(v))))
            .collect(),
    )
}

pub fn describe(view: &LedgerView, id: TransactionId) -> serde_json::Value {
    for s in view.accounts() {
        if let Some(e) = s.register().iter().find(|e| e.transaction().id() == id) {
            let t = e.transaction();
            return json!({
                "account": s.account().name(),
                "date": fmt_day(day_of(t.date())),
                "num": t.number(),
                "payee": t.payee(),
                "amount": data_amount_text(cents(t.amount())),
                "status": format!("{:?}", t.cleared_status()),
            });
        }
    }
    json!({ "id": format!("{id:?}") })
}

/// A transaction by number or payee, preferring the reconcile set, then the newest.
fn find(view: &LedgerView, account: AccountId, what: &str, recon: &Recon) -> Option<TransactionId> {
    let s = summary(view, account)?;
    if let Some(t) = s
        .register()
        .iter()
        .map(|e| e.transaction())
        .find(|t| t.number() == Some(what))
    {
        return Some(t.id());
    }
    let set: Vec<TransactionId> = items(view, recon, account).iter().map(|i| i.id).collect();
    let q = what.to_lowercase();
    s.register()
        .iter()
        .map(|e| e.transaction())
        .filter(|t| t.payee().to_lowercase().contains(&q))
        .max_by_key(|t| (set.contains(&t.id()), day_of(t.date())))
        .map(|t| t.id())
}
