// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The keypad under the Register's Payment and Deposit cells, driven through the
//! AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;
use icu_locale::{langid, locale};

/// The App on stub data, in the test locale, wide enough to show every Register column.
fn app() -> Harness<'static, App> {
    app_in(Localizer::for_tests())
}

/// The App on stub data, in German with `de-DE` formatting.
fn app_in_german() -> Harness<'static, App> {
    app_in(
        Localizer::for_tests_in(&langid!("de"), &locale!("de-DE"))
            .expect("the embedded de messages and de-DE formatting data load"),
    )
}

/// The App on stub data, shown by `i18n`, wide enough to show every Register column.
fn app_in(i18n: Localizer) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(|cc| {
            App::on_ledger(
                cc,
                i18n,
                Ledger::stub(),
                Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
                PreferencesFile::for_tests(),
            )
        });
    harness.run();
    harness
}

/// Opens the account named `name` from the Account bar.
fn open(harness: &mut Harness<'static, App>, name: &str) {
    harness.get_by_role_and_label(Role::Button, name).click();
    harness.run();
}

/// Plays `event` in one frame, first passing it through the App's `raw_input_hook`
/// as eframe does. `egui_kittest` skips the hook.
fn send(harness: &mut Harness<'static, App>, event: Event) {
    let ctx = harness.ctx.clone();
    let mut input = std::mem::take(harness.input_mut());
    input.events.push(event);
    harness.state_mut().raw_input_hook(&ctx, &mut input);
    *harness.input_mut() = input;
    harness.step();
}

/// Presses `key` with `modifiers` held, then lets the App settle.
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
    harness.run();
}

/// Types `text` into whatever has the keyboard, then lets the App settle.
fn type_text(harness: &mut Harness<'static, App>, text: &str) {
    send(harness, Event::Text(text.to_owned()));
    harness.run();
}

/// Puts the cell cursor on the cell labelled `label` by clicking it.
fn click_cell(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::Cell, label).click();
    harness.run();
}

/// Clicks the button labelled `label`.
fn click_button(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run();
}

/// Opens Fresh Market's Payment cell, 86.42, for editing.
fn edit_fresh_market_payment(harness: &mut Harness<'static, App>) {
    open(harness, "Everyday Checking");
    click_cell(harness, "86.42");
    press(harness, Modifiers::NONE, Key::Enter);
}

/// The text in the open cell editor labelled `column`, or `None` when no such editor
/// is open.
fn editor(harness: &Harness<'static, App>, column: &str) -> Option<String> {
    harness
        .query_by_role_and_label(Role::TextInput, column)
        .map(|node| node.accesskit_node().value().unwrap_or_default())
}

/// The heading of the column holding the cell cursor.
fn cursor_column(harness: &Harness<'static, App>) -> String {
    let mut cursors = harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| cell.accesskit_node().is_selected() == Some(true));
    let cursor = cursors.next().expect("a cell holds the cell cursor");
    assert!(cursors.next().is_none(), "one cell holds the cell cursor");
    let x = cursor.rect().center().x;
    harness
        .query_all_by_role(Role::ColumnHeader)
        .find(|header| header.rect().x_range().contains(x))
        .and_then(|header| header.accesskit_node().label())
        .expect("a heading tops the cell cursor's column")
}

/// The labels of the Register cells under the header `title`, top row first.
fn column(harness: &Harness<'static, App>, title: &str) -> Vec<String> {
    let header = harness
        .get_by_role_and_label(Role::ColumnHeader, title)
        .rect()
        .x_range();
    harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| header.contains(cell.rect().center().x))
        .map(|cell| cell.accesskit_node().label().unwrap_or_default())
        .collect()
}

/// The Payment cell of the first row whose Payee is `payee`.
fn payment_of(harness: &Harness<'static, App>, payee: &str) -> String {
    let row = column(harness, "Payee")
        .iter()
        .position(|cell| cell == payee)
        .expect("the Register lists the payee");
    column(harness, "Payment").swap_remove(row)
}

/// The text in the keypad's display, or `None` while no keypad is open.
fn display(harness: &Harness<'static, App>) -> Option<String> {
    harness
        .query_by_role_and_label(Role::TextInput, "Keypad")
        .map(|node| node.accesskit_node().value().unwrap_or_default())
}

#[test]
fn the_keypad_button_opens_the_keypad_on_the_cells_number() {
    let mut harness = app();
    edit_fresh_market_payment(&mut harness);

    click_button(&mut harness, "Keypad");

    assert_eq!(display(&harness).as_deref(), Some("86.42"));
}

#[test]
fn alt_down_opens_the_keypad() {
    let mut harness = app();
    edit_fresh_market_payment(&mut harness);

    press(&mut harness, Modifiers::ALT, Key::ArrowDown);

    assert_eq!(display(&harness).as_deref(), Some("86.42"));
}

#[test]
fn an_operator_typed_in_the_cell_stays_in_the_cell() {
    let mut harness = app();
    edit_fresh_market_payment(&mut harness);

    type_text(&mut harness, "+");

    assert_eq!(display(&harness), None);
    assert_eq!(editor(&harness, "Payment").as_deref(), Some("86.42+"));
}

#[test]
fn an_empty_cell_opens_an_empty_keypad() {
    let mut harness = app();
    edit_fresh_market_payment(&mut harness);
    for _ in "86.42".chars() {
        press(&mut harness, Modifiers::NONE, Key::Backspace);
    }

    click_button(&mut harness, "Keypad");

    assert_eq!(display(&harness).as_deref(), Some(""));
}

