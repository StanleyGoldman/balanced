// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Editing a category from the Category List: renaming it, moving it under another
//! parent, and changing its type, driven through the AccessKit tree the way a user
//! drives it.

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

/// Selects the category named `name` in the Category List and opens Edit… on it.
fn edit(harness: &mut Harness<'static, App>, name: &str) {
    click(harness, Role::TreeItem, name);
    click(harness, Role::Button, "Edit…");
}

/// Replaces the text of the focused field with `text`, first passing it through the
/// App's `raw_input_hook` as eframe does, then runs the App until it settles.
fn retype(harness: &mut Harness<'static, App>, text: &str) {
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.run();
    let ctx = harness.ctx.clone();
    let mut input = std::mem::take(harness.input_mut());
    input.events.push(Event::Text(text.to_owned()));
    harness.state_mut().raw_input_hook(&ctx, &mut input);
    *harness.input_mut() = input;
    harness.step();
    harness.run();
}

/// Whether the Edit Category Dialog is open.
fn edit_open(harness: &Harness<'static, App>) -> bool {
    harness
        .query_by_role_and_label(Role::Window, "Edit Category")
        .is_some()
}

/// The names of the Category List's rows, top to bottom, down to the first transfer
/// row.
fn categories(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::TreeItem)
        .map(|row| row.accesskit_node().label().unwrap_or_default())
        .take_while(|name| !name.starts_with('['))
        .collect()
}

#[test]
fn edit_renames_the_selected_category_in_the_list_and_the_register() {
    let mut harness = category_list();
    edit(&mut harness, "Groceries");

    retype(&mut harness, "Supermarket");
    click(&mut harness, Role::Button, "OK");

    assert!(!edit_open(&harness));
    assert!(categories(&harness).contains(&"Supermarket".to_owned()));
    click(&mut harness, Role::Button, "Close");
    click(&mut harness, Role::Button, "Everyday Checking");
    assert!(
        harness
            .query_by_role_and_label(Role::Cell, "Food & Dining:Supermarket")
            .is_some()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Cell, "Food & Dining:Groceries")
            .is_none()
    );
}

/// Picks `item` from the Edit Category Dialog's Parent picker, scrolling it into view
/// first: a click on an item scrolled out of the picker lands outside the Dialog.
fn choose_parent(harness: &mut Harness<'static, App>, item: &str) {
    click(harness, Role::ComboBox, "Parent");
    harness
        .get_by_role_and_label(Role::Button, item)
        .scroll_to_me();
    harness.run();
    click(harness, Role::Button, item);
}

/// Whether a label reads `text`.
fn shows(harness: &Harness<'static, App>, text: &str) -> bool {
    harness
        .query_all_by_role(Role::Label)
        .any(|label| label.accesskit_node().value().as_deref() == Some(text))
}

/// What a screen reader reads after the name of the row named `name`: its type first.
fn usage(harness: &Harness<'static, App>, name: &str) -> String {
    harness
        .get_by_role_and_label(Role::TreeItem, name)
        .accesskit_node()
        .description()
        .unwrap_or_default()
}

/// Whether the type choice labelled `label` can be picked.
fn type_enabled(harness: &Harness<'static, App>, label: &str) -> bool {
    !harness
        .get_by_role_and_label(Role::RadioButton, label)
        .accesskit_node()
        .is_disabled()
}

#[test]
fn a_new_parent_moves_the_category_under_it() {
    let mut harness = category_list();
    edit(&mut harness, "Groceries");

    choose_parent(&mut harness, "Shopping");
    click(&mut harness, Role::Button, "OK");

    assert!(!edit_open(&harness));
    click(&mut harness, Role::Button, "Close");
    click(&mut harness, Role::Button, "Everyday Checking");
    assert!(
        harness
            .query_by_role_and_label(Role::Cell, "Shopping:Groceries")
            .is_some()
    );
}

#[test]
fn a_move_under_one_of_its_own_subcategories_is_refused_in_the_dialog() {
    let mut harness = category_list();
    edit(&mut harness, "Food & Dining");

    choose_parent(&mut harness, "Food & Dining:Restaurants");
    click(&mut harness, Role::Button, "OK");

    assert!(edit_open(&harness));
    assert!(shows(
        &harness,
        "A category can't go under itself or one of its own subcategories."
    ));
}

#[test]
fn a_move_beside_a_category_of_the_same_name_is_refused_in_the_dialog() {
    let mut harness = category_list();
    // Home's Insurance comes second in the tree, after Auto & Transport's.
    harness
        .query_all_by_role_and_label(Role::TreeItem, "Insurance")
        .nth(1)
        .expect("the stub has two categories named Insurance")
        .click();
    harness.run();
    click(&mut harness, Role::Button, "Edit…");

    choose_parent(&mut harness, "Auto & Transport");
    click(&mut harness, Role::Button, "OK");

    assert!(edit_open(&harness));
    assert!(shows(
        &harness,
        "A category named “Insurance” is already there."
    ));
}

#[test]
fn only_a_top_level_category_offers_a_type() {
    let mut harness = category_list();
    edit(&mut harness, "Groceries");
    let under_a_parent = type_enabled(&harness, "Income");

    choose_parent(&mut harness, "(Top level)");

    assert!(!under_a_parent);
    assert!(type_enabled(&harness, "Income"));
}

#[test]
fn a_top_level_categorys_new_type_is_its_subcategories_type_too() {
    let mut harness = category_list();
    edit(&mut harness, "Shopping");

    click(&mut harness, Role::RadioButton, "Income");
    click(&mut harness, Role::Button, "OK");

    assert!(usage(&harness, "Household").starts_with("Income:"));
}

#[test]
fn one_undo_takes_back_a_rename_and_a_move_made_together() {
    let mut harness = category_list();
    edit(&mut harness, "Groceries");
    retype(&mut harness, "Supermarket");
    choose_parent(&mut harness, "Shopping");
    click(&mut harness, Role::Button, "OK");
    click(&mut harness, Role::Button, "Close");

    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();

    click(&mut harness, Role::Button, "Everyday Checking");
    assert!(
        harness
            .query_by_role_and_label(Role::Cell, "Food & Dining:Groceries")
            .is_some()
    );
}

#[test]
fn a_renamed_category_is_offered_by_its_new_name_in_the_category_field() {
    let mut harness = category_list();
    edit(&mut harness, "Groceries");
    retype(&mut harness, "Supermarket");
    click(&mut harness, Role::Button, "OK");
    click(&mut harness, Role::Button, "Close");
    click(&mut harness, Role::Button, "Everyday Checking");
    click(&mut harness, Role::Cell, "Auto & Transport:Fuel");

    retype(&mut harness, "Food");

    let offered: Vec<String> = harness
        .query_all_by_role(Role::ListBoxOption)
        .map(|row| row.accesskit_node().label().unwrap_or_default())
        .collect();
    assert!(
        offered.contains(&"Food & Dining:Supermarket".to_owned()),
        "{offered:?}"
    );
    assert!(
        !offered.contains(&"Food & Dining:Groceries".to_owned()),
        "{offered:?}"
    );
}
