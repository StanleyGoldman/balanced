// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Marking transactions Cleared from the Register's Clr column, driven through the
//! AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::egui::accesskit::{Action, Role};
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use egui_kittest::{Harness, Node};
use icu_calendar::Date;

/// Fresh Market's Clr cell in Everyday Checking, which starts Uncleared.
const FRESH_MARKET: &str = "Clr Fresh Market, 9/5/2026, -86.42";

/// Bean There's Clr cell in Everyday Checking, which starts Cleared.
const BEAN_THERE: &str = "Clr Bean There, 9/6/2026, -4.75";

/// The first salary's Clr cell in Everyday Checking, which starts Reconciled.
const ACME_CORP: &str = "Clr Acme Corp, 9/1/2026, 3,200.00";

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

/// The status the Clr cell labelled `label` reads out.
fn status(harness: &Harness<'static, App>, label: &str) -> String {
    harness
        .get_by_role_and_label(Role::Button, label)
        .accesskit_node()
        .value()
        .expect("a Clr cell reads out its status")
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

/// The Register's Clr cells, top to bottom.
fn clr_nodes<'h>(harness: &'h Harness<'static, App>) -> Vec<Node<'h>> {
    harness
        .query_all_by_role(Role::Button)
        .filter(|node| {
            node.accesskit_node()
                .label()
                .is_some_and(|label| label.starts_with("Clr "))
        })
        .collect()
}

/// The text of every Register cell that has some, top to bottom and left to right. The
/// selected row's Actions cell is a menu button rather than an empty cell, so empty
/// cells are left out.
fn cells(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::Cell)
        .filter_map(|cell| cell.accesskit_node().label())
        .filter(|label| !label.is_empty())
        .collect()
}

#[test]
fn clicking_an_uncleared_rows_clr_cell_marks_it_cleared() {
    let mut harness = app_showing("Everyday Checking");

    click(&mut harness, Role::Button, FRESH_MARKET);

    assert_eq!(status(&harness, FRESH_MARKET), "Cleared");
}

#[test]
fn clicking_a_cleared_rows_clr_cell_marks_it_uncleared() {
    let mut harness = app_showing("Everyday Checking");
    assert_eq!(status(&harness, BEAN_THERE), "Cleared");

    click(&mut harness, Role::Button, BEAN_THERE);

    assert_eq!(status(&harness, BEAN_THERE), "Uncleared");
}

#[test]
fn clicking_a_reconciled_rows_clr_cell_leaves_it_reconciled() {
    let mut harness = app_showing("Everyday Checking");

    click(&mut harness, Role::Button, ACME_CORP);

    assert_eq!(status(&harness, ACME_CORP), "Reconciled");
}

#[test]
fn undo_puts_back_the_status_a_click_changed() {
    let mut harness = app_showing("Everyday Checking");
    click(&mut harness, Role::Button, FRESH_MARKET);

    undo(&mut harness);

    assert_eq!(status(&harness, FRESH_MARKET), "Uncleared");
}

#[test]
fn a_click_leaves_every_row_in_its_place_with_its_balance() {
    let mut harness = app_showing("Everyday Checking");
    let before = cells(&harness);

    click(&mut harness, Role::Button, FRESH_MARKET);

    assert_eq!(cells(&harness), before);
}

#[test]
fn a_closed_accounts_clr_cells_are_disabled_and_take_no_click() {
    let harness = old_checking();

    let cells: Vec<(bool, bool)> = clr_nodes(&harness)
        .iter()
        .map(|node| {
            let node = node.accesskit_node();
            (
                node.is_disabled(),
                node.data().supports_action(Action::Click),
            )
        })
        .collect();

    assert_eq!(cells, [(true, false); 3]);
}

#[test]
fn only_transactions_have_clr_cells_and_the_draft_row_has_none() {
    let harness = app_showing("Joint Checking");

    let labels: Vec<String> = clr_nodes(&harness)
        .iter()
        .filter_map(|node| node.accesskit_node().label())
        .collect();

    assert_eq!(labels, ["Clr Opening Balance, 1/1/2026, 1,032.00"]);
}
