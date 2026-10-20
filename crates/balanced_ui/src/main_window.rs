// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Main window: Menu bar, tab row, Account bar, Main view and Status bar.

use std::path::PathBuf;

use balanced_core::{
    AccountId, AccountSummary, ClearedStatus, DomainCommand, DomainError, LedgerView,
    TransactionEdit, TransactionId,
};
use balanced_i18n::{AmountStyle, Localizer};
use eframe::egui::accesskit::Role;
use eframe::egui::text::LayoutJob;
use eframe::egui::{
    self, Align, Button, CentralPanel, FontSelection, Id, Label, MenuBar, Panel, Response,
    RichText, Sides,
};
use icu_calendar::{Date, Iso};

use crate::account_bar::{AccountBar, BarFrame};
use crate::accounts_model::AccountsModel;
use crate::command::{AppCommand, CommandSender, DialogKind, StatusMessage};
use crate::data_file::{self, DataFileRefusal};
use crate::dialog::{DialogOutcome, FocusReturn, HeavyDialog, LightDialog, OpenDialog};
use crate::formatted_amount::FormattedAmount;
use crate::i18n::fl;
use crate::keymap::{self, KEYMAP};
use crate::layout::Layout;
use crate::manage_accounts::{self, ManageAccountsForm, ManageAccountsOutcome};
use crate::menu_nav::{DrawnMenus, MenuNav};
use crate::new_account::{self, NewAccountForm, WizardOutcome};
use crate::new_category::NewCategoryForm;
use crate::preferences::{NetWorthPlacement, Preferences};
use crate::preferences_dialog::PreferencesForm;
use crate::register::{ColumnWidths, Register};
use crate::register_model::RegisterRow;
use crate::route::{Route, TopLevelTab};
use crate::theme::Theme;
use crate::user_action::UserAction;

/// What the Main window shows. Only its [`Layout`] outlives the run: every run starts
/// on Home, with no Register open.
#[derive(Debug)]
pub(crate) struct MainWindow {
    tab: TopLevelTab,
    status: Option<StatusMessage>,
    account_bar: AccountBar,
    /// The accounts of the last snapshot shown, rebuilt when it is out of date.
    accounts: Option<AccountsModel>,
    /// The Register open under the Accounts tab, if any.
    open_register: Option<OpenRegister>,
    /// The column widths every Register shares.
    column_widths: ColumnWidths,
    /// The delete transaction confirm, while it is open.
    delete_transaction: Option<OpenDialog<DeleteTransactionForm>>,
    /// The confirm for changing a Reconciled transaction's amount, while it is open.
    reconciled_amount: Option<OpenDialog<ReconciledAmountForm>>,
    /// The un-reconcile confirm, while it is open.
    unreconcile: Option<OpenDialog<UnreconcileForm>>,
    /// The New Account wizard, while it is open.
    new_account: Option<OpenDialog<NewAccountForm>>,
    /// The New Category Dialog, while it is open.
    new_category: Option<OpenDialog<NewCategoryForm>>,
    /// Manage Accounts, while it is open.
    manage_accounts: Option<OpenDialog<ManageAccountsForm>>,
    /// The Preferences Dialog, while it is open.
    preferences: Option<OpenDialog<PreferencesForm>>,
    /// The Dialog saying why a data file was refused, while it is open.
    data_file_refusal: Option<OpenDialog<DataFileRefusal>>,
    /// Where focus goes now that a Dialog has closed, until a frame restores it.
    focus_return: Option<FocusReturn>,
    menu_nav: MenuNav,
}

/// What the Main window is drawn with in one frame.
#[derive(Clone, Copy)]
pub(crate) struct WindowFrame<'a> {
    pub(crate) i18n: &'a Localizer,
    pub(crate) theme: &'a Theme,
    /// The Preferences shown: the open Preferences Dialog's draft, or else those in use.
    pub(crate) preferences: &'a Preferences,
    pub(crate) commands: &'a CommandSender,
    pub(crate) view: &'a LedgerView,
    pub(crate) today: Date<Iso>,
    /// The Recent files, newest first.
    pub(crate) recent_files: &'a [PathBuf],
}

/// The Register open under the Accounts tab and the account it shows.
#[derive(Debug)]
struct OpenRegister {
    account: AccountId,
    register: Register,
}

/// The delete transaction confirm's form: which transaction it deletes. Its question
/// is formatted every frame from the Register's row, so it follows the locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeleteTransactionForm {
    transaction: TransactionId,
}

/// The confirm for changing a Reconciled transaction's amount: which transaction, and
/// the amount the cell edit would save, formatted as the Dialog opens. No Preferences
/// change can reach it while it is open, so the text keeps up with the locale.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ReconciledAmountForm {
    transaction: TransactionId,
    amount: FormattedAmount,
}

