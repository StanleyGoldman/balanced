//! [`App`]: the eframe application, which owns the ledger and the Main window.

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use eframe::egui;

use crate::command::{AppCommand, CommandSender, StatusMessage};
use crate::i18n::fl;
use crate::main_window::MainWindow;
use crate::register_tabs::RegisterTabs;
use crate::theme::Theme;

/// Balanced's eframe application.
#[derive(Debug)]
pub struct App {
    i18n: Localizer,
    ledger: Ledger,
    theme: Theme,
    commands: CommandSender,
    command_receiver: Receiver<AppCommand>,
    main_window: MainWindow,
}

impl App {
    /// The App over `ledger`, showing text through `i18n`.
    pub fn new(cc: &eframe::CreationContext<'_>, i18n: Localizer, ledger: Ledger) -> Self {
        Self::with_theme(cc, i18n, ledger, Theme::default())
    }

    /// The App over `ledger`, showing text through `i18n` in `theme`.
    pub(crate) fn with_theme(
        cc: &eframe::CreationContext<'_>,
        i18n: Localizer,
        ledger: Ledger,
        theme: Theme,
    ) -> Self {
        // Balanced's Scheme, not the OS, picks light or dark, so egui's light and dark
        // styles are both the theme's style.
        let style = Arc::new(theme.style());
        cc.egui_ctx.set_style_of(egui::Theme::Light, style.clone());
        cc.egui_ctx.set_style_of(egui::Theme::Dark, style);
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::Title(fl!(
                &i18n,
                "main-window-title"
            )));
        let (commands, command_receiver) = CommandSender::new(cc.egui_ctx.clone());
        Self {
            i18n,
            ledger,
            theme,
            commands,
            command_receiver,
            main_window: MainWindow::default(),
        }
    }

    fn apply(&mut self, ctx: &egui::Context, command: AppCommand) {
        match command {
            AppCommand::Domain(command) => {
                if let Err(error) = self.ledger.apply(command) {
                    self.main_window.show_status(StatusMessage::Rejected(error));
                }
            }
            AppCommand::Navigate(route) => self.main_window.navigate(route),
            AppCommand::CloseRegisterTab => self
                .main_window
                .change_register_tabs(RegisterTabs::close_current),
            AppCommand::NextRegisterTab => {
                self.main_window.change_register_tabs(RegisterTabs::next);
            }
            AppCommand::PreviousRegisterTab => self
                .main_window
                .change_register_tabs(RegisterTabs::previous),
            AppCommand::ShowStatus(message) => self.main_window.show_status(message),
            AppCommand::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }
}

impl eframe::App for App {
    /// Applies every Command sent since the last frame.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(command) = self.command_receiver.try_recv() {
            self.apply(ctx, command);
        }
    }

    /// Takes Tab away from egui's focus navigation while the Register owns the
    /// keyboard, and hands it to the Register.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if self.main_window.register_takes_tab(ctx) {
            raw_input.events.retain(|event| {
                let egui::Event::Key {
                    key: egui::Key::Tab,
                    pressed,
                    modifiers,
                    ..
                } = event
                else {
                    return true;
                };
                if *pressed {
                    self.main_window.press_tab(*modifiers);
                }
                false
            });
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let view = self.ledger.snapshot();
        self.main_window
            .show(ui, &self.i18n, &self.theme, &self.commands, &view);
    }
}
