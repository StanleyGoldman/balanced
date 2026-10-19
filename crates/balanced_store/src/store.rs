//! Creating and opening a data file, and the transactions every write runs in.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, TransactionBehavior};

use crate::migration;

/// The application ID stamped in every data file's header: ASCII `BLNC`.
const APPLICATION_ID: u32 = u32::from_be_bytes(*b"BLNC");

/// An open data file.
#[derive(Debug)]
pub struct Store {
    pub(crate) connection: Connection,
}

/// Why a data file couldn't be created, opened, read or written.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("a file already exists at {0:?}")]
    AlreadyExists(PathBuf),
    #[error("couldn't create the file at {path:?}")]
    Io { path: PathBuf, source: io::Error },
    #[error("sqlite failed")]
    Sqlite(#[from] rusqlite::Error),
}

impl Store {
    /// Creates an empty data file at `path` whose amounts are in `currency`, an ISO 4217
    /// code such as `"USD"`. Refuses a path that already holds a file, and leaves no file
    /// behind when creating fails.
    pub fn create(path: &Path, currency: &str) -> Result<Self, StoreError> {
        // Creating the file exclusively, before SQLite opens it, means the file this
        // removes on failure is always the one made here.
        File::create_new(path).map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                StoreError::AlreadyExists(path.to_owned())
            } else {
                StoreError::Io {
                    path: path.to_owned(),
                    source,
                }
            }
        })?;
        Connection::open(path)
            .map_err(StoreError::from)
            .and_then(|connection| Self::write_schema(connection, currency))
            .inspect_err(|_| {
                // The file is half made and nobody has seen it, so it goes. Failing to remove
                // it leaves an empty or partial file, which opening later refuses.
                fs::remove_file(path).ok();
            })
    }

    /// An empty data file in memory, gone when the store is dropped, whose amounts are
    /// in `currency`.
    pub fn create_in_memory(currency: &str) -> Result<Self, StoreError> {
        Self::write_schema(Connection::open_in_memory()?, currency)
    }

    /// Opens the data file at `path` for reading and writing.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        // Without `SQLITE_OPEN_CREATE`, a missing file is an error rather than a new,
        // empty database.
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.pragma_update(None, "foreign_keys", true)?;
        Ok(Self { connection })
    }

    /// Starts a transaction that every write until [`Self::commit`] or
    /// [`Self::rollback`] joins.
    pub fn begin(&self) -> Result<(), StoreError> {
        Ok(self.connection.execute_batch("BEGIN IMMEDIATE")?)
    }

    /// Keeps every write since [`Self::begin`].
    pub fn commit(&self) -> Result<(), StoreError> {
        Ok(self.connection.execute_batch("COMMIT")?)
    }

    /// Undoes every write since [`Self::begin`]. Does nothing when no transaction is
    /// open, as after a failed commit SQLite rolled back itself.
    pub fn rollback(&self) -> Result<(), StoreError> {
        if self.connection.is_autocommit() {
            return Ok(());
        }
        Ok(self.connection.execute_batch("ROLLBACK")?)
    }

    /// Fills the empty database `connection` holds with the latest schema and the file's currency, in
    /// one transaction.
    fn write_schema(mut connection: Connection, currency: &str) -> Result<Self, StoreError> {
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
