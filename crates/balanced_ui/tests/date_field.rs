//! The Register's segmented date field, driven through the AccessKit tree the way a
//! user drives it, on a clock stopped on Wednesday 14 October 2026.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, PointerButton, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;
use icu_locale::{langid, locale};

/// The App on stub data in `i18n`'s locale, on 14 October 2026, wide enough to show
/// every Register column.
fn app_in(i18n: Localizer) -> Harness<'static, App> {
    let today = Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date");
    Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(move |cc| App::new(cc, i18n, Ledger::stub(), Clock::fixed(today)))
}

/// The App in the test locale, `en-US`.
fn app() -> Harness<'static, App> {
    app_in(Localizer::for_tests())
}

/// The App with German formatting and `en-US` text.
fn app_de() -> Harness<'static, App> {
    app_in(Localizer::new(&langid!("en-US"), &locale!("de-DE")).expect("en-US and de-DE ship"))
}

/// Opens Everyday Checking, puts the cell cursor on the Date cell labelled `date`, and
/// opens it with Enter.
fn edit_date(harness: &mut Harness<'static, App>, date: &str) {
    harness
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .click();
    harness.run();
    harness.get_by_role_and_label(Role::Cell, date).click();
    harness.run();
    press(harness, Modifiers::NONE, Key::Enter);
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

/// Types `text` in one go, as a paste or a quick typist's input method does.
fn type_at_once(harness: &mut Harness<'static, App>, text: &str) {
    send(harness, Event::Text(text.to_owned()));
    harness.run();
}

/// The text of the open date field.
fn field(harness: &Harness<'static, App>) -> String {
    harness
        .get_by_role_and_label(Role::TextInput, "Date")
        .accesskit_node()
        .value()
        .unwrap_or_default()
}

/// The Date cell on the row whose payee is `payee`.
fn date_of(harness: &Harness<'static, App>, payee: &str) -> String {
    let payee_x = harness
        .get_by_role_and_label(Role::ColumnHeader, "Payee")
        .rect()
        .x_range();
    let date_x = harness
        .get_by_role_and_label(Role::ColumnHeader, "Date")
        .rect()
        .x_range();
    let row = harness
        .query_all_by_role(Role::Cell)
        .find(|cell| {
            payee_x.contains(cell.rect().center().x)
                && cell.accesskit_node().label().as_deref() == Some(payee)
        })
        .expect("the Register lists the payee")
        .rect()
        .y_range();
    harness
        .query_all_by_role(Role::Cell)
        .find(|cell| {
            date_x.contains(cell.rect().center().x) && row.contains(cell.rect().center().y)
        })
        .expect("the row has a Date cell")
        .accesskit_node()
        .label()
        .unwrap_or_default()
}

#[test]
fn t_sets_today() {
    let mut harness = app();
    edit_date(&mut harness, "9/5/2026");

    type_keys(&mut harness, "t");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(date_of(&harness, "Fresh Market"), "10/14/2026");
}

#[test]
fn m_moves_to_the_first_of_the_month() {
    let mut harness = app();
    edit_date(&mut harness, "9/5/2026");

    type_keys(&mut harness, "m");

    assert_eq!(field(&harness), "9/1/2026");
}

#[test]
fn each_shortcut_moves_the_date_of_an_unedited_field() {
    let typed = [
        ("+", "9/6/2026"),
        ("=", "9/6/2026"),
        ("-", "9/4/2026"),
        ("h", "9/30/2026"),
        ("y", "1/1/2026"),
        ("r", "12/31/2026"),
        ("T", "10/14/2026"),
    ];
    let pressed = [
        (Key::ArrowUp, "9/6/2026"),
        (Key::ArrowDown, "9/4/2026"),
        (Key::PageUp, "8/5/2026"),
        (Key::PageDown, "10/5/2026"),
    ];
    for (text, expected) in typed {
        let mut harness = app();
        edit_date(&mut harness, "9/5/2026");

        type_keys(&mut harness, text);

        assert_eq!(field(&harness), expected, "{text:?}");
    }
    for (key, expected) in pressed {
        let mut harness = app();
        edit_date(&mut harness, "9/5/2026");

        press(&mut harness, Modifiers::NONE, key);

        assert_eq!(field(&harness), expected, "{key:?}");
    }
}

#[test]
fn a_full_or_impossible_segment_moves_to_the_next() {
    let mut harness = app();
    edit_date(&mut harness, "9/5/2026");

    type_keys(&mut harness, "35");
    let after_month_and_day = field(&harness);
    type_keys(&mut harness, "2026");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(after_month_and_day, "3/5/");
    assert_eq!(date_of(&harness, "Fresh Market"), "3/5/2026");
}

#[test]
fn a_typed_separator_moves_on_early_unless_typing_just_did() {
    let mut harness = app();
    edit_date(&mut harness, "9/5/2026");

    type_keys(&mut harness, "1/2/2026");
    let early = field(&harness);
    press(&mut harness, Modifiers::NONE, Key::Escape);
    press(&mut harness, Modifiers::NONE, Key::Enter);
    type_keys(&mut harness, "10/12/2026");

    assert_eq!(early, "1/2/2026");
    assert_eq!(field(&harness), "10/12/2026");
}

#[test]
fn german_segments_go_day_month_year() {
    let mut harness = app_de();
    edit_date(&mut harness, "05.09.2026");

    type_keys(&mut harness, "2510");
    let after_day_and_month = field(&harness);
    type_keys(&mut harness, "2026");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(after_day_and_month, "25.10.");
    assert_eq!(date_of(&harness, "Fresh Market"), "25.10.2026");
}

#[test]
fn eight_digits_typed_with_no_separators_make_a_date() {
    let mut harness = app();
    edit_date(&mut harness, "9/5/2026");

    type_keys(&mut harness, "20261005");
    let typed = field(&harness);
    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(typed, "10/5/2026");
    assert_eq!(date_of(&harness, "Fresh Market"), "10/5/2026");
}

#[test]
fn a_pasted_date_fills_the_segments() {
    for pasted in ["10/5/2026", "2026-10-05", "20261005"] {
        let mut harness = app();
        edit_date(&mut harness, "9/5/2026");

        send(&mut harness, Event::Paste(pasted.to_owned()));
        harness.run();

        assert_eq!(field(&harness), "10/5/2026", "{pasted:?}");
    }
}

#[test]
fn a_date_typed_at_once_fills_the_segments() {
    let mut harness = app();
    edit_date(&mut harness, "9/5/2026");

    type_at_once(&mut harness, "20261005");

    assert_eq!(field(&harness), "10/5/2026");
}

/// Whether a label reads `text`, ignoring Fluent's bidi isolation marks, which a
/// localizer built for the app puts around each argument. AccessKit holds a label's
/// text as its value.
fn shows(harness: &Harness<'static, App>, text: &str) -> bool {
    harness.query_all_by_role(Role::Label).any(|label| {
        label
            .accesskit_node()
            .value()
            .is_some_and(|label| label.replace(['\u{2068}', '\u{2069}'], "") == text)
    })
}

/// The format message in `en-US`.
const FORMAT_MESSAGE: &str = "Not a date. Expected M/D/YYYY, for example 10/5/2026.";

#[test]
fn a_partial_or_impossible_date_is_refused_with_the_format_message() {
    for (typed, shown) in [("5", "5//"), ("3/5", "3/5/"), ("2/30/2026", "2/30/2026")] {
        let mut harness = app();
        edit_date(&mut harness, "9/5/2026");
        type_keys(&mut harness, typed);

        press(&mut harness, Modifiers::NONE, Key::Enter);

        assert!(
            shows(&harness, FORMAT_MESSAGE),
            "{typed:?} shows the format message"
        );
        assert_eq!(field(&harness), shown, "{typed:?} stays in the field");
        assert_eq!(date_of(&harness, "Fresh Market"), "9/5/2026", "{typed:?}");
    }
}

#[test]
fn german_partial_dates_are_refused_with_the_german_pattern() {
    let mut harness = app_de();
    edit_date(&mut harness, "05.09.2026");
    type_keys(&mut harness, "5.3.");

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(shows(
        &harness,
        "Not a date. Expected DD.MM.YYYY, for example 05.10.2026."
    ));
}

#[test]
fn typing_again_clears_the_format_message() {
    let mut harness = app();
    edit_date(&mut harness, "9/5/2026");
    type_keys(&mut harness, "5");
    press(&mut harness, Modifiers::NONE, Key::Enter);

    type_keys(&mut harness, "/");

    assert!(!shows(&harness, FORMAT_MESSAGE));
}

#[test]
fn shortcuts_do_nothing_once_the_field_is_edited() {
    let mut harness = app();
    edit_date(&mut harness, "9/5/2026");
    type_keys(&mut harness, "1");

    type_keys(&mut harness, "tm+");
    press(&mut harness, Modifiers::NONE, Key::ArrowUp);
    press(&mut harness, Modifiers::NONE, Key::PageDown);

    assert_eq!(field(&harness), "1//");
}

#[test]
fn the_calendar_button_shows_only_while_the_cell_is_edited() {
    let mut harness = app();
    harness
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Cell, "9/5/2026")
        .click();
    harness.run();
    let before = harness
        .query_by_role_and_label(Role::Button, "Calendar")
        .is_some();

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert!(!before);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Calendar")
            .is_some()
    );
}

