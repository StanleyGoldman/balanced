// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Split dialog, where a transaction's split lines are edited and kept until its
//! row is saved.

use balanced_core::{
    AccountId, AccountStatus, CategoryId, CurrencyCode, DomainCommand, Ledger, Money, NewSplitLine,
    NewTransaction,
};
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

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

/// The App on stub data, in the test locale, wide enough to show every Register column.
/// Everyday Checking holds Costco, split between Groceries and Household.
fn app() -> Harness<'static, App> {
    on(Ledger::stub())
}

/// The App on stub data in which Everyday Checking also holds a Payday split on 1
/// October: 50.00 on Food & Dining, and 200.00 to Emergency Fund, whose other side
/// there is memoed Rainy day.
fn payday() -> Harness<'static, App> {
    on(payday_ledger())
}

/// The stub ledger with the Payday split of [`payday`].
fn payday_ledger() -> Ledger {
    let mut ledger = Ledger::stub();
    let everyday = account(&ledger, "Everyday Checking");
    let emergency = account(&ledger, "Emergency Fund");
    let food = category(&ledger, "Food & Dining");
    ledger
        .apply(DomainCommand::PostTransaction {
            account: everyday,
            transaction: NewTransaction::split(
                Date::try_new_iso(2026, 10, 1).expect("1 October 2026 is a date"),
                "Payday",
                vec![
                    NewSplitLine::category(food, "Lunch", usd("-50.00")),
                    NewSplitLine::transfer(emergency, "Rainy day", usd("-200.00")),
                ],
            ),
        })
        .expect("the stub takes the split");
    ledger
}

/// The App on `ledger`, in the test locale, wide enough to show every Register column.
fn on(ledger: Ledger) -> Harness<'static, App> {
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

/// Clicks the button labelled `label`.
fn click_button(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run();
}

/// The text of the field labelled `label`.
fn field(harness: &Harness<'static, App>, label: &str) -> String {
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .accesskit_node()
        .value()
        .unwrap_or_default()
}

/// Every line the Split dialog shows, top first: its Category, Memo and Amount fields'
/// text.
fn lines(harness: &Harness<'static, App>) -> Vec<[String; 3]> {
    (1..)
        .map_while(|line| {
            harness
                .query_by_role_and_label(Role::TextInput, &format!("Line {line} category"))
                .map(|_| {
                    ["category", "memo", "amount"]
                        .map(|name| field(harness, &format!("Line {line} {name}")))
                })
        })
        .collect()
}

/// The lines `expected` as [`lines`] gives them.
fn expect_lines<const N: usize>(expected: [[&str; 3]; N]) -> Vec<[String; 3]> {
    expected
        .iter()
        .map(|line| line.map(str::to_owned))
        .collect()
}

#[test]
fn clicking_split_in_a_split_transactions_category_cell_shows_its_lines() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    click_cell(&mut harness, "[Split]");

    assert_eq!(
        lines(&harness),
        expect_lines([
            ["Food & Dining:Groceries", "Food", "142.37"],
            ["Shopping:Household", "Paper towels, detergent", "63.90"],
        ])
    );
}

/// Replaces the text of the field labelled `label` with `text`.
fn fill(harness: &mut Harness<'static, App>, label: &str, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .click();
    harness.run();
    press(harness, Modifiers::COMMAND, Key::A);
    type_text(harness, text);
}

/// Opens Fresh Market's Payee cell, which makes its row the editing row.
fn edit_fresh_market(harness: &mut Harness<'static, App>) {
    click_cell(harness, "Fresh Market");
    press(harness, Modifiers::NONE, Key::Enter);
}

/// How many cells are labelled `label`.
fn cells(harness: &Harness<'static, App>, label: &str) -> usize {
    harness
        .query_all_by_role_and_label(Role::Cell, label)
        .count()
}

#[test]
fn split_on_a_transaction_in_one_category_starts_with_one_line_for_its_whole_amount() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    edit_fresh_market(&mut harness);

    click_button(&mut harness, "Split");

    assert_eq!(
        lines(&harness),
        expect_lines([["Food & Dining:Groceries", "", "86.42"]])
    );
    assert_eq!(field(&harness, "Total"), "86.42");
}

