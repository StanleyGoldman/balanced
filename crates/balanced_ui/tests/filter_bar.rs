//! The filter bar above the Register, which narrows its rows by search text, date
//! range and transaction type, driven through the AccessKit tree the way a user
//! drives it.

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::App as _;
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT as _, Queryable as _};
use icu_calendar::Date;

/// The App on stub data, in the test locale, wide enough to show every Register column,
/// on 14 October 2026.
fn app() -> Harness<'static, App> {
    app_on(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date"))
}

/// The App on stub data, in the test locale, wide enough to show every Register column,
/// with `today` as today's date.
fn app_on(today: Date<icu_calendar::Iso>) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(|cc| {
            App::new(
                cc,
                Localizer::for_tests(),
                Ledger::stub(),
                Clock::fixed(today),
                PreferencesFile::for_tests(),
            )
        });
    harness.run();
    harness
}

/// The App on 14 October 2026 with the account named `name` open.
fn app_showing(name: &str) -> Harness<'static, App> {
    let mut harness = app();
    click(&mut harness, Role::Button, name);
    harness
}

/// Clicks the widget with `role` labelled `label`, then lets the App settle.
fn click(harness: &mut Harness<'static, App>, role: Role, label: &str) {
    harness.get_by_role_and_label(role, label).click();
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

/// Types `text` in one go into whatever has the keyboard.
fn type_text(harness: &mut Harness<'static, App>, text: &str) {
    send(harness, Event::Text(text.to_owned()));
    harness.run();
}

/// Types `text` into the search box.
fn search(harness: &mut Harness<'static, App>, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, "Search")
        .focus();
    harness.run();
    type_text(harness, text);
}

/// Picks `item` from the combobox named `combo`.
fn choose(harness: &mut Harness<'static, App>, combo: &str, item: &str) {
    click(harness, Role::ComboBox, combo);
    click(harness, Role::Button, item);
}

/// The App on `today`, with Everyday Checking open and its dates narrowed to `range`.
fn everyday_on(today: (i32, u8, u8), range: &str) -> Harness<'static, App> {
    let (year, month, day) = today;
    let mut harness = app_on(Date::try_new_iso(year, month, day).expect("tests pass real dates"));
    click(&mut harness, Role::Button, "Everyday Checking");
    choose(&mut harness, "Dates", range);
    harness
}

/// The text of the filter bar's count of rows shown, if it shows one.
fn count(harness: &Harness<'static, App>) -> Option<String> {
    harness
        .query_all_by_role(Role::Label)
        .filter_map(|node| node.accesskit_node().value())
        .find(|value| value.ends_with(" shown"))
}

