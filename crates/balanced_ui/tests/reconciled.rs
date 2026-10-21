// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Changing and deleting Reconciled transactions, which asks first, driven through the
//! AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, Rangef, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The confirm for changing the September rent, Reconciled, to 1,500.00.
const RENT_CONFIRM: &str = "This transaction is reconciled. Change its amount to -1,500.00? \
                            Maple Property Management, 9/3/2026, -1,450.00";

/// The App on stub data, in the test locale, wide enough to show every Register column,
/// with Everyday Checking open.
fn everyday_checking() -> Harness<'static, App> {
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

/// The x range of the Register heading `title`.
fn heading(harness: &Harness<'static, App>, title: &str) -> Rangef {
    harness
        .get_by_role_and_label(Role::ColumnHeader, title)
        .rect()
        .x_range()
}

/// The labels of the Register cells under the heading `title`, top row first.
fn column(harness: &Harness<'static, App>, title: &str) -> Vec<String> {
    let header = heading(harness, title);
    harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| header.contains(cell.rect().center().x))
        .map(|cell| cell.accesskit_node().label().unwrap_or_default())
        .collect()
}

/// The row of the first transaction whose Payee is `payee`, top row first.
fn row_of(harness: &Harness<'static, App>, payee: &str) -> usize {
    column(harness, "Payee")
        .iter()
        .position(|cell| cell == payee)
        .expect("the Register lists the payee")
}

/// The cell under the heading `title` of the first row whose Payee is `payee`.
fn cell_of(harness: &Harness<'static, App>, title: &str, payee: &str) -> String {
    column(harness, title).swap_remove(row_of(harness, payee))
}

/// Puts the cell cursor on the cell under the heading `title` of the first row whose
/// Payee is `payee`, by clicking it.
fn click_cell(harness: &mut Harness<'static, App>, title: &str, payee: &str) {
    let row = row_of(harness, payee);
    let header = heading(harness, title);
    harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| header.contains(cell.rect().center().x))
        .nth(row)
        .expect("the row has a cell under the heading")
        .click();
    harness.run();
}

/// Types `text` over the cell under `title` in the first row whose Payee is `payee`,
/// then presses Enter to save the row.
fn commit(harness: &mut Harness<'static, App>, title: &str, payee: &str, text: &str) {
    click_cell(harness, title, payee);
    type_text(harness, text);
    press(harness, Modifiers::NONE, Key::Enter);
}

/// Whether a Dialog shows `question`.
fn asks(harness: &Harness<'static, App>, question: &str) -> bool {
    harness.query_by_label(question).is_some()
}

/// The status the Clr cell labelled `label` reads out.
fn status(harness: &Harness<'static, App>, label: &str) -> String {
    harness
        .get_by_role_and_label(Role::Button, label)
        .accesskit_node()
        .value()
        .expect("a Clr cell reads out its status")
}

/// Whether the cell cursor is on the cell under the heading `title` of the first row
/// whose Payee is `payee`.
fn cursor_on(harness: &Harness<'static, App>, title: &str, payee: &str) -> bool {
    let mut cursors = harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| cell.accesskit_node().is_selected() == Some(true));
    let cursor = cursors.next().expect("a cell holds the cell cursor");
    assert!(cursors.next().is_none(), "one cell holds the cell cursor");
    let payees = heading(harness, "Payee");
    let row = harness
        .query_all_by_role(Role::Cell)
        .filter(|cell| payees.contains(cell.rect().center().x))
        .nth(row_of(harness, payee))
        .expect("the Register lists the payee")
        .rect()
        .y_range();
    let centre = cursor.rect().center();
    heading(harness, title).contains(centre.x) && row.contains(centre.y)
}

/// Whether a cell editor is open: a text field other than the filter bar's search box.
fn editing(harness: &Harness<'static, App>) -> bool {
    harness
        .query_all_by_role(Role::TextInput)
        .any(|node| node.accesskit_node().label().as_deref() != Some("Search"))
}

#[test]
fn committing_a_reconciled_rows_payment_asks_first() {
    let mut harness = everyday_checking();

    commit(&mut harness, "Payment", "Maple Property Management", "1500");

    assert!(asks(&harness, RENT_CONFIRM));
}

#[test]
fn committing_a_reconciled_rows_deposit_asks_first() {
    let mut harness = everyday_checking();

    commit(&mut harness, "Deposit", "Acme Corp", "3300");

    assert!(asks(
        &harness,
        "This transaction is reconciled. Change its amount to 3,300.00? \
         Acme Corp, 9/1/2026, 3,200.00"
    ));
}

#[test]
fn accepting_the_amount_confirm_saves_the_amount() {
    let mut harness = everyday_checking();
    commit(&mut harness, "Payment", "Maple Property Management", "1500");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(!asks(&harness, RENT_CONFIRM));
    assert_eq!(
        cell_of(&harness, "Payment", "Maple Property Management"),
        "1,500.00"
    );
}

#[test]
fn a_saved_amount_leaves_the_transaction_reconciled() {
    let mut harness = everyday_checking();
    commit(&mut harness, "Payment", "Maple Property Management", "1500");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(
        status(
            &harness,
            "Clr Maple Property Management, 9/3/2026, -1,500.00"
        ),
        "Reconciled"
    );
}

