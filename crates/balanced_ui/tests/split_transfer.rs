// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Transfers in split lines, in the Register: a split line's other side, and the split
//! that holds the line.

use balanced_core::{
    AccountId, CategoryId, CurrencyCode, DomainCommand, Ledger, Money, NewSplitLine, NewTransaction,
};
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::{Role, Toggled};
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

/// The App on stub data in which Everyday Checking holds a Payday split on 1 October:
/// 50.00 on Food & Dining, and 200.00 to Emergency Fund as a line of its own.
fn payday() -> Harness<'static, App> {
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
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 720.0))
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

/// The text the Status bar shows.
fn status(harness: &Harness<'static, App>) -> Vec<String> {
    harness
        .get_by_role(Role::Status)
        .query_all_by_role(Role::Label)
        .filter_map(|label| label.accesskit_node().value())
        .collect()
}

#[test]
fn deleting_a_split_lines_other_side_says_to_change_the_split_naming_its_account() {
    let mut harness = payday();
    open(&mut harness, "Emergency Fund");
    harness.get_by_role_and_label(Role::Cell, "Payday").click();
    harness.run();

    press(&mut harness, Modifiers::COMMAND, Key::D);
    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(
        status(&harness),
        [
            "This is the other side of a split line. Change its date, amount or category, or delete it, from the split in Everyday Checking."
        ]
    );
    assert_eq!(
        column(&harness, "Payee")
            .iter()
            .filter(|payee| *payee == "Payday")
            .count(),
        1
    );
}

#[test]
fn searching_for_a_split_lines_account_finds_the_split() {
    let mut harness = payday();
    open(&mut harness, "Everyday Checking");

    harness
        .get_by_role_and_label(Role::TextInput, "Search")
        .focus();
    harness.run();
    send(&mut harness, Event::Text("[emergency".to_owned()));
    harness.run();

    assert_eq!(column(&harness, "Payee"), ["Payday", ""]);
}

#[test]
fn go_to_on_a_split_lines_other_side_goes_to_the_split() {
    let mut harness = payday();
    open(&mut harness, "Emergency Fund");
    harness.get_by_role_and_label(Role::Cell, "Payday").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Actions")
        .click();
    harness.run();

    harness
        .get_by_role_and_label(Role::Button, "Go to [Everyday Checking]")
        .click();
    harness.run();

    let cells: Vec<_> = harness
        .query_all_by_role(Role::Cell)
        .map(|cell| {
            let node = cell.accesskit_node();
            (node.label().unwrap_or_default(), node.toggled())
        })
        .collect();
    let payee = cells
        .iter()
        .position(|(label, _)| label == "Payday")
        .expect("the Register lists the split");
    assert_eq!(cells[payee].1, Some(Toggled::True));
    assert_eq!(cells[payee + 1].0, "[Split]");
}
