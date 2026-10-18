//! The Main window: Menu bar, tab row, Account bar, Main view and Status bar.

use balanced_core::{AccountSummary, DomainCommand, DomainError, LedgerView, TransactionId};
use balanced_i18n::Localizer;
use eframe::egui::accesskit::Role;
use eframe::egui::{self, Button, CentralPanel, Id, Label, MenuBar, Panel, RichText, Sides};
use icu_calendar::{Date, Iso};

use crate::account_bar::{AccountBar, BarFrame};
use crate::accounts_model::AccountsModel;
use crate::command::{AppCommand, CommandSender, DialogKind, StatusMessage};
use crate::dialog::{DialogOutcome, FocusReturn, HeavyDialog, LightDialog, OpenDialog};
use crate::i18n::fl;
use crate::keymap::{self, KEYMAP};
use crate::layout::Layout;
use crate::manage_accounts::{self, ManageAccountsForm, ManageAccountsOutcome};
use crate::menu_nav::{DrawnMenus, MenuNav};
use crate::new_account::{self, NewAccountForm, WizardOutcome};
use crate::new_category::NewCategoryForm;
use crate::preferences::{NetWorthPlacement, Preferences};
use crate::preferences_dialog::PreferencesForm;
use crate::register::ColumnWidths;
use crate::register_model::RegisterRow;
use crate::register_tabs::RegisterTabs;
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
    /// The Registers open under the Accounts tab.
    registers: RegisterTabs,
    /// The column widths every Register shares.
    column_widths: ColumnWidths,
    /// The delete transaction confirm, while it is open.
    delete_transaction: Option<OpenDialog<DeleteTransactionForm>>,
    /// The New Account wizard, while it is open.
    new_account: Option<OpenDialog<NewAccountForm>>,
    /// The New Category Dialog, while it is open.
    new_category: Option<OpenDialog<NewCategoryForm>>,
    /// Manage Accounts, while it is open.
    manage_accounts: Option<OpenDialog<ManageAccountsForm>>,
    /// The Preferences Dialog, while it is open.
    preferences: Option<OpenDialog<PreferencesForm>>,
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
}

