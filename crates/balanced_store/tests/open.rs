//! Opening a data file: which files it refuses, that it never writes to a refused file,
//! and the lock it holds while open.

use std::path::{Path, PathBuf};
use std::time::Duration;

use balanced_store::{Damage, OpenError, Store};
use rusqlite::Connection;
use tempfile::TempDir;

/// A path in a new temporary directory, with no file there yet. The directory lives as
/// long as the test holds it.
fn empty_path() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = dir.path().join("Household.balanced");
    (dir, path)
}

/// A new data file in a temporary directory, closed. The directory lives as long as the
/// test holds it.
fn created_file() -> (TempDir, PathBuf) {
    let (dir, path) = empty_path();
    drop(Store::create(&path, "USD").expect("a new path takes a data file"));
    (dir, path)
}

/// Runs `sql` on the file at `path` through plain SQLite, as another program would.
fn alter(path: &Path, sql: &str) {
    Connection::open(path)
        .and_then(|connection| connection.execute_batch(sql))
        .expect("the test's SQL runs");
}

/// Overwrites the second page of the SQLite file at `path`, the root of a table, with
/// bytes that aren't a page. The header and schema on page 1 stay intact, so the file
/// still reads as a Balanced file.
fn damage(path: &Path) {
    let mut bytes = std::fs::read(path).expect("the test made the file");
    // A created file holds several pages, so its 100-byte header and its second page
    // are in range.
    let page_size = usize::from(u16::from_be_bytes([bytes[16], bytes[17]]));
    bytes[page_size..2 * page_size].fill(0xFF);
    std::fs::write(path, bytes).expect("the test's file is writable");
}

/// A plain SQLite connection to the file at `path`, as another program would hold, that
/// gives up at once on a locked file.
fn other_program(path: &Path) -> Connection {
    let connection = Connection::open(path).expect("the test's file opens in SQLite");
    connection
        .busy_timeout(Duration::ZERO)
        .expect("the busy timeout takes zero");
    connection
}

/// Opens the file at `path`, expecting a refusal, and checks the refusal left the file's
/// bytes as they were.
fn refusal(path: &Path) -> OpenError {
    let before = std::fs::read(path).expect("the test made the file");
    let error = Store::open(path).expect_err("the test's file is refused");
    let after = std::fs::read(path).expect("a refusal leaves the file in place");
    assert_eq!(after, before, "a refusal never writes to the file");
    error
}

#[test]
fn a_missing_path_is_refused_as_missing() {
    let (_dir, path) = empty_path();

    let result = Store::open(&path);

    assert!(
        matches!(&result, Err(OpenError::Missing(missing)) if *missing == path),
        "{result:?}"
    );
    assert!(!path.exists());
}

#[test]
fn a_text_file_is_refused_as_not_balanced() {
    let (_dir, path) = empty_path();
    std::fs::write(&path, "Groceries, 12.50\nRent, 950.00\n").unwrap();

    let error = refusal(&path);

    assert!(matches!(error, OpenError::NotBalanced), "{error:?}");
}

#[test]
fn another_programs_sqlite_file_is_refused_as_not_balanced() {
    let (_dir, path) = empty_path();
    alter(
        &path,
        "PRAGMA application_id = 1; CREATE TABLE note (text TEXT);",
    );

    let error = refusal(&path);

    assert!(matches!(error, OpenError::NotBalanced), "{error:?}");
}

#[test]
fn a_healthy_file_opens() {
    let (_dir, path) = created_file();

    let result = Store::open(&path);

    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn a_file_from_a_newer_balanced_is_refused_as_newer() {
    let (_dir, path) = created_file();
    alter(&path, "PRAGMA user_version = 1000");

    let error = refusal(&path);

    assert!(
        matches!(error, OpenError::Newer { version: 1000, .. }),
        "{error:?}"
    );
}

#[test]
fn a_file_with_a_broken_page_is_refused_as_damaged() {
    let (_dir, path) = created_file();
    damage(&path);

    let error = refusal(&path);

    assert!(
        matches!(error, OpenError::Damaged(Damage::Corrupt(_))),
        "{error:?}"
    );
}

#[test]
fn a_file_breaking_its_own_constraints_is_refused_as_damaged() {
    let (_dir, path) = created_file();
    alter(
        &path,
        "PRAGMA ignore_check_constraints = ON; UPDATE data_file SET currency = 'dollars'",
    );

    let error = refusal(&path);

    let OpenError::Damaged(Damage::Problems(problems)) = error else {
        panic!("{error:?}");
    };
    assert_eq!(
        problems,
        ["CHECK constraint failed in data_file"],
        "the check names the broken table"
    );
}

#[test]
fn a_file_another_store_holds_is_refused_as_locked() {
    let (_dir, path) = created_file();
    let _holder = Store::open(&path).unwrap();

    let error = refusal(&path);

    assert!(matches!(error, OpenError::Locked), "{error:?}");
}

#[test]
fn a_file_another_program_holds_is_refused_as_locked() {
    let (_dir, path) = created_file();
    let holder = other_program(&path);
    holder
        .execute_batch("PRAGMA locking_mode = EXCLUSIVE; BEGIN EXCLUSIVE; COMMIT;")
        .unwrap();

    let error = refusal(&path);

    assert!(matches!(error, OpenError::Locked), "{error:?}");
}

#[test]
fn another_program_cannot_read_a_file_a_store_holds() {
    let (_dir, path) = created_file();
    let _store = Store::open(&path).unwrap();

    let result = other_program(&path).query_row("SELECT count(*) FROM account", [], |row| {
        row.get::<_, i64>(0)
    });

    assert_eq!(
        result.unwrap_err().sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy)
    );
}

#[test]
fn a_new_store_holds_its_file() {
    let (_dir, path) = empty_path();
    let _store = Store::create(&path, "USD").unwrap();

    let error = refusal(&path);

    assert!(matches!(error, OpenError::Locked), "{error:?}");
}

#[test]
fn dropping_a_store_lets_its_file_open_again() {
    let (_dir, path) = created_file();
    let store = Store::open(&path).unwrap();

    drop(store);
    let result = Store::open(&path);

    assert!(result.is_ok(), "{result:?}");
}
