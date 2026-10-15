//! Register tabs, driven through the AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::App;
use eframe::egui::accesskit::{Role, Toggled};
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

/// The App on stub data, in the test locale, wide enough to show every Register column.
fn app() -> Harness<'static, App> {
    Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(|cc| App::new(cc, Localizer::for_tests(), Ledger::stub()))
}

/// Opens the account named `name` in a new Register tab, from its right-click menu in
/// the Account bar.
fn open_in_new_tab(harness: &mut Harness<'static, App>, name: &str) {
    harness
        .get_by_role_and_label(Role::Button, name)
        .click_secondary();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Open in New Tab")
        .click();
    harness.run();
}

/// The labels of the Register tabs, left to right.
fn tabs(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::Tab)
        .map(|tab| tab.accesskit_node().label().unwrap_or_default())
        .collect()
}

/// The label of the current Register tab.
fn current_tab(harness: &Harness<'static, App>) -> String {
    let current = harness
        .query_all_by_role(Role::Tab)
        .find(|tab| tab.accesskit_node().is_selected() == Some(true))
        .expect("a Register tab is current");
    current.accesskit_node().label().unwrap_or_default()
}

/// Presses `key` with `modifiers` held, as a single event, and runs the App until it
/// settles.
fn press(harness: &mut Harness<'static, App>, modifiers: Modifiers, key: Key) {
    harness.event(Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    });
    harness.run();
}

/// The App with Everyday Checking, Emergency Fund and Visa Rewards open in Register
/// tabs, the last of them current.
fn three_tabs() -> Harness<'static, App> {
    let mut harness = app();
    open_in_new_tab(&mut harness, "Everyday Checking");
    open_in_new_tab(&mut harness, "Emergency Fund");
    open_in_new_tab(&mut harness, "Visa Rewards");
    harness
}

#[test]
fn open_in_new_tab_adds_a_register_tab() {
    let mut harness = app();

    open_in_new_tab(&mut harness, "Everyday Checking");
    open_in_new_tab(&mut harness, "Emergency Fund");

    assert_eq!(tabs(&harness), ["Everyday Checking", "Emergency Fund"]);
}

#[test]
fn a_tabs_close_button_closes_it() {
    let mut harness = app();
    open_in_new_tab(&mut harness, "Everyday Checking");
    open_in_new_tab(&mut harness, "Emergency Fund");

    harness
        .get_by_role_and_label(Role::Button, "Close Everyday Checking")
        .click();
    harness.run();

    assert_eq!(tabs(&harness), ["Emergency Fund"]);
}

#[test]
fn ctrl_w_closes_the_current_tab() {
    let mut harness = three_tabs();

    press(&mut harness, Modifiers::COMMAND, Key::W);

    assert_eq!(tabs(&harness), ["Everyday Checking", "Emergency Fund"]);
    assert_eq!(current_tab(&harness), "Emergency Fund");
}

#[test]
fn ctrl_f4_closes_the_current_tab() {
    let mut harness = three_tabs();

    press(&mut harness, Modifiers::COMMAND, Key::F4);

    assert_eq!(tabs(&harness), ["Everyday Checking", "Emergency Fund"]);
}

#[test]
fn ctrl_tab_moves_to_the_next_tab_and_wraps_to_the_first() {
    let mut harness = three_tabs();

    press(&mut harness, Modifiers::COMMAND, Key::Tab);

    assert_eq!(current_tab(&harness), "Everyday Checking");
    press(&mut harness, Modifiers::COMMAND, Key::Tab);
    assert_eq!(current_tab(&harness), "Emergency Fund");
}

#[test]
fn ctrl_shift_tab_moves_to_the_previous_tab_and_wraps_to_the_last() {
    let mut harness = three_tabs();
    harness
        .get_by_role_and_label(Role::Tab, "Everyday Checking")
        .click();
    harness.run();

    press(
        &mut harness,
        Modifiers::COMMAND | Modifiers::SHIFT,
        Key::Tab,
    );

    assert_eq!(current_tab(&harness), "Visa Rewards");
    press(
        &mut harness,
        Modifiers::COMMAND | Modifiers::SHIFT,
        Key::Tab,
    );
    assert_eq!(current_tab(&harness), "Emergency Fund");
}

