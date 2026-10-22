// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The "Can't save this transaction" Dialog: a save of the Register's editing row
//! that can't go through says why, and offers Continue editing or Discard changes.

use balanced_core::{
    AccountId, AccountStatus, CurrencyCode, DomainCommand, Ledger, Money, NewTransaction,
};
use balanced_i18n::Localizer;
use balanced_ui::testing;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The Dialog's heading.
const CANT_SAVE: &str = "Can't save this transaction";

/// Why "1.2.3" is no amount, in the test locale.
const UNREADABLE: &str = "Write amounts with “.” as the decimal mark and “,” between thousands.";

/// The App on stub data, in the test locale, wide enough to show every Register column.
fn app() -> Harness<'static, App> {
    app_on(Ledger::stub())
}

/// The App on `ledger`, as [`app`] makes it.
fn app_on(ledger: Ledger) -> Harness<'static, App> {
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

/// Whether a label shows `text`.
fn shows_text(harness: &Harness<'static, App>, text: &str) -> bool {
    harness
        .query_all_by_role(Role::Label)
        .any(|label| label.accesskit_node().value().as_deref() == Some(text))
}

/// Types an amount that can't be read into Fresh Market's Payment cell, leaving the
/// edit open.
fn mistype_fresh_markets_payment(harness: &mut Harness<'static, App>) {
    open(harness, "Everyday Checking");
    click_cell(harness, "86.42");
    type_text(harness, "1.2.3");
}

#[test]
fn an_unreadable_amount_saved_by_enter_says_why() {
    let mut harness = app();
    mistype_fresh_markets_payment(&mut harness);

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(shows_text(&harness, CANT_SAVE));
    assert!(shows_text(&harness, UNREADABLE));
}

/// Whether the "Can't save this transaction" Dialog is open.
fn refused(harness: &Harness<'static, App>) -> bool {
    shows_text(harness, CANT_SAVE)
}

/// Clicks the button labelled `label`.
fn click_button(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run();
}

/// The text in the open cell editor labelled `column`, or `None` when no such editor
/// is open.
fn editor(harness: &Harness<'static, App>, column: &str) -> Option<String> {
    harness
        .query_by_role_and_label(Role::TextInput, column)
        .map(|node| node.accesskit_node().value().unwrap_or_default())
}

/// The text the Status bar shows.
fn status(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .get_by_role(Role::Status)
        .query_all_by_role(Role::Label)
        .filter_map(|label| label.accesskit_node().value())
        .filter(|text| !text.is_empty())
        .collect()
}

/// Whether the Register shows a cell labelled `label`.
fn shows(harness: &Harness<'static, App>, label: &str) -> bool {
    harness.query_by_role_and_label(Role::Cell, label).is_some()
}

/// Fills the draft row of the account just opened, from the Date cell under the
/// cursor: Corner Shop paid `payment` on 15 October 2026, in `category`, if any.
fn fill_draft(harness: &mut Harness<'static, App>, category: Option<&str>, payment: &str) {
    for c in "10/15/2026".chars() {
        send(harness, Event::Text(c.to_string()));
        harness.run();
    }
    // Date, Num, Payee.
    press(harness, Modifiers::NONE, Key::Tab);
    press(harness, Modifiers::NONE, Key::Tab);
    type_text(harness, "Corner Shop");
    press(harness, Modifiers::NONE, Key::Tab);
    if let Some(category) = category {
        type_text(harness, category);
    }
    // Category, Memo, Payment.
    press(harness, Modifiers::NONE, Key::Tab);
    press(harness, Modifiers::NONE, Key::Tab);
    type_text(harness, payment);
}

#[test]
fn a_refused_save_shows_nothing_in_the_status_bar_or_a_bubble() {
    let mut harness = app();
    mistype_fresh_markets_payment(&mut harness);
    // Tab is refused too, saying why in a bubble below the cell.
    press(&mut harness, Modifiers::NONE, Key::Tab);

    press(&mut harness, Modifiers::NONE, Key::Enter);
    click_button(&mut harness, "Continue editing");

    assert_eq!(status(&harness), Vec::<String>::new());
    assert!(!shows_text(&harness, UNREADABLE));
}

