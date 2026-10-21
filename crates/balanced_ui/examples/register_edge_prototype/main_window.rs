// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Main window: Menu bar, tab row, Account bar, Main view and Status bar.

use balanced_core::{
    AccountId, AccountSummary, ClearedStatus, CurrencyCode, DomainCommand, DomainError, LedgerView,
    TransactionEdit, TransactionId,
};
use balanced_i18n::{AmountStyle, Localizer};
use eframe::egui::accesskit::Role;
use eframe::egui::text::LayoutJob;
use eframe::egui::{
    self, Align, Button, CentralPanel, FontSelection, Id, Label, MenuBar, Panel, Rect, Response,
    RichText, Sides, vec2,
};
use icu_calendar::{Date, Iso};

use crate::account_bar::{AccountBar, BarFrame};
use crate::accounts_model::AccountsModel;
use crate::calculator::{self, Calculator};
use crate::command::{AppCommand, CommandSender, DialogKind, StatusMessage};
use crate::data_file::{self, DataFileRefusal};
use crate::dialog::{DialogOutcome, FocusReturn, HeavyDialog, LightDialog, OpenDialog};
use crate::formatted_amount::FormattedAmount;
use crate::go_to_date::GoToDateForm;
use crate::i18n::fl;
use crate::keymap::{self, KEYMAP};
use crate::layout::Layout;
use crate::manage_accounts::{self, ManageAccountsForm, ManageAccountsOutcome};
use crate::menu_nav::{DrawnMenus, MenuNav};
use crate::new_account::{self, NewAccountForm, WizardOutcome};
use crate::new_category::NewCategoryForm;
use crate::preferences::{NetWorthPlacement, Preferences};
use crate::preferences_dialog::PreferencesForm;
use crate::recent_files::RecentFiles;
use crate::register::{ColumnWidths, LeaveAnswer, LeftRow, Register};
use crate::register_model::RegisterRow;
use crate::route::{Route, TopLevelTab};
use crate::save_refusal::{RefusalAnswer, SaveRefusal};
use crate::split_dialog::{DoneOutcome, SplitForm};
use crate::theme::Theme;
use crate::user_action::UserAction;

/// What the Main window shows. Only its [`Layout`] outlives the run. A data file opens
/// on the view it last closed on, which the Recent files keep.
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
    /// Go to date, while it is open.
    go_to_date: Option<OpenDialog<GoToDateForm>>,
    /// The leave-row prompt, while it is open.
    leave_row: Option<OpenDialog<LeaveRowForm>>,
    /// The "Can't save this transaction" Dialog, with why the save was refused, while
    /// it is open.
    cant_save: Option<OpenDialog<SaveRefusal>>,
    /// The Split dialog, while it is open.
    split: Option<OpenDialog<SplitForm>>,
    /// The Calculator, while it is open.
    calculator: Option<Calculator>,
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
    pub(crate) recent_files: &'a RecentFiles,
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

/// The confirm for changing a Reconciled transaction's amount: which transaction, the
/// amount its row's save would give it, formatted as the Dialog opens, and the save.
/// No Preferences change can reach it while it is open, so the text keeps up with the
/// locale.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ReconciledAmountForm {
    transaction: TransactionId,
    amount: FormattedAmount,
    save: DomainCommand,
}

