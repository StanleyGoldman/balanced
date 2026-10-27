// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Payee recall: committing the draft row's Payee cell with a payee an earlier
//! transaction has fills the draft row's empty cells from that transaction.

use balanced_core::{
    AccountId, AccountStatus, CategoryId, CurrencyCode, DomainCommand, Ledger, Money, NewSplitLine,
    NewTransaction, RegisterEntry, TransactionEdit,
};
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on `ledger`, in the test locale, wide enough to show every Register column,
/// keeping its Preferences in `preferences`.
fn app_with(ledger: Ledger, preferences: PreferencesFile) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 720.0))
        .build_eframe(|cc| {
            App::on_ledger(
                cc,
                Localizer::for_tests(),
                ledger,
                Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
                preferences,
            )
        });
    harness.run();
    harness
}

/// The App on `ledger`, in the test locale, wide enough to show every Register column.
fn app_on(ledger: Ledger) -> Harness<'static, App> {
    app_with(ledger, PreferencesFile::for_tests())
}

/// Opens Everyday Checking from the Account bar.
fn open_everyday_checking(harness: &mut Harness<'static, App>) {
    harness
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .click();
    harness.run();
}

/// Clicks the widget with `role` labelled `label`, then lets the App settle.
fn click(harness: &mut Harness<'static, App>, role: Role, label: &str) {
    harness.get_by_role_and_label(role, label).click();
    harness.run();
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

/// The id of the stub category named `name`.
fn category(ledger: &Ledger, name: &str) -> CategoryId {
    ledger
        .snapshot()
        .categories()
        .iter()
        .find(|category| category.name() == name)
        .expect("the stub has the category")
        .id()
}

/// `amount`, a decimal, in US dollars.
fn usd(amount: &str) -> Money {
    let usd = CurrencyCode::new("USD").expect("USD is an ISO 4217 currency");
    Money::new(amount.parse().expect("a decimal"), usd).expect("an amount")
}

/// Posts a payment of `amount` to Corner Cafe in Joint Checking, on `day` October
/// 2026, in the stub category named `category_name`.
fn pay_corner_cafe(ledger: &mut Ledger, day: u8, category_name: &str, amount: &str) {
    let amount = usd(amount);
    let date = Date::try_new_iso(2026, 10, day).expect("a day of October 2026");
    let transaction =
        NewTransaction::new(date, "Corner Cafe", category(ledger, category_name), amount);
    ledger
        .apply(DomainCommand::PostTransaction {
            account: account(ledger, "Joint Checking"),
            transaction,
        })
        .expect("the stub takes the payment");
}

/// Posts a transfer of `amount` from the stub account named `from` to the one named
/// `to`, to "Saving up" on 1 October 2026, with memo "Monthly top-up".
fn transfer(ledger: &mut Ledger, from: &str, to: &str, amount: &str) {
    let amount = usd(amount);
    let date = Date::try_new_iso(2026, 10, 1).expect("1 October 2026 is a date");
    let transaction = NewTransaction::transfer(date, "Saving up", account(ledger, to), amount)
        .memo("Monthly top-up");
    ledger
        .apply(DomainCommand::PostTransaction {
            account: account(ledger, from),
            transaction,
        })
        .expect("the stub takes the transfer");
}

/// Renames the payee of the transaction to "Saving up" in the stub account named `name`
/// to `payee`.
fn rename_saving_up_in(ledger: &mut Ledger, name: &str, payee: &str) {
    let snapshot = ledger.snapshot();
    let transaction = snapshot
        .account(account(ledger, name))
        .expect("the stub has the account")
        .register()
        .iter()
        .map(RegisterEntry::transaction)
        .find(|transaction| transaction.payee() == "Saving up")
        .expect("the account has the transfer")
        .id();
    ledger
        .apply(DomainCommand::EditTransaction {
            transaction,
            edit: TransactionEdit::Payee(payee.to_owned()),
        })
        .expect("the stub renames the payee");
}

/// Closes the stub account named `name`.
fn close(ledger: &mut Ledger, name: &str) {
    ledger
        .apply(DomainCommand::SetAccountStatus {
            account: account(ledger, name),
            status: AccountStatus::Closed,
        })
        .expect("the stub closes the account");
}

/// The App on `ledger` with Everyday Checking open, its cell cursor on the draft row's
/// Date cell.
fn everyday_checking_on(ledger: Ledger) -> Harness<'static, App> {
    let mut harness = app_on(ledger);
    open_everyday_checking(&mut harness);
    harness
}

