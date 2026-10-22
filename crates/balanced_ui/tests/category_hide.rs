// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Hiding a category from the Category List, and what a hidden category is in the
//! list and in the Register's Category field, driven through the AccessKit tree the way
//! a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on stub data, with the Category List open.
fn category_list() -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
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
    open_category_list(&mut harness);
    harness
}

/// Opens the Category List from the Tools menu.
fn open_category_list(harness: &mut Harness<'static, App>) {
    click(harness, Role::Button, "Tools");
    click(harness, Role::Button, "Category List…");
}

/// Clicks the node of `role` labelled `label`, then runs the App until it settles.
fn click(harness: &mut Harness<'static, App>, role: Role, label: &str) {
    harness.get_by_role_and_label(role, label).click();
    harness.run();
}

/// Plays `event` in one frame, first passing it through the App's `raw_input_hook`
/// as eframe does, then runs the App until it settles.
fn send(harness: &mut Harness<'static, App>, event: Event) {
    let ctx = harness.ctx.clone();
    let mut input = std::mem::take(harness.input_mut());
    input.events.push(event);
    harness.state_mut().raw_input_hook(&ctx, &mut input);
    *harness.input_mut() = input;
    harness.step();
    harness.run();
}

/// Presses `key` with `modifiers` held, then runs the App until it settles.
fn press(harness: &mut Harness<'static, App>, modifiers: Modifiers, key: Key) {
    send(
        harness,
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        },
    );
}

/// Selects the category named `name` in the Category List and clicks Hide.
fn hide(harness: &mut Harness<'static, App>, name: &str) {
    click(harness, Role::TreeItem, name);
    click(harness, Role::Button, "Hide");
}

/// Ticks or unticks Show hidden categories.
fn toggle_show_hidden(harness: &mut Harness<'static, App>) {
    click(harness, Role::CheckBox, "Show hidden categories");
}

/// The names of the Category List's rows, top to bottom, down to the first transfer
/// row.
fn categories(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::TreeItem)
        .filter_map(|row| row.accesskit_node().label())
        .take_while(|name| !name.starts_with('['))
        .collect()
}

/// What a screen reader reads after the name of the row named `name`.
fn description(harness: &Harness<'static, App>, name: &str) -> String {
    harness
        .get_by_role_and_label(Role::TreeItem, name)
        .accesskit_node()
        .description()
        .unwrap_or_default()
}

/// The stub's categories in the Category List, top to bottom, without those named in
/// `left_out`.
fn stub_categories_but(left_out: &[&str]) -> Vec<String> {
    [
        "Auto & Transport",
        "Fuel",
        "Insurance",
        "Food & Dining",
        "Groceries",
        "Restaurants",
        "Coffee Shops",
        "Home",
        "Insurance",
        "Utilities",
        "Income",
        "Interest",
        "Salary",
        "Shopping",
        "Household",
    ]
    .into_iter()
    .filter(|name| !left_out.contains(name))
    .map(str::to_owned)
    .collect()
}

#[test]
fn hide_takes_the_category_and_those_under_it_out_of_the_list() {
    let mut harness = category_list();

    hide(&mut harness, "Restaurants");

    assert_eq!(
        categories(&harness),
        stub_categories_but(&["Restaurants", "Coffee Shops"])
    );
}

#[test]
fn hiding_the_selected_category_drops_the_selection() {
    let mut harness = category_list();

    hide(&mut harness, "Restaurants");

    let selected = harness
        .query_all_by_role(Role::TreeItem)
        .any(|row| row.accesskit_node().is_selected() == Some(true));
    assert!(!selected);
    let edit = harness.get_by_role_and_label(Role::Button, "Edit…");
    assert!(edit.accesskit_node().is_disabled());
}

#[test]
fn show_hidden_categories_lists_them_marked_hidden() {
    let mut harness = category_list();
    hide(&mut harness, "Restaurants");

    toggle_show_hidden(&mut harness);

    assert_eq!(categories(&harness), stub_categories_but(&[]));
    assert!(
        description(&harness, "Coffee Shops").starts_with("Hidden, Expense: "),
        "{}",
        description(&harness, "Coffee Shops")
    );
    assert!(
        description(&harness, "Groceries").starts_with("Expense: "),
        "{}",
        description(&harness, "Groceries")
    );
}

