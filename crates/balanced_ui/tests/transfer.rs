// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Transfers in the Register: `[Account]` in the Category combobox, and the transfer
//! rows it posts.

use balanced_core::{
    AccountId, AccountStatus, CurrencyCode, DomainCommand, Ledger, Money, NewTransaction,
};
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::{Role, Toggled};
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on `ledger`, in the test locale, wide enough to show every Register column.
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

/// The App on stub data, with Everyday Checking open.
fn everyday() -> Harness<'static, App> {
    let mut harness = app_on(Ledger::stub());
    open(&mut harness, "Everyday Checking");
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

/// Types `text` in one go into whatever has the keyboard.
fn type_text(harness: &mut Harness<'static, App>, text: &str) {
    send(harness, Event::Text(text.to_owned()));
    harness.run();
}

/// Fills the draft row's Date, Payee and Category cells, from the Date cell under the
/// cursor, leaving the Category cell's editor open.
fn draft_to_category(harness: &mut Harness<'static, App>, payee: &str, category: &str) {
    for c in "10/15/2026".chars() {
        send(harness, Event::Text(c.to_string()));
        harness.run();
    }
    // Date, Num, Payee.
    press(harness, Modifiers::NONE, Key::Tab);
    press(harness, Modifiers::NONE, Key::Tab);
    type_text(harness, payee);
    press(harness, Modifiers::NONE, Key::Tab);
    type_text(harness, category);
}

/// Adds a payment of `payment` to `category` from the draft row.
fn add_transaction(
    harness: &mut Harness<'static, App>,
    payee: &str,
    category: &str,
    payment: &str,
) {
    draft_to_category(harness, payee, category);
    // Category, Memo, Payment.
    press(harness, Modifiers::NONE, Key::Tab);
    press(harness, Modifiers::NONE, Key::Tab);
    type_text(harness, payment);
    press(harness, Modifiers::COMMAND, Key::Enter);
}

/// The labels of the rows of the open combobox list, top first.
fn list_rows(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::ListBoxOption)
        .map(|row| row.accesskit_node().label().unwrap_or_default())
        .collect()
}

/// The labels of the Register cells under the header `title`, top row first. The last
/// is the draft row's, on an account that can change.
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

/// The cells of the row whose payee is `payee`, from Date to Balance.
fn row_of(harness: &Harness<'static, App>, payee: &str) -> Vec<String> {
    let cells: Vec<String> = harness
        .query_all_by_role(Role::Cell)
        .map(|cell| cell.accesskit_node().label().unwrap_or_default())
        .collect();
    let payee = cells
        .iter()
        .position(|cell| cell == payee)
        .expect("the Register lists the payee");
    cells[payee - 2..payee + 6].to_vec()
}

/// The text the Status bar shows.
fn status(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .get_by_role(Role::Status)
        .query_all_by_role(Role::Label)
        .filter_map(|label| label.accesskit_node().value())
        .collect()
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

/// The stub ledger with a transfer of 200 from Everyday Checking to the account named
/// `to`, on 1 October 2026, paid to "Saving up".
fn with_transfer(to: &str) -> Ledger {
    with_transfer_on(
        "Everyday Checking",
        to,
        Date::try_new_iso(2026, 10, 1).expect("1 October 2026 is a date"),
    )
}

/// The stub ledger with a transfer of 200 from the account named `from` to the one
/// named `to`, on `date`, paid to "Saving up".
fn with_transfer_on(from: &str, to: &str, date: Date<icu_calendar::Iso>) -> Ledger {
    let mut ledger = Ledger::stub();
    let from = account(&ledger, from);
    let to = account(&ledger, to);
    let usd = CurrencyCode::new("USD").expect("USD is an ISO 4217 currency");
    let amount = Money::new("-200.00".parse().expect("a decimal"), usd).expect("an amount");
    ledger
        .apply(DomainCommand::PostTransaction {
            account: from,
            transaction: NewTransaction::transfer(date, "Saving up", to, amount),
        })
        .expect("the stub takes the transfer");
    ledger
}

/// Selects the "Saving up" row by clicking its Payee cell.
fn select_saving_up(harness: &mut Harness<'static, App>) {
    harness
        .get_by_role_and_label(Role::Cell, "Saving up")
        .click();
    harness.run();
}

/// Selects the "Saving up" row and picks `item` from its Actions menu.
fn pick_action(harness: &mut Harness<'static, App>, item: &str) {
    select_saving_up(harness);
    harness
        .get_by_role_and_label(Role::Button, "Actions")
        .click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, item).click();
    harness.run();
}

