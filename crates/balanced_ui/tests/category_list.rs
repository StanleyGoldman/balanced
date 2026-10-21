// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Reading the category tree and its usage in the Category List, driven through the
//! AccessKit tree the way a user drives it.

use balanced_core::{
    AccountId, AccountSummary, CurrencyCode, DomainCommand, Ledger, Money, NewTransaction,
};
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::egui::accesskit::Action;
use eframe::egui::accesskit::Role;
use eframe::egui::style::ScrollStyle;
use eframe::egui::{Event, Key, Modifiers, Vec2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on `ledger`, in the test locale.
fn app_on(ledger: Ledger) -> Harness<'static, App> {
    app_in(ledger, vec2(1280.0, 800.0))
}

/// The App on `ledger`, in the test locale, in a window of `size`.
fn app_in(ledger: Ledger, size: Vec2) -> Harness<'static, App> {
    let mut harness = Harness::builder().with_size(size).build_eframe(|cc| {
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

/// Opens the Category List from the Tools menu.
fn open_category_list(harness: &mut Harness<'static, App>) {
    harness.get_by_role_and_label(Role::Button, "Tools").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Category List…")
        .click();
    harness.run();
}

/// The App on stub data, with the Category List open.
fn category_list() -> Harness<'static, App> {
    let mut harness = app_on(Ledger::stub());
    open_category_list(&mut harness);
    harness
}

/// The id of the stub account named `name`.
fn account(ledger: &Ledger, name: &str) -> AccountId {
    ledger
        .snapshot()
        .accounts()
        .iter()
        .map(AccountSummary::account)
        .find(|account| account.name() == name)
        .expect("the stub has the account")
        .id()
}

/// Whether the Category List is open.
fn is_open(harness: &Harness<'static, App>) -> bool {
    harness
        .query_by_role_and_label(Role::Window, "Category List")
        .is_some()
}

/// Each row the Category List lays out, top to bottom, as a screen reader reads it:
/// its level in the tree, counting from 1, and its name.
fn tree(harness: &Harness<'static, App>) -> Vec<(usize, String)> {
    harness
        .query_all_by_role(Role::TreeItem)
        .map(|row| {
            let node = row.accesskit_node();
            (
                node.level().unwrap_or_default(),
                node.label().unwrap_or_default(),
            )
        })
        .collect()
}

/// What a screen reader reads after the name of the row named `name`: how many
/// transactions and split lines use it, and their total.
fn usage(harness: &Harness<'static, App>, name: &str) -> String {
    harness
        .get_by_role_and_label(Role::TreeItem, name)
        .accesskit_node()
        .description()
        .unwrap_or_default()
}

/// Whether a label in the Category List reads `text`.
fn shows(harness: &Harness<'static, App>, text: &str) -> bool {
    harness
        .query_all_by_role(Role::Label)
        .any(|label| label.accesskit_node().value().as_deref() == Some(text))
}

/// Whether the row named `name` shows the categories under it, as a screen reader
/// reads it.
fn expanded(harness: &Harness<'static, App>, name: &str) -> Option<bool> {
    harness
        .get_by_role_and_label(Role::TreeItem, name)
        .accesskit_node()
        .data()
        .is_expanded()
}

/// The names of the rows the Category List lays out, top to bottom, down to the first
/// transfer row.
fn categories(harness: &Harness<'static, App>) -> Vec<String> {
    tree(harness)
        .into_iter()
        .map(|(_, name)| name)
        .take_while(|name| !name.starts_with('['))
        .collect()
}

/// The name of the selected row, if any.
fn selected(harness: &Harness<'static, App>) -> Option<String> {
    harness
        .query_all_by_role(Role::TreeItem)
        .find(|row| row.accesskit_node().is_selected() == Some(true))
        .and_then(|row| row.accesskit_node().label())
}

/// The name of the row with keyboard focus, if any.
fn focused(harness: &Harness<'static, App>) -> Option<String> {
    harness
        .query_all_by_role(Role::TreeItem)
        .find(egui_kittest::Node::is_focused)
        .and_then(|row| row.accesskit_node().label())
}

/// Clicks the row named `name`, then runs the App until it settles.
fn click_row(harness: &mut Harness<'static, App>, name: &str) {
    harness.get_by_role_and_label(Role::TreeItem, name).click();
    harness.run();
}

/// Clicks the button labelled `label`, then runs the App until it settles.
fn click(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run();
}

/// Presses `key` with no modifiers, then runs the App until it settles.
fn press(harness: &mut Harness<'static, App>, key: Key) {
    harness.event(Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    harness.run();
}

#[test]
fn tools_category_list_opens_the_dialog_and_close_dismisses_it() {
    let mut harness = category_list();
    let opened = is_open(&harness);

    harness.get_by_role_and_label(Role::Button, "Close").click();
    harness.run();

    assert!(opened);
    assert!(!is_open(&harness));
}

#[test]
fn escape_dismisses_the_category_list() {
    let mut harness = category_list();

    press(&mut harness, Key::Escape);

    assert!(!is_open(&harness));
}

#[test]
fn the_tree_shows_each_category_under_its_parent_siblings_by_name() {
    let harness = category_list();

    let tree = tree(&harness);

    let expected = [
        (1, "Auto & Transport"),
        (2, "Fuel"),
        (2, "Insurance"),
        (1, "Food & Dining"),
        (2, "Groceries"),
        (2, "Restaurants"),
        (3, "Coffee Shops"),
        (1, "Home"),
        (2, "Insurance"),
        (2, "Utilities"),
        (1, "Income"),
        (2, "Interest"),
        (2, "Salary"),
        (1, "Shopping"),
        (2, "Household"),
    ]
    .map(|(level, name)| (level, name.to_owned()));
    let categories: Vec<_> = tree.into_iter().take(expected.len()).collect();
    assert_eq!(categories, expected);
}

// Household Checking repeats ten transactions 2,000 times: among them Fresh Market
// for -100.00 and -150.00 in Groceries, Hardware Barn for -200.00 in Household,
// Trattoria Roma for -75.00 in Restaurants and Bean There for -5.00 in Coffee Shops.
// Everyday Checking adds Fresh Market for -86.42, and Costco, split into Groceries
// for -142.37 and Household for -63.90.

#[test]
fn a_category_counts_its_whole_transactions_and_its_split_lines() {
    let harness = category_list();

    assert_eq!(
        usage(&harness, "Groceries"),
        "4,002 transactions, totalling -$500,228.79"
    );
    assert_eq!(
        usage(&harness, "Household"),
        "2,001 transactions, totalling -$400,063.90"
    );
}

#[test]
fn a_parent_counts_its_own_transactions_and_its_subcategories() {
    let harness = category_list();

    // Old Checking's closing withdrawal of -250.42 is in Shopping itself.
    assert_eq!(
        usage(&harness, "Shopping"),
        "2,002 transactions, totalling -$400,314.32"
    );
}

#[test]
fn a_parent_counts_its_subcategories_at_every_depth() {
    let harness = category_list();

    // Restaurants holds Trattoria Roma and Coffee Shops; Everyday Checking adds
    // -64.80 in Restaurants and -4.75 and -5.25 in Coffee Shops.
    assert_eq!(
        usage(&harness, "Restaurants"),
        "4,003 transactions, totalling -$160,074.80"
    );
    assert_eq!(
        usage(&harness, "Food & Dining"),
        "8,005 transactions, totalling -$660,303.59"
    );
}

#[test]
fn the_txns_and_total_cells_show_the_usage() {
    let harness = category_list();

    assert!(shows(&harness, "8,005"));
    assert!(shows(&harness, "-$660,303.59"));
}

#[test]
fn a_transfer_row_counts_the_transfers_to_its_account() {
    let mut ledger = Ledger::stub();
    let usd = CurrencyCode::new("USD").unwrap();
    ledger
        .apply(DomainCommand::PostTransaction {
            account: account(&ledger, "Joint Checking"),
            transaction: NewTransaction::transfer(
                Date::try_new_iso(2026, 10, 1).unwrap(),
                "Monthly saving",
                account(&ledger, "Emergency Fund"),
                Money::new("-200.00".parse().unwrap(), usd).unwrap(),
            ),
        })
        .unwrap();
    let mut harness = app_on(ledger);

    open_category_list(&mut harness);

    assert_eq!(
        usage(&harness, "[Emergency Fund]"),
        "1 transaction, totalling -$200.00"
    );
    assert_eq!(
        usage(&harness, "[Joint Checking]"),
        "1 transaction, totalling $200.00"
    );
    assert_eq!(usage(&harness, "[Everyday Checking]"), "No transactions");
}

#[test]
fn collapsing_a_parent_hides_every_category_under_it() {
    let mut harness = category_list();

    click(&mut harness, "Collapse Food & Dining");

    assert_eq!(
        categories(&harness)[3..6],
        ["Food & Dining", "Home", "Insurance"]
    );
    assert_eq!(expanded(&harness, "Food & Dining"), Some(false));
}

#[test]
fn expanding_a_parent_shows_its_categories_again() {
    let mut harness = category_list();
    click(&mut harness, "Collapse Food & Dining");

    click(&mut harness, "Expand Food & Dining");

    assert_eq!(
        categories(&harness)[3..8],
        [
            "Food & Dining",
            "Groceries",
            "Restaurants",
            "Coffee Shops",
            "Home"
        ]
    );
    assert_eq!(expanded(&harness, "Food & Dining"), Some(true));
}

#[test]
fn a_category_with_none_under_it_has_no_fold() {
    let harness = category_list();

    assert_eq!(expanded(&harness, "Groceries"), None);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Collapse Groceries")
            .is_none()
    );
}

#[test]
fn clicking_a_row_selects_it_and_gives_it_focus() {
    let mut harness = category_list();

    click_row(&mut harness, "Groceries");

    assert_eq!(selected(&harness).as_deref(), Some("Groceries"));
    assert_eq!(focused(&harness).as_deref(), Some("Groceries"));
}

#[test]
fn down_and_up_move_the_selection_through_the_rows_shown() {
    let mut harness = category_list();
    click_row(&mut harness, "Food & Dining");
    click(&mut harness, "Collapse Food & Dining");

    press(&mut harness, Key::ArrowDown);
    let after_down = selected(&harness);
    press(&mut harness, Key::ArrowUp);
    press(&mut harness, Key::ArrowUp);

    assert_eq!(after_down.as_deref(), Some("Home"));
    // Auto & Transport's Insurance, the row above Food & Dining.
    assert_eq!(selected(&harness).as_deref(), Some("Insurance"));
    assert_eq!(focused(&harness).as_deref(), Some("Insurance"));
}

#[test]
fn left_collapses_the_selected_row() {
    let mut harness = category_list();
    click_row(&mut harness, "Restaurants");

    press(&mut harness, Key::ArrowLeft);

    assert_eq!(expanded(&harness, "Restaurants"), Some(false));
    assert_eq!(
        categories(&harness)[4..7],
        ["Groceries", "Restaurants", "Home"]
    );
}

#[test]
fn right_expands_the_selected_row() {
    let mut harness = category_list();
    click_row(&mut harness, "Restaurants");
    press(&mut harness, Key::ArrowLeft);

    press(&mut harness, Key::ArrowRight);

    assert_eq!(expanded(&harness, "Restaurants"), Some(true));
    assert_eq!(
        categories(&harness)[4..8],
        ["Groceries", "Restaurants", "Coffee Shops", "Home"]
    );
}

#[test]
fn down_scrolls_the_selection_into_view() {
    let mut harness = app_in(Ledger::stub(), vec2(1280.0, 480.0));
    open_category_list(&mut harness);
    click_row(&mut harness, "Auto & Transport");

    // Fifteen categories, then the stub's thirteen accounts, Mortgage last.
    for _ in 0..27 {
        press(&mut harness, Key::ArrowDown);
    }

    assert_eq!(selected(&harness).as_deref(), Some("[Mortgage]"));
    assert_eq!(focused(&harness).as_deref(), Some("[Mortgage]"));
}

#[test]
fn the_tree_is_one_tab_stop() {
    let mut harness = category_list();
    click_row(&mut harness, "Groceries");

    let focusable: Vec<String> = harness
        .query_all_by_role(Role::TreeItem)
        .filter(|row| row.accesskit_node().data().supports_action(Action::Focus))
        .filter_map(|row| row.accesskit_node().label())
        .collect();

    assert_eq!(focusable, ["Groceries"]);
}

#[test]
fn the_scroll_bar_sits_beside_the_total_column_not_over_it() {
    let harness = category_list();
    let window = harness
        .get_by_role_and_label(Role::Window, "Category List")
        .rect();

    let total = harness
        .query_all_by_role(Role::Label)
        .find(|label| label.accesskit_node().value().as_deref() == Some("-$660,303.59"))
        .expect("Food & Dining's total is shown")
        .rect();

    // The stub's rows overflow the Dialog, so the table shows its scroll bar.
    let bar = ScrollStyle::solid();
    assert!(
        window.right() - total.right() >= bar.bar_width + bar.bar_inner_margin,
        "Total ends at {}, too close to the Dialog's edge at {}",
        total.right(),
        window.right()
    );
}

#[test]
fn deleting_a_transaction_in_the_register_takes_it_out_of_the_usage() {
    let mut harness = app_on(Ledger::stub());
    harness
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .click();
    harness.run();
    harness.get_by_role_and_label(Role::Cell, "Costco").click();
    harness.run();
    harness.event(Event::Key {
        key: Key::D,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    });
    harness.run();
    press(&mut harness, Key::Enter);

    open_category_list(&mut harness);

    // Costco's split lines were -142.37 in Groceries and -63.90 in Household.
    assert_eq!(
        usage(&harness, "Groceries"),
        "4,001 transactions, totalling -$500,086.42"
    );
    assert_eq!(
        usage(&harness, "Household"),
        "2,000 transactions, totalling -$400,000.00"
    );
}