#[test]
fn enter_in_the_keypad_puts_its_result_in_the_cell_and_moves_on() {
    let mut harness = app();
    edit_fresh_market_payment(&mut harness);
    click_button(&mut harness, "Keypad");
    click_button(&mut harness, "Clear");
    type_text(&mut harness, "154*2");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(display(&harness), None);
    assert_eq!(payment_of(&harness, "Fresh Market"), "308.00");
    assert_eq!(cursor_column(&harness), "Deposit");
}

#[test]
fn ok_puts_the_keypads_result_in_the_cell_and_moves_on() {
    let mut harness = app();
    edit_fresh_market_payment(&mut harness);
    click_button(&mut harness, "Keypad");
    click_button(&mut harness, "Clear");
    for key in ["1", "5", "4", "Multiply", "2"] {
        click_button(&mut harness, key);
    }

    click_button(&mut harness, "OK");

    assert_eq!(display(&harness), None);
    assert_eq!(payment_of(&harness, "Fresh Market"), "308.00");
    assert_eq!(cursor_column(&harness), "Deposit");
}

#[test]
fn the_keypad_on_the_draft_row_moves_on_without_adding_the_row() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    while cursor_column(&harness) != "Payment" {
        press(&mut harness, Modifiers::NONE, Key::ArrowRight);
    }
    press(&mut harness, Modifiers::NONE, Key::Enter);
    press(&mut harness, Modifiers::ALT, Key::ArrowDown);
    type_text(&mut harness, "12");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(cursor_column(&harness), "Deposit");
}

#[test]
fn escape_closes_the_keypad_and_leaves_the_cell_as_it_was() {
    let mut harness = app();
    edit_fresh_market_payment(&mut harness);
    click_button(&mut harness, "Keypad");
    type_text(&mut harness, "*2");

    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert_eq!(display(&harness), None);
    assert_eq!(editor(&harness, "Payment").as_deref(), Some("86.42"));
}

#[test]
fn a_click_outside_closes_the_keypad_and_leaves_the_cell_as_it_was() {
    let mut harness = app();
    edit_fresh_market_payment(&mut harness);
    click_button(&mut harness, "Keypad");
    type_text(&mut harness, "*2");

    click_cell(&mut harness, "Bean There");

    assert_eq!(display(&harness), None);
    assert_eq!(payment_of(&harness, "Fresh Market"), "86.42");
}

/// Whether a label in the window reads `text`.
fn shows(harness: &Harness<'static, App>, text: &str) -> bool {
    harness
        .query_all_by_role(Role::Label)
        .any(|label| label.accesskit_node().value().as_deref() == Some(text))
}

/// Opens Fresh Market's Payment cell's keypad, empty, and types `sum` in it.
fn type_in_keypad(harness: &mut Harness<'static, App>, sum: &str) {
    edit_fresh_market_payment(harness);
    click_button(harness, "Keypad");
    click_button(harness, "Clear");
    type_text(harness, sum);
}

#[test]
fn a_rounded_result_is_marked() {
    let mut harness = app();

    type_in_keypad(&mut harness, "100/3");

    assert!(shows(&harness, "~ $33.33"));
}

#[test]
fn a_sum_ending_in_an_operator_waits() {
    let mut harness = app();

    type_in_keypad(&mut harness, "12+");

    assert!(shows(&harness, "…"));
}

#[test]
fn text_that_is_no_sum_is_not_an_amount() {
    let mut harness = app();

    type_in_keypad(&mut harness, "1.2.3");

    assert!(shows(&harness, "Not an amount"));
}

#[test]
fn ok_refuses_text_that_is_not_an_amount() {
    let mut harness = app();
    type_in_keypad(&mut harness, "1.2.3");

    click_button(&mut harness, "OK");

    assert_eq!(display(&harness).as_deref(), Some("1.2.3"));
    assert_eq!(payment_of(&harness, "Fresh Market"), "86.42");
}

#[test]
fn the_cells_preview_stays_while_the_keypad_is_open() {
    let mut harness = app();
    edit_fresh_market_payment(&mut harness);
    type_text(&mut harness, "+5");
    click_button(&mut harness, "Keypad");

    click_button(&mut harness, "Clear");

    assert!(shows(&harness, "= $91.42"));
}

#[test]
fn the_deposit_cell_has_a_keypad_too() {
    let mut harness = app();
    edit_fresh_market_payment(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Tab);

    press(&mut harness, Modifiers::ALT, Key::ArrowDown);

    assert_eq!(editor(&harness, "Deposit").as_deref(), Some(""));
    assert_eq!(display(&harness).as_deref(), Some(""));
}

#[test]
fn the_keypads_display_refuses_letters() {
    let mut harness = app();

    type_in_keypad(&mut harness, "1a+b2");

    assert_eq!(display(&harness).as_deref(), Some("1+2"));
}

#[test]
fn the_keypad_writes_german_amounts_with_a_decimal_comma() {
    let mut harness = app_in_german();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "86,42");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    click_button(&mut harness, "Rechner");
    click_button(&mut harness, "Löschen");
    for key in ["1", "Dezimaltrennzeichen", "5", "Multiplizieren", "2"] {
        click_button(&mut harness, key);
    }

    click_button(&mut harness, "OK");

    harness.get_by_role_and_label(Role::Cell, "3,00");
}
