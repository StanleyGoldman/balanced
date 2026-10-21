// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The leave-row prompt: leaving a changed row of the Register asks Save, Continue
//! editing or Discard first.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::{Role, Toggled};
use eframe::egui::{Event, Key, Modifiers, ViewportEvent, ViewportId, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

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

/// Clicks the button labelled `label`, other than one in the editing row's toolbar.
fn click_button(harness: &mut Harness<'static, App>, label: &str) {
    dialog_button(harness, label).click();
    harness.run();
}

/// The text in the open cell editor labelled `column`, or `None` when no such editor
/// is open.
fn editor(harness: &Harness<'static, App>, column: &str) -> Option<String> {
    harness
        .query_by_role_and_label(Role::TextInput, column)
        .map(|node| node.accesskit_node().value().unwrap_or_default())
}

/// The label of the cell under the cell cursor.
fn cursor(harness: &Harness<'static, App>) -> String {
    let mut cursors = harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| cell.accesskit_node().is_selected() == Some(true));
    let cursor = cursors.next().expect("a cell holds the cell cursor");
    assert!(cursors.next().is_none(), "one cell holds the cell cursor");
    cursor.accesskit_node().label().unwrap_or_default()
}

/// Whether the Register shows a cell labelled `label`.
fn shows(harness: &Harness<'static, App>, label: &str) -> bool {
    harness.query_by_role_and_label(Role::Cell, label).is_some()
}

/// Whether the leave-row prompt is open.
fn prompted(harness: &Harness<'static, App>) -> bool {
    harness
        .query_by_role_and_label(Role::Button, "Continue editing")
        .is_some()
}

/// Whether the Edit menu's Undo item is enabled, opening the menu to look and closing
/// it again.
fn can_undo(harness: &mut Harness<'static, App>) -> bool {
    click_button(harness, "Edit");
    let enabled = !harness
        .get_by_role_and_label(Role::Button, "Undo Ctrl+Z")
        .accesskit_node()
        .is_disabled();
    click_button(harness, "Edit");
    enabled
}

/// Asks to close the window, as its title bar's × does, for one frame.
fn close_window(harness: &mut Harness<'static, App>) {
    root_events(harness).push(ViewportEvent::Close);
    harness.step();
    // `egui_kittest` keeps a viewport's events from frame to frame; eframe doesn't.
    root_events(harness).clear();
    harness.run();
}

/// The events of the root viewport the next frame's input holds.
fn root_events<'a>(harness: &'a mut Harness<'static, App>) -> &'a mut Vec<ViewportEvent> {
    &mut harness
        .input_mut()
        .viewports
        .entry(ViewportId::ROOT)
        .or_default()
        .events
}

/// Fills the draft row of the account just opened, from the Date cell under the
/// cursor: Corner Shop, paid `payment` for groceries on 15 October 2026.
fn fill_draft(harness: &mut Harness<'static, App>, payment: &str) {
    for c in "10/15/2026".chars() {
        send(harness, Event::Text(c.to_string()));
        harness.run();
    }
    // Date, Num, Payee.
    press(harness, Modifiers::NONE, Key::Tab);
    press(harness, Modifiers::NONE, Key::Tab);
    type_text(harness, "Corner Shop");
    press(harness, Modifiers::NONE, Key::Tab);
    type_text(harness, "Food & Dining:Groceries");
    // Category, Memo, Payment.
    press(harness, Modifiers::NONE, Key::Tab);
    press(harness, Modifiers::NONE, Key::Tab);
    type_text(harness, payment);
}

/// Renames Fresh Market to Corner Shop in its Payee cell, leaving the edit open.
fn rename_fresh_market(harness: &mut Harness<'static, App>) {
    open(harness, "Everyday Checking");
    click_cell(harness, "Fresh Market");
    type_text(harness, "Corner Shop");
}

/// The Dialog's button labelled `label`, not a button of the same name in the editing
/// row's toolbar behind it.
fn dialog_button<'tree>(
    harness: &'tree Harness<'static, App>,
    label: &'tree str,
) -> egui_kittest::Node<'tree> {
    harness
        .query_all_by_role_and_label(Role::Button, label)
        .find(|button| {
            button
                .accesskit_node()
                .parent()
                .is_none_or(|parent| parent.role() != Role::Toolbar)
        })
        .expect("the Dialog has the button")
}

#[test]
fn clicking_another_row_of_a_changed_row_asks_first() {
    let mut harness = app();
    rename_fresh_market(&mut harness);

    click_cell(&mut harness, "Bean There");

    assert!(prompted(&harness));
}