/// Whether the "Saving up" row holds the cell cursor.
fn saving_up_is_selected(harness: &Harness<'static, App>) -> bool {
    harness
        .get_by_role_and_label(Role::Cell, "Saving up")
        .accesskit_node()
        .toggled()
        == Some(Toggled::True)
}

#[test]
fn a_bracket_lists_every_open_account_but_this_one_hidden_ones_included() {
    let mut harness = everyday();

    draft_to_category(&mut harness, "Saving up", "[");

    // The list shows as many rows as fit, so its Create row is out of view.
    assert_eq!(
        list_rows(&harness),
        [
            "[401(k)]",
            "[Brokerage]",
            "[Car]",
            "[Emergency Fund]",
            "[Holiday Fund]",
            "[House]",
            "[Household Checking]",
            "[Joint Checking]",
            "[Mortgage]",
            "[Roth IRA]",
            "[Visa Rewards]",
        ]
    );
}

#[test]
fn the_accounts_come_after_the_categories() {
    let mut harness = everyday();

    draft_to_category(&mut harness, "Saving up", "house");

    assert_eq!(
        list_rows(&harness),
        [
            "Shopping:Household",
            "[House]",
            "[Household Checking]",
            "Create \"house\"…",
        ]
    );
}

#[test]
fn typing_an_account_s_bracketed_start_narrows_the_list_to_it() {
    let mut harness = everyday();

    draft_to_category(&mut harness, "Saving up", "[Emer");

    assert_eq!(
        list_rows(&harness).first().map(String::as_str),
        Some("[Emergency Fund]")
    );
}

#[test]
fn a_transfer_from_the_draft_row_shows_the_other_account_on_both_sides() {
    let mut harness = everyday();

    add_transaction(&mut harness, "Saving up", "[Emergency Fund]", "200");

    assert_eq!(
        row_of(&harness, "Saving up")[..6],
        [
            "10/15/2026",
            "",
            "Saving up",
            "[Emergency Fund]",
            "",
            "200.00"
        ]
    );
    open(&mut harness, "Emergency Fund");
    assert_eq!(
        row_of(&harness, "Saving up")[..7],
        [
            "10/15/2026",
            "",
            "Saving up",
            "[Everyday Checking]",
            "",
            "",
            "200.00"
        ]
    );
}

#[test]
fn searching_for_the_other_account_finds_the_transfer() {
    let mut harness = everyday();
    add_transaction(&mut harness, "Saving up", "[Emergency Fund]", "200");
    open(&mut harness, "Emergency Fund");

    harness
        .get_by_role_and_label(Role::TextInput, "Search")
        .focus();
    harness.run();
    type_text(&mut harness, "[everyday");

    assert_eq!(column(&harness, "Payee"), ["Saving up", ""]);
}

#[test]
fn deleting_a_transfer_whose_other_account_is_closed_says_why_it_was_refused() {
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
    let mut harness = app_on(ledger);
    open(&mut harness, "Everyday Checking");
    harness
        .get_by_role_and_label(Role::Cell, "Saving up")
        .click();
    harness.run();

    press(&mut harness, Modifiers::COMMAND, Key::D);
    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(
        status(&harness),
        [
            "The other account of this transfer is closed, so the transfer can't be made, changed or deleted."
        ]
    );
    assert_eq!(
        column(&harness, "Payee")
            .iter()
            .filter(|payee| *payee == "Saving up")
            .count(),
        1
    );
}

