//! The Recent files: opening or creating a data file moves it to the top of
//! `recent-files.toml`, and the File menu lists them. The native file picker can't be
//! driven, so a test that picks a file sends the Command the picker sends.

use std::path::{Path, PathBuf};

use balanced_i18n::Localizer;
use balanced_ui::testing;
use balanced_ui::{App, Clock, PreferencesFile, RecentFilesFile};
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;
use tempfile::TempDir;

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

/// The text of `recent-files.toml` listing `files`, newest first.
fn listing(files: &[&Path]) -> String {
    let files: Vec<_> = files
        .iter()
        .map(|file| format!("\"{}\"", file.display()))
        .collect();
    format!("version = 1\nfiles = [{}]\n", files.join(", "))
}

/// Writes `files`, newest first, as the Recent files in the config directory `config`.
fn recent_files(config: &Path, files: &[&Path]) {
    std::fs::write(config.join("recent-files.toml"), listing(files))
        .expect("the temp dir is writable");
}

/// The text of `recent-files.toml` in the config directory `config`.
fn listed_in(config: &Path) -> String {
    std::fs::read_to_string(config.join("recent-files.toml"))
        .expect("the App wrote the recent files file")
}

/// A temporary directory, by its resolved path, so the paths in it are written as the
/// App lists them.
fn temp_dir() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = dir.path().canonicalize().expect("the temp dir resolves");
    (dir, path)
}

#[test]
fn opening_a_file_puts_it_at_the_top() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    let business = testing::data_file(&dir, "Business.balanced", &["Payroll"]);
    recent_files(&dir, &[&household]);
    let mut harness = app(&dir);

    testing::open_data_file(&harness, &business);
    harness.run();

    assert_eq!(listed_in(&dir), listing(&[&business, &household]));
}

#[test]
fn a_file_is_listed_by_its_resolved_path_once() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    let business = testing::data_file(&dir, "Business.balanced", &["Payroll"]);
    recent_files(&dir, &[&household, &business]);
    let mut harness = app(&dir);

    testing::open_data_file(&harness, &dir.join(".").join("Business.balanced"));
    harness.run();

    assert_eq!(listed_in(&dir), listing(&[&business, &household]));
}

#[test]
fn creating_a_file_puts_it_at_the_top() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    recent_files(&dir, &[&household]);
    let mut harness = app(&dir);
    let new = dir.join("New.balanced");

    testing::new_data_file(&harness, &new);
    harness.run();

    assert_eq!(listed_in(&dir), listing(&[&new, &household]));
}

#[test]
fn a_missing_file_stays_listed() {
    let (_dir, dir) = temp_dir();
    let gone = dir.join("Gone.balanced");
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    recent_files(&dir, &[&gone]);
    let mut harness = app(&dir);

    testing::open_data_file(&harness, &household);
    harness.run();

    assert_eq!(listed_in(&dir), listing(&[&household, &gone]));
}

#[test]
fn the_list_keeps_the_four_newest() {
    let (_dir, dir) = temp_dir();
    let [one, two, three, four, five] = ["One", "Two", "Three", "Four", "Five"]
        .map(|name| testing::data_file(&dir, &format!("{name}.balanced"), &[]));
    recent_files(&dir, &[&one, &two, &three, &four]);
    let mut harness = app(&dir);

    testing::open_data_file(&harness, &five);
    harness.run();

    assert_eq!(listed_in(&dir), listing(&[&five, &one, &two, &three]));
}

/// Opens the File menu.
fn open_file_menu(harness: &mut Harness<'static, App>) {
    harness.get_by_role_and_label(Role::Button, "File").click();
    harness.run();
}

#[test]
fn the_file_menu_lists_the_recent_files_numbered_above_quit() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    let business = testing::data_file(&dir, "Business.balanced", &["Payroll"]);
    recent_files(&dir, &[&household, &business]);
    let mut harness = app(&dir);

    open_file_menu(&mut harness);

    let items: Vec<String> = harness
        .get_all_by_role(Role::Button)
        .filter_map(|node| node.accesskit_node().label())
        .collect();
    let at = |label: &str| {
        items
            .iter()
            .position(|item| item == label)
            .unwrap_or_else(|| panic!("the File menu has no {label:?}: {items:?}"))
    };
    assert_eq!(at("1 Household.balanced") + 1, at("2 Business.balanced"));
    assert_eq!(at("2 Business.balanced") + 1, at("Quit Ctrl+Q"));
}

#[test]
fn clicking_a_recent_file_opens_it() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    let business = testing::data_file(&dir, "Business.balanced", &["Payroll"]);
    recent_files(&dir, &[&household, &business]);
    let mut harness = app(&dir);
    open_file_menu(&mut harness);

    harness
        .get_by_role_and_label(Role::Button, "2 Business.balanced")
        .click();
    harness.run();

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Payroll")
            .is_some()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Everyday")
            .is_none()
    );
    assert_eq!(listed_in(&dir), listing(&[&business, &household]));
}