/// The label of the widget with keyboard focus, or `None` when nothing has it.
fn focused(harness: &Harness<'static, App>) -> Option<String> {
    harness
        .query_all(By::default())
        .find(|node| node.accesskit_node().is_focused())
        .map(|node| node.accesskit_node().label().unwrap_or_default())
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

/// Fills the draft row, from the Date cell under the cursor, Tabbing from cell to
/// cell, with `payment` in the Payment cell, and adds it with Ctrl+Enter.
fn add_transaction(harness: &mut Harness<'static, App>, date: &str, payee: &str, payment: &str) {
    for c in date.chars() {
        send(harness, Event::Text(c.to_string()));
        harness.run();
    }
    // Date, Num, Payee.
    press(harness, Modifiers::NONE, Key::Tab);
    press(harness, Modifiers::NONE, Key::Tab);
    type_text(harness, payee);
    press(harness, Modifiers::NONE, Key::Tab);
    type_text(harness, "Food & Dining:Groceries");
    // Category, Memo, Payment.
    press(harness, Modifiers::NONE, Key::Tab);
    press(harness, Modifiers::NONE, Key::Tab);
    type_text(harness, payment);
    press(harness, Modifiers::COMMAND, Key::Enter);
}

/// Every payee in Everyday Checking, in date order, then the draft row's.
const EVERYDAY: [&str; 11] = [
    "Opening Balance",
    "Acme Corp",
    "Maple Property Management",
    "Fresh Market",
    "Bean There",
    "City Power & Light",
    "Costco",
    "Shell",
    "Acme Corp",
    "Trattoria Roma",
    "",
];

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

#[test]
fn searching_narrows_the_rows_to_payees_with_the_text_in_any_case() {
    let mut harness = app_showing("Everyday Checking");

    search(&mut harness, "aCmE");

    assert_eq!(column(&harness, "Payee"), ["Acme Corp", "Acme Corp", ""]);
}

#[test]
fn searching_matches_a_category_and_a_split_lines_category() {
    let mut harness = app_showing("Everyday Checking");

    search(&mut harness, "HOUSEHOLD");

    assert_eq!(column(&harness, "Payee"), ["Costco", ""]);
}

#[test]
fn searching_matches_the_category_path() {
    let mut harness = app_showing("Everyday Checking");

    search(&mut harness, "home:util");

    assert_eq!(column(&harness, "Payee"), ["City Power & Light", ""]);
}

#[test]
fn searching_matches_a_memo() {
    let mut harness = app_showing("Everyday Checking");

    search(&mut harness, "Rent");

    assert_eq!(column(&harness, "Payee"), ["Maple Property Management", ""]);
}

#[test]
fn last_30_days_starts_29_days_before_today() {
    let harness = everyday_on((2026, 10, 5), "Last 30 days");

    assert_eq!(column(&harness, "Payee"), &EVERYDAY[4..]);
}

#[test]
fn last_30_days_ends_today() {
    let harness = everyday_on((2026, 9, 17), "Last 30 days");

    assert_eq!(
        column(&harness, "Payee"),
        [&EVERYDAY[1..8], &EVERYDAY[10..]].concat()
    );
}

#[test]
fn last_12_months_starts_the_day_after_the_same_date_a_year_ago() {
    let harness = everyday_on((2027, 1, 1), "Last 12 months");

    assert_eq!(column(&harness, "Payee"), &EVERYDAY[1..]);
}

#[test]
fn this_year_is_the_whole_calendar_year_with_its_future_dates() {
    let mut harness = app_on(Date::try_new_iso(2026, 9, 10).expect("10 September 2026 is a date"));
    click(&mut harness, Role::Button, "Everyday Checking");

    choose(&mut harness, "Dates", "This year");

    assert_eq!(column(&harness, "Payee"), EVERYDAY);
}

#[test]
fn this_year_leaves_out_last_year() {
    let harness = everyday_on((2027, 3, 1), "This year");

    assert_eq!(column(&harness, "Payee"), [""]);
}

#[test]
fn last_year_is_the_calendar_year_before_this_one() {
    let harness = everyday_on((2027, 3, 1), "Last year");

    assert_eq!(column(&harness, "Payee"), EVERYDAY);
}

#[test]
fn last_year_leaves_out_this_year() {
    let harness = everyday_on((2026, 10, 14), "Last year");

    assert_eq!(column(&harness, "Payee"), [""]);
}

#[test]
fn payments_shows_only_money_out() {
    let mut harness = app_showing("Everyday Checking");

    choose(&mut harness, "Transaction type", "Payments");

    assert_eq!(
        column(&harness, "Payee"),
        [
            "Maple Property Management",
            "Fresh Market",
            "Bean There",
            "City Power & Light",
            "Costco",
            "Shell",
            "Trattoria Roma",
            "",
        ]
    );
}

#[test]
fn deposits_shows_only_money_in() {
    let mut harness = app_showing("Everyday Checking");

    choose(&mut harness, "Transaction type", "Deposits");

    assert_eq!(
        column(&harness, "Payee"),
        ["Opening Balance", "Acme Corp", "Acme Corp", ""]
    );
}

#[test]
fn flagged_shows_only_flagged_transactions() {
    let mut harness = app_showing("Everyday Checking");

    choose(&mut harness, "Transaction type", "Flagged");

    assert_eq!(column(&harness, "Payee"), ["City Power & Light", ""]);
}

#[test]
fn search_dates_and_type_combine() {
    let mut harness = app_showing("Everyday Checking");

    choose(&mut harness, "Dates", "Last 30 days");
    choose(&mut harness, "Transaction type", "Payments");
    search(&mut harness, "in");

    // Shell is a payment in range without "in"; Acme Corp (Income:Salary) is a deposit
    // in range with it; Fresh Market (Food & Dining:Groceries) a payment out of range.
    assert_eq!(column(&harness, "Payee"), ["Trattoria Roma", ""]);
}

#[test]
fn a_set_filter_counts_the_rows_it_shows() {
    let mut harness = app_showing("Everyday Checking");

    search(&mut harness, "acme");

    assert_eq!(count(&harness).as_deref(), Some("2 of 10 shown"));
}

#[test]
fn a_filter_matching_nothing_counts_0_and_keeps_the_draft_row() {
    let mut harness = app_showing("Everyday Checking");

    search(&mut harness, "zzz");

    assert_eq!(count(&harness).as_deref(), Some("0 of 10 shown"));
    assert_eq!(column(&harness, "Payee"), [""]);
}

#[test]
fn no_filter_shows_no_count() {
    let harness = app_showing("Everyday Checking");

    assert_eq!(count(&harness), None);
}

#[test]
fn reset_is_disabled_with_no_filter_set() {
    let harness = app_showing("Everyday Checking");

    let reset = harness.get_by_role_and_label(Role::Button, "Reset");

    assert!(reset.accesskit_node().is_disabled());
}

#[test]
fn reset_clears_every_filter() {
    let mut harness = app_showing("Everyday Checking");
    choose(&mut harness, "Dates", "Last 30 days");
    choose(&mut harness, "Transaction type", "Payments");
    search(&mut harness, "r");

    click(&mut harness, Role::Button, "Reset");

    assert_eq!(column(&harness, "Payee"), EVERYDAY);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Search")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("")
    );
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Reset")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn a_filtered_rows_balance_is_its_running_balance() {
    let mut harness = app_showing("Everyday Checking");

    search(&mut harness, "trattoria");

    assert_eq!(column(&harness, "Balance"), ["5,647.46", ""]);
}

