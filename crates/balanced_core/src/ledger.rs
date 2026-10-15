//! The [`Ledger`]: the financial data, and the snapshots windows read it through.

use std::sync::Arc;

use crate::{Account, AccountId, AccountKind, CurrencyCode, Money, MoneyError};

/// Counts the changes applied to a [`Ledger`]. A window's view model rebuilds when
/// the generation of the snapshot it reads changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Generation(u64);

impl Generation {
    /// The generation of a ledger no change has been applied to.
    pub const FIRST: Self = Self(0);
}

/// An account in a [`LedgerView`], with its balance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSummary {
    account: Account,
    balance: Money,
}

impl AccountSummary {
    pub fn account(&self) -> &Account {
        &self.account
    }

    /// The account's balance, in the account's currency.
    pub fn balance(&self) -> Money {
        self.balance
    }
}

/// An immutable snapshot of the ledger, shared by every window that reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerView {
    generation: Generation,
    accounts: Vec<AccountSummary>,
    net_worth: Money,
}

impl LedgerView {
    /// Which change to the ledger this snapshot follows.
    pub fn generation(&self) -> Generation {
        self.generation
    }

    /// Every account, hidden and closed ones included, in Account bar order: by
    /// group, then by sub-group, then by the account's order within its sub-group.
    pub fn accounts(&self) -> &[AccountSummary] {
        &self.accounts
    }

    /// The account with `id`, or `None` once it has been deleted.
    pub fn account(&self, id: AccountId) -> Option<&AccountSummary> {
        self.accounts
            .iter()
            .find(|summary| summary.account.id() == id)
    }

    /// The sum of every account's balance, hidden and closed accounts included.
    pub fn net_worth(&self) -> Money {
        self.net_worth
    }
}

/// An account to open in a new [`Ledger`].
#[derive(Debug, Clone)]
pub(crate) struct NewAccount {
    name: String,
    kind: AccountKind,
    // TODO(#85): post the opening balance as a transaction, and compute the balance
    // from the account's transactions.
    opening_balance: Money,
    hidden: bool,
    closed: bool,
}

impl NewAccount {
    /// An open, shown account in the currency of `opening_balance`.
    pub(crate) fn new(name: &str, kind: AccountKind, opening_balance: Money) -> Self {
        Self {
            name: name.to_owned(),
            kind,
            opening_balance,
            hidden: false,
            closed: false,
        }
    }

    /// The same account, hidden from the Account bar.
    pub(crate) fn hidden(self) -> Self {
        Self {
            hidden: true,
            ..self
        }
    }

    /// The same account, closed.
    pub(crate) fn closed(self) -> Self {
        Self {
            closed: true,
            ..self
        }
    }
}

/// The financial data. It changes only by applying a `DomainCommand`, and windows
/// read it through [`Ledger::snapshot`].
#[derive(Debug)]
pub struct Ledger {
    view: Arc<LedgerView>,
}

impl Ledger {
    /// A ledger in `currency` holding `accounts`, each landing last in its
    /// sub-group. Fails when an account is in another currency than the ledger, or
    /// when Net Worth overflows.
    pub(crate) fn with_accounts(
        currency: CurrencyCode,
        accounts: impl IntoIterator<Item = NewAccount>,
    ) -> Result<Self, MoneyError> {
        let mut summaries: Vec<AccountSummary> = Vec::new();
        let mut net_worth = Money::zero(currency);
        for (id, new) in (0..).zip(accounts) {
            net_worth = net_worth.checked_add(new.opening_balance)?;
            let mut account = Account::new(
                AccountId::new(id),
                new.name,
                new.kind,
                new.opening_balance.currency(),
            );
            account.set_hidden(new.hidden);
            account.set_closed(new.closed);
            // Last in its sub-group: after every account of its kind or an earlier one.
            let position = summaries.partition_point(|summary| summary.account.kind() <= new.kind);
            summaries.insert(
                position,
                AccountSummary {
                    account,
                    balance: new.opening_balance,
                },
            );
        }
        Ok(Self {
            view: Arc::new(LedgerView {
                generation: Generation::FIRST,
                accounts: summaries,
                net_worth,
            }),
        })
    }

    /// The in-memory data Balanced runs on until it reads data files: accounts in
    /// every sub-group, with one closed account and one hidden one.
    pub fn stub() -> Self {
        let usd = CurrencyCode::new("USD").expect("USD is an ISO 4217 currency");
        let account = |name, kind, amount: &str| {
            let amount = amount.parse().expect("stub amounts are decimal numbers");
            let balance = Money::new(amount, usd).expect("stub amounts have two decimal places");
            NewAccount::new(name, kind, balance)
        };
        Self::with_accounts(
            usd,
            [
                account("Everyday Checking", AccountKind::Checking, "2450.18"),
                account("Joint Checking", AccountKind::Checking, "1032.00"),
                account("Old Checking", AccountKind::Checking, "0.00").closed(),
                account("Emergency Fund", AccountKind::Savings, "12000.00"),
                account("Holiday Fund", AccountKind::Savings, "800.00").hidden(),
                account("Visa Rewards", AccountKind::CreditCard, "-412.30"),
                account("Brokerage", AccountKind::Investment, "25310.55"),
                account("401(k)", AccountKind::Retirement, "88420.00"),
                account("Roth IRA", AccountKind::Retirement, "31200.00"),
                account("House", AccountKind::Asset, "420000.00"),
                account("Car", AccountKind::Asset, "14500.00"),
                account("Mortgage", AccountKind::Liability, "-310500.00"),
            ],
        )
        .expect("the stub accounts are all in USD, far from overflow")
    }

