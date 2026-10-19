//! Creating and opening data files from the App. The native file picker can't be
//! driven, so each test starts at the path it would return, sending the Command that
//! File → New Data File… or File → Open Data File… sends once a path is picked.

use std::path::{Path, PathBuf};

use balanced_core::{AccountKind, CurrencyCode, DomainCommand, Ledger, Money};
use balanced_i18n::Localizer;
use balanced_ui::testing;
use balanced_ui::{App, Clock, PreferencesFile};
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use icu_calendar::Date;
use tempfile::TempDir;

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
                PreferencesFile::for_tests(),
            )
        });
    harness.run();
    harness
}

fn usd() -> CurrencyCode {
    CurrencyCode::new("USD").expect("USD is an ISO 4217 currency")
}

/// A closed data file at `name` in `dir`, holding checking accounts with `accounts`,
/// created in that order, so the last one is the change Undo reverses first.
fn data_file(dir: &TempDir, name: &str, accounts: &[&str]) -> PathBuf {
    let path = dir.path().join(name);
    let mut ledger = Ledger::create(&path, usd()).expect("a new path takes a data file");
    for account in accounts {
        ledger
            .apply(DomainCommand::CreateAccount {
                name: (*account).to_owned(),
                kind: AccountKind::Checking,
                opened: Date::try_new_iso(2026, 9, 1).expect("1 September 2026 is a date"),
                opening_balance: Money::new("10.00".parse().expect("10.00 is a number"), usd())
                    .expect("10.00 has two decimal places"),
            })
            .expect("a new file takes a new account");
    }
    path
}

/// The names of the accounts in the data file at `path`, opened afresh.
fn accounts_in(path: &Path) -> Vec<String> {
    Ledger::open(path)
        .expect("the test's file opens")
        .snapshot()
        .accounts()
        .iter()
        .map(|summary| summary.account().name().to_owned())
        .collect()
}

/// Whether the Account bar lists an account named `name`.
fn listed(harness: &Harness<'static, App>, name: &str) -> bool {
    harness
        .query_by_role_and_label(Role::Button, name)
        .is_some()
}

/// Creates a data file at `path`, as File → New Data File… does once it is picked.
fn create(harness: &mut Harness<'static, App>, path: &Path) {
    testing::new_data_file(harness, path);
    harness.run();
}

/// Opens the data file at `path`, as File → Open Data File… does once it is picked.
fn open(harness: &mut Harness<'static, App>, path: &Path) {
    testing::open_data_file(harness, path);
    harness.run();
}

#[test]
fn opening_a_data_file_shows_its_accounts_in_the_account_bar() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let household = data_file(&dir, "Household.balanced", &["Household Checking"]);
    let mut harness = app();

    open(&mut harness, &household);

    assert!(listed(&harness, "Household Checking"));
    assert!(!listed(&harness, "Joint Checking"));
}

/// Clicks Edit → Undo.
fn undo(harness: &mut Harness<'static, App>) {
    harness.get_by_role_and_label(Role::Button, "Edit").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Undo Ctrl+Z")
        .click();
    harness.run();
}

#[test]
fn undo_after_switching_back_undoes_that_files_last_change() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let household = data_file(&dir, "Household.balanced", &["Everyday", "Savings"]);
    let business = data_file(&dir, "Business.balanced", &["Payroll", "Reserve"]);
    let mut harness = app();
    open(&mut harness, &household);
    open(&mut harness, &business);
    open(&mut harness, &household);

    undo(&mut harness);

    assert!(listed(&harness, "Everyday"));
    assert!(!listed(&harness, "Savings"));
    drop(harness);
    assert_eq!(accounts_in(&business), ["Payroll", "Reserve"]);
}

/// Opens the file at `path`, expecting a refusal, and checks the refusal Dialog says
/// `message` and leaves the stub open: its accounts stay in the Account bar, and OK
/// closes the Dialog.
fn assert_refused(path: &Path, message: &str) {
    let mut harness = app();

    open(&mut harness, path);

    harness.get_by_label(message);
    assert!(listed(&harness, "Joint Checking"));
    harness.get_by_role_and_label(Role::Button, "OK").click();
    harness.run();
    assert!(harness.query_by_label(message).is_none());
}

#[test]
fn a_missing_file_is_refused_with_a_dialog() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");

    assert_refused(
        &dir.path().join("Gone.balanced"),
        "Balanced can't find “Gone.balanced”. It may have been moved, renamed or deleted.",
    );
}

