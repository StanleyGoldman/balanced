// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Reading the category tree and its usage in the Category List, driven through the
//! AccessKit tree the way a user drives it.

use balanced_core::{
    AccountId, CategoryId, CategoryType, CurrencyCode, DomainCommand, Ledger, Money, NewTransaction,
};
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::Action;
use eframe::egui::accesskit::Role;
use eframe::egui::style::ScrollStyle;
use eframe::egui::{Event, Key, Modifiers, PointerButton, Vec2, pos2, vec2};
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
        .map(|summary| summary.account())
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

/// What a screen reader reads after the name of the row named `name`: a category's
/// type, then how many transactions and split lines use it, and their total.
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
        "Expense: 4,002 transactions, totalling -$500,228.79"
    );
    assert_eq!(
        usage(&harness, "Household"),
        "Expense: 2,001 transactions, totalling -$400,063.90"
    );
}

#[test]
fn a_parent_counts_its_own_transactions_and_its_subcategories() {
    let harness = category_list();

    // Old Checking's closing withdrawal of -250.42 is in Shopping itself.
    assert_eq!(
        usage(&harness, "Shopping"),
        "Expense: 2,002 transactions, totalling -$400,314.32"
    );
}

#[test]
fn a_parent_counts_its_subcategories_at_every_depth() {
    let harness = category_list();

    // Restaurants holds Trattoria Roma and Coffee Shops; Everyday Checking adds
    // -64.80 in Restaurants and -4.75 and -5.25 in Coffee Shops.
    assert_eq!(
        usage(&harness, "Restaurants"),
        "Expense: 4,003 transactions, totalling -$160,074.80"
    );
    assert_eq!(
        usage(&harness, "Food & Dining"),
        "Expense: 8,005 transactions, totalling -$660,303.59"
    );
}

#[test]
fn each_category_reads_its_type_before_its_usage() {
    let mut ledger = Ledger::stub();
    ledger
        .apply(DomainCommand::CreateCategory {
            name: "Gifts".to_owned(),
            parent: None,
            category_type: CategoryType::Income,
        })
        .unwrap();
    let mut harness = app_on(ledger);
    open_category_list(&mut harness);

    // Household Checking's ten repeated transactions include Acme Corp for 1,000.00 in
    // Salary, and Everyday Checking adds two paydays of 3,200.00.
    assert_eq!(
        usage(&harness, "Salary"),
        "Income: 2,002 transactions, totalling $2,006,400.00"
    );
    assert_eq!(usage(&harness, "Gifts"), "Income: No transactions");
    assert_eq!(
        usage(&harness, "Shopping"),
        "Expense: 2,002 transactions, totalling -$400,314.32"
    );
}

#[test]
fn the_type_column_shows_each_categorys_type() {
    let harness = category_list();

    let types: Vec<String> = harness
        .query_all_by_role(Role::Label)
        .filter_map(|label| label.accesskit_node().value())
        .filter(|value| ["Income", "Expense"].contains(&value.as_str()))
        .collect();

    // The tree's order: Auto & Transport and Food & Dining with their subcategories,
    // Home and its two, then Income and its two, then Shopping and Household.
    let expected = [
        ["Expense"; 10].as_slice(),
        ["Income"; 3].as_slice(),
        ["Expense"; 2].as_slice(),
    ]
    .concat();
    assert_eq!(types, expected);
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
        "Expense: 4,001 transactions, totalling -$500,086.42"
    );
    assert_eq!(
        usage(&harness, "Household"),
        "Expense: 2,000 transactions, totalling -$400,000.00"
    );
}

/// Adds a category nothing uses, named `name`, under `parent` or at the top of the
/// tree as an Expense category, returning its id.
fn create_category(ledger: &mut Ledger, name: &str, parent: Option<CategoryId>) -> CategoryId {
    ledger
        .apply(DomainCommand::CreateCategory {
            name: name.to_owned(),
            parent,
            category_type: CategoryType::Expense,
        })
        .expect("the ledger creates the category");
    ledger
        .snapshot()
        .categories()
        .last()
        .expect("the ledger holds the new category")
        .id()
}

