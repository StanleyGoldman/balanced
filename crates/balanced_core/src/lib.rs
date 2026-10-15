//! The financial domain: accounts, transactions, money and the `Ledger` that applies
//! `DomainCommand`s. Knows nothing about egui.

mod ledger;
mod money;

pub use ledger::{Generation, Ledger, LedgerView};
pub use money::{CurrencyCode, Money, MoneyError, RoundingPolicy};
