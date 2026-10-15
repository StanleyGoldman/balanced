//! The Register, driven through the AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::App;
use eframe::egui::accesskit::{Role, Toggled};
use eframe::egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

/// The App on stub data, in the test locale, wide enough to show every Register column.
fn app() -> Harness<'static, App> {
    Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(|cc| App::new(cc, Localizer::for_tests(), Ledger::stub()))
}

/// Opens the account named `name` from the Account bar.
fn open(harness: &mut Harness<'static, App>, name: &str) {
    harness.get_by_role_and_label(Role::Button, name).click();
    harness.run();
}

/// The labels of every Register cell, top row first, each row left to right.
fn cells(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::Cell)
        .map(|cell| cell.accesskit_node().label().unwrap_or_default())
        .collect()
}

#[test]
fn a_transaction_shows_each_column_with_its_running_balance() {
    let mut harness = app();

    open(&mut harness, "Everyday Checking");

    let cells = cells(&harness);
    let payee = cells
        .iter()
        .position(|cell| cell == "Fresh Market")
        .expect("the Register lists Fresh Market");
    // Date, Num, Payee, Category, Memo, Amount, Balance.
    assert_eq!(
        cells[payee - 2..payee + 5],
        [
            "9/5/2026",
            "",
            "Fresh Market",
            "Food & Dining:Groceries",
            "",
            "-86.42",
            "2,863.58",
        ]
    );
}

#[test]
fn a_split_transaction_shows_split_in_its_category() {
    let mut harness = app();

    open(&mut harness, "Everyday Checking");

    let cells = cells(&harness);
    let payee = cells
        .iter()
        .position(|cell| cell == "Costco")
        .expect("the Register lists Costco");
    assert_eq!(
        cells[payee..payee + 4],
        ["Costco", "[Split]", "", "-206.27"]
    );
}

#[test]
fn the_opening_balance_is_the_first_transaction() {
    let mut harness = app();

    open(&mut harness, "Everyday Checking");

    let cells = cells(&harness);
    assert_eq!(
        cells[..7],
        [
            "1/1/2026",
            "",
            "Opening Balance",
            "",
            "",
            "1,200.00",
            "1,200.00",
        ]
    );
}

#[test]
fn clicking_a_transaction_selects_its_row() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    harness.get_by_role_and_label(Role::Cell, "Costco").click();
    harness.run();

    let costco = harness.get_by_role_and_label(Role::Cell, "Costco");
    let fresh_market = harness.get_by_role_and_label(Role::Cell, "Fresh Market");
    assert_eq!(costco.accesskit_node().toggled(), Some(Toggled::True));
    assert_eq!(
        fresh_market.accesskit_node().toggled(),
        Some(Toggled::False)
    );
}

#[test]
fn the_selected_row_shows_its_actions_menu() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    let menus_before = harness
        .query_all_by_role_and_label(Role::Button, "Actions")
        .count();

    harness.get_by_role_and_label(Role::Cell, "Costco").click();
    harness.run();

    let menus_after = harness
        .query_all_by_role_and_label(Role::Button, "Actions")
        .count();
    assert_eq!((menus_before, menus_after), (0, 1));
}

#[test]
fn an_open_account_offers_a_new_transaction() {
    let mut harness = app();

    open(&mut harness, "Everyday Checking");

    let add = harness.get_by_role_and_label(Role::Button, "+ Transaction");
    assert!(!add.accesskit_node().is_disabled());
    assert!(harness.query_by_label("Closed — read-only").is_none());
}

#[test]
fn a_closed_account_is_read_only() {
    let mut harness = app();
    open(&mut harness, "Closed");

    open(&mut harness, "Old Checking");

    let add = harness.get_by_role_and_label(Role::Button, "+ Transaction");
    assert!(add.accesskit_node().is_disabled());
    harness.get_by_label("Closed — read-only");
}
