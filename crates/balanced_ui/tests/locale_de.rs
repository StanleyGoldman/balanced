// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The App in German text with `de-DE` formatting: amounts and dates on screen and
//! typed into fields follow the formatting locale.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;
use icu_locale::{langid, locale};

/// The App on stub data in German with `de-DE` formatting, wide enough to show every
/// Register column.
fn app() -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(|cc| {
            App::on_ledger(
                cc,
                Localizer::for_tests_in(&langid!("de"), &locale!("de-DE"))
                    .expect("the embedded de messages and de-DE formatting data load"),
                Ledger::stub(),
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

/// Types `text` one character at a time, as a keyboard does.
fn type_keys(harness: &mut Harness<'static, App>, text: &str) {
    for c in text.chars() {
        send(harness, Event::Text(c.to_string()));
        harness.run();
    }
}

/// Types `text` in one go into whatever has the keyboard.
fn type_text(harness: &mut Harness<'static, App>, text: &str) {
    send(harness, Event::Text(text.to_owned()));
    harness.run();
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

#[test]
fn the_register_shows_dates_and_amounts_the_german_way() {
    let mut harness = app();

    open(&mut harness, "Everyday Checking");

    assert_eq!(
        row_of(&harness, "Fresh Market"),
        [
            "05.09.2026",
            "",
            "Fresh Market",
            "Food & Dining:Groceries",
            "",
            "86,42",
            "",
            "2.863,58",
        ]
    );
}

#[test]
fn the_account_bar_shows_net_worth_the_german_way() {
    let harness = app();

    // The stub's Net Worth, $286,922.06 in en-US. ICU4X puts a no-break space before
    // the symbol.
    harness.get_by_role_and_label(Role::Label, "286.922,06\u{a0}$");
}

#[test]
fn the_draft_row_reads_a_date_and_an_amount_typed_the_german_way() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    type_keys(&mut harness, "15.10.2026");
    // Date, Num, Payee.
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::NONE, Key::Tab);
    type_text(&mut harness, "Corner Cafe");
    press(&mut harness, Modifiers::NONE, Key::Tab);
    type_text(&mut harness, "Food & Dining:Groceries");
    // Category, Memo, Payment.
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::NONE, Key::Tab);
    type_text(&mut harness, "1.234,5");

    press(&mut harness, Modifiers::COMMAND, Key::Enter);

    assert_eq!(
        row_of(&harness, "Corner Cafe"),
        [
            "15.10.2026",
            "",
            "Corner Cafe",
            "Food & Dining:Groceries",
            "",
            "1.234,50",
            "",
            "4.407,71",
        ]
    );
}

#[test]
fn hovering_a_heading_cut_short_beside_its_total_shows_the_whole_heading() {
    let mut harness = app();
    // The Account bar's Preferences fall back to en-US text, with its terms.
    for (role, label) in [
        (Role::Button, "Extras"),
        (Role::Button, "Einstellungen …"),
        (Role::Tab, "Account bar"),
        (Role::CheckBox, "Show group totals"),
        (Role::Button, "OK"),
    ] {
        harness.get_by_role_and_label(role, label).click();
        harness.run();
    }

    // The tab of the same name has no value; the heading holds its total.
    harness
        .query_all_by_label("Anlagen & Verbindlichkeiten")
        .find(|node| node.accesskit_node().value().is_some())
        .expect("the Account bar shows the Property & Debt heading with its total")
        .hover();
    harness.run();

    // The tooltip shows among the window's other Labels.
    let labels: Vec<String> = harness
        .query_all_by_role(Role::Label)
        .filter_map(|label| label.accesskit_node().value())
        .collect();
    assert!(
        labels.contains(&"Anlagen & Verbindlichkeiten".to_owned()),
        "{labels:?}"
    );
}

#[test]
fn the_new_account_wizard_reads_and_shows_german_amounts_and_dates_in_english_text() {
    let mut harness = app();
    for label in ["Datei", "Neues Konto …", "Weiter"] {
        harness.get_by_role_and_label(Role::Button, label).click();
        harness.run();
    }
    // The wizard's fields and Review lines fall back to en-US text.
    let name = harness.get_by_role_and_label(Role::TextInput, "Name");
    name.focus();
    name.type_text("Rainy Day");
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Weiter")
        .click();
    harness.run();
    let balance = harness.get_by_role_and_label(Role::TextInput, "Opening balance");
    balance.focus();
    balance.type_text("1.234,56");
    harness.run();

    harness
        .get_by_role_and_label(Role::Button, "Weiter")
        .click();
    harness.run();

    harness.get_by_value("Opening balance: 1.234,56\u{a0}$ as of 14.10.2026");
}

#[test]
fn a_heavy_dialogs_close_button_reads_german() {
    let mut harness = app();

    open(&mut harness, "Extras");
    open(&mut harness, "Einstellungen …");

    harness.get_by_role_and_label(Role::Button, "Dialog schließen");
}

#[test]
fn a_clr_cell_reads_its_label_and_status_in_german() {
    let mut harness = app();

    open(&mut harness, "Everyday Checking");

    let status = harness
        .get_by_role_and_label(Role::Button, "Abgl Bean There, 06.09.2026, -4,75")
        .accesskit_node()
        .value();
    assert_eq!(status.as_deref(), Some("Bestätigt"));
}
