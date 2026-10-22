// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Calculator, a Tool window opened from Tools ▸ Calculator, and its Paste into the
//! Register's amount cells, driven through the AccessKit tree the way a user drives it.

use balanced_core::{
    AccountId, CategoryId, CurrencyCode, DomainCommand, Ledger, Money, NewSplitLine, NewTransaction,
};
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
    app_on(i18n, Ledger::stub())
}

/// The App on `ledger`, shown by `i18n`, wide enough to show every Register column.
fn app_on(i18n: Localizer, ledger: Ledger) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(|cc| {
            App::on_ledger(
                cc,
                i18n,
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

/// Clicks the button labelled `label`.
fn click_button(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run();
}

/// Opens the Calculator from the Tools menu.
fn open_calculator(harness: &mut Harness<'static, App>) {
    click_button(harness, "Tools");
    click_button(harness, "Calculator");
}

/// The text in the Calculator's display, or `None` while the Calculator is closed.
fn display(harness: &Harness<'static, App>) -> Option<String> {
    harness
        .query_by_role_and_label(Role::TextInput, "Calculator")
        .map(|node| node.accesskit_node().value().unwrap_or_default())
}

#[test]
fn tools_calculator_opens_an_empty_calculator() {
    let mut harness = app();

    open_calculator(&mut harness);

    harness.get_by_role_and_label(Role::Window, "Calculator");
    assert_eq!(display(&harness).as_deref(), Some(""));
}

/// Whether a label in the window reads `text`.
fn shows(harness: &Harness<'static, App>, text: &str) -> bool {
    harness
        .query_all_by_role(Role::Label)
        .any(|label| label.accesskit_node().value().as_deref() == Some(text))
}

#[test]
fn the_calculators_keys_build_a_sum_and_show_its_result() {
    let mut harness = app();
    open_calculator(&mut harness);

    for key in ["1", "5", "4", "Multiply", "2"] {
        click_button(&mut harness, key);
    }

    assert_eq!(display(&harness).as_deref(), Some("154*2"));
    assert!(shows(&harness, "= $308.00"));
}

#[test]
fn equals_replaces_the_sum_with_its_result() {
    let mut harness = app();
    open_calculator(&mut harness);
    type_text(&mut harness, "154*2");

    click_button(&mut harness, "Equals");

    assert_eq!(display(&harness).as_deref(), Some("308.00"));
}

#[test]
fn enter_replaces_the_sum_with_its_result() {
    let mut harness = app();
    open_calculator(&mut harness);
    type_text(&mut harness, "100/3");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(display(&harness).as_deref(), Some("33.33"));
}

#[test]
fn a_rounded_result_is_marked() {
    let mut harness = app();
    open_calculator(&mut harness);

    type_text(&mut harness, "100/3");

    assert!(shows(&harness, "~ $33.33"));
}

#[test]
fn text_that_is_no_sum_is_not_an_amount_and_equals_leaves_it() {
    let mut harness = app();
    open_calculator(&mut harness);
    type_text(&mut harness, "1.2.3");

    click_button(&mut harness, "Equals");

    assert!(shows(&harness, "Not an amount"));
    assert_eq!(display(&harness).as_deref(), Some("1.2.3"));
}

#[test]
fn the_calculators_display_refuses_letters() {
    let mut harness = app();
    open_calculator(&mut harness);

    type_text(&mut harness, "1a+b2");

    assert_eq!(display(&harness).as_deref(), Some("1+2"));
}

#[test]
fn escape_closes_the_calculator() {
    let mut harness = app();
    open_calculator(&mut harness);

    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert_eq!(display(&harness), None);
}

#[test]
fn the_close_button_closes_the_calculator() {
    let mut harness = app();
    open_calculator(&mut harness);

    click_button(&mut harness, "Close calculator");

    assert_eq!(display(&harness), None);
}

#[test]
fn reopening_the_calculator_starts_empty() {
    let mut harness = app();
    open_calculator(&mut harness);
    type_text(&mut harness, "12");
    press(&mut harness, Modifiers::NONE, Key::Escape);

    open_calculator(&mut harness);

    assert_eq!(display(&harness).as_deref(), Some(""));
}

/// The text in the open cell editor labelled `column`, or `None` when no such editor
/// is open.
fn editor(harness: &Harness<'static, App>, column: &str) -> Option<String> {
    harness
        .query_by_role_and_label(Role::TextInput, column)
        .map(|node| node.accesskit_node().value().unwrap_or_default())
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

/// The cell under the header `title` of the first row whose Payee is `payee`.
fn cell_of(harness: &Harness<'static, App>, title: &str, payee: &str) -> String {
    let row = column(harness, "Payee")
        .iter()
        .position(|cell| cell == payee)
        .expect("the Register lists the payee");
    column(harness, title).swap_remove(row)
}

/// The Payment cell of the first row whose Payee is `payee`.
fn payment_of(harness: &Harness<'static, App>, payee: &str) -> String {
    cell_of(harness, "Payment", payee)
}

/// The Status bar's text.
fn status(harness: &Harness<'static, App>) -> String {
    let bar = harness.get_by_role(Role::Status);
    bar.query_all_by_role(Role::Label)
        .filter_map(|label| label.accesskit_node().value())
        .collect()
}

/// Works out `sum` in the open Calculator, typed into its display.
fn calculate(harness: &mut Harness<'static, App>, sum: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, "Calculator")
        .click();
    harness.run();
    type_text(harness, sum);
}

#[test]
fn the_calculator_stays_open_while_the_register_is_edited() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    open_calculator(&mut harness);
    type_text(&mut harness, "12");

    click_cell(&mut harness, "86.42");
    type_text(&mut harness, "5");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(display(&harness).as_deref(), Some("12"));
    assert_eq!(payment_of(&harness, "Fresh Market"), "5.00");
}

#[test]
fn paste_puts_the_result_into_the_amount_cell_last_clicked() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    open_calculator(&mut harness);
    calculate(&mut harness, "154*2");
    click_cell(&mut harness, "86.42");

    click_button(&mut harness, "Paste");

    assert_eq!(editor(&harness, "Payment").as_deref(), Some("308.00"));
    assert_eq!(payment_of(&harness, "Fresh Market"), "86.42");
}

#[test]
fn a_pasted_result_saves_when_the_cell_is_left() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    open_calculator(&mut harness);
    calculate(&mut harness, "154*2");
    click_cell(&mut harness, "86.42");
    click_button(&mut harness, "Paste");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(payment_of(&harness, "Fresh Market"), "308.00");
}

#[test]
fn a_pasted_negative_result_in_payment_saves_as_a_deposit() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    open_calculator(&mut harness);
    calculate(&mut harness, "10-22");
    click_cell(&mut harness, "86.42");
    click_button(&mut harness, "Paste");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(
        (
            payment_of(&harness, "Fresh Market"),
            cell_of(&harness, "Deposit", "Fresh Market"),
        ),
        (String::new(), "12.00".to_owned())
    );
}