#[test]
fn continue_editing_opens_the_amount_with_its_text_selected() {
    let mut harness = app();
    mistype_fresh_markets_payment(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Enter);

    click_button(&mut harness, "Continue editing");
    type_text(&mut harness, "5");

    assert!(!refused(&harness));
    assert_eq!(editor(&harness, "Payment").as_deref(), Some("5"));
}

#[test]
fn enter_in_the_dialog_continues_editing() {
    let mut harness = app();
    mistype_fresh_markets_payment(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Enter);

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(!refused(&harness));
    assert_eq!(editor(&harness, "Payment").as_deref(), Some("1.2.3"));
}

#[test]
fn escape_in_the_dialog_continues_editing() {
    let mut harness = app();
    mistype_fresh_markets_payment(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Enter);

    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert!(!refused(&harness));
    assert_eq!(editor(&harness, "Payment").as_deref(), Some("1.2.3"));
}

#[test]
fn discard_changes_throws_the_rows_changes_away() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Fresh Market");
    type_text(&mut harness, "Corner Shop");
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::NONE, Key::Tab);
    type_text(&mut harness, "1.2.3");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    click_button(&mut harness, "Discard changes");

    assert!(!refused(&harness));
    assert_eq!(editor(&harness, "Payment"), None);
    assert!(shows(&harness, "Fresh Market"));
    assert!(shows(&harness, "86.42"));
}

#[test]
fn ctrl_enter_on_an_unreadable_amount_says_why() {
    let mut harness = app();
    mistype_fresh_markets_payment(&mut harness);

    press(&mut harness, Modifiers::COMMAND, Key::Enter);

    assert!(refused(&harness));
    assert!(shows_text(&harness, UNREADABLE));
}

#[test]
fn leaving_a_row_that_cant_be_saved_says_why_instead_of_asking() {
    let mut harness = app();
    mistype_fresh_markets_payment(&mut harness);

    open(&mut harness, "Joint Checking");

    assert!(refused(&harness));
    assert!(shows_text(&harness, UNREADABLE));
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Save")
            .is_none()
    );
}

#[test]
fn discard_changes_lets_the_leave_go_on() {
    let mut harness = app();
    mistype_fresh_markets_payment(&mut harness);
    open(&mut harness, "Joint Checking");

    click_button(&mut harness, "Discard changes");

    assert!(!refused(&harness));
    assert!(!shows(&harness, "Fresh Market"));
}

#[test]
fn a_draft_row_with_no_category_says_why() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    fill_draft(&mut harness, None, "5.00");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(refused(&harness));
    assert!(shows_text(&harness, "Category is missing"));
}

#[test]
fn continue_editing_a_draft_row_with_no_category_opens_its_category() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    fill_draft(&mut harness, None, "5.00");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    click_button(&mut harness, "Continue editing");

    assert_eq!(editor(&harness, "Category").as_deref(), Some(""));
}

#[test]
fn leaving_a_draft_row_with_no_category_says_why_instead_of_asking() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    fill_draft(&mut harness, None, "5.00");

    click_cell(&mut harness, "Fresh Market");

    assert!(shows_text(&harness, "Category is missing"));
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Add")
            .is_none()
    );
}

#[test]
fn a_draft_row_the_ledger_refuses_says_why() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    // Too large for the account's balance to hold.
    fill_draft(
        &mut harness,
        Some("Food & Dining:Groceries"),
        "792281625142643375935439503.35",
    );

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(refused(&harness));
    assert!(shows_text(
        &harness,
        "That change would make a balance too large to hold."
    ));
    assert_eq!(status(&harness), Vec::<String>::new());
}

#[test]
fn continue_editing_after_the_ledger_refuses_opens_the_cell_saved_from() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    fill_draft(
        &mut harness,
        Some("Food & Dining:Groceries"),
        "792281625142643375935439503.35",
    );
    press(&mut harness, Modifiers::NONE, Key::Enter);

    click_button(&mut harness, "Continue editing");

    assert_eq!(
        editor(&harness, "Payment").as_deref(),
        Some("792,281,625,142,643,375,935,439,503.35")
    );
}