#[test]
fn splitting_a_transaction_in_one_category_and_saving_the_row_makes_it_a_split() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    edit_fresh_market(&mut harness);
    click_button(&mut harness, "Split");
    fill(&mut harness, "Line 1 amount", "60.00");
    click_button(&mut harness, "Add line");
    type_text(&mut harness, "Shopping:Household");
    fill(&mut harness, "Line 2 amount", "26.42");

    click_button(&mut harness, "Done");
    click_button(&mut harness, "Save");
    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert_eq!(cells(&harness, "[Split]"), 2);
    click_cell(&mut harness, "Fresh Market");
    press(&mut harness, Modifiers::NONE, Key::ArrowRight);
    press(&mut harness, Modifiers::NONE, Key::Enter);
    assert_eq!(
        lines(&harness),
        expect_lines([
            ["Food & Dining:Groceries", "", "60.00"],
            ["Shopping:Household", "", "26.42"],
        ])
    );
}

/// Opens the Split dialog on Payday's lines, from its Category cell.
fn split_payday(harness: &mut Harness<'static, App>) {
    click_cell(harness, "Payday");
    press(harness, Modifiers::NONE, Key::ArrowRight);
    press(harness, Modifiers::NONE, Key::Enter);
}

/// Whether a label shows `text`, as Dialogs show messages.
fn shows_text(harness: &Harness<'static, App>, text: &str) -> bool {
    harness
        .query_all_by_role(Role::Label)
        .any(|label| label.accesskit_node().value().as_deref() == Some(text))
}

#[test]
fn a_line_to_an_account_shows_as_the_account_in_brackets() {
    let mut harness = payday();
    open(&mut harness, "Everyday Checking");

    split_payday(&mut harness);

    assert_eq!(
        lines(&harness),
        expect_lines([
            ["Food & Dining", "Lunch", "50.00"],
            ["[Emergency Fund]", "Rainy day", "200.00"],
        ])
    );
}

#[test]
fn saving_a_new_line_to_an_account_gives_it_an_other_side_there() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "[Split]");
    fill(&mut harness, "Line 2 category", "[Emergency Fund]");

    click_button(&mut harness, "Done");
    click_button(&mut harness, "Save");
    open(&mut harness, "Emergency Fund");

    assert_eq!(cells(&harness, "Costco"), 1);
    assert_eq!(cells(&harness, "Paper towels, detergent"), 1);
    assert_eq!(cells(&harness, "63.90"), 1);
}

#[test]
fn remove_takes_its_line_out() {
    let mut harness = payday();
    open(&mut harness, "Everyday Checking");
    split_payday(&mut harness);

    click_button(&mut harness, "Remove line 1");

    assert_eq!(
        lines(&harness),
        expect_lines([["[Emergency Fund]", "Rainy day", "200.00"]])
    );
}

#[test]
fn saving_without_a_line_to_an_account_deletes_its_other_side() {
    let mut harness = payday();
    open(&mut harness, "Everyday Checking");
    split_payday(&mut harness);
    click_button(&mut harness, "Remove line 2");
    fill(&mut harness, "Total", "50.00");

    click_button(&mut harness, "Done");
    click_button(&mut harness, "Save");
    open(&mut harness, "Emergency Fund");

    assert_eq!(cells(&harness, "Payday"), 0);
}

#[test]
fn a_kept_line_to_an_account_keeps_its_other_side_and_that_sides_memo() {
    let mut harness = payday();
    open(&mut harness, "Everyday Checking");
    split_payday(&mut harness);
    fill(&mut harness, "Line 2 memo", "Holiday");
    fill(&mut harness, "Line 2 amount", "250.00");
    fill(&mut harness, "Total", "300.00");

    click_button(&mut harness, "Done");
    click_button(&mut harness, "Save");
    open(&mut harness, "Emergency Fund");

    assert_eq!(cells(&harness, "Rainy day"), 1);
    assert_eq!(cells(&harness, "250.00"), 1);
}

#[test]
fn done_with_lines_that_dont_add_up_stays_open_and_shows_the_difference() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "[Split]");
    fill(&mut harness, "Line 1 amount", "100.00");

    click_button(&mut harness, "Done");

    assert!(shows_text(
        &harness,
        "The lines add up to 163.90, not to the total of 206.27. Difference: 42.37"
    ));
    assert_eq!(lines(&harness).len(), 2);
}

/// Renames Fresh Market to Corner Shop in its Payee cell, leaving the cell open.
fn rename_fresh_market(harness: &mut Harness<'static, App>) {
    click_cell(harness, "Fresh Market");
    type_text(harness, "Corner Shop");
}

