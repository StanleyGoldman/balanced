//! The Main window: Menu bar, tab row, Account bar, Main view and Status bar.

use balanced_i18n::Localizer;
use eframe::egui::{self, CentralPanel, MenuBar, Panel};

use crate::command::{AppCommand, CommandSender, StatusMessage};
use crate::i18n::fl;
use crate::route::{Route, TopLevelTab};
use crate::theme::Theme;
use crate::user_action::UserAction;

/// What the Main window shows. None of it outlives the run: every run starts on Home.
#[derive(Debug, Default)]
pub(crate) struct MainWindow {
    tab: TopLevelTab,
    status: Option<StatusMessage>,
}

impl MainWindow {
    /// Shows `route` in the Main view.
    pub(crate) fn navigate(&mut self, route: Route) {
        match route {
            Route::Tab(tab) => self.tab = tab,
        }
    }

    /// Shows `message` in the Status bar.
    pub(crate) fn show_status(&mut self, message: StatusMessage) {
        self.status = Some(message);
    }

    /// Draws the Main window's panels, outer ones first and the Main view last.
    pub(crate) fn show(
        &self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
    ) {
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
        Panel::left("account_bar")
            .frame(theme.chrome_frame())
            .show(ui, |_| {});
        CentralPanel::default().show(ui, |ui| {
            ui.label(self.tab.placeholder(i18n));
        });
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
