// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Creating a data file, observed by reopening it with plain SQLite.

use balanced_store::{Store, StoreError};
use rusqlite::Connection;
use tempfile::TempDir;

/// A new data file in a temporary directory, reopened with plain SQLite. The directory
/// lives as long as the connection needs it.
fn created_file() -> (TempDir, Connection) {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = dir.path().join("Household.balanced");
    drop(Store::create(&path, "USD").expect("a new path takes a data file"));
    let connection = Connection::open(&path).expect("the created file opens");
    (dir, connection)
}

#[test]
fn a_new_file_is_stamped_as_a_balanced_file() {
    let (_dir, connection) = created_file();

    let application_id: u32 = connection
        .pragma_query_value(None, "application_id", |row| row.get(0))
        .unwrap();

    assert_eq!(application_id.to_be_bytes(), *b"BLNC");
}

#[test]
fn a_new_file_is_at_schema_version_2() {
    let (_dir, connection) = created_file();

    let user_version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();

    assert_eq!(user_version, 2);
}

#[test]
fn a_new_file_holds_its_currency() {
    let (_dir, connection) = created_file();

    let currency: String = connection
        .query_row("SELECT currency FROM data_file", [], |row| row.get(0))
        .unwrap();

    assert_eq!(currency, "USD");
}

#[test]
fn a_new_file_has_the_whole_schema() {
    let (_dir, connection) = created_file();

    let mut statement = connection
        .prepare("SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY type, name")
        .unwrap();
    let schema = statement
        .query_map([], |row| {
            let (kind, name, table, sql): (String, String, String, Option<String>) =
                (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?);
            let sql = sql.unwrap_or_else(|| "(automatic)".to_owned());
            Ok(format!("{kind} {name} on {table}\n{sql}\n"))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .join("\n");

    insta::assert_snapshot!(schema);
}

#[test]
fn creating_refuses_a_path_that_holds_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Notes.txt");
    std::fs::write(&path, "keep me").unwrap();

    let result = Store::create(&path, "USD");

    assert!(matches!(result, Err(StoreError::AlreadyExists(_))));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep me");
}

#[test]
fn a_failed_create_leaves_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Household.balanced");

    let result = Store::create(&path, "dollars");

    assert!(matches!(result, Err(StoreError::Sqlite(_))));
    assert!(!path.exists());
}

#[test]
fn creating_in_a_missing_folder_fails_with_the_io_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Missing").join("Household.balanced");

    let result = Store::create(&path, "USD");

    assert!(matches!(
        result,
        Err(StoreError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound
    ));
}