/// Splits the editing row, Fresh Market's, between Groceries and Household and closes
/// the Split dialog with Done.
fn split_fresh_market(harness: &mut Harness<'static, App>) {
    click_button(harness, "Split");
    fill(harness, "Line 1 amount", "60.00");
    click_button(harness, "Add line");
    type_text(harness, "Shopping:Household");
    fill(harness, "Line 2 amount", "26.42");
    click_button(harness, "Done");
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

#[test]
fn escape_closes_the_dialog_dropping_its_changes_and_keeps_the_rows_other_changes() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);
    click_button(&mut harness, "Split");
    fill(&mut harness, "Line 1 amount", "60.00");

    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert_eq!(lines(&harness), expect_lines([]));
    click_button(&mut harness, "Save");
    assert_eq!(cells(&harness, "Corner Shop"), 1);
    assert_eq!(cells(&harness, "[Split]"), 1);
    assert!(can_undo(&mut harness));
}

#[test]
fn cancel_on_the_row_after_done_throws_away_its_lines_too() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    edit_fresh_market(&mut harness);
    split_fresh_market(&mut harness);
    assert_eq!(cells(&harness, "[Split]"), 2);

    click_button(&mut harness, "Cancel");

    assert_eq!(cells(&harness, "[Split]"), 1);
    assert!(!can_undo(&mut harness));
}

#[test]
fn one_undo_takes_back_the_lines_and_the_rows_other_changes() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);
    split_fresh_market(&mut harness);
    click_button(&mut harness, "Save");
    press(&mut harness, Modifiers::NONE, Key::Escape);

    click_button(&mut harness, "Edit");
    click_button(&mut harness, "Undo Ctrl+Z");

    assert_eq!(cells(&harness, "Fresh Market"), 1);
    assert_eq!(cells(&harness, "[Split]"), 1);
    assert!(!can_undo(&mut harness));
}

#[test]
fn a_changed_amount_then_split_saves_as_the_lines_total() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "86.42");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    press(&mut harness, Modifiers::COMMAND, Key::A);
    type_text(&mut harness, "90.00");
    click_button(&mut harness, "Split");
    fill(&mut harness, "Line 1 amount", "50.00");
    click_button(&mut harness, "Add line");
    type_text(&mut harness, "Shopping:Household");
    fill(&mut harness, "Line 2 amount", "40.00");
    click_button(&mut harness, "Done");

    // A refused save opens the "Can't save this transaction" Dialog a few Commands
    // later, which Esc then closes.
    harness.get_by_role_and_label(Role::Button, "Save").click();
    harness.run_steps(8);
    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert!(can_undo(&mut harness));
    assert_eq!(cells(&harness, "90.00"), 1);
    assert_eq!(cells(&harness, "[Split]"), 2);
}

/// Opens the Split dialog on the September rent, a Reconciled transaction of 1,450.00
/// numbered 1041.
fn split_rent(harness: &mut Harness<'static, App>) {
    click_cell(harness, "1041");
    press(harness, Modifiers::NONE, Key::Enter);
    click_button(harness, "Split");
}

#[test]
fn lines_that_change_a_reconciled_transactions_amount_ask_first() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    split_rent(&mut harness);
    fill(&mut harness, "Line 1 amount", "1500.00");
    fill(&mut harness, "Total", "1500.00");
    click_button(&mut harness, "Done");

    click_button(&mut harness, "Save");

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Change Amount")
            .is_some()
    );
}

#[test]
fn lines_that_keep_a_reconciled_transactions_amount_save_without_asking() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    split_rent(&mut harness);
    fill(&mut harness, "Line 1 amount", "1400.00");
    click_button(&mut harness, "Add line");
    type_text(&mut harness, "Shopping:Household");
    fill(&mut harness, "Line 2 amount", "50.00");
    click_button(&mut harness, "Done");

    click_button(&mut harness, "Save");

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Change Amount")
            .is_none()
    );
    assert!(can_undo(&mut harness));
}

#[test]
fn adding_a_split_draft_row_posts_its_lines() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    for c in "10/15/2026".chars() {
        type_text(&mut harness, &c.to_string());
    }
    // Date, Num, Payee.
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::NONE, Key::Tab);
    type_text(&mut harness, "Corner Cafe");
    click_button(&mut harness, "Split");
    fill(&mut harness, "Line 1 category", "Food & Dining:Groceries");
    fill(&mut harness, "Line 1 amount", "20.00");
    click_button(&mut harness, "Add line");
    type_text(&mut harness, "Shopping:Household");
    fill(&mut harness, "Line 2 amount", "10.00");
    fill(&mut harness, "Total", "30.00");
    click_button(&mut harness, "Done");

    click_button(&mut harness, "Add");

    assert!(can_undo(&mut harness));
    click_cell(&mut harness, "Corner Cafe");
    press(&mut harness, Modifiers::NONE, Key::ArrowRight);
    press(&mut harness, Modifiers::NONE, Key::Enter);
    assert_eq!(
        lines(&harness),
        expect_lines([
            ["Food & Dining:Groceries", "", "20.00"],
            ["Shopping:Household", "", "10.00"],
        ])
    );
}