#[test]
fn clicking_the_calendar_button_opens_the_fields_month() {
    let mut harness = app();
    edit_date(&mut harness, "9/5/2026");

    harness
        .get_by_role_and_label(Role::Button, "Calendar")
        .click();
    harness.run();

    assert!(shows(&harness, "September 2026"));
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "9/30/2026")
            .is_some()
    );
    assert_eq!(field(&harness), "9/5/2026");
}

#[test]
fn alt_down_opens_the_calendar_without_moving_the_date() {
    let mut harness = app();
    edit_date(&mut harness, "9/5/2026");

    press(&mut harness, Modifiers::ALT, Key::ArrowDown);

    assert!(shows(&harness, "September 2026"));
    assert_eq!(field(&harness), "9/5/2026");
}

/// The calendar's weekday headings, left to right.
fn weekdays(harness: &Harness<'static, App>) -> Vec<String> {
    let mut headings: Vec<_> = harness
        .query_all_by_role(Role::ColumnHeader)
        .filter(|heading| heading.rect().top() > 100.0)
        .map(|heading| {
            (
                heading.rect().left(),
                heading.accesskit_node().label().unwrap_or_default(),
            )
        })
        .collect();
    headings.sort_by(|a, b| a.0.total_cmp(&b.0));
    headings.into_iter().map(|(_, label)| label).collect()
}

