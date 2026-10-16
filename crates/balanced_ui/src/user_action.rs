//! The user-action list: what the Menu bar and the keymap offer, each action's label
//! and the [`AppCommand`] it sends.

use balanced_i18n::Localizer;

use crate::command::{AppCommand, DialogKind, StatusMessage};
use crate::i18n::fl;

/// Something the user can do from the Menu bar or with a Shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UserAction {
    Undo,
    Redo,
    CloseRegisterTab,
    NextRegisterTab,
    PreviousRegisterTab,
    /// Move keyboard focus between the Account bar and the Main view.
    SwitchPane,
    /// Start a new transaction in the open Register.
    NewTransaction,
    /// Delete the Register's selected transaction, once confirmed.
    DeleteTransaction,
    Quit,
    About,
}

impl UserAction {
    /// The action's menu item text.
    pub(crate) fn label(self, i18n: &Localizer) -> String {
        match self {
            Self::Undo => fl!(i18n, "menu-edit-undo"),
            Self::Redo => fl!(i18n, "menu-edit-redo"),
            Self::CloseRegisterTab => fl!(i18n, "menu-file-close-register-tab"),
            Self::NextRegisterTab => fl!(i18n, "menu-view-next-register-tab"),
            Self::PreviousRegisterTab => fl!(i18n, "menu-view-previous-register-tab"),
            Self::SwitchPane => fl!(i18n, "menu-view-switch-pane"),
            Self::NewTransaction => fl!(i18n, "menu-new-transaction"),
            Self::DeleteTransaction => fl!(i18n, "menu-delete-transaction"),
            Self::Quit => fl!(i18n, "menu-file-quit"),
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
            | Self::CloseRegisterTab
            | Self::NextRegisterTab
            | Self::PreviousRegisterTab
            | Self::SwitchPane
            | Self::Quit
            | Self::About => false,
        }
    }

    /// The Command that carries the action out.
    pub(crate) fn command(self) -> AppCommand {
        match self {
            Self::Undo => AppCommand::Undo,
            Self::Redo => AppCommand::Redo,
            Self::CloseRegisterTab => AppCommand::CloseRegisterTab,
            Self::NextRegisterTab => AppCommand::NextRegisterTab,
            Self::PreviousRegisterTab => AppCommand::PreviousRegisterTab,
            Self::SwitchPane => AppCommand::SwitchPane,
            // TODO(#93): go to the draft row.
            Self::NewTransaction => AppCommand::ShowStatus(StatusMessage::NotAvailableYet),
            Self::DeleteTransaction => AppCommand::OpenDialog(DialogKind::DeleteTransaction),
            Self::Quit => AppCommand::Quit,
            Self::About => AppCommand::ShowStatus(StatusMessage::About),
        }
    }
}