#[test]
fn an_emptied_payment_saves_as_zero() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "86.42");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    for _ in 0.."86.42".len() {
        press(&mut harness, Modifiers::NONE, Key::Backspace);
    }

    press(&mut harness, Modifiers::NONE, Key::Enter);
    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert!(!refused(&harness));
    assert!(!shows(&harness, "86.42"));
}

/// The id of the stub account named `name`.
fn account(ledger: &Ledger, name: &str) -> AccountId {
    ledger
        .snapshot()
        .accounts()
        .iter()
        .map(|summary| summary.account())
        .find(|account| account.name() == name)
        .expect("the stub has the account")
        .id()
}

/// The stub ledger with a transfer of 200 from Everyday Checking to Emergency Fund on
/// 1 October 2026, paid to "Saving up", and Emergency Fund closed since, so the ledger
/// refuses any change to the transfer.
fn with_closed_transfer() -> Ledger {
    let mut ledger = Ledger::stub();
    let everyday = account(&ledger, "Everyday Checking");
    let emergency = account(&ledger, "Emergency Fund");
    let usd = CurrencyCode::new("USD").expect("USD is an ISO 4217 currency");
    let amount = Money::new("-200.00".parse().expect("a decimal"), usd).expect("an amount");
    for command in [
        DomainCommand::PostTransaction {
            account: everyday,
            transaction: NewTransaction::transfer(
                Date::try_new_iso(2026, 10, 1).expect("1 October 2026 is a date"),
                "Saving up",
                emergency,
                amount,
            ),
        },
        DomainCommand::SetAccountStatus {
            account: emergency,
            status: AccountStatus::Closed,
        },
    ] {
        ledger.apply(command).expect("the stub takes the command");
    }
    ledger
}

/// Changes the closed transfer's payment to 250 in its Payment cell, leaving the edit
/// open.
fn change_closed_transfer(harness: &mut Harness<'static, App>) {
    open(harness, "Everyday Checking");
    click_cell(harness, "200.00");
    type_text(harness, "250");
}

#[test]
fn a_save_the_ledger_refuses_says_why() {
    let mut harness = app_on(with_closed_transfer());
    change_closed_transfer(&mut harness);

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(refused(&harness));
    assert!(shows_text(
        &harness,
        "The other account of this transfer is closed, so the transfer can't be made, changed or deleted."
    ));
    assert_eq!(status(&harness), Vec::<String>::new());
}

#[test]
fn continue_editing_after_the_ledger_refuses_a_save_opens_the_cell_saved_from() {
    let mut harness = app_on(with_closed_transfer());
    change_closed_transfer(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Enter);

    click_button(&mut harness, "Continue editing");

    assert_eq!(editor(&harness, "Payment").as_deref(), Some("250.00"));
}

/// Presses Enter, then sends `other` before the App applies the save Enter sent, so
/// the ledger takes both in the same frame.
fn press_enter_with(harness: &mut Harness<'static, App>, other: DomainCommand) {
    send(
        harness,
        Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        },
    );
    testing::send_domain(harness, other);
    harness.run();
}

/// The Command that renames the account named `name` in `ledger` to `to`, which the
/// ledger applies.
fn rename(ledger: &Ledger, name: &str, to: &str) -> DomainCommand {
    DomainCommand::RenameAccount {
        account: account(ledger, name),
        name: to.to_owned(),
    }
}

#[test]
fn a_save_refused_as_another_change_is_applied_keeps_the_rows_changes() {
    let ledger = with_closed_transfer();
    let other = rename(&ledger, "Everyday Checking", "Daily Checking");
    let mut harness = app_on(ledger);
    change_closed_transfer(&mut harness);

    press_enter_with(&mut harness, other);

    assert!(shows_text(
        &harness,
        "The other account of this transfer is closed, so the transfer can't be made, changed or deleted."
    ));
    click_button(&mut harness, "Continue editing");
    assert_eq!(editor(&harness, "Payment").as_deref(), Some("250.00"));
}

