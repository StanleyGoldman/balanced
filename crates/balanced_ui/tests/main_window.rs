//! The Main window, driven through the AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::egui::accesskit::{Role, Toggled};
use eframe::egui::{Event, Key, Modifiers, ViewportCommand, ViewportId};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on stub data, in the test locale.
fn app() -> Harness<'static, App> {
    let mut harness = Harness::new_eframe(|cc| {
        App::new(
            cc,
            Localizer::for_tests(),
            Ledger::stub(),
            Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
            PreferencesFile::for_tests(),
        )
    });
    harness.run();
    harness
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

#[test]
fn selecting_an_account_opens_it_under_the_accounts_tab() {
    let mut harness = app();

    harness
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .click();
    harness.run();

    let accounts = harness.get_by_role_and_label(Role::Button, "Accounts");
    assert_eq!(accounts.accesskit_node().toggled(), Some(Toggled::True));
    harness.get_by_role_and_label(Role::Label, "Everyday Checking");
    harness.get_by_role_and_label(Role::Label, "$5,647.46");
}

#[test]
fn selecting_another_account_opens_it_in_place_of_the_open_one() {
    let mut harness = app();
    harness
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .click();
    harness.run();

    harness
        .get_by_role_and_label(Role::Button, "Emergency Fund")
        .click();
    harness.run();

    harness.get_by_role_and_label(Role::Label, "Emergency Fund");
    assert!(
        harness
            .query_by_role_and_label(Role::Label, "Everyday Checking")
            .is_none()
    );
}

#[test]
fn folding_a_group_hides_its_accounts() {
    let mut harness = app();

    harness
        .get_by_role_and_label(Role::Button, "Banking")
        .click();
    harness.run();

    let banking = harness.get_by_role_and_label(Role::Button, "Banking");
    assert_eq!(banking.accesskit_node().toggled(), Some(Toggled::False));
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Everyday Checking")
            .is_none()
    );
}

#[test]
fn closed_accounts_start_folded_and_unfold() {
    let mut harness = app();
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Old Checking")
            .is_none()
    );

    harness
        .get_by_role_and_label(Role::Button, "Closed")
        .click();
    harness.run();

    harness.get_by_role_and_label(Role::Button, "Old Checking");
}

#[test]
fn hidden_accounts_are_left_out_but_count_in_net_worth() {
    let harness = app();

    let holiday_fund = harness.query_by_role_and_label(Role::Button, "Holiday Fund");

    assert!(holiday_fund.is_none());
    // The stub's Net Worth with the hidden Holiday Fund's $800.00 in it.
    harness.get_by_role_and_label(Role::Label, "$288,497.71");
}

/// The text of every `Label` on screen, which holds a tooltip's text as its value.
/// A tooltip shows among the window's other Labels, so tests check it is among them.
fn label_values(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::Label)
        .filter_map(|label| label.accesskit_node().value())
        .collect()
}

#[test]
fn hovering_an_account_whose_name_is_cut_short_shows_the_whole_name() {
    let mut harness = app();

    // "Household Checking" doesn't fit beside its balance in the default width.
    harness
        .get_by_role_and_label(Role::Button, "Household Checking")
        .hover();
    harness.run();

    assert!(
        label_values(&harness).contains(&"Household Checking".to_owned()),
        "{:?}",
        label_values(&harness)
    );
}

#[test]
fn hovering_an_account_whose_name_fits_shows_no_tooltip() {
    let mut harness = app();

    harness.get_by_role_and_label(Role::Button, "Car").hover();
    harness.run();

    assert!(!label_values(&harness).contains(&"Car".to_owned()));
}
