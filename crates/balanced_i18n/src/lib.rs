//! Translated messages and locale-aware formatting. Knows nothing about egui.
//!
//! Text comes from the Fluent files in `locales/<language>/balanced.ftl`, and
//! numbers and dates are formatted with ICU4X. [`Localizer`] owns both.
//! `docs/practices/i18n.md` describes how to add and use messages.

mod chain;
mod error;
mod format;
mod localizer;
mod missing;
mod pseudo;

pub use error::I18nError;
pub use localizer::Localizer;
