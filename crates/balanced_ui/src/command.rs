// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`AppCommand`]s and the [`CommandSender`] that carries them to `App::logic`.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use balanced_core::{AccountId, DomainCommand, DomainError, TransactionId, WriteError};
use balanced_i18n::Localizer;
use eframe::egui;

use crate::data_file::DataFilePicker;
use crate::i18n::fl;
use crate::preferences::Preferences;
use crate::register_columns::RegisterColumns;
use crate::route::Route;

/// An app-level action. UI code sends one instead of changing app or domain state
/// itself, and `App::logic` applies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AppCommand {
    /// Apply a change to the ledger. A rejected change shows in the Status bar.
    Domain(DomainCommand),
    /// Apply the change the open Dialog's form asks for. The Dialog closes once it is
    /// applied, and shows a rejected change inline, staying open.
    SubmitForm(DomainCommand),
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
    /// Open `account`'s Register under the Accounts tab and put the cell cursor on
    /// `transaction`'s row.
    GoToTransaction {
        account: AccountId,
        transaction: TransactionId,
    },
    /// Put the cell cursor on the open Register's draft row, showing the Register,
    /// unless its account is closed.
    GoToDraftRow,
    /// Move keyboard focus from the Account bar to the Main view, or from anywhere
    /// else to the Account bar.
    SwitchPane,
    /// Open a Dialog over the Main window.
    OpenDialog(DialogKind),
    /// Apply a change the open Dialog's form asks for. The Dialog closes once the
    /// change is applied, or shows inline why the ledger refused it.
    SubmitDialog(DomainCommand),
    /// Replace the Preferences with these, show them and write them to the
    /// preferences file. Preferences are not undoable.
    SetPreferences(Preferences),
    /// Replace the Preferences' Register columns with these, show them in every
    /// Register and write them to the preferences file.
    SetRegisterColumns(RegisterColumns),
    /// Show the native picker for a data file to create or open, which sends
    /// `NewDataFile` or `OpenDataFile` once a path is picked.
    ChooseDataFile(DataFilePicker),
    /// Create an empty data file at the path, already picked, and switch to it. A file
    /// already there is replaced, since the native picker has asked to overwrite it,
    /// unless it is the open file. A file that can't be created shows a Dialog saying
    /// why, and the open file stays open.
    NewDataFile(PathBuf),
    /// Open the data file at the path, already picked, and switch to it. A file that
    /// can't be opened shows a Dialog saying why, and the open file stays open.
    OpenDataFile(PathBuf),
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
    /// The New Account wizard, on its first step.
    NewAccount,
    /// Manage Accounts, holding every account as the ledger has it.
    ManageAccounts,
    /// Create a category, starting from the name typed in a Category cell.
    NewCategory { name: String },
    /// The Preferences Dialog, starting from the Preferences in use.
    Preferences,
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
    /// The preferences file couldn't be read, so the app started from the default
    /// Preferences and kept the file as `preferences.toml.broken`.
    PreferencesReset,
    /// The preferences file couldn't be read, so the app started from the default
    /// Preferences, and the file couldn't be kept aside either.
    PreferencesUnreadable,
    /// The Preferences apply, but writing them to the preferences file failed.
    PreferencesNotSaved,
    /// Why the ledger refused a change.
    Rejected(DomainError),
}

impl StatusMessage {
    pub(crate) fn text(&self, i18n: &Localizer) -> String {
        match self {
            Self::About => fl!(i18n, "status-about", version = env!("CARGO_PKG_VERSION")),
            Self::NotAvailableYet => fl!(i18n, "status-not-available-yet"),
            Self::NoSuchCategory => fl!(i18n, "status-no-such-category"),
            Self::PreferencesReset => fl!(i18n, "status-preferences-reset"),
            Self::PreferencesUnreadable => fl!(i18n, "status-preferences-unreadable"),
            Self::PreferencesNotSaved => fl!(i18n, "status-preferences-not-saved"),
            Self::Rejected(error) => match error {
                DomainError::AccountClosed { .. } => fl!(i18n, "status-account-closed"),
                DomainError::TransferAccountClosed { .. } => {
                    fl!(i18n, "status-transfer-account-closed")
                }
                DomainError::TransferToOwnAccount { .. } => {
                    fl!(i18n, "status-transfer-to-own-account")
                }
                DomainError::UnknownTransaction { .. } => {
                    fl!(i18n, "status-unknown-transaction")
                }
                DomainError::BlankAccountName => fl!(i18n, "status-blank-account-name"),
                DomainError::SubGroupEdge { .. } => fl!(i18n, "status-sub-group-edge"),
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
                DomainError::DuplicateCategory { name } => {
                    fl!(i18n, "status-duplicate-category", name = name.clone())
                }
                DomainError::CategoryInUse { .. } => fl!(i18n, "status-category-in-use"),
                DomainError::BlankCategoryName => fl!(i18n, "status-blank-category-name"),
                DomainError::Store(WriteError::AmountOutOfRange(_)) => {
                    fl!(i18n, "status-amount-too-large")
                }
                DomainError::Store(WriteError::Sqlite(_) | WriteError::RollbackFailed { .. }) => {
                    fl!(i18n, "status-change-not-saved")
                }
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