/// The delete transaction confirm's form: which transaction it deletes. Its question
/// is formatted every frame from the Register's row, so it follows the locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeleteTransactionForm {
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
            registers: RegisterTabs::default(),
            column_widths: layout.column_widths(),
            delete_transaction: None,
            new_account: None,
            new_category: None,
            manage_accounts: None,
            preferences: None,
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
            Route::Account { account, open_in } => {
                self.tab = TopLevelTab::Accounts;
                self.registers.open(account, open_in);
            }
        }
    }

    /// Applies `change` to the Register tabs, if the Accounts tab shows them.
    pub(crate) fn change_register_tabs(&mut self, change: impl FnOnce(&mut RegisterTabs)) {
        if self.shows_registers() {
            change(&mut self.registers);
        }
    }

    /// Whether the Main view shows the Register tabs: the Accounts tab is active and
    /// a tab is open.
    fn shows_registers(&self) -> bool {
        self.tab == TopLevelTab::Accounts && self.registers.len() > 0
    }

    /// Whether `action` can act now on `view`. Its menu item is disabled while it
    /// can't.
    fn can_act(&self, action: UserAction, view: &LedgerView) -> bool {
        match action {
            UserAction::Undo => view.can_undo(),
            UserAction::Redo => view.can_redo(),
            UserAction::CloseRegisterTab
            | UserAction::NewTransaction
            | UserAction::DeleteTransaction => self.shows_registers(),
            UserAction::NextRegisterTab | UserAction::PreviousRegisterTab => {
                self.shows_registers() && self.registers.len() > 1
            }
            UserAction::SwitchPane
            | UserAction::NewAccount
            | UserAction::ManageAccounts
            | UserAction::Quit
            | UserAction::Preferences
            | UserAction::About => true,
        }
    }

    /// Whether the current Register takes Tab this frame: it is shown and owns the
    /// keyboard.
    pub(crate) fn register_takes_tab(&self, ctx: &egui::Context) -> bool {
        self.tab == TopLevelTab::Accounts
            && self
                .registers
                .current_register()
                .is_some_and(|register| register.owns_keyboard(ctx))
    }

    /// Takes the arrow keys out of `raw_input` while a menu is open, for the Menu bar's
    /// keyboard navigation.
    pub(crate) fn take_menu_arrows(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.menu_nav.take_arrows(ctx, raw_input);
    }

    /// Hands a Tab press, with `modifiers` held, to the current Register.
    pub(crate) fn press_tab(&mut self, modifiers: egui::Modifiers) {
        if let Some(register) = self.registers.current_register_mut() {
            register.press_tab(modifiers);
        }
    }

    /// Moves keyboard focus from the Account bar to the Main view, or from anywhere
    /// else to the Account bar.
    pub(crate) fn switch_pane(&mut self, ctx: &egui::Context) {
        if self.account_bar.has_focus() {
            // The Main view takes keys with no widget focused, as the Register does.
            ctx.memory_mut(|memory| {
                if let Some(id) = memory.focused() {
                    memory.surrender_focus(id);
                }
            });
        } else {
            self.account_bar.request_focus();
        }
    }

    /// Shows the current Register and puts its cell cursor on the draft row, with the
    /// keyboard, unless no Register is open or its account in `view` is closed.
    pub(crate) fn go_to_draft_row(&mut self, ctx: &egui::Context, view: &LedgerView) {
        let open = self
            .registers
            .current_account()
            .and_then(|account| view.account(account))
            .is_some_and(|summary| !summary.account().is_closed());
        let Some(register) = self.registers.current_register_mut().filter(|_| open) else {
            return;
        };
        self.tab = TopLevelTab::Accounts;
        register.go_to_draft_row();
        // The Register takes keys with no widget focused.
        ctx.memory_mut(|memory| {
            if let Some(id) = memory.focused() {
                memory.surrender_focus(id);
            }
        });
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
                    .registers
                    .current_register()
                    .and_then(|register| register.selected_row(summary.account().id()))
                    .and_then(RegisterRow::transaction)
                else {
                    return;
                };
                let form = DeleteTransactionForm { transaction };
                self.delete_transaction = Some(OpenDialog::new(ctx, form));
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
        if let (Some(path), Some(register)) = (path, self.registers.current_register_mut()) {
            register.category_created(path);
        }
    }

    /// Whether a Dialog is open over the Main window.
    fn dialog_open(&self) -> bool {
        self.delete_transaction.is_some()
            || self.new_account.is_some()
            || self.new_category.is_some()
            || self.manage_accounts.is_some()
            || self.preferences.is_some()
    }

    /// The account the Main view shows, if it shows one.
    fn open_account<'a>(&self, view: &'a LedgerView) -> Option<&'a AccountSummary> {
        match self.tab {
            TopLevelTab::Accounts => self
                .registers
                .current_account()
                .and_then(|id| view.account(id)),
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
                if let Some(register) = self.registers.current_register_mut() {
                    register.commit_edit(ui.ctx(), i18n, commands);
                }
            }
            for action in actions {
                commands.send(action.command());
            }
        }
        let accounts = match self.accounts.take() {
            Some(model) if model.is_current(view, i18n) => model,
            _ => AccountsModel::new(view, i18n),
        };
        Panel::top("menu_bar")
            .frame(theme.chrome_frame())
            .show(ui, |ui| {
                let drawn = self.menu_bar(ui, i18n, commands, view);
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
            selected: self.registers.current_account(),
            preferences,
        };
        self.account_bar.show(ui, &bar_frame);
        CentralPanel::default().show(ui, |ui| {
            if self.tab == TopLevelTab::Accounts {
                self.registers.show_strip(ui, i18n, view);
            }
            match self.open_account(view) {
                Some(summary) => {
                    Self::context_header(ui, i18n, theme, commands, summary, &accounts);
                    if let Some(register) = self.registers.current_register_mut() {
                        register.show(
                            ui,
                            i18n,
                            theme,
                            commands,
                            summary,
                            view,
                            &mut self.column_widths,
                            &preferences.register_columns,
                            today,
                        );
                    }
                }
                None => {
                    ui.label(self.tab.placeholder(i18n));
                }
            }
        });
        self.accounts = Some(accounts);
        self.delete_transaction_dialog(ui.ctx(), i18n, theme, commands, view);
        self.new_account_dialog(ui.ctx(), i18n, theme, commands, today);
        self.new_category_dialog(ui.ctx(), i18n, theme, commands, view);
        self.preferences_dialog(ui.ctx(), i18n, theme, commands);
        self.manage_accounts_dialog(ui.ctx(), i18n, theme, commands, view);
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
                if let Some(register) = self.registers.current_register_mut() {
                    register.category_cancelled();
                }
            }
        }
    }

    /// The delete transaction confirm, while it is open. Delete has first focus, so
    /// Enter deletes.
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
        // The Register's rows can't change while the Dialog blocks it, so the row is
        // there for as long as the Dialog is open.
        let question = self
            .open_account(view)
            .and_then(|summary| {
                self.registers
                    .current_register()?
                    .row(summary.account().id(), form.transaction)
            })
            .map(|row| {
                fl!(
                    i18n,
                    "dialog-delete-transaction-message",
                    payee = row.payee(),
                    date = row.date(),
                    amount = row.amount_text()
                )
            })
            .expect("the Register shows the transaction the confirm asks about");
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

    /// The open account's name and balance above its Register, with + Transaction. A
    /// closed account's header says it is read-only, and + Transaction is disabled. The
    /// name and balance clip when the Main view is narrow.
    fn context_header(
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        summary: &AccountSummary,
        accounts: &AccountsModel,
    ) {
        let account = summary.account();
        let balance = accounts
            .balance(account.id())
            .expect("the model formats a balance for every account in its snapshot");
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
                ui.label(balance.styled(theme).heading());
                let add = Button::new(fl!(i18n, "register-new-transaction"));
                if ui.add_enabled(!closed, add).clicked() {
                    commands.send(AppCommand::GoToDraftRow);
                }
            },
        );
    }

    /// Draws the Menu bar, and returns what it drew for the keyboard navigation.
    fn menu_bar(
        &self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        commands: &CommandSender,
        view: &LedgerView,
    ) -> DrawnMenus {
        let item = |ui: &mut egui::Ui, drawn: &mut DrawnMenus, action| {
            self.menu_item(ui, i18n, commands, view, drawn, action);
        };
        let mut drawn = DrawnMenus::default();
        MenuBar::new().ui(ui, |ui| {
            let file = ui.menu_button(fl!(i18n, "menu-file"), |ui| {
                item(ui, &mut drawn, UserAction::NewAccount);
                ui.separator();
                item(ui, &mut drawn, UserAction::CloseRegisterTab);
                ui.separator();
                item(ui, &mut drawn, UserAction::Quit);
            });
            drawn.note_menu(&file.response);
            let edit = ui.menu_button(fl!(i18n, "menu-edit"), |ui| {
                item(ui, &mut drawn, UserAction::Undo);
                item(ui, &mut drawn, UserAction::Redo);
            });
            drawn.note_menu(&edit.response);
            let view_menu = ui.menu_button(fl!(i18n, "menu-view"), |ui| {
                item(ui, &mut drawn, UserAction::NextRegisterTab);
                item(ui, &mut drawn, UserAction::PreviousRegisterTab);
                ui.separator();
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

    /// The menu item for `action`, with its Shortcut text, disabled while the action
    /// can't act on `view`.
    fn menu_item(
        &self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        commands: &CommandSender,
        view: &LedgerView,
        drawn: &mut DrawnMenus,
        action: UserAction,
    ) {
        let mut button = egui::Button::new(action.label(i18n));
        if let Some(chord) = keymap::chord(action) {
            button = button.shortcut_text(keymap::shortcut_text(chord, i18n, ui.ctx().os()));
        }
        let item = ui.add_enabled(self.can_act(action, view), button);
        drawn.note_item(&item);
        if item.clicked() {
            commands.send(action.command());
            // egui closes a menu on a pointer click only, not on Enter.
            ui.close();
        }
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