/// Clicks the Register cell labelled `label`, which opens its editor.
fn click_cell(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::Cell, label).click();
    harness.run();
}

/// Retypes the Category cell labelled `old` as `new`, and saves it with Enter.
fn change_category(harness: &mut Harness<'static, App>, old: &str, new: &str) {
    click_cell(harness, old);
    press(harness, Modifiers::COMMAND, Key::A);
    type_text(harness, new);
    press(harness, Modifiers::NONE, Key::Enter);
}

#[test]
fn picking_an_account_in_a_category_cell_makes_the_row_a_transfer() {
    let mut harness = everyday();

    change_category(&mut harness, "Auto & Transport:Fuel", "[Emergency Fund]");

    assert_eq!(row_of(&harness, "Shell")[3], "[Emergency Fund]");
    open(&mut harness, "Emergency Fund");
    assert_eq!(row_of(&harness, "Shell")[3], "[Everyday Checking]");
}

#[test]
fn picking_another_account_in_a_transfer_s_category_cell_moves_its_other_side() {
    let mut harness = everyday();
    add_transaction(&mut harness, "Saving up", "[Emergency Fund]", "200");

    change_category(&mut harness, "[Emergency Fund]", "[Joint Checking]");

    assert_eq!(row_of(&harness, "Saving up")[3], "[Joint Checking]");
    open(&mut harness, "Joint Checking");
    assert_eq!(row_of(&harness, "Saving up")[3], "[Everyday Checking]");
    open(&mut harness, "Emergency Fund");
    assert_eq!(column(&harness, "Payee"), ["Opening Balance", ""]);
}

#[test]
fn picking_a_category_in_a_transfer_s_category_cell_deletes_its_other_side() {
    let mut harness = everyday();
    add_transaction(&mut harness, "Saving up", "[Emergency Fund]", "200");

    change_category(&mut harness, "[Emergency Fund]", "Food & Dining:Groceries");

    assert_eq!(row_of(&harness, "Saving up")[3], "Food & Dining:Groceries");
    open(&mut harness, "Emergency Fund");
    assert_eq!(column(&harness, "Payee"), ["Opening Balance", ""]);
}

#[test]
fn a_transfer_s_date_and_amount_changed_in_its_row_change_its_other_side_on_save() {
    let mut harness = app_on(with_transfer("Emergency Fund"));
    open(&mut harness, "Everyday Checking");
    click_cell(&mut harness, "Saving up");
    press(&mut harness, Modifiers::NONE, Key::ArrowLeft);
    press(&mut harness, Modifiers::NONE, Key::ArrowLeft);
    // A day later.
    type_text(&mut harness, "+");
    // Date, Num, Payee, Category, Memo, Payment.
    for _ in 0..5 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
    }
    press(&mut harness, Modifiers::COMMAND, Key::A);
    type_text(&mut harness, "250");

    press(&mut harness, Modifiers::NONE, Key::Enter);
    open(&mut harness, "Emergency Fund");

    let other_side = row_of(&harness, "Saving up");
    assert_eq!(
        [other_side[0].as_str(), other_side[6].as_str()],
        ["10/2/2026", "250.00"]
    );
}

#[test]
fn go_to_opens_the_other_account_on_the_other_side() {
    let mut harness = app_on(with_transfer("Emergency Fund"));
    open(&mut harness, "Everyday Checking");

    pick_action(&mut harness, "Go to [Emergency Fund]");

    assert_eq!(row_of(&harness, "Saving up")[3], "[Everyday Checking]");
    assert!(saving_up_is_selected(&harness));
}

#[test]
fn go_to_from_the_other_side_comes_back() {
    let mut harness = app_on(with_transfer("Emergency Fund"));
    open(&mut harness, "Emergency Fund");

    pick_action(&mut harness, "Go to [Everyday Checking]");

    assert_eq!(row_of(&harness, "Saving up")[3], "[Emergency Fund]");
    assert!(saving_up_is_selected(&harness));
}