/// Opens the Menu bar menu named `menu`.
fn open_menu(harness: &mut Harness<'static, App>, menu: &str) {
    harness.get_by_role_and_label(Role::Button, menu).click();
    harness.run();
}

/// The full label of the menu item whose label starts with `item`. egui reads a menu
/// item's label and its Shortcut text as one label.
fn menu_item_label(harness: &Harness<'static, App>, item: &str) -> String {
    harness
        .query_all_by_role(Role::Button)
        .filter_map(|button| button.accesskit_node().label())
        .find(|label| label.starts_with(item))
        .unwrap_or_else(|| panic!("the open menu has {item}"))
}

#[test]
fn the_menus_show_the_tab_shortcuts() {
    let mut harness = three_tabs();

    open_menu(&mut harness, "View");

    assert_eq!(
        menu_item_label(&harness, "Next Register Tab"),
        "Next Register Tab Ctrl+Tab"
    );
    assert_eq!(
        menu_item_label(&harness, "Previous Register Tab"),
        "Previous Register Tab Ctrl+Shift+Tab"
    );
    open_menu(&mut harness, "File");
    assert_eq!(
        menu_item_label(&harness, "Close Register Tab"),
        "Close Register Tab Ctrl+W"
    );
}

#[test]
fn file_close_register_tab_closes_the_current_tab() {
    let mut harness = three_tabs();
    open_menu(&mut harness, "File");

    let close = menu_item_label(&harness, "Close Register Tab");
    harness.get_by_role_and_label(Role::Button, &close).click();
    harness.run();

    assert_eq!(tabs(&harness), ["Everyday Checking", "Emergency Fund"]);
}

#[test]
fn the_tab_menu_items_are_disabled_while_no_tab_is_open() {
    let mut harness = app();

    open_menu(&mut harness, "View");

    let next = menu_item_label(&harness, "Next Register Tab");
    let next = harness.get_by_role_and_label(Role::Button, &next);
    assert!(next.accesskit_node().is_disabled());
    open_menu(&mut harness, "File");
    let close = menu_item_label(&harness, "Close Register Tab");
    let close = harness.get_by_role_and_label(Role::Button, &close);
    assert!(close.accesskit_node().is_disabled());
}

#[test]
fn a_tab_keeps_its_selected_row_while_another_tab_is_current() {
    let mut harness = app();
    open_in_new_tab(&mut harness, "Everyday Checking");
    harness
        .get_by_role_and_label(Role::Cell, "Fresh Market")
        .click();
    harness.run();
    open_in_new_tab(&mut harness, "Emergency Fund");

    harness
        .get_by_role_and_label(Role::Tab, "Everyday Checking")
        .click();
    harness.run();

    let payee = harness.get_by_role_and_label(Role::Cell, "Fresh Market");
    assert_eq!(payee.accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn selecting_an_account_opens_it_in_the_current_tab() {
    let mut harness = three_tabs();
    harness
        .get_by_role_and_label(Role::Tab, "Emergency Fund")
        .click();
    harness.run();

    harness
        .get_by_role_and_label(Role::Button, "Joint Checking")
        .click();
    harness.run();

    assert_eq!(
        tabs(&harness),
        ["Everyday Checking", "Joint Checking", "Visa Rewards"]
    );
    assert_eq!(current_tab(&harness), "Joint Checking");
}

#[test]
fn opening_an_account_that_has_a_tab_makes_its_tab_current() {
    let mut harness = three_tabs();

    open_in_new_tab(&mut harness, "Everyday Checking");

    assert_eq!(current_tab(&harness), "Everyday Checking");
    // A second Everyday Checking tab would share the first's AccessKit node, so the
    // tab after the current one tells them apart.
    press(&mut harness, Modifiers::COMMAND, Key::Tab);
    assert_eq!(current_tab(&harness), "Emergency Fund");
}
