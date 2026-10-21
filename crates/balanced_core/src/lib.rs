// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The financial domain: accounts, transactions, money and the `Ledger` that applies
//! `DomainCommand`s. Knows nothing about egui.

mod account;
mod category;
mod command;
mod data_file;
mod ledger;
mod money;
mod stub;
mod transaction;
mod undo_log;
mod usage;

pub use account::{Account, AccountGroup, AccountId, AccountKind, AccountStatus, Visibility};
pub use category::{Category, CategoryId};
pub use command::{Applied, DomainCommand, DomainError, MoveDirection, OtherSide, TransactionEdit};
pub use data_file::{Damage, OpenError, WriteError};
pub use ledger::{AccountSummary, Generation, Ledger, LedgerView, RegisterEntry};
pub use money::{CurrencyCode, Money, MoneyError, RoundingPolicy};
pub use transaction::{
    Allocation, ClearedStatus, EditedSplitLine, EditedSplitTarget, NewSplitLine, NewTransaction,
    SplitLine, SplitTarget, Transaction, TransactionId,
};
pub use undo_log::UndoEntryError;
pub use usage::Usage;