#[test]
fn go_to_opens_a_hidden_account() {
    let mut harness = app_on(with_transfer("Holiday Fund"));
    open(&mut harness, "Everyday Checking");

    pick_action(&mut harness, "Go to [Holiday Fund]");

    assert_eq!(row_of(&harness, "Saving up")[3], "[Everyday Checking]");
    assert!(saving_up_is_selected(&harness));
}

#[test]
fn go_to_works_from_the_keyboard() {
    let mut harness = app_on(with_transfer("Emergency Fund"));
    open(&mut harness, "Everyday Checking");
    select_saving_up(&mut harness);

    harness
        .get_by_role_and_label(Role::Button, "Actions")
        .focus();
    harness.run();
    press(&mut harness, Modifiers::NONE, Key::Enter);
    harness
        .get_by_role_and_label(Role::Button, "Go to [Emergency Fund]")
        .focus();
    harness.run();
    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(row_of(&harness, "Saving up")[3], "[Everyday Checking]");
    assert!(saving_up_is_selected(&harness));
}

#[test]
fn a_row_that_is_no_transfer_has_no_go_to() {
    let mut harness = app_on(with_transfer("Emergency Fund"));
    open(&mut harness, "Everyday Checking");
    harness.get_by_role_and_label(Role::Cell, "Costco").click();
    harness.run();

    harness
        .get_by_role_and_label(Role::Button, "Actions")
        .click();
    harness.run();

    let go_to: Vec<String> = harness
        .query_all_by_role(Role::Button)
        .filter_map(|button| button.accesskit_node().label())
        .filter(|label| label.starts_with("Go to"))
        .collect();
    assert_eq!(go_to, Vec::<String>::new());
}

#[test]
fn keys_reach_the_register_go_to_opened() {
    let mut harness = app_on(with_transfer("Emergency Fund"));
    open(&mut harness, "Everyday Checking");
    pick_action(&mut harness, "Go to [Emergency Fund]");

    press(&mut harness, Modifiers::NONE, Key::ArrowDown);
    let moved_off = !saving_up_is_selected(&harness);
    press(&mut harness, Modifiers::NONE, Key::ArrowUp);

    assert!(moved_off);
    assert!(saving_up_is_selected(&harness));
}

#[test]
fn go_to_scrolls_a_far_off_other_side_into_view() {
    let mut harness = app_on(with_transfer_on(
        "Emergency Fund",
        "Household Checking",
        // Halfway through its 20,000 rows, far from the end the Register opens on.
        Date::try_new_iso(2020, 1, 1).expect("1 January 2020 is a date"),
    ));
    open(&mut harness, "Emergency Fund");

    pick_action(&mut harness, "Go to [Household Checking]");

    // The Register lays out only the rows in view.
    let in_view = harness
        .query_by_role_and_label(Role::Cell, "Saving up")
        .is_some();
    assert!(in_view);
    assert!(saving_up_is_selected(&harness));
}

#[test]
fn ctrl_g_leaves_the_selected_transfer_where_it_is() {
    let mut harness = app_on(with_transfer("Emergency Fund"));
    open(&mut harness, "Everyday Checking");
    select_saving_up(&mut harness);

    press(&mut harness, Modifiers::COMMAND, Key::G);
    // Ctrl+G opens Go to date; Esc closes it.
    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert_eq!(row_of(&harness, "Saving up")[3], "[Emergency Fund]");
}

#[test]
fn the_edit_menu_has_no_go_to_transfer() {
    let mut harness = app_on(with_transfer("Emergency Fund"));
    open(&mut harness, "Everyday Checking");
    select_saving_up(&mut harness);

    harness.get_by_role_and_label(Role::Button, "Edit").click();
    harness.run();

    let go_to: Vec<String> = harness
        .query_all_by_role(Role::Button)
        .filter_map(|button| button.accesskit_node().label())
        .filter(|label| label.starts_with("Go to"))
        .collect();
    assert_eq!(go_to, ["Go to Date… Ctrl+G"]);
}