/// The un-reconcile confirm's form: which Reconciled transaction it sets back to
/// Cleared. Its question is formatted every frame from the Register's row, so it
/// follows the locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UnreconcileForm {
    transaction: TransactionId,
}

impl MainWindow {
    /// The Main window on Home, laid out as `layout` says.
    pub(crate) fn new(layout: &Layout) -> Self {
        Self {
            tab: TopLevelTab::default(),
            status: None,
            account_bar: AccountBar::new(layout),
            accounts: None,
            open_register: None,
            column_widths: layout.column_widths(),
            delete_transaction: None,
            reconciled_amount: None,
            unreconcile: None,
            new_account: None,
            new_category: None,
            manage_accounts: None,
            preferences: None,
            data_file_refusal: None,
            focus_return: None,
            menu_nav: MenuNav::default(),
        }
    }

    /// The Main window's layout as it is now, for saving.
    pub(crate) fn layout(&self) -> Layout {
        let bar = &self.account_bar;
        Layout::new(
            bar.width(),
            bar.fold(),
            bar.folded_sections(),
            &self.column_widths,
        )
    }

    /// Shows `route` in the Main view.
    pub(crate) fn navigate(&mut self, route: Route) {
        match route {
            Route::Tab(tab) => self.tab = tab,
            Route::Account(account) => {
                self.tab = TopLevelTab::Accounts;
                // The account already open keeps its Register as it is.
                if self.open_account_id() != Some(account) {
                    self.open_register = Some(OpenRegister {
                        account,
                        register: Register::default(),
                    });
                }
            }
        }
    }

    /// The account whose Register is open, or `None` while none is.
    fn open_account_id(&self) -> Option<AccountId> {
        self.open_register.as_ref().map(|open| open.account)
    }

    /// The open Register, or `None` while none is.
    fn register(&self) -> Option<&Register> {
        self.open_register.as_ref().map(|open| &open.register)
    }

    /// The open Register, or `None` while none is.
    fn register_mut(&mut self) -> Option<&mut Register> {
        self.open_register.as_mut().map(|open| &mut open.register)
    }

    /// Whether the Main view shows a Register: the Accounts tab is active and a
    /// Register is open.
    fn shows_register(&self) -> bool {
        self.tab == TopLevelTab::Accounts && self.open_register.is_some()
    }

    /// Whether `action` can act now on `view`. Its menu item is disabled while it
    /// can't.
    fn can_act(&self, action: UserAction, view: &LedgerView) -> bool {
        match action {
            UserAction::Undo => view.can_undo(),
            UserAction::Redo => view.can_redo(),
            UserAction::NewTransaction
            | UserAction::DeleteTransaction
            | UserAction::PrintRegister
            | UserAction::ExportToCsv => self.shows_register(),
            UserAction::SwitchPane
            | UserAction::NewDataFile
            | UserAction::OpenDataFile
            | UserAction::NewAccount
            | UserAction::ManageAccounts
            | UserAction::Quit
            | UserAction::Preferences
            | UserAction::About
            | UserAction::EditAccountDetails
            | UserAction::RegisterPreferences => true,
            UserAction::Reconcile => self
                .open_account(view)
                .is_some_and(|summary| !summary.account().is_closed()),
        }
    }

    /// Whether the open Register takes Tab this frame: it is shown and owns the
    /// keyboard.
    pub(crate) fn register_takes_tab(&self, ctx: &egui::Context) -> bool {
        self.tab == TopLevelTab::Accounts
            && self
                .register()
                .is_some_and(|register| register.owns_keyboard(ctx))
    }

    /// Takes the arrow keys out of `raw_input` while a menu is open, for the Menu bar's
    /// keyboard navigation.
    pub(crate) fn take_menu_arrows(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.menu_nav.take_arrows(ctx, raw_input);
    }

    /// Hands a Tab press, with `modifiers` held, to the open Register.
    pub(crate) fn press_tab(&mut self, modifiers: egui::Modifiers) {
        if let Some(register) = self.register_mut() {
            register.press_tab(modifiers);
        }
    }

    /// Moves keyboard focus from the Account bar to the Main view, or from anywhere
    /// else to the Account bar.
    pub(crate) fn switch_pane(&mut self, ctx: &egui::Context) {
        if self.account_bar.has_focus() {
            // The Main view takes keys with no widget focused, as the Register does.
            release_keyboard(ctx);
        } else {
            self.account_bar.request_focus();
        }
    }

    /// Opens `account`'s Register under the Accounts tab and puts its cell cursor on
    /// `transaction`'s row. The keyboard follows, since the menu item that asked loses
    /// focus as it stops being drawn.
    pub(crate) fn go_to_transaction(&mut self, account: AccountId, transaction: TransactionId) {
        self.navigate(Route::Account(account));
        if let Some(register) = self.register_mut() {
            register.go_to(transaction);
        }
    }

