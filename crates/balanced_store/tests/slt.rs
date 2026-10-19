//! Runs every `tests/slt/*.slt` file against a new data file. Each file is one test and
//! gets a file of its own, made by `Store::create`, so the SQL under test is the schema
//! the app creates.

use balanced_store::Store;
use rusqlite::Connection;
use rusqlite::types::ValueRef;
use sqllogictest::{DBOutput, DefaultColumnType};
use tempfile::TempDir;

/// A new data file, reopened with plain SQLite as a tool inspecting it would.
struct DataFile {
    connection: Connection,
    /// Holds the file until the test ends.
    _dir: TempDir,
}

impl sqllogictest::DB for DataFile {
    type Error = rusqlite::Error;
    type ColumnType = DefaultColumnType;

    fn run(&mut self, sql: &str) -> Result<DBOutput<Self::ColumnType>, Self::Error> {
        let mut statement = self.connection.prepare(sql)?;
        let columns = statement.column_count();
        if columns == 0 {
            let changed = statement.execute([])?;
            return Ok(DBOutput::StatementComplete(changed as u64));
        }
        let rows = statement
            .query_map([], |row| {
                (0..columns).map(|i| row.get_ref(i).map(cell)).collect()
            })
            .and_then(Iterator::collect)?;
        Ok(DBOutput::Rows {
            types: vec![DefaultColumnType::Any; columns],
            rows,
        })
    }
}

/// A value as `.slt` files write it: `NULL`, and `(empty)` for the empty string.
fn cell(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => "NULL".to_owned(),
        ValueRef::Integer(n) => n.to_string(),
        ValueRef::Real(x) => x.to_string(),
        ValueRef::Text(b"") => "(empty)".to_owned(),
        ValueRef::Text(text) => String::from_utf8_lossy(text).into_owned(),
        ValueRef::Blob(blob) => format!("<{} byte blob>", blob.len()),
    }
}

fn data_file() -> DataFile {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = dir.path().join("Household.balanced");
    drop(Store::create(&path, "USD").expect("a new path takes a data file"));
    let connection = Connection::open(&path).expect("the created file opens");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("foreign keys can be switched on outside a transaction");
    DataFile {
        connection,
        _dir: dir,
    }
}

sqllogictest::harness!(data_file, "tests/slt/*.slt");