/// The App on stub data with an unused top-level category, Pets, added, and the
/// Category List open.
fn category_list_with_pets() -> Harness<'static, App> {
    let mut ledger = Ledger::stub();
    create_category(&mut ledger, "Pets", None);
    let mut harness = app_on(ledger);
    open_category_list(&mut harness);
    harness
}

/// Whether the New Category Dialog is open.
fn new_category_open(harness: &Harness<'static, App>) -> bool {
    harness
        .query_by_role_and_label(Role::Window, "New Category")
        .is_some()
}

/// The button column with only New… enabled.
const ONLY_NEW: [(&str, bool); 5] = [
    ("New…", true),
    ("Edit…", false),
    ("Merge…", false),
    ("Hide", false),
    ("Delete…", false),
];

/// Which of the button column's buttons are enabled, in order.
fn enabled_buttons(harness: &Harness<'static, App>) -> Vec<(&'static str, bool)> {
    ["New…", "Edit…", "Merge…", "Hide", "Delete…"]
        .map(|label| {
            let button = harness.get_by_role_and_label(Role::Button, label);
            (label, !button.accesskit_node().is_disabled())
        })
        .to_vec()
}

#[test]
fn with_no_row_selected_only_new_is_enabled() {
    let harness = category_list();

    assert_eq!(enabled_buttons(&harness), ONLY_NEW);
}

#[test]
fn selecting_an_unused_category_with_none_under_it_enables_delete() {
    let mut harness = category_list_with_pets();

    click_row(&mut harness, "Pets");

    assert_eq!(enabled_buttons(&harness), NEW_EDIT_AND_DELETE);
}

/// The button column with a category selected: New…, Edit… and Delete… enabled.
const NEW_EDIT_AND_DELETE: [(&str, bool); 5] = [
    ("New…", true),
    ("Edit…", true),
    ("Merge…", false),
    ("Hide", false),
    ("Delete…", true),
];

#[test]
fn selecting_a_used_category_enables_delete() {
    let mut harness = category_list();

    click_row(&mut harness, "Groceries");

    assert_eq!(enabled_buttons(&harness), NEW_EDIT_AND_DELETE);
}

#[test]
fn selecting_a_parent_enables_delete() {
    let mut ledger = Ledger::stub();
    let pets = create_category(&mut ledger, "Pets", None);
    create_category(&mut ledger, "Vet", Some(pets));
    let mut harness = app_on(ledger);
    open_category_list(&mut harness);

    click_row(&mut harness, "Pets");

    assert_eq!(enabled_buttons(&harness), NEW_EDIT_AND_DELETE);
}

#[test]
fn a_transfer_row_disables_every_button_but_new() {
    let mut harness = category_list();

    click_row(&mut harness, "[Emergency Fund]");

    assert_eq!(enabled_buttons(&harness), ONLY_NEW);
}

#[test]
fn the_status_line_says_changes_apply_at_once() {
    let harness = category_list();

    assert!(shows(
        &harness,
        "Changes apply as soon as you confirm them."
    ));
}

/// Types `text` into whatever has the keyboard, first passing it through the App's
/// `raw_input_hook` as eframe does, then runs the App until it settles.
fn type_text(harness: &mut Harness<'static, App>, text: &str) {
    let ctx = harness.ctx.clone();
    let mut input = std::mem::take(harness.input_mut());
    input.events.push(Event::Text(text.to_owned()));
    harness.state_mut().raw_input_hook(&ctx, &mut input);
    *harness.input_mut() = input;
    harness.step();
    harness.run();
}

