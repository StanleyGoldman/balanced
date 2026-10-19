//! The schema, built up by numbered migrations. A file's `user_version` is the number of
//! migrations it has had.

use rusqlite::Transaction;

/// Every migration's SQL, in order: the first is version 1.
const MIGRATIONS: &[&str] = &[include_str!("migration/0001_schema.sql")];

/// The schema version a file has after every migration.
pub(crate) fn latest_version() -> u32 {
    u32::try_from(MIGRATIONS.len()).expect("there are far fewer migrations than u32::MAX")
}

/// Runs, inside `transaction`, every migration after version `from` up to and including
/// version `to`, then records `to` as the file's `user_version`.
pub(crate) fn migrate(transaction: &Transaction<'_>, from: u32, to: u32) -> rusqlite::Result<()> {
    for sql in MIGRATIONS.iter().take(to as usize).skip(from as usize) {
        transaction.execute_batch(sql)?;
    }
    transaction.pragma_update(None, "user_version", to)
}
