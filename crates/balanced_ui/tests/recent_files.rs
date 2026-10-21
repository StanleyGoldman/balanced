// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The Recent files: opening or creating a data file moves it to the top of
//! `recent-files.toml`, the File menu lists them, and each file reopens on the view it
//! closed on. The native file picker can't be driven, so a test that picks a file
//! sends the Command the picker sends.

use std::path::{Path, PathBuf};

use balanced_core::{AccountId, AccountStatus, AccountSummary, DomainCommand, Ledger};
use balanced_i18n::Localizer;
use balanced_ui::testing;
use balanced_ui::{App, Clock, PreferencesFile, RecentFilesFile};
use eframe::egui::accesskit::{Role, Toggled};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use icu_calendar::Date;
use tempfile::TempDir;

/// The App started with its config directory at `config`, in the test locale.
fn app(config: &Path) -> Harness<'static, App> {
    let config = config.to_owned();
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .build_eframe(move |cc| {
            App::new(
                cc,
                Localizer::for_tests(),
                Clock::fixed(Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")),
                PreferencesFile::for_tests(),
                RecentFilesFile::in_dir(&config),
            )
        });
    harness.run();
    harness
}

/// Writes `files`, newest first, as the Recent files in the config directory `config`,
/// with no saved views.
fn recent_files(config: &Path, files: &[&Path]) {
    let entries: Vec<_> = files
        .iter()
        .map(|file| format!("[[files]]\npath = \"{}\"\n", file.display()))
        .collect();
    let entries = entries.concat();
    std::fs::write(
        config.join("recent-files.toml"),
        format!("version = 2\n{entries}"),
    )
    .expect("the temp dir is writable");
}

/// The entries of `recent-files.toml` in the config directory `config`, newest first.
fn entries_in(config: &Path) -> Vec<toml::Value> {
    let text = std::fs::read_to_string(config.join("recent-files.toml"))
        .expect("the App wrote the recent files file");
    let mut table: toml::Table = text.parse().expect("the App writes valid TOML");
    assert_eq!(table["version"].as_integer(), Some(2));
    match table.remove("files") {
        Some(toml::Value::Array(files)) => files,
        files => panic!("the files are an array: {files:?}"),
    }
}

/// The files `recent-files.toml` in the config directory `config` lists, newest first.
fn listed_in(config: &Path) -> Vec<PathBuf> {
    entries_in(config)
        .iter()
        .map(|entry| PathBuf::from(entry["path"].as_str().expect("a path is text")))
        .collect()
}

/// The paths of `files`, as `listed_in` returns them.
fn listing(files: &[&Path]) -> Vec<PathBuf> {
    files.iter().map(|file| file.to_path_buf()).collect()
}

/// A temporary directory, by its resolved path, so the paths in it are written as the
/// App lists them.
fn temp_dir() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = dir.path().canonicalize().expect("the temp dir resolves");
    (dir, path)
}

#[test]
fn opening_a_file_puts_it_at_the_top() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    let business = testing::data_file(&dir, "Business.balanced", &["Payroll"]);
    recent_files(&dir, &[&household]);
    let mut harness = app(&dir);

    testing::open_data_file(&harness, &business);
    harness.run();

    assert_eq!(listed_in(&dir), listing(&[&business, &household]));
}

#[test]
fn a_file_is_listed_by_its_resolved_path_once() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    let business = testing::data_file(&dir, "Business.balanced", &["Payroll"]);
    recent_files(&dir, &[&household, &business]);
    let mut harness = app(&dir);

    testing::open_data_file(&harness, &dir.join(".").join("Business.balanced"));
    harness.run();

    assert_eq!(listed_in(&dir), listing(&[&business, &household]));
}

#[test]
fn creating_a_file_puts_it_at_the_top() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    recent_files(&dir, &[&household]);
    let mut harness = app(&dir);
    let new = dir.join("New.balanced");

    testing::new_data_file(&harness, &new);
    harness.run();

    assert_eq!(listed_in(&dir), listing(&[&new, &household]));
}

#[test]
fn a_missing_file_stays_listed() {
    let (_dir, dir) = temp_dir();
    let gone = dir.join("Gone.balanced");
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    recent_files(&dir, &[&gone]);
    let mut harness = app(&dir);

    testing::open_data_file(&harness, &household);
    harness.run();

    assert_eq!(listed_in(&dir), listing(&[&household, &gone]));
}

