//! [`App`]: the eframe application, which owns the ledger and the Main window.

use std::sync::mpsc::Receiver;

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use eframe::egui;

use crate::command::{AppCommand, CommandSender};
use crate::i18n::fl;
use crate::main_window::MainWindow;
use crate::theme::Theme;

/// Balanced's eframe application.
#[derive(Debug)]
pub struct App {
    i18n: Localizer,
    #[expect(
        dead_code,
        reason = "the Account bar (#84) is the first reader of its snapshot"
    )]
    ledger: Ledger,
    theme: Theme,
    commands: CommandSender,
    received: Receiver<AppCommand>,
    main_window: MainWindow,
}

impl App {
    /// The App over `ledger`, showing text through `i18n`.
    pub fn new(cc: &eframe::CreationContext<'_>, i18n: Localizer, ledger: Ledger) -> Self {
        let theme = Theme::default();
        cc.egui_ctx.set_global_style(theme.style());
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::Title(fl!(
                &i18n,
                "main-window-title"
            )));
        let (commands, received) = CommandSender::new(cc.egui_ctx.clone());
        Self {
            i18n,
            ledger,
            theme,
            commands,
            received,
            main_window: MainWindow::default(),
        }
    }

    fn apply(&mut self, ctx: &egui::Context, command: AppCommand) {
        match command {
            AppCommand::Navigate(route) => self.main_window.navigate(route),
            AppCommand::ShowStatus(message) => self.main_window.show_status(message),
            AppCommand::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }
}

impl eframe::App for App {
    /// Applies every Command sent since the last frame.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(command) = self.received.try_recv() {
            self.apply(ctx, command);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.main_window
            .show(ui, &self.i18n, &self.theme, &self.commands);
    }
}
