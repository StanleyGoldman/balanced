//! The financial domain: accounts, transactions, money and the `Ledger` that applies
//! `DomainCommand`s. Knows nothing about egui.

mod account;
mod ledger;
mod money;

pub use account::{Account, AccountGroup, AccountId, AccountKind};
pub use ledger::{AccountSummary, Generation, Ledger, LedgerView};
pub use money::{CurrencyCode, Money, MoneyError, RoundingPolicy};
