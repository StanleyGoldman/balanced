//! Translated messages and locale-aware formatting. Knows nothing about egui.
//!
//! Text comes from the Fluent files in `locales/<language>/balanced.ftl`, and
//! numbers, dates and money amounts are formatted with ICU4X. [`Localizer`] owns both.
//! `docs/practices/i18n.md` describes how to add and use messages.

mod calculator;
mod chain;
mod error;
mod format;
mod localizer;
mod missing;
mod money;
mod pseudo;

pub use calculator::{AmountReader, Calculation};
pub use error::{AmountParseError, I18nError};
pub use localizer::Localizer;
pub use money::{AmountStyle, Currency};