#[test]
fn save_in_the_prompt_saves_the_row_and_moves_on() {
    let mut harness = app();
    rename_fresh_market(&mut harness);
    click_cell(&mut harness, "Bean There");

    click_button(&mut harness, "Save");

    assert!(!prompted(&harness));
    assert!(shows(&harness, "Corner Shop"));
    assert_eq!(cursor(&harness), "Bean There");
    assert!(can_undo(&mut harness));
}

#[test]
fn continue_editing_stays_on_the_row_with_its_cell_open() {
    let mut harness = app();
    rename_fresh_market(&mut harness);
    click_cell(&mut harness, "Bean There");

    click_button(&mut harness, "Continue editing");

    assert!(!prompted(&harness));
    assert_eq!(editor(&harness, "Payee").as_deref(), Some("Corner Shop"));
    assert!(!can_undo(&mut harness));
}

#[test]
fn discard_throws_the_changes_away_and_moves_on() {
    let mut harness = app();
    rename_fresh_market(&mut harness);
    click_cell(&mut harness, "Bean There");

    click_button(&mut harness, "Discard");

    assert!(!prompted(&harness));
    assert!(shows(&harness, "Fresh Market"));
    assert!(!shows(&harness, "Corner Shop"));
    assert_eq!(cursor(&harness), "Bean There");
    assert!(!can_undo(&mut harness));
}

#[test]
fn enter_in_the_prompt_saves() {
    let mut harness = app();
    rename_fresh_market(&mut harness);
    click_cell(&mut harness, "Bean There");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(!prompted(&harness));
    assert_eq!(cursor(&harness), "Bean There");
    assert!(can_undo(&mut harness));
}

#[test]
fn escape_in_the_prompt_continues_editing() {
    let mut harness = app();
    rename_fresh_market(&mut harness);
    click_cell(&mut harness, "Bean There");

    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert!(!prompted(&harness));
    assert_eq!(editor(&harness, "Payee").as_deref(), Some("Corner Shop"));
}

#[test]
fn no_key_discards() {
    let mut harness = app();
    rename_fresh_market(&mut harness);
    click_cell(&mut harness, "Bean There");

    for key in [Key::D, Key::Delete, Key::Backspace] {
        press(&mut harness, Modifiers::NONE, key);
    }

    assert!(prompted(&harness));
}

#[test]
fn opening_another_account_asks_first() {
    let mut harness = app();
    rename_fresh_market(&mut harness);

    open(&mut harness, "Visa Rewards");

    assert!(prompted(&harness));
    assert!(shows(&harness, "Corner Shop"));
}

#[test]
fn save_as_another_account_opens_saves_the_row_and_opens_it() {
    let mut harness = app();
    rename_fresh_market(&mut harness);
    open(&mut harness, "Visa Rewards");

    click_button(&mut harness, "Save");

    assert!(!shows(&harness, "Corner Shop"));
    assert!(!shows(&harness, "Fresh Market"));
    assert!(can_undo(&mut harness));
}

#[test]
fn discard_as_another_account_opens_opens_it_without_the_changes() {
    let mut harness = app();
    rename_fresh_market(&mut harness);
    open(&mut harness, "Visa Rewards");

    click_button(&mut harness, "Discard");

    assert!(!shows(&harness, "Corner Shop"));
    assert!(!shows(&harness, "Fresh Market"));
    assert!(!can_undo(&mut harness));
}

#[test]
fn another_tab_asks_first() {
    let mut harness = app();
    rename_fresh_market(&mut harness);

    click_button(&mut harness, "Home");

    assert!(prompted(&harness));
}

#[test]
fn closing_the_window_asks_first() {
    let mut harness = app();
    rename_fresh_market(&mut harness);

    close_window(&mut harness);

    assert!(prompted(&harness));
}

#[test]
fn save_as_the_window_closes_saves_the_row() {
    let mut harness = app();
    rename_fresh_market(&mut harness);
    close_window(&mut harness);

    dialog_button(&harness, "Save").click();
    // The save, then the close it asks for: more frames than `run` allows.
    harness.run_steps(8);

    assert!(can_undo(&mut harness));
}

#[test]
fn closing_the_window_with_nothing_changed_asks_nothing() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");

    close_window(&mut harness);

    assert!(!prompted(&harness));
}