#[test]
fn a_transaction_added_under_a_filter_stays_until_the_filter_changes() {
    let mut harness = app_showing("Everyday Checking");
    search(&mut harness, "acme");
    click(&mut harness, Role::Button, "+ Transaction");

    add_transaction(&mut harness, "10/15/2026", "Corner Cafe", "12");

    assert_eq!(
        column(&harness, "Payee"),
        ["Acme Corp", "Acme Corp", "Corner Cafe", ""]
    );
    search(&mut harness, " corp");
    assert_eq!(column(&harness, "Payee"), ["Acme Corp", "Acme Corp", ""]);
}

#[test]
fn opening_another_account_clears_the_filters() {
    let mut harness = app_showing("Everyday Checking");
    choose(&mut harness, "Transaction type", "Payments");
    search(&mut harness, "zzz");

    click(&mut harness, Role::Button, "Joint Checking");

    assert_eq!(column(&harness, "Payee"), ["Opening Balance", ""]);
    assert_eq!(count(&harness), None);
}

#[test]
fn tab_reaches_the_search_box_and_the_filter_combos() {
    let mut harness = app_showing("Everyday Checking");
    harness
        .get_by_role_and_label(Role::Button, "+ Transaction")
        .focus();
    harness.run();

    let mut seen = Vec::new();
    for _ in 0..3 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
        seen.push(focused(&harness).unwrap_or_default());
    }

    assert_eq!(seen, ["Search", "Dates", "Transaction type"]);
}

#[test]
fn tab_reaches_reset_once_a_filter_is_set() {
    let mut harness = app_showing("Everyday Checking");
    choose(&mut harness, "Transaction type", "Payments");
    harness
        .get_by_role_and_label(Role::Button, "+ Transaction")
        .focus();
    harness.run();

    let mut seen = Vec::new();
    for _ in 0..4 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
        seen.push(focused(&harness).unwrap_or_default());
    }

    assert_eq!(seen, ["Search", "Dates", "Transaction type", "Reset"]);
}

#[test]
fn a_search_of_only_spaces_sets_no_filter() {
    let mut harness = app_showing("Everyday Checking");

    search(&mut harness, "  ");

    assert_eq!(count(&harness), None);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Reset")
            .accesskit_node()
            .is_disabled()
    );
}
