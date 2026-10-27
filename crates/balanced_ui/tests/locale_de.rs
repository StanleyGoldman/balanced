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
        .with_size(vec2(1440.0, 720.0))
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
fn the_split_dialog_reads_german_and_shows_german_amounts() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    harness.get_by_role_and_label(Role::Cell, "[Split]").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, "Betrag, Zeile 1")
        .click();
    harness.run();
    press(&mut harness, Modifiers::COMMAND, Key::A);
    type_text(&mut harness, "100");

    open(&mut harness, "Fertig");

    let mismatch = "Die Zeilen ergeben 163,90, nicht die Summe von 206,27. Differenz: 42,37";
    assert!(
        harness.query_all_by_role(Role::Label).any(|label| label
            .accesskit_node()
            .value()
            .as_deref()
            == Some(mismatch))
    );
}

/// The text of the context header's Cleared figure, the one Label starting `Bestätigt `.
fn cleared_figure(harness: &Harness<'static, App>) -> String {
    let figures: Vec<String> = harness
        .query_all_by_role(Role::Label)
        .filter_map(|label| label.accesskit_node().value())
        .filter(|value| value.starts_with("Bestätigt "))
        .collect();
    let [figure] = <[String; 1]>::try_from(figures).expect("the header shows one Cleared figure");
    figure
}

#[test]
fn the_context_header_shows_the_cleared_balance_the_german_way() {
    let mut harness = app();

    open(&mut harness, "Everyday Checking");

    assert_eq!(cleared_figure(&harness), "Bestätigt 2.643,58\u{a0}$");
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

#[test]
fn the_reconciled_amount_confirm_reads_german() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    harness
        .get_by_role_and_label(Role::Cell, "1.450,00")
        .click();
    harness.run();
    type_text(&mut harness, "1500");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    harness.get_by_label(
        "Diese Buchung wurde abgeglichen. Betrag auf -1.500,00 ändern? \
         Maple Property Management, 03.09.2026, -1.450,00",
    );
    harness.get_by_role_and_label(Role::Button, "Betrag ändern");
}

#[test]
fn the_reconciled_delete_confirm_reads_german() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    harness
        .get_by_role_and_label(Role::Cell, "1.450,00")
        .click();
    harness.run();

    press(&mut harness, Modifiers::COMMAND, Key::D);

    harness.get_by_label(
        "Diese Buchung wurde abgeglichen. Trotzdem löschen? \
         Maple Property Management, 03.09.2026, -1.450,00",
    );
}

#[test]
fn the_unreconcile_confirm_reads_german() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");

    open(&mut harness, "Abgl Acme Corp, 01.09.2026, 3.200,00");

    harness.get_by_label(
        "Abgleich dieser Buchung aufheben? Die Buchung wurde mit einem Kontoauszug abgeglichen und wird auf „Bestätigt“ gesetzt. Acme Corp, 01.09.2026, 3.200,00",
    );
    harness.get_by_role_and_label(Role::Button, "Auf „Bestätigt“ setzen");
}

#[test]
fn the_calendars_today_button_reads_german() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    harness
        .get_by_role_and_label(Role::Cell, "05.09.2026")
        .click();
    harness.run();
    press(&mut harness, Modifiers::NONE, Key::Enter);

    press(&mut harness, Modifiers::ALT, Key::ArrowDown);

    harness.get_by_role_and_label(Role::Button, "Heute");
}

#[test]
fn the_editing_rows_buttons_read_in_german() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    harness
        .get_by_role_and_label(Role::Cell, "Fresh Market")
        .click();
    harness.run();

    press(&mut harness, Modifiers::NONE, Key::Enter);

    let shown = ["Speichern", "Abbrechen", "Mehrteilig", "Weitere Aktionen"].map(|label| {
        harness
            .query_all_by_role_and_label(Role::Button, label)
            .count()
    });
    assert_eq!(shown, [1, 1, 1, 1]);
}

#[test]
fn the_enter_moves_to_the_next_row_preference_reads_german() {
    let mut harness = app();
    open(&mut harness, "Extras");
    open(&mut harness, "Einstellungen …");

    // The group names fall back to en-US text.
    harness.get_by_role_and_label(Role::Tab, "Register").click();
    harness.run();

    harness.get_by_role_and_label(Role::CheckBox, "Mit »Eingabe« zur nächsten Zeile gehen");
}

#[test]
fn the_leave_row_prompt_reads_german() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    harness
        .get_by_role_and_label(Role::Cell, "Fresh Market")
        .click();
    harness.run();
    type_text(&mut harness, "Corner Shop");

    harness
        .get_by_role_and_label(Role::Cell, "Bean There")
        .click();
    harness.run();

    harness.get_by_label("Geänderte Buchung speichern?");
    harness.get_by_role_and_label(Role::Button, "Speichern");
    harness.get_by_role_and_label(Role::Button, "Weiter bearbeiten");
    harness.get_by_role_and_label(Role::Button, "Verwerfen");
    harness.get_by_role_and_label(Role::CheckBox, "Immer ohne Nachfrage speichern");
}