/// The App on stub data with Everyday Checking open, its cell cursor on the draft
/// row's Date cell.
fn everyday_checking() -> Harness<'static, App> {
    everyday_checking_on(Ledger::stub())
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

/// The labels of every Register cell, top row first, each row left to right.
fn cells(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::Cell)
        .map(|cell| cell.accesskit_node().label().unwrap_or_default())
        .collect()
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

#[test]
fn tab_from_a_remembered_payee_fills_category_memo_and_payment() {
    let mut harness = everyday_checking();
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

/// The cells of the first row dated `date`, from Date to Deposit.
fn row_dated(harness: &Harness<'static, App>, date: &str) -> Vec<String> {
    let cells = cells(harness);
    let start = cells
        .iter()
        .position(|cell| cell == date)
        .expect("the Register lists a row of that date");
    cells[start..start + 7].to_vec()
}

/// The cells of the first row whose payee is `payee`, from Date to Deposit.
fn row_of(harness: &Harness<'static, App>, payee: &str) -> Vec<String> {
    let cells = cells(harness);
    let payee = cells
        .iter()
        .position(|cell| cell == payee)
        .expect("the Register lists the payee");
    cells[payee - 2..payee + 5].to_vec()
}

#[test]
fn a_recalled_row_posts_with_what_recall_filled() {
    let mut harness = everyday_checking();
    type_text(&mut harness, "10/15/2026");
    type_payee(&mut harness, "Trattoria Roma");
    press(&mut harness, Modifiers::NONE, Key::Tab);

    press(&mut harness, Modifiers::COMMAND, Key::Enter);

    assert_eq!(
        row_dated(&harness, "10/15/2026"),
        [
            "10/15/2026",
            "",
            "Trattoria Roma",
            "Food & Dining:Restaurants",
            "Anniversary dinner",
            "64.80",
            "",
        ]
    );
    assert_eq!(draft_row(&harness), ["", "", "", "", "", "", ""]);
}

#[test]
fn a_remembered_deposit_fills_deposit() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "Acme Corp");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        ["", "", "Acme Corp", "Income:Salary", "", "", "3,200.00"]
    );
}

#[test]
fn enter_on_a_remembered_payee_fills_the_row() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "Trattoria Roma");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Trattoria Roma",
            "Food & Dining:Restaurants",
            "Anniversary dinner",
            "64.80",
            "",
        ]
    );
}

#[test]
fn picking_a_remembered_payee_from_the_list_fills_the_row() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "Tratt");

    harness
        .get_by_role_and_label(Role::ListBoxOption, "Trattoria Roma")
        .click();
    harness.run();

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Trattoria Roma",
            "Food & Dining:Restaurants",
            "Anniversary dinner",
            "64.80",
            "",
        ]
    );
}

#[test]
fn a_payee_last_paid_from_another_account_recalls_that_payment() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "Safe Auto Insurance");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Safe Auto Insurance",
            "Auto & Transport:Insurance",
            "",
            "110.00",
            "",
        ]
    );
}

#[test]
fn a_payee_last_paid_from_a_closed_account_recalls_that_payment() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "Cash withdrawal");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Cash withdrawal",
            "Shopping",
            "Closing the account",
            "250.42",
            "",
        ]
    );
}

#[test]
fn the_latest_date_wins_over_the_later_entered() {
    let mut ledger = Ledger::stub();
    pay_corner_cafe(&mut ledger, 5, "Restaurants", "-30.00");
    pay_corner_cafe(&mut ledger, 1, "Coffee Shops", "-4.00");
    let mut harness = everyday_checking_on(ledger);
    type_payee(&mut harness, "Corner Cafe");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Corner Cafe",
            "Food & Dining:Restaurants",
            "",
            "30.00",
            "",
        ]
    );
}