#[test]
fn paste_goes_to_the_amount_cell_last_clicked_after_other_cells() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    open_calculator(&mut harness);
    calculate(&mut harness, "12");
    click_cell(&mut harness, "86.42");
    click_cell(&mut harness, "Bean There");

    click_button(&mut harness, "Paste");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(payment_of(&harness, "Fresh Market"), "12.00");
}

#[test]
fn paste_goes_to_a_deposit_cell_reached_by_keyboard() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    open_calculator(&mut harness);
    calculate(&mut harness, "12");
    click_cell(&mut harness, "86.42");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    press(&mut harness, Modifiers::NONE, Key::Tab);

    click_button(&mut harness, "Paste");

    assert_eq!(editor(&harness, "Deposit").as_deref(), Some("12.00"));
}

#[test]
fn paste_with_no_amount_cell_clicked_changes_nothing_and_says_why() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    open_calculator(&mut harness);
    calculate(&mut harness, "12");

    click_button(&mut harness, "Paste");

    assert_eq!(editor(&harness, "Payment"), None);
    assert_eq!(
        status(&harness),
        "Click a Payment or Deposit cell first, then Paste puts the result there."
    );
}

#[test]
fn paste_of_a_sum_that_is_no_amount_changes_nothing_and_says_why() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    open_calculator(&mut harness);
    calculate(&mut harness, "1.2.3");
    click_cell(&mut harness, "86.42");

    click_button(&mut harness, "Paste");

    assert_eq!(editor(&harness, "Payment"), None);
    assert_eq!(
        status(&harness),
        "The calculator's sum isn't an amount, so there is nothing to paste."
    );
}

