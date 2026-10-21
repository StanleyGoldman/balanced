// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Usage: how many transactions and split lines are in each category, or transfer to
//! each account, and what they total.

use std::collections::HashMap;
use std::hash::Hash;

use crate::{
    AccountId, AccountSummary, Allocation, Category, CategoryId, CurrencyCode, Money, MoneyError,
    SplitTarget, TransactionId,
};

/// How many transactions and split lines are in a category, or transfer to an account,
/// and their total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    count: usize,
    total: Result<Money, MoneyError>,
}

impl Usage {
    /// No transaction, totalling zero in `currency`.
    fn none(currency: CurrencyCode) -> Self {
        Self {
            count: 0,
            total: Ok(Money::zero(currency)),
        }
    }

    /// The same usage, with one more transaction or split line of `amount`.
    fn add(&mut self, amount: Money) {
        self.count += 1;
        self.total = self
            .total
            .clone()
            .and_then(|total| total.checked_add(amount));
    }

    /// How many transactions and split lines there are. A split counts once for each
    /// of its lines.
    pub fn count(&self) -> usize {
        self.count
    }

    /// The sum of their amounts, each split line counting its own amount. Fails when
    /// the sum overflows.
    pub fn total(&self) -> Result<Money, MoneyError> {
        self.total.clone()
    }
}

/// The usage of every category and every account, as a [`LedgerView`](crate::LedgerView)
/// holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UsageCounts {
    /// Each used category's usage, its subcategories' at every depth included.
    categories: HashMap<CategoryId, Usage>,
    /// Each account's usage as a transfer target.
    transfers: HashMap<AccountId, Usage>,
    /// What an unused category or account has.
    none: Usage,
}

impl UsageCounts {
    /// No usage at all, in `currency`.
    pub(crate) fn empty(currency: CurrencyCode) -> Self {
        Self {
            categories: HashMap::new(),
            transfers: HashMap::new(),
            none: Usage::none(currency),
        }
    }

    /// The usage of `categories` and `accounts` in `accounts`' transactions.
    pub(crate) fn count(
        categories: &[Category],
        accounts: &[AccountSummary],
        currency: CurrencyCode,
    ) -> Self {
        let mut counts = Self::empty(currency);
        let parents: HashMap<CategoryId, Option<CategoryId>> = categories
            .iter()
            .map(|category| (category.id(), category.parent()))
            .collect();
        // The category and every category above it, up to the top of the tree. A tree
        // has no loops, so the walk ends within as many steps as there are categories.
        let with_parents = |id: CategoryId| {
            std::iter::successors(Some(id), |child| parents.get(child).copied().flatten())
                .take(categories.len())
        };
        // The account each transaction is in, to find where a transfer goes.
        let held_in: HashMap<TransactionId, AccountId> = accounts
            .iter()
            .flat_map(|summary| {
                let account = summary.account().id();
                summary
                    .register()
                    .iter()
                    .map(move |entry| (entry.transaction().id(), account))
            })
            .collect();
        let account_of = |other_side: TransactionId| {
            *held_in
                .get(&other_side)
                .expect("a transfer's other side is in an account of the same snapshot")
        };
        for entry in accounts.iter().flat_map(AccountSummary::register) {
            let transaction = entry.transaction();
            let amount = transaction.amount();
            match transaction.allocation() {
                Allocation::Category(category) => {
                    for id in with_parents(*category) {
                        add(&mut counts.categories, id, &counts.none, amount);
                    }
                }
                Allocation::Transfer(other_side) => {
                    add(
                        &mut counts.transfers,
                        account_of(*other_side),
                        &counts.none,
                        amount,
                    );
                }
                Allocation::Split(lines) => {
                    for line in lines {
                        match line.target() {
                            SplitTarget::Category(category) => {
                                for id in with_parents(category) {
                                    add(&mut counts.categories, id, &counts.none, line.amount());
                                }
                            }
                            SplitTarget::Transfer(other_side) => {
                                let account = account_of(other_side);
                                add(&mut counts.transfers, account, &counts.none, line.amount());
                            }
                        }
                    }
                }
                Allocation::OpeningBalance => {}
            }
        }
        counts
    }

