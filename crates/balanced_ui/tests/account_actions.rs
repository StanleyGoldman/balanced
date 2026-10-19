//! The Account Actions menu at the right end of the context header, driven through the
//! AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on stub data, in the test locale.
fn app() -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .build_eframe(|cc| {
            App::on_ledger(
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

/// Clicks the button labelled `label`, then lets the App settle.
fn click(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run();
}

/// The App with `account` open in the Register.
fn app_on(account: &str) -> Harness<'static, App> {
    let mut harness = app();
    click(&mut harness, account);
    harness
}

/// The App with `account` open and its Account Actions menu open.
fn actions_on(account: &str) -> Harness<'static, App> {
    let mut harness = app_on(account);
    click(&mut harness, "Account Actions ⚙");
    harness
}

/// The text the Status bar shows.
fn status(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .get_by_role(Role::Status)
        .query_all_by_role(Role::Label)
        .filter_map(|label| label.accesskit_node().value())
        .collect()
}

#[test]
fn the_context_header_has_an_account_actions_button_after_new_transaction() {
    let harness = app_on("Everyday Checking");

    let actions = harness.get_by_role_and_label(Role::Button, "Account Actions ⚙");
    let add = harness.get_by_role_and_label(Role::Button, "+ Transaction");
    let chooser = harness.get_by_role_and_label(Role::Button, "Choose columns");

    assert!(actions.rect().left() > add.rect().right());
    assert!(chooser.rect().top() > actions.rect().bottom());
}

#[test]
fn the_menu_lists_the_account_tasks_in_order() {
    let harness = actions_on("Everyday Checking");

    let tops = [
        "Reconcile…",
        "Edit account details…",
        "Register preferences…",
        "Print Register…",
        "Export to CSV…",
    ]
    .map(|item| {
        harness
            .get_by_role_and_label(Role::Button, item)
            .rect()
            .top()
    });

    assert!(tops.is_sorted(), "{tops:?}");
}

#[test]
fn edit_account_details_opens_manage_accounts() {
    let mut harness = actions_on("Everyday Checking");

    click(&mut harness, "Edit account details…");

    harness.get_by_role_and_label(Role::Window, "Manage Accounts");
}

#[test]
fn register_preferences_opens_the_preferences_dialog() {
    let mut harness = actions_on("Everyday Checking");

    click(&mut harness, "Register preferences…");

    harness.get_by_role_and_label(Role::Window, "Preferences");
}

/// Clicks `item` in Everyday Checking's Account Actions menu, and returns what the
/// Status bar then shows.
fn status_after(item: &str) -> Vec<String> {
    let mut harness = actions_on("Everyday Checking");
    click(&mut harness, item);
    status(&harness)
}

#[test]
fn reconcile_says_it_is_not_available_yet() {
    assert_eq!(status_after("Reconcile…"), ["Not available yet"]);
}

#[test]
fn print_register_says_it_is_not_available_yet() {
    assert_eq!(status_after("Print Register…"), ["Not available yet"]);
}

#[test]
fn export_to_csv_says_it_is_not_available_yet() {
    assert_eq!(status_after("Export to CSV…"), ["Not available yet"]);
}

#[test]
fn reconcile_is_disabled_on_a_closed_account() {
    let mut harness = app();
    click(&mut harness, "Closed");

    click(&mut harness, "Old Checking");
    click(&mut harness, "Account Actions ⚙");

    let reconcile = harness.get_by_role_and_label(Role::Button, "Reconcile…");
    assert!(reconcile.accesskit_node().is_disabled());
}

#[test]
fn reconcile_is_enabled_on_an_open_account() {
    let harness = actions_on("Everyday Checking");

    let reconcile = harness.get_by_role_and_label(Role::Button, "Reconcile…");
    assert!(!reconcile.accesskit_node().is_disabled());
}