/// The leave-row prompt's form: which kind of row is being left, and whether "Always
/// save without asking" is ticked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LeaveRowForm {
    row: LeftRow,
    always: bool,
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
            go_to_date: None,
            leave_row: None,
            cant_save: None,
            split: None,
            calculator: None,
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

    /// Where the Main view is: the open account's Register on the Accounts tab, or
    /// else the tab shown.
    pub(crate) fn route(&self) -> Route {
        match self.open_account_id() {
            Some(account) if self.tab == TopLevelTab::Accounts => Route::Account(account),
            _ => Route::Tab(self.tab),
        }
    }

    /// Shows `route`, the view a data file last closed on, now that the file is open
    /// again as `view`. An account no longer in the file opens the Accounts tab with
    /// no Register.
    pub(crate) fn reopen(&mut self, route: Route, view: &LedgerView) {
        match route {
            Route::Account(account) if view.account(account).is_none() => {
                self.navigate(Route::Tab(TopLevelTab::Accounts));
            }
            Route::Tab(_) | Route::Account(_) => self.navigate(route),
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

    /// PROTOTYPE: the open Register, for the App's Quit settings.
    pub(crate) fn proto_register_mut(&mut self) -> Option<&mut Register> {
        self.register_mut()
    }

    /// The open Register, or `None` while none is.
    fn register_mut(&mut self) -> Option<&mut Register> {
        self.open_register.as_mut().map(|open| &mut open.register)
    }

    /// Whether showing `route` takes the Main view away from the open Register: to
    /// another account's, or from the Register shown to another tab.
    pub(crate) fn leaves_register(&self, route: Route) -> bool {
        match route {
            Route::Account(account) => self.open_account_id().is_some_and(|open| open != account),
            Route::Tab(tab) => tab != TopLevelTab::Accounts && self.shows_register(),
        }
    }

    /// Whether the Main view may go on to `then` at once, leaving the open Register's
    /// editing row. A changed row is not left: the leave-row prompt asks about it, and
    /// `then` is sent once it is saved or its changes thrown away.
    pub(crate) fn leave_editing_row(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        preferences: &Preferences,
        commands: &CommandSender,
        then: AppCommand,
    ) -> bool {
        self.register_mut()
            .is_none_or(|register| register.leave(ctx, i18n, preferences, commands, then))
    }

    /// Saves the changes to the open Register's editing row, cell being edited and
    /// all, with no prompt, as the window closes without having asked. The draft row
    /// is not added.
    pub(crate) fn save_editing_row(
        &mut self,
        i18n: &Localizer,
        preferences: &Preferences,
        commands: &CommandSender,
    ) {
        if let Some(register) = self.register_mut() {
            register.save_on_closing(i18n, preferences, commands);
        }
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
            | UserAction::GoToDate
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
            | UserAction::Calculator
            | UserAction::About
            | UserAction::EditAccountDetails
            | UserAction::RegisterPreferences => true,
            UserAction::Reconcile => self
                .open_account(view)
                .is_some_and(|summary| !summary.account().is_closed()),
        }
    }

    /// Whether the open Register takes Tab this frame: it is shown and owns the
    /// keyboard, or its editing row's buttons hold it.
    pub(crate) fn register_takes_tab(&self, ctx: &egui::Context) -> bool {
        self.tab == TopLevelTab::Accounts
            && self
                .register()
                .is_some_and(|register| register.takes_tab(ctx))
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

    /// Puts the open Register's cell cursor on the first row dated on or after `date`,
    /// or on its last row when none is, and scrolls the row into view. With no Register
    /// shown it does nothing.
    pub(crate) fn go_to_date(&mut self, date: Date<Iso>) {
        if !self.shows_register() {
            return;
        }
        if let Some(register) = self.register_mut() {
            register.go_to_date(date);
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
            go_to_date,
            leave_row,
            cant_save,
            split,
            // The Preferences, and a refusal shown meanwhile, belong to no one file.
            preferences: _,
            data_file_refusal: _,
            // The Calculator holds a sum, not the file's contents; its Paste target
            // goes with the Register.
            calculator: _,
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
        *go_to_date = None;
        *leave_row = None;
        *cant_save = None;
        *split = None;
    }

    /// Opens the Dialog saying why a data file was refused, over any Dialog already
    /// open, since the native file picker leaves the Main window free meanwhile.
    pub(crate) fn refuse_data_file(&mut self, ctx: &egui::Context, refusal: DataFileRefusal) {
        self.data_file_refusal = Some(OpenDialog::new(ctx, refusal));
    }

    /// Opens the Calculator in `currency`, unless it is already open.
    pub(crate) fn open_calculator(&mut self, i18n: &Localizer, currency: CurrencyCode) {
        self.calculator
            .get_or_insert_with(|| Calculator::new(i18n, currency));
    }

    /// The Calculator, while it is open, opening at the top right of `main_view`. Its
    /// Paste goes to the shown Register's last amount cell,
    /// and the Status bar says why when there is none or the sum isn't an amount.
    fn show_calculator(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        main_view: Rect,
    ) {
        let Some(calculator) = &mut self.calculator else {
            return;
        };
        let spot = main_view.right_top() + vec2(-Theme::DIALOG_MARGIN, Theme::DIALOG_MARGIN);
        match calculator.show(ctx, i18n, theme, spot) {
            calculator::Outcome::Open => {}
            calculator::Outcome::Closed => self.calculator = None,
            calculator::Outcome::Paste(text) => {
                let shown = self.shows_register();
                match self.register_mut().filter(|_| shown) {
                    Some(register) => register.paste(ctx, text),
                    None => self.show_status(StatusMessage::NoPasteTarget),
                }
            }
            calculator::Outcome::PasteRefused => {
                self.show_status(StatusMessage::PasteNotAnAmount);
            }
        }
    }

    /// Shows `message` in the Status bar.
    pub(crate) fn show_status(&mut self, message: StatusMessage) {
        self.status = Some(message);
    }

    /// Hands the ledger's refusal of a save of the Register's editing row, `message`,
    /// to the Register, or shows it in the Status bar when no save waits on it.
    pub(crate) fn save_refused(&mut self, message: StatusMessage) {
        let unclaimed = match self.register_mut() {
            Some(register) => register.save_refused(message),
            None => Some(message),
        };
        if let Some(message) = unclaimed {
            self.show_status(message);
        }
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
    /// nothing for it to act on. The leave-row prompt opens over any other Dialog. A
    /// Dialog that dates anything starts on `today`, and the Preferences Dialog on the
    /// `preferences` in use.
    pub(crate) fn open_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        kind: &DialogKind,
        view: &LedgerView,
        today: Date<Iso>,
        preferences: &Preferences,
    ) {
        if let DialogKind::LeaveRow(row) = *kind {
            let form = LeaveRowForm { row, always: false };
            self.leave_row = Some(OpenDialog::new(ctx, form));
            return;
        }
        if let DialogKind::CantSave(refusal) = kind {
            self.cant_save = Some(OpenDialog::new(ctx, refusal.clone()));
            return;
        }
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
            DialogKind::ChangeReconciledAmount {
                transaction,
                amount,
                save,
            } => {
                let form = ReconciledAmountForm {
                    transaction: *transaction,
                    amount: FormattedAmount::new(i18n, *amount, AmountStyle::Bare),
                    save: save.clone(),
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
            DialogKind::GoToDate => {
                let Some(summary) = self.open_account(view) else {
                    return;
                };
                let seed = self
                    .register()
                    .and_then(|register| register.selected_date(summary.account().id()))
                    .unwrap_or(today);
                self.go_to_date = Some(OpenDialog::new(ctx, GoToDateForm::new(seed)));
            }
            DialogKind::Split(start) => {
                let Some(summary) = self.open_account(view) else {
                    return;
                };
                let form = SplitForm::new(start, view, summary.account().id(), i18n);
                self.split = Some(OpenDialog::new(ctx, form));
            }
            DialogKind::LeaveRow(_) => unreachable!("the leave-row prompt opened above"),
            DialogKind::CantSave(_) => unreachable!("the refusal opened above"),
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
            || self.go_to_date.is_some()
            || self.leave_row.is_some()
            || self.cant_save.is_some()
            || self.split.is_some()
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
                    register.commit_edit(ui.ctx(), i18n, preferences, commands);
                }
            }
            for action in actions {
                // PROTOTYPE #444: Ctrl+D on a Reconciled amount change asks the amount
                // confirm first, then the delete confirm.
                if action == UserAction::DeleteTransaction
                    && let Some(register) = self.register_mut()
                    && register.proto_reconciled_amount_pending()
                {
                    if crate::proto::is(crate::proto::Setting::CtrlDReconciled, 1) {
                        crate::proto::decided(
                            crate::proto::Setting::CtrlDReconciled,
                            "amount confirm first, then delete",
                        );
                        if register.proto_save_then(
                            ui.ctx(),
                            i18n,
                            preferences,
                            commands,
                            action.command(),
                        ) {
                            commands.send(action.command());
                        }
                        continue;
                    }
                    crate::proto::decided(
                        crate::proto::Setting::CtrlDReconciled,
                        "delete confirm only",
                    );
                }
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
        let main_view = CentralPanel::default().show(ui, |ui| match self.open_account(view) {
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
        self.show_calculator(ui.ctx(), i18n, theme, main_view.response.rect);
        self.dialogs(ui.ctx(), frame);
    }

    /// Draws whichever Dialogs are open, from `frame`. The leave-row prompt and the
    /// refusal of a save come last, since they open over any other Dialog.
    fn dialogs(&mut self, ctx: &egui::Context, frame: &WindowFrame<'_>) {
        let WindowFrame {
            i18n,
            theme,
            preferences,
            commands,
            view,
            today,
            recent_files: _,
        } = *frame;
        self.delete_transaction_dialog(ctx, i18n, theme, commands, view);
        self.reconciled_amount_dialog(ctx, i18n, theme, commands, view);
        self.unreconcile_dialog(ctx, i18n, theme, commands, view);
        self.new_account_dialog(ctx, i18n, theme, commands, today);
        self.new_category_dialog(ctx, i18n, theme, commands, view);
        self.preferences_dialog(ctx, i18n, theme, commands);
        self.manage_accounts_dialog(ctx, i18n, theme, commands, view);
        self.data_file_refusal_dialog(ctx, i18n, theme);
        self.go_to_date_dialog(ctx, i18n, theme, commands, today);
        self.split_dialog(ctx, i18n, theme, preferences);
        self.leave_row_dialog(ctx, i18n, theme, commands, preferences);
        self.cant_save_dialog(ctx, i18n, theme);
    }

    /// The Split dialog, while it is open. Done hands its lines to the Register's row,
    /// unless they don't add up, which the Dialog then shows; Cancel and Esc drop the
    /// changes made in it.
    fn split_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        preferences: &Preferences,
    ) {
        let Some(dialog) = &mut self.split else {
            return;
        };
        let outcome = HeavyDialog::new(
            Id::new("split_dialog"),
            fl!(i18n, "dialog-split-title"),
            fl!(i18n, "dialog-split-done"),
            fl!(i18n, "dialog-cancel"),
            fl!(i18n, "dialog-close"),
            theme,
        )
        .fitted(SplitForm::body_width(ctx))
        .show(ctx, dialog, |ui, form| {
            form.show(ui, i18n, theme, preferences.inline_completion);
        });
        match outcome {
            DialogOutcome::Open => {}
            DialogOutcome::Accepted => {
                let done = match dialog.form.done() {
                    DoneOutcome::Refused => return,
                    DoneOutcome::Unchanged => None,
                    DoneOutcome::Changed(done) => Some(done),
                };
                self.close_dialog(ctx, |window| &mut window.split);
                if let Some(register) = self.register_mut() {
                    register.split_closed(done);
                }
            }
            DialogOutcome::Back | DialogOutcome::Discarded => {
                unreachable!("a heavy Dialog has no Back or Discard button")
            }
            DialogOutcome::Cancelled => {
                self.close_dialog(ctx, |window| &mut window.split);
                if let Some(register) = self.register_mut() {
                    register.split_closed(None);
                }
            }
        }
    }

    /// The "Can't save this transaction" Dialog, while it is open, drawn over any other
    /// Dialog. Continue editing has first focus, and Enter and Esc both press it; only a
    /// click discards. The Register carries out the answer.
    fn cant_save_dialog(&mut self, ctx: &egui::Context, i18n: &Localizer, theme: &Theme) {
        let Some(dialog) = &mut self.cant_save else {
            return;
        };
        let outcome = LightDialog::notice(
            Id::new("cant_save_dialog"),
            fl!(i18n, "dialog-cant-save-continue"),
            theme,
        )
        .with_discard(fl!(i18n, "dialog-cant-save-discard"))
        .show(ctx, dialog, |ui, refusal| {
            ui.heading(fl!(i18n, "dialog-cant-save-title"));
            ui.label(refusal.text(i18n));
        });
        let answer = match outcome {
            DialogOutcome::Open => return,
            DialogOutcome::Accepted | DialogOutcome::Cancelled => RefusalAnswer::ContinueEditing,
            DialogOutcome::Discarded => RefusalAnswer::Discard,
            DialogOutcome::Back => unreachable!("the refusal has no Back button"),
        };
        self.close_dialog(ctx, |window| &mut window.cant_save);
        if let Some(register) = self.register_mut() {
            register.answer_refusal(answer);
        }
    }

    /// The leave-row prompt, while it is open, drawn over any other Dialog. Save, or Add
    /// on the draft row, has first focus, so Enter saves, and Esc continues editing;
    /// only a click discards. Save with "Always save without asking" ticked also turns
    /// the Preference on. The Register carries out the answer.
    fn leave_row_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        preferences: &Preferences,
    ) {
        // PROTOTYPE #462: an Undo or Redo asks with Discard and Continue editing only.
        let undo_only = crate::proto::is(crate::proto::Setting::UndoPrompt, 1)
            && self
                .register()
                .is_some_and(Register::proto_leaving_for_undo);
        let Some(dialog) = &mut self.leave_row else {
            return;
        };
        let (save, message) = match dialog.form.row {
            LeftRow::Draft => (
                fl!(i18n, "dialog-leave-row-add"),
                fl!(i18n, "dialog-leave-row-draft-message"),
            ),
            LeftRow::Transaction => (
                fl!(i18n, "dialog-leave-row-save"),
                fl!(i18n, "dialog-leave-row-message"),
            ),
        };
        let light = if undo_only {
            LightDialog::notice(
                Id::new("leave_row_dialog"),
                fl!(i18n, "dialog-leave-row-continue"),
                theme,
            )
        } else {
            LightDialog::new(
                Id::new("leave_row_dialog"),
                save,
                fl!(i18n, "dialog-leave-row-continue"),
                theme,
            )
        };
        let outcome = light
            .with_discard(fl!(i18n, "dialog-leave-row-discard"))
            .show(ctx, dialog, |ui, form| {
                ui.label(message);
                if !undo_only {
                    ui.checkbox(&mut form.always, fl!(i18n, "dialog-leave-row-always"));
                }
            });
        let outcome = match outcome {
            DialogOutcome::Accepted if undo_only => DialogOutcome::Cancelled,
            outcome => outcome,
        };
        let answer = match outcome {
            DialogOutcome::Open => return,
            DialogOutcome::Accepted => {
                if dialog.form.always {
                    commands.send(AppCommand::SetPreferences(Preferences {
                        save_without_asking: true,
                        ..preferences.clone()
                    }));
                    // The prompt can open over the Preferences Dialog, whose OK would
                    // otherwise put the Preference back.
                    if let Some(open) = &mut self.preferences {
                        open.form.save_without_asking();
                    }
                }
                LeaveAnswer::Save
            }
            DialogOutcome::Cancelled => LeaveAnswer::ContinueEditing,
            DialogOutcome::Discarded => LeaveAnswer::Discard,
            DialogOutcome::Back => unreachable!("the leave-row prompt has no Back button"),
        };
        self.close_dialog(ctx, |window| &mut window.leave_row);
        if let Some(register) = self.register_mut() {
            register.answer_leave(answer);
        }
    }

    /// Go to date, while it is open: a small window around the date field, as wide as
    /// the Register's Date column. Go, or Enter, sends the date and closes it; a box
    /// holding no date says so and stays open.
    fn go_to_date_dialog(
        &mut self,
        ctx: &egui::Context,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        today: Date<Iso>,
    ) {
        let width = self.column_widths.date();
        let Some(dialog) = &mut self.go_to_date else {
            return;
        };
        let outcome = HeavyDialog::new(
            Id::new("go_to_date_dialog"),
            fl!(i18n, "dialog-go-to-date-title"),
            fl!(i18n, "dialog-go-to-date-go"),
            fl!(i18n, "dialog-cancel"),
            fl!(i18n, "dialog-close"),
            theme,
        )
        .fitted(width)
        .show(ctx, dialog, |ui, form| {
            form.show(ui, i18n, theme, today, width);
        });
        match outcome {
            DialogOutcome::Open => return,
            DialogOutcome::Accepted => {
                let Some(date) = dialog.form.submit(i18n) else {
                    return;
                };
                commands.send(AppCommand::GoToDate(date));
            }
            DialogOutcome::Back | DialogOutcome::Discarded => {
                unreachable!("a heavy Dialog has no Back or Discard button")
            }
            DialogOutcome::Cancelled => {}
        }
        self.close_dialog(ctx, |window| &mut window.go_to_date);
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
            DialogOutcome::Back | DialogOutcome::Discarded => {
                unreachable!("a heavy Dialog has no Back or Discard button")
            }
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
            DialogOutcome::Back | DialogOutcome::Discarded => {
                unreachable!("a heavy Dialog has no Back or Discard button")
            }
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
                if let Some(register) = self.register_mut() {
                    register.discard_deleted_row(form.transaction);
                }
                commands.send(AppCommand::Domain(DomainCommand::DeleteTransaction {
                    transaction: form.transaction,
                }));
            }
            DialogOutcome::Cancelled => {}
            DialogOutcome::Back | DialogOutcome::Discarded => {
                unreachable!("the delete confirm has no Back or Discard button")
            }
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
            DialogOutcome::Accepted => commands.send(AppCommand::SaveRow(form.save)),
            DialogOutcome::Cancelled => {
                if let Some(register) = self.register_mut() {
                    register.confirm_cancelled();
                }
            }
            DialogOutcome::Back | DialogOutcome::Discarded => {
                unreachable!("the amount confirm has no Back or Discard button")
            }
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
            DialogOutcome::Back | DialogOutcome::Discarded => {
                unreachable!("the un-reconcile confirm has no Back or Discard button")
            }
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
        recent_files: &RecentFiles,
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
                if recent_files.newest().is_some() {
                    Self::recent_file_items(ui, i18n, commands, &mut drawn, recent_files);
                    ui.separator();
                }
                item(ui, &mut drawn, UserAction::Quit);
            });
            drawn.note_menu(&file.response);
            let edit = ui.menu_button(fl!(i18n, "menu-edit"), |ui| {
                item(ui, &mut drawn, UserAction::Undo);
                item(ui, &mut drawn, UserAction::Redo);
                ui.separator();
                item(ui, &mut drawn, UserAction::GoToDate);
            });
            drawn.note_menu(&edit.response);
            let view_menu = ui.menu_button(fl!(i18n, "menu-view"), |ui| {
                item(ui, &mut drawn, UserAction::SwitchPane);
            });
            drawn.note_menu(&view_menu.response);
            let tools = ui.menu_button(fl!(i18n, "menu-tools"), |ui| {
                item(ui, &mut drawn, UserAction::ManageAccounts);
                item(ui, &mut drawn, UserAction::Preferences);
                ui.separator();
                item(ui, &mut drawn, UserAction::Calculator);
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
        recent_files: &RecentFiles,
    ) {
        for (number, path) in (1_u32..).zip(recent_files.paths()) {
            let file = data_file::file_name(path);
            let item = ui
                .button(fl!(i18n, "menu-file-recent", number = number, file = file))
                .on_hover_text(path.display().to_string());
            drawn.note_item(&item);
            if item.clicked() {
                commands.send(AppCommand::OpenDataFile(path.to_owned()));
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
