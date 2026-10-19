//! The draft row at the bottom of each Register, which adds transactions.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on stub data, in the test locale, wide enough to show every Register column.
fn app() -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(|cc| {
            App::new(
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

/// Types `text` one character at a time, as a keyboard does.
fn type_keys(harness: &mut Harness<'static, App>, text: &str) {
    for c in text.chars() {
        send(harness, Event::Text(c.to_string()));
        harness.run();
    }
}

/// Types `text` in one go into whatever has the keyboard.
fn type_text(harness: &mut Harness<'static, App>, text: &str) {
    send(harness, Event::Text(text.to_owned()));
    harness.run();
}

/// The labels of every Register cell, top row first, each row left to right.
fn cells(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::Cell)
        .map(|cell| cell.accesskit_node().label().unwrap_or_default())
        .collect()
}

/// The cell under the cell cursor: its label and the top of its row.
fn cursor(harness: &Harness<'static, App>) -> (String, f32) {
    let mut cursors = harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| cell.accesskit_node().is_selected() == Some(true));
    let cursor = cursors.next().expect("a cell holds the cell cursor");
    assert!(cursors.next().is_none(), "one cell holds the cell cursor");
    (
        cursor.accesskit_node().label().unwrap_or_default(),
        cursor.rect().top(),
    )
}

/// The top of the row holding the cell labelled `label`.
fn row_top(harness: &Harness<'static, App>, label: &str) -> f32 {
    harness
        .get_by_role_and_label(Role::Cell, label)
        .rect()
        .top()
}

/// The cells of the row whose payee is `payee`, from Date to Balance.
fn row_of(harness: &Harness<'static, App>, payee: &str) -> Vec<String> {
    let cells = cells(harness);
    let payee = cells
        .iter()
        .position(|cell| cell == payee)
        .expect("the Register lists the payee");
    cells[payee - 2..payee + 6].to_vec()
}

/// Fills the draft row, from the Date cell under the cursor, Tabbing from cell to
/// cell, with `payment` in the Payment cell. An empty `date` leaves the Date cell alone.
fn fill_draft(harness: &mut Harness<'static, App>, date: &str, payee: &str, payment: &str) {
    if !date.is_empty() {
        type_keys(harness, date);
    }
    // Date, Num, Payee.
    press(harness, Modifiers::NONE, Key::Tab);
    press(harness, Modifiers::NONE, Key::Tab);
    type_text(harness, payee);
    press(harness, Modifiers::NONE, Key::Tab);
    type_text(harness, "Food & Dining:Groceries");
    // Category, Memo, Payment.
    press(harness, Modifiers::NONE, Key::Tab);
    press(harness, Modifiers::NONE, Key::Tab);
    type_text(harness, payment);
}

#[test]
fn ctrl_enter_adds_the_draft_row_with_its_running_balance() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    fill_draft(&mut harness, "10/15/2026", "Corner Cafe", "12");

    press(&mut harness, Modifiers::COMMAND, Key::Enter);

    assert_eq!(
        row_of(&harness, "Corner Cafe"),
        [
            "10/15/2026",
            "",
            "Corner Cafe",
            "Food & Dining:Groceries",
            "",
            "12.00",
            "",
            "5,635.46",
        ]
    );
}

#[test]
fn an_amount_in_the_draft_rows_deposit_adds_money_in() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    fill_draft(&mut harness, "10/15/2026", "Corner Cafe", "");
    press(&mut harness, Modifiers::NONE, Key::Tab);
    type_text(&mut harness, "12");

    press(&mut harness, Modifiers::COMMAND, Key::Enter);

    assert_eq!(
        row_of(&harness, "Corner Cafe"),
        [
            "10/15/2026",
            "",
            "Corner Cafe",
            "Food & Dining:Groceries",
            "",
            "",
            "12.00",
            "5,659.46",
        ]
    );
}

