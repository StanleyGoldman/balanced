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
/// Register column, with the Account bar unfolded.
fn app() -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(|cc| {
            App::new(
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
        .get_by_role_and_label(Role::Button, "Kontenleiste ausklappen")
        .click();
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
    cells[payee - 2..payee + 5].to_vec()
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
            "-86,42",
            "2.863,58",
        ]
    );
}

#[test]
fn the_account_bar_shows_net_worth_the_german_way() {
    let harness = app();

    // The stub's Net Worth, $288,497.71 in en-US. ICU4X puts a no-break space before
    // the symbol.
    harness.get_by_role_and_label(Role::Label, "288.497,71\u{a0}$");
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
    // Category, Memo, Amount.
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::NONE, Key::Tab);
    type_text(&mut harness, "-1.234,5");

    press(&mut harness, Modifiers::COMMAND, Key::Enter);

    assert_eq!(
        row_of(&harness, "Corner Cafe"),
        [
            "15.10.2026",
            "",
            "Corner Cafe",
            "Food & Dining:Groceries",
            "",
            "-1.234,50",
            "4.412,96",
        ]
    );
}