#[test]
fn a_post_refused_as_another_change_is_applied_keeps_the_draft() {
    let ledger = Ledger::stub();
    let other = rename(&ledger, "Everyday Checking", "Daily Checking");
    let mut harness = app_on(ledger);
    open(&mut harness, "Everyday Checking");
    // Too large for the account's balance to hold.
    fill_draft(
        &mut harness,
        Some("Food & Dining:Groceries"),
        "792281625142643375935439503.35",
    );

    press_enter_with(&mut harness, other);

    assert!(shows_text(
        &harness,
        "That change would make a balance too large to hold."
    ));
    click_button(&mut harness, "Continue editing");
    assert_eq!(
        editor(&harness, "Payment").as_deref(),
        Some("792,281,625,142,643,375,935,439,503.35")
    );
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

#[test]
fn clicking_another_row_of_a_row_that_cant_be_saved_says_why() {
    let mut harness = app();
    mistype_fresh_markets_payment(&mut harness);

    click_cell(&mut harness, "Bean There");

    assert!(refused(&harness));
    assert!(shows_text(&harness, UNREADABLE));
}

#[test]
fn discard_changes_lets_the_click_on_another_row_go_on() {
    let mut harness = app();
    mistype_fresh_markets_payment(&mut harness);
    click_cell(&mut harness, "Bean There");

    click_button(&mut harness, "Discard changes");

    assert_eq!(cursor(&harness), "Bean There");
    assert!(shows(&harness, "86.42"));
}

/// Types a date that can't be read into Fresh Market's Date cell, leaving the edit
/// open.
fn mistype_fresh_markets_date(harness: &mut Harness<'static, App>) {
    open(harness, "Everyday Checking");
    click_cell(harness, "9/5/2026");
    for c in "3/5".chars() {
        send(harness, Event::Text(c.to_string()));
        harness.run();
    }
}

#[test]
fn a_date_that_cant_be_read_saved_by_enter_says_why() {
    let mut harness = app();
    mistype_fresh_markets_date(&mut harness);

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(refused(&harness));
    assert!(shows_text(
        &harness,
        "Not a date. Expected M/D/YYYY, for example 10/5/2026."
    ));
}

#[test]
fn continue_editing_a_refused_date_opens_it_on_the_rows_date() {
    let mut harness = app();
    mistype_fresh_markets_date(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Enter);

    click_button(&mut harness, "Continue editing");

    assert_eq!(editor(&harness, "Date").as_deref(), Some("9/5/2026"));
}

#[test]
fn clicking_another_row_of_a_row_naming_no_category_says_why() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Auto & Transport:Fuel");
    type_text(&mut harness, "Pets");
    // ↑ clears the highlight, so the text stays as typed.
    press(&mut harness, Modifiers::NONE, Key::ArrowUp);

    click_cell(&mut harness, "Bean There");

    assert!(refused(&harness));
    assert!(shows_text(
        &harness,
        "No category has that name. Pick one from the list, or create it."
    ));
    assert_eq!(status(&harness), Vec::<String>::new());
}

#[test]
fn clicking_another_row_of_a_row_with_a_date_that_cant_be_read_says_why() {
    let mut harness = app();
    mistype_fresh_markets_date(&mut harness);

    click_cell(&mut harness, "Bean There");

    assert!(refused(&harness));
    assert!(shows_text(
        &harness,
        "Not a date. Expected M/D/YYYY, for example 10/5/2026."
    ));
}

#[test]
fn clicking_another_cell_of_a_row_naming_no_category_keeps_editing_and_says_why() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Auto & Transport:Fuel");
    type_text(&mut harness, "Pets");
    press(&mut harness, Modifiers::NONE, Key::ArrowUp);

    click_cell(&mut harness, "Shell");

    assert!(!refused(&harness));
    assert_eq!(editor(&harness, "Category").as_deref(), Some("Pets"));
    assert_eq!(
        status(&harness),
        ["No category has that name. Pick one from the list, or create it."]
    );
}
