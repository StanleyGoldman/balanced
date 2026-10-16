//! [`FormattedAmount`]: an amount with its text, formatted once for a view model.

use balanced_core::{CurrencyCode, Money};
use balanced_i18n::{AmountStyle, Currency, Localizer};
use eframe::egui::RichText;

use crate::theme::Theme;

/// An amount and its text, formatted in the formatting locale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FormattedAmount {
    money: Money,
    text: String,
}

impl FormattedAmount {
    /// `money`, formatted by `i18n` in `style`.
    pub(crate) fn new(i18n: &Localizer, money: Money, style: AmountStyle) -> Self {
        Self {
            money,
            text: i18n.format_amount(money.amount(), i18n_currency(money.currency()), style),
        }
    }

    pub(crate) fn money(&self) -> Money {
        self.money
    }

    /// The text, unstyled.
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// The text, styled by `theme` for the amount's sign.
    pub(crate) fn styled(&self, theme: &Theme) -> RichText {
        theme.amount(&self.money, self.text.clone())
    }
}

/// `code` as `balanced_i18n` formats and reads amounts in it.
pub(crate) fn i18n_currency(code: CurrencyCode) -> Currency {
    Currency::new(code.as_str(), code.decimal_places())
        .expect("a `CurrencyCode` is three uppercase letters with at most 4 decimal places")
}
