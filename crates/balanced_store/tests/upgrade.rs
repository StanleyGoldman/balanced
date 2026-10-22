// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Upgrading a data file from an older Balanced, driven by migrations the tests define on
//! top of Balanced's own: the backup taken first, the upgrade itself, and what a failure
//! leaves behind.

use std::path::{Path, PathBuf};

use balanced_store::{
    AccountRow, MIGRATIONS, Migration, OpenError, Stack, Store, UndoFormat, UpgradeError,
};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};
use tempfile::TempDir;

/// The first step after Balanced's own: every account gains a nickname, its name in
/// capitals.
const NICKNAMES: Migration = Migration {
    sql: "ALTER TABLE account ADD COLUMN nickname TEXT NOT NULL DEFAULT ''",
    data: Some(|connection| {
        connection.execute("UPDATE account SET nickname = upper(name)", [])?;
        Ok(())
    }),
    undo_format: UndoFormat::Kept,
};

/// The second step after Balanced's own: a table of tags, with no Rust step.
const TAGS: Migration = Migration {
    sql: "CREATE TABLE tag (id INTEGER PRIMARY KEY, name TEXT NOT NULL) STRICT",
    data: None,
    undo_format: UndoFormat::Kept,
};

/// Balanced's own migrations, then `extra`.
fn migrations(extra: &[Migration]) -> Vec<Migration> {
    MIGRATIONS.iter().chain(extra).copied().collect()
}

/// The schema version of a file that has had every one of Balanced's own migrations.
fn current() -> usize {
    MIGRATIONS.len()
}

/// The name of the backup of `Household.balanced` taken before upgrading it from the
/// current version.
fn backup_name() -> String {
    format!("Household (before upgrade, v{}).balanced", current())
}

/// A data file at the current version in a temporary directory, holding one account,
/// closed. The directory lives as long as the test holds it.
fn current_file() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = dir.path().join("Household.balanced");
    let store = Store::create(&path, "USD").expect("a new path takes a data file");
    store
        .insert_account(&AccountRow {
            id: 1,
            name: "Everyday".to_owned(),
            kind: "checking".to_owned(),
            position: 0,
            hidden: false,
            closed: false,
        })
        .expect("a new file takes an account");
    (dir, path)
}

/// Runs `sql` on the file at `path` through plain SQLite, read-only, returning the first
/// column of every row as text.
fn query(path: &Path, sql: &str) -> Vec<String> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("the test's file opens in SQLite");
    connection
        .prepare(sql)
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get_ref(0).map(text))?
                .collect()
        })
        .expect("the test's SQL runs")
}

/// A value as text: `NULL` for null.
fn text(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => "NULL".to_owned(),
        ValueRef::Integer(n) => n.to_string(),
        ValueRef::Real(x) => x.to_string(),
        ValueRef::Text(text) => String::from_utf8_lossy(text).into_owned(),
        ValueRef::Blob(blob) => format!("<{} byte blob>", blob.len()),
    }
}

fn user_version(path: &Path) -> String {
    query(path, "PRAGMA user_version").concat()
}

#[test]
fn opening_an_older_file_upgrades_it_to_the_latest_version() {
    let (_dir, path) = current_file();

    let store = Store::open_with_migrations(&path, &migrations(&[NICKNAMES, TAGS]));
    drop(store.expect("the upgrade succeeds"));

    assert_eq!(user_version(&path), (current() + 2).to_string());
    assert_eq!(query(&path, "PRAGMA integrity_check"), ["ok"]);
    assert_eq!(
        query(&path, "PRAGMA foreign_key_check"),
        Vec::<String>::new()
    );
    assert_eq!(query(&path, "SELECT nickname FROM account"), ["EVERYDAY"]);
    assert_eq!(query(&path, "SELECT count(*) FROM tag"), ["0"]);
}

#[test]
fn an_upgrade_first_backs_the_file_up_beside_it() {
    let (dir, path) = current_file();

    drop(Store::open_with_migrations(&path, &migrations(&[NICKNAMES, TAGS])).unwrap());

    let backup = dir.path().join(backup_name());
    assert!(backup.exists());
    assert_eq!(user_version(&backup), current().to_string());
    assert_eq!(query(&backup, "SELECT name FROM account"), ["Everyday"]);
    let reopened = Store::open(&backup);
    assert!(reopened.is_ok(), "{reopened:?}");
}

/// A step whose SQL fails.
const BROKEN: Migration = Migration {
    sql: "UPDATE no_such_table SET name = ''",
    data: None,
    undo_format: UndoFormat::Kept,
};

/// Opens the file at `path` with Balanced's migrations and `extra`, expecting the upgrade
/// to fail, and checks it left the file at its version with its backup kept.
fn failed_upgrade(dir: &TempDir, path: &Path, extra: &[Migration]) -> UpgradeError {
    let error =
        Store::open_with_migrations(path, &migrations(extra)).expect_err("the upgrade fails");

    let OpenError::UpgradeFailed { backup, source } = error else {
        panic!("{error:?}");
    };
    assert_eq!(user_version(path), current().to_string());
    assert_eq!(
        query(
            path,
            "SELECT name FROM pragma_table_info('account') WHERE name = 'nickname'"
        ),
        Vec::<String>::new(),
        "the steps before the failing one are rolled back too"
    );
    assert_eq!(backup, dir.path().join(backup_name()));
    assert_eq!(user_version(&backup), current().to_string());
    source
}

