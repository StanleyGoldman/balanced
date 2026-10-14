//! The financial domain: accounts, transactions, money and the `Ledger` that applies
//! `DomainCommand`s. Knows nothing about egui.

mod money;

pub use money::{CurrencyCode, Money, MoneyError, RoundingPolicy};