#[test]
fn the_calendar_week_starts_on_the_locales_first_weekday() {
    let mut en_us = app();
    let mut de_de = app_de();
    edit_date(&mut en_us, "9/5/2026");
    edit_date(&mut de_de, "05.09.2026");

    press(&mut en_us, Modifiers::ALT, Key::ArrowDown);
    press(&mut de_de, Modifiers::ALT, Key::ArrowDown);

    assert_eq!(
        weekdays(&en_us),
        ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
    );
    assert_eq!(weekdays(&de_de), ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"]);
}

#[test]
fn a_calendar_day_picks_on_pointer_press() {
    let mut harness = app();
    edit_date(&mut harness, "9/5/2026");
    press(&mut harness, Modifiers::ALT, Key::ArrowDown);
    let day = harness
        .get_by_role_and_label(Role::Button, "9/24/2026")
        .rect()
        .center();

    harness.event(Event::PointerMoved(day));
    harness.event(Event::PointerButton {
        pos: day,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    harness.run();

    assert_eq!(field(&harness), "9/24/2026");
    assert!(!shows(&harness, "September 2026"));
}

#[test]
fn a_clicked_day_stays_in_the_open_field_until_enter() {
    let mut harness = app();
    edit_date(&mut harness, "9/5/2026");
    press(&mut harness, Modifiers::ALT, Key::ArrowDown);
    harness
        .get_by_role_and_label(Role::Button, "9/24/2026")
        .click();
    harness.run();
    let picked = field(&harness);

    press(&mut harness, Modifiers::NONE, Key::Enter);

    assert_eq!(picked, "9/24/2026");
    assert_eq!(date_of(&harness, "Fresh Market"), "9/24/2026");
}

#[test]
fn esc_closes_the_calendar_before_it_cancels_the_edit() {
    let mut harness = app();
    edit_date(&mut harness, "9/5/2026");
    press(&mut harness, Modifiers::ALT, Key::ArrowDown);

    press(&mut harness, Modifiers::NONE, Key::Escape);

    assert!(!shows(&harness, "September 2026"));
    assert_eq!(field(&harness), "9/5/2026");
}

#[test]
fn typing_on_a_date_cell_opens_it_and_acts_on_the_key() {
    let mut harness = app();
    harness
        .get_by_role_and_label(Role::Button, "Everyday Checking")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Cell, "9/5/2026")
        .click();
    harness.run();

    type_keys(&mut harness, "m");

    assert_eq!(field(&harness), "9/1/2026");
}

#[test]
fn clicking_away_from_a_refused_date_keeps_the_edit_and_its_message() {
    let mut harness = app();
    edit_date(&mut harness, "9/5/2026");
    type_keys(&mut harness, "5");

    harness.get_by_role_and_label(Role::Cell, "Shell").click();
    harness.run();

    assert!(shows(&harness, FORMAT_MESSAGE));
    assert_eq!(field(&harness), "5//");
    assert_eq!(date_of(&harness, "Fresh Market"), "9/5/2026");
}
