// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`AppCommand`]s and the [`CommandSender`] that carries them to `App::logic`.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use balanced_core::{
    AccountId, DomainCommand, DomainError, LedgerView, Money, TransactionId, WriteError,
};
use balanced_i18n::Localizer;
use eframe::egui;
use icu_calendar::{Date, Iso};

use crate::data_file::DataFilePicker;
use crate::i18n::fl;
use crate::preferences::Preferences;
use crate::register::LeftRow;
use crate::register_columns::RegisterColumns;
use crate::route::Route;
use crate::save_refusal::SaveRefusal;
use crate::split_dialog::SplitStart;

/// An app-level action. UI code sends one instead of changing app or domain state
/// itself, and `App::logic` applies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AppCommand {
    /// Apply a change to the ledger. A rejected change shows in the Status bar.
    Domain(DomainCommand),
    /// Apply the save of a Register row, or the post of its draft row, and hand the
    /// ledger's answer to what the save names.
    SaveRow(RowSave),
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
    /// Open `account`'s Register under the Accounts tab and put the cell cursor on
    /// `transaction`'s row.
    GoToTransaction {
        account: AccountId,
        transaction: TransactionId,
    },
    /// Put the cell cursor on the open Register's draft row, showing the Register,
    /// unless its account is closed.
    GoToDraftRow,
    /// Put the open Register's cell cursor on the first row dated on or after the
    /// date, or on its last row when none is, and scroll that row into view.
    GoToDate(Date<Iso>),
    /// Move keyboard focus from the Account bar to the Main view, or from anywhere
    /// else to the Account bar.
    SwitchPane,
    /// Open the Calculator over the Main view, unless it is already open.
    OpenCalculator,
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

/// The save of a Register row, or the post of its draft row, with what the ledger's
/// answer to it goes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RowSave {
    pub(crate) command: DomainCommand,
    pub(crate) answer: AnswerTo,
}

/// What the ledger's answer to the save of a Register row goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnswerTo {
    /// The Register, which waits on it: an applied save moves on, and a refused one
    /// says why in the "Can't save this transaction" Dialog.
    Register,
    /// The Status bar, as nothing waits on it: a refused save shows there, and an
    /// applied one changes nothing more. A row's save sent as another row's changes
    /// start goes here.
    StatusBar,
}

/// A Dialog the Main window can open, with what it opens prefilled with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DialogKind {
    /// The confirm for deleting the Register's selected transaction. Nothing opens
    /// while no transaction is selected, or while its account is closed.
    DeleteTransaction,
    /// The confirm for changing a Reconciled transaction's amount to `amount`, from a
    /// save of its row, `save`, that is not sent until it is accepted.
    ChangeReconciledAmount {
        transaction: TransactionId,
        amount: Money,
        save: RowSave,
    },
    /// The confirm for setting a Reconciled transaction back to Cleared. Nothing opens
    /// unless the open Register shows the transaction as Reconciled, or while its
    /// account is closed.
    Unreconcile { transaction: TransactionId },
    /// The New Account wizard, on its first step.
    NewAccount,
    /// Manage Accounts, holding every account as the ledger has it.
    ManageAccounts,
    /// The Category List, showing every category and its usage as the ledger has them.
    CategoryList,
    /// Create a category, starting from the name typed in a Category cell.
    NewCategory { name: String },
    /// The Preferences Dialog, starting from the Preferences in use.
    Preferences,
    /// Go to date, starting on the Register's selected row's date, or today. Nothing
    /// opens while no Register is shown.
    GoToDate,
    /// The leave-row prompt, asking whether to save the changed row being left, of the
    /// kind given. It opens over any other Dialog, since closing the window can ask.
    LeaveRow(LeftRow),
    /// The "Can't save this transaction" Dialog, saying why a save of the Register's
    /// editing row was refused. It opens over any other Dialog, since the leave-row
    /// prompt's Save can be refused.
    CantSave(SaveRefusal),
    /// The Split dialog, on the lines of a row of the open Register.
    Split(SplitStart),
}