#[test]
fn the_list_keeps_the_four_newest() {
    let (_dir, dir) = temp_dir();
    let [one, two, three, four, five] = ["One", "Two", "Three", "Four", "Five"]
        .map(|name| testing::data_file(&dir, &format!("{name}.balanced"), &[]));
    recent_files(&dir, &[&one, &two, &three, &four]);
    let mut harness = app(&dir);

    testing::open_data_file(&harness, &five);
    harness.run();

    assert_eq!(listed_in(&dir), listing(&[&five, &one, &two, &three]));
}

/// Opens the File menu.
fn open_file_menu(harness: &mut Harness<'static, App>) {
    harness.get_by_role_and_label(Role::Button, "File").click();
    harness.run();
}

#[test]
fn the_file_menu_lists_the_recent_files_numbered_above_quit() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    let business = testing::data_file(&dir, "Business.balanced", &["Payroll"]);
    recent_files(&dir, &[&household, &business]);
    let mut harness = app(&dir);

    open_file_menu(&mut harness);

    let items: Vec<String> = harness
        .get_all_by_role(Role::Button)
        .filter_map(|node| node.accesskit_node().label())
        .collect();
    let at = |label: &str| {
        items
            .iter()
            .position(|item| item == label)
            .unwrap_or_else(|| panic!("the File menu has no {label:?}: {items:?}"))
    };
    assert_eq!(at("1 Household.balanced") + 1, at("2 Business.balanced"));
    assert_eq!(at("2 Business.balanced") + 1, at("Quit Ctrl+Q"));
}

#[test]
fn clicking_a_recent_file_opens_it() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    let business = testing::data_file(&dir, "Business.balanced", &["Payroll"]);
    recent_files(&dir, &[&household, &business]);
    let mut harness = app(&dir);
    open_file_menu(&mut harness);

    harness
        .get_by_role_and_label(Role::Button, "2 Business.balanced")
        .click();
    harness.run();

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Payroll")
            .is_some()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Everyday")
            .is_none()
    );
    assert_eq!(listed_in(&dir), listing(&[&business, &household]));
}

/// Opens the data file at `path` through the File menu's Recent files, where it is
/// listed `number`th.
fn reopen(harness: &mut Harness<'static, App>, number: u32, path: &Path) {
    open_file_menu(harness);
    let name = path.file_name().expect("a data file has a name").display();
    harness
        .get_by_role_and_label(Role::Button, &format!("{number} {name}"))
        .click();
    harness.run();
}

/// Clicks the button labelled `label`: a tab, or an account in the Account bar.
fn click(harness: &mut Harness<'static, App>, label: &str) {
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run();
}

/// The tabs in the tab row, in order.
const TABS: [&str; 7] = [
    "Home",
    "Accounts",
    "Bills",
    "Spending",
    "Planning",
    "Investing",
    "Property & Debt",
];

/// The label of the tab the tab row shows as selected.
fn selected_tab(harness: &Harness<'static, App>) -> &'static str {
    let selected: Vec<_> = TABS
        .into_iter()
        .filter(|tab| {
            harness
                .get_by_role_and_label(Role::Button, tab)
                .accesskit_node()
                .toggled()
                == Some(Toggled::True)
        })
        .collect();
    match selected[..] {
        [tab] => tab,
        _ => panic!("one tab is selected: {selected:?}"),
    }
}

/// Whether the Main view shows the Register of the account named `account`, whose
/// name heads it.
fn shows_register(harness: &Harness<'static, App>, account: &str) -> bool {
    harness
        .query_by_role_and_label(Role::Label, account)
        .is_some()
}

#[test]
fn a_file_reopens_on_the_account_it_was_left_on() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday", "Savings"]);
    let business = testing::data_file(&dir, "Business.balanced", &["Payroll"]);
    recent_files(&dir, &[&household]);
    let mut harness = app(&dir);
    click(&mut harness, "Savings");
    testing::open_data_file(&harness, &business);
    harness.run();
    click(&mut harness, "Home");

    reopen(&mut harness, 2, &household);

    assert_eq!(selected_tab(&harness), "Accounts");
    assert!(shows_register(&harness, "Savings"));
}