#[test]
fn on_the_same_date_the_later_entered_wins() {
    let mut ledger = Ledger::stub();
    pay_corner_cafe(&mut ledger, 1, "Restaurants", "-30.00");
    pay_corner_cafe(&mut ledger, 1, "Coffee Shops", "-4.00");
    let mut harness = everyday_checking_on(ledger);
    type_payee(&mut harness, "Corner Cafe");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Corner Cafe",
            "Food & Dining:Restaurants:Coffee Shops",
            "",
            "4.00",
            "",
        ]
    );
}

#[test]
fn a_payee_typed_in_another_case_recalls_and_takes_the_remembered_spelling() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "trattoria ROMA");
    // ↑ clears the list's highlight, so Tab commits the text as typed.
    press(&mut harness, Modifiers::NONE, Key::ArrowUp);

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Trattoria Roma",
            "Food & Dining:Restaurants",
            "Anniversary dinner",
            "64.80",
            "",
        ]
    );
}

#[test]
fn a_second_remembered_payee_fills_only_the_cells_still_empty() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "Daily Grind");
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::SHIFT, Key::Tab);
    for _ in "Daily Grind".chars() {
        press(&mut harness, Modifiers::NONE, Key::Backspace);
    }
    type_text(&mut harness, "Trattoria Roma");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Trattoria Roma",
            "Food & Dining:Restaurants:Coffee Shops",
            "Anniversary dinner",
            "5.25",
            "",
        ]
    );
}

#[test]
fn a_memo_typed_before_the_payee_is_kept() {
    let mut harness = everyday_checking();
    // Date, Num, Payee, Category, Memo.
    for _ in 0..4 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
    }
    type_text(&mut harness, "Lunch");
    for _ in 0..2 {
        press(&mut harness, Modifiers::SHIFT, Key::Tab);
    }
    type_text(&mut harness, "Trattoria Roma");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Trattoria Roma",
            "Food & Dining:Restaurants",
            "Lunch",
            "64.80",
            "",
        ]
    );
}

#[test]
fn a_recalled_category_lets_a_draft_row_refused_for_its_category_be_added() {
    let mut harness = everyday_checking();
    type_text(&mut harness, "10/15/2026");
    press(&mut harness, Modifiers::COMMAND, Key::Enter);
    harness.get_by_label("Category is missing");
    click(&mut harness, Role::Button, "Continue editing");
    // Continue editing put the cursor on Category; back to Payee.
    press(&mut harness, Modifiers::SHIFT, Key::Tab);
    type_text(&mut harness, "Trattoria Roma");
    press(&mut harness, Modifiers::NONE, Key::Tab);

    press(&mut harness, Modifiers::COMMAND, Key::Enter);

    assert_eq!(draft_row(&harness), ["", "", "", "", "", "", ""]);
}

#[test]
fn a_payee_with_no_earlier_transaction_fills_nothing() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "Corner Cafe");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(draft_row(&harness), ["", "", "Corner Cafe", "", "", "", ""]);
}

#[test]
fn changing_an_existing_transactions_payee_changes_only_its_payee() {
    let mut harness = everyday_checking();
    harness
        .get_by_role_and_label(Role::Cell, "Bean There")
        .click();
    harness.run();
    type_text(&mut harness, "Trattoria Roma");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(
        row_of(&harness, "Trattoria Roma"),
        [
            "9/6/2026",
            "",
            "Trattoria Roma",
            "Food & Dining:Restaurants:Coffee Shops",
            "",
            "4.75",
            "",
        ]
    );
    assert_eq!(draft_row(&harness), ["", "", "", "", "", "", ""]);
}

#[test]
fn undo_after_a_recall_and_a_post_takes_the_post_back_out() {
    let mut harness = everyday_checking();
    let before = cells(&harness);
    type_text(&mut harness, "10/15/2026");
    type_payee(&mut harness, "Trattoria Roma");
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::COMMAND, Key::Enter);

    press(&mut harness, Modifiers::COMMAND, Key::Z);

    assert_eq!(cells(&harness), before);
}

