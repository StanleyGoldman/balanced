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

pub use account::{Account, AccountGroup, AccountId, AccountKind, AccountStatus, Visibility};
pub use category::{Category, CategoryId};
pub use command::{Applied, DomainCommand, DomainError, MoveDirection, TransactionEdit};
pub use data_file::{OpenError, WriteError};
pub use ledger::{AccountSummary, Generation, Ledger, LedgerView, RegisterEntry};
pub use money::{CurrencyCode, Money, MoneyError, RoundingPolicy};
pub use transaction::{Allocation, NewTransaction, SplitLine, Transaction, TransactionId};
