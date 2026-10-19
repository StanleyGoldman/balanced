//! Creating and opening a data file, and the transactions every write runs in.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, TransactionBehavior};

use crate::migration::{self, MIGRATIONS, Migration, UpgradeError};

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

/// Why a data file couldn't be opened. Every refusal leaves the file as it was.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    /// No file exists at the path.
    #[error("no file exists at {0:?}")]
    Missing(PathBuf),
    /// The file isn't SQLite, or another program's SQLite file.
    #[error("the file isn't a Balanced data file")]
    NotBalanced,
    /// The file fails SQLite's integrity check.
    #[error("the data file is damaged")]
    Damaged(#[source] Damage),
    /// The file's schema is from a newer Balanced than this one.
    #[error("the data file is at schema version {version}, above this Balanced's {latest}")]
    Newer { version: u32, latest: u32 },
    /// Another connection, in this program or another, holds the file.
    #[error("the data file is open elsewhere")]
    Locked,
    /// The file is from an older Balanced, and the backup taken before upgrading it
    /// couldn't be written, so it wasn't upgraded.
    #[error("the data file couldn't be backed up before its upgrade")]
    BackupFailed(#[source] StoreError),
    /// Upgrading the file from an older Balanced failed, so it was left at its old
    /// version. The backup taken first stays at `backup`.
    #[error("the data file couldn't be upgraded; its backup is at {backup:?}")]
    UpgradeFailed {
        backup: PathBuf,
        #[source]
        source: UpgradeError,
    },
    /// SQLite failed for a reason that isn't one of the refusals above.
    #[error("the data file couldn't be read")]
    Store(#[from] StoreError),
}

/// What is wrong with a damaged data file.
#[derive(Debug, thiserror::Error)]
pub enum Damage {
    /// `PRAGMA quick_check` listed these problems.
    #[error("the integrity check found problems: {0:?}")]
    Problems(Vec<String>),
    /// SQLite found the file corrupt while reading it.
    #[error("sqlite found the file corrupt")]
    Corrupt(#[source] rusqlite::Error),
}

impl Store {
    /// Creates an empty data file at `path` whose amounts are in `currency`, an ISO 4217
    /// code such as `"USD"`. Refuses a path that already holds a file, and leaves no file
    /// behind when creating fails.
    pub fn create(path: &Path, currency: &str) -> Result<Self, StoreError> {
        Self::create_from(path, currency, MIGRATIONS)
    }

    /// [`Self::create`] with `migrations` in place of Balanced's own, for tests that build
    /// a file at an older version or with steps of their own.
    #[cfg(feature = "test-support")]
    pub fn create_with_migrations(
        path: &Path,
        currency: &str,
        migrations: &[Migration],
    ) -> Result<Self, StoreError> {
        Self::create_from(path, currency, migrations)
    }

    fn create_from(
        path: &Path,
        currency: &str,
        migrations: &[Migration],
    ) -> Result<Self, StoreError> {
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
            .and_then(|connection| Self::write_schema(connection, currency, migrations))
            .and_then(|store| {
                store
                    .connection
                    .pragma_update(None, "locking_mode", "EXCLUSIVE")?;
                lock(&store.connection)?;
                Ok(store)
            })
            .inspect_err(|_| {
                // The file is half made and nobody has seen it, so it goes. Failing to remove
                // it leaves an empty or partial file, which opening later refuses.
                fs::remove_file(path).ok();
            })
    }

    /// An empty data file in memory, gone when the store is dropped, whose amounts are
    /// in `currency`.
    pub fn create_in_memory(currency: &str) -> Result<Self, StoreError> {
        Self::write_schema(Connection::open_in_memory()?, currency, MIGRATIONS)
    }

    /// Opens the data file at `path` for reading and writing, and holds it locked until
    /// the store is dropped. Refuses a file that is missing, isn't a Balanced data file, is
    /// damaged, comes from a newer Balanced, or that another connection holds, and writes
    /// nothing to a file it refuses.
    pub fn open(path: &Path) -> Result<Self, OpenError> {
        Self::open_from(path, MIGRATIONS)
    }

    /// [`Self::open`] with `migrations` in place of Balanced's own, for tests that upgrade
    /// a file through steps of their own.
    #[cfg(feature = "test-support")]
    pub fn open_with_migrations(path: &Path, migrations: &[Migration]) -> Result<Self, OpenError> {
        Self::open_from(path, migrations)
    }

    fn open_from(path: &Path, migrations: &[Migration]) -> Result<Self, OpenError> {
        if fs::metadata(path).is_err_and(|error| error.kind() == io::ErrorKind::NotFound) {
            return Err(OpenError::Missing(path.to_owned()));
        }
        // Without `SQLITE_OPEN_CREATE`, a missing file is an error rather than a new,
        // empty database.
        let mut connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(open_refusal)?;
        // A file another connection holds is refused at once rather than after waiting.
        // Once this store holds the lock, no other connection can contend for it.
        connection
            .busy_timeout(Duration::ZERO)
            .map_err(open_refusal)?;
        // In exclusive locking mode the shared lock the first check takes is never
        // released, so no other connection can change the file between the checks and
        // the lock.
        connection
            .pragma_update(None, "locking_mode", "EXCLUSIVE")
            .map_err(open_refusal)?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(open_refusal)?;
        let application_id: u32 = connection
            .pragma_query_value(None, "application_id", |row| row.get(0))
            .map_err(open_refusal)?;
        if application_id != APPLICATION_ID {
            return Err(OpenError::NotBalanced);
        }
        let problems = quick_check(&connection).map_err(open_refusal)?;
        if !problems.is_empty() {
            return Err(OpenError::Damaged(Damage::Problems(problems)));
        }
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(open_refusal)?;
        let latest = migration::latest_version(migrations);
        if version > latest {
            return Err(OpenError::Newer { version, latest });
        }
        lock(&connection).map_err(open_refusal)?;
        if version < latest {
            let backup = back_up(&connection, path, version).map_err(OpenError::BackupFailed)?;
            migration::upgrade(&mut connection, migrations, version)
                .map_err(|source| OpenError::UpgradeFailed { backup, source })?;
        }
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
    fn write_schema(
        mut connection: Connection,
        currency: &str,
        migrations: &[Migration],
    ) -> Result<Self, StoreError> {
        connection.pragma_update(None, "foreign_keys", true)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.pragma_update(None, "application_id", APPLICATION_ID)?;
        migration::migrate(&transaction, migrations, 0).map_err(|error| match error {
            UpgradeError::Sqlite(error) => StoreError::Sqlite(error),
            UpgradeError::ForeignKeys { .. } => {
                unreachable!("an empty file's foreign keys have nothing to refer to")
            }
        })?;
        transaction.execute(
            "INSERT INTO data_file (id, currency) VALUES (1, ?1)",
            [currency],
        )?;
        transaction.commit()?;
        Ok(Self { connection })
    }
}

/// The problems `PRAGMA quick_check` finds in the database `connection` holds, empty
/// when there are none.
fn quick_check(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    let report = connection
        .prepare("PRAGMA quick_check")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    // A healthy file's report is the single row `ok`.
    Ok(if report == ["ok"] { Vec::new() } else { report })
}

/// Copies the data file `connection` holds at `path`, at schema `version`, to a new file
/// beside it named `<name> (before upgrade, v<version>).balanced`, and returns the copy's
/// path. The name gains a number when a file already holds it, so an earlier backup is
/// never replaced.
fn back_up(connection: &Connection, path: &Path, version: u32) -> Result<PathBuf, StoreError> {
    let name = path.file_stem().unwrap_or_default().to_string_lossy();
    let mut backup = path.with_file_name(format!("{name} (before upgrade, v{version}).balanced"));
    let mut copy = 1;
    while backup.try_exists().map_err(|source| StoreError::Io {
        path: backup.clone(),
        source,
    })? {
        copy += 1;
        backup = path.with_file_name(format!(
            "{name} (before upgrade, v{version}) {copy}.balanced"
        ));
    }
    connection
        .execute("VACUUM INTO ?1", [backup.to_string_lossy()])
        .inspect_err(|_| {
            // The file didn't exist before, so whatever the failed copy left is its own.
            fs::remove_file(&backup).ok();
        })?;
    Ok(backup)
}

/// Takes, on the file `connection` holds in exclusive locking mode, an exclusive lock it
/// keeps until it closes. Writes nothing: in that mode the lock a transaction takes
/// outlives it, and an empty transaction leaves the file as it was.
fn lock(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch("BEGIN EXCLUSIVE; COMMIT;")
}

/// The refusal SQLite's `error` while opening a file amounts to.
fn open_refusal(error: rusqlite::Error) -> OpenError {
    match error.sqlite_error_code() {
        Some(rusqlite::ErrorCode::NotADatabase) => OpenError::NotBalanced,
        Some(rusqlite::ErrorCode::DatabaseCorrupt) => OpenError::Damaged(Damage::Corrupt(error)),
        Some(rusqlite::ErrorCode::DatabaseBusy) => OpenError::Locked,
        _ => OpenError::Store(StoreError::Sqlite(error)),
    }
}