#[test]
fn a_failing_step_rolls_the_whole_upgrade_back() {
    let (dir, path) = current_file();

    let error = failed_upgrade(&dir, &path, &[NICKNAMES, BROKEN]);

    assert!(matches!(error, UpgradeError::Sqlite(_)), "{error:?}");
}

#[test]
fn a_failing_rust_step_rolls_the_whole_upgrade_back() {
    let (dir, path) = current_file();
    let failing = Migration {
        sql: "",
        data: Some(|connection| {
            connection.execute("DELETE FROM no_such_table", [])?;
            Ok(())
        }),
        undo_format: UndoFormat::Kept,
    };

    let error = failed_upgrade(&dir, &path, &[NICKNAMES, failing]);

    assert!(matches!(error, UpgradeError::Sqlite(_)), "{error:?}");
}

#[test]
fn an_upgrade_that_breaks_a_foreign_key_rolls_back() {
    let (dir, path) = current_file();
    let orphan = Migration {
        sql: "INSERT INTO \"transaction\" (id, account_id, date, payee, memo, allocation, amount)
              VALUES (1, 99, '2026-10-19', '', '', 'opening_balance', 0)",
        data: None,
        undo_format: UndoFormat::Kept,
    };

    let error = failed_upgrade(&dir, &path, &[NICKNAMES, orphan]);

    assert!(
        matches!(error, UpgradeError::ForeignKeys { count: 1 }),
        "{error:?}"
    );
}

#[test]
fn an_upgrade_whose_backup_cant_be_written_is_refused_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    // A name of 239 bytes fits the usual 255-byte limit, with room for SQLite's
    // `-journal`, but the backup's longer name doesn't. A read-only directory would
    // stop the backup too, but not when the tests run as root, as they do in CI.
    let path = dir.path().join(format!("{}.balanced", "H".repeat(230)));
    drop(Store::create(&path, "USD").expect("a long name still takes a data file"));
    let before = std::fs::read(&path).unwrap();

    let error = Store::open_with_migrations(&path, &migrations(&[NICKNAMES]))
        .expect_err("the backup fails");

    assert!(matches!(error, OpenError::BackupFailed(_)), "{error:?}");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "the file isn't upgraded"
    );
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        1,
        "nothing is left beside the file"
    );
}

#[test]
fn an_earlier_backup_is_never_replaced() {
    let (dir, path) = current_file();
    let earlier = dir.path().join(backup_name());
    std::fs::write(&earlier, "an earlier backup").unwrap();

    drop(Store::open_with_migrations(&path, &migrations(&[NICKNAMES])).unwrap());

    assert_eq!(
        std::fs::read_to_string(&earlier).unwrap(),
        "an earlier backup"
    );
    let backup = dir.path().join(format!(
        "Household (before upgrade, v{}) 2.balanced",
        current()
    ));
    assert_eq!(user_version(&backup), current().to_string());
}

/// A data file at the current version, with one entry on each undo stack.
fn current_file_with_history() -> (TempDir, PathBuf) {
    let (dir, path) = current_file();
    let store = Store::open(&path).expect("the test's file opens");
    store
        .push_entry(Stack::Undo, r#"{"delete_account":{"account":1}}"#)
        .expect("the file takes a JSON entry");
    store
        .push_entry(Stack::Redo, r#"{"delete_account":{"account":2}}"#)
        .expect("the file takes a JSON entry");
    drop(store);
    (dir, path)
}

#[test]
fn an_upgrade_that_keeps_the_undo_format_keeps_the_undo_log() {
    let (_dir, path) = current_file_with_history();

    let store = Store::open_with_migrations(&path, &migrations(&[NICKNAMES, TAGS])).unwrap();

    let contents = store.read().unwrap();
    assert_eq!(contents.undo, [r#"{"delete_account":{"account":1}}"#]);
    assert_eq!(contents.redo, [r#"{"delete_account":{"account":2}}"#]);
}

#[test]
fn an_upgrade_through_a_step_that_changes_the_undo_format_clears_both_stacks() {
    let (_dir, path) = current_file_with_history();
    let new_format = Migration {
        undo_format: UndoFormat::Changed,
        ..TAGS
    };

    let store = Store::open_with_migrations(&path, &migrations(&[NICKNAMES, new_format])).unwrap();

    let contents = store.read().unwrap();
    assert_eq!((contents.undo, contents.redo), (Vec::new(), Vec::new()));
}

#[test]
fn a_step_the_file_already_had_doesnt_clear_the_undo_log() {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = dir.path().join("Household.balanced");
    let new_format = Migration {
        undo_format: UndoFormat::Changed,
        ..NICKNAMES
    };
    let store = Store::create_with_migrations(&path, "USD", &migrations(&[new_format])).unwrap();
    store
        .push_entry(Stack::Undo, r#"{"delete_account":{"account":1}}"#)
        .unwrap();
    drop(store);

    let store = Store::open_with_migrations(&path, &migrations(&[new_format, TAGS])).unwrap();

    assert_eq!(
        store.read().unwrap().undo,
        [r#"{"delete_account":{"account":1}}"#]
    );
}

#[test]
fn an_upgrade_can_stop_at_an_intermediate_version() {
    let (_dir, path) = current_file();
    let all = migrations(&[NICKNAMES, TAGS]);

    drop(Store::open_with_migrations(&path, &all[..=current()]).unwrap());

    assert_eq!(user_version(&path), (current() + 1).to_string());
    assert_eq!(query(&path, "SELECT nickname FROM account"), ["EVERYDAY"]);
    assert_eq!(
        query(&path, "SELECT name FROM sqlite_schema WHERE name = 'tag'"),
        Vec::<String>::new()
    );
}