#[test]
fn delete_asks_before_deleting_an_unused_category() {
    let mut harness = category_list_with_pets();
    click_row(&mut harness, "Pets");

    click(&mut harness, "Delete…");

    assert!(shows(&harness, "Delete Pets? No transaction uses it."));
    assert!(categories(&harness).contains(&"Pets".to_owned()));
}

#[test]
fn confirming_the_delete_takes_the_category_out_of_the_list() {
    let mut harness = category_list_with_pets();
    click_row(&mut harness, "Pets");
    click(&mut harness, "Delete…");

    click(&mut harness, "Delete");

    assert!(is_open(&harness));
    assert!(!categories(&harness).contains(&"Pets".to_owned()));
}

#[test]
fn cancelling_the_delete_keeps_the_category() {
    let mut harness = category_list_with_pets();
    click_row(&mut harness, "Pets");
    click(&mut harness, "Delete…");

    click(&mut harness, "Cancel");

    assert!(is_open(&harness));
    assert!(!shows(&harness, "Delete Pets? No transaction uses it."));
    assert!(categories(&harness).contains(&"Pets".to_owned()));
}

#[test]
fn one_undo_puts_a_deleted_category_back() {
    let mut harness = category_list_with_pets();
    click_row(&mut harness, "Pets");
    click(&mut harness, "Delete…");
    click(&mut harness, "Delete");
    click(&mut harness, "Close");

    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    open_category_list(&mut harness);

    assert!(categories(&harness).contains(&"Pets".to_owned()));
}

#[test]
fn new_creates_a_category_that_appears_in_the_list() {
    let mut harness = category_list();
    click(&mut harness, "New…");

    type_text(&mut harness, "Pets");
    click(&mut harness, "OK");

    assert!(!new_category_open(&harness));
    assert!(categories(&harness).contains(&"Pets".to_owned()));
}

#[test]
fn a_category_made_with_new_is_offered_in_the_category_field() {
    let mut harness = category_list();
    click(&mut harness, "New…");
    type_text(&mut harness, "Pets");
    click(&mut harness, "OK");
    click(&mut harness, "Close");

    harness
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Cell, "Auto & Transport:Fuel")
        .click();
    harness.run();
    type_text(&mut harness, "pet");

    let rows: Vec<String> = harness
        .query_all_by_role(Role::ListBoxOption)
        .map(|row| row.accesskit_node().label().unwrap_or_default())
        .collect();
    assert_eq!(rows, ["Pets", "Create \"pet\"…"]);
}

#[test]
fn escape_closes_new_category_and_leaves_the_list_open() {
    let mut harness = category_list();
    click(&mut harness, "New…");

    press(&mut harness, Key::Escape);

    assert!(!new_category_open(&harness));
    assert!(is_open(&harness));
}

/// Opens the Delete confirm for the row named `name`.
fn delete_row(harness: &mut Harness<'static, App>, name: &str) {
    click_row(harness, name);
    click(harness, "Delete…");
}

#[test]
fn deleting_a_used_category_says_how_many_transactions_it_moves() {
    let mut harness = category_list();

    delete_row(&mut harness, "Groceries");

    assert!(shows(
        &harness,
        "Food & Dining:Groceries is used by 4,002 transactions totalling -$500,228.79."
    ));
}

/// Whether the button labelled `label` takes clicks.
fn enabled(harness: &Harness<'static, App>, label: &str) -> bool {
    !harness
        .get_by_role_and_label(Role::Button, label)
        .accesskit_node()
        .is_disabled()
}

/// The Reassign target list's label.
const MOVE_TO: &str = "Move those transactions to:";

/// What the Reassign target list shows as picked.
fn target(harness: &Harness<'static, App>) -> Option<String> {
    harness
        .get_by_role_and_label(Role::ComboBox, MOVE_TO)
        .accesskit_node()
        .value()
}

/// Opens the Reassign target list.
fn open_targets(harness: &mut Harness<'static, App>) {
    harness
        .get_by_role_and_label(Role::ComboBox, MOVE_TO)
        .click();
    harness.run();
}