/// Opens the Split dialog on Costco and starts typing `text` in a new third line's
/// Category field.
fn type_third_category(harness: &mut Harness<'static, App>, text: &str) {
    open(harness, "Everyday Checking");
    click_cell(harness, "[Split]");
    click_button(harness, "Add line");
    type_text(harness, text);
}

#[test]
fn typing_a_lines_category_completes_it() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "[Split]");
    click_button(&mut harness, "Add line");

    type_text(&mut harness, "Shopping:Ho");

    assert_eq!(field(&harness, "Line 3 category"), "Shopping:Household");
}

#[test]
fn tab_from_a_lines_category_takes_the_lists_highlighted_entry() {
    let mut harness = app();
    type_third_category(&mut harness, "Insurance");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        field(&harness, "Line 3 category"),
        "Auto & Transport:Insurance"
    );
}

#[test]
fn enter_in_a_lines_open_category_list_takes_the_entry_and_keeps_the_dialog_open() {
    let mut harness = app();
    type_third_category(&mut harness, "[Emer");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(field(&harness, "Line 3 category"), "[Emergency Fund]");
}

#[test]
fn escape_in_a_lines_open_category_list_closes_the_list_and_keeps_the_dialog_open() {
    let mut harness = app();
    type_third_category(&mut harness, "Shopping:Ho");

    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert_eq!(field(&harness, "Line 3 category"), "Shopping:Ho");
}

#[test]
fn down_moves_the_highlight_in_a_lines_category_list() {
    let mut harness = app();
    type_third_category(&mut harness, "Insurance");

    press(&mut harness, Modifiers::NONE, Key::ArrowDown);
    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(field(&harness, "Line 3 category"), "Home:Insurance");
}

#[test]
fn escape_in_a_lines_category_with_its_list_closed_closes_the_dialog() {
    let mut harness = app();
    type_third_category(&mut harness, "Shopping:Ho");
    press(&mut harness, Modifiers::NONE, Key::Escape);

    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert_eq!(lines(&harness), expect_lines([]));
}

#[test]
fn left_and_right_in_a_lines_category_move_the_caret_and_keep_the_field_focused() {
    let mut harness = app();
    type_third_category(&mut harness, "Zq");

    press(&mut harness, Modifiers::NONE, Key::ArrowLeft);
    type_text(&mut harness, "x");
    press(&mut harness, Modifiers::NONE, Key::ArrowRight);
    type_text(&mut harness, "y");

    assert_eq!(field(&harness, "Line 3 category"), "Zxqy");
    assert!(
        harness
            .get_by_role_and_label(Role::TextInput, "Line 3 category")
            .is_focused()
    );
}

#[test]
fn clicking_split_on_another_row_asks_about_the_changed_row_first() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);

    click_cell(&mut harness, "[Split]");

    assert!(shows_text(&harness, "Save the changed transaction?"));
    assert_eq!(lines(&harness), expect_lines([]));
}

/// The button labelled `label` in the open Dialog, not among the editing row's
/// buttons.
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

/// Costco's lines, which the Split dialog shows once it opens on Costco's row.
fn costco_lines() -> Vec<[String; 3]> {
    expect_lines([
        ["Food & Dining:Groceries", "Food", "142.37"],
        ["Shopping:Household", "Paper towels, detergent", "63.90"],
    ])
}

#[test]
fn save_in_the_prompt_a_split_click_raised_saves_the_row_and_opens_the_split_dialog() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);
    click_cell(&mut harness, "[Split]");

    harness.get_by_role_and_label(Role::Button, "Save").click();
    // The save, then the Split dialog it asks for: more frames than `run` allows.
    harness.run_steps(8);

    assert_eq!(cells(&harness, "Corner Shop"), 1);
    assert_eq!(lines(&harness), costco_lines());
}

#[test]
fn discard_in_the_prompt_a_split_click_raised_drops_the_changes_and_opens_the_split_dialog() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);
    click_cell(&mut harness, "[Split]");

    click_button(&mut harness, "Discard");

    assert_eq!(cells(&harness, "Fresh Market"), 1);
    assert_eq!(lines(&harness), costco_lines());
}