#[test]
fn the_button_reads_unhide_on_a_hidden_category() {
    let mut harness = category_list();
    hide(&mut harness, "Restaurants");
    toggle_show_hidden(&mut harness);

    click(&mut harness, Role::TreeItem, "Coffee Shops");

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Hide")
            .is_none()
    );
    let unhide = harness.get_by_role_and_label(Role::Button, "Unhide");
    assert!(!unhide.accesskit_node().is_disabled());
}

#[test]
fn unhide_puts_the_category_back_in_the_list() {
    let mut harness = category_list();
    hide(&mut harness, "Restaurants");
    toggle_show_hidden(&mut harness);
    click(&mut harness, Role::TreeItem, "Restaurants");

    click(&mut harness, Role::Button, "Unhide");
    toggle_show_hidden(&mut harness);

    assert_eq!(categories(&harness), stub_categories_but(&[]));
}

#[test]
fn one_undo_shows_a_hidden_category_and_those_under_it_again() {
    let mut harness = category_list();
    hide(&mut harness, "Restaurants");
    click(&mut harness, Role::Button, "Close");

    press(&mut harness, Modifiers::COMMAND, Key::Z);
    open_category_list(&mut harness);

    assert_eq!(categories(&harness), stub_categories_but(&[]));
}

/// Hides Groceries, closes the Category List, opens Everyday Checking, and clicks
/// the Shell row's Category cell.
fn editing_shells_category_with_groceries_hidden() -> Harness<'static, App> {
    let mut harness = category_list();
    hide(&mut harness, "Groceries");
    click(&mut harness, Role::Button, "Close");
    click(&mut harness, Role::Button, "Everyday Checking");
    click(&mut harness, Role::Cell, "Auto & Transport:Fuel");
    harness
}

/// Replaces the text of the focused field with `text`.
fn retype(harness: &mut Harness<'static, App>, text: &str) {
    press(harness, Modifiers::COMMAND, Key::A);
    send(harness, Event::Text(text.to_owned()));
}

/// The rows of the open Category list.
fn offered(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::ListBoxOption)
        .map(|row| row.accesskit_node().label().unwrap_or_default())
        .collect()
}

/// How many Register cells read `text`.
fn cells_reading(harness: &Harness<'static, App>, text: &str) -> usize {
    harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| cell.accesskit_node().label().as_deref() == Some(text))
        .count()
}

#[test]
fn a_hidden_category_still_shows_in_the_register_cells_that_use_it() {
    let harness = editing_shells_category_with_groceries_hidden();

    assert!(cells_reading(&harness, "Food & Dining:Groceries") > 0);
}

#[test]
fn the_category_field_leaves_a_hidden_category_out_of_its_list() {
    let mut harness = editing_shells_category_with_groceries_hidden();

    retype(&mut harness, "Food");

    assert_eq!(
        offered(&harness),
        [
            "Food & Dining",
            "Food & Dining:Restaurants",
            "Food & Dining:Restaurants:Coffee Shops",
            "Create \"Food\"…",
        ]
    );
}

#[test]
fn the_category_field_never_completes_to_a_hidden_category() {
    let mut harness = editing_shells_category_with_groceries_hidden();

    retype(&mut harness, "Food & Dining:G");

    let editor = harness
        .get_by_role_and_label(Role::TextInput, "Category")
        .accesskit_node()
        .value();
    assert_eq!(editor.as_deref(), Some("Food & Dining:G"));
}

#[test]
fn a_hidden_categorys_full_path_typed_in_the_category_field_uses_it() {
    let mut harness = editing_shells_category_with_groceries_hidden();
    let before = cells_reading(&harness, "Food & Dining:Groceries");

    retype(&mut harness, "food & dining:groceries");
    let offered = offered(&harness);
    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(offered, Vec::<String>::new());
    assert!(
        harness
            .query_by_role_and_label(Role::Window, "New Category")
            .is_none()
    );
    assert_eq!(
        cells_reading(&harness, "Food & Dining:Groceries"),
        before + 1
    );
}
