//! The [`Ledger`]: the financial data, and the snapshots windows read it through.

use std::sync::Arc;

use crate::account::NewAccount;
use crate::category::NewCategory;
use crate::transaction::TransactionId;
use crate::{
    Account, AccountId, Category, CategoryId, CurrencyCode, Money, MoneyError, Transaction,
};

/// Counts the changes applied to a [`Ledger`]. A window's view model rebuilds when
/// the generation of the snapshot it reads changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Generation(u64);

impl Generation {
    /// The generation of a ledger no change has been applied to.
    pub const FIRST: Self = Self(0);
}

/// A transaction in its account's register, with the account's balance after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterEntry {
    transaction: Transaction,
    balance: Money,
}

impl RegisterEntry {
    pub fn transaction(&self) -> &Transaction {
        &self.transaction
    }

    /// The running balance: the account's balance once this transaction and every one
    /// before it in the register is counted.
    pub fn balance(&self) -> Money {
        self.balance
    }
}

/// An account in a [`LedgerView`], with its transactions and its balance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSummary {
    account: Account,
    register: Vec<RegisterEntry>,
    balance: Money,
}

impl AccountSummary {
    pub fn account(&self) -> &Account {
        &self.account
    }

    /// The account's transactions in date order, each with its running balance.
    /// Transactions on the same date keep the order they were posted in.
    pub fn register(&self) -> &[RegisterEntry] {
        &self.register
    }

    /// The account's balance, in the account's currency: the running balance after its
    /// last transaction.
    pub fn balance(&self) -> Money {
        self.balance
    }
}

/// An immutable snapshot of the ledger, shared by every window that reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerView {
    generation: Generation,
    accounts: Vec<AccountSummary>,
    categories: Vec<Category>,
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

    /// The category with `id`, or `None` once it has been deleted.
    pub fn category(&self, id: CategoryId) -> Option<&Category> {
        self.categories.iter().find(|category| category.id() == id)
    }

    /// The sum of every account's balance, hidden and closed accounts included.
    pub fn net_worth(&self) -> Money {
        self.net_worth
    }
}

/// The financial data. It changes only by applying a `DomainCommand`, and windows
/// read it through [`Ledger::snapshot`].
#[derive(Debug)]
pub struct Ledger {
    view: Arc<LedgerView>,
}

impl Ledger {
    /// A ledger in `currency` holding `categories` and `accounts`, each account landing
    /// last in its sub-group. Fails when an amount is in another currency than the
    /// ledger, or when a balance overflows.
    ///
    /// Panics when a transaction names a category that isn't in `categories`.
    pub(crate) fn with_accounts(
        currency: CurrencyCode,
        categories: impl IntoIterator<Item = NewCategory>,
        accounts: impl IntoIterator<Item = NewAccount>,
    ) -> Result<Self, MoneyError> {
        let categories: Vec<Category> = (0..)
            .zip(categories)
            .map(|(id, new)| new.into_category(CategoryId::new(id)))
            .collect();
        let mut transaction_ids = (0..).map(TransactionId::new);
        let mut summaries: Vec<AccountSummary> = Vec::new();
        let mut net_worth = Money::zero(currency);
        for (id, new) in (0..).zip(accounts) {
            let kind = new.kind();
            let (account, transactions) = new.into_account(AccountId::new(id));
            let mut posted = Vec::new();
            for new in transactions {
                for category in new.categories() {
                    assert!(
                        categories.iter().any(|known| known.id() == category),
                        "a new transaction names {category:?}, which is not in the ledger"
                    );
                }
                let id = transaction_ids.next().expect("the ids never run out");
                posted.push(new.into_transaction(id, currency)?);
            }
            // A stable sort, so transactions on one date keep the order they were posted in.
            posted.sort_by_key(Transaction::date);
            let mut register = Vec::with_capacity(posted.len());
            let mut balance = Money::zero(currency);
            for transaction in posted {
                balance = balance.checked_add(transaction.amount())?;
                register.push(RegisterEntry {
                    transaction,
                    balance,
                });
            }
            net_worth = net_worth.checked_add(balance)?;
            // Last in its sub-group: after every account of its kind or an earlier one.
            let position = summaries.partition_point(|summary| summary.account.kind() <= kind);
            summaries.insert(
                position,
                AccountSummary {
                    account,
                    register,
                    balance,
                },
            );
        }
        Ok(Self {
            view: Arc::new(LedgerView {
                generation: Generation::FIRST,
                accounts: summaries,
                categories,
                net_worth,
            }),
        })
    }

