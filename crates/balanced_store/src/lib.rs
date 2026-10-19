//! The data file: a SQLite database holding one household's finances. Creates the file
//! and owns all of Balanced's SQL. Speaks in primitive rows (integer ids, amounts in
//! minor units, ISO date text) and knows nothing of `balanced_core`'s types.

mod migration;
mod store;

pub use store::{Store, StoreError};