/// Picks `path` from the Reassign target list.
fn choose_target(harness: &mut Harness<'static, App>, path: &str) {
    open_targets(harness);
    harness
        .get_by_role_and_label(Role::Button, path)
        .scroll_to_me();
    harness.run();
    click(harness, path);
}

/// Whether the open Reassign target list offers `path`.
fn offers(harness: &Harness<'static, App>, path: &str) -> bool {
    harness.query_by_role_and_label(Role::Button, path).is_some()
}

#[test]
fn delete_waits_for_a_category_to_move_the_transactions_to() {
    let mut harness = category_list();
    delete_row(&mut harness, "Groceries");
    assert_eq!(target(&harness).as_deref(), Some("Choose a category"));
    assert!(!enabled(&harness, "Delete"));

    choose_target(&mut harness, "Shopping:Household");

    assert_eq!(target(&harness).as_deref(), Some("Shopping:Household"));
    assert!(enabled(&harness, "Delete"));
}

#[test]
fn the_targets_leave_out_the_category_and_those_under_it() {
    let mut harness = category_list();
    delete_row(&mut harness, "Food & Dining");

    open_targets(&mut harness);

    assert!(!offers(&harness, "Food & Dining"));
    assert!(!offers(&harness, "Food & Dining:Groceries"));
    assert!(!offers(&harness, "Food & Dining:Restaurants:Coffee Shops"));
    assert!(offers(&harness, "Shopping"));
}

#[test]
fn a_click_on_the_dimmed_window_closes_only_the_target_list() {
    let mut harness = category_list();
    delete_row(&mut harness, "Groceries");
    open_targets(&mut harness);
    // The window's top left corner, which the dimmed window covers.
    let behind = pos2(4.0, 4.0);

    harness.event(Event::PointerMoved(behind));
    for pressed in [true, false] {
        harness.event(Event::PointerButton {
            pos: behind,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    harness.run();

    assert!(!offers(&harness, "Shopping"));
    assert!(harness.query_by_role_and_label(Role::ComboBox, MOVE_TO).is_some());
}

#[test]
fn enter_deletes_nothing_until_a_target_is_picked() {
    let mut harness = category_list();
    delete_row(&mut harness, "Groceries");

    press(&mut harness, Key::Enter);

    assert!(harness.query_by_role_and_label(Role::ComboBox, MOVE_TO).is_some());
    assert!(categories(&harness).contains(&"Groceries".to_owned()));
}

/// Deletes the row named `name`, moving its transactions to `path`, then closes the
/// Category List.
fn delete_moving_to(harness: &mut Harness<'static, App>, name: &str, path: &str) {
    delete_row(harness, name);
    choose_target(harness, path);
    click(harness, "Delete");
    click(harness, "Close");
}

#[test]
fn deleting_a_used_category_moves_its_transactions_to_the_target() {
    let mut harness = category_list();

    delete_moving_to(&mut harness, "Groceries", "Shopping:Household");
    open_category_list(&mut harness);

    assert!(!categories(&harness).contains(&"Groceries".to_owned()));
    // Its 2,001 transactions totalling -$400,063.90, and Groceries' 4,002 totalling
    // -$500,228.79.
    assert_eq!(
        usage(&harness, "Household"),
        "Expense: 6,003 transactions, totalling -$900,292.69"
    );
}

#[test]
fn one_undo_puts_back_a_reassigned_category_and_its_transactions() {
    let mut harness = category_list();
    delete_moving_to(&mut harness, "Groceries", "Shopping:Household");

    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    open_category_list(&mut harness);

    assert_eq!(
        usage(&harness, "Groceries"),
        "Expense: 4,002 transactions, totalling -$500,228.79"
    );
    assert_eq!(
        usage(&harness, "Household"),
        "Expense: 2,001 transactions, totalling -$400,063.90"
    );
}