#[test]
fn a_file_that_isnt_a_data_file_is_refused_with_a_dialog() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = dir.path().join("Notes.balanced");
    std::fs::write(&path, "Groceries: milk, eggs").expect("the temp dir is writable");

    assert_refused(&path, "“Notes.balanced” isn't a Balanced data file.");
}

/// Rewrites the bytes of the data file at `path` with `change`.
fn rewrite(path: &Path, change: impl FnOnce(&mut [u8])) {
    let mut bytes = std::fs::read(path).expect("the test made the file");
    change(&mut bytes);
    std::fs::write(path, bytes).expect("the test's file is writable");
}

#[test]
fn a_damaged_file_is_refused_with_a_dialog() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = data_file(&dir, "Household.balanced", &["Everyday"]);
    // Fills the second page, a table's root, with bytes that aren't a page. The SQLite
    // header holds the page size at offset 16, big-endian.
    rewrite(&path, |bytes| {
        let page_size = usize::from(u16::from_be_bytes([bytes[16], bytes[17]]));
        bytes[page_size..2 * page_size].fill(0xFF);
    });

    assert_refused(
        &path,
        "“Household.balanced” is damaged. Restore it from a backup.",
    );
}

#[test]
fn a_file_from_a_newer_balanced_is_refused_with_a_dialog() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = data_file(&dir, "Household.balanced", &["Everyday"]);
    // The SQLite header holds the schema version, `user_version`, at offset 60,
    // big-endian.
    rewrite(&path, |bytes| {
        bytes[60..64].copy_from_slice(&1000_u32.to_be_bytes());
    });

    assert_refused(
        &path,
        "“Household.balanced” was made by a newer version of Balanced. \
         Update Balanced to open it.",
    );
}

#[test]
fn a_file_open_elsewhere_is_refused_with_a_dialog() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = data_file(&dir, "Household.balanced", &["Everyday"]);
    let _elsewhere = Ledger::open(&path).expect("the test's file opens");

    assert_refused(
        &path,
        "“Household.balanced” is open in another program. Close it there, then try again.",
    );
}

#[test]
fn reopening_the_open_file_changes_nothing() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let household = data_file(&dir, "Household.balanced", &["Everyday"]);
    let mut harness = app();
    open(&mut harness, &household);
    harness
        .get_by_role_and_label(Role::Button, "Everyday")
        .click();
    harness.run();

    open(&mut harness, &household);

    // No refusal, and the Register opened before is still open.
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "OK")
            .is_none()
    );
    harness.get_by_role_and_label(Role::Button, "+ Transaction");
}

#[test]
fn a_new_data_file_is_empty_in_usd_and_open() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = dir.path().join("Household.balanced");
    let mut harness = app();

    create(&mut harness, &path);

    assert!(!listed(&harness, "Joint Checking"));
    drop(harness);
    let ledger = Ledger::open(&path).expect("the new file opens");
    let view = ledger.snapshot();
    assert_eq!(view.accounts().len(), 0);
    assert_eq!(view.net_worth().currency(), usd());
}

#[test]
fn a_new_data_file_replaces_another_file_at_its_path() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let old = data_file(&dir, "Household.balanced", &["Everyday"]);
    let mut harness = app();

    create(&mut harness, &old);

    assert!(!listed(&harness, "Everyday"));
    assert!(!listed(&harness, "Joint Checking"));
    drop(harness);
    assert_eq!(accounts_in(&old), Vec::<String>::new());
}

#[test]
fn a_new_data_file_onto_the_open_file_is_refused_and_leaves_it_as_it_was() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let household = data_file(&dir, "Household.balanced", &["Everyday"]);
    let mut harness = app();
    open(&mut harness, &household);

    create(&mut harness, &household);

    harness.get_by_label(
        "“Household.balanced” is the data file you have open, so a new one can't replace it.",
    );
    assert!(listed(&harness, "Everyday"));
    drop(harness);
    assert_eq!(accounts_in(&household), ["Everyday"]);
}

#[test]
fn a_new_data_file_that_cant_be_created_is_refused_with_a_dialog() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = dir.path().join("No such folder").join("Household.balanced");
    let mut harness = app();

    create(&mut harness, &path);

    harness.get_by_label("Balanced couldn't create “Household.balanced”.");
    assert!(listed(&harness, "Joint Checking"));
}

#[test]
fn the_file_menu_offers_new_and_open_data_file() {
    let mut harness = app();

    harness.get_by_role_and_label(Role::Button, "File").click();
    harness.run();

    harness.get_by_role_and_label(Role::Button, "New Data File…");
    harness.get_by_role_and_label(Role::Button, "Open Data File…");
}