#[test]
fn add_in_the_prompt_a_split_click_raised_on_the_draft_row_opens_the_split_dialog() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    for c in "10/15/2026".chars() {
        type_text(&mut harness, &c.to_string());
    }
    // Date, Num, Payee, then Category, Memo, Payment.
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::NONE, Key::Tab);
    type_text(&mut harness, "Corner Shop");
    press(&mut harness, Modifiers::NONE, Key::Tab);
    type_text(&mut harness, "Food & Dining:Groceries");
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::NONE, Key::Tab);
    type_text(&mut harness, "12.00");
    click_cell(&mut harness, "[Split]");

    harness.get_by_role_and_label(Role::Button, "Add").click();
    // The post, then the Split dialog it asks for: more frames than `run` allows.
    harness.run_steps(8);

    assert_eq!(cells(&harness, "Corner Shop"), 1);
    assert_eq!(lines(&harness), costco_lines());
}

#[test]
fn continue_editing_in_the_prompt_a_split_click_raised_opens_nothing() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    rename_fresh_market(&mut harness);
    click_cell(&mut harness, "[Split]");

    click_button(&mut harness, "Continue editing");

    assert_eq!(lines(&harness), expect_lines([]));
    assert_eq!(field(&harness, "Payee"), "Corner Shop");
}

#[test]
fn a_refused_save_in_the_prompt_a_split_click_raised_opens_nothing() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "1,450.00");
    type_text(&mut harness, "1500.00");
    click_cell(&mut harness, "[Split]");
    click_button(&mut harness, "Save");

    dialog_button(&harness, "Cancel").click();
    harness.run();

    assert_eq!(lines(&harness), expect_lines([]));
}

#[test]
fn done_with_a_line_naming_no_category_says_so() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "[Split]");
    fill(&mut harness, "Line 2 category", "Nowhere");
    press(&mut harness, Modifiers::NONE, Key::ArrowUp);

    click_button(&mut harness, "Done");

    assert!(shows_text(
        &harness,
        "Line 2: no category or account is named “Nowhere”."
    ));
}

#[test]
fn done_with_nothing_changed_leaves_a_transaction_in_one_category_as_it_was() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    edit_fresh_market(&mut harness);
    click_button(&mut harness, "Split");

    click_button(&mut harness, "Done");

    assert_eq!(cells(&harness, "[Split]"), 1);
    assert_eq!(cells(&harness, "Food & Dining:Groceries"), 1);
}

#[test]
fn escape_from_a_dialog_opened_on_a_highlighted_row_leaves_it_the_editing_row() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "[Split]");

    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Save")
            .is_some()
    );
}

/// The App on the data of [`payday`], with Emergency Fund, which Payday has a line to,
/// closed since.
fn payday_to_a_closed_account() -> Harness<'static, App> {
    let mut ledger = payday_ledger();
    let emergency = account(&ledger, "Emergency Fund");
    ledger
        .apply(DomainCommand::SetAccountStatus {
            account: emergency,
            status: AccountStatus::Closed,
        })
        .expect("the stub closes the account");
    on(ledger)
}

/// Why the ledger refuses any change to Payday's lines or date in
/// [`payday_to_a_closed_account`].
const SPLIT_LINE_CLOSED: &str =
    "This split has a line to Emergency Fund, which is closed. Reopen it to change the split.";

#[test]
fn changing_a_splits_lines_while_one_goes_to_a_closed_account_names_the_account() {
    let mut harness = payday_to_a_closed_account();
    open(&mut harness, "Everyday Checking");
    split_payday(&mut harness);
    fill(&mut harness, "Line 1 memo", "Dinner");

    click_button(&mut harness, "Done");
    // A refused save opens the "Can't save this transaction" Dialog a few Commands
    // later.
    harness.get_by_role_and_label(Role::Button, "Save").click();
    harness.run_steps(8);

    assert!(shows_text(&harness, "Can't save this transaction"));
    assert!(shows_text(&harness, SPLIT_LINE_CLOSED));
}

#[test]
fn done_reads_a_kept_line_to_a_closed_account_as_that_account() {
    let mut harness = payday_to_a_closed_account();
    open(&mut harness, "Everyday Checking");
    split_payday(&mut harness);

    click_button(&mut harness, "Done");

    assert_eq!(lines(&harness), expect_lines([]));
}

#[test]
fn changing_a_splits_date_while_a_line_goes_to_a_closed_account_names_the_account() {
    let mut harness = payday_to_a_closed_account();
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "10/1/2026");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    type_text(&mut harness, "+");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(shows_text(&harness, "Can't save this transaction"));
    assert!(shows_text(&harness, SPLIT_LINE_CLOSED));
}
