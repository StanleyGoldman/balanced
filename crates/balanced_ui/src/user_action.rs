//! The user-action list: what the Menu bar and the keymap offer, each action's label
//! and the [`AppCommand`] it sends.

use balanced_i18n::Localizer;

use crate::command::{AppCommand, StatusMessage};
use crate::i18n::fl;

/// Something the user can do from the Menu bar or with a Shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UserAction {
    Undo,
    Redo,
    CloseRegisterTab,
    NextRegisterTab,
    PreviousRegisterTab,
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
            Self::Quit => fl!(i18n, "menu-file-quit"),
            Self::About => fl!(i18n, "menu-help-about"),
        }
    }

    /// Whether a focused text field does the action itself, on its own text: Undo and
    /// Redo undo and redo the typing there.
    pub(crate) fn text_field_does_it(self) -> bool {
        match self {
            Self::Undo | Self::Redo => true,
            Self::CloseRegisterTab
            | Self::NextRegisterTab
            | Self::PreviousRegisterTab
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
            Self::Quit => AppCommand::Quit,
            Self::About => AppCommand::ShowStatus(StatusMessage::About),
        }
    }
}
