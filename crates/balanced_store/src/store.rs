//! Creating a data file.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, TransactionBehavior};

use crate::migration;

/// The application ID stamped in every data file's header: ASCII `BLNC`.
const APPLICATION_ID: u32 = u32::from_be_bytes(*b"BLNC");

/// An open data file.
#[derive(Debug)]
pub struct Store {
    #[expect(
        dead_code,
        reason = "TODO(#239): Ledger reads and writes rows through it"
    )]
    connection: Connection,
}

/// Why a data file couldn't be created.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("a file already exists at {0:?}")]
    AlreadyExists(PathBuf),
    #[error("sqlite failed")]
    Sqlite(#[from] rusqlite::Error),
}

impl Store {
    /// Creates an empty data file at `path` whose amounts are in `currency`, an ISO 4217
    /// code such as `"USD"`. Refuses a path that already holds a file, and leaves no file
    /// behind when creating fails.
    pub fn create(path: &Path, currency: &str) -> Result<Self, StoreError> {
        if path.exists() {
            return Err(StoreError::AlreadyExists(path.to_owned()));
        }
        Self::create_new(path, currency).inspect_err(|_| {
            // The file is half made and nobody has seen it, so it goes. Failing to remove
            // it leaves an empty or partial file, which opening later refuses.
            fs::remove_file(path).ok();
        })
    }

    /// Creates the file at `path`, which holds nothing yet.
    fn create_new(path: &Path, currency: &str) -> Result<Self, StoreError> {
        let mut connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.pragma_update(None, "application_id", APPLICATION_ID)?;
        migration::migrate(&transaction, 0, migration::latest_version())?;
        transaction.execute(
            "INSERT INTO data_file (id, currency) VALUES (1, ?1)",
            [currency],
        )?;
        transaction.commit()?;
        Ok(Self { connection })
    }
}
