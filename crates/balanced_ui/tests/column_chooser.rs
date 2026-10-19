//! The Register's column chooser, and the column list it keeps in the preferences file,
//! driven through the AccessKit tree the way a user drives it.

use std::path::Path;

use balanced_core::Ledger;
use balanced_i18n::Localizer;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::egui::accesskit::{Role, SortDirection, Toggled};
use eframe::egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;
use tempfile::TempDir;

/// The App on stub data, in the test locale, wide enough to show every Register column,
/// keeping its Preferences in `config_dir`.
fn app_in(config_dir: &Path) -> Harness<'static, App> {
    let file = PreferencesFile::in_dir(config_dir);
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 720.0))
        .build_eframe(move |cc| {
            App::new(
                cc,
                Localizer::for_tests(),
                Ledger::stub(),
                Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
                file,
            )
        });
    harness.run();
    harness
}

/// A config directory that lasts as long as the returned `TempDir`.
fn config_dir() -> TempDir {
    tempfile::tempdir().expect("the test machine can make a temporary directory")
}

/// Opens the account named `name` from the Account bar.
fn open(harness: &mut Harness<'static, App>, name: &str) {
    harness.get_by_role_and_label(Role::Button, name).click();
    harness.run();
}

/// Clicks the widget with `role` labelled `label`, then lets the App settle.
fn click(harness: &mut Harness<'static, App>, role: Role, label: &str) {
    harness.get_by_role_and_label(role, label).click();
    harness.run();
}

/// Opens the column chooser.
fn open_chooser(harness: &mut Harness<'static, App>) {
    click(harness, Role::Button, "Choose columns");
}

/// The Register's column headings, left to right. The Actions column's is empty.
fn headings(harness: &Harness<'static, App>) -> Vec<String> {
    let mut headers: Vec<_> = harness
        .query_all_by_role(Role::ColumnHeader)
        .map(|header| {
            let label = header.accesskit_node().label().unwrap_or_default();
            (header.rect().left(), label)
        })
        .collect();
    headers.sort_by(|(left, _), (right, _)| left.total_cmp(right));
    headers.into_iter().map(|(_, label)| label).collect()
}

/// The text of the preferences file in `config_dir`.
fn file_text(config_dir: &TempDir) -> String {
    std::fs::read_to_string(config_dir.path().join("preferences.toml"))
        .expect("the App wrote the preferences file")
}

#[test]
fn unticking_memo_hides_it_and_saves_it_as_hidden() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open(&mut harness, "Everyday Checking");
    open_chooser(&mut harness);

    click(&mut harness, Role::CheckBox, "Memo");

    assert_eq!(
        headings(&harness),
        [
            "Date", "Num", "Payee", "Category", "Payment", "Deposit", "Balance", "Flag", ""
        ]
    );
    assert_eq!(
        file_text(&dir),
        "version = 2\n\
         scheme = \"light\"\n\
         font_face = \"egui\"\n\
         dense_rows = false\n\
         monospace_money = false\n\
         coloured_negatives = true\n\
         alternating_rows = true\n\
         today_divider = true\n\
         gridlines = \"off\"\n\
         group_totals = false\n\
         net_worth_placement = \"account_bar\"\n\
         register_columns = [\"date\", \"num\", \"payee\", \"category\", \"memo\", \"payment\", \"deposit\", \"balance\", \"flag\"]\n\
         hidden_register_columns = [\"memo\"]\n"
    );
}

#[test]
fn moving_payee_up_puts_it_before_num_and_saves_the_order() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open(&mut harness, "Everyday Checking");
    open_chooser(&mut harness);

    click(&mut harness, Role::Button, "Move Payee up");

    assert_eq!(
        headings(&harness),
        [
            "Date", "Payee", "Num", "Category", "Memo", "Payment", "Deposit", "Balance", "Flag", ""
        ]
    );
    assert_eq!(
        file_text(&dir),
        "version = 2\n\
         scheme = \"light\"\n\
         font_face = \"egui\"\n\
         dense_rows = false\n\
         monospace_money = false\n\
         coloured_negatives = true\n\
         alternating_rows = true\n\
         today_divider = true\n\
         gridlines = \"off\"\n\
         group_totals = false\n\
         net_worth_placement = \"account_bar\"\n\
         register_columns = [\"date\", \"payee\", \"num\", \"category\", \"memo\", \"payment\", \"deposit\", \"balance\", \"flag\"]\n\
         hidden_register_columns = []\n"
    );
}

#[test]
fn the_chooser_stays_open_after_hiding_a_column() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open(&mut harness, "Everyday Checking");
    open_chooser(&mut harness);

    click(&mut harness, Role::CheckBox, "Memo");
    click(&mut harness, Role::Button, "Move Payee up");

    assert_eq!(
        headings(&harness),
        [
            "Date", "Payee", "Num", "Category", "Payment", "Deposit", "Balance", "Flag", ""
        ]
    );
}

#[test]
fn hiding_the_sorted_column_sorts_by_date() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open(&mut harness, "Everyday Checking");
    click(&mut harness, Role::ColumnHeader, "Payee");
    open_chooser(&mut harness);

    click(&mut harness, Role::CheckBox, "Payee");

    let date = harness.get_by_role_and_label(Role::ColumnHeader, "Date");
    assert_eq!(
        date.accesskit_node().sort_direction(),
        Some(SortDirection::Ascending)
    );
}

#[test]
fn the_chooser_lists_payment_and_deposit_with_checkboxes_that_cant_be_unticked() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open(&mut harness, "Everyday Checking");
    open_chooser(&mut harness);

    let checkbox = |label| {
        let node = harness.get_by_role_and_label(Role::CheckBox, label);
        let node = node.accesskit_node();
        (node.toggled(), node.is_disabled())
    };

    assert_eq!(checkbox("Payment"), (Some(Toggled::True), true));
    assert_eq!(checkbox("Deposit"), (Some(Toggled::True), true));
}

#[test]
fn the_chooser_lists_flag_ticked() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open(&mut harness, "Everyday Checking");

    open_chooser(&mut harness);

    let flag = harness.get_by_role_and_label(Role::CheckBox, "Flag");
    assert_eq!(flag.accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn unticking_flag_hides_the_flag_column() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open(&mut harness, "Everyday Checking");
    open_chooser(&mut harness);

    click(&mut harness, Role::CheckBox, "Flag");

    assert_eq!(
        headings(&harness),
        [
            "Date", "Num", "Payee", "Category", "Memo", "Payment", "Deposit", "Balance", ""
        ]
    );
}

#[test]
fn moving_flag_up_puts_it_before_balance() {
    let dir = config_dir();
    let mut harness = app_in(dir.path());
    open(&mut harness, "Everyday Checking");
    open_chooser(&mut harness);

    click(&mut harness, Role::Button, "Move Flag up");

    assert_eq!(
        headings(&harness),
        [
            "Date", "Num", "Payee", "Category", "Memo", "Payment", "Deposit", "Flag", "Balance", ""
        ]
    );
}