/// Ticks "Always save without asking" in the open prompt.
fn tick_always_save(harness: &mut Harness<'static, App>) {
    harness
        .get_by_role_and_label(Role::CheckBox, "Always save without asking")
        .click();
    harness.run();
}

#[test]
fn saving_with_always_save_ticked_turns_the_preference_on() {
    let mut harness = app();
    rename_fresh_market(&mut harness);
    click_cell(&mut harness, "Bean There");
    tick_always_save(&mut harness);

    click_button(&mut harness, "Save");

    click_button(&mut harness, "Tools");
    click_button(&mut harness, "Preferences…");
    harness.get_by_role_and_label(Role::Tab, "Register").click();
    harness.run();
    let preference = harness
        .get_by_role_and_label(Role::CheckBox, "Save a changed transaction without asking")
        .accesskit_node()
        .toggled();
    assert_eq!(preference, Some(Toggled::True));
}

#[test]
fn always_save_ticked_over_the_preferences_dialog_ticks_its_preference_too() {
    let mut harness = app();
    rename_fresh_market(&mut harness);
    click_button(&mut harness, "Tools");
    click_button(&mut harness, "Preferences…");
    harness.get_by_role_and_label(Role::Tab, "Register").click();
    harness.run();
    close_window(&mut harness);
    tick_always_save(&mut harness);

    dialog_button(&harness, "Save").click();
    harness.run_steps(8);

    let preference = harness
        .get_by_role_and_label(Role::CheckBox, "Save a changed transaction without asking")
        .accesskit_node()
        .toggled();
    assert_eq!(preference, Some(Toggled::True));
}

#[test]
fn after_always_save_the_next_leave_saves_with_no_prompt() {
    let mut harness = app();
    rename_fresh_market(&mut harness);
    click_cell(&mut harness, "Bean There");
    tick_always_save(&mut harness);
    click_button(&mut harness, "Save");
    type_text(&mut harness, "Cafe");

    click_cell(&mut harness, "Corner Shop");

    assert!(!prompted(&harness));
    assert!(shows(&harness, "Cafe"));
    assert_eq!(cursor(&harness), "Corner Shop");
}

#[test]
fn leaving_a_filled_draft_row_asks_add() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    fill_draft(&mut harness, "5.00");

    click_cell(&mut harness, "Fresh Market");

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Add")
            .is_some()
    );
}

#[test]
fn add_in_the_prompt_adds_the_draft_row_and_moves_on() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    fill_draft(&mut harness, "5.00");
    click_cell(&mut harness, "Fresh Market");

    click_button(&mut harness, "Add");

    assert!(shows(&harness, "Corner Shop"));
    assert_eq!(cursor(&harness), "Fresh Market");
    assert!(can_undo(&mut harness));
}

#[test]
fn a_draft_row_the_ledger_refuses_to_add_brings_the_cursor_back_to_it() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    // Too large for the account's balance to hold.
    fill_draft(&mut harness, "792281625142643375935439503.35");
    click_cell(&mut harness, "Fresh Market");

    harness.get_by_role_and_label(Role::Button, "Add").click();
    // The prompt's answer, the post, the ledger's refusal, then the Dialog saying so.
    harness.run_steps(8);
    click_button(&mut harness, "Continue editing");

    assert_eq!(
        editor(&harness, "Payment").as_deref(),
        Some("792,281,625,142,643,375,935,439,503.35")
    );
}

#[test]
fn discarding_a_filled_draft_row_empties_it() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    fill_draft(&mut harness, "5.00");
    click_cell(&mut harness, "Fresh Market");

    click_button(&mut harness, "Discard");

    assert!(!shows(&harness, "Corner Shop"));
    assert_eq!(cursor(&harness), "Fresh Market");
    assert!(!can_undo(&mut harness));
}

#[test]
fn an_empty_draft_row_never_asks() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    click_cell(&mut harness, "Fresh Market");

    assert!(!prompted(&harness));
}

#[test]
fn an_unchanged_row_never_asks() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    click_cell(&mut harness, "Bean There");

    assert!(!prompted(&harness));
}

#[test]
fn a_row_changed_back_to_its_saved_values_never_asks() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    // Fresh Market's Payment.
    click_cell(&mut harness, "86.42");
    type_text(&mut harness, "1");
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::SHIFT, Key::Tab);
    for _ in "1.00".chars() {
        press(&mut harness, Modifiers::NONE, Key::Backspace);
    }
    type_text(&mut harness, "86.42");

    click_cell(&mut harness, "Bean There");

    assert_eq!(cursor(&harness), "Bean There");
    assert!(!prompted(&harness));
}

