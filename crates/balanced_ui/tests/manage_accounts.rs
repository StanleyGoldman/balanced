//! Managing accounts through the Manage Accounts Dialog, driven through the AccessKit
//! tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock};
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on stub data, in the test locale.
fn app() -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .build_eframe(|cc| {
            App::new(
                cc,
                Localizer::for_tests(),
                Ledger::stub(),
                Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
            )
        });
    harness.run();
    harness
}

/// The App with Manage Accounts opened from Joint Checking's menu in the Account bar.
fn manage_accounts() -> Harness<'static, App> {
    let mut harness = app();
    harness
        .get_by_role_and_label(Role::Button, "Joint Checking")
        .click_secondary();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Manage Accounts…")
        .click();
    harness.run();
    harness
}

/// Replaces the name in the Name field showing `old` with `new`.
fn rename(harness: &mut Harness<'static, App>, old: &str, new: &str) {
    harness
        .get_by(|node| node.role() == Role::TextInput && node.value().as_deref() == Some(old))
        .focus();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.run();
    harness
        .get_by(|node| node.role() == Role::TextInput && node.value().as_deref() == Some(old))
        .type_text(new);
    harness.run();
}

/// Scrolls the button labelled `label` into view, then clicks it.
fn click(harness: &mut Harness<'static, App>, label: &str) {
    harness
        .get_by_role_and_label(Role::Button, label)
        .scroll_to_me();
    harness.run();
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run();
}

/// Whether the Account bar lists an account named `name`.
fn listed(harness: &Harness<'static, App>, name: &str) -> bool {
    harness
        .query_by_role_and_label(Role::Button, name)
        .is_some()
}

#[test]
fn ok_applies_a_rename_and_a_hide_to_the_account_bar() {
    let mut harness = manage_accounts();

    rename(&mut harness, "Joint Checking", "Joint");
    harness
        .get_by_role_and_label(Role::CheckBox, "Hide Emergency Fund")
        .click();
    harness.run();
    click(&mut harness, "OK");

    assert_eq!(
        [
            listed(&harness, "Joint"),
            listed(&harness, "Joint Checking"),
            listed(&harness, "Emergency Fund"),
        ],
        [true, false, false]
    );
}

#[test]
fn cancel_discards_every_change() {
    let mut harness = manage_accounts();

    rename(&mut harness, "Joint Checking", "Joint");
    harness
        .get_by_role_and_label(Role::CheckBox, "Hide Emergency Fund")
        .click();
    harness.run();
    click(&mut harness, "Cancel");

    assert_eq!(
        [
            listed(&harness, "Joint"),
            listed(&harness, "Joint Checking"),
            listed(&harness, "Emergency Fund"),
        ],
        [false, true, true]
    );
}

/// Whether a label shows `text`. A `Label`'s text is its AccessKit value.
fn shows(harness: &Harness<'static, App>, text: &str) -> bool {
    harness
        .query_all_by_role(Role::Label)
        .any(|label| label.accesskit_node().value().as_deref() == Some(text))
}

#[test]
fn closing_an_account_with_a_balance_warns_and_still_closes_it() {
    let mut harness = manage_accounts();

    click(&mut harness, "Close Joint Checking…");
    let warned = shows(
        &harness,
        "Close Joint Checking? It still has a balance of $1,032.00. Its Register becomes \
         read-only. You can reopen it later.",
    );
    click(&mut harness, "Close");

    assert!(warned);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Reopen Joint Checking")
            .is_some()
    );
}

/// Whether the button labelled `label` can be pressed.
fn enabled(harness: &Harness<'static, App>, label: &str) -> bool {
    !harness
        .get_by_role_and_label(Role::Button, label)
        .accesskit_node()
        .is_disabled()
}

#[test]
fn an_account_moves_no_further_than_the_edges_of_its_sub_group() {
    let harness = manage_accounts();

    // Savings holds Emergency Fund, then Holiday Fund, between Checking and Credit Card.
    assert_eq!(
        [
            enabled(&harness, "Move Emergency Fund up"),
            enabled(&harness, "Move Emergency Fund down"),
            enabled(&harness, "Move Holiday Fund up"),
            enabled(&harness, "Move Holiday Fund down"),
        ],
        [false, true, true, false]
    );
}

/// The labels of the Account bar's buttons, top to bottom.
fn buttons(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::Button)
        .filter_map(|node| node.accesskit_node().label())
        .collect()
}

#[test]
fn ok_applies_a_move_to_the_account_bar() {
    let mut harness = manage_accounts();

    click(&mut harness, "Move Roth IRA up");
    click(&mut harness, "OK");

    let retirement: Vec<String> = buttons(&harness)
        .into_iter()
        .filter(|label| label == "401(k)" || label == "Roth IRA")
        .collect();
    assert_eq!(retirement, ["Roth IRA", "401(k)"]);
}

#[test]
fn ok_moves_a_closed_account_to_the_account_bars_closed_group() {
    let mut harness = manage_accounts();

    click(&mut harness, "Close Joint Checking…");
    click(&mut harness, "Close");
    click(&mut harness, "OK");
    click(&mut harness, "Closed");

    let closed: Vec<String> = buttons(&harness)
        .into_iter()
        .skip_while(|label| label != "Closed")
        .skip(1)
        .take(2)
        .collect();
    assert_eq!(closed, ["Joint Checking", "Old Checking"]);
}

#[test]
fn one_undo_reverts_everything_ok_applied() {
    let mut harness = manage_accounts();
    rename(&mut harness, "Joint Checking", "Joint");
    harness
        .get_by_role_and_label(Role::CheckBox, "Hide Emergency Fund")
        .click();
    harness.run();
    click(&mut harness, "OK");

    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();

    assert_eq!(
        [
            listed(&harness, "Joint"),
            listed(&harness, "Joint Checking"),
            listed(&harness, "Emergency Fund"),
        ],
        [false, true, true]
    );
}

#[test]
fn a_confirmed_delete_takes_the_account_out_on_ok() {
    let mut harness = manage_accounts();

    click(&mut harness, "Delete Car…");
    let asked = shows(&harness, "Delete Car and every transaction in it?");
    click(&mut harness, "Delete");
    click(&mut harness, "OK");

    assert!(asked);
    assert!(!listed(&harness, "Car"));
}

#[test]
fn reopen_acts_at_once_without_a_confirm() {
    let mut harness = manage_accounts();

    click(&mut harness, "Reopen Old Checking");

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Close Old Checking…")
            .is_some()
    );
}

#[test]
fn ok_with_a_blank_name_stays_open_and_says_why() {
    let mut harness = manage_accounts();

    rename(&mut harness, "Joint Checking", " ");
    click(&mut harness, "OK");

    assert!(shows(&harness, "Enter a name for the account."));
    assert!(
        harness
            .query_by_role_and_label(Role::CheckBox, "Hide Emergency Fund")
            .is_some()
    );
}

#[test]
fn reconcile_in_an_accounts_menu_says_it_is_not_available_yet() {
    let mut harness = app();
    harness
        .get_by_role_and_label(Role::Button, "Joint Checking")
        .click_secondary();
    harness.run();

    click(&mut harness, "Reconcile");

    assert!(shows(&harness, "Not available yet"));
}

#[test]
fn tools_opens_manage_accounts() {
    let mut harness = app();

    click(&mut harness, "Tools");
    click(&mut harness, "Manage Accounts…");

    assert!(
        harness
            .query_by_role_and_label(Role::CheckBox, "Hide Emergency Fund")
            .is_some()
    );
}