#[test]
fn the_leave_row_prompt_on_the_draft_row_reads_german() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    type_keys(&mut harness, "15.10.2026");
    // Date, Num, Payee, Category.
    for _ in 0..3 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
    }
    type_text(&mut harness, "Food & Dining:Groceries");
    press(&mut harness, Modifiers::NONE, Key::Tab);

    harness
        .get_by_role_and_label(Role::Cell, "Fresh Market")
        .click();
    harness.run();

    harness.get_by_label("Neue Buchung hinzufügen?");
    harness.get_by_role_and_label(Role::Button, "Hinzufügen");
}

#[test]
fn the_cant_save_dialog_reads_german() {
    let mut harness = app();
    open(&mut harness, "Everyday Checking");
    harness.get_by_role_and_label(Role::Cell, "86,42").click();
    harness.run();
    type_text(&mut harness, "1,2,3");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    harness.get_by_label("Diese Buchung kann nicht gespeichert werden");
    harness.get_by_role_and_label(Role::Button, "Weiter bearbeiten");
    harness.get_by_role_and_label(Role::Button, "Änderungen verwerfen");
}

#[test]
fn the_category_lists_buttons_and_delete_confirm_read_german() {
    let mut harness = app();
    harness
        .get_by_role_and_label(Role::Button, "Extras")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Category List…")
        .click();
    harness.run();
    // Home:Insurance is the stub's one category nothing uses.
    harness
        .query_all_by_role_and_label(Role::TreeItem, "Insurance")
        .last()
        .expect("the stub has Home:Insurance")
        .click();
    harness.run();

    for label in ["Neu …", "Bearbeiten …", "Zusammenführen …", "Ausblenden"] {
        harness.get_by_role_and_label(Role::Button, label);
    }
    harness.get_by_label("Änderungen gelten sofort nach der Bestätigung.");
    harness
        .get_by_role_and_label(Role::Button, "Löschen …")
        .click();
    harness.run();

    harness.get_by_label("Home:Insurance löschen? Keine Buchung verwendet diese Kategorie.");
    harness.get_by_role_and_label(Role::Button, "Löschen");
}

#[test]
fn the_reassign_prompt_reads_german() {
    let mut harness = app();
    harness
        .get_by_role_and_label(Role::Button, "Extras")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Category List…")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::TreeItem, "Food & Dining")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Löschen …")
        .click();
    harness.run();

    harness.get_by_label(
        "Food & Dining und ihre 3 Unterkategorien werden von 8.005 Buchungen mit insgesamt \
         -660.303,59\u{a0}$ verwendet.",
    );
    let target = harness.get_by_role_and_label(Role::ComboBox, "Diese Buchungen verschieben nach:");
    assert_eq!(
        target.accesskit_node().value().as_deref(),
        Some("Kategorie auswählen")
    );
}

#[test]
fn the_edit_category_dialog_reads_german() {
    let mut harness = app();
    harness
        .get_by_role_and_label(Role::Button, "Extras")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Category List…")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::TreeItem, "Groceries")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Bearbeiten …")
        .click();
    harness.run();

    harness.get_by_role_and_label(Role::Window, "Kategorie bearbeiten");
    harness.get_by_role_and_label(Role::ComboBox, "Übergeordnete Kategorie");
    harness.get_by_role_and_label(Role::RadioButton, "Ausgabe");
}

#[test]
fn the_category_lists_filter_bar_reads_german() {
    let mut harness = app();
    harness
        .get_by_role_and_label(Role::Button, "Extras")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Category List…")
        .click();
    harness.run();

    harness.get_by_role_and_label(Role::TextInput, "Suchen");
    harness
        .get_by_role_and_label(Role::ComboBox, "Anzeigen")
        .click();
    harness.run();

    for option in ["Alle", "Einnahme", "Ausgabe", "Benutzt", "Unbenutzt"] {
        harness.get_by_role_and_label(Role::Button, option);
    }
}

/// Opens the merge Dialog for the Category List row named `name`.
fn merge_row(harness: &mut Harness<'static, App>, name: &str) {
    harness
        .get_by_role_and_label(Role::Button, "Extras")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Category List…")
        .click();
    harness.run();
    harness.get_by_role_and_label(Role::TreeItem, name).click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Zusammenführen …")
        .click();
    harness.run();
}

#[test]
fn the_merge_dialog_reads_german() {
    let mut harness = app();
    merge_row(&mut harness, "Household");

    harness.get_by_label("Diese Kategorie mit einer anderen zusammenführen:");
    harness.get_by_label("2.001 Buchungen werden verschoben.");
    harness.get_by_role_and_label(Role::CheckBox, "Unbenutzte ausgewählte Kategorien löschen");
    harness.get_by_role_and_label(Role::Button, "Zusammenführen");
    let target = harness.get_by_role_and_label(Role::ComboBox, "Buchungen zusammenführen in:");
    assert_eq!(
        target.accesskit_node().value().as_deref(),
        Some("Kategorie auswählen")
    );
    target.click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Income:Salary")
        .scroll_to_me();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Income:Salary")
        .click();
    harness.run();

    harness.get_by_label("Dies führt Ausgabe mit Einnahme zusammen.");
}

#[test]
fn the_merge_refusal_reads_german() {
    let mut harness = app();

    merge_row(&mut harness, "Food & Dining");

    harness
        .get_by_label("Food & Dining hat Unterkategorien und kann nicht zusammengeführt werden:");
}
