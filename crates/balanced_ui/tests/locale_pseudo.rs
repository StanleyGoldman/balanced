//! The App in the `en-XA` pseudo-locale, where text that came through `fl!` shows
//! accented and bracketed, and anything else shows as plain English.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::egui::accesskit::Role;
use eframe::egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use icu_calendar::Date;
use icu_locale::locale;

/// The `en-XA` localizer, formatting in `de-DE` as `Localizer::pseudo` does.
fn pseudo() -> Localizer {
    Localizer::for_tests_in(&Localizer::PSEUDO_LANGUAGE, &locale!("de-DE"))
        .expect("the embedded en-US messages and de-DE formatting data load")
}

/// The text the App shows for the message `id` in `en-XA`.
fn text(id: &str) -> String {
    let i18n = pseudo();
    i18n.finish(i18n.loader().get(id))
}

/// The App on stub data in `en-XA`.
fn app() -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(|cc| {
            App::new(
                cc,
                pseudo(),
                Ledger::stub(),
                Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
                PreferencesFile::for_tests(),
            )
        });
    harness.run();
    harness
}

/// Clicks the button that reads the message `id`, then lets the App settle.
fn click(harness: &mut Harness<'static, App>, id: &str) {
    harness
        .get_by_role_and_label(Role::Button, &text(id))
        .click();
    harness.run();
}

#[test]
fn a_heavy_dialogs_close_button_reads_the_pseudo_locale() {
    let mut harness = app();

    click(&mut harness, "menu-tools");
    click(&mut harness, "menu-tools-preferences");

    // "Close dialog", accented, with doubled vowels and bracketed.
    harness.get_by_role_and_label(Role::Button, "[Çĺööšéé ďîîååĺööğ]");
    assert!(harness.query_by_label("Close window").is_none());
}