#[test]
fn cancel_saves_nothing_and_keeps_the_row_open() {
    let mut harness = everyday_checking();
    commit(&mut harness, "Payment", "Maple Property Management", "1500");

    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click();
    harness.run();
    let open = editing(&harness);
    // Throws the row's changes away, showing what the ledger holds.
    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert!(!asks(&harness, RENT_CONFIRM));
    assert!(open);
    assert_eq!(
        cell_of(&harness, "Payment", "Maple Property Management"),
        "1,450.00"
    );
}

#[test]
fn esc_saves_nothing_and_keeps_the_row_open() {
    let mut harness = everyday_checking();
    commit(&mut harness, "Payment", "Maple Property Management", "1500");

    press(&mut harness, Modifiers::NONE, Key::Escape);
    let open = editing(&harness);
    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert!(!asks(&harness, RENT_CONFIRM));
    assert!(open);
    assert_eq!(
        cell_of(&harness, "Payment", "Maple Property Management"),
        "1,450.00"
    );
}

#[test]
fn an_uncleared_rows_amount_saves_without_asking() {
    let mut harness = everyday_checking();

    commit(&mut harness, "Payment", "Fresh Market", "90");

    assert_eq!(cell_of(&harness, "Payment", "Fresh Market"), "90.00");
}

#[test]
fn a_cleared_rows_amount_saves_without_asking() {
    let mut harness = everyday_checking();

    commit(&mut harness, "Payment", "Bean There", "5");

    assert_eq!(cell_of(&harness, "Payment", "Bean There"), "5.00");
}

#[test]
fn a_reconciled_rows_number_saves_without_asking() {
    let mut harness = everyday_checking();

    commit(&mut harness, "Num", "Maple Property Management", "2041");

    assert_eq!(
        cell_of(&harness, "Num", "Maple Property Management"),
        "2041"
    );
}

#[test]
fn a_reconciled_rows_payee_saves_without_asking() {
    let mut harness = everyday_checking();
    let row = row_of(&harness, "Maple Property Management");

    commit(
        &mut harness,
        "Payee",
        "Maple Property Management",
        "Maple Homes",
    );

    assert_eq!(column(&harness, "Payee")[row], "Maple Homes");
}

#[test]
fn a_reconciled_rows_memo_saves_without_asking() {
    let mut harness = everyday_checking();

    commit(&mut harness, "Memo", "Maple Property Management", "Rent");

    assert_eq!(
        cell_of(&harness, "Memo", "Maple Property Management"),
        "Rent"
    );
}

#[test]
fn a_reconciled_rows_category_saves_without_asking() {
    let mut harness = everyday_checking();

    commit(
        &mut harness,
        "Category",
        "Maple Property Management",
        "Auto & Transport:Fuel",
    );

    assert_eq!(
        cell_of(&harness, "Category", "Maple Property Management"),
        "Auto & Transport:Fuel"
    );
}

#[test]
fn a_reconciled_rows_date_saves_without_asking() {
    let mut harness = everyday_checking();
    click_cell(&mut harness, "Date", "Maple Property Management");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    send(&mut harness, Event::Text("t".to_owned()));
    harness.run();

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(
        cell_of(&harness, "Date", "Maple Property Management"),
        "10/14/2026"
    );
}

#[test]
fn the_delete_confirm_says_a_reconciled_transaction_is_reconciled() {
    let mut harness = everyday_checking();
    click_cell(&mut harness, "Payee", "Maple Property Management");

    press(&mut harness, Modifiers::COMMAND, Key::D);

    assert!(asks(
        &harness,
        "This transaction is reconciled. Delete it anyway? \
         Maple Property Management, 9/3/2026, -1,450.00"
    ));
}

#[test]
fn the_delete_confirm_for_an_uncleared_transaction_asks_as_usual() {
    let mut harness = everyday_checking();
    click_cell(&mut harness, "Payee", "Fresh Market");

    press(&mut harness, Modifiers::COMMAND, Key::D);

    assert!(asks(
        &harness,
        "Delete this transaction? Fresh Market, 9/5/2026, -86.42"
    ));
}

#[test]
fn a_reconciled_amount_asks_on_save_not_on_leaving_its_cell() {
    let mut harness = everyday_checking();
    click_cell(&mut harness, "Payment", "Maple Property Management");
    type_text(&mut harness, "1500");

    press(&mut harness, Modifiers::NONE, Key::Tab);
    let asked_on_tab = asks(&harness, RENT_CONFIRM);
    let moved = cursor_on(&harness, "Deposit", "Maple Property Management");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(!asked_on_tab);
    assert!(moved);
    assert!(asks(&harness, RENT_CONFIRM));
}

#[test]
fn opening_another_account_says_why_a_changed_reconciled_amount_was_not_saved() {
    let mut harness = everyday_checking();
    click_cell(&mut harness, "Payment", "Maple Property Management");
    type_text(&mut harness, "1500");
    press(&mut harness, Modifiers::NONE, Key::Tab);

    harness
        .get_by_role_and_label(Role::Button, "Visa Rewards")
        .click();
    harness.run();

    let shown: Vec<String> = harness
        .get_by_role(Role::Status)
        .query_all_by_role(Role::Label)
        .filter_map(|label| label.accesskit_node().value())
        .collect();
    assert_eq!(
        shown,
        [
            "The changed row wasn't saved: a reconciled transaction's amount changes only once you confirm it."
        ]
    );
}
