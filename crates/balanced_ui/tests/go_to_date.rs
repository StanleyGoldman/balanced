// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Go to date: Ctrl+G and Edit ▸ Go to date open a box that is the date field, and Go
//! moves the Register's cell cursor to the first row on or after its date. Driven
//! through the AccessKit tree on a clock stopped on Wednesday 14 October 2026.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::{Role, Toggled};
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on stub data, on 14 October 2026, with the account named `account` open.
fn app_with(account: &str) -> Harness<'static, App> {
    let today = Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date");
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(move |cc| {
            App::on_ledger(
                cc,
                Localizer::for_tests(),
                Ledger::stub(),
                Clock::fixed(today),
                PreferencesFile::for_tests(),
            )
        });
    harness.run();
    harness.get_by_role_and_label(Role::Button, account).click();
    harness.run();
    harness
}

/// The App with Everyday Checking open.
fn everyday() -> Harness<'static, App> {
    app_with("Everyday Checking")
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

/// Selects the row whose payee is `payee` by clicking its Payee cell.
fn select(harness: &mut Harness<'static, App>, payee: &str) {
    harness.get_by_role_and_label(Role::Cell, payee).click();
    harness.run();
}

/// Types `text` in one go into whatever has the keyboard.
fn type_text(harness: &mut Harness<'static, App>, text: &str) {
    send(harness, Event::Text(text.to_owned()));
    harness.run();
}

/// Clicks the button labelled `label`.
fn click(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run();
}

/// The Date cell of the row holding the cell cursor, while that row is in view.
fn selected_date(harness: &Harness<'static, App>) -> Option<String> {
    let date_x = harness
        .get_by_role_and_label(Role::ColumnHeader, "Date")
        .rect()
        .x_range();
    harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| date_x.contains(cell.rect().center().x))
        .find(|cell| cell.accesskit_node().toggled() == Some(Toggled::True))
        .map(|cell| cell.accesskit_node().label().unwrap_or_default())
}

/// The text of Go to date's box, while it is open.
fn go_to_box(harness: &Harness<'static, App>) -> Option<String> {
    harness
        .query_by_role_and_label(Role::TextInput, "Date")
        .map(|field| field.accesskit_node().value().unwrap_or_default())
}

#[test]
fn ctrl_g_opens_go_to_date_on_the_selected_row_s_date() {
    let mut harness = everyday();
    select(&mut harness, "Fresh Market");

    press(&mut harness, Modifiers::COMMAND, Key::G);

    assert_eq!(go_to_box(&harness).as_deref(), Some("9/5/2026"));
}

#[test]
fn ctrl_g_with_no_row_selected_opens_go_to_date_on_today() {
    // The account opens with the cell cursor on the draft row, which is no
    // transaction's.
    let mut harness = everyday();

    press(&mut harness, Modifiers::COMMAND, Key::G);

    assert_eq!(go_to_box(&harness).as_deref(), Some("10/14/2026"));
}

#[test]
fn go_moves_the_cell_cursor_to_the_first_row_on_or_after_the_date() {
    let mut harness = everyday();
    select(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::COMMAND, Key::G);

    // Nothing is dated 11 September; Costco is on the 12th.
    type_text(&mut harness, "9/11/2026");
    click(&mut harness, "Go");

    assert_eq!(selected_date(&harness).as_deref(), Some("9/12/2026"));
}

#[test]
fn enter_in_the_box_goes_to_the_date() {
    let mut harness = everyday();
    select(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::COMMAND, Key::G);

    type_text(&mut harness, "9/18/2026");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(selected_date(&harness).as_deref(), Some("9/18/2026"));
}

#[test]
fn a_date_after_every_row_goes_to_the_last_row() {
    let mut harness = everyday();
    select(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::COMMAND, Key::G);

    type_text(&mut harness, "1/1/2030");
    click(&mut harness, "Go");

    // November's rent is the account's last transaction.
    assert_eq!(selected_date(&harness).as_deref(), Some("11/1/2026"));
}

#[test]
fn go_scrolls_a_far_off_row_into_view() {
    // Household Checking opens on its last rows, years after 2015.
    let mut harness = app_with("Household Checking");
    press(&mut harness, Modifiers::COMMAND, Key::G);

    type_text(&mut harness, "3/1/2015");
    click(&mut harness, "Go");

    // The Register lays out only the rows in view.
    assert_eq!(selected_date(&harness).as_deref(), Some("3/1/2015"));
}

#[test]
fn esc_closes_go_to_date_with_the_cell_cursor_unmoved() {
    let mut harness = everyday();
    select(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::COMMAND, Key::G);
    type_text(&mut harness, "9/18/2026");

    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert_eq!(go_to_box(&harness), None);
    assert_eq!(selected_date(&harness).as_deref(), Some("9/5/2026"));
}

#[test]
fn cancel_closes_go_to_date_with_the_cell_cursor_unmoved() {
    let mut harness = everyday();
    select(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::COMMAND, Key::G);
    type_text(&mut harness, "9/18/2026");

    click(&mut harness, "Cancel");

    assert_eq!(go_to_box(&harness), None);
    assert_eq!(selected_date(&harness).as_deref(), Some("9/5/2026"));
}

#[test]
fn the_box_takes_the_date_field_s_keys() {
    let mut harness = everyday();
    select(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::COMMAND, Key::G);

    // `h` moves the date to the last day of its month.
    type_text(&mut harness, "h");

    assert_eq!(go_to_box(&harness).as_deref(), Some("9/30/2026"));
}

#[test]
fn the_box_s_calendar_button_opens_its_calendar() {
    let mut harness = everyday();
    select(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::COMMAND, Key::G);

    click(&mut harness, "Calendar");

    // The calendar shows the box's month, with its days as buttons.
    let shown = harness
        .query_by_role_and_label(Role::Button, "9/24/2026")
        .is_some();
    assert!(shown);
}

#[test]
fn go_to_date_in_the_edit_menu_opens_the_box() {
    let mut harness = everyday();
    select(&mut harness, "Fresh Market");

    click(&mut harness, "Edit");
    click(&mut harness, "Go to Date… Ctrl+G");

    assert_eq!(go_to_box(&harness).as_deref(), Some("9/5/2026"));
}
