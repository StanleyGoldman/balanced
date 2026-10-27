// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Changing and deleting Reconciled transactions, which asks first, driven through the
//! AccessKit tree the way a user drives it.

use balanced_core::{CurrencyCode, DomainCommand, Ledger, Money, TransactionEdit, TransactionId};
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile, testing};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{
    Event, Key, Modifiers, Rangef, ViewportCommand, ViewportEvent, ViewportId, vec2,
};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The confirm for changing the September rent, Reconciled, to 1,500.00.
const RENT_CONFIRM: &str = "This transaction is reconciled. Change its amount to -1,500.00? \
                            Maple Property Management, 9/3/2026, -1,450.00";

/// The App on stub data, in the test locale, wide enough to show every Register column,
/// with Everyday Checking open.
fn everyday_checking() -> Harness<'static, App> {
    everyday_checking_on(Ledger::stub())
}

/// The App on `ledger`, in the test locale, wide enough to show every Register column,
/// with Everyday Checking open.
fn everyday_checking_on(ledger: Ledger) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(|cc| {
            App::on_ledger(
                cc,
                Localizer::for_tests(),
                ledger,
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
fn a_negative_in_a_reconciled_rows_deposit_asks_first_to_make_it_a_payment() {
    let mut harness = everyday_checking();

    commit(&mut harness, "Deposit", "Acme Corp", "-12");

    assert!(asks(
        &harness,
        "This transaction is reconciled. Change its amount to -12.00? \
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

    dialog_button(&harness, "Cancel").click();
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

/// Changes the rent's Payment to 1500, keeps it in the row with Tab, then leaves the
/// row as `leave` does, and answers the leave-row prompt with Save.
fn change_the_rent_then_leave_and_save(
    harness: &mut Harness<'static, App>,
    leave: impl FnOnce(&mut Harness<'static, App>),
) {
    click_cell(harness, "Payment", "Maple Property Management");
    type_text(harness, "1500");
    press(harness, Modifiers::NONE, Key::Tab);
    leave(harness);
    dialog_button(harness, "Save").click();
    harness.run();
}

/// Opens Visa Rewards from the Account bar.
fn open_visa_rewards(harness: &mut Harness<'static, App>) {
    harness
        .get_by_role_and_label(Role::Button, "Visa Rewards")
        .click();
    harness.run();
}

#[test]
fn saving_a_changed_reconciled_amount_as_another_account_opens_asks_to_confirm_it() {
    let mut harness = everyday_checking();

    change_the_rent_then_leave_and_save(&mut harness, open_visa_rewards);

    assert!(asks(&harness, RENT_CONFIRM));
}

#[test]
fn confirming_the_amount_as_another_account_opens_saves_it_and_opens_the_account() {
    let mut harness = everyday_checking();
    change_the_rent_then_leave_and_save(&mut harness, open_visa_rewards);

    harness
        .get_by_role_and_label(Role::Button, "Change Amount")
        .click();
    harness.run();

    // Visa Rewards is open, and the save can be undone.
    assert!(
        !column(&harness, "Payee")
            .iter()
            .any(|payee| payee == "Maple Property Management")
    );
    harness.get_by_role_and_label(Role::Button, "Edit").click();
    harness.run();
    let undo = harness
        .get_by_role_and_label(Role::Button, "Undo Ctrl+Z")
        .accesskit_node()
        .is_disabled();
    assert!(!undo);
}

#[test]
fn cancelling_the_confirm_as_another_row_is_clicked_stays_on_the_row_and_asks_no_more() {
    let mut harness = everyday_checking();
    change_the_rent_then_leave_and_save(&mut harness, |harness| {
        click_cell(harness, "Payee", "Fresh Market");
    });

    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click();
    harness.run();

    assert!(cursor_on(&harness, "Payment", "Maple Property Management"));
    assert!(editing(&harness));
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Continue editing")
            .is_none()
    );
}

#[test]
fn an_accepted_amount_the_ledger_refuses_says_why() {
    let mut harness = everyday_checking();
    // Too large for the account's balance to hold.
    commit(
        &mut harness,
        "Payment",
        "Maple Property Management",
        "792281625142643375935439503.35",
    );

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(asks(&harness, "Can't save this transaction"));
    assert!(asks(
        &harness,
        "That change would make a balance too large to hold."
    ));
}

/// The id of the transaction paid to `payee` in Everyday Checking in `ledger`.
fn transaction(ledger: &Ledger, payee: &str) -> TransactionId {
    ledger
        .snapshot()
        .accounts()
        .iter()
        .find(|summary| summary.account().name() == "Everyday Checking")
        .expect("the ledger has Everyday Checking")
        .register()
        .iter()
        .find(|entry| entry.transaction().payee() == payee)
        .expect("Everyday Checking has a transaction paid to the payee")
        .transaction()
        .id()
}

/// `dollars` US dollars paid out.
fn payment(dollars: &str) -> Money {
    let usd = CurrencyCode::new("USD").expect("USD is an ISO 4217 currency");
    let amount = format!("-{dollars}").parse().expect("a decimal");
    Money::new(amount, usd).expect("an amount")
}

#[test]
fn cancelling_a_silent_saves_confirm_leaves_the_waiting_save_waiting() {
    let ledger = Ledger::stub();
    let acme = transaction(&ledger, "Acme Corp");
    let mut harness = everyday_checking_on(ledger);
    click_cell(&mut harness, "Payment", "Maple Property Management");
    type_text(&mut harness, "1500");
    // No path reaches this: a silent save's confirm opening as the rent's save asks for
    // its own, which is refused while a Dialog is open, so the rent's save waits on a
    // confirm that never opens.
    let amount = payment("10.00");
    testing::ask_to_confirm_silent_save(
        &harness,
        acme,
        amount,
        DomainCommand::Compound(vec![DomainCommand::EditTransaction {
            transaction: acme,
            edit: TransactionEdit::Amount(amount),
        }]),
    );
    press(&mut harness, Modifiers::NONE, Key::Enter);
    assert!(
        !asks(&harness, RENT_CONFIRM) && asks(&harness, "Change Amount"),
        "the silent save's confirm is open, not the rent's"
    );
    assert!(!editing(&harness), "the rent's save waits");

    dialog_button(&harness, "Cancel").click();
    harness.run();

    // The rent's save still waits on its confirm, so its cell doesn't open again.
    assert!(!editing(&harness));
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

/// Asks to close the window, as its title bar's × does, in the next frame only.
fn request_close(harness: &mut Harness<'static, App>) {
    root_events(harness).push(ViewportEvent::Close);
    harness.step();
    // `egui_kittest` keeps a viewport's events from frame to frame; eframe doesn't.
    root_events(harness).clear();
}

/// Asks to close the window, as its title bar's × does, then lets the App settle.
fn close_window(harness: &mut Harness<'static, App>) {
    request_close(harness);
    harness.run();
}

/// Whether the App closes the window over the next four frames: those a confirm or
/// a save takes to be sent, applied, and to apply the Quit it sends. Each frame's
/// output carries only the commands sent in that frame.
fn closes(harness: &mut Harness<'static, App>) -> bool {
    let mut closes = false;
    for _ in 0..4 {
        harness.step();
        closes |= harness.output().viewport_output[&ViewportId::ROOT]
            .commands
            .contains(&ViewportCommand::Close);
    }
    closes
}

/// Clicks the open Dialog's `label` button, then whether the App closes the window,
/// and lets the App settle.
fn click_and_closes(harness: &mut Harness<'static, App>, label: &str) -> bool {
    dialog_button(harness, label).click();
    let closes = closes(harness);
    harness.run();
    closes
}

/// Whether the Edit menu's Undo item is enabled, opening the menu to look and closing
/// it again.
fn can_undo(harness: &mut Harness<'static, App>) -> bool {
    harness.get_by_role_and_label(Role::Button, "Edit").click();
    harness.run();
    let enabled = !harness
        .get_by_role_and_label(Role::Button, "Undo Ctrl+Z")
        .accesskit_node()
        .is_disabled();
    harness.get_by_role_and_label(Role::Button, "Edit").click();
    harness.run();
    enabled
}

/// Types 1500 over the rent's Payment, leaving the edit open, then closes the window.
fn change_the_rent_then_close(harness: &mut Harness<'static, App>) {
    click_cell(harness, "Payment", "Maple Property Management");
    type_text(harness, "1500");
    close_window(harness);
}

#[test]
fn save_on_closing_with_a_changed_reconciled_amount_asks_to_confirm_it_first() {
    let mut harness = everyday_checking();
    change_the_rent_then_close(&mut harness);

    let closed = click_and_closes(&mut harness, "Save");

    assert!(!closed);
    assert!(asks(&harness, RENT_CONFIRM));
}

#[test]
fn confirming_the_amount_on_closing_saves_it_and_closes_the_window() {
    let mut harness = everyday_checking();
    change_the_rent_then_close(&mut harness);
    dialog_button(&harness, "Save").click();
    harness.run();

    let closed = click_and_closes(&mut harness, "Change Amount");

    assert!(closed);
    assert!(can_undo(&mut harness));
}

#[test]
fn cancelling_the_amount_confirm_on_closing_keeps_the_window_and_the_change() {
    let mut harness = everyday_checking();
    change_the_rent_then_close(&mut harness);
    dialog_button(&harness, "Save").click();
    harness.run();

    let closed = click_and_closes(&mut harness, "Cancel");

    assert!(!closed);
    assert!(cursor_on(&harness, "Payment", "Maple Property Management"));
    assert!(editing(&harness));
    assert!(!can_undo(&mut harness));
}

#[test]
fn discard_on_closing_closes_without_asking_to_confirm_the_amount() {
    let mut harness = everyday_checking();
    change_the_rent_then_close(&mut harness);

    let closed = click_and_closes(&mut harness, "Discard");

    assert!(closed);
    assert!(!asks(&harness, RENT_CONFIRM));
}

#[test]
fn save_on_closing_with_a_changed_reconciled_memo_closes_without_a_confirm() {
    let mut harness = everyday_checking();
    click_cell(&mut harness, "Memo", "Maple Property Management");
    type_text(&mut harness, "September");
    close_window(&mut harness);

    let closed = click_and_closes(&mut harness, "Save");

    assert!(closed);
    assert!(can_undo(&mut harness));
}

/// Ticks "Save a changed transaction without asking", off until then, in the
/// Preferences.
fn turn_on_save_without_asking(harness: &mut Harness<'static, App>) {
    harness.get_by_role_and_label(Role::Button, "Tools").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Preferences…")
        .click();
    harness.run();
    harness.get_by_role_and_label(Role::Tab, "Register").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::CheckBox, "Save a changed transaction without asking")
        .click();
    harness.run();
    dialog_button(harness, "OK").click();
    harness.run();
}

#[test]
fn closing_with_save_without_asking_on_asks_to_confirm_a_reconciled_amount() {
    let mut harness = everyday_checking();
    turn_on_save_without_asking(&mut harness);

    change_the_rent_then_close(&mut harness);

    assert!(asks(&harness, RENT_CONFIRM));
}

#[test]
fn confirming_the_amount_with_save_without_asking_on_closes_once_saved() {
    let mut harness = everyday_checking();
    turn_on_save_without_asking(&mut harness);
    change_the_rent_then_close(&mut harness);

    let closed = click_and_closes(&mut harness, "Change Amount");

    assert!(closed);
    assert!(can_undo(&mut harness));
}

#[test]
fn cancelling_the_amount_confirm_with_save_without_asking_on_keeps_the_window() {
    let mut harness = everyday_checking();
    turn_on_save_without_asking(&mut harness);
    change_the_rent_then_close(&mut harness);

    let closed = click_and_closes(&mut harness, "Cancel");

    assert!(!closed);
    assert!(cursor_on(&harness, "Payment", "Maple Property Management"));
    assert!(editing(&harness));
    assert!(!can_undo(&mut harness));
}

#[test]
fn closing_with_save_without_asking_on_and_a_changed_reconciled_memo_closes() {
    let mut harness = everyday_checking();
    turn_on_save_without_asking(&mut harness);
    click_cell(&mut harness, "Memo", "Maple Property Management");
    type_text(&mut harness, "September");
    // The close's frame leaves the row, and the next ones save it and close.
    request_close(&mut harness);
    let closed = closes(&mut harness);

    assert!(closed);
}
