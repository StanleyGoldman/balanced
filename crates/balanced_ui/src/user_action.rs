// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The user-action list: what the Menu bar and the keymap offer, each action's label
//! and the [`AppCommand`] it sends.

use balanced_i18n::Localizer;

use crate::command::{AppCommand, DialogKind, StatusMessage};
use crate::data_file::DataFilePicker;
use crate::i18n::fl;

/// Something the user can do from the Menu bar or with a Shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UserAction {
    Undo,
    Redo,
    /// Move keyboard focus between the Account bar and the Main view.
    SwitchPane,
    /// Start a new transaction in the open Register.
    NewTransaction,
    /// Delete the Register's selected transaction, once confirmed.
    DeleteTransaction,
    /// Open Go to date, which moves the Register's cell cursor to a date.
    GoToDate,
    /// Pick a path and create an empty data file there.
    NewDataFile,
    /// Pick a data file and open it.
    OpenDataFile,
    /// Open the New Account wizard.
    NewAccount,
    /// Open Manage Accounts.
    ManageAccounts,
    /// Open the Category List.
    CategoryList,
    Quit,
    /// Open the Preferences Dialog.
    Preferences,
    /// Open the Calculator.
    Calculator,
    About,
    /// Reconcile the open account. It reports that it isn't available yet.
    Reconcile,
    /// Open Manage Accounts from the open account's Account Actions menu.
    EditAccountDetails,
    /// Open the Preferences Dialog from the open account's Account Actions menu.
    RegisterPreferences,
    /// Print the open Register. It reports that it isn't available yet.
    PrintRegister,
    /// Export the open Register to a CSV file. It reports that it isn't available
    /// yet.
    ExportToCsv,
}

impl UserAction {
    /// The action's menu item text.
    pub(crate) fn label(self, i18n: &Localizer) -> String {
        match self {
            Self::Undo => fl!(i18n, "menu-edit-undo"),
            Self::Redo => fl!(i18n, "menu-edit-redo"),
            Self::SwitchPane => fl!(i18n, "menu-view-switch-pane"),
            Self::NewTransaction => fl!(i18n, "menu-new-transaction"),
            Self::DeleteTransaction => fl!(i18n, "menu-delete-transaction"),
            Self::GoToDate => fl!(i18n, "menu-edit-go-to-date"),
            Self::NewDataFile => fl!(i18n, "menu-file-new-data-file"),
            Self::OpenDataFile => fl!(i18n, "menu-file-open-data-file"),
            Self::NewAccount => fl!(i18n, "menu-file-new-account"),
            Self::ManageAccounts => fl!(i18n, "menu-tools-manage-accounts"),
            Self::CategoryList => fl!(i18n, "menu-tools-category-list"),
            Self::Quit => fl!(i18n, "menu-file-quit"),
            Self::Preferences => fl!(i18n, "menu-tools-preferences"),
            Self::Calculator => fl!(i18n, "menu-tools-calculator"),
            Self::About => fl!(i18n, "menu-help-about"),
            Self::Reconcile => fl!(i18n, "register-account-actions-reconcile"),
            Self::EditAccountDetails => fl!(i18n, "register-account-actions-edit-account-details"),
            Self::RegisterPreferences => fl!(i18n, "register-account-actions-register-preferences"),
            Self::PrintRegister => fl!(i18n, "register-account-actions-print-register"),
            Self::ExportToCsv => fl!(i18n, "register-account-actions-export-to-csv"),
        }
    }

    /// Whether a focused text field does the action itself, on its own text: Undo and
    /// Redo undo and redo the typing there.
    pub(crate) fn text_field_does_it(self) -> bool {
        match self {
            Self::Undo | Self::Redo => true,
            Self::NewTransaction
            | Self::DeleteTransaction
            | Self::GoToDate
            | Self::SwitchPane
            | Self::NewDataFile
            | Self::OpenDataFile
            | Self::NewAccount
            | Self::ManageAccounts
            | Self::CategoryList
            | Self::Quit
            | Self::Preferences
            | Self::Calculator
            | Self::About
            | Self::Reconcile
            | Self::EditAccountDetails
            | Self::RegisterPreferences
            | Self::PrintRegister
            | Self::ExportToCsv => false,
        }
    }

    /// Whether the action steps through the undo history: Undo and Redo.
    pub(crate) fn steps_undo_history(self) -> bool {
        match self {
            Self::Undo | Self::Redo => true,
            Self::NewTransaction
            | Self::DeleteTransaction
            | Self::GoToDate
            | Self::SwitchPane
            | Self::NewDataFile
            | Self::OpenDataFile
            | Self::NewAccount
            | Self::ManageAccounts
            | Self::CategoryList
            | Self::Quit
            | Self::Preferences
            | Self::Calculator
            | Self::About
            | Self::Reconcile
            | Self::EditAccountDetails
            | Self::RegisterPreferences
            | Self::PrintRegister
            | Self::ExportToCsv => false,
        }
    }

    /// The Command that carries the action out.
    pub(crate) fn command(self) -> AppCommand {
        match self {
            Self::Undo => AppCommand::Undo,
            Self::Redo => AppCommand::Redo,
            Self::SwitchPane => AppCommand::SwitchPane,
            Self::NewTransaction => AppCommand::GoToDraftRow,
            Self::DeleteTransaction => AppCommand::OpenDialog(DialogKind::DeleteTransaction),
            Self::GoToDate => AppCommand::OpenDialog(DialogKind::GoToDate),
            Self::NewDataFile => AppCommand::ChooseDataFile(DataFilePicker::New),
            Self::OpenDataFile => AppCommand::ChooseDataFile(DataFilePicker::Open),
            Self::NewAccount => AppCommand::OpenDialog(DialogKind::NewAccount),
            Self::ManageAccounts | Self::EditAccountDetails => {
                AppCommand::OpenDialog(DialogKind::ManageAccounts)
            }
            Self::CategoryList => AppCommand::OpenDialog(DialogKind::CategoryList),
            Self::Quit => AppCommand::Quit,
            Self::Preferences | Self::RegisterPreferences => {
                AppCommand::OpenDialog(DialogKind::Preferences)
            }
            Self::Calculator => AppCommand::OpenCalculator,
            Self::About => AppCommand::ShowStatus(StatusMessage::About),
            Self::Reconcile | Self::PrintRegister | Self::ExportToCsv => {
                AppCommand::ShowStatus(StatusMessage::NotAvailableYet)
            }
        }
    }
}
