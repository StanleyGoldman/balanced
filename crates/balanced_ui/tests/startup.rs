//! Starting the App from the Recent files: the newest opens, and with none, or with
//! the newest refused, the welcome Dialog asks for a data file. The native file picker
//! can't be driven, so a test that picks a file sends the Command the picker sends.

use std::path::Path;

use balanced_i18n::Localizer;
use balanced_ui::testing;
use balanced_ui::{App, Clock, PreferencesFile, RecentFilesFile};
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, ViewportCommand, ViewportId};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use icu_calendar::Date;

const WELCOME: &str = "Balanced keeps your finances in a data file.";

/// The App started with its config directory at `config`, in the test locale.
fn app(config: &Path) -> Harness<'static, App> {
    let config = config.to_owned();
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .build_eframe(move |cc| {
            App::new(
                cc,
                Localizer::for_tests(),
                Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
                PreferencesFile::for_tests(),
                RecentFilesFile::in_dir(&config),
            )
        });
    harness.run();
    harness
}

/// Writes `files`, newest first, as the Recent files in the config directory `config`.
fn recent_files(config: &Path, files: &[&Path]) {
    let files: Vec<_> = files
        .iter()
        .map(|file| format!("\"{}\"", file.display()))
        .collect();
    std::fs::write(
        config.join("recent-files.toml"),
        format!("version = 1\nfiles = [{}]\n", files.join(", ")),
    )
    .expect("the temp dir is writable");
}

/// Whether the Account bar lists an account named `name`.
fn listed(harness: &Harness<'static, App>, name: &str) -> bool {
    harness
        .query_by_role_and_label(Role::Button, name)
        .is_some()
}

fn welcome_shown(harness: &Harness<'static, App>) -> bool {
    harness.query_by_label(WELCOME).is_some()
}

#[test]
fn the_app_starts_on_the_newest_recent_file() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let household = testing::data_file(dir.path(), "Household.balanced", &["Household Checking"]);
    let business = testing::data_file(dir.path(), "Business.balanced", &["Payroll"]);
    recent_files(dir.path(), &[&household, &business]);

    let harness = app(dir.path());

    assert!(listed(&harness, "Household Checking"));
    assert!(!listed(&harness, "Payroll"));
    assert!(!welcome_shown(&harness));
}

#[test]
fn with_no_recent_files_the_app_starts_on_the_welcome_dialog() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");

    let harness = app(dir.path());

    harness.get_by_label(WELCOME);
    harness.get_by_role_and_label(Role::RadioButton, "Create a new data file…");
    harness.get_by_role_and_label(Role::RadioButton, "Open an existing data file…");
}

/// Checks the App shows `message`, the refusal, without the welcome Dialog, and that
/// OK brings the welcome Dialog.
fn assert_refusal_then_welcome(harness: &mut Harness<'static, App>, message: &str) {
    harness.get_by_label(message);
    assert!(!welcome_shown(harness));
    harness.get_by_role_and_label(Role::Button, "OK").click();
    harness.run();
    assert!(harness.query_by_label(message).is_none());
    assert!(welcome_shown(harness));
}

#[test]
fn a_refused_newest_file_shows_why_then_the_welcome_dialog() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    recent_files(dir.path(), &[&dir.path().join("Gone.balanced")]);

    let mut harness = app(dir.path());

    assert_refusal_then_welcome(
        &mut harness,
        "Balanced can't find “Gone.balanced”. It may have been moved, renamed or deleted.",
    );
}

/// The commands the App sends the window over the next few frames. Each frame's
/// output carries only the commands sent in that frame.
fn viewport_commands(harness: &mut Harness<'static, App>) -> Vec<ViewportCommand> {
    let mut commands = Vec::new();
    for _ in 0..3 {
        harness.step();
        commands.extend(
            harness.output().viewport_output[&ViewportId::ROOT]
                .commands
                .iter()
                .cloned(),
        );
    }
    commands
}

#[test]
fn quit_on_the_welcome_dialog_closes_the_window() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let mut harness = app(dir.path());

    harness.get_by_role_and_label(Role::Button, "Quit").click();

    assert_eq!(viewport_commands(&mut harness), [ViewportCommand::Close]);
}

#[test]
fn esc_on_the_welcome_dialog_closes_the_window() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let mut harness = app(dir.path());

    harness.key_press(Key::Escape);

    assert_eq!(viewport_commands(&mut harness), [ViewportCommand::Close]);
}

#[test]
fn a_file_opened_from_the_welcome_screen_shows_its_accounts() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let household = testing::data_file(dir.path(), "Household.balanced", &["Household Checking"]);
    let mut harness = app(dir.path());

    testing::open_data_file(&harness, &household);
    harness.run();

    assert!(listed(&harness, "Household Checking"));
    assert!(!welcome_shown(&harness));
}

#[test]
fn a_file_refused_from_the_welcome_screen_shows_why_then_the_welcome_dialog() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let not_balanced = dir.path().join("Notes.balanced");
    std::fs::write(&not_balanced, "shopping list").expect("the temp dir is writable");
    let mut harness = app(dir.path());

    testing::open_data_file(&harness, &not_balanced);
    harness.run();

    assert_refusal_then_welcome(&mut harness, "“Notes.balanced” isn't a Balanced data file.");
}
