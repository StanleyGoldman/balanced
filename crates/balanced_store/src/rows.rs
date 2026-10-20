// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The rows the store reads and writes, in primitive types: ids as `u32`, amounts as
//! `i64` minor units, dates as ISO `YYYY-MM-DD` text, and enums as their TEXT names.

/// An account, at `position` among the accounts of its `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRow {
    pub id: u32,
    pub name: String,
    /// `checking`, `savings`, `credit_card`, `investment`, `retirement`, `asset` or
    /// `liability`.
    pub kind: String,
    /// Counts from 0 within the kind.
    pub position: u32,
    pub hidden: bool,
    pub closed: bool,
}

/// A category, under `parent_id` or at the top of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryRow {
    pub id: u32,
    pub parent_id: Option<u32>,
    pub name: String,
}

/// A transaction in account `account_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionRow {
    pub id: u32,
    pub account_id: u32,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Never `Some("")`.
    pub number: Option<String>,
    pub payee: String,
    pub memo: String,
    pub allocation: AllocationRow,
    pub flagged: bool,
    /// `uncleared`, `cleared` or `reconciled`.
    pub cleared_status: String,
}

/// Where a transaction's amount went, with the amount where the row stores one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationRow {
    OpeningBalance {
        amount: i64,
    },
    Category {
        category_id: u32,
        amount: i64,
    },
    /// The lines in order. The transaction's amount is their sum.
    Split(Vec<SplitLineRow>),
    /// One side of a transfer, whose other side is transaction `other_side_id`.
    Transfer {
        other_side_id: u32,
        amount: i64,
    },
}

/// One line of a split transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitLineRow {
    pub target: SplitTargetRow,
    pub memo: String,
    pub amount: i64,
}

/// Where a split line's part of the amount went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitTargetRow {
    Category { category_id: u32 },
}

/// An account as read back, with its balance from the `account_balance` view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAccount {
    pub row: AccountRow,
    pub balance: i64,
}

/// A transaction as read back, with its amount and running balance from the
/// `register_entry` view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTransaction {
    pub row: TransactionRow,
    pub amount: i64,
    pub balance: i64,
}

/// Everything in a data file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contents {
    /// The ISO 4217 code every amount is in.
    pub currency: String,
    /// By kind name, then position.
    pub accounts: Vec<StoredAccount>,
    /// By id.
    pub categories: Vec<CategoryRow>,
    /// By account, then in register order: by date, then id.
    pub transactions: Vec<StoredTransaction>,
    /// From the `net_worth` view.
    pub net_worth: i64,
    /// Each Account bar group's name (`banking`, `investing`, `property_debt`) with its
    /// total, from the `group_total` view.
    pub group_totals: Vec<(String, i64)>,
    /// From the `closed_total` view.
    pub closed_total: i64,
    /// The highest id each table has ever held, deleted rows included, or `None` when
    /// it has never held a row.
    pub last_ids: LastIds,
    /// The undo stack's entries, oldest first.
    pub undo: Vec<String>,
    /// The redo stack's entries, oldest first.
    pub redo: Vec<String>,
}

/// The highest id each table has ever held, from `sqlite_sequence`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastIds {
    pub account: Option<u32>,
    pub category: Option<u32>,
    pub transaction: Option<u32>,
}

/// One of the undo log's two stacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stack {
    Undo,
    Redo,
}

impl Stack {
    /// The name the `stack` column holds.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Undo => "undo",
            Self::Redo => "redo",
        }
    }
}