/// A Status bar message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StatusMessage {
    /// Balanced's name and version, in answer to Help → About.
    About,
    /// The answer to a command whose feature isn't built yet.
    NotAvailableYet,
    /// Why the Calculator's Paste changed nothing: no Payment or Deposit cell has been
    /// edited in the Register shown.
    NoPasteTarget,
    /// Why the Calculator's Paste changed nothing: its sum isn't an amount.
    PasteNotAnAmount,
    /// Why a Category cell refused its typed text: no category has that name.
    NoSuchCategory,
    /// Why the editing row's changes were lost as its Register went: they change a
    /// Reconciled transaction's amount, which needs the confirm, and the confirm can't
    /// open over a Register that is going.
    ReconciledAmountNotSaved,
    /// The preferences file couldn't be read, so the app started from the default
    /// Preferences and kept the file as `preferences.toml.broken`.
    PreferencesReset,
    /// The preferences file couldn't be read, so the app started from the default
    /// Preferences, and the file couldn't be kept aside either.
    PreferencesUnreadable,
    /// The Preferences apply, but writing them to the preferences file failed.
    PreferencesNotSaved,
    /// The data file just opened isn't listed in Recent files, since the file that
    /// keeps them can't store its path.
    NotListedInRecentFiles,
    /// Writing the Recent files to the recent files file failed.
    RecentFilesNotSaved,
    /// Why the ledger refused a change.
    Rejected(DomainError),
    /// Why the ledger refused to change the date or amount of a split line's other side,
    /// or to delete it: those change only from the split, in the account named
    /// `split_account`.
    SplitLineOtherSide { split_account: String },
}

impl StatusMessage {
    /// Why the ledger refused a change with `error`, naming from `view` the account the
    /// refusal points to.
    pub(crate) fn refusal(error: DomainError, view: &LedgerView) -> Self {
        if let DomainError::SplitLineOtherSide { account } = error
            && let Some(summary) = view.account(account)
        {
            return Self::SplitLineOtherSide {
                split_account: summary.account().name().to_owned(),
            };
        }
        Self::Rejected(error)
    }

    pub(crate) fn text(&self, i18n: &Localizer) -> String {
        match self {
            Self::About => fl!(i18n, "status-about", version = env!("CARGO_PKG_VERSION")),
            Self::NotAvailableYet => fl!(i18n, "status-not-available-yet"),
            Self::NoPasteTarget => fl!(i18n, "status-no-paste-target"),
            Self::PasteNotAnAmount => fl!(i18n, "status-paste-not-an-amount"),
            Self::NoSuchCategory => fl!(i18n, "status-no-such-category"),
            Self::ReconciledAmountNotSaved => fl!(i18n, "status-reconciled-amount-not-saved"),
            Self::PreferencesReset => fl!(i18n, "status-preferences-reset"),
            Self::PreferencesUnreadable => fl!(i18n, "status-preferences-unreadable"),
            Self::PreferencesNotSaved => fl!(i18n, "status-preferences-not-saved"),
            Self::NotListedInRecentFiles => fl!(i18n, "status-not-listed-in-recent-files"),
            Self::RecentFilesNotSaved => fl!(i18n, "status-recent-files-not-saved"),
            Self::SplitLineOtherSide { split_account } => fl!(
                i18n,
                "status-split-line-other-side-in",
                account = split_account.clone()
            ),
            Self::Rejected(error) => match error {
                DomainError::AccountClosed { .. } => fl!(i18n, "status-account-closed"),
                DomainError::TransferAccountClosed { .. } => {
                    fl!(i18n, "status-transfer-account-closed")
                }
                DomainError::TransferToOwnAccount { .. } => {
                    fl!(i18n, "status-transfer-to-own-account")
                }
                DomainError::SplitLineOtherSide { .. } => {
                    fl!(i18n, "status-split-line-other-side")
                }
                DomainError::AccountHasTransfers { .. } => {
                    fl!(i18n, "status-account-has-transfers")
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
                DomainError::CategoryUnderItself { .. } => {
                    fl!(i18n, "status-category-under-itself")
                }
                DomainError::SubcategoryType { .. } => fl!(i18n, "status-subcategory-type"),
                DomainError::ReassignIntoItself { .. } => {
                    fl!(i18n, "status-reassign-into-itself")
                }
                DomainError::NotInCategory { .. } => fl!(i18n, "status-not-in-category"),
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

    /// A sender to a channel of its own, waking the same `ctx`, and its receiving end,
    /// for Commands the App applies at once rather than on the next frame.
    pub(crate) fn with_own_channel(&self) -> (Self, Receiver<AppCommand>) {
        Self::new(self.ctx.clone())
    }

    pub(crate) fn send(&self, command: AppCommand) {
        self.sender
            .send(command)
            .expect("the App holds the receiver for as long as any sender exists");
        self.ctx.request_repaint();
    }
}