    /// Category `id`'s usage, its subcategories' at every depth included.
    pub(crate) fn category(&self, id: CategoryId) -> &Usage {
        self.categories.get(&id).unwrap_or(&self.none)
    }

    /// Account `id`'s usage as a transfer target: the transactions and split lines in
    /// other accounts that transfer to it.
    pub(crate) fn transfer(&self, id: AccountId) -> &Usage {
        self.transfers.get(&id).unwrap_or(&self.none)
    }
}

/// Adds one transaction or split line of `amount` to `key`'s usage in `usages`, which
/// starts at `none`.
fn add<K: Eq + Hash>(usages: &mut HashMap<K, Usage>, key: K, none: &Usage, amount: Money) {
    usages
        .entry(key)
        .or_insert_with(|| none.clone())
        .add(amount);
}

#[cfg(test)]
mod tests {
    use icu_calendar::{Date, Iso};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    use crate::account::NewAccount;
    use crate::category::NewCategory;
    use crate::{
        AccountId, AccountKind, CategoryId, CurrencyCode, DomainCommand, Ledger, Money,
        NewSplitLine, NewTransaction, RegisterEntry, TransactionEdit, TransactionId,
    };

    fn usd(amount: Decimal) -> Money {
        let usd = CurrencyCode::new("USD").expect("USD is an ISO 4217 currency");
        Money::new(amount, usd).expect("the tests' amounts have two decimal places")
    }

    fn date(day: u8) -> Date<Iso> {
        Date::try_new_iso(2026, 9, day).expect("the tests' days are in September 2026")
    }

    const FOOD: CategoryId = CategoryId::new(0);
    const GROCERIES: CategoryId = CategoryId::new(1);
    const HOUSEHOLD: CategoryId = CategoryId::new(2);
    const ORGANIC: CategoryId = CategoryId::new(3);

    /// A ledger whose categories are Food, Groceries under Food, Household, and Organic
    /// under Groceries, with one account holding `transactions` after a 500.00 opening balance.
    fn ledger(transactions: impl IntoIterator<Item = NewTransaction>) -> Ledger {
        Ledger::with_accounts(
            CurrencyCode::new("USD").expect("USD is a currency"),
            [
                NewCategory::new("Food"),
                NewCategory::new("Groceries").under(FOOD),
                NewCategory::new("Household"),
                NewCategory::new("Organic").under(GROCERIES),
            ],
            [NewAccount::new(
                "Everyday",
                AccountKind::Checking,
                date(1),
                usd(dec!(500.00)),
            )
            .with_transactions(transactions)],
        )
        .expect("the amounts are in USD, far from overflow")
    }

    fn paid(payee: &str, category: CategoryId, amount: Decimal) -> NewTransaction {
        NewTransaction::new(date(5), payee, category, usd(amount))
    }

    #[test]
    fn a_category_counts_and_totals_the_transactions_in_it() {
        let ledger = ledger([
            paid("Fresh Market", GROCERIES, dec!(-86.42)),
            paid("Hardware Barn", HOUSEHOLD, dec!(-12.00)),
            paid("Fresh Market", GROCERIES, dec!(-10.00)),
        ]);

        let view = ledger.snapshot();

        let groceries = view.category_usage(GROCERIES);
        assert_eq!(groceries.count(), 2);
        assert_eq!(groceries.total(), Ok(usd(dec!(-96.42))));
    }

    #[test]
    fn a_split_line_counts_in_its_own_category_with_its_own_amount() {
        let ledger = ledger([
            NewTransaction::split(
                date(12),
                "Costco",
                vec![
                    NewSplitLine::category(GROCERIES, "Food", usd(dec!(-142.37))),
                    NewSplitLine::category(HOUSEHOLD, "Paper towels", usd(dec!(-63.90))),
                ],
            ),
            paid("Fresh Market", GROCERIES, dec!(-10.00)),
        ]);

        let view = ledger.snapshot();

        let groceries = view.category_usage(GROCERIES);
        let household = view.category_usage(HOUSEHOLD);
        assert_eq!(
            (groceries.count(), groceries.total()),
            (2, Ok(usd(dec!(-152.37))))
        );
        assert_eq!(
            (household.count(), household.total()),
            (1, Ok(usd(dec!(-63.90))))
        );
    }

