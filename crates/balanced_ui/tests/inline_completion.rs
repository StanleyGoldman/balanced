// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Inline completion: typing in a Payee or Category cell completes the text with the
//! rest of the first entry that starts with it, selected, which Tab or Enter takes.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on stub data, in the test locale, wide enough to show every Register
/// column, keeping its Preferences in `preferences`, with Everyday Checking open and
/// its cell cursor on the draft row's Date cell.
fn everyday_checking_with(preferences: PreferencesFile) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 720.0))
        .build_eframe(|cc| {
            App::on_ledger(
                cc,
                Localizer::for_tests(),
                Ledger::stub(),
                Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
                preferences,
            )
        });
    harness.run();
    click(&mut harness, Role::Button, "Everyday Checking");
    harness
}

/// The App on stub data with Everyday Checking open, its cell cursor on the draft
/// row's Date cell.
fn everyday_checking() -> Harness<'static, App> {
    everyday_checking_with(PreferencesFile::for_tests())
}

/// Clicks the widget with `role` labelled `label`, then lets the App settle.
fn click(harness: &mut Harness<'static, App>, role: Role, label: &str) {
    harness.get_by_role_and_label(role, label).click();
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

/// Types `text` in one go into whatever has the keyboard.
fn type_text(harness: &mut Harness<'static, App>, text: &str) {
    send(harness, Event::Text(text.to_owned()));
    harness.run();
}

/// Tabs from the draft row's Date cell to its Payee cell and types `payee` there.
fn type_payee(harness: &mut Harness<'static, App>, payee: &str) {
    press(harness, Modifiers::NONE, Key::Tab);
    press(harness, Modifiers::NONE, Key::Tab);
    type_text(harness, payee);
}

/// The text in the cell editor of the `column` column.
fn editor(harness: &Harness<'static, App>, column: &str) -> String {
    harness
        .get_by_role_and_label(Role::TextInput, column)
        .accesskit_node()
        .value()
        .expect("a text field has a value")
}

#[test]
fn typing_the_start_of_a_payee_completes_it() {
    let mut harness = everyday_checking();

    type_payee(&mut harness, "Tratt");

    assert_eq!(editor(&harness, "Payee"), "Trattoria Roma");
}

#[test]
fn typing_over_a_completion_completes_again_from_the_new_text() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "S");

    type_text(&mut harness, "h");

    assert_eq!(editor(&harness, "Payee"), "Shell");
}

#[test]
fn backspace_drops_the_completion_and_keeps_the_typed_text() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "Tratt");

    press(&mut harness, Modifiers::NONE, Key::Backspace);

    assert_eq!(editor(&harness, "Payee"), "Tratt");
}

#[test]
fn delete_drops_the_completion_and_keeps_the_typed_text() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "Tratt");

    press(&mut harness, Modifiers::NONE, Key::Delete);

    assert_eq!(editor(&harness, "Payee"), "Tratt");
}

#[test]
fn backspace_after_dropping_a_completion_deletes_a_typed_character() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "Tratt");
    press(&mut harness, Modifiers::NONE, Key::Backspace);

    press(&mut harness, Modifiers::NONE, Key::Backspace);

    assert_eq!(editor(&harness, "Payee"), "Trat");
}

/// The draft row's cells, from Date to Deposit. It is the last row.
fn draft_row(harness: &Harness<'static, App>) -> Vec<String> {
    let cells: Vec<_> = harness
        .query_all_by_role(Role::Cell)
        .map(|cell| {
            let label = cell.accesskit_node().label().unwrap_or_default();
            (cell.rect().top(), label)
        })
        .collect();
    let last = cells.last().expect("the Register shows the draft row").0;
    cells
        .into_iter()
        .filter(|(top, _)| (*top - last).abs() < 1.0)
        .take(7)
        .map(|(_, label)| label)
        .collect()
}

/// The labels of the open list's highlighted rows.
fn highlighted(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::ListBoxOption)
        .filter(|row| row.accesskit_node().is_selected() == Some(true))
        .map(|row| row.accesskit_node().label().unwrap_or_default())
        .collect()
}

#[test]
fn the_list_highlights_the_completed_entry() {
    let mut harness = everyday_checking();

    // "Fresh Market" holds "ma" too, and lists first.
    type_payee(&mut harness, "Ma");

    assert_eq!(highlighted(&harness), ["Maple Property Management"]);
}

#[test]
fn tab_takes_a_completion_spelled_as_its_entry_and_recalls_its_payee() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "ma");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Maple Property Management",
            "Home",
            "November rent",
            "1,475.00",
            "",
        ]
    );
}

#[test]
fn enter_takes_a_completion_and_recalls_its_payee() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "ma");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Maple Property Management",
            "Home",
            "November rent",
            "1,475.00",
            "",
        ]
    );
}

