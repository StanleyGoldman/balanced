//! Flagging transactions from the Register's Flag column and its Actions menu, driven
//! through the AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::egui::accesskit::{Action, Role, Toggled};
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// Fresh Market's Flag cell in Everyday Checking, which starts unflagged.
const FRESH_MARKET: &str = "Flag Fresh Market, 9/5/2026, -86.42";

/// City Power & Light's Flag cell in Everyday Checking, which starts flagged.
const CITY_POWER: &str = "Flag City Power & Light, 9/10/2026, -92.10";

/// The App on stub data, in the test locale, wide enough to show every Register column,
/// with the account named `name` open.
fn app_showing(name: &str) -> Harness<'static, App> {
    let mut harness = app();
    click(&mut harness, Role::Button, name);
    harness
}

/// The App with Old Checking, a closed account, open from the Closed section.
fn old_checking() -> Harness<'static, App> {
    let mut harness = app();
    click(&mut harness, Role::Button, "Closed");
    click(&mut harness, Role::Button, "Old Checking");
    harness
}

/// The App on stub data, in the test locale, wide enough to show every Register column.
fn app() -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 720.0))
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

/// Clicks the widget with `role` labelled `label`, then lets the App settle.
fn click(harness: &mut Harness<'static, App>, role: Role, label: &str) {
    harness.get_by_role_and_label(role, label).click();
    harness.run();
}

/// Whether the Flag cell labelled `label` shows its transaction flagged.
fn flagged(harness: &Harness<'static, App>, label: &str) -> bool {
    let toggled = harness
        .get_by_role_and_label(Role::CheckBox, label)
        .accesskit_node()
        .toggled();
    toggled == Some(Toggled::True)
}

/// Presses Ctrl+Z, Undo, then lets the App settle.
fn undo(harness: &mut Harness<'static, App>) {
    harness.event(Event::Key {
        key: Key::Z,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    });
    harness.run();
}

/// Selects the row whose Payee cell reads `payee` and picks `item` from its Actions
/// menu.
fn pick_action(harness: &mut Harness<'static, App>, payee: &str, item: &str) {
    click(harness, Role::Cell, payee);
    click(harness, Role::Button, "Actions");
    click(harness, Role::Button, item);
}

/// The labels of the Register's Flag cells.
fn flag_cells(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::CheckBox)
        .filter_map(|node| node.accesskit_node().label())
        .filter(|label| label.starts_with("Flag "))
        .collect()
}

#[test]
fn clicking_an_unflagged_rows_flag_cell_flags_it() {
    let mut harness = app_showing("Everyday Checking");

    click(&mut harness, Role::CheckBox, FRESH_MARKET);

    assert!(flagged(&harness, FRESH_MARKET));
}

#[test]
fn clicking_a_flagged_rows_flag_cell_unflags_it() {
    let mut harness = app_showing("Everyday Checking");
    assert!(flagged(&harness, CITY_POWER));

    click(&mut harness, Role::CheckBox, CITY_POWER);

    assert!(!flagged(&harness, CITY_POWER));
}

#[test]
fn undo_takes_a_clicked_flag_off_again() {
    let mut harness = app_showing("Everyday Checking");
    click(&mut harness, Role::CheckBox, FRESH_MARKET);

    undo(&mut harness);

    assert!(!flagged(&harness, FRESH_MARKET));
}

#[test]
fn flag_in_the_actions_menu_flags_the_row() {
    let mut harness = app_showing("Everyday Checking");

    pick_action(&mut harness, "Fresh Market", "Flag");

    assert!(flagged(&harness, FRESH_MARKET));
}

#[test]
fn unflag_in_the_actions_menu_unflags_the_row() {
    let mut harness = app_showing("Everyday Checking");

    pick_action(&mut harness, "City Power & Light", "Unflag");

    assert!(!flagged(&harness, CITY_POWER));
}

#[test]
fn undo_puts_back_a_flag_taken_off_in_the_actions_menu() {
    let mut harness = app_showing("Everyday Checking");
    pick_action(&mut harness, "City Power & Light", "Unflag");

    undo(&mut harness);

    assert!(flagged(&harness, CITY_POWER));
}

#[test]
fn a_closed_accounts_flag_cells_take_no_click() {
    let harness = old_checking();

    let clickable: Vec<bool> = harness
        .query_all_by_role(Role::CheckBox)
        .filter(|node| {
            node.accesskit_node()
                .label()
                .is_some_and(|label| label.starts_with("Flag "))
        })
        .map(|node| node.accesskit_node().data().supports_action(Action::Click))
        .collect();

    assert_eq!(clickable, [false, false, false]);
}

#[test]
fn a_closed_accounts_actions_menu_cant_flag() {
    let mut harness = old_checking();
    click(&mut harness, Role::Cell, "Cash withdrawal");

    click(&mut harness, Role::Button, "Actions");

    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Flag")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn only_transactions_have_flag_cells_and_the_draft_row_has_none() {
    let harness = app_showing("Joint Checking");

    let cells = flag_cells(&harness);

    assert_eq!(cells, ["Flag Opening Balance, 1/1/2026, 1,032.00"]);
}