#[test]
fn with_payee_recall_off_a_remembered_payee_fills_nothing_and_keeps_its_typing() {
    let dir = tempfile::tempdir().expect("the test machine can make a temporary directory");
    std::fs::write(
        dir.path().join("preferences.toml"),
        "version = 2\npayee_recall = false\n",
    )
    .expect("the test can write the preferences file");
    let mut harness = app_with(Ledger::stub(), PreferencesFile::in_dir(dir.path()));
    open_everyday_checking(&mut harness);
    type_payee(&mut harness, "trattoria ROMA");
    // ↑ clears the list's highlight, so Tab commits the text as typed.
    press(&mut harness, Modifiers::NONE, Key::ArrowUp);

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        ["", "", "trattoria ROMA", "", "", "", ""]
    );
}

#[test]
fn turning_payee_recall_off_in_the_preferences_dialog_stops_it_on_ok() {
    let dir = tempfile::tempdir().expect("the test machine can make a temporary directory");
    let mut harness = app_with(Ledger::stub(), PreferencesFile::in_dir(dir.path()));
    open_everyday_checking(&mut harness);
    click(&mut harness, Role::Button, "Tools");
    click(&mut harness, Role::Button, "Preferences…");
    click(&mut harness, Role::Tab, "Register");
    click(
        &mut harness,
        Role::CheckBox,
        "Fill in a new transaction from its payee's last one",
    );
    click(&mut harness, Role::Button, "OK");
    // The Dialog took the cell cursor; Ctrl+N puts it back on the draft row.
    press(&mut harness, Modifiers::COMMAND, Key::N);
    type_payee(&mut harness, "Trattoria Roma");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        ["", "", "Trattoria Roma", "", "", "", ""]
    );
}

#[test]
fn a_transfer_between_two_other_accounts_fills_the_account_it_went_to() {
    let mut ledger = Ledger::stub();
    transfer(&mut ledger, "Joint Checking", "Emergency Fund", "-200.00");
    let mut harness = everyday_checking_on(ledger);
    type_payee(&mut harness, "Saving up");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Saving up",
            "[Emergency Fund]",
            "Monthly top-up",
            "200.00",
            "",
        ]
    );
}

#[test]
fn a_transfer_from_this_account_fills_the_account_it_went_to() {
    let mut ledger = Ledger::stub();
    transfer(
        &mut ledger,
        "Everyday Checking",
        "Emergency Fund",
        "-200.00",
    );
    let mut harness = everyday_checking_on(ledger);
    type_payee(&mut harness, "Saving up");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Saving up",
            "[Emergency Fund]",
            "Monthly top-up",
            "200.00",
            "",
        ]
    );
}

#[test]
fn a_transfer_into_this_account_fills_the_account_it_came_from_and_flips_the_amount() {
    let mut ledger = Ledger::stub();
    transfer(
        &mut ledger,
        "Joint Checking",
        "Everyday Checking",
        "-200.00",
    );
    let mut harness = everyday_checking_on(ledger);
    type_payee(&mut harness, "Saving up");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Saving up",
            "[Joint Checking]",
            "Monthly top-up",
            "",
            "200.00",
        ]
    );
}

#[test]
fn a_transfer_to_an_account_now_closed_fills_memo_and_amount_but_no_category() {
    let mut ledger = Ledger::stub();
    transfer(&mut ledger, "Joint Checking", "Emergency Fund", "-200.00");
    close(&mut ledger, "Emergency Fund");
    let mut harness = everyday_checking_on(ledger);
    type_payee(&mut harness, "Saving up");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        ["", "", "Saving up", "", "Monthly top-up", "200.00", ""]
    );
}

