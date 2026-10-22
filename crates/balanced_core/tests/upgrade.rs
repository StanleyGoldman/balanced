// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! A `Ledger` over a data file from an older Balanced: what the upgrade on open keeps of
//! the undo log, driven by migrations the tests define on top of Balanced's own.

use std::path::PathBuf;

use balanced_core::{AccountKind, CurrencyCode, DomainCommand, Ledger, Money};
use balanced_store::{MIGRATIONS, Migration, UndoFormat};
use icu_calendar::Date;
use tempfile::TempDir;

/// A step after Balanced's own: a table of tags, which leaves the undo log's format as
/// it was.
const TAGS: Migration = Migration {
    sql: "CREATE TABLE tag (id INTEGER PRIMARY KEY, name TEXT NOT NULL) STRICT",
    data: None,
    undo_format: UndoFormat::Kept,
};

fn usd_code() -> CurrencyCode {
    CurrencyCode::new("USD").expect("USD is an ISO 4217 currency")
}

/// A data file at the current version in a temporary directory, where account Everyday
/// was created then Joint was created and undone, closed. The directory lives as long as
/// the test holds it.
fn current_file_with_history() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = dir.path().join("Household.balanced");
    let mut ledger = Ledger::create(&path, usd_code()).expect("a new path takes a data file");
    for name in ["Everyday", "Joint"] {
        ledger
            .apply(DomainCommand::CreateAccount {
                name: name.to_owned(),
                kind: AccountKind::Checking,
                opened: Date::try_new_iso(2026, 10, 19).expect("a real date"),
                opening_balance: Money::zero(usd_code()),
            })
            .expect("a new file takes an account");
    }
    ledger.undo().expect("the second create undoes");
    (dir, path)
}

/// Opens the file at `path` with Balanced's migrations and then `extra`.
fn open_upgraded(path: &std::path::Path, extra: &[Migration]) -> Ledger {
    let migrations: Vec<Migration> = MIGRATIONS.iter().chain(extra).copied().collect();
    Ledger::open_with_migrations(path, &migrations).expect("the upgrade succeeds")
}

fn names(ledger: &Ledger) -> Vec<String> {
    ledger
        .snapshot()
        .accounts()
        .iter()
        .map(|summary| summary.account().name().to_owned())
        .collect()
}

#[test]
fn undo_and_redo_work_after_an_upgrade_that_keeps_the_undo_format() {
    let (_dir, path) = current_file_with_history();

    let mut ledger = open_upgraded(&path, &[TAGS]);

    ledger.redo().expect("the undone create redoes");
    assert_eq!(names(&ledger), ["Everyday", "Joint"]);
    ledger.undo().expect("the redone create undoes");
    ledger.undo().expect("the first create undoes");
    assert_eq!(names(&ledger), Vec::<String>::new());
}

#[test]
fn an_upgrade_that_changes_the_undo_format_leaves_nothing_to_undo_or_redo() {
    let (_dir, path) = current_file_with_history();
    let new_format = Migration {
        undo_format: UndoFormat::Changed,
        ..TAGS
    };

    let ledger = open_upgraded(&path, &[new_format]);

    let snapshot = ledger.snapshot();
    assert_eq!((snapshot.can_undo(), snapshot.can_redo()), (false, false));
    assert_eq!(names(&ledger), ["Everyday"]);
}