#[test]
fn tab_after_dropping_a_completion_commits_the_typed_text() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "Tratt");
    press(&mut harness, Modifiers::NONE, Key::Backspace);

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(draft_row(&harness), ["", "", "Tratt", "", "", "", ""]);
}

/// Clicks the Shell row's Category cell and types `typed` over it.
fn type_over_shells_category(harness: &mut Harness<'static, App>, typed: &str) {
    click(harness, Role::Cell, "Auto & Transport:Fuel");
    type_text(harness, typed);
}

#[test]
fn typing_the_start_of_a_category_path_completes_it() {
    let mut harness = everyday_checking();

    type_over_shells_category(&mut harness, "food & dining:gr");

    assert_eq!(editor(&harness, "Category"), "food & dining:groceries");
}

#[test]
fn typing_the_start_of_an_account_in_brackets_completes_it() {
    let mut harness = everyday_checking();

    type_over_shells_category(&mut harness, "[Emer");

    assert_eq!(editor(&harness, "Category"), "[Emergency Fund]");
}

#[test]
fn text_no_entry_starts_with_is_left_as_typed() {
    let mut harness = everyday_checking();

    // "Fresh Market" holds "market", but doesn't start with it.
    type_payee(&mut harness, "Market");

    assert_eq!(editor(&harness, "Payee"), "Market");
}

#[test]
fn typing_with_the_caret_before_the_end_completes_nothing() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "es");
    press(&mut harness, Modifiers::NONE, Key::Home);

    type_text(&mut harness, "Fr");

    assert_eq!(editor(&harness, "Payee"), "Fres");
}

#[test]
fn the_memo_cell_completes_nothing() {
    let mut harness = everyday_checking();
    for _ in 0..4 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
    }

    type_text(&mut harness, "Tratt");

    assert_eq!(editor(&harness, "Memo"), "Tratt");
}

#[test]
fn tab_on_a_category_with_no_completion_still_takes_the_create_row() {
    let mut harness = everyday_checking();
    type_over_shells_category(&mut harness, "Pets");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    let name = harness
        .get_by_role_and_label(Role::TextInput, "Name")
        .accesskit_node()
        .value();
    assert_eq!(name.as_deref(), Some("Pets"));
}

/// Everyday Checking open as [`everyday_checking`] does, with Inline completion turned
/// off in `preferences.toml`, which lives in the returned directory.
fn everyday_checking_without_completion() -> (tempfile::TempDir, Harness<'static, App>) {
    let dir = tempfile::tempdir().expect("the test machine can make a temporary directory");
    std::fs::write(
        dir.path().join("preferences.toml"),
        "version = 2\ninline_completion = false\n",
    )
    .expect("the test can write the preferences file");
    let harness = everyday_checking_with(PreferencesFile::in_dir(dir.path()));
    (dir, harness)
}

#[test]
fn with_inline_completion_off_typing_completes_nothing() {
    let (_dir, mut harness) = everyday_checking_without_completion();

    type_payee(&mut harness, "Tratt");

    assert_eq!(editor(&harness, "Payee"), "Tratt");
}

#[test]
fn with_inline_completion_off_tab_takes_the_lists_top_row() {
    let (_dir, mut harness) = everyday_checking_without_completion();
    // "Fresh Market" holds "ma" too, and lists first.
    type_payee(&mut harness, "Ma");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(draft_row(&harness)[2], "Fresh Market");
}

#[test]
fn with_inline_completion_off_a_whole_typed_payee_still_recalls() {
    let (_dir, mut harness) = everyday_checking_without_completion();
    type_payee(&mut harness, "Maple Property Management");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Maple Property Management",
            "Home",
            "November rent",
            "1,475.00",
            "",
        ]
    );
}

#[test]
fn turning_inline_completion_off_in_the_preferences_dialog_stops_it_on_ok() {
    let dir = tempfile::tempdir().expect("the test machine can make a temporary directory");
    let mut harness = everyday_checking_with(PreferencesFile::in_dir(dir.path()));
    click(&mut harness, Role::Button, "Tools");
    click(&mut harness, Role::Button, "Preferences…");
    click(&mut harness, Role::Tab, "Register");
    click(
        &mut harness,
        Role::CheckBox,
        "Complete payees and categories as you type",
    );
    click(&mut harness, Role::Button, "OK");
    // The Dialog took the cell cursor; Ctrl+N puts it back on the draft row.
    press(&mut harness, Modifiers::COMMAND, Key::N);

    type_payee(&mut harness, "Tratt");

    assert_eq!(editor(&harness, "Payee"), "Tratt");
}

#[test]
fn esc_drops_the_completion_with_the_list() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "Tratt");

    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert_eq!(editor(&harness, "Payee"), "Tratt");
}

#[test]
fn backspace_after_typing_a_whole_entry_keeps_the_lists_top_row_for_tab() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "shell");
    press(&mut harness, Modifiers::NONE, Key::Backspace);

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(draft_row(&harness)[2], "Shell");
}
