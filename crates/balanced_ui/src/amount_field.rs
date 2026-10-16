//! The amount field's rules, shared by every field that takes an amount: the
//! characters it refuses, the result of arithmetic typed in it, and why it refuses its
//! text.

use balanced_core::CurrencyCode;
use balanced_i18n::{AmountParseError, AmountReader, AmountStyle, Calculation, Localizer};
use eframe::egui::{self, Event};

use crate::formatted_amount::i18n_currency;
use crate::i18n::fl;

/// Removes from this frame's typed and pasted text every character `reader` refuses,
/// before the field sees it.
pub(crate) fn refuse_characters(ctx: &egui::Context, reader: &AmountReader) {
    ctx.input_mut(|input| {
        for event in &mut input.events {
            if let Event::Text(text) | Event::Paste(text) = event {
                text.retain(|c| reader.accepts(c));
            }
        }
    });
}

/// The result of typed arithmetic, as the field shows it while typing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Preview {
    /// The result, formatted with its currency's symbol.
    amount: String,
    /// Whether the result was rounded to the currency's decimal places, which the
    /// preview marks with `~`.
    rounded: bool,
}

impl Preview {
    /// The result of the arithmetic in `text`, read by `reader` in `currency`, or
    /// `None` when the text is a plain amount or can't be read.
    pub(crate) fn of(
        i18n: &Localizer,
        reader: &AmountReader,
        currency: CurrencyCode,
        text: &str,
    ) -> Option<Self> {
        let calculation = reader
            .read(text.trim())
            .ok()
            .filter(Calculation::is_arithmetic)?;
        let amount = i18n.format_amount(
            calculation.amount(),
            i18n_currency(currency),
            AmountStyle::Symbol,
        );
        Some(Self {
            amount,
            rounded: calculation.rounded(),
        })
    }

    /// The preview's text: `= $33.33`, or `~ $33.33` when rounded.
    pub(crate) fn text(&self, i18n: &Localizer) -> String {
        let amount = self.amount.clone();
        if self.rounded {
            fl!(i18n, "field-amount-preview-rounded", amount = amount)
        } else {
            fl!(i18n, "field-amount-preview", amount = amount)
        }
    }
}

/// Why typed text is not an amount in `currency`, in words naming what is expected.
pub(crate) fn error_text(
    i18n: &Localizer,
    error: AmountParseError,
    reader: &AmountReader,
    currency: CurrencyCode,
) -> String {
    match error {
        AmountParseError::Empty => fl!(i18n, "field-amount-empty"),
        AmountParseError::Invalid => fl!(
            i18n,
            "field-amount-invalid",
            decimal = reader.decimal_mark(),
            group = reader.group_separator()
        ),
        AmountParseError::TooManyDecimalPlaces { decimal_places } => fl!(
            i18n,
            "field-amount-too-many-decimal-places",
            currency = currency.as_str(),
            places = decimal_places
        ),
        AmountParseError::OutOfRange => fl!(i18n, "field-amount-out-of-range"),
        AmountParseError::DivisionByZero => fl!(i18n, "field-amount-division-by-zero"),
    }
}