/// Renames Fresh Market to Corner Shop and saves it, closing the cell the save opens.
fn save_a_rename(harness: &mut Harness<'static, App>) {
    rename_fresh_market(harness);
    press(harness, Modifiers::NONE, Key::Enter);
    press(harness, Modifiers::NONE, Key::Escape);
}

/// Renames Bean There to Cafe and closes the cell, leaving the row changed with no
/// text field focused.
fn change_bean_there(harness: &mut Harness<'static, App>) {
    click_cell(harness, "Bean There");
    type_text(harness, "Cafe");
    // A click on the row's Date cell closes the edit and opens nothing.
    click_cell(harness, "9/6/2026");
}

#[test]
fn ctrl_z_outside_a_text_field_asks_before_undoing() {
    let mut harness = app();
    save_a_rename(&mut harness);
    change_bean_there(&mut harness);

    press(&mut harness, Modifiers::COMMAND, Key::Z);

    assert!(prompted(&harness));
    assert!(shows(&harness, "Corner Shop"));
}

#[test]
fn discard_before_an_undo_throws_the_changes_away_then_undoes() {
    let mut harness = app();
    save_a_rename(&mut harness);
    change_bean_there(&mut harness);
    press(&mut harness, Modifiers::COMMAND, Key::Z);

    click_button(&mut harness, "Discard");

    assert!(shows(&harness, "Fresh Market"));
    assert!(shows(&harness, "Bean There"));
}

#[test]
fn ctrl_y_outside_a_text_field_asks_before_redoing() {
    let mut harness = app();
    save_a_rename(&mut harness);
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    change_bean_there(&mut harness);

    press(&mut harness, Modifiers::COMMAND, Key::Y);

    assert!(prompted(&harness));
    assert!(shows(&harness, "Fresh Market"));
}

#[test]
fn discard_before_a_redo_throws_the_changes_away_then_redoes() {
    let mut harness = app();
    save_a_rename(&mut harness);
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    change_bean_there(&mut harness);
    press(&mut harness, Modifiers::COMMAND, Key::Y);

    click_button(&mut harness, "Discard");

    assert!(shows(&harness, "Corner Shop"));
    assert!(shows(&harness, "Bean There"));
}

#[test]
fn edit_undo_with_a_changed_row_asks_first() {
    let mut harness = app();
    save_a_rename(&mut harness);
    change_bean_there(&mut harness);

    click_button(&mut harness, "Edit");
    click_button(&mut harness, "Undo Ctrl+Z");

    assert!(prompted(&harness));
}

#[test]
fn save_before_an_undo_saves_then_undoes_that_save() {
    let mut harness = app();
    save_a_rename(&mut harness);
    change_bean_there(&mut harness);
    click_button(&mut harness, "Edit");
    click_button(&mut harness, "Undo Ctrl+Z");

    click_button(&mut harness, "Save");

    // The undo takes back the save just made, and nothing before it.
    assert!(shows(&harness, "Bean There"));
    assert!(shows(&harness, "Corner Shop"));
}

#[test]
fn ctrl_z_in_a_text_field_of_a_changed_row_undoes_only_typing() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    type_text(&mut harness, " Downtown");
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::SHIFT, Key::Tab);
    type_text(&mut harness, "!");

    press(&mut harness, Modifiers::COMMAND, Key::Z);

    assert!(!prompted(&harness));
    assert_eq!(
        editor(&harness, "Payee").as_deref(),
        Some("Fresh Market Downtown")
    );
}

#[test]
fn continue_editing_after_opening_another_account_reopens_the_cell() {
    let mut harness = app();
    rename_fresh_market(&mut harness);
    open(&mut harness, "Visa Rewards");

    click_button(&mut harness, "Continue editing");
    type_text(&mut harness, " Downtown");

    assert_eq!(
        editor(&harness, "Payee").as_deref(),
        Some("Corner Shop Downtown")
    );
}

#[test]
fn deleting_a_changed_row_throws_its_changes_away_with_no_prompt() {
    let mut harness = app();
    rename_fresh_market(&mut harness);

    press(&mut harness, Modifiers::COMMAND, Key::D);
    click_button(&mut harness, "Delete");
    click_cell(&mut harness, "Bean There");

    assert!(!prompted(&harness));
    assert!(!shows(&harness, "Corner Shop"));
    assert!(!shows(&harness, "Fresh Market"));
}
