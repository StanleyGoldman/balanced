// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The schema, built up by numbered migrations. A file's `user_version` is the number of
//! migrations it has had.

use rusqlite::{Connection, Transaction, TransactionBehavior};

/// One step of the schema: SQL, then an optional Rust function for changes SQL can't
/// make. Both run inside the upgrade's transaction.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub sql: &'static str,
    pub data: Option<fn(&Connection) -> rusqlite::Result<()>>,
    /// Whether the undo log's entries still decode after this step.
    pub undo_format: UndoFormat,
}

/// Whether a migration changes the format of the undo log's entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoFormat {
    /// Entries written before the step still decode after it.
    Kept,
    /// Entries written before the step may not decode after it, so an upgrade through
    /// it clears both stacks.
    Changed,
}

/// Balanced's migrations, in order: the first is version 1. The `.slt` tests run against
/// the last; `tests/slt.rs` says how to test a schema between two of them.
pub const MIGRATIONS: &[Migration] = &[Migration {
    sql: include_str!("migration/0001_schema.sql"),
    data: None,
    undo_format: UndoFormat::Kept,
}];

/// The schema version a file has after every one of `migrations`.
pub(crate) fn latest_version(migrations: &[Migration]) -> u32 {
    u32::try_from(migrations.len()).expect("there are far fewer migrations than u32::MAX")
}

/// Why an upgrade failed. The file is left at its old version.
#[derive(Debug, thiserror::Error)]
pub enum UpgradeError {
    #[error("a migration failed")]
    Sqlite(#[from] rusqlite::Error),
    /// After every step, rows referred to rows that don't exist.
    #[error("the upgraded file breaks {count} foreign keys")]
    ForeignKeys { count: usize },
}

/// Upgrades the file `connection` holds from version `from` through every later one of
/// `migrations`, in one transaction, so the file ends at its old version or the latest.
/// Foreign keys are off while the steps run, so a step can rebuild a table others refer
/// to, and are checked once all of them have.
pub(crate) fn upgrade(
    connection: &mut Connection,
    migrations: &[Migration],
    from: u32,
) -> Result<(), UpgradeError> {
    // `foreign_keys` can't change inside a transaction, so it brackets the upgrade's.
    connection.pragma_update(None, "foreign_keys", false)?;
    let upgraded = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(UpgradeError::from)
        .and_then(|transaction| {
            migrate(&transaction, migrations, from)?;
            Ok(transaction.commit()?)
        });
    let restored = connection.pragma_update(None, "foreign_keys", true);
    // A failed upgrade reports its own cause, not the restore's.
    upgraded.and(restored.map_err(UpgradeError::from))
}

/// Runs, inside `transaction`, every one of `migrations` after version `from`, clears the
/// undo log if one of them changes its format, checks the foreign keys, then records the
/// latest version as the file's `user_version`.
pub(crate) fn migrate(
    transaction: &Transaction<'_>,
    migrations: &[Migration],
    from: u32,
) -> Result<(), UpgradeError> {
    let pending = migrations.get(from as usize..).unwrap_or_default();
    for migration in pending {
        transaction.execute_batch(migration.sql)?;
        if let Some(data) = migration.data {
            data(transaction)?;
        }
    }
    if pending
        .iter()
        .any(|migration| migration.undo_format == UndoFormat::Changed)
    {
        transaction.execute("DELETE FROM undo_entry", [])?;
    }
    let count = transaction
        .prepare("PRAGMA foreign_key_check")?
        .query_map([], |_| Ok(()))?
        .count();
    if count > 0 {
        return Err(UpgradeError::ForeignKeys { count });
    }
    Ok(transaction.pragma_update(None, "user_version", latest_version(migrations))?)
}
