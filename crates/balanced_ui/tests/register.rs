//! The Register, driven through the AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::App;
use eframe::egui::accesskit::{Action, Role, SortDirection, Toggled};
use eframe::egui::{CursorIcon, vec2};
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

/// Clicks the Register header `title`.
fn sort_by(harness: &mut Harness<'static, App>, title: &str) {
    harness
        .get_by_role_and_label(Role::ColumnHeader, title)
        .click();
    harness.run();
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

#[test]
fn the_hovered_row_shows_its_actions_menu() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    harness.get_by_role_and_label(Role::Cell, "Costco").hover();
    harness.run();

    let menus = harness
        .query_all_by_role_and_label(Role::Button, "Actions")
        .count();
    assert_eq!(menus, 1);
}

#[test]
fn only_the_rows_in_view_of_a_large_register_are_laid_out() {
    let mut harness = app();

    open(&mut harness, "Household Checking");

    // The account has 20,001 transactions, of eight cells each; the window shows a few
    // dozen rows.
    let rows = harness.query_all_by_role(Role::Cell).count() / 8;
    assert!((1..=50).contains(&rows), "{rows} rows laid out");
}

#[test]
fn opening_another_account_clears_the_selection() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    harness.get_by_role_and_label(Role::Cell, "Costco").click();
    harness.run();

    open(&mut harness, "Joint Checking");
    open(&mut harness, "Everyday Checking");

    let costco = harness.get_by_role_and_label(Role::Cell, "Costco");
    assert_eq!(costco.accesskit_node().toggled(), Some(Toggled::False));
}

#[test]
fn clicking_payee_sorts_by_payee_and_blanks_the_balance() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    sort_by(&mut harness, "Payee");

    assert_eq!(
        column(&harness, "Payee"),
        [
            "Acme Corp",
            "Acme Corp",
            "Bean There",
            "City Power & Light",
            "Costco",
            "Fresh Market",
            "Maple Property Management",
            "Opening Balance",
            "Shell",
            "Trattoria Roma",
        ]
    );
    assert_eq!(column(&harness, "Balance"), [""; 10]);
}

#[test]
fn clicking_payee_again_reverses_the_sort_but_not_its_ties() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    sort_by(&mut harness, "Payee");

    sort_by(&mut harness, "Payee");

    let payees = column(&harness, "Payee");
    let dates = column(&harness, "Date");
    assert_eq!(
        payees,
        [
            "Trattoria Roma",
            "Shell",
            "Opening Balance",
            "Maple Property Management",
            "Fresh Market",
            "Costco",
            "City Power & Light",
            "Bean There",
            "Acme Corp",
            "Acme Corp",
        ]
    );
    // The two Acme Corp salaries tie, so they stay oldest first.
    assert_eq!(dates[8..], ["9/1/2026", "9/18/2026"]);
}

#[test]
fn sorting_by_date_again_brings_the_balances_back() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    sort_by(&mut harness, "Payee");

    sort_by(&mut harness, "Date");

    assert_eq!(
        column(&harness, "Balance"),
        [
            "1,200.00", "4,400.00", "2,950.00", "2,863.58", "2,858.83", "2,766.73", "2,560.46",
            "2,512.26", "5,712.26", "5,647.46",
        ]
    );
}

#[test]
fn a_register_opens_sorted_by_date_with_every_column_but_number_and_balance_sortable() {
    let mut harness = app();

    open(&mut harness, "Everyday Checking");

    let header = |title| harness.get_by_role_and_label(Role::ColumnHeader, title);
    let sorts = |title| {
        let node = header(title).accesskit_node();
        (
            node.data().supports_action(Action::Click),
            node.sort_direction(),
        )
    };
    assert_eq!(sorts("Date"), (true, Some(SortDirection::Ascending)));
    for title in ["Payee", "Category", "Memo", "Amount"] {
        assert_eq!(sorts(title), (true, None), "{title}");
    }
    for title in ["Num", "Balance"] {
        assert_eq!(sorts(title), (false, None), "{title}");
    }
}

#[test]
fn a_sortable_header_shows_a_pointing_hand() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    harness
        .get_by_role_and_label(Role::ColumnHeader, "Payee")
        .hover();
    harness.run();

    assert_eq!(
        harness.output().platform_output.cursor_icon,
        CursorIcon::PointingHand
    );
}

#[test]
fn the_selected_row_stays_centred_after_a_sort() {
    let mut harness = app();
    open(&mut harness, "Household Checking");
    harness
        .get_by_role_and_label(Role::Cell, "Opening Balance")
        .click();
    harness.run();

    // Of 20,001 rows, the opening balance sorts thousands of rows down by payee.
    sort_by(&mut harness, "Payee");

    let header = harness
        .get_by_role_and_label(Role::ColumnHeader, "Payee")
        .rect();
    let bottom = harness
        .query_all_by_role(Role::Cell)
        .map(|cell| cell.rect().bottom())
        .fold(header.bottom(), f32::max);
    let middle = header.bottom().midpoint(bottom);
    let selected = harness
        .query_by_role_and_label(Role::Cell, "Opening Balance")
        .map(|cell| cell.rect().center().y);
    assert!(
        selected.is_some_and(|y| (y - middle).abs() <= header.height()),
        "the selected row is at {selected:?}, the middle at {middle}"
    );
}
