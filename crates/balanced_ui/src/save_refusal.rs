// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`SaveRefusal`]: why a save of the Register's editing row was refused, as the
//! "Can't save this transaction" Dialog says it.

use balanced_core::CurrencyCode;
use balanced_i18n::{AmountParseError, Localizer};

use crate::amount_field;
use crate::command::StatusMessage;
use crate::date_field;
use crate::draft_row::Missing;
use crate::formatted_amount::i18n_currency;

/// Why a save of the editing row was refused: a field's own reason for refusing its
/// text, a draft row cell left empty, or the ledger's refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SaveRefusal {
    /// The Payment or Deposit cell's text can't be read as an amount in `currency`.
    Amount {
        error: AmountParseError,
        currency: CurrencyCode,
    },
    /// The Date cell's text is no date.
    Date,
    /// The draft row lacks a cell the transaction can't do without.
    Missing(Missing),
    /// The reason as the Status bar words it: a Category cell naming no category, or
    /// the ledger's refusal.
    Status(StatusMessage),
}

impl SaveRefusal {
    /// The reason, as the Dialog says it. An amount is read in the formatting locale,
    /// which can't change while the Dialog is open.
    pub(crate) fn text(&self, i18n: &Localizer) -> String {
        match self {
            Self::Amount { error, currency } => {
                let reader = i18n.amount_reader(i18n_currency(*currency));
                amount_field::error_text(i18n, *error, &reader, *currency)
            }
            Self::Date => date_field::format_message(i18n),
            Self::Missing(missing) => missing.message(i18n),
            Self::Status(message) => message.text(i18n),
        }
    }
}

/// The answer to the "Can't save this transaction" Dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RefusalAnswer {
    /// Stay on the row, with the field at fault open and its text selected.
    ContinueEditing,
    /// Throw away the row's changes, and let what started the save go on.
    Discard,
}
