// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Creating a category from a Split dialog line's Category field, through the New
//! Category Dialog opened over the Split dialog.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on stub data, in the test locale, wide enough to show every Register column,
/// with Everyday Checking open. It holds Costco, split between Groceries and Household.
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
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .click();
    harness.run();
    harness
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

/// Presses `key` with no modifiers, then lets the App settle.
fn press(harness: &mut Harness<'static, App>, key: Key) {
    send(
        harness,
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        },
    );
    harness.run();
}

/// Types `text` into whatever has the keyboard, then lets the App settle.
fn type_text(harness: &mut Harness<'static, App>, text: &str) {
    send(harness, Event::Text(text.to_owned()));
    harness.run();
}

/// Clicks the button labelled `label`.
fn click_button(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run();
}

/// The text of the field labelled `label`, or `None` when none is shown.
fn field(harness: &Harness<'static, App>, label: &str) -> Option<String> {
    harness
        .query_by_role_and_label(Role::TextInput, label)
        .map(|node| node.accesskit_node().value().unwrap_or_default())
}

/// The labels of the rows of the open combobox list, top first.
fn list_rows(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::ListBoxOption)
        .map(|row| row.accesskit_node().label().unwrap_or_default())
        .collect()
}

/// The App with the Split dialog open on Costco and `typed` typed into a third line's
/// Category field, whose list is open.
fn typed_in_line_3(typed: &str) -> Harness<'static, App> {
    let mut harness = app();
    harness.get_by_role_and_label(Role::Cell, "[Split]").click();
    harness.run();
    click_button(&mut harness, "Add line");
    type_text(&mut harness, typed);
    harness
}

#[test]
fn typing_an_unknown_category_in_a_line_offers_to_create_it() {
    let harness = typed_in_line_3("Pets");

    assert_eq!(list_rows(&harness), ["Create \"Pets\"…"]);
    assert_eq!(field(&harness, "Line 3 category").as_deref(), Some("Pets"));
}

/// The App with New Category open over the Split dialog from line 3's Create row for
/// `typed`, taken with Enter.
fn new_category_for(typed: &str) -> Harness<'static, App> {
    let mut harness = typed_in_line_3(typed);
    press(&mut harness, Key::Enter);
    harness
}

#[test]
fn enter_on_the_create_row_opens_new_category_over_the_split_dialog_prefilled() {
    let harness = new_category_for("Pets");

    assert_eq!(field(&harness, "Name").as_deref(), Some("Pets"));
    assert_eq!(field(&harness, "Line 3 category").as_deref(), Some("Pets"));
}

#[test]
fn clicking_the_create_row_opens_new_category_prefilled() {
    let mut harness = typed_in_line_3("Pets");

    harness
        .get_by_role_and_label(Role::ListBoxOption, "Create \"Pets\"…")
        .click();
    harness.run();

    assert_eq!(field(&harness, "Name").as_deref(), Some("Pets"));
}

/// Whether the field labelled `label` has keyboard focus.
fn focused(harness: &Harness<'static, App>, label: &str) -> bool {
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .accesskit_node()
        .is_focused()
}

/// Replaces the text of the field labelled `label` with `text`.
fn fill(harness: &mut Harness<'static, App>, label: &str, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .click();
    harness.run();
    send(
        harness,
        Event::Key {
            key: Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        },
    );
    type_text(harness, text);
}

#[test]
fn ok_names_the_new_category_in_the_line_and_gives_it_focus_back() {
    let mut harness = new_category_for("Pets");

    click_button(&mut harness, "OK");

    assert_eq!(field(&harness, "Name"), None);
    assert_eq!(field(&harness, "Line 3 category").as_deref(), Some("Pets"));
    assert!(focused(&harness, "Line 3 category"));
}

#[test]
fn done_after_creating_a_category_saves_the_split_with_it() {
    let mut harness = new_category_for("Pets");
    click_button(&mut harness, "OK");
    fill(&mut harness, "Line 1 amount", "100.00");
    fill(&mut harness, "Line 3 amount", "42.37");

    click_button(&mut harness, "Done");
    assert_eq!(field(&harness, "Line 1 category"), None);
    click_button(&mut harness, "Save");
    press(&mut harness, Key::Escape);
    harness.get_by_role_and_label(Role::Cell, "[Split]").click();
    harness.run();

    assert_eq!(field(&harness, "Line 3 category").as_deref(), Some("Pets"));
    assert_eq!(field(&harness, "Line 3 amount").as_deref(), Some("42.37"));
}

/// Line 1 and 2's Category and Amount fields, which Costco's split fills.
fn costco_lines(harness: &Harness<'static, App>) -> Vec<Option<String>> {
    [
        "Line 1 category",
        "Line 1 amount",
        "Line 2 category",
        "Line 2 amount",
    ]
    .map(|label| field(harness, label))
    .to_vec()
}

/// [`costco_lines`] as the split holds them.
fn costco_split() -> Vec<Option<String>> {
    [
        "Food & Dining:Groceries",
        "142.37",
        "Shopping:Household",
        "63.90",
    ]
    .map(|text| Some(text.to_owned()))
    .to_vec()
}

#[test]
fn cancel_gives_the_line_its_typed_text_and_focus_back_and_leaves_the_split_alone() {
    let mut harness = new_category_for("Pets");

    // The Split dialog behind has a Cancel button too.
    harness
        .get_by_role_and_label(Role::Window, "New Category")
        .get_by_role_and_label(Role::Button, "Cancel")
        .click();
    harness.run();

    assert_eq!(field(&harness, "Name"), None);
    assert_eq!(field(&harness, "Line 3 category").as_deref(), Some("Pets"));
    assert!(focused(&harness, "Line 3 category"));
    assert_eq!(costco_lines(&harness), costco_split());
}

#[test]
fn escape_in_new_category_cancels_it_and_keeps_the_split_dialog_open() {
    let mut harness = new_category_for("Pets");

    press(&mut harness, Key::Escape);

    assert_eq!(field(&harness, "Name"), None);
    assert_eq!(field(&harness, "Line 3 category").as_deref(), Some("Pets"));
    assert_eq!(costco_lines(&harness), costco_split());
}
