// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Balanced's own migrations, each run on a file at the version before it.

use std::path::{Path, PathBuf};

use balanced_store::{CategoryRow, MIGRATIONS, Store};
use rusqlite::Connection;
use tempfile::TempDir;

/// A data file at version `version` in a temporary directory, changed by `sql` through
/// plain SQLite, as that version's Balanced would have written it. The directory lives
/// as long as the test holds it.
fn file_at(version: usize, sql: &str) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = dir.path().join("Household.balanced");
    drop(
        Store::create_with_migrations(&path, "USD", &MIGRATIONS[..version])
            .expect("a new path takes a data file"),
    );
    let connection = Connection::open(&path).expect("the test's file opens in SQLite");
    connection
        .execute_batch(sql)
        .expect("the test's SQL fits the schema");
    drop(connection);
    (dir, path)
}

/// The name and type of each category in the file at `path`, once Balanced has opened
/// it, by id.
fn category_types(path: &Path) -> Vec<(String, String)> {
    Store::open(path)
        .expect("the upgrade succeeds")
        .read()
        .expect("the upgraded file reads")
        .categories
        .into_iter()
        .map(|row| (row.name, row.category_type))
        .collect()
}

/// An account, Everyday, at version 1, with the categories and usage the category type
/// tests read.
const VERSION_1_CATEGORIES: &str = r#"
    INSERT INTO account (id, name, kind, position, hidden, closed)
        VALUES (0, 'Everyday', 'checking', 0, 0, 0);
    INSERT INTO category (id, parent_id, name) VALUES
        (0, NULL, 'Income'), (1, 0, 'Salary'),
        (2, NULL, 'Food'), (3, 2, 'Groceries'),
        (4, NULL, 'Refunds'),
        (5, NULL, 'Unused'),
        (6, NULL, 'Even'),
        (7, NULL, 'Gifts'), (8, 7, 'Received'), (9, 8, 'Cash');
    INSERT INTO "transaction" (id, account_id, date, payee, memo, allocation, amount, category_id)
        VALUES
        (0, 0, '2026-10-01', 'Acme', '', 'category', 300000, 1),
        (1, 0, '2026-10-02', 'Shop', '', 'category', 1000, 2),
        (2, 0, '2026-10-03', 'Shop', '', 'split', NULL, NULL),
        (3, 0, '2026-10-04', 'Even', '', 'category', 1000, 6),
        (4, 0, '2026-10-05', 'Even', '', 'category', -1000, 6),
        (5, 0, '2026-10-06', 'Gran', '', 'category', 10000, 9);
    INSERT INTO split_line (transaction_id, position, category_id, memo, amount) VALUES
        (2, 0, 3, '', -5000),
        (2, 1, 4, '', 2000);
"#;

#[test]
fn the_category_type_migration_infers_each_top_level_categorys_type_from_its_usage() {
    let (_dir, path) = file_at(1, VERSION_1_CATEGORIES);

    let types = category_types(&path);

    let expected = [
        // Its subcategory's transactions add up to more than zero.
        ("Income", "income"),
        ("Salary", "income"),
        // Its own refund and its subcategory's split line add up to less than zero.
        ("Food", "expense"),
        ("Groceries", "expense"),
        // A split line alone, more than zero.
        ("Refunds", "income"),
        ("Unused", "expense"),
        // Exactly zero.
        ("Even", "expense"),
        // A subcategory two levels down counts, and takes the top's type.
        ("Gifts", "income"),
        ("Received", "income"),
        ("Cash", "income"),
    ]
    .map(|(name, category_type)| (name.to_owned(), category_type.to_owned()));
    assert_eq!(types, expected);
}

#[test]
fn the_category_type_migration_clears_the_undo_log() {
    let (_dir, path) = file_at(
        1,
        r#"INSERT INTO undo_entry (stack, position, command) VALUES
            ('undo', 0, '{"delete_category":{"category":1}}'),
            ('redo', 0, '{"restore_category":{"category":{"id":1,"name":"Fuel","parent":null}}}');"#,
    );

    let contents = Store::open(&path)
        .expect("the upgrade succeeds")
        .read()
        .expect("the upgraded file reads");

    assert_eq!((contents.undo, contents.redo), (Vec::new(), Vec::new()));
}

#[test]
fn a_category_written_after_the_migration_keeps_its_type() {
    let (_dir, path) = file_at(2, "");
    let store = Store::open(&path).expect("the file opens");
    store
        .insert_category(&CategoryRow {
            id: 0,
            parent_id: None,
            name: "Salary".to_owned(),
            category_type: "income".to_owned(),
        })
        .expect("the file takes a category");
    drop(store);

    let types = category_types(&path);

    assert_eq!(types, [("Salary".to_owned(), "income".to_owned())]);
}
