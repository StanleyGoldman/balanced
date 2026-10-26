// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The keys a text field takes before egui's focus handling reads them.

use eframe::egui::EventFilter;

/// The keys a focused text field takes before egui's focus handling reads them. Every
/// field takes Esc, so it can cancel or revert before egui drops its focus, and ← and
/// →, so they move the caret instead of moving focus to the widget beside the field.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FieldKeys(EventFilter);

impl FieldKeys {
    /// Esc, ← and →, the keys every text field takes.
    pub(crate) fn text_field() -> Self {
        Self(EventFilter {
            escape: true,
            horizontal_arrows: true,
            vertical_arrows: false,
            tab: false,
        })
    }

    /// These keys and ↑ and ↓, for a field whose code moves a list's highlight or a
    /// value with them.
    pub(crate) fn with_vertical_arrows(self) -> Self {
        Self(EventFilter {
            vertical_arrows: true,
            ..self.0
        })
    }

    /// These keys and Tab, for a field whose code handles Tab itself.
    pub(crate) fn with_tab(self) -> Self {
        Self(EventFilter {
            tab: true,
            ..self.0
        })
    }
}

impl From<FieldKeys> for EventFilter {
    fn from(keys: FieldKeys) -> Self {
        keys.0
    }
}
