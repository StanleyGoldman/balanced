//! The Main window: Menu bar, tab row, Account bar, Main view and Status bar.

use balanced_core::{AccountId, AccountSummary, LedgerView};
use balanced_i18n::Localizer;
use eframe::egui::{self, CentralPanel, Label, MenuBar, Panel, RichText, Sides};

use crate::account_bar::AccountBar;
use crate::accounts_model::AccountsModel;
use crate::command::{AppCommand, CommandSender, StatusMessage};
use crate::i18n::fl;
use crate::route::{Route, TopLevelTab};
use crate::theme::Theme;
use crate::user_action::UserAction;

/// What the Main window shows. None of it outlives the run: every run starts on Home.
#[derive(Debug, Default)]
pub(crate) struct MainWindow {
    tab: TopLevelTab,
    /// The account open under the Accounts tab.
    account: Option<AccountId>,
    status: Option<StatusMessage>,
    account_bar: AccountBar,
    /// The accounts of the last snapshot shown, rebuilt when it is out of date.
    accounts: Option<AccountsModel>,
}

impl MainWindow {
    /// Shows `route` in the Main view.
    pub(crate) fn navigate(&mut self, route: Route) {
        match route {
            Route::Tab(tab) => self.tab = tab,
            Route::Account(id) => {
                self.tab = TopLevelTab::Accounts;
                self.account = Some(id);
            }
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
        let accounts = match self.accounts.take() {
            Some(model) if model.is_current(view, i18n) => model,
            _ => AccountsModel::new(view, i18n),
        };
        Panel::top("menu_bar")
            .frame(theme.chrome_frame())
            .show(ui, |ui| Self::menu_bar(ui, i18n, commands));
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
        self.account_bar
            .show(ui, i18n, theme, commands, &accounts, self.account);
        CentralPanel::default().show(ui, |ui| {
            let open_account = match self.tab {
                TopLevelTab::Accounts => self.account.and_then(|id| view.account(id)),
                TopLevelTab::Home
                | TopLevelTab::Bills
                | TopLevelTab::Spending
                | TopLevelTab::Planning
                | TopLevelTab::Investing
                | TopLevelTab::PropertyDebt => None,
            };
            match open_account {
                Some(summary) => Self::context_header(ui, theme, summary, &accounts),
                None => {
                    ui.label(self.tab.placeholder(i18n));
                }
            }
        });
        self.accounts = Some(accounts);
    }

    /// The open account's name and balance, above its Register. Both clip when the
    /// Main view is narrow.
    // TODO(#85): show the account's Register under the header.
    fn context_header(
        ui: &mut egui::Ui,
        theme: &Theme,
        summary: &AccountSummary,
        accounts: &AccountsModel,
    ) {
        let account = summary.account();
        let balance = accounts
            .balance(account.id())
            .expect("the model formats a balance for every account in its snapshot");
        Sides::new().shrink_left().truncate().show(
            ui,
            |ui| ui.add(Label::new(RichText::new(account.name()).heading())),
            |ui| ui.label(theme.amount(&balance.money, balance.text.clone()).heading()),
        );
    }

    fn menu_bar(ui: &mut egui::Ui, i18n: &Localizer, commands: &CommandSender) {
        MenuBar::new().ui(ui, |ui| {
            ui.menu_button(fl!(i18n, "menu-file"), |ui| {
                Self::menu_item(ui, i18n, commands, UserAction::Quit);
            });
            ui.menu_button(fl!(i18n, "menu-help"), |ui| {
                Self::menu_item(ui, i18n, commands, UserAction::About);
            });
        });
    }

    fn menu_item(
        ui: &mut egui::Ui,
        i18n: &Localizer,
        commands: &CommandSender,
        action: UserAction,
    ) {
        if ui.button(action.label(i18n)).clicked() {
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