#[test]
fn a_transfer_into_this_account_from_one_now_closed_fills_memo_and_amount_but_no_category() {
    let mut ledger = Ledger::stub();
    transfer(
        &mut ledger,
        "Emergency Fund",
        "Everyday Checking",
        "-200.00",
    );
    close(&mut ledger, "Emergency Fund");
    let mut harness = everyday_checking_on(ledger);
    type_payee(&mut harness, "Saving up");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        ["", "", "Saving up", "", "Monthly top-up", "", "200.00"]
    );
}

#[test]
fn a_split_fills_memo_and_its_total_but_no_category() {
    let mut ledger = Ledger::stub();
    // Its line to Emergency Fund puts a transfer with the same payee there too.
    let transaction = NewTransaction::split(
        Date::try_new_iso(2026, 10, 1).expect("1 October 2026 is a date"),
        "Warehouse Club",
        vec![
            NewSplitLine::category(category(&ledger, "Groceries"), "", usd("-60.00")),
            NewSplitLine::transfer(account(&ledger, "Emergency Fund"), "", usd("-40.00")),
        ],
    )
    .memo("Monthly stock-up");
    ledger
        .apply(DomainCommand::PostTransaction {
            account: account(&ledger, "Joint Checking"),
            transaction,
        })
        .expect("the stub takes the split");
    let mut harness = everyday_checking_on(ledger);
    type_payee(&mut harness, "Warehouse Club");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Warehouse Club",
            "",
            "Monthly stock-up",
            "100.00",
            ""
        ]
    );
}

#[test]
fn a_category_typed_before_a_transfers_payee_is_kept() {
    let mut ledger = Ledger::stub();
    transfer(&mut ledger, "Joint Checking", "Emergency Fund", "-200.00");
    let mut harness = everyday_checking_on(ledger);
    // Date, Num, Payee, Category.
    for _ in 0..3 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
    }
    type_text(&mut harness, "Shopping");
    press(&mut harness, Modifiers::SHIFT, Key::Tab);
    type_text(&mut harness, "Saving up");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Saving up",
            "Shopping",
            "Monthly top-up",
            "200.00",
            "",
        ]
    );
}

#[test]
fn a_payee_only_the_far_side_of_a_transfer_has_recalls_that_side() {
    let mut ledger = Ledger::stub();
    transfer(&mut ledger, "Joint Checking", "Emergency Fund", "-200.00");
    rename_saving_up_in(&mut ledger, "Emergency Fund", "Rainy day");
    let mut harness = everyday_checking_on(ledger);
    type_payee(&mut harness, "Rainy day");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(
        draft_row(&harness),
        [
            "",
            "",
            "Rainy day",
            "[Joint Checking]",
            "Monthly top-up",
            "",
            "200.00",
        ]
    );
}

#[test]
fn with_payee_recall_off_a_transfers_payee_fills_nothing() {
    let dir = tempfile::tempdir().expect("the test machine can make a temporary directory");
    std::fs::write(
        dir.path().join("preferences.toml"),
        "version = 2\npayee_recall = false\n",
    )
    .expect("the test can write the preferences file");
    let mut ledger = Ledger::stub();
    transfer(&mut ledger, "Joint Checking", "Emergency Fund", "-200.00");
    let mut harness = app_with(ledger, PreferencesFile::in_dir(dir.path()));
    open_everyday_checking(&mut harness);
    type_payee(&mut harness, "Saving up");

    press(&mut harness, Modifiers::NONE, Key::Tab);

    assert_eq!(draft_row(&harness), ["", "", "Saving up", "", "", "", ""]);
}

#[test]
fn another_rows_flag_cell_recalls_nothing_until_the_payee_is_committed() {
    let mut harness = everyday_checking();
    type_payee(&mut harness, "Maple Property Management");

    click(
        &mut harness,
        Role::CheckBox,
        "Flag Shell, 9/15/2026, -48.20",
    );

    // The Payee cell is still open, so nothing is recalled yet.
    assert_eq!(draft_row(&harness), ["", "", "", "", "", "", ""]);
    let payee = harness.get_by_role_and_label(Role::TextInput, "Payee");
    assert_eq!(
        payee.accesskit_node().value().as_deref(),
        Some("Maple Property Management")
    );
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