#[test]
fn enter_on_the_amount_adds_the_draft_row_and_leaves_the_cursor_on_an_empty_one() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    fill_draft(&mut harness, "10/15/2026", "Corner Cafe", "12");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    let (label, top) = cursor(&harness);
    assert_eq!(label, "");
    assert!(top > row_top(&harness, "Corner Cafe"));
}

#[test]
fn a_row_with_no_date_is_refused_with_the_cursor_on_its_date() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    fill_draft(&mut harness, "", "Corner Cafe", "12");

    press(&mut harness, Modifiers::COMMAND, Key::Enter);

    harness.get_by_label("Date is missing");
    let date = harness
        .get_by_role_and_label(Role::ColumnHeader, "Date")
        .rect()
        .x_range();
    let (_, top) = cursor(&harness);
    let cursor_cell = harness
        .query_all_by_role(Role::Cell)
        .find(|cell| cell.accesskit_node().is_selected() == Some(true))
        .expect("a cell holds the cell cursor");
    assert!(date.contains(cursor_cell.rect().center().x));
    assert_eq!(top, row_top(&harness, "Corner Cafe"));
}

#[test]
fn enter_on_the_amount_of_a_row_with_no_date_refuses_it_too() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    fill_draft(&mut harness, "", "Corner Cafe", "12");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    harness.get_by_label("Date is missing");
}

#[test]
fn ctrl_z_takes_a_posted_transaction_back_out() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    let before = cells(&harness);
    fill_draft(&mut harness, "10/15/2026", "Corner Cafe", "12");
    press(&mut harness, Modifiers::COMMAND, Key::Enter);
    harness.get_by_role_and_label(Role::Cell, "Corner Cafe");

    press(&mut harness, Modifiers::COMMAND, Key::Z);

    assert_eq!(cells(&harness), before);
}

#[test]
fn opening_an_account_puts_the_cursor_on_its_draft_row() {
    let mut harness = app();

    open(&mut harness, "Everyday Checking");

    let (label, top) = cursor(&harness);
    assert_eq!(label, "");
    assert!(top > row_top(&harness, "Trattoria Roma"));
}

#[test]
fn opening_a_large_account_scrolls_to_its_draft_row() {
    let mut harness = app();

    open(&mut harness, "Household Checking");

    let (label, _) = cursor(&harness);
    assert_eq!(label, "");
}

#[test]
fn ctrl_n_puts_the_cursor_on_the_draft_row() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    harness.get_by_role_and_label(Role::Cell, "Costco").click();
    harness.run();

    press(&mut harness, Modifiers::COMMAND, Key::N);

    let (label, top) = cursor(&harness);
    assert_eq!(label, "");
    assert!(top > row_top(&harness, "Trattoria Roma"));
}

#[test]
fn a_closed_account_has_no_draft_row() {
    let mut harness = app();
    open(&mut harness, "Closed");

    open(&mut harness, "Old Checking");

    // The opening balance and two transactions, of nine cells each.
    assert_eq!(cells(&harness).len(), 3 * 9);
}

#[test]
fn enter_on_the_last_transactions_last_cell_goes_to_the_draft_row() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    // Trattoria Roma's Deposit, the last transaction's last cell.
    harness.get_by_role_and_label(Role::Cell, "64.80").click();
    harness.run();
    press(&mut harness, Modifiers::NONE, Key::ArrowRight);

    press(&mut harness, Modifiers::NONE, Key::Enter);
    press(&mut harness, Modifiers::NONE, Key::Enter);

    let (label, top) = cursor(&harness);
    assert_eq!(label, "");
    assert!(top > row_top(&harness, "Trattoria Roma"));
}

#[test]
fn new_transaction_puts_the_cursor_on_the_draft_row() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    harness.get_by_role_and_label(Role::Cell, "Costco").click();
    harness.run();

    harness
        .get_by_role_and_label(Role::Button, "+ Transaction")
        .click();
    harness.run();
    type_keys(&mut harness, "1");

    let date = harness.get_by_role_and_label(Role::TextInput, "Date");
    assert!(date.rect().top() > row_top(&harness, "Trattoria Roma"));
}
