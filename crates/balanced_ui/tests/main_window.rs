//! The Main window, driven through the AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::App;
use eframe::egui::accesskit::{Role, Toggled};
use eframe::egui::{Event, Key, Modifiers, ViewportCommand, ViewportId};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

/// The App on stub data, in the test locale.
fn app() -> Harness<'static, App> {
    Harness::new_eframe(|cc| App::new(cc, Localizer::for_tests(), Ledger::stub()))
}

/// Queues one key press with `modifiers` held, as a single event. `step()` plays
/// each queued event in a frame of its own, so `key_press_modifiers`, which queues
/// four, runs past the frame whose output a test reads.
fn press(harness: &Harness<'static, App>, modifiers: Modifiers, key: Key) {
    harness.event(Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    });
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
    harness
        .get_by_role_and_label(Role::Button, "Quit Ctrl+Q")
        .click();
    // One step plays the click, which sends Quit; the next applies it. Only that
    // frame's output carries the viewport command.
    harness.step();
    harness.step();

    let root = &harness.output().viewport_output[&ViewportId::ROOT];
    assert_eq!(root.commands, [ViewportCommand::Close]);
}

#[test]
fn ctrl_q_closes_the_main_window() {
    let mut harness = app();

    press(&harness, Modifiers::COMMAND, Key::Q);
    // One step plays the key press, which sends Quit; the next applies it.
    harness.step();
    harness.step();

    let root = &harness.output().viewport_output[&ViewportId::ROOT];
    assert_eq!(root.commands, [ViewportCommand::Close]);
}

#[test]
fn file_quit_shows_its_shortcut() {
    let mut harness = app();

    harness.get_by_role_and_label(Role::Button, "File").click();
    harness.run();

    // egui reads a menu item's label and its Shortcut text as one label.
    harness.get_by_role_and_label(Role::Button, "Quit Ctrl+Q");
}
