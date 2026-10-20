// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The data file: a SQLite database holding one household's finances. Creates and opens
//! the file and owns all of Balanced's SQL. Speaks in primitive rows (integer ids,
//! amounts in minor units, ISO date text) and knows nothing of `balanced_core`'s types.

// Without `test-support`, nothing outside the crate reaches the migrations.
#[cfg_attr(
    not(feature = "test-support"),
    expect(unreachable_pub, reason = "only `test-support` exports the migrations")
)]
mod migration;
mod read;
mod rows;
mod store;
mod write;

pub use migration::UpgradeError;
#[cfg(feature = "test-support")]
pub use migration::{MIGRATIONS, Migration, UndoFormat};
pub use rows::{
    AccountRow, AllocationRow, CategoryRow, Contents, LastIds, SplitLineRow, SplitTargetRow, Stack,
    StoredAccount, StoredTransaction, TransactionRow,
};
pub use store::{Damage, OpenError, Store, StoreError};
