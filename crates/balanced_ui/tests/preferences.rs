// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Tools → Preferences: each group's Preferences, their live preview, OK and Cancel,
//! and the preferences file, driven through the AccessKit tree the way a user drives
//! it.

use std::path::Path;

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::testing::check_saved_preferences;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::egui::accesskit::{Role, Toggled};
use eframe::egui::{FontFamily, Key};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;
use tempfile::TempDir;

/// The App on stub data, in the test locale, keeping its Preferences in `config_dir`.
fn app_in(config_dir: &Path) -> Harness<'static, App> {
    let file = PreferencesFile::in_dir(config_dir);
    let mut harness = Harness::new_eframe(move |cc| {
        App::on_ledger(
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

#[test]
fn dark_then_ok_applies_the_dark_theme_and_writes_the_file() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open_preferences(&mut harness);

    click(&mut harness, Role::RadioButton, "Dark");
    click(&mut harness, Role::Button, "OK");

    assert!(is_dark(&harness));
    check_saved_preferences(
        dir.path(),
        "version = 2\n\
         scheme = \"dark\"\n",
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
    check_saved_preferences(
        dir.path(),
        "version = 2\n\
         font_face = \"noto_sans\"\n\
         dense_rows = true\n\
         monospace_money = true\n\
         coloured_negatives = false\n",
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

/// Shows the Preferences of `group` in the open Preferences Dialog.
fn show_group(harness: &mut Harness<'static, App>, group: &str) {
    click(harness, Role::Tab, group);
}

#[test]
fn the_register_preferences_are_saved_on_ok() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open_preferences(&mut harness);
    show_group(&mut harness, "Register");

    click(&mut harness, Role::CheckBox, "Alternating rows");
    click(&mut harness, Role::CheckBox, "Today divider");
    click(&mut harness, Role::RadioButton, "Both");
    click(&mut harness, Role::CheckBox, "Enter moves to the next row");
    click(
        &mut harness,
        Role::CheckBox,
        "Save a changed transaction without asking",
    );
    click(&mut harness, Role::Button, "OK");

    check_saved_preferences(
        dir.path(),
        "version = 2\n\
         alternating_rows = false\n\
         today_divider = false\n\
         gridlines = \"both\"\n\
         enter_moves_to_next_row = false\n\
         save_without_asking = true\n",
    );
}

#[test]
fn turning_off_the_today_divider_hides_it() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    click(&mut harness, Role::Button, "Everyday Checking");
    open_preferences(&mut harness);
    show_group(&mut harness, "Register");

    click(&mut harness, Role::CheckBox, "Today divider");
    click(&mut harness, Role::Button, "OK");

    assert!(!labels(&harness).contains(&"Today · 10/14/2026".to_owned()));
}

#[test]
fn the_account_bar_preferences_are_saved_on_ok() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open_preferences(&mut harness);
    show_group(&mut harness, "Account bar");
    click(&mut harness, Role::CheckBox, "Show group totals");
    click(&mut harness, Role::RadioButton, "In the Status bar");

    click(&mut harness, Role::Button, "OK");

    check_saved_preferences(
        dir.path(),
        "version = 2\n\
         group_totals = true\n\
         net_worth_placement = \"status_bar\"\n",
    );
}

/// The total the Account bar heading `section` shows, if any.
fn heading_total(harness: &Harness<'static, App>, section: &str) -> Option<String> {
    harness
        .get_by_role_and_label(Role::Button, section)
        .accesskit_node()
        .value()
}

#[test]
fn enter_moves_to_the_next_row_is_on_by_default() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open_preferences(&mut harness);
    show_group(&mut harness, "Register");

    let checkbox = harness.get_by_role_and_label(Role::CheckBox, "Enter moves to the next row");

    assert_eq!(checkbox.accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn group_totals_are_off_by_default() {
    let dir = config_dir();

    let harness = app_in(dir.path());

    assert_eq!(heading_total(&harness, "Banking"), None);
}

#[test]
fn group_totals_show_on_the_headings_while_previewed() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open_preferences(&mut harness);
    show_group(&mut harness, "Account bar");

    click(&mut harness, Role::CheckBox, "Show group totals");

    // Everyday Checking $4,071.81, Joint Checking $1,032.00, Household Checking
    // $500.00, Emergency Fund $12,000.00, the hidden Holiday Fund $800.00 and Visa
    // Rewards -$412.30. Closed Old Checking is left out.
    assert_eq!(
        heading_total(&harness, "Banking").as_deref(),
        Some("$17,991.51")
    );
}

/// How many times Net Worth's amount is shown inside the Status bar, and how many
/// times anywhere.
fn net_worth_in_status_bar_and_anywhere(harness: &Harness<'static, App>) -> (usize, usize) {
    let amount = "$286,922.06";
    let in_status_bar = harness
        .get_by_role(Role::Status)
        .query_all_by_role_and_label(Role::Label, amount)
        .count();
    let anywhere = harness
        .query_all_by_role_and_label(Role::Label, amount)
        .count();
    (in_status_bar, anywhere)
}

#[test]
fn net_worth_starts_in_the_account_bar_footer() {
    let dir = config_dir();

    let harness = app_in(dir.path());

    assert_eq!(net_worth_in_status_bar_and_anywhere(&harness), (0, 1));
}

#[test]
fn net_worth_moves_to_the_status_bar() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open_preferences(&mut harness);
    show_group(&mut harness, "Account bar");

    click(&mut harness, Role::RadioButton, "In the Status bar");
    click(&mut harness, Role::Button, "OK");

    assert_eq!(net_worth_in_status_bar_and_anywhere(&harness), (1, 1));
}

#[test]
fn add_account_stays_at_the_foot_of_the_account_bar_with_net_worth_in_the_status_bar() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open_preferences(&mut harness);
    show_group(&mut harness, "Account bar");

    click(&mut harness, Role::RadioButton, "In the Status bar");
    click(&mut harness, Role::Button, "OK");

    let add_account = harness
        .get_by_role_and_label(Role::Button, "+ Add Account")
        .rect();
    let net_worth = harness
        .get_by_role(Role::Status)
        .get_by_role_and_label(Role::Label, "$286,922.06")
        .rect();
    // The Account bar's footer ends where the Status bar, and its Net Worth, begin.
    let gap = net_worth.top() - add_account.bottom();
    assert!((0.0..add_account.height()).contains(&gap), "{gap}");
}

#[test]
fn net_worth_previews_in_the_status_bar() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open_preferences(&mut harness);
    show_group(&mut harness, "Account bar");

    click(&mut harness, Role::RadioButton, "In the Status bar");

    assert_eq!(net_worth_in_status_bar_and_anywhere(&harness), (1, 1));
}

/// The labels of the checkboxes in the open Preferences Dialog's shown group.
fn checkboxes(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::CheckBox)
        .filter_map(|checkbox| checkbox.accesskit_node().label())
        .collect()
}

#[test]
fn the_account_bar_group_has_only_the_group_totals_checkbox() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open_preferences(&mut harness);

    show_group(&mut harness, "Account bar");

    assert_eq!(checkboxes(&harness), ["Show group totals"]);
}
