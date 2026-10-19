// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Manage Accounts: a heavy Dialog listing every account, grouped as the Account bar
//! groups them, where each can be renamed, hidden, closed or reopened, deleted, and
//! moved within its sub-group. Nothing changes until OK, which sends every change as
//! one compound Command, so one Undo reverts them all.

use std::collections::HashSet;

use balanced_core::{
    AccountId, AccountKind, AccountStatus, DomainCommand, DomainError, LedgerView, MoveDirection,
    Visibility,
};
use balanced_i18n::Localizer;
use eframe::egui::{self, Checkbox, Grid, Id, Label, Response, RichText, TextEdit, vec2};

use crate::accounts_model::{AccountsModel, Section, kind_label};
use crate::command::{AppCommand, CommandSender, StatusMessage};
use crate::dialog::{DialogOutcome, FocusReturn, HeavyDialog, LightDialog, OpenDialog};
use crate::i18n::fl;
use crate::theme::Theme;
use crate::triangle_button::{Direction, triangle_button};

/// What Manage Accounts holds while it is open: every account as the user has left
/// it, in the order they will be in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManageAccountsForm {
    /// The accounts not deleted, by group, then sub-group, then the order the user
    /// has put them in.
    rows: Vec<Row>,
    /// Why the ledger refused the changes, shown below the table.
    error: Option<DomainError>,
    /// Whether OK sent the changes and the form waits for the ledger's answer.
    submitted: bool,
    /// The close or delete confirm, while it is open over the table.
    confirm: Option<OpenDialog<Confirm>>,
    /// Where focus goes now that a confirm has closed, until a frame restores it.
    focus_return: Option<FocusReturn>,
}

/// One account in the table, as the user has left it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    id: AccountId,
    kind: AccountKind,
    /// The Name field's text.
    name: String,
    visibility: Visibility,
    status: AccountStatus,
}

/// What a confirm opened from the table asks about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Confirm {
    Close(AccountId),
    Delete(AccountId),
}

/// What a click in the table asks for, done once the table is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableAction {
    /// Swap the row at this index with its neighbour.
    Move(usize, MoveDirection),
    Ask(Confirm),
}

/// What OK found to do.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Submission {
    /// Nothing changed, so the Dialog just closes.
    Unchanged,
    /// The changes were already sent, and the form waits for the ledger's answer.
    Waiting,
    /// Every change, as one Command.
    Changes(DomainCommand),
}

/// What became of Manage Accounts this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManageAccountsOutcome {
    Open,
    /// Cancelled, or OK with nothing changed.
    Closed,
}

impl ManageAccountsForm {
    /// The form holding every account of `view`, as the ledger has it.
    pub(crate) fn new(view: &LedgerView) -> Self {
        let rows = view
            .accounts()
            .iter()
            .map(|summary| {
                let account = summary.account();
                Row {
                    id: account.id(),
                    kind: account.kind(),
                    name: account.name().to_owned(),
                    visibility: account.visibility(),
                    status: account.status(),
                }
            })
            .collect();
        Self {
            rows,
            error: None,
            submitted: false,
            confirm: None,
            focus_return: None,
        }
    }

    /// Shows why the ledger refused the changes, which are kept for another try.
    pub(crate) fn rejected(&mut self, error: DomainError) {
        self.submitted = false;
        self.error = Some(error);
    }

    /// What OK does with the form, measured against `view`.
    fn submit(&mut self, view: &LedgerView) -> Submission {
        if self.submitted {
            return Submission::Waiting;
        }
        let changes = self.changes(view);
        if changes.is_empty() {
            return Submission::Unchanged;
        }
        self.submitted = true;
        self.error = None;
        Submission::Changes(DomainCommand::Compound(changes))
    }

