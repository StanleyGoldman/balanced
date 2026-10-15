//! [`AppCommand`]s and the [`CommandSender`] that carries them to `App::logic`.

use std::sync::mpsc::{self, Receiver, Sender};

use balanced_i18n::Localizer;
use eframe::egui;

use crate::i18n::fl;
use crate::route::Route;

/// An app-level action. UI code sends one instead of changing app or domain state
/// itself, and `App::logic` applies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppCommand {
    /// Show `Route` in the Main view.
    Navigate(Route),
    /// Show a message in the Status bar until the next one replaces it.
    ShowStatus(StatusMessage),
    /// Close the Main window, which ends the app.
    Quit,
}

/// A Status bar message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusMessage {
    /// Balanced's name and version, in answer to Help → About.
    About,
}

impl StatusMessage {
    pub(crate) fn text(self, i18n: &Localizer) -> String {
        match self {
            Self::About => fl!(i18n, "status-about", version = env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Sends [`AppCommand`]s to `App::logic`. Sending requests a repaint, so the
/// Command is applied on the next frame. Clone it to send from anywhere.
#[derive(Debug, Clone)]
pub(crate) struct CommandSender {
    sender: Sender<AppCommand>,
    ctx: egui::Context,
}

impl CommandSender {
    /// A sender that wakes `ctx`, and the receiving end `App::logic` drains.
    pub(crate) fn new(ctx: egui::Context) -> (Self, Receiver<AppCommand>) {
        let (sender, receiver) = mpsc::channel();
        (Self { sender, ctx }, receiver)
    }

    pub(crate) fn send(&self, command: AppCommand) {
        self.sender
            .send(command)
            .expect("the App holds the receiver for as long as any sender exists");
        self.ctx.request_repaint();
    }
}
