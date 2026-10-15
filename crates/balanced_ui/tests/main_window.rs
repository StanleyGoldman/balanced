//! The Main window, driven through the AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::App;
use eframe::egui::accesskit::{Role, Toggled};
use eframe::egui::{ViewportCommand, ViewportId};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

/// The App on stub data, in the test locale.
fn app() -> Harness<'static, App> {
    Harness::new_eframe(|cc| App::new(cc, Localizer::for_tests(), Ledger::stub()))
}

#[test]
fn home_is_active_at_start() {
    let harness = app();

    let home = harness.get_by_role_and_label(Role::Button, "Home");

    assert_eq!(home.accesskit_node().toggled(), Some(Toggled::True));
    harness.get_by_label("Your overview of accounts, bills and spending will appear here.");
}

#[test]
fn clicking_a_tab_shows_its_placeholder() {
    let mut harness = app();

    harness.get_by_role_and_label(Role::Button, "Bills").click();
    harness.run();

    let bills = harness.get_by_role_and_label(Role::Button, "Bills");
    assert_eq!(bills.accesskit_node().toggled(), Some(Toggled::True));
    harness.get_by_label("Upcoming bills and deposits will appear here.");
}

#[test]
fn help_about_shows_the_version_in_the_status_bar() {
    let mut harness = app();

    harness.get_by_role_and_label(Role::Button, "Help").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "About Balanced")
        .click();
    harness.run();

    harness.get_by_label("Balanced 0.0.0");
}

#[test]
fn file_quit_closes_the_main_window() {
    let mut harness = app();

    harness.get_by_role_and_label(Role::Button, "File").click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Quit").click();
    // One step plays the click, which sends Quit; the next applies it. Only that
    // frame's output carries the viewport command.
    harness.step();
    harness.step();

    let root = &harness.output().viewport_output[&ViewportId::ROOT];
    assert_eq!(root.commands, [ViewportCommand::Close]);
}