    /// The Commands that make `view` what the form shows: deletions first, then each
    /// account's changes, then the moves that put the accounts in the form's order.
    fn changes(&self, view: &LedgerView) -> Vec<DomainCommand> {
        let kept: HashSet<AccountId> = self.rows.iter().map(|row| row.id).collect();
        let mut changes: Vec<DomainCommand> = view
            .accounts()
            .iter()
            .map(|summary| summary.account().id())
            .filter(|id| !kept.contains(id))
            .map(|account| DomainCommand::DeleteAccount { account })
            .collect();
        for row in &self.rows {
            let account = view
                .account(row.id)
                .expect("the ledger can't change while Manage Accounts is open")
                .account();
            let name = row.name.trim();
            if name != account.name() {
                changes.push(DomainCommand::RenameAccount {
                    account: row.id,
                    name: name.to_owned(),
                });
            }
            if row.visibility != account.visibility() {
                changes.push(DomainCommand::SetAccountVisibility {
                    account: row.id,
                    visibility: row.visibility,
                });
            }
            if row.status != account.status() {
                changes.push(DomainCommand::SetAccountStatus {
                    account: row.id,
                    status: row.status,
                });
            }
        }
        // Each row in turn moves up to its place. Rows move only within their
        // sub-group, so the rows above its place are already in theirs, and it never
        // passes a row of another sub-group.
        let mut order: Vec<AccountId> = view
            .accounts()
            .iter()
            .map(|summary| summary.account().id())
            .filter(|id| kept.contains(id))
            .collect();
        for (place, row) in self.rows.iter().enumerate() {
            let from = order
                .iter()
                .position(|&id| id == row.id)
                .expect("the form and the ledger hold the same accounts");
            for at in (place..from).rev() {
                order.swap(at, at + 1);
                changes.push(DomainCommand::MoveAccount {
                    account: row.id,
                    direction: MoveDirection::Up,
                });
            }
        }
        changes
    }

    fn row(&self, id: AccountId) -> Option<&Row> {
        self.rows.iter().find(|row| row.id == id)
    }

    /// Draws the table, and the error below it while there is one.
    fn show(&mut self, ui: &mut egui::Ui, i18n: &Localizer, theme: &Theme, model: &AccountsModel) {
        if let Some(focus) = self.focus_return.take() {
            self.focus_return = focus.restore_in(ui.ctx(), dialog_id());
        }
        let kinds: Vec<AccountKind> = self.rows.iter().map(|row| row.kind).collect();
        let mut action = None;
        Grid::new("manage_accounts_table").show(ui, |ui| {
            let name_header = ui.strong(fl!(i18n, "dialog-manage-accounts-name"));
            ui.strong(fl!(i18n, "dialog-manage-accounts-type"));
            ui.strong(fl!(i18n, "dialog-manage-accounts-balance"));
            ui.strong(fl!(i18n, "dialog-manage-accounts-hidden"));
            ui.strong(fl!(i18n, "dialog-manage-accounts-status"));
            ui.end_row();
            for (index, row) in self.rows.iter_mut().enumerate() {
                // `kinds` holds a kind for each row, and `above` is a row before this one.
                let above = index.checked_sub(1).map(|above| kinds[above]);
                if above.is_none_or(|above| above.group() != row.kind.group()) {
                    ui.strong(Section::Group(row.kind.group()).label(i18n));
                    ui.end_row();
                }
                if above != Some(row.kind) {
                    ui.label(RichText::new(kind_label(i18n, row.kind)).italics());
                    ui.end_row();
                }
                let below = kinds.get(index + 1).copied();
                let edges = SubGroupEdges {
                    first: above != Some(row.kind),
                    last: below != Some(row.kind),
                };
                if let Some(asked) = row.show(ui, i18n, theme, model, name_header.id, edges) {
                    action = Some(match asked {
                        RowAction::Move(direction) => TableAction::Move(index, direction),
                        RowAction::Ask(confirm) => TableAction::Ask(confirm),
                    });
                }
                ui.end_row();
            }
        });
        if let Some(error) = &self.error {
            let text = StatusMessage::Rejected(error.clone()).text(i18n);
            ui.add(Label::new(
                RichText::new(text).color(ui.visuals().error_fg_color),
            ));
        }
        match action {
            None => {}
            Some(TableAction::Move(index, direction)) => {
                // A row's arrow is enabled only when a row of its sub-group is that way.
                let neighbour = direction
                    .neighbour(index)
                    .filter(|&neighbour| neighbour < self.rows.len())
                    .expect("a row moves only towards a row of its sub-group");
                self.rows.swap(index, neighbour);
            }
            Some(TableAction::Ask(confirm)) => {
                self.confirm = Some(OpenDialog::new(ui.ctx(), confirm));
            }
        }
    }

    /// Carries out the confirm the user accepted.
    fn confirmed(&mut self, confirm: Confirm) {
        match confirm {
            Confirm::Close(id) => {
                if let Some(row) = self.rows.iter_mut().find(|row| row.id == id) {
                    row.status = AccountStatus::Closed;
                }
            }
            Confirm::Delete(id) => self.rows.retain(|row| row.id != id),
        }
    }
}

