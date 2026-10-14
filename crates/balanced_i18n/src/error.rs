//! The crate's error type.

/// A failure to load messages or formatting data.
#[derive(Debug, thiserror::Error)]
pub enum I18nError {
    /// The embedded message files could not be loaded.
    #[error("could not load messages")]
    Messages(#[from] i18n_embed::I18nEmbedError),
    /// ICU4X has no number or week data for the formatting locale.
    #[error("no formatting data for the locale")]
    FormattingData(#[from] icu_provider::DataError),
    /// ICU4X has no date pattern for the formatting locale.
    #[error("no date formatting data for the locale")]
    DateFormattingData(#[from] icu_datetime::DateTimeFormatterLoadError),
    /// The currency code is not three uppercase ASCII letters, or its decimal places
    /// don't fit.
    #[error("invalid currency {code:?}")]
    Currency {
        /// The code as given.
        code: String,
    },
}

/// Why typed text is not an amount. `balanced_ui` shows each as an inline error on
/// the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AmountParseError {
    /// Nothing but spaces or a currency symbol was typed.
    #[error("no amount entered")]
    Empty,
    /// The text is not a number written the formatting locale's way.
    #[error("not an amount")]
    Invalid,
    /// The amount has more decimal places than its currency allows. Excess digits are
    /// rejected, not rounded.
    #[error("amounts have {decimal_places} decimal places")]
    TooManyDecimalPlaces {
        /// The currency's decimal places.
        decimal_places: u32,
    },
    /// The amount is too large to hold.
    #[error("amount out of range")]
    OutOfRange,
}
