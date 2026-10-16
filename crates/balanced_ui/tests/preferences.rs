//! Tools → Preferences: the Display group, its live preview, OK and Cancel, and the
//! preferences file, driven through the AccessKit tree the way a user drives it.

use std::path::Path;

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::egui::accesskit::Role;
use eframe::egui::{FontFamily, Key};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;
use tempfile::TempDir;

/// The App on stub data, in the test locale, keeping its Preferences in `config_dir`.
fn app_in(config_dir: &Path) -> Harness<'static, App> {
    let file = PreferencesFile::in_dir(config_dir);
    let mut harness = Harness::new_eframe(move |cc| {
        App::new(
            cc,
            Localizer::for_tests(),
            Ledger::stub(),
            Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
            file,
        )
    });
    harness.run();
    harness
}

/// A config directory that lasts as long as the returned `TempDir`.
fn config_dir() -> TempDir {
    tempfile::tempdir().expect("the test machine can make a temporary directory")
}

/// Opens Tools → Preferences.
fn open_preferences(harness: &mut Harness<'static, App>) {
    harness.get_by_role_and_label(Role::Button, "Tools").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Preferences…")
        .click();
    harness.run();
}

/// Clicks the button labelled `label`, then lets the App settle.
fn click(harness: &mut Harness<'static, App>, role: Role, label: &str) {
    harness.get_by_role_and_label(role, label).click();
    harness.run();
}

/// Whether the App draws in the dark scheme.
fn is_dark(harness: &Harness<'static, App>) -> bool {
    harness.ctx.global_style().visuals.dark_mode
}

/// The text of the preferences file in `config_dir`.
fn file_text(config_dir: &TempDir) -> String {
    std::fs::read_to_string(config_dir.path().join("preferences.toml"))
        .expect("the App wrote the preferences file")
}

#[test]
fn dark_then_ok_applies_the_dark_theme_and_writes_the_file() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open_preferences(&mut harness);

    click(&mut harness, Role::RadioButton, "Dark");
    click(&mut harness, Role::Button, "OK");

    assert!(is_dark(&harness));
    assert_eq!(
        file_text(&dir),
        "version = 1\n\
         scheme = \"dark\"\n\
         font_face = \"egui\"\n\
         dense_rows = false\n\
         monospace_money = false\n\
         coloured_negatives = true\n\
         register_columns = [\"date\", \"num\", \"payee\", \"category\", \"memo\", \"amount\", \"balance\"]\n\
         hidden_register_columns = []\n"
    );
}

#[test]
fn the_draft_shows_before_ok() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open_preferences(&mut harness);

    click(&mut harness, Role::RadioButton, "Dark");

    assert!(is_dark(&harness));
}

#[test]
fn cancel_goes_back_to_the_preferences_in_use_and_writes_nothing() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open_preferences(&mut harness);
    click(&mut harness, Role::RadioButton, "Dark");

    click(&mut harness, Role::Button, "Cancel");

    assert!(!is_dark(&harness));
    assert!(!dir.path().join("preferences.toml").exists());
}

#[test]
fn escape_goes_back_to_the_preferences_in_use() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open_preferences(&mut harness);
    click(&mut harness, Role::RadioButton, "Dark");

    harness.key_press(Key::Escape);
    harness.run();

    assert!(!is_dark(&harness));
    assert!(!dir.path().join("preferences.toml").exists());
}

#[test]
fn the_next_run_starts_with_the_saved_preferences() {
    let dir = config_dir();
    let mut first_run = app_in(dir.path());
    open_preferences(&mut first_run);
    click(&mut first_run, Role::RadioButton, "Dark");
    click(&mut first_run, Role::Button, "OK");
    drop(first_run);

    let second_run = app_in(dir.path());

    assert!(is_dark(&second_run));
}

#[test]
fn the_other_display_preferences_apply_and_are_saved_on_ok() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open_preferences(&mut harness);

    click(&mut harness, Role::ComboBox, "Font");
    click(&mut harness, Role::Button, "Noto Sans");
    click(&mut harness, Role::CheckBox, "Dense rows");
    click(
        &mut harness,
        Role::CheckBox,
        "Line up amounts in a fixed-width font",
    );
    click(&mut harness, Role::CheckBox, "Show negative amounts in red");
    click(&mut harness, Role::Button, "OK");

    let row_height = harness.ctx.global_style().spacing.interact_size.y;
    let first_face = harness
        .ctx
        .fonts(|fonts| fonts.definitions().families[&FontFamily::Proportional][0].clone());
    assert_eq!(row_height, 16.0);
    assert_eq!(first_face, "Noto Sans");
    assert_eq!(
        file_text(&dir),
        "version = 1\n\
         scheme = \"light\"\n\
         font_face = \"noto_sans\"\n\
         dense_rows = true\n\
         monospace_money = true\n\
         coloured_negatives = false\n\
         register_columns = [\"date\", \"num\", \"payee\", \"category\", \"memo\", \"amount\", \"balance\"]\n\
         hidden_register_columns = []\n"
    );
}

#[test]
fn an_unreadable_file_is_set_aside_and_the_app_starts_from_the_defaults() {
    let dir = config_dir();
    std::fs::write(
        dir.path().join("preferences.toml"),
        "scheme = \"dark\"\nnot toml [",
    )
    .unwrap();

    let harness = app_in(dir.path());

    let broken = std::fs::read_to_string(dir.path().join("preferences.toml.broken")).unwrap();
    assert_eq!(broken, "scheme = \"dark\"\nnot toml [");
    assert!(!is_dark(&harness));
    assert!(
        labels(&harness).contains(
            &"Your preferences couldn't be read, so the defaults are in use. \
          The old file is kept as preferences.toml.broken."
                .to_owned()
        )
    );
}

/// The text of every label on screen, such as the Status bar's message.
fn labels(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::Label)
        .filter_map(|label| label.accesskit_node().value())
        .collect()
}