    #[test]
    fn a_category_counts_every_category_under_it_at_every_depth() {
        let ledger = ledger([
            paid("Farmers Market", FOOD, dec!(-20.00)),
            paid("Fresh Market", GROCERIES, dec!(-10.00)),
            paid("Green Grocer", ORGANIC, dec!(-5.00)),
            paid("Hardware Barn", HOUSEHOLD, dec!(-12.00)),
        ]);

        let view = ledger.snapshot();

        let food = view.category_usage(FOOD);
        let groceries = view.category_usage(GROCERIES);
        assert_eq!((food.count(), food.total()), (3, Ok(usd(dec!(-35.00)))));
        assert_eq!(
            (groceries.count(), groceries.total()),
            (2, Ok(usd(dec!(-15.00))))
        );
    }

    #[test]
    fn an_account_counts_the_transactions_and_split_lines_that_transfer_to_it() {
        let everyday = AccountId::new(0);
        let savings = AccountId::new(1);
        let mut ledger = Ledger::with_accounts(
            CurrencyCode::new("USD").unwrap(),
            [NewCategory::new("Groceries")],
            [
                NewAccount::new(
                    "Everyday",
                    AccountKind::Checking,
                    date(1),
                    usd(dec!(500.00)),
                ),
                NewAccount::new("Savings", AccountKind::Savings, date(1), usd(dec!(0.00))),
            ],
        )
        .unwrap();
        let post = |transaction| DomainCommand::PostTransaction {
            account: everyday,
            transaction,
        };

        ledger
            .apply(post(NewTransaction::transfer(
                date(5),
                "Monthly saving",
                savings,
                usd(dec!(-100.00)),
            )))
            .unwrap();
        ledger
            .apply(post(NewTransaction::split(
                date(6),
                "Payday",
                vec![
                    NewSplitLine::transfer(savings, "Saved", usd(dec!(-25.00))),
                    NewSplitLine::category(CategoryId::new(0), "Food", usd(dec!(-5.00))),
                ],
            )))
            .unwrap();

        let view = ledger.snapshot();
        let to_savings = view.transfer_usage(savings);
        let to_everyday = view.transfer_usage(everyday);
        assert_eq!(
            (to_savings.count(), to_savings.total()),
            (2, Ok(usd(dec!(-125.00))))
        );
        assert_eq!(
            (to_everyday.count(), to_everyday.total()),
            (2, Ok(usd(dec!(125.00))))
        );
    }

    /// The id of the transaction paid to `payee` in `ledger`'s only account.
    fn paid_to(ledger: &Ledger, payee: &str) -> TransactionId {
        ledger.snapshot().accounts()[0]
            .register()
            .iter()
            .map(RegisterEntry::transaction)
            .find(|transaction| transaction.payee() == payee)
            .expect("the ledger holds a transaction to the payee")
            .id()
    }

    #[test]
    fn editing_and_deleting_a_transaction_moves_its_usage() {
        let mut ledger = ledger([
            paid("Fresh Market", GROCERIES, dec!(-10.00)),
            paid("Hardware Barn", HOUSEHOLD, dec!(-12.00)),
        ]);
        let fresh_market = paid_to(&ledger, "Fresh Market");
        let hardware_barn = paid_to(&ledger, "Hardware Barn");

        ledger
            .apply(DomainCommand::EditTransaction {
                transaction: fresh_market,
                edit: TransactionEdit::Category(HOUSEHOLD),
            })
            .unwrap();
        ledger
            .apply(DomainCommand::DeleteTransaction {
                transaction: hardware_barn,
            })
            .unwrap();

        let view = ledger.snapshot();
        let groceries = view.category_usage(GROCERIES);
        let household = view.category_usage(HOUSEHOLD);
        assert_eq!(
            (groceries.count(), groceries.total()),
            (0, Ok(usd(dec!(0.00))))
        );
        assert_eq!(
            (household.count(), household.total()),
            (1, Ok(usd(dec!(-10.00))))
        );
    }
}
