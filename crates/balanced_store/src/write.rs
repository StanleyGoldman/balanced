// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Writing rows. Each write joins the transaction [`Store::begin`] started, if any.

use rusqlite::params;

use crate::rows::{AccountRow, AllocationRow, CategoryRow, Stack, TransactionRow};
use crate::{Store, StoreError};

impl Store {
    /// Adds an account at its position in its kind, moving the accounts at that
    /// position and after it one place down.
    pub fn insert_account(&self, account: &AccountRow) -> Result<(), StoreError> {
        // UNIQUE (kind, position) is checked row by row, so the accounts that move first
        // go below 0, out of each other's way, then up to their new places.
        self.connection.execute(
            "UPDATE account SET position = -2 - position WHERE kind = ?1 AND position >= ?2",
            params![account.kind, account.position],
        )?;
        self.connection.execute(
            "UPDATE account SET position = -1 - position WHERE kind = ?1 AND position < 0",
            [&account.kind],
        )?;
        self.connection
            .prepare_cached(
                "INSERT INTO account (id, name, kind, position, hidden, closed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?
            .execute(params![
                account.id,
                account.name,
                account.kind,
                account.position,
                account.hidden,
                account.closed,
            ])?;
        Ok(())
    }

    /// Deletes account `id` with its transactions and their split lines, moving the
    /// accounts after it in its kind one place up.
    pub fn delete_account(&self, id: u32) -> Result<(), StoreError> {
        let (kind, position): (String, u32) = self.connection.query_row(
            "SELECT kind, position FROM account WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        self.connection.execute(
            "DELETE FROM split_line WHERE transaction_id IN
                (SELECT id FROM \"transaction\" WHERE account_id = ?1)",
            [id],
        )?;
        self.connection
            .execute("DELETE FROM \"transaction\" WHERE account_id = ?1", [id])?;
        self.connection
            .execute("DELETE FROM account WHERE id = ?1", [id])?;
        // As in `insert_account`, the accounts that move pass below 0 first.
        self.connection.execute(
            "UPDATE account SET position = -position WHERE kind = ?1 AND position > ?2",
            params![kind, position],
        )?;
        self.connection.execute(
            "UPDATE account SET position = -position - 1 WHERE kind = ?1 AND position < 0",
            [kind],
        )?;
        Ok(())
    }

    /// Renames account `id`.
    pub fn rename_account(&self, id: u32, name: &str) -> Result<(), StoreError> {
        self.connection.execute(
            "UPDATE account SET name = ?2 WHERE id = ?1",
            params![id, name],
        )?;
        Ok(())
    }

    /// Hides account `id`, or shows it.
    pub fn set_account_hidden(&self, id: u32, hidden: bool) -> Result<(), StoreError> {
        self.connection.execute(
            "UPDATE account SET hidden = ?2 WHERE id = ?1",
            params![id, hidden],
        )?;
        Ok(())
    }

    /// Closes account `id`, or opens it.
    pub fn set_account_closed(&self, id: u32, closed: bool) -> Result<(), StoreError> {
        self.connection.execute(
            "UPDATE account SET closed = ?2 WHERE id = ?1",
            params![id, closed],
        )?;
        Ok(())
    }

    /// Swaps the positions of accounts `a` and `b`, which are of one kind.
    pub fn swap_account_positions(&self, a: u32, b: u32) -> Result<(), StoreError> {
        let position = |id: u32| -> rusqlite::Result<i64> {
            self.connection
                .query_row("SELECT position FROM account WHERE id = ?1", [id], |row| {
                    row.get(0)
                })
        };
        let (position_a, position_b) = (position(a)?, position(b)?);
        // UNIQUE (kind, position) is checked row by row, so `a` waits at -1, outside the
        // range, while `b` takes its place.
        let set = "UPDATE account SET position = ?2 WHERE id = ?1";
        self.connection.execute(set, params![a, -1])?;
        self.connection.execute(set, params![b, position_a])?;
        self.connection.execute(set, params![a, position_b])?;
        Ok(())
    }

    /// Adds a category.
    pub fn insert_category(&self, category: &CategoryRow) -> Result<(), StoreError> {
        self.connection
            .prepare_cached("INSERT INTO category (id, parent_id, name) VALUES (?1, ?2, ?3)")?
            .execute(params![category.id, category.parent_id, category.name])?;
        Ok(())
    }

    /// Deletes category `id`. The file refuses it while a transaction, a split line or
    /// another category uses it.
    pub fn delete_category(&self, id: u32) -> Result<(), StoreError> {
        self.connection
            .execute("DELETE FROM category WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Adds a transaction, with its split lines.
    pub fn insert_transaction(&self, transaction: &TransactionRow) -> Result<(), StoreError> {
        let (allocation, amount, category_id) = allocation_columns(&transaction.allocation);
        self.connection
            .prepare_cached(
                "INSERT INTO \"transaction\"
                 (id, account_id, date, number, payee, memo, allocation, amount, category_id,
                  flagged)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?
            .execute(params![
                transaction.id,
                transaction.account_id,
                transaction.date,
                transaction.number,
                transaction.payee,
                transaction.memo,
                allocation,
                amount,
                category_id,
                transaction.flagged,
            ])?;
        self.insert_split_lines(transaction)
    }

    /// Replaces transaction `transaction.id`, with its split lines, by `transaction`.
    pub fn update_transaction(&self, transaction: &TransactionRow) -> Result<(), StoreError> {
        // Ids are inserted explicitly, so the row goes back in under its own id.
        self.delete_transaction(transaction.id)?;
        self.insert_transaction(transaction)
    }

    /// Deletes transaction `id` with its split lines.
    pub fn delete_transaction(&self, id: u32) -> Result<(), StoreError> {
        self.connection
            .execute("DELETE FROM split_line WHERE transaction_id = ?1", [id])?;
        self.connection
            .execute("DELETE FROM \"transaction\" WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Puts `command`, a Command as JSON, on top of `stack`.
    pub fn push_entry(&self, stack: Stack, command: &str) -> Result<(), StoreError> {
        self.connection
            .prepare_cached(
                "INSERT INTO undo_entry (stack, position, command)
                 SELECT ?1, coalesce(max(position) + 1, 0), ?2 FROM undo_entry WHERE stack = ?1",
            )?
            .execute(params![stack.name(), command])?;
        Ok(())
    }

    /// Takes the latest entry off `stack`. Does nothing when it is empty.
    pub fn pop_entry(&self, stack: Stack) -> Result<(), StoreError> {
        self.connection
            .prepare_cached(
                "DELETE FROM undo_entry WHERE stack = ?1
                 AND position = (SELECT max(position) FROM undo_entry WHERE stack = ?1)",
            )?
            .execute([stack.name()])?;
        Ok(())
    }

    /// Takes the oldest entry off `stack`. Does nothing when it is empty.
    pub fn drop_oldest_entry(&self, stack: Stack) -> Result<(), StoreError> {
        self.connection
            .prepare_cached(
                "DELETE FROM undo_entry WHERE stack = ?1
                 AND position = (SELECT min(position) FROM undo_entry WHERE stack = ?1)",
            )?
            .execute([stack.name()])?;
        Ok(())
    }

    /// Takes every entry off `stack`.
    pub fn clear_stack(&self, stack: Stack) -> Result<(), StoreError> {
        self.connection
            .prepare_cached("DELETE FROM undo_entry WHERE stack = ?1")?
            .execute([stack.name()])?;
        Ok(())
    }

    /// Adds `transaction`'s split lines, if it is split.
    fn insert_split_lines(&self, transaction: &TransactionRow) -> Result<(), StoreError> {
        if let AllocationRow::Split(lines) = &transaction.allocation {
            let mut insert = self.connection.prepare_cached(
                "INSERT INTO split_line (transaction_id, position, category_id, memo, amount)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for (position, line) in (0_i64..).zip(lines) {
                insert.execute(params![
                    transaction.id,
                    position,
                    line.category_id,
                    line.memo,
                    line.amount
                ])?;
            }
        }
        Ok(())
    }
}

/// The `allocation`, `amount` and `category_id` columns that hold `allocation`.
fn allocation_columns(allocation: &AllocationRow) -> (&'static str, Option<i64>, Option<u32>) {
    match allocation {
        AllocationRow::OpeningBalance { amount } => ("opening_balance", Some(*amount), None),
        AllocationRow::Category {
            category_id,
            amount,
        } => ("category", Some(*amount), Some(*category_id)),
        AllocationRow::Split(_) => ("split", None, None),
    }
}