/// Whether a row is the first or the last of its sub-group, so can't move up or down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SubGroupEdges {
    first: bool,
    last: bool,
}

/// What a click in one row asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowAction {
    Move(MoveDirection),
    Ask(Confirm),
}

impl Row {
    /// Draws the row's cells: Name, Type, Balance, Hidden, Status, Delete…, ↑ and ↓.
    /// Reopen acts at once; the other buttons return what they ask for.
    fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        model: &AccountsModel,
        name_header: Id,
        edges: SubGroupEdges,
    ) -> Option<RowAction> {
        let mut action = None;
        let height = ui.spacing().interact_size.y;
        ui.add_sized(
            vec2(Theme::MANAGE_ACCOUNTS_NAME_WIDTH, height),
            TextEdit::singleline(&mut self.name).id(Id::new(("manage_accounts_name", self.id))),
        )
        .labelled_by(name_header);
        // Screen readers name each control after the account, as the Name field shows it.
        let name = self.name.clone();
        ui.horizontal(|ui| {
            ui.label(kind_label(i18n, self.kind));
            if self.status == AccountStatus::Closed {
                ui.label(
                    RichText::new(fl!(i18n, "dialog-manage-accounts-closed"))
                        .italics()
                        .color(theme.closed_tag()),
                );
            }
        });
        match model.ending_balance(self.id) {
            Some(balance) => ui.label(balance.styled(theme)),
            None => ui.label(""),
        };
        let mut hidden = self.visibility == Visibility::Hidden;
        let checkbox = ui.add(Checkbox::without_text(&mut hidden));
        name_for_screen_readers(
            &checkbox,
            fl!(i18n, "dialog-manage-accounts-hide", name = name.clone()),
        );
        self.visibility = if hidden {
            Visibility::Hidden
        } else {
            Visibility::Shown
        };
        match self.status {
            AccountStatus::Open => {
                let close = labelled_button(
                    ui,
                    fl!(i18n, "dialog-manage-accounts-close"),
                    fl!(
                        i18n,
                        "dialog-manage-accounts-close",
                        "a11y",
                        name = name.clone()
                    ),
                );
                if close.clicked() {
                    action = Some(RowAction::Ask(Confirm::Close(self.id)));
                }
            }
            AccountStatus::Closed => {
                let reopen = labelled_button(
                    ui,
                    fl!(i18n, "dialog-manage-accounts-reopen"),
                    fl!(
                        i18n,
                        "dialog-manage-accounts-reopen",
                        "a11y",
                        name = name.clone()
                    ),
                );
                if reopen.clicked() {
                    self.status = AccountStatus::Open;
                }
            }
        }
        let delete = labelled_button(
            ui,
            fl!(i18n, "dialog-manage-accounts-delete"),
            fl!(
                i18n,
                "dialog-manage-accounts-delete",
                "a11y",
                name = name.clone()
            ),
        );
        if delete.clicked() {
            action = Some(RowAction::Ask(Confirm::Delete(self.id)));
        }
        if let Some(direction) = arrows(ui, i18n, &name, edges) {
            action = Some(RowAction::Move(direction));
        }
        action
    }
}

/// The ↑ and ↓ buttons of the row of the account named `name`, each disabled at its
/// sub-group's edge. Returns the way the clicked one moves the row.
fn arrows(
    ui: &mut egui::Ui,
    i18n: &Localizer,
    name: &str,
    edges: SubGroupEdges,
) -> Option<MoveDirection> {
    ui.horizontal(|ui| {
        let up = ui
            .add_enabled_ui(!edges.first, |ui| {
                let label = fl!(
                    i18n,
                    "dialog-manage-accounts-move-up",
                    name = name.to_owned()
                );
                triangle_button(ui, label, Direction::Up)
            })
            .inner;
        let down = ui
            .add_enabled_ui(!edges.last, |ui| {
                let label = fl!(
                    i18n,
                    "dialog-manage-accounts-move-down",
                    name = name.to_owned()
                );
                triangle_button(ui, label, Direction::Down)
            })
            .inner;
        if up.clicked() {
            Some(MoveDirection::Up)
        } else if down.clicked() {
            Some(MoveDirection::Down)
        } else {
            None
        }
    })
    .inner
}

