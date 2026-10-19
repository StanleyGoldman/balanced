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
    /// Pick a path and create an empty data file there.
    NewDataFile,
    /// Pick a data file and open it.
    OpenDataFile,
    /// Open the New Account wizard.
    NewAccount,
    /// Open Manage Accounts.
    ManageAccounts,
    Quit,
    /// Open the Preferences Dialog.
    Preferences,
    About,
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
            Self::NewDataFile => fl!(i18n, "menu-file-new-data-file"),
            Self::OpenDataFile => fl!(i18n, "menu-file-open-data-file"),
            Self::NewAccount => fl!(i18n, "menu-file-new-account"),
            Self::ManageAccounts => fl!(i18n, "menu-tools-manage-accounts"),
            Self::Quit => fl!(i18n, "menu-file-quit"),
            Self::Preferences => fl!(i18n, "menu-tools-preferences"),
            Self::About => fl!(i18n, "menu-help-about"),
        }
    }

    /// Whether a focused text field does the action itself, on its own text: Undo and
    /// Redo undo and redo the typing there.
    pub(crate) fn text_field_does_it(self) -> bool {
        match self {
            Self::Undo | Self::Redo => true,
            Self::NewTransaction
            | Self::DeleteTransaction
            | Self::SwitchPane
            | Self::NewDataFile
            | Self::OpenDataFile
            | Self::NewAccount
            | Self::ManageAccounts
            | Self::Quit
            | Self::Preferences
            | Self::About => false,
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
            Self::NewDataFile => AppCommand::ChooseDataFile(DataFilePicker::New),
            Self::OpenDataFile => AppCommand::ChooseDataFile(DataFilePicker::Open),
            Self::NewAccount => AppCommand::OpenDialog(DialogKind::NewAccount),
            Self::ManageAccounts => AppCommand::OpenDialog(DialogKind::ManageAccounts),
            Self::Quit => AppCommand::Quit,
            Self::Preferences => AppCommand::OpenDialog(DialogKind::Preferences),
            Self::About => AppCommand::ShowStatus(StatusMessage::About),
        }
    }
}
