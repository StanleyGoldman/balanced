//! Reading a whole data file, with the derived numbers from its views.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, Row};

use crate::rows::{
    AccountRow, AllocationRow, CategoryRow, Contents, LastIds, SplitLineRow, Stack, StoredAccount,
    StoredTransaction, TransactionRow,
};
use crate::{Store, StoreError};

impl Store {
    /// Everything in the file. The balances and totals come from its views.
    pub fn read(&self) -> Result<Contents, StoreError> {
        let connection = &self.connection;
        let total = |view: &str| -> rusqlite::Result<i64> {
            connection.query_row(&format!("SELECT total FROM {view}"), [], |row| row.get(0))
        };
        Ok(Contents {
            currency: connection
                .query_row("SELECT currency FROM data_file", [], |row| row.get(0))?,
            accounts: accounts(connection)?,
            categories: categories(connection)?,
            transactions: transactions(connection)?,
            net_worth: total("net_worth")?,
            group_totals: connection
                .prepare("SELECT account_group, total FROM group_total ORDER BY account_group")?
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<_, _>>()?,
            closed_total: total("closed_total")?,
            last_ids: LastIds {
                account: last_id(connection, "account")?,
                category: last_id(connection, "category")?,
                transaction: last_id(connection, "transaction")?,
            },
            undo: entries(connection, Stack::Undo)?,
            redo: entries(connection, Stack::Redo)?,
        })
    }
}

fn accounts(connection: &Connection) -> rusqlite::Result<Vec<StoredAccount>> {
    connection
        .prepare(
            "SELECT a.id, a.name, a.kind, a.position, a.hidden, a.closed, b.balance
             FROM account AS a JOIN account_balance AS b ON b.account_id = a.id
             ORDER BY a.kind, a.position",
        )?
        .query_map([], |row| {
            Ok(StoredAccount {
                row: AccountRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    position: row.get(3)?,
                    hidden: row.get(4)?,
                    closed: row.get(5)?,
                },
                balance: row.get(6)?,
            })
        })?
        .collect()
}

fn categories(connection: &Connection) -> rusqlite::Result<Vec<CategoryRow>> {
    connection
        .prepare("SELECT id, parent_id, name FROM category ORDER BY id")?
        .query_map([], |row| {
            Ok(CategoryRow {
                id: row.get(0)?,
                parent_id: row.get(1)?,
                name: row.get(2)?,
            })
        })?
        .collect()
}

fn transactions(connection: &Connection) -> rusqlite::Result<Vec<StoredTransaction>> {
    let mut lines = split_lines(connection)?;
    connection
        .prepare(
            "SELECT t.id, t.account_id, t.date, t.number, t.payee, t.memo, t.allocation,
                 t.amount, t.category_id, r.amount, r.balance
             FROM \"transaction\" AS t
             JOIN register_entry AS r ON r.transaction_id = t.id
             ORDER BY t.account_id, t.date, t.id",
        )?
        .query_map([], |row| {
            let id = row.get(0)?;
            let allocation = allocation(row, lines.remove(&id).unwrap_or_default())?;
            Ok(StoredTransaction {
                row: TransactionRow {
                    id,
                    account_id: row.get(1)?,
                    date: row.get(2)?,
                    number: row.get(3)?,
                    payee: row.get(4)?,
                    memo: row.get(5)?,
                    allocation,
                },
                amount: row.get(9)?,
                balance: row.get(10)?,
            })
        })?
        .collect()
}

/// The allocation of the transaction `row` holds, columns 6 to 8, with `lines` when it is
/// split.
fn allocation(row: &Row<'_>, lines: Vec<SplitLineRow>) -> rusqlite::Result<AllocationRow> {
    let allocation: String = row.get(6)?;
    Ok(match allocation.as_str() {
        "opening_balance" => AllocationRow::OpeningBalance {
            amount: row.get(7)?,
        },
        "category" => AllocationRow::Category {
            category_id: row.get(8)?,
            amount: row.get(7)?,
        },
        // The CHECK on `allocation` admits no other value.
        _ => AllocationRow::Split(lines),
    })
}

/// Every split line, by transaction, each transaction's lines in order.
fn split_lines(connection: &Connection) -> rusqlite::Result<HashMap<u32, Vec<SplitLineRow>>> {
    let mut lines: HashMap<u32, Vec<SplitLineRow>> = HashMap::new();
    let mut statement = connection.prepare(
        "SELECT transaction_id, category_id, memo, amount FROM split_line
         ORDER BY transaction_id, position",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        lines.entry(row.get(0)?).or_default().push(SplitLineRow {
            category_id: row.get(1)?,
            memo: row.get(2)?,
            amount: row.get(3)?,
        });
    }
    Ok(lines)
}

/// The highest id `table` has ever held, or `None` when it has never held a row.
fn last_id(connection: &Connection, table: &str) -> rusqlite::Result<Option<u32>> {
    connection
        .query_row(
            "SELECT seq FROM sqlite_sequence WHERE name = ?1",
            [table],
            |row| row.get(0),
        )
        .optional()
}

/// The entries on `stack`, oldest first.
fn entries(connection: &Connection, stack: Stack) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare("SELECT command FROM undo_entry WHERE stack = ?1 ORDER BY position")?
        .query_map([stack.name()], |row| row.get(0))?
        .collect()
}