    /// The current snapshot. It stays valid, unchanged, after the ledger changes.
    pub fn snapshot(&self) -> Arc<LedgerView> {
        Arc::clone(&self.view)
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    use super::*;

    fn usd_code() -> CurrencyCode {
        CurrencyCode::new("USD").unwrap()
    }

    fn usd(amount: Decimal) -> Money {
        Money::new(amount, usd_code()).unwrap()
    }

    fn names(view: &LedgerView) -> Vec<&str> {
        view.accounts()
            .iter()
            .map(|summary| summary.account().name())
            .collect()
    }

    #[test]
    fn a_stub_ledger_starts_at_the_first_generation() {
        let ledger = Ledger::stub();

        let view = ledger.snapshot();

        assert_eq!(view.generation(), Generation::FIRST);
    }

    #[test]
    fn accounts_are_listed_by_group_then_sub_group_keeping_their_order_within_it() {
        let ledger = Ledger::with_accounts(
            usd_code(),
            [
                NewAccount::new("Mortgage", AccountKind::Liability, usd(dec!(-1))),
                NewAccount::new("House", AccountKind::Asset, usd(dec!(1))),
                NewAccount::new("Visa", AccountKind::CreditCard, usd(dec!(-1))),
                NewAccount::new("Everyday", AccountKind::Checking, usd(dec!(1))),
                NewAccount::new("401(k)", AccountKind::Retirement, usd(dec!(1))),
                NewAccount::new("Brokerage", AccountKind::Investment, usd(dec!(1))),
                NewAccount::new("Joint", AccountKind::Checking, usd(dec!(1))),
                NewAccount::new("Rainy Day", AccountKind::Savings, usd(dec!(1))),
            ],
        )
        .unwrap();

        let view = ledger.snapshot();

        assert_eq!(
            names(&view),
            [
                "Everyday",
                "Joint",
                "Rainy Day",
                "Visa",
                "Brokerage",
                "401(k)",
                "House",
                "Mortgage",
            ]
        );
    }

    #[test]
    fn net_worth_sums_every_balance_hidden_and_closed_accounts_included() {
        let ledger = Ledger::with_accounts(
            usd_code(),
            [
                NewAccount::new("Everyday", AccountKind::Checking, usd(dec!(1000.50))),
                NewAccount::new("Holiday Fund", AccountKind::Savings, usd(dec!(200.25))).hidden(),
                NewAccount::new("Old Checking", AccountKind::Checking, usd(dec!(10.00))).closed(),
                NewAccount::new("Visa", AccountKind::CreditCard, usd(dec!(-400.00))),
            ],
        )
        .unwrap();

        let view = ledger.snapshot();

        assert_eq!(view.net_worth(), usd(dec!(810.75)));
    }

    #[test]
    fn an_account_in_another_currency_than_the_ledger_is_an_error() {
        let eur = CurrencyCode::new("EUR").unwrap();
        let euros = Money::new(dec!(5), eur).unwrap();

        let result = Ledger::with_accounts(
            usd_code(),
            [NewAccount::new("Euro Savings", AccountKind::Savings, euros)],
        );

        assert_eq!(
            result.unwrap_err(),
            MoneyError::CurrencyMismatch {
                left: usd_code(),
                right: eur,
            }
        );
    }

    #[test]
    fn hidden_and_closed_are_independent() {
        let ledger = Ledger::with_accounts(
            usd_code(),
            [
                NewAccount::new("Shown", AccountKind::Checking, usd(dec!(1))),
                NewAccount::new("Hidden", AccountKind::Checking, usd(dec!(1))).hidden(),
                NewAccount::new("Closed", AccountKind::Checking, usd(dec!(1))).closed(),
                NewAccount::new("Both", AccountKind::Checking, usd(dec!(1)))
                    .hidden()
                    .closed(),
            ],
        )
        .unwrap();

        let view = ledger.snapshot();

        let flags: Vec<_> = view
            .accounts()
            .iter()
            .map(|summary| {
                let account = summary.account();
                (account.name(), account.is_hidden(), account.is_closed())
            })
            .collect();
        assert_eq!(
            flags,
            [
                ("Shown", false, false),
                ("Hidden", true, false),
                ("Closed", false, true),
                ("Both", true, true),
            ]
        );
    }

    #[test]
    fn an_account_is_found_by_its_id() {
        let ledger = Ledger::with_accounts(
            usd_code(),
            [
                NewAccount::new("Everyday", AccountKind::Checking, usd(dec!(1))),
                NewAccount::new("Visa", AccountKind::CreditCard, usd(dec!(-2))),
            ],
        )
        .unwrap();
        let view = ledger.snapshot();
        let visa = view.accounts()[1].account().id();

        let found = view.account(visa).unwrap();

        assert_eq!(found.account().name(), "Visa");
        assert_eq!(found.balance(), usd(dec!(-2)));
    }
}