    /// Shows the open Register and puts its cell cursor on the draft row, with the
    /// keyboard, unless no Register is open or its account in `view` is closed.
    pub(crate) fn go_to_draft_row(&mut self, ctx: &egui::Context, view: &LedgerView) {
        let open = self
            .open_account_id()
            .and_then(|account| view.account(account))
            .is_some_and(|summary| !summary.account().is_closed());
        let Some(OpenRegister { register, .. }) = self.open_register.as_mut().filter(|_| open)
        else {
            return;
        };
        self.tab = TopLevelTab::Accounts;
        register.go_to_draft_row();
        // The Register takes keys with no widget focused.
        release_keyboard(ctx);
    }

    /// Forgets everything shown from the data file that was open, now that another one
    /// is: the Register and its account, the accounts model, the Dialogs that act on
    /// the file's contents, and the Status bar's message. The new file's snapshots
    /// count generations afresh, so a cached model could pass as current.
    pub(crate) fn data_file_switched(&mut self) {
        let Self {
            tab: _,
            status,
            account_bar: _,
            accounts,
            open_register,
            column_widths: _,
            delete_transaction,
            reconciled_amount,
            unreconcile,
            new_account,
            new_category,
            manage_accounts,
            // The Preferences, and a refusal shown meanwhile, belong to no one file.
            preferences: _,
            data_file_refusal: _,
            focus_return: _,
            menu_nav: _,
        } = self;
        *status = None;
        *accounts = None;
        *open_register = None;
        *delete_transaction = None;
        *reconciled_amount = None;
        *unreconcile = None;
        *new_account = None;
        *new_category = None;
        *manage_accounts = None;
    }

    /// Opens the Dialog saying why a data file was refused, over any Dialog already
    /// open, since the native file picker leaves the Main window free meanwhile.
    pub(crate) fn refuse_data_file(&mut self, ctx: &egui::Context, refusal: DataFileRefusal) {
        self.data_file_refusal = Some(OpenDialog::new(ctx, refusal));
    }

    /// Shows `message` in the Status bar.
    pub(crate) fn show_status(&mut self, message: StatusMessage) {
        self.status = Some(message);
    }

    /// Hands the open Dialog the outcome of the Command its form submitted: it closes
    /// once the Command is applied, and shows a rejection inline. With no Dialog open
    /// to take it, a rejection goes to the Status bar.
    pub(crate) fn form_applied(&mut self, ctx: &egui::Context, result: Result<(), DomainError>) {
        match (result, &mut self.new_account) {
            (Ok(()), Some(_)) => self.close_dialog(ctx, |window| &mut window.new_account),
            (Ok(()), None) => {}
            (Err(error), Some(dialog)) => dialog.form.refused(error),
            (Err(error), None) => self.show_status(StatusMessage::Rejected(error)),
        }
    }

