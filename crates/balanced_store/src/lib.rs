//! The data file: a SQLite database holding one household's finances. Creates and opens
//! the file and owns all of Balanced's SQL. Speaks in primitive rows (integer ids,
//! amounts in minor units, ISO date text) and knows nothing of `balanced_core`'s types.

mod migration;
mod read;
mod rows;
mod store;
mod write;

pub use rows::{
    AccountRow, AllocationRow, CategoryRow, Contents, LastIds, SplitLineRow, Stack, StoredAccount,
    StoredTransaction, TransactionRow,
};
pub use store::{Damage, OpenError, Store, StoreError};