/// A button showing `text`, which screen readers name `label`.
fn labelled_button(ui: &mut egui::Ui, text: String, label: String) -> Response {
    let button = ui.button(text);
    name_for_screen_readers(&button, label);
    button
}

/// Gives `response`'s widget `label` as its name for screen readers, in place of the
/// text it shows.
fn name_for_screen_readers(response: &Response, label: String) {
    response
        .ctx
        .accesskit_node_builder(response.id, |node| node.set_label(label));
}

fn dialog_id() -> Id {
    Id::new("manage_accounts_dialog")
}

/// Draws Manage Accounts and any confirm open over it. OK sends every change as one
/// Command, and the Dialog stays open until the ledger answers; with nothing changed,
/// OK closes it as Cancel does.
pub(crate) fn show(
    ctx: &egui::Context,
    i18n: &Localizer,
    theme: &Theme,
    commands: &CommandSender,
    dialog: &mut OpenDialog<ManageAccountsForm>,
    view: &LedgerView,
    model: &AccountsModel,
) -> ManageAccountsOutcome {
    let outcome = HeavyDialog::new(
        dialog_id(),
        fl!(i18n, "dialog-manage-accounts-title"),
        fl!(i18n, "dialog-ok"),
        fl!(i18n, "dialog-cancel"),
        fl!(i18n, "dialog-close"),
        theme,
    )
    .with_size(Theme::MANAGE_ACCOUNTS_SIZE)
    .show(ctx, dialog, |ui, form| form.show(ui, i18n, theme, model));
    // Drawn after the heavy Dialog, so it is raised above it.
    show_confirm(ctx, i18n, theme, &mut dialog.form, model);
    match outcome {
        DialogOutcome::Open => ManageAccountsOutcome::Open,
        DialogOutcome::Accepted => match dialog.form.submit(view) {
            Submission::Unchanged => ManageAccountsOutcome::Closed,
            Submission::Waiting => ManageAccountsOutcome::Open,
            Submission::Changes(command) => {
                commands.send(AppCommand::SubmitDialog(command));
                ManageAccountsOutcome::Open
            }
        },
        DialogOutcome::Back => unreachable!("a heavy Dialog has no Back button"),
        DialogOutcome::Cancelled => ManageAccountsOutcome::Closed,
    }
}

/// The close or delete confirm, while one is open over the table. Its default button
/// has first focus, so Enter confirms.
fn show_confirm(
    ctx: &egui::Context,
    i18n: &Localizer,
    theme: &Theme,
    form: &mut ManageAccountsForm,
    model: &AccountsModel,
) {
    let Some(confirm) = form.confirm.as_ref().map(|dialog| dialog.form) else {
        return;
    };
    let (Confirm::Close(id) | Confirm::Delete(id)) = confirm;
    let name = form
        .row(id)
        .map(|row| row.name.trim().to_owned())
        .expect("a confirm asks about a row in the table");
    let (message, accept) = match confirm {
        Confirm::Close(_) => {
            let balance = model
                .ending_balance(id)
                .filter(|balance| !balance.money().amount().is_zero());
            let message = match balance {
                None => fl!(i18n, "dialog-close-account-message", name = name),
                Some(balance) => fl!(
                    i18n,
                    "dialog-close-account-message-balance",
                    name = name,
                    amount = balance.text().to_owned()
                ),
            };
            (message, fl!(i18n, "dialog-close-account-close"))
        }
        Confirm::Delete(_) => (
            fl!(i18n, "dialog-delete-account-message", name = name),
            fl!(i18n, "dialog-delete-account-delete"),
        ),
    };
    let Some(dialog) = &mut form.confirm else {
        return;
    };
    let outcome = LightDialog::new(
        Id::new("manage_accounts_confirm"),
        accept,
        fl!(i18n, "dialog-cancel"),
        theme,
    )
    .show(ctx, dialog, |ui, _confirm| {
        ui.label(message);
    });
    match outcome {
        DialogOutcome::Open => return,
        DialogOutcome::Accepted => form.confirmed(confirm),
        DialogOutcome::Cancelled => {}
        DialogOutcome::Back => unreachable!("a confirm has no Back button"),
    }
    if let Some(dialog) = form.confirm.take() {
        form.focus_return = Some(dialog.close(ctx));
    }
}
