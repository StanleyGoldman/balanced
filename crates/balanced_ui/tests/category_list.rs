// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Reading the category tree and its usage in the Category List, driven through the
//! AccessKit tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on `ledger`, in the test locale.
fn app_on(ledger: Ledger) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 800.0))
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