    /// Opens the Dialog `kind` over `view`, unless a Dialog is already open or there is
    /// nothing for it to act on. A Dialog that dates anything starts on `today`, and
    /// the Preferences Dialog on the `preferences` in use.
    pub(crate) fn open_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        kind: &DialogKind,
        view: &LedgerView,
        today: Date<Iso>,
        preferences: &Preferences,
    ) {
        if self.dialog_open() {
            return;
        }
        match kind {
            DialogKind::DeleteTransaction => {
                let Some(summary) = self.open_account(view) else {
                    return;
                };
                if summary.account().is_closed() {
                    return;
                }
                let Some(transaction) = self
                    .register()
                    .and_then(|register| register.selected_row(summary.account().id()))
                    .and_then(RegisterRow::transaction)
                else {
                    return;
                };
                let form = DeleteTransactionForm { transaction };
                self.delete_transaction = Some(OpenDialog::new(ctx, form));
            }
            &DialogKind::ChangeReconciledAmount {
                transaction,
                amount,
            } => {
                let form = ReconciledAmountForm {
                    transaction,
                    amount: FormattedAmount::new(i18n, amount, AmountStyle::Bare),
                };
                self.reconciled_amount = Some(OpenDialog::new(ctx, form));
            }
            DialogKind::Unreconcile { transaction } => {
                let Some(summary) = self.open_account(view) else {
                    return;
                };
                if summary.account().is_closed() {
                    return;
                }
                let reconciled = self
                    .register()
                    .and_then(|register| register.row(summary.account().id(), *transaction))
                    .is_some_and(|row| row.cleared_status() == ClearedStatus::Reconciled);
                if !reconciled {
                    return;
                }
                let form = UnreconcileForm {
                    transaction: *transaction,
                };
                self.unreconcile = Some(OpenDialog::new(ctx, form));
            }
            DialogKind::NewAccount => {
                let form = NewAccountForm::new(i18n, view.net_worth().currency(), today);
                self.new_account = Some(OpenDialog::new(ctx, form));
            }
            DialogKind::ManageAccounts => {
                self.manage_accounts = Some(OpenDialog::new(ctx, ManageAccountsForm::new(view)));
            }
            DialogKind::NewCategory { name } => {
                let form = NewCategoryForm::new(name.clone());
                self.new_category = Some(OpenDialog::new(ctx, form));
            }
            DialogKind::Preferences => {
                let form = PreferencesForm::new(preferences.clone());
                self.preferences = Some(OpenDialog::new(ctx, form));
            }
        }
    }

    /// Closes the Dialog `slot` holds, if it is open, and keeps where focus goes back to.
    fn close_dialog<F>(
        &mut self,
        ctx: &egui::Context,
        slot: impl FnOnce(&mut Self) -> &mut Option<OpenDialog<F>>,
    ) {
        if let Some(dialog) = slot(self).take() {
            self.focus_return = Some(dialog.close(ctx));
        }
    }

    /// Hands the open Dialog the ledger's answer to the change its form asked for: it
    /// closes once the change is `applied`, or shows why the ledger refused it.
    /// A refusal with no Dialog open to show it goes to the Status bar.
    pub(crate) fn dialog_applied(&mut self, ctx: &egui::Context, applied: Result<(), DomainError>) {
        if let Some(dialog) = &mut self.manage_accounts {
            match applied {
                Ok(()) => self.close_dialog(ctx, |window| &mut window.manage_accounts),
                Err(error) => dialog.form.rejected(error),
            }
            return;
        }
        let Some(dialog) = &mut self.new_category else {
            if let Err(error) = applied {
                self.show_status(StatusMessage::Rejected(error));
            }
            return;
        };
        if let Err(error) = applied {
            dialog.form.rejected(error);
            return;
        }
        let path = dialog.form.take_created_path();
        self.close_dialog(ctx, |window| &mut window.new_category);
        if let (Some(path), Some(register)) = (path, self.register_mut()) {
            register.category_created(path);
        }
    }

    /// Whether a Dialog is open over the Main window.
    fn dialog_open(&self) -> bool {
        self.delete_transaction.is_some()
            || self.reconciled_amount.is_some()
            || self.unreconcile.is_some()
            || self.new_account.is_some()
            || self.new_category.is_some()
            || self.manage_accounts.is_some()
            || self.preferences.is_some()
            || self.data_file_refusal.is_some()
    }

    /// The account the Main view shows, if it shows one.
    fn open_account<'a>(&self, view: &'a LedgerView) -> Option<&'a AccountSummary> {
        match self.tab {
            TopLevelTab::Accounts => self.open_account_id().and_then(|id| view.account(id)),
            TopLevelTab::Home
            | TopLevelTab::Bills
            | TopLevelTab::Spending
            | TopLevelTab::Planning
            | TopLevelTab::Investing
            | TopLevelTab::PropertyDebt => None,
        }
    }

    /// Draws the Main window's panels from `frame`, outer ones first and the Main view
    /// last.
    pub(crate) fn show(&mut self, ui: &mut egui::Ui, frame: &WindowFrame<'_>) {
        let WindowFrame {
            i18n,
            theme,
            preferences,
            commands,
            view,
            today,
            recent_files,
        } = *frame;
        self.focus_return = self
            .focus_return
            .take()
            .and_then(|focus| focus.restore(ui.ctx()));
        // No Main window Shortcut acts behind a Dialog.
        if !self.dialog_open() {
            self.menu_nav.keys(ui.ctx());
            let actions = keymap::dispatch(ui.ctx(), KEYMAP);
            // A Shortcut closes the open menu, and one pressed mid-edit saves the edit
            // before it acts.
            // TODO(#90): a refused commit stops the Shortcut and shows the error bubble.
            if !actions.is_empty() {
                self.menu_nav.close(ui.ctx());
                if let Some(register) = self.register_mut() {
                    register.commit_edit(ui.ctx(), i18n, commands);
                }
            }
            for action in actions {
                commands.send(action.command());
            }
        }
        let accounts = match self.accounts.take() {
            Some(model) if model.is_current(view, i18n, today) => model,
            _ => AccountsModel::new(view, i18n, today),
        };
        Panel::top("menu_bar")
            .frame(theme.chrome_frame())
            .show(ui, |ui| {
                let drawn = self.menu_bar(ui, i18n, commands, view, recent_files);
                self.menu_nav.drawn(ui.ctx(), drawn);
            });
        Panel::bottom("status_bar")
            .frame(theme.chrome_frame())
            .show(ui, |ui| {
                ui.ctx()
                    .accesskit_node_builder(ui.unique_id(), |node| node.set_role(Role::Status));
                // An empty label keeps the Status bar one line tall.
                let text = self.status.as_ref().map(|message| message.text(i18n));
                match preferences.net_worth_placement {
                    NetWorthPlacement::AccountBar => {
                        ui.label(text.unwrap_or_default());
                    }
                    NetWorthPlacement::StatusBar => {
                        Sides::new().shrink_left().truncate().show(
                            ui,
                            |ui| ui.label(text.unwrap_or_default()),
                            |ui| AccountBar::net_worth(ui, i18n, theme, accounts.net_worth()),
                        );
                    }
                }
            });
        Panel::top("tab_row")
            .frame(theme.chrome_frame())
            .show(ui, |ui| self.tab_row(ui, i18n, commands));
        let bar_frame = BarFrame {
            i18n,
            theme,
            commands,
            model: &accounts,
            selected: self.open_account_id(),
            preferences,
        };
        self.account_bar.show(ui, &bar_frame);
        CentralPanel::default().show(ui, |ui| match self.open_account(view) {
            Some(summary) => {
                self.context_header(ui, i18n, theme, commands, view, summary, &accounts);
                if let Some(OpenRegister { register, .. }) = &mut self.open_register {
                    register.show(
                        ui,
                        i18n,
                        theme,
                        commands,
                        summary,
                        view,
                        &mut self.column_widths,
                        preferences,
                        today,
                    );
                }
            }
            None => {
                ui.label(self.tab.placeholder(i18n));
            }
        });
        self.accounts = Some(accounts);
        self.delete_transaction_dialog(ui.ctx(), i18n, theme, commands, view);
        self.reconciled_amount_dialog(ui.ctx(), i18n, theme, commands, view);
        self.unreconcile_dialog(ui.ctx(), i18n, theme, commands, view);
        self.new_account_dialog(ui.ctx(), i18n, theme, commands, today);
        self.new_category_dialog(ui.ctx(), i18n, theme, commands, view);
        self.preferences_dialog(ui.ctx(), i18n, theme, commands);
        self.manage_accounts_dialog(ui.ctx(), i18n, theme, commands, view);
        self.data_file_refusal_dialog(ui.ctx(), i18n, theme);
    }

    /// The Dialog saying why a data file was refused, while it is open. OK closes it.
    fn data_file_refusal_dialog(&mut self, ctx: &egui::Context, i18n: &Localizer, theme: &Theme) {
        let Some(dialog) = &mut self.data_file_refusal else {
            return;
        };
        if DataFileRefusal::show_dialog(ctx, i18n, theme, dialog) {
            self.close_dialog(ctx, |window| &mut window.data_file_refusal);
        }
    }

    /// Manage Accounts, while it is open.
    fn manage_accounts_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
    ) {
        let (Some(dialog), Some(model)) = (&mut self.manage_accounts, &self.accounts) else {
            return;
        };
        match manage_accounts::show(ctx, i18n, theme, commands, dialog, view, model) {
            ManageAccountsOutcome::Open => {}
            ManageAccountsOutcome::Closed => {
                self.close_dialog(ctx, |window| &mut window.manage_accounts);
            }
        }
    }

    /// The New Account wizard, while it is open.
    fn new_account_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        today: Date<Iso>,
    ) {
        let Some(dialog) = &mut self.new_account else {
            return;
        };
        match new_account::show(ctx, i18n, theme, commands, dialog, today) {
            WizardOutcome::Open => {}
            WizardOutcome::Cancelled => self.close_dialog(ctx, |window| &mut window.new_account),
        }
    }

    /// The Preferences in the open Preferences Dialog, which the App shows while it is
    /// open.
    pub(crate) fn preferences_draft(&self) -> Option<&Preferences> {
        self.preferences.as_ref().map(|dialog| dialog.form.draft())
    }

    /// The Preferences Dialog, while it is open. OK sends its Preferences; Cancel
    /// closes it, and the App goes back to showing the Preferences in use.
    fn preferences_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
    ) {
        let Some(dialog) = &mut self.preferences else {
            return;
        };
        let outcome = HeavyDialog::new(
            Id::new("preferences_dialog"),
            fl!(i18n, "dialog-preferences-title"),
            fl!(i18n, "dialog-ok"),
            fl!(i18n, "dialog-cancel"),
            fl!(i18n, "dialog-close"),
            theme,
        )
        .show(ctx, dialog, |ui, form| form.show(ui, i18n));
        match outcome {
            DialogOutcome::Open => return,
            DialogOutcome::Accepted => {
                commands.send(AppCommand::SetPreferences(dialog.form.draft().clone()));
            }
            DialogOutcome::Back => unreachable!("a heavy Dialog has no Back button"),
            DialogOutcome::Cancelled => {}
        }
        self.close_dialog(ctx, |window| &mut window.preferences);
    }

    /// The New Category Dialog, while it is open. OK sends the Command creating the
    /// category, and the Dialog stays open until the ledger answers.
    fn new_category_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
    ) {
        let Some(dialog) = &mut self.new_category else {
            return;
        };
        let outcome = HeavyDialog::new(
            Id::new("new_category_dialog"),
            fl!(i18n, "dialog-new-category-title"),
            fl!(i18n, "dialog-ok"),
            fl!(i18n, "dialog-cancel"),
            fl!(i18n, "dialog-close"),
            theme,
        )
        .show(ctx, dialog, |ui, form| form.show(ui, i18n));
        match outcome {
            DialogOutcome::Open => {}
            DialogOutcome::Accepted => {
                if let Some(command) = dialog.form.submit(view) {
                    commands.send(AppCommand::SubmitDialog(command));
                }
            }
            DialogOutcome::Back => unreachable!("a heavy Dialog has no Back button"),
            DialogOutcome::Cancelled => {
                self.close_dialog(ctx, |window| &mut window.new_category);
                if let Some(register) = self.register_mut() {
                    register.category_cancelled();
                }
            }
        }
    }

    /// The Register's row for `transaction`, which a confirm open over the Register asks
    /// about. The Register's rows can't change while the confirm blocks it, so the row
    /// is there for as long as the confirm is open.
    fn confirmed_row(&self, view: &LedgerView, transaction: TransactionId) -> &RegisterRow {
        self.open_account(view)
            .and_then(|summary| self.register()?.row(summary.account().id(), transaction))
            .expect("the Register shows the transaction the confirm asks about")
    }

    /// The delete transaction confirm, while it is open. Delete has first focus, so
    /// Enter deletes. For a Reconciled transaction the question says it is reconciled.
    fn delete_transaction_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
    ) {
        let Some(form) = self.delete_transaction.as_ref().map(|dialog| dialog.form) else {
            return;
        };
        let row = self.confirmed_row(view, form.transaction);
        let (payee, date, amount) = (row.payee(), row.date(), row.amount_text());
        let question = match row.cleared_status() {
            ClearedStatus::Reconciled => fl!(
                i18n,
                "dialog-delete-reconciled-transaction-message",
                payee = payee,
                date = date,
                amount = amount
            ),
            ClearedStatus::Uncleared | ClearedStatus::Cleared => fl!(
                i18n,
                "dialog-delete-transaction-message",
                payee = payee,
                date = date,
                amount = amount
            ),
        };
        let Some(dialog) = &mut self.delete_transaction else {
            return;
        };
        let outcome = LightDialog::new(
            Id::new("delete_transaction_dialog"),
            fl!(i18n, "dialog-delete-transaction-delete"),
            fl!(i18n, "dialog-cancel"),
            theme,
        )
        .show(ctx, dialog, |ui, _form| {
            ui.label(question);
        });
        match outcome {
            DialogOutcome::Open => return,
            DialogOutcome::Accepted => {
                commands.send(AppCommand::Domain(DomainCommand::DeleteTransaction {
                    transaction: form.transaction,
                }));
            }
            DialogOutcome::Cancelled => {}
            DialogOutcome::Back => unreachable!("the delete confirm has no Back button"),
        }
        self.close_dialog(ctx, |window| &mut window.delete_transaction);
    }

    /// The confirm for changing a Reconciled transaction's amount, while it is open.
    /// Change Amount has first focus, so Enter sends the edit; Cancel sends nothing, so
    /// the cell keeps the amount it had.
    fn reconciled_amount_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
    ) {
        let Some(form) = self
            .reconciled_amount
            .as_ref()
            .map(|dialog| dialog.form.clone())
        else {
            return;
        };
        let row = self.confirmed_row(view, form.transaction);
        let question = fl!(
            i18n,
            "dialog-reconciled-amount-message",
            new = form.amount.text(),
            payee = row.payee(),
            date = row.date(),
            amount = row.amount_text()
        );
        let Some(dialog) = &mut self.reconciled_amount else {
            return;
        };
        let outcome = LightDialog::new(
            Id::new("reconciled_amount_dialog"),
            fl!(i18n, "dialog-reconciled-amount-change"),
            fl!(i18n, "dialog-cancel"),
            theme,
        )
        .show(ctx, dialog, |ui, _form| {
            ui.label(question);
        });
        match outcome {
            DialogOutcome::Open => return,
            DialogOutcome::Accepted => {
                commands.send(AppCommand::Domain(DomainCommand::EditTransaction {
                    transaction: form.transaction,
                    edit: TransactionEdit::Amount(form.amount.money()),
                }));
            }
            DialogOutcome::Cancelled => {}
            DialogOutcome::Back => unreachable!("the amount confirm has no Back button"),
        }
        self.close_dialog(ctx, |window| &mut window.reconciled_amount);
    }

    /// The un-reconcile confirm, while it is open. Mark Cleared has first focus, so
    /// Enter sets the transaction back to Cleared.
    fn unreconcile_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
    ) {
        let Some(form) = self.unreconcile.as_ref().map(|dialog| dialog.form) else {
            return;
        };
        let row = self.confirmed_row(view, form.transaction);
        let question = fl!(
            i18n,
            "dialog-unreconcile-message",
            payee = row.payee(),
            date = row.date(),
            amount = row.amount_text()
        );
        let Some(dialog) = &mut self.unreconcile else {
            return;
        };
        let outcome = LightDialog::new(
            Id::new("unreconcile_dialog"),
            fl!(i18n, "dialog-unreconcile-accept"),
            fl!(i18n, "dialog-cancel"),
            theme,
        )
        .show(ctx, dialog, |ui, _form| {
            ui.label(question);
        });
        match outcome {
            DialogOutcome::Open => return,
            DialogOutcome::Accepted => {
                commands.send(AppCommand::Domain(DomainCommand::EditTransaction {
                    transaction: form.transaction,
                    edit: TransactionEdit::ClearedStatus(ClearedStatus::Cleared),
                }));
            }
            DialogOutcome::Cancelled => {}
            DialogOutcome::Back => unreachable!("the un-reconcile confirm has no Back button"),
        }
        self.close_dialog(ctx, |window| &mut window.unreconcile);
    }

    /// The open account's name above its Register, with its Current, Ending and Cleared
    /// balances, the + Transaction button and the Account Actions menu on the right. A
    /// closed account's header says it is read-only, and + Transaction is disabled. The
    /// name clips when the Main view is narrow.
    #[expect(
        clippy::too_many_arguments,
        reason = "the Account Actions menu items need `view` and `commands` beside the header's own drawing context"
    )]
    fn context_header(
        &self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
        summary: &AccountSummary,
        accounts: &AccountsModel,
    ) {
        let account = summary.account();
        let missing = "the model formats every balance of every account in its snapshot";
        let current = accounts.current_balance(account.id()).expect(missing);
        let ending = accounts.ending_balance(account.id()).expect(missing);
        let cleared = accounts.cleared_balance(account.id()).expect(missing);
        let closed = account.is_closed();
        Sides::new().shrink_left().truncate().show(
            ui,
            |ui| {
                ui.add(Label::new(RichText::new(account.name()).heading()));
                if closed {
                    ui.label(fl!(i18n, "register-closed-read-only"));
                }
            },
            |ui| {
                // Sides lays the right side out from the right edge, so the last added
                // shows first.
                ui.menu_button(fl!(i18n, "register-account-actions"), |ui| {
                    for action in [
                        UserAction::Reconcile,
                        UserAction::EditAccountDetails,
                        UserAction::RegisterPreferences,
                        UserAction::PrintRegister,
                        UserAction::ExportToCsv,
                    ] {
                        self.action_item(ui, i18n, commands, view, action);
                    }
                });
                let add = Button::new(fl!(i18n, "register-new-transaction"));
                if ui.add_enabled(!closed, add).clicked() {
                    commands.send(AppCommand::GoToDraftRow);
                }
                for (message, balance) in [
                    (
                        fl!(i18n, "register-cleared-balance", amount = cleared.text()),
                        cleared,
                    ),
                    (
                        fl!(i18n, "register-ending-balance", amount = ending.text()),
                        ending,
                    ),
                    (
                        fl!(i18n, "register-current-balance", amount = current.text()),
                        current,
                    ),
                ] {
                    // One Label each, so a screen reader reads the name with its amount.
                    ui.label(header_balance(ui, theme, &message, balance));
                }
            },
        );
    }

    /// Draws the Menu bar, with `recent_files` numbered in the File menu above Quit,
    /// and returns what it drew for the keyboard navigation.
    fn menu_bar(
        &self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        commands: &CommandSender,
        view: &LedgerView,
        recent_files: &[PathBuf],
    ) -> DrawnMenus {
        let item = |ui: &mut egui::Ui, drawn: &mut DrawnMenus, action| {
            self.menu_item(ui, i18n, commands, view, drawn, action);
        };
        let mut drawn = DrawnMenus::default();
        MenuBar::new().ui(ui, |ui| {
            let file = ui.menu_button(fl!(i18n, "menu-file"), |ui| {
                item(ui, &mut drawn, UserAction::NewDataFile);
                item(ui, &mut drawn, UserAction::OpenDataFile);
                ui.separator();
                item(ui, &mut drawn, UserAction::NewAccount);
                ui.separator();
                if !recent_files.is_empty() {
                    Self::recent_file_items(ui, i18n, commands, &mut drawn, recent_files);
                    ui.separator();
                }
                item(ui, &mut drawn, UserAction::Quit);
            });
            drawn.note_menu(&file.response);
            let edit = ui.menu_button(fl!(i18n, "menu-edit"), |ui| {
                item(ui, &mut drawn, UserAction::Undo);
                item(ui, &mut drawn, UserAction::Redo);
            });
            drawn.note_menu(&edit.response);
            let view_menu = ui.menu_button(fl!(i18n, "menu-view"), |ui| {
                item(ui, &mut drawn, UserAction::SwitchPane);
            });
            drawn.note_menu(&view_menu.response);
            let tools = ui.menu_button(fl!(i18n, "menu-tools"), |ui| {
                item(ui, &mut drawn, UserAction::ManageAccounts);
                item(ui, &mut drawn, UserAction::Preferences);
            });
            drawn.note_menu(&tools.response);
            let help = ui.menu_button(fl!(i18n, "menu-help"), |ui| {
                item(ui, &mut drawn, UserAction::About);
            });
            drawn.note_menu(&help.response);
        });
        drawn
    }

    /// The File menu's items for `recent_files`, numbered from 1 at the newest, each
    /// showing its file's name and, on hover, its path. Clicking one opens its file.
    fn recent_file_items(
        ui: &mut egui::Ui,
        i18n: &Localizer,
        commands: &CommandSender,
        drawn: &mut DrawnMenus,
        recent_files: &[PathBuf],
    ) {
        for (number, path) in (1_u32..).zip(recent_files) {
            let file = data_file::file_name(path);
            let item = ui
                .button(fl!(i18n, "menu-file-recent", number = number, file = file))
                .on_hover_text(path.display().to_string());
            drawn.note_item(&item);
            if item.clicked() {
                commands.send(AppCommand::OpenDataFile(path.clone()));
                // egui closes a menu on a pointer click only, not on Enter.
                ui.close();
            }
        }
    }

    /// The Menu bar item for `action`, noted in `drawn` for the keyboard navigation.
    fn menu_item(
        &self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        commands: &CommandSender,
        view: &LedgerView,
        drawn: &mut DrawnMenus,
        action: UserAction,
    ) {
        let item = self.action_item(ui, i18n, commands, view, action);
        drawn.note_item(&item);
    }

    /// The menu item for `action`, with its Shortcut text, disabled while the action
    /// can't act on `view`. Clicking it sends the action's Command and closes the menu.
    fn action_item(
        &self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        commands: &CommandSender,
        view: &LedgerView,
        action: UserAction,
    ) -> Response {
        let mut button = egui::Button::new(action.label(i18n));
        if let Some(chord) = keymap::chord(action) {
            button = button.shortcut_text(keymap::shortcut_text(chord, i18n, ui.ctx().os()));
        }
        let item = ui.add_enabled(self.can_act(action, view), button);
        if item.clicked() {
            commands.send(action.command());
            // egui closes a menu on a pointer click only, not on Enter.
            ui.close();
        }
        item
    }

    fn tab_row(&self, ui: &mut egui::Ui, i18n: &Localizer, commands: &CommandSender) {
        ui.horizontal(|ui| {
            for tab in TopLevelTab::ALL {
                if ui
                    .selectable_label(tab == self.tab, tab.label(i18n))
                    .clicked()
                {
                    commands.send(AppCommand::Navigate(Route::Tab(tab)));
                }
            }
        });
    }
}

/// `message`, a context header balance such as "Current $5,642.21", as small, weak text
/// with `balance`'s amount in it styled by `theme` for its sign.
fn header_balance(
    ui: &egui::Ui,
    theme: &Theme,
    message: &str,
    balance: &FormattedAmount,
) -> LayoutJob {
    let (before, amount, after) = match message.split_once(balance.text()) {
        Some((before, after)) => (before, Some(balance.styled(theme)), after),
        None => (message, None, ""),
    };
    let mut text = LayoutJob::default();
    let parts = [
        Some(RichText::new(before).weak()),
        amount,
        Some(RichText::new(after).weak()),
    ];
    for part in parts.into_iter().flatten() {
        part.small()
            .append_to(&mut text, ui.style(), FontSelection::Default, Align::Center);
    }
    text
}

/// Takes keyboard focus off whatever widget holds it, so keys reach the Main view.
fn release_keyboard(ctx: &egui::Context) {
    ctx.memory_mut(|memory| {
        if let Some(id) = memory.focused() {
            memory.surrender_focus(id);
        }
    });
}