#[test]
fn paste_leaves_the_calculator_open_with_its_sum() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    open_calculator(&mut harness);
    calculate(&mut harness, "154*2");
    click_cell(&mut harness, "86.42");

    click_button(&mut harness, "Paste");

    assert_eq!(display(&harness).as_deref(), Some("154*2"));
}

#[test]
fn the_calculator_pastes_german_amounts_with_a_decimal_comma() {
    let mut harness = app_in_german();
    open(&mut harness, "Everyday Checking");
    click_button(&mut harness, "Extras");
    click_button(&mut harness, "Taschenrechner");
    for key in ["1", "Dezimaltrennzeichen", "5", "Multiplizieren", "2"] {
        click_button(&mut harness, key);
    }
    click_cell(&mut harness, "86,42");

    click_button(&mut harness, "Einfügen");

    assert_eq!(editor(&harness, "Payment").as_deref(), Some("3,00"));
}

/// The heading of the column holding the cell cursor.
fn cursor_column(harness: &Harness<'static, App>) -> String {
    let cursor = harness
        .query_all_by_role(Role::Cell)
        .find(|cell| cell.accesskit_node().is_selected() == Some(true))
        .expect("a cell holds the cell cursor");
    let x = cursor.rect().center().x;
    harness
        .query_all_by_role(Role::ColumnHeader)
        .find(|header| header.rect().x_range().contains(x))
        .and_then(|header| header.accesskit_node().label())
        .expect("a heading tops the cell cursor's column")
}

#[test]
fn paste_goes_to_the_draft_row() {
    let mut harness = app();
    // Opening an account puts the cell cursor on its draft row.
    open(&mut harness, "Everyday Checking");
    while cursor_column(&harness) != "Payment" {
        press(&mut harness, Modifiers::NONE, Key::ArrowRight);
    }
    press(&mut harness, Modifiers::NONE, Key::Enter);
    open_calculator(&mut harness);
    calculate(&mut harness, "12");

    click_button(&mut harness, "Paste");

    assert_eq!(editor(&harness, "Payment").as_deref(), Some("12.00"));
    assert_eq!(payment_of(&harness, "Fresh Market"), "86.42");
}

fn usd(amount: &str) -> Money {
    let currency = CurrencyCode::new("USD").expect("USD is an ISO 4217 currency");
    Money::new(amount.parse().expect("a decimal"), currency).expect("an amount")
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

/// The id of the stub's top-level category named `name`.
fn category(ledger: &Ledger, name: &str) -> CategoryId {
    ledger
        .snapshot()
        .categories()
        .iter()
        .find(|category| category.name() == name)
        .expect("the stub has the category")
        .id()
}

#[test]
fn a_split_rows_amount_cell_is_no_paste_target() {
    let mut ledger = Ledger::stub();
    let everyday = account(&ledger, "Everyday Checking");
    let food = category(&ledger, "Food & Dining");
    ledger
        .apply(DomainCommand::PostTransaction {
            account: everyday,
            transaction: NewTransaction::split(
                Date::try_new_iso(2026, 10, 1).expect("1 October 2026 is a date"),
                "Payday",
                vec![
                    NewSplitLine::category(food, "Lunch", usd("-50.00")),
                    NewSplitLine::category(food, "Dinner", usd("-70.00")),
                ],
            ),
        })
        .expect("the stub takes the split");
    let mut harness = app_on(Localizer::for_tests(), ledger);
    open(&mut harness, "Everyday Checking");
    open_calculator(&mut harness);
    calculate(&mut harness, "12");
    click_cell(&mut harness, "86.42");
    click_cell(&mut harness, "120.00");

    click_button(&mut harness, "Paste");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(payment_of(&harness, "Fresh Market"), "12.00");
}

#[test]
fn another_account_has_no_paste_target() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    open_calculator(&mut harness);
    calculate(&mut harness, "12");
    click_cell(&mut harness, "86.42");
    open(&mut harness, "Joint Checking");

    click_button(&mut harness, "Paste");

    assert_eq!(editor(&harness, "Payment"), None);
    assert_eq!(
        status(&harness),
        "Click a Payment or Deposit cell first, then Paste puts the result there."
    );
}