#[test]
fn go_to_in_a_transfer_s_category_cell_menu_goes_to_its_other_side() {
    let mut harness = app_on(with_transfer("Emergency Fund"));
    open(&mut harness, "Everyday Checking");

    harness
        .get_by_role_and_label(Role::Cell, "[Emergency Fund]")
        .click_secondary();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Go to [Emergency Fund]")
        .click();
    harness.run();

    assert_eq!(row_of(&harness, "Saving up")[3], "[Everyday Checking]");
    assert!(saving_up_is_selected(&harness));
}

/// Renames "Saving up" to "Rainy day" in its Payee cell, leaving the edit open.
fn rename_saving_up(harness: &mut Harness<'static, App>) {
    select_saving_up(harness);
    type_text(harness, "Rainy day");
}

/// Picks `item` from the editing row's More actions menu.
fn pick_more(harness: &mut Harness<'static, App>, item: &str) {
    for label in ["More actions", item] {
        harness.get_by_role_and_label(Role::Button, label).click();
        harness.run();
    }
}

/// Whether the leave-row prompt is open.
fn prompted(harness: &Harness<'static, App>) -> bool {
    harness
        .query_by_role_and_label(Role::Button, "Continue editing")
        .is_some()
}

#[test]
fn go_to_from_more_on_a_changed_row_asks_first() {
    let mut harness = app_on(with_transfer("Emergency Fund"));
    open(&mut harness, "Everyday Checking");
    rename_saving_up(&mut harness);

    pick_more(&mut harness, "Go to [Emergency Fund]");

    assert!(prompted(&harness));
    assert_eq!(row_of(&harness, "Rainy day")[3], "[Emergency Fund]");
}

#[test]
fn continue_editing_after_go_to_from_more_stays_on_the_row() {
    let mut harness = app_on(with_transfer("Emergency Fund"));
    open(&mut harness, "Everyday Checking");
    rename_saving_up(&mut harness);
    pick_more(&mut harness, "Go to [Emergency Fund]");

    harness
        .get_by_role_and_label(Role::Button, "Continue editing")
        .click();
    harness.run();

    assert!(!prompted(&harness));
    assert_eq!(row_of(&harness, "Rainy day")[3], "[Emergency Fund]");
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Payee")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Rainy day")
    );
}

#[test]
fn save_after_go_to_from_more_saves_the_row_then_goes() {
    let mut harness = app_on(with_transfer("Emergency Fund"));
    open(&mut harness, "Everyday Checking");
    rename_saving_up(&mut harness);
    pick_more(&mut harness, "Go to [Emergency Fund]");

    // The prompt's Save, not the one in the editing row's toolbar behind it.
    harness
        .query_all_by_role_and_label(Role::Button, "Save")
        .find(|button| {
            button
                .accesskit_node()
                .parent()
                .is_none_or(|parent| parent.role() != Role::Toolbar)
        })
        .expect("the prompt has Save")
        .click();
    harness.run();

    assert!(!prompted(&harness));
    // The other side, in Emergency Fund, keeps its payee.
    assert!(
        harness
            .query_by_role_and_label(Role::Cell, "Saving up")
            .is_some()
    );
    assert_eq!(row_of(&harness, "Saving up")[3], "[Everyday Checking]");
    open(&mut harness, "Everyday Checking");
    assert_eq!(row_of(&harness, "Rainy day")[3], "[Emergency Fund]");
}

#[test]
fn go_to_from_more_on_an_unchanged_row_goes_at_once() {
    let mut harness = app_on(with_transfer("Emergency Fund"));
    open(&mut harness, "Everyday Checking");
    select_saving_up(&mut harness);
    // Opens the Payee cell, unchanged.
    press(&mut harness, Modifiers::NONE, Key::Enter);

    pick_more(&mut harness, "Go to [Emergency Fund]");

    assert!(!prompted(&harness));
    assert_eq!(row_of(&harness, "Saving up")[3], "[Everyday Checking]");
}