#[test]
fn a_file_reopens_on_the_tab_it_was_left_on() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    let business = testing::data_file(&dir, "Business.balanced", &["Payroll"]);
    recent_files(&dir, &[&household]);
    let mut harness = app(&dir);
    click(&mut harness, "Bills");
    testing::open_data_file(&harness, &business);
    harness.run();
    click(&mut harness, "Planning");

    reopen(&mut harness, 2, &household);

    assert_eq!(selected_tab(&harness), "Bills");
}

#[test]
fn a_file_with_no_saved_view_opens_on_the_tab_shown() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    let business = testing::data_file(&dir, "Business.balanced", &["Payroll"]);
    recent_files(&dir, &[&household]);
    let mut harness = app(&dir);
    click(&mut harness, "Everyday");

    testing::open_data_file(&harness, &business);
    harness.run();

    assert_eq!(selected_tab(&harness), "Accounts");
    assert!(!shows_register(&harness, "Everyday"));
    assert!(!shows_register(&harness, "Payroll"));
}

#[test]
fn exiting_saves_where_the_file_was_left_for_the_next_start() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday", "Savings"]);
    recent_files(&dir, &[&household]);
    let mut harness = app(&dir);
    click(&mut harness, "Savings");

    eframe::App::on_exit(harness.state_mut(), None);
    drop(harness);
    let harness = app(&dir);

    assert_eq!(selected_tab(&harness), "Accounts");
    assert!(shows_register(&harness, "Savings"));
}

/// Writes `file` as the only Recent file in the config directory `config`, saved as
/// closed on the Register of the account numbered `account`.
fn left_on_account(config: &Path, file: &Path, account: u32) {
    std::fs::write(
        config.join("recent-files.toml"),
        format!(
            "version = 2\n\
             [[files]]\n\
             path = \"{}\"\n\
             last_view = {{ tab = \"accounts\", account = {account} }}\n",
            file.display()
        ),
    )
    .expect("the temp dir is writable");
}

#[test]
fn an_account_no_longer_in_the_file_opens_the_accounts_tab_with_no_register() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    left_on_account(&dir, &household, 99);

    let mut harness = app(&dir);

    assert_eq!(selected_tab(&harness), "Accounts");
    harness.get_by_label("Select an account in the Account bar to open its Register.");
    eframe::App::on_exit(harness.state_mut(), None);
    let last_view = &entries_in(&dir)[0]["last_view"];
    assert_eq!(
        last_view.get("tab").and_then(|tab| tab.as_str()),
        Some("accounts")
    );
    assert_eq!(last_view.get("account"), None);
}

/// Closes the account named `name` in the data file at `file`, and returns its id.
fn close_account(file: &Path, name: &str) -> AccountId {
    let mut ledger = Ledger::open(file).expect("the test made the data file");
    let account = ledger
        .snapshot()
        .accounts()
        .iter()
        .map(AccountSummary::account)
        .find(|account| account.name() == name)
        .expect("the test made the account")
        .id();
    ledger
        .apply(DomainCommand::SetAccountStatus {
            account,
            status: AccountStatus::Closed,
        })
        .expect("an open account closes");
    account
}

#[test]
fn a_closed_account_reopens_like_any_other() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday", "Old"]);
    let old = close_account(&household, "Old");
    left_on_account(&dir, &household, u32::from(old));

    let harness = app(&dir);

    assert_eq!(selected_tab(&harness), "Accounts");
    assert!(shows_register(&harness, "Old"));
}

#[test]
fn a_new_file_over_a_listed_one_forgets_its_saved_view() {
    let (_dir, dir) = temp_dir();
    let household = testing::data_file(&dir, "Household.balanced", &["Everyday"]);
    let business = testing::data_file(&dir, "Business.balanced", &["Payroll"]);
    recent_files(&dir, &[&household, &business]);
    let mut harness = app(&dir);
    click(&mut harness, "Everyday");
    testing::open_data_file(&harness, &business);
    harness.run();
    click(&mut harness, "Bills");

    testing::new_data_file(&harness, &household);
    harness.run();

    assert_eq!(selected_tab(&harness), "Bills");
}
