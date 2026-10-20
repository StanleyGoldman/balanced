// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Balanced desktop application.

use std::path::PathBuf;

use anyhow::{Context as _, Result, anyhow};
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile, RecentFilesFile};
use directories::ProjectDirs;
use icu_locale::Locale;
use tracing_subscriber::EnvFilter;

/// The environment variable that replaces the OS locale with a language tag, such
/// as `de-DE`, or `en-XA` for the pseudo-locale.
const LOCALE_OVERRIDE: &str = "BALANCED_LOCALE";

fn main() -> Result<()> {
    install_tracing();
    // An empty override, as `BALANCED_LOCALE= cargo run` sets, means no override.
    let locale_override = std::env::var(LOCALE_OVERRIDE)
        .ok()
        .filter(|tag| !tag.is_empty());
    let i18n = localizer(locale_override.as_deref())?;
    let config_dir = config_dir()?;
    let preferences_file = PreferencesFile::in_dir(&config_dir);
    let recent_files_file = RecentFilesFile::in_dir(&config_dir);
    eframe::run_native(
        "Balanced",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            Ok(Box::new(App::new(
                cc,
                i18n,
                Clock::default(),
                preferences_file,
                recent_files_file,
            )))
        }),
    )
    // `eframe::Error` isn't `Send + Sync`, so it can't be kept as the source.
    .map_err(|err| anyhow!("the Main window failed: {err}"))
}

/// The directory Balanced keeps its configuration in, such as `~/.config/balanced`
/// on Linux.
fn config_dir() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("", "", "Balanced")
        .context("the OS reports no home directory to keep the configuration in")?;
    Ok(dirs.config_dir().to_owned())
}

/// Logs to stderr at `info` and above, or as `RUST_LOG` says.
fn install_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    #[expect(
        clippy::disallowed_methods,
        reason = "the binary installs the one tracing subscriber"
    )]
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

/// The localizer for `locale_override`, a language tag, or for the OS locale when
/// there is none.
fn localizer(locale_override: Option<&str>) -> Result<Localizer> {
    let Some(tag) = locale_override else {
        return Localizer::from_os().context("loading the OS locale's messages failed");
    };
    let locale = Locale::try_from_str(tag)
        .with_context(|| format!("{LOCALE_OVERRIDE} is not a language tag: {tag:?}"))?;
    let i18n = if locale.id == Localizer::PSEUDO_LANGUAGE {
        Localizer::pseudo()
    } else {
        Localizer::new(&locale.id, &locale)
    };
    i18n.with_context(|| format!("loading the messages for {LOCALE_OVERRIDE} failed: {tag:?}"))
}

#[cfg(test)]
mod tests {
    use balanced_i18n::Localizer;
    use icu_locale::{langid, locale};

    use super::*;

    #[test]
    fn en_xa_picks_the_pseudo_locale_formatting_in_german() {
        let i18n = localizer(Some("en-XA")).unwrap();

        assert_eq!(i18n.text_language(), &Localizer::PSEUDO_LANGUAGE);
        assert_eq!(i18n.formatting_locale(), &locale!("de-DE"));
    }

    #[test]
    fn a_language_tag_sets_text_and_formatting() {
        let i18n = localizer(Some("de-DE")).unwrap();

        assert_eq!(i18n.text_language(), &langid!("de-DE"));
        assert_eq!(i18n.formatting_locale(), &locale!("de-DE"));
    }

    #[test]
    fn a_malformed_tag_is_an_error() {
        let result = localizer(Some("not a locale"));

        assert!(result.is_err());
    }
}
