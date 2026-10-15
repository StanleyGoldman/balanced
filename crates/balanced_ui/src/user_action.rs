//! The user-action list: what the Menu bar and the keymap offer, each action's label
//! and the [`AppCommand`] it sends.

use balanced_i18n::Localizer;

use crate::command::{AppCommand, StatusMessage};
use crate::i18n::fl;

/// Something the user can do from the Menu bar or with a Shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UserAction {
    Quit,
    About,
}

impl UserAction {
    /// The action's menu item text.
    pub(crate) fn label(self, i18n: &Localizer) -> String {
        match self {
            Self::Quit => fl!(i18n, "menu-file-quit"),
            Self::About => fl!(i18n, "menu-help-about"),
        }
    }

    /// The Command that carries the action out.
    pub(crate) fn command(self) -> AppCommand {
        match self {
            Self::Quit => AppCommand::Quit,
            Self::About => AppCommand::ShowStatus(StatusMessage::About),
        }
    }
}