    /// The current snapshot. It stays valid, unchanged, after the ledger changes.
    pub fn snapshot(&self) -> Arc<LedgerView> {
        Arc::clone(&self.view)
    }
}

#[cfg(test)]
mod tests {
    use icu_calendar::{Date, Iso};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::transaction::{NewTransaction, SplitLine};
    use crate::{AccountKind, Allocation};

    fn usd_code() -> CurrencyCode {
        CurrencyCode::new("USD").unwrap()
    }

    fn usd(amount: Decimal) -> Money {
        Money::new(amount, usd_code()).unwrap()
    }

    fn date(day: u8) -> Date<Iso> {
        Date::try_new_iso(2026, 9, day).unwrap()
    }

    /// An account opened on 1 September 2026 with `opening_balance`.
    fn account(name: &str, kind: AccountKind, opening_balance: Money) -> NewAccount {
        NewAccount::new(name, kind, date(1), opening_balance)
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
            [],
            [
                account("Mortgage", AccountKind::Liability, usd(dec!(-1))),
                account("House", AccountKind::Asset, usd(dec!(1))),
                account("Visa", AccountKind::CreditCard, usd(dec!(-1))),
                account("Everyday", AccountKind::Checking, usd(dec!(1))),
                account("401(k)", AccountKind::Retirement, usd(dec!(1))),
                account("Brokerage", AccountKind::Investment, usd(dec!(1))),
                account("Joint", AccountKind::Checking, usd(dec!(1))),
                account("Rainy Day", AccountKind::Savings, usd(dec!(1))),
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
            [],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(1000.50))),
                account("Holiday Fund", AccountKind::Savings, usd(dec!(200.25))).hidden(),
                account("Old Checking", AccountKind::Checking, usd(dec!(10.00))).closed(),
                account("Visa", AccountKind::CreditCard, usd(dec!(-400.00))),
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
            [],
            [account("Euro Savings", AccountKind::Savings, euros)],
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
            [],
            [
                account("Shown", AccountKind::Checking, usd(dec!(1))),
                account("Hidden", AccountKind::Checking, usd(dec!(1))).hidden(),
                account("Closed", AccountKind::Checking, usd(dec!(1))).closed(),
                account("Both", AccountKind::Checking, usd(dec!(1)))
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
            [],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(1))),
                account("Visa", AccountKind::CreditCard, usd(dec!(-2))),
            ],
        )
        .unwrap();
        let view = ledger.snapshot();
        let visa = view.accounts()[1].account().id();

        let found = view.account(visa).unwrap();

        assert_eq!(found.account().name(), "Visa");
        assert_eq!(found.balance(), usd(dec!(-2)));
    }

    #[test]
    fn running_balances_accumulate_in_date_order() {
        let groceries = CategoryId::new(0);
        let salary = CategoryId::new(1);
        let ledger = Ledger::with_accounts(
            usd_code(),
            [NewCategory::new("Groceries"), NewCategory::new("Salary")],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(100.00))).with_transactions([
                    NewTransaction::new(date(10), "Fresh Market", groceries, usd(dec!(-30.25))),
                    NewTransaction::new(date(5), "Acme Corp", salary, usd(dec!(500.00))),
                    NewTransaction::new(date(10), "Bean There", groceries, usd(dec!(-4.75))),
                ]),
            ],
        )
        .unwrap();

        let view = ledger.snapshot();

        let everyday = &view.accounts()[0];
        let register: Vec<_> = everyday
            .register()
            .iter()
            .map(|entry| (entry.transaction().payee(), entry.balance()))
            .collect();
        assert_eq!(
            register,
            [
                ("", usd(dec!(100.00))),
                ("Acme Corp", usd(dec!(600.00))),
                ("Fresh Market", usd(dec!(569.75))),
                ("Bean There", usd(dec!(565.00))),
            ]
        );
        assert_eq!(everyday.balance(), usd(dec!(565.00)));
    }

    #[test]
    fn a_split_transaction_counts_the_sum_of_its_lines() {
        let groceries = CategoryId::new(0);
        let household = CategoryId::new(1);
        let ledger = Ledger::with_accounts(
            usd_code(),
            [NewCategory::new("Groceries"), NewCategory::new("Household")],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(500.00))).with_transactions([
                    NewTransaction::split(
                        date(12),
                        "Costco",
                        vec![
                            SplitLine::new(groceries, "Food", usd(dec!(-142.37))),
                            SplitLine::new(household, "Detergent", usd(dec!(-63.90))),
                        ],
                    ),
                    NewTransaction::new(date(15), "Fresh Market", groceries, usd(dec!(-10.00))),
                ]),
            ],
        )
        .unwrap();

        let view = ledger.snapshot();

        let register = view.accounts()[0].register();
        let split = &register[1];
        assert_eq!(split.transaction().amount(), usd(dec!(-206.27)));
        assert_eq!(split.balance(), usd(dec!(293.73)));
        assert_eq!(register[2].balance(), usd(dec!(283.73)));
    }

    #[test]
    fn a_transaction_keeps_what_it_was_posted_with() {
        let groceries = CategoryId::new(0);
        let household = CategoryId::new(1);
        let ledger = Ledger::with_accounts(
            usd_code(),
            [NewCategory::new("Groceries"), NewCategory::new("Household")],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(500.00))).with_transactions([
                    NewTransaction::new(date(3), "Maple", household, usd(dec!(-1450.00)))
                        .number("1041")
                        .memo("September rent"),
                    NewTransaction::split(
                        date(12),
                        "Costco",
                        vec![SplitLine::new(groceries, "Food", usd(dec!(-142.37)))],
                    ),
                ]),
            ],
        )
        .unwrap();

        let view = ledger.snapshot();

        let register = view.accounts()[0].register();
        let rent = register[1].transaction();
        assert_eq!(
            (rent.number(), rent.payee(), rent.memo(), rent.allocation()),
            (
                Some("1041"),
                "Maple",
                "September rent",
                &Allocation::Category(household)
            )
        );
        let costco = register[2].transaction();
        let Allocation::Split(lines) = costco.allocation() else {
            panic!("Costco was posted as a split");
        };
        let lines: Vec<_> = lines
            .iter()
            .map(|line| (line.category(), line.memo(), line.amount()))
            .collect();
        assert_eq!(lines, [(groceries, "Food", usd(dec!(-142.37)))]);
        assert_eq!((costco.number(), costco.memo()), (None, ""));
    }

    #[test]
    fn a_category_is_found_by_its_id_with_its_parent() {
        let food = CategoryId::new(0);
        let groceries = CategoryId::new(1);
        let ledger = Ledger::with_accounts(
            usd_code(),
            [
                NewCategory::new("Food & Dining"),
                NewCategory::new("Groceries").under(food),
            ],
            [],
        )
        .unwrap();

        let view = ledger.snapshot();

        let found = |id| {
            let category = view.category(id).unwrap();
            (category.id(), category.name(), category.parent())
        };
        assert_eq!(found(food), (food, "Food & Dining", None));
        assert_eq!(found(groceries), (groceries, "Groceries", Some(food)));
    }

    #[test]
    #[should_panic(expected = "which is not in the ledger")]
    fn a_transaction_in_an_unknown_category_is_a_bug() {
        let unknown = CategoryId::new(0);

        Ledger::with_accounts(
            usd_code(),
            [],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(1))).with_transactions([
                    NewTransaction::new(date(2), "Fresh Market", unknown, usd(dec!(-1))),
                ]),
            ],
        )
        .ok();
    }
}
