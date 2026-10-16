//! [`AppCommand`]s and the [`CommandSender`] that carries them to `App::logic`.

use std::sync::mpsc::{self, Receiver, Sender};

use balanced_core::{DomainCommand, DomainError, TransactionId};
use balanced_i18n::Localizer;
use eframe::egui;

use crate::i18n::fl;
use crate::route::Route;

/// An app-level action. UI code sends one instead of changing app or domain state
/// itself, and `App::logic` applies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AppCommand {
    /// Apply a change to the ledger. A rejected change shows in the Status bar.
    Domain(DomainCommand),
    /// Reverse the latest ledger change not yet undone. A refused reversal shows in
    /// the Status bar.
    Undo,
    /// Make the latest undone ledger change again. A refused change shows in the
    /// Status bar.
    Redo,
    /// Show `Route` in the Main view.
    Navigate(Route),
    /// Show the split lines of a split transaction.
    OpenSplitView(TransactionId),
    /// Close the current Register tab.
    CloseRegisterTab,
    /// Make the Register tab right of the current one current, wrapping to the first.
    NextRegisterTab,
    /// Make the Register tab left of the current one current, wrapping to the last.
    PreviousRegisterTab,
    /// Move keyboard focus from the Account bar to the Main view, or from anywhere
    /// else to the Account bar.
    SwitchPane,
    /// Open a Dialog over the Main window.
    OpenDialog(DialogKind),
    /// Show a message in the Status bar until the next one replaces it.
    ShowStatus(StatusMessage),
    /// Close the Main window, which ends the app.
    Quit,
}

/// A Dialog the Main window can open, with what it opens prefilled with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DialogKind {
    /// The confirm for deleting the Register's selected transaction. Nothing opens
    /// while no transaction is selected, or while its account is closed.
    DeleteTransaction,
    /// Create a category, starting from the name typed in a Category cell.
    NewCategory { name: String },
}

/// A Status bar message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StatusMessage {
    /// Balanced's name and version, in answer to Help → About.
    About,
    /// The answer to a command whose feature isn't built yet.
    NotAvailableYet,
    /// Why a Category cell refused its typed text: no category has that name.
    NoSuchCategory,
    /// Why the ledger refused a change.
    Rejected(DomainError),
}

impl StatusMessage {
    pub(crate) fn text(&self, i18n: &Localizer) -> String {
        match self {
            Self::About => fl!(i18n, "status-about", version = env!("CARGO_PKG_VERSION")),
            Self::NotAvailableYet => fl!(i18n, "status-not-available-yet"),
            Self::NoSuchCategory => fl!(i18n, "status-no-such-category"),
            Self::Rejected(error) => match error {
                DomainError::AccountClosed { .. } => fl!(i18n, "status-account-closed"),
                DomainError::UnknownTransaction { .. } => {
                    fl!(i18n, "status-unknown-transaction")
                }
                DomainError::BlankAccountName => fl!(i18n, "status-blank-account-name"),
                DomainError::OpeningBalancePayee { .. } => {
                    fl!(i18n, "status-opening-balance-payee")
                }
                DomainError::UnknownAccount { .. } => fl!(i18n, "status-unknown-account"),
                DomainError::Money(_) => fl!(i18n, "status-amount-too-large"),
                DomainError::NotInOneCategory { .. } => {
                    fl!(i18n, "status-not-in-one-category")
                }
                DomainError::UnknownCategory { .. } => fl!(i18n, "status-unknown-category"),
                DomainError::SplitAmount { .. } => fl!(i18n, "status-split-amount"),
                DomainError::CurrencyMismatch { .. } => fl!(i18n, "status-currency-mismatch"),
            },
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
