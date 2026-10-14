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
}
