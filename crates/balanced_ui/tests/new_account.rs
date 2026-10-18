//! Creating an account through the New Account wizard, driven through the AccessKit
//! tree the way a user drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on stub data, in the test locale, on 14 October 2026.
fn app() -> Harness<'static, App> {
    let mut harness = Harness::new_eframe(|cc| {
        App::new(
            cc,
            Localizer::for_tests(),
            Ledger::stub(),
            Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
            PreferencesFile::for_tests(),
        )
    });
    harness.run();
    harness
}

/// The App with the wizard opened from File → New Account….
fn wizard() -> Harness<'static, App> {
    let mut harness = app();
    harness.get_by_role_and_label(Role::Button, "File").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "New Account…")
        .click();
    harness.run();
    harness
}

/// Presses `key` with no modifier, then runs the App until it settles.
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

/// Types `text` into the text field labelled `label`.
fn type_into(harness: &mut Harness<'static, App>, label: &str, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .focus();
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .type_text(text);
    harness.run();
}

/// The text of every label shown.
fn labels(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::Label)
        .filter_map(|node| node.accesskit_node().value())
        .collect()
}

/// Whether a label shows `text`.
fn shows(harness: &Harness<'static, App>, text: &str) -> bool {
    labels(harness).iter().any(|label| label == text)
}

/// The accounts the Account bar lists, in order.
fn account_bar(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::Button)
        .filter_map(|node| node.accesskit_node().label())
        .skip_while(|label| label != "Banking")
        .take_while(|label| label != "Closed")
        .collect()
}

#[test]
fn the_wizard_walks_from_type_to_a_new_account_last_in_its_sub_group() {
    let mut harness = wizard();
    harness
        .get_by_role_and_label(Role::RadioButton, "Savings")
        .click();
    harness.run();
    press(&mut harness, Key::Enter);
    press(&mut harness, Key::Enter);
    assert!(shows(&harness, "Enter a name for the account."));
    type_into(&mut harness, "Name", "Rainy Day");
    press(&mut harness, Key::Enter);
    type_into(&mut harness, "Opening balance", "1,234.56");
    press(&mut harness, Key::Enter);
    assert!(shows(&harness, "Step 4 of 4: Review"));
    assert!(shows(
        &harness,
        "Opening balance: $1,234.56 as of 10/14/2026"
    ));

    press(&mut harness, Key::Enter);

    assert!(!shows(&harness, "Step 4 of 4: Review"));
    let accounts = account_bar(&harness);
    let savings: Vec<_> = accounts
        .iter()
        .skip_while(|name| *name != "Emergency Fund")
        .take(3)
        .collect();
    assert_eq!(savings, ["Emergency Fund", "Rainy Day", "Visa Rewards"]);
    harness
        .get_by_role_and_label(Role::Button, "Rainy Day")
        .click();
    harness.run();
    assert!(shows(&harness, "$1,234.56"), "{:?}", labels(&harness));
}

#[test]
fn the_account_bar_opens_the_wizard() {
    let mut harness = app();

    harness
        .get_by_role_and_label(Role::Button, "+ Account")
        .click();
    harness.run();

    assert!(shows(&harness, "Step 1 of 4: Type"));
}

#[test]
fn back_returns_to_the_last_step_keeping_what_was_typed() {
    let mut harness = wizard();
    press(&mut harness, Key::Enter);
    type_into(&mut harness, "Name", "Joint");
    harness.get_by_role_and_label(Role::Button, "Back").click();
    harness.run();
    assert!(shows(&harness, "Step 1 of 4: Type"));

    press(&mut harness, Key::Enter);

    let name = harness.get_by_role_and_label(Role::TextInput, "Name");
    assert_eq!(name.accesskit_node().value().as_deref(), Some("Joint"));
}

#[test]
fn an_amount_the_field_refuses_keeps_the_opening_balance_step() {
    let mut harness = wizard();
    press(&mut harness, Key::Enter);
    type_into(&mut harness, "Name", "Joint");
    press(&mut harness, Key::Enter);
    type_into(&mut harness, "Opening balance", "12.345");

    press(&mut harness, Key::Enter);

    assert!(shows(&harness, "Step 3 of 4: Opening balance"));
    assert!(shows(&harness, "USD amounts have 2 decimal places"));
}

#[test]
fn escape_cancels_the_wizard_and_creates_nothing() {
    let mut harness = wizard();
    press(&mut harness, Key::Enter);
    type_into(&mut harness, "Name", "Joint");
    let before = account_bar(&harness);

    press(&mut harness, Key::Escape);

    assert!(!shows(&harness, "Step 2 of 4: Details"));
    assert_eq!(account_bar(&harness), before);
}

#[test]
fn a_refused_finish_shows_why_and_keeps_the_wizard_open() {
    let mut harness = wizard();
    press(&mut harness, Key::Enter);
    type_into(&mut harness, "Name", "Windfall");
    press(&mut harness, Key::Enter);
    // The largest amount there is, which takes Net Worth out of range.
    type_into(
        &mut harness,
        "Opening balance",
        "792281625142643375935439503",
    );
    press(&mut harness, Key::Enter);

    press(&mut harness, Key::Enter);

    assert!(shows(&harness, "Step 4 of 4: Review"));
    assert!(shows(
        &harness,
        "That change would make a balance too large to hold."
    ));
}

#[test]
fn a_day_picked_from_the_calendar_dates_the_opening_balance() {
    let mut harness = wizard();
    press(&mut harness, Key::Enter);
    type_into(&mut harness, "Name", "Rainy Day");
    press(&mut harness, Key::Enter);
    type_into(&mut harness, "Opening balance", "10.00");
    harness
        .get_by_role_and_label(Role::Button, "Calendar")
        .click();
    harness.run();

    harness
        .get_by_role_and_label(Role::Button, "10/24/2026")
        .click();
    harness.run();
    press(&mut harness, Key::Enter);

    assert!(shows(&harness, "Opening balance: $10.00 as of 10/24/2026"));
}
