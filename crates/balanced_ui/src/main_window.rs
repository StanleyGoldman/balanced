//! The Main window: Menu bar, tab row, Account bar, Main view and Status bar.

use balanced_core::{AccountSummary, LedgerView};
use balanced_i18n::Localizer;
use eframe::egui::{self, Button, CentralPanel, Label, MenuBar, Panel, RichText, Sides};

use crate::account_bar::AccountBar;
use crate::accounts_model::AccountsModel;
use crate::command::{AppCommand, CommandSender, StatusMessage};
use crate::i18n::fl;
use crate::keymap::{self, KEYMAP};
use crate::layout::Layout;
use crate::register::ColumnWidths;
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

    /// Whether `action` can act now. Its menu item is disabled while it can't.
    fn can_act(&self, action: UserAction) -> bool {
        match action {
            UserAction::CloseRegisterTab => self.shows_registers(),
            UserAction::NextRegisterTab | UserAction::PreviousRegisterTab => {
                self.shows_registers() && self.registers.len() > 1
            }
            UserAction::Quit | UserAction::About => true,
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

    /// Hands a Tab press, with `modifiers` held, to the current Register.
    pub(crate) fn press_tab(&mut self, modifiers: egui::Modifiers) {
        if let Some(register) = self.registers.current_register_mut() {
            register.press_tab(modifiers);
        }
    }

    /// Shows `message` in the Status bar.
    pub(crate) fn show_status(&mut self, message: StatusMessage) {
        self.status = Some(message);
    }

    /// Draws the Main window's panels from `view`, outer ones first and the Main view
    /// last.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        view: &LedgerView,
    ) {
        for action in keymap::dispatch(ui.ctx(), KEYMAP) {
            commands.send(action.command());
        }
        let accounts = match self.accounts.take() {
            Some(model) if model.is_current(view, i18n) => model,
            _ => AccountsModel::new(view, i18n),
        };
        Panel::top("menu_bar")
            .frame(theme.chrome_frame())
            .show(ui, |ui| self.menu_bar(ui, i18n, commands));
        Panel::bottom("status_bar")
            .frame(theme.chrome_frame())
            .show(ui, |ui| {
                // An empty label keeps the Status bar one line tall.
                let text = self.status.map(|message| message.text(i18n));
                ui.label(text.unwrap_or_default());
            });
        Panel::top("tab_row")
            .frame(theme.chrome_frame())
            .show(ui, |ui| self.tab_row(ui, i18n, commands));
        self.account_bar.show(
            ui,
            i18n,
            theme,
            commands,
            &accounts,
            self.registers.current_account(),
        );
        CentralPanel::default().show(ui, |ui| {
            let open_account = match self.tab {
                TopLevelTab::Accounts => {
                    self.registers.show_strip(ui, i18n, view);
                    self.registers
                        .current_account()
                        .and_then(|id| view.account(id))
                }
                TopLevelTab::Home
                | TopLevelTab::Bills
                | TopLevelTab::Spending
                | TopLevelTab::Planning
                | TopLevelTab::Investing
                | TopLevelTab::PropertyDebt => None,
            };
            match open_account {
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
                        );
                    }
                }
                None => {
                    ui.label(self.tab.placeholder(i18n));
                }
            }
        });
        self.accounts = Some(accounts);
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
                    // TODO(#93): go to the draft row.
                    commands.send(AppCommand::ShowStatus(StatusMessage::NotAvailableYet));
                }
            },
        );
    }

    fn menu_bar(&self, ui: &mut egui::Ui, i18n: &Localizer, commands: &CommandSender) {
        MenuBar::new().ui(ui, |ui| {
            ui.menu_button(fl!(i18n, "menu-file"), |ui| {
                self.menu_item(ui, i18n, commands, UserAction::CloseRegisterTab);
                ui.separator();
                self.menu_item(ui, i18n, commands, UserAction::Quit);
            });
            ui.menu_button(fl!(i18n, "menu-view"), |ui| {
                self.menu_item(ui, i18n, commands, UserAction::NextRegisterTab);
                self.menu_item(ui, i18n, commands, UserAction::PreviousRegisterTab);
            });
            ui.menu_button(fl!(i18n, "menu-help"), |ui| {
                self.menu_item(ui, i18n, commands, UserAction::About);
            });
        });
    }

    /// The menu item for `action`, with its Shortcut text, disabled while the action
    /// can't act.
    fn menu_item(
        &self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        commands: &CommandSender,
        action: UserAction,
    ) {
        let mut button = egui::Button::new(action.label(i18n));
        if let Some(chord) = keymap::chord(action) {
            button = button.shortcut_text(keymap::shortcut_text(chord, i18n, ui.ctx().os()));
        }
        if ui.add_enabled(self.can_act(action), button).clicked() {
            commands.send(action.command());
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
