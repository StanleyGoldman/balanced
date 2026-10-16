//! The [`Ledger`]: the financial data, and the snapshots windows read it through.

use std::sync::Arc;

use crate::account::NewAccount;
use crate::category::NewCategory;
use crate::transaction::TransactionId;
use crate::{
    Account, AccountId, Applied, Category, CategoryId, CurrencyCode, DomainCommand, DomainError,
    Money, MoneyError, Transaction, TransactionEdit,
};

/// Counts the changes applied to a [`Ledger`]. A window's view model rebuilds when
/// the generation of the snapshot it reads changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Generation(u64);

impl Generation {
    /// The generation of a ledger no change has been applied to.
    pub const FIRST: Self = Self(0);

    fn next(self) -> Self {
        Self(self.0 + 1)
    }
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
    /// `account` holding `transactions`, which are in register order, with their running
    /// balances in `currency`. Fails when a balance overflows.
    fn new(
        account: Account,
        transactions: Vec<Transaction>,
        currency: CurrencyCode,
    ) -> Result<Self, MoneyError> {
        let mut register = Vec::with_capacity(transactions.len());
        let mut balance = Money::zero(currency);
        for transaction in transactions {
            balance = balance.checked_add(transaction.amount())?;
            register.push(RegisterEntry {
                transaction,
                balance,
            });
        }
        Ok(Self {
            account,
            register,
            balance,
        })
    }

    /// A copy of the account's transactions, in register order.
    fn transactions(&self) -> Vec<Transaction> {
        self.register
            .iter()
            .map(|entry| entry.transaction.clone())
            .collect()
    }

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
    can_undo: bool,
    can_redo: bool,
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

    /// Every category, each after the category it sits under.
    pub fn categories(&self) -> &[Category] {
        &self.categories
    }

    /// The category with `id`, or `None` once it has been deleted.
    pub fn category(&self, id: CategoryId) -> Option<&Category> {
        self.categories.iter().find(|category| category.id() == id)
    }

    /// The names on the way from the top of the category tree down to category `id`,
    /// ending with its own: `["Food & Dining", "Groceries"]`. `None` once it has been
    /// deleted.
    pub fn category_path(&self, id: CategoryId) -> Option<Vec<&str>> {
        let mut category = self.category(id)?;
        let mut names = vec![category.name()];
        while let Some(parent) = category.parent() {
            // A parent comes before its children, so each step moves to a smaller id.
            assert!(parent < category.id(), "a parent comes before its children");
            category = self
                .category(parent)
                .expect("a category's parent is in the same snapshot");
            names.push(category.name());
        }
        names.reverse();
        Some(names)
    }

    /// Where transaction `id` is, or `None` when no account holds it.
    fn find_transaction(&self, id: TransactionId) -> Option<TransactionPosition> {
        self.accounts
            .iter()
            .enumerate()
            .find_map(|(account, summary)| {
                summary
                    .register
                    .iter()
                    .position(|entry| entry.transaction.id() == id)
                    .map(|entry| TransactionPosition { account, entry })
            })
    }

    /// The account at `index` in [`Self::accounts`], rebuilt in `currency` after `edit`
    /// changes its transactions, with what `edit` returned. Fails when a balance
    /// overflows.
    fn edit_register<T>(
        &self,
        index: usize,
        currency: CurrencyCode,
        edit: impl FnOnce(&mut Vec<Transaction>) -> T,
    ) -> Result<(AccountSummary, T), MoneyError> {
        // Callers take `index` from a lookup in `self.accounts`.
        let summary = &self.accounts[index];
        let mut transactions = summary.transactions();
        let edited = edit(&mut transactions);
        let summary = AccountSummary::new(summary.account.clone(), transactions, currency)?;
        Ok((summary, edited))
    }

    /// The sum of every account's balance, hidden and closed accounts included.
    pub fn net_worth(&self) -> Money {
        self.net_worth
    }

    /// Whether there is a change Undo can reverse.
    pub fn can_undo(&self) -> bool {
        self.can_undo
    }

    /// Whether there is an undone change Redo can make again.
    pub fn can_redo(&self) -> bool {
        self.can_redo
    }
}

/// Where a transaction is in a [`LedgerView`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TransactionPosition {
    /// Its account's index in [`LedgerView::accounts`].
    account: usize,
    /// Its index in that account's register.
    entry: usize,
}

/// The financial data. It changes only by applying a `DomainCommand`, and windows
/// read it through [`Ledger::snapshot`].
#[derive(Debug)]
pub struct Ledger {
    view: Arc<LedgerView>,
    /// The inverses of the changes Undo can reverse, the latest last.
    undo_stack: Vec<DomainCommand>,
    /// The inverses of the undone changes Redo can make again, the latest undone last.
    redo_stack: Vec<DomainCommand>,
}

impl Ledger {
    /// A ledger in `currency` holding `categories` and `accounts`, each account landing
    /// last in its sub-group. Fails when an amount is in another currency than the
    /// ledger, or when a balance overflows.
    ///
    /// Panics when a category sits under one that isn't before it in `categories`, or
    /// when a transaction names a category that isn't in `categories`.
    pub(crate) fn with_accounts(
        currency: CurrencyCode,
        categories: impl IntoIterator<Item = NewCategory>,
        accounts: impl IntoIterator<Item = NewAccount>,
    ) -> Result<Self, MoneyError> {
        let categories: Vec<Category> = (0..)
            .zip(categories)
            .map(|(id, new)| new.into_category(CategoryId::new(id)))
            .collect();
        for category in &categories {
            if let Some(parent) = category.parent() {
                assert!(
                    parent < category.id(),
                    "{:?} sits under a later category, {parent:?}",
                    category.id()
                );
            }
        }
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
            // Ids follow posting order, so this keeps a date's transactions in that order.
            posted.sort_by_key(Transaction::register_order);
            let summary = AccountSummary::new(account, posted, currency)?;
            net_worth = net_worth.checked_add(summary.balance)?;
            // Last in its sub-group: after every account of its kind or an earlier one.
            let position = summaries.partition_point(|summary| summary.account.kind() <= kind);
            summaries.insert(position, summary);
        }
        Ok(Self {
            view: Arc::new(LedgerView {
                generation: Generation::FIRST,
                accounts: summaries,
                categories,
                net_worth,
                can_undo: false,
                can_redo: false,
            }),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    /// The current snapshot. It stays valid, unchanged, after the ledger changes.
    pub fn snapshot(&self) -> Arc<LedgerView> {
        Arc::clone(&self.view)
    }

    /// Applies `command`, publishing a new snapshot with the next generation, or
    /// rejects it and leaves the ledger as it was. An applied change can be undone,
    /// and nothing undone before it can be redone any more.
    pub fn apply(&mut self, command: DomainCommand) -> Result<Applied, DomainError> {
        let applied = self.apply_unrecorded(command)?;
        self.undo_stack.push(applied.inverse().clone());
        self.redo_stack.clear();
        self.publish_history();
        Ok(applied)
    }

    /// Reverses the latest change not yet undone, publishing a new snapshot. Does
    /// nothing when there is no change to undo. Fails, leaving the ledger and the
    /// change to undo as they were, when the ledger refuses the reversal.
    pub fn undo(&mut self) -> Result<(), DomainError> {
        let Some(inverse) = self.undo_stack.last().cloned() else {
            return Ok(());
        };
        let applied = self.apply_unrecorded(inverse)?;
        self.undo_stack.pop();
        self.redo_stack.push(applied.into_inverse());
        self.publish_history();
        Ok(())
    }

    /// Makes the latest undone change again, publishing a new snapshot. Does nothing
    /// when there is no undone change to redo. Fails, leaving the ledger and the
    /// change to redo as they were, when the ledger refuses the change.
    pub fn redo(&mut self) -> Result<(), DomainError> {
        let Some(command) = self.redo_stack.last().cloned() else {
            return Ok(());
        };
        let applied = self.apply_unrecorded(command)?;
        self.redo_stack.pop();
        self.undo_stack.push(applied.into_inverse());
        self.publish_history();
        Ok(())
    }

    /// Tells the current snapshot whether Undo and Redo can act.
    fn publish_history(&mut self) {
        let view = Arc::make_mut(&mut self.view);
        view.can_undo = !self.undo_stack.is_empty();
        view.can_redo = !self.redo_stack.is_empty();
    }

    /// Applies `command` without recording it for Undo.
    fn apply_unrecorded(&mut self, command: DomainCommand) -> Result<Applied, DomainError> {
        let inverse = match command {
            DomainCommand::EditTransaction { transaction, edit } => {
                self.edit_transaction(transaction, edit)?
            }
            DomainCommand::DeleteTransaction { transaction } => {
                self.delete_transaction(transaction)?
            }
            DomainCommand::RestoreTransaction {
                account,
                transaction,
            } => self.restore_transaction(account, transaction)?,
        };
        Ok(Applied { inverse })
    }

    /// Where transaction `id` is, when its account is open to changes.
    fn changeable(&self, id: TransactionId) -> Result<TransactionPosition, DomainError> {
        let position = self
            .view
            .find_transaction(id)
            .ok_or(DomainError::UnknownTransaction { transaction: id })?;
        // `find_transaction` returns the index of an account in the view.
        let account = &self.view.accounts[position.account].account;
        if account.is_closed() {
            return Err(DomainError::AccountClosed {
                account: account.id(),
            });
        }
        Ok(position)
    }

    fn edit_transaction(
        &mut self,
        id: TransactionId,
        edit: TransactionEdit,
    ) -> Result<DomainCommand, DomainError> {
        let position = self.changeable(id)?;
        // Both indices were just found in this same view.
        if let TransactionEdit::Category(category) = edit
            && self.view.category(category).is_none()
        {
            return Err(DomainError::UnknownCategory { category });
        }
        let old = &self.view.accounts[position.account].register[position.entry].transaction;
        let undo = old.undoing(&edit);
        let edited = old.edited(edit)?;
        if edited.date() == old.date() {
            // Windows still holding the old snapshot keep it; the ledger moves to a copy.
            let view = Arc::make_mut(&mut self.view);
            view.accounts[position.account].register[position.entry].transaction = edited;
            view.generation = view.generation.next();
        } else {
            // A new date moves the transaction into date order, which changes the running
            // balances between its old place and its new one.
            let currency = self.view.net_worth.currency();
            let (summary, ()) =
                self.view
                    .edit_register(position.account, currency, |transactions| {
                        transactions[position.entry] = edited;
                        transactions.sort_by_key(Transaction::register_order);
                    })?;
            self.replace_account(position.account, summary)?;
        }
        Ok(DomainCommand::EditTransaction {
            transaction: id,
            edit: undo,
        })
    }

    fn delete_transaction(&mut self, id: TransactionId) -> Result<DomainCommand, DomainError> {
        let position = self.changeable(id)?;
        let currency = self.view.net_worth.currency();
        let (summary, transaction) =
            self.view
                .edit_register(position.account, currency, |transactions| {
                    transactions.remove(position.entry)
                })?;
        let account = summary.account.id();
        self.replace_account(position.account, summary)?;
        Ok(DomainCommand::RestoreTransaction {
            account,
            transaction,
        })
    }

    fn restore_transaction(
        &mut self,
        account: AccountId,
        transaction: Transaction,
    ) -> Result<DomainCommand, DomainError> {
        debug_assert!(
            self.view.find_transaction(transaction.id()).is_none(),
            "a restored transaction's id is not in use"
        );
        let index = self
            .view
            .accounts
            .iter()
            .position(|summary| summary.account.id() == account)
            .ok_or(DomainError::UnknownAccount { account })?;
        let currency = self.view.net_worth.currency();
        let inverse = DomainCommand::DeleteTransaction {
            transaction: transaction.id(),
        };
        let (summary, ()) = self.view.edit_register(index, currency, |transactions| {
            let key = transaction.register_order();
            let Err(position) =
                transactions.binary_search_by_key(&key, Transaction::register_order)
            else {
                panic!(
                    "restored {:?}, which its account still holds",
                    transaction.id()
                );
            };
            transactions.insert(position, transaction);
        })?;
        self.replace_account(index, summary)?;
        Ok(inverse)
    }

    /// Puts `summary` in place of the account at `index`, recounting Net Worth, and
    /// moves to the next generation. Fails, changing nothing, when Net Worth overflows.
    fn replace_account(&mut self, index: usize, summary: AccountSummary) -> Result<(), MoneyError> {
        let net_worth = self
            .view
            .accounts
            .iter()
            .enumerate()
            .map(|(other, unchanged)| {
                if other == index {
                    summary.balance
                } else {
                    unchanged.balance
                }
            })
            .try_fold(
                Money::zero(self.view.net_worth.currency()),
                Money::checked_add,
            )?;
        // Windows still holding the old snapshot keep it; the ledger moves to a copy.
        let view = Arc::make_mut(&mut self.view);
        // Callers take `index` from a lookup in this same list of accounts.
        view.accounts[index] = summary;
        view.net_worth = net_worth;
        view.generation = view.generation.next();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use icu_calendar::{Date, Iso};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::transaction::{NewTransaction, SplitLine};
    use crate::{AccountKind, Allocation, TransactionEdit};

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

    #[test]
    fn a_category_path_runs_from_the_top_of_the_tree() {
        let food = CategoryId::new(0);
        let restaurants = CategoryId::new(1);
        let coffee = CategoryId::new(2);
        let ledger = Ledger::with_accounts(
            usd_code(),
            [
                NewCategory::new("Food & Dining"),
                NewCategory::new("Restaurants").under(food),
                NewCategory::new("Coffee Shops").under(restaurants),
            ],
            [],
        )
        .unwrap();

        let view = ledger.snapshot();

        assert_eq!(
            view.category_path(coffee).unwrap(),
            ["Food & Dining", "Restaurants", "Coffee Shops"]
        );
        assert_eq!(view.category_path(food).unwrap(), ["Food & Dining"]);
    }

    /// A ledger with one checking account: a 100.00 opening balance, then Acme Corp,
    /// Fresh Market and Bean There, in that order.
    fn three_payments() -> Ledger {
        let groceries = CategoryId::new(0);
        Ledger::with_accounts(
            usd_code(),
            [NewCategory::new("Groceries")],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(100.00))).with_transactions([
                    NewTransaction::new(date(5), "Acme Corp", groceries, usd(dec!(500.00))),
                    NewTransaction::new(date(10), "Fresh Market", groceries, usd(dec!(-30.25))),
                    NewTransaction::new(date(10), "Bean There", groceries, usd(dec!(-4.75))),
                ]),
                account("Visa", AccountKind::CreditCard, usd(dec!(-50.00))),
            ],
        )
        .unwrap()
    }

    /// The id of the transaction with `payee` in the ledger's first account.
    fn transaction(ledger: &Ledger, payee: &str) -> TransactionId {
        ledger.snapshot().accounts()[0]
            .register()
            .iter()
            .map(RegisterEntry::transaction)
            .find(|transaction| transaction.payee() == payee)
            .unwrap()
            .id()
    }

    /// Each transaction of the ledger's first account, by payee, with its running balance.
    fn register(ledger: &Ledger) -> Vec<(String, Money)> {
        ledger.snapshot().accounts()[0]
            .register()
            .iter()
            .map(|entry| (entry.transaction().payee().to_owned(), entry.balance()))
            .collect()
    }

    #[test]
    fn deleting_a_transaction_takes_it_out_of_the_running_balances() {
        let mut ledger = three_payments();
        let fresh_market = transaction(&ledger, "Fresh Market");

        ledger
            .apply(DomainCommand::DeleteTransaction {
                transaction: fresh_market,
            })
            .unwrap();

        let view = ledger.snapshot();
        assert_eq!(
            register(&ledger),
            [
                (String::new(), usd(dec!(100.00))),
                ("Acme Corp".to_owned(), usd(dec!(600.00))),
                ("Bean There".to_owned(), usd(dec!(595.25))),
            ]
        );
        assert_eq!(view.accounts()[0].balance(), usd(dec!(595.25)));
        assert_eq!(view.net_worth(), usd(dec!(545.25)));
    }

    #[test]
    fn deleting_a_transaction_advances_the_generation() {
        let mut ledger = three_payments();
        let before = ledger.snapshot().generation();
        let fresh_market = transaction(&ledger, "Fresh Market");

        ledger
            .apply(DomainCommand::DeleteTransaction {
                transaction: fresh_market,
            })
            .unwrap();

        assert!(ledger.snapshot().generation() > before);
    }

    #[test]
    fn undoing_a_delete_puts_the_transaction_back_where_it_was() {
        let mut ledger = three_payments();
        let before = ledger.snapshot();
        let fresh_market = transaction(&ledger, "Fresh Market");
        let applied = ledger
            .apply(DomainCommand::DeleteTransaction {
                transaction: fresh_market,
            })
            .unwrap();

        ledger.apply(applied.into_inverse()).unwrap();

        let after = ledger.snapshot();
        assert_eq!(after.accounts(), before.accounts());
        assert_eq!(after.net_worth(), before.net_worth());
    }

    #[test]
    fn the_inverse_of_undoing_a_delete_deletes_again() {
        let mut ledger = three_payments();
        let fresh_market = transaction(&ledger, "Fresh Market");
        let deleted = ledger
            .apply(DomainCommand::DeleteTransaction {
                transaction: fresh_market,
            })
            .unwrap();

        let restored = ledger.apply(deleted.into_inverse()).unwrap();

        assert_eq!(
            restored.inverse(),
            &DomainCommand::DeleteTransaction {
                transaction: fresh_market
            }
        );
    }

    #[test]
    fn undo_puts_a_deleted_transaction_back() {
        let mut ledger = three_payments();
        let before = register(&ledger);
        let fresh_market = transaction(&ledger, "Fresh Market");
        ledger
            .apply(DomainCommand::DeleteTransaction {
                transaction: fresh_market,
            })
            .unwrap();

        ledger.undo().unwrap();

        assert_eq!(register(&ledger), before);
    }

    #[test]
    fn redo_deletes_an_undeleted_transaction_again() {
        let mut ledger = three_payments();
        let fresh_market = transaction(&ledger, "Fresh Market");
        ledger
            .apply(DomainCommand::DeleteTransaction {
                transaction: fresh_market,
            })
            .unwrap();
        let deleted = register(&ledger);
        ledger.undo().unwrap();

        ledger.redo().unwrap();

        assert_eq!(register(&ledger), deleted);
    }

    #[test]
    fn deleting_a_deleted_transaction_is_rejected_and_changes_nothing() {
        let mut ledger = three_payments();
        let fresh_market = transaction(&ledger, "Fresh Market");
        ledger
            .apply(DomainCommand::DeleteTransaction {
                transaction: fresh_market,
            })
            .unwrap();
        let before = ledger.snapshot();

        let result = ledger.apply(DomainCommand::DeleteTransaction {
            transaction: fresh_market,
        });

        assert_eq!(
            result.unwrap_err(),
            DomainError::UnknownTransaction {
                transaction: fresh_market
            }
        );
        assert_eq!(ledger.snapshot(), before);
    }

    #[test]
    fn deleting_from_a_closed_account_is_rejected() {
        let groceries = CategoryId::new(0);
        let mut ledger = Ledger::with_accounts(
            usd_code(),
            [NewCategory::new("Groceries")],
            [account("Old Checking", AccountKind::Checking, usd(dec!(1)))
                .closed()
                .with_transactions([NewTransaction::new(
                    date(2),
                    "Fresh Market",
                    groceries,
                    usd(dec!(-1)),
                )])],
        )
        .unwrap();
        let before = ledger.snapshot();
        let fresh_market = transaction(&ledger, "Fresh Market");

        let result = ledger.apply(DomainCommand::DeleteTransaction {
            transaction: fresh_market,
        });

        let old_checking = before.accounts()[0].account().id();
        assert_eq!(
            result.unwrap_err(),
            DomainError::AccountClosed {
                account: old_checking
            }
        );
        assert_eq!(ledger.snapshot(), before);
    }

    #[test]
    fn restoring_into_an_unknown_account_is_rejected() {
        let mut ledger = three_payments();
        let fresh_market = transaction(&ledger, "Fresh Market");
        let applied = ledger
            .apply(DomainCommand::DeleteTransaction {
                transaction: fresh_market,
            })
            .unwrap();
        let DomainCommand::RestoreTransaction { transaction, .. } = applied.into_inverse() else {
            panic!("a delete's inverse restores the transaction");
        };
        let unknown = AccountId::new(9);

        let result = ledger.apply(DomainCommand::RestoreTransaction {
            account: unknown,
            transaction,
        });

        assert_eq!(
            result.unwrap_err(),
            DomainError::UnknownAccount { account: unknown }
        );
    }

    #[test]
    #[should_panic(expected = "sits under a later category")]
    fn a_category_under_a_later_one_is_a_bug() {
        Ledger::with_accounts(
            usd_code(),
            [
                NewCategory::new("Groceries").under(CategoryId::new(1)),
                NewCategory::new("Food & Dining"),
            ],
            [],
        )
        .ok();
    }

    /// A ledger with the categories Groceries and Household, and one open account,
    /// Everyday, holding its opening balance and one payment to Fresh Market in
    /// Groceries, numbered 1041 with a memo.
    fn everyday() -> Ledger {
        let groceries = CategoryId::new(0);
        Ledger::with_accounts(
            usd_code(),
            [NewCategory::new("Groceries"), NewCategory::new("Household")],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(100.00))).with_transactions([
                    NewTransaction::new(date(5), "Fresh Market", groceries, usd(dec!(-30.25)))
                        .number("1041")
                        .memo("Weekly shop"),
                ]),
            ],
        )
        .unwrap()
    }

    /// The id of the `index`th transaction in the first account's register.
    fn transaction_id(ledger: &Ledger, index: usize) -> TransactionId {
        ledger.snapshot().accounts()[0].register()[index]
            .transaction()
            .id()
    }

    fn edit(transaction: TransactionId, edit: TransactionEdit) -> DomainCommand {
        DomainCommand::EditTransaction { transaction, edit }
    }

    #[test]
    fn editing_a_payee_shows_in_the_next_snapshot() {
        let mut ledger = everyday();
        let fresh_market = transaction_id(&ledger, 1);

        ledger
            .apply(edit(
                fresh_market,
                TransactionEdit::Payee("Corner Shop".into()),
            ))
            .unwrap();

        let view = ledger.snapshot();
        let transaction = view.accounts()[0].register()[1].transaction();
        assert_eq!(transaction.payee(), "Corner Shop");
        assert_ne!(view.generation(), Generation::FIRST);
    }

    #[test]
    fn undoing_an_edit_puts_back_each_field() {
        let mut ledger = everyday();
        let before = ledger.snapshot();
        let fresh_market = transaction_id(&ledger, 1);
        let edits = [
            TransactionEdit::Number(Some("1043".into())),
            TransactionEdit::Payee("Corner Shop".into()),
            TransactionEdit::Memo("Weekly shop".into()),
        ];
        let inverses: Vec<_> = edits
            .into_iter()
            .map(|change| {
                ledger
                    .apply(edit(fresh_market, change))
                    .unwrap()
                    .into_inverse()
            })
            .collect();

        for inverse in inverses.into_iter().rev() {
            ledger.apply(inverse).unwrap();
        }

        assert_eq!(ledger.snapshot().accounts(), before.accounts());
    }

    #[test]
    fn editing_a_number_and_a_memo_shows_in_the_next_snapshot() {
        let mut ledger = everyday();
        let fresh_market = transaction_id(&ledger, 1);

        ledger
            .apply(edit(
                fresh_market,
                TransactionEdit::Number(Some("1042".into())),
            ))
            .unwrap();
        ledger
            .apply(edit(
                fresh_market,
                TransactionEdit::Memo("Party food".into()),
            ))
            .unwrap();

        let view = ledger.snapshot();
        let transaction = view.accounts()[0].register()[1].transaction();
        assert_eq!(
            (transaction.number(), transaction.memo()),
            (Some("1042"), "Party food")
        );
    }

    #[test]
    fn a_number_can_be_cleared() {
        let mut ledger = everyday();
        let fresh_market = transaction_id(&ledger, 1);

        ledger
            .apply(edit(fresh_market, TransactionEdit::Number(None)))
            .unwrap();

        let view = ledger.snapshot();
        assert_eq!(
            view.accounts()[0].register()[1].transaction().number(),
            None
        );
    }

    #[test]
    fn each_applied_command_moves_to_a_new_generation() {
        let mut ledger = everyday();
        let fresh_market = transaction_id(&ledger, 1);
        ledger
            .apply(edit(fresh_market, TransactionEdit::Memo("A".into())))
            .unwrap();
        let first = ledger.snapshot().generation();

        ledger
            .apply(edit(fresh_market, TransactionEdit::Memo("B".into())))
            .unwrap();

        assert!(ledger.snapshot().generation() > first);
    }

    #[test]
    fn an_earlier_snapshot_is_unchanged_by_an_edit() {
        let mut ledger = everyday();
        let before = ledger.snapshot();
        let fresh_market = transaction_id(&ledger, 1);

        ledger
            .apply(edit(
                fresh_market,
                TransactionEdit::Payee("Corner Shop".into()),
            ))
            .unwrap();

        let transaction = before.accounts()[0].register()[1].transaction();
        assert_eq!(transaction.payee(), "Fresh Market");
        assert_eq!(before.generation(), Generation::FIRST);
    }

    #[test]
    fn editing_a_closed_accounts_transaction_is_rejected() {
        let groceries = CategoryId::new(0);
        let mut ledger = Ledger::with_accounts(
            usd_code(),
            [NewCategory::new("Groceries")],
            [account("Old", AccountKind::Checking, usd(dec!(10.00)))
                .with_transactions([NewTransaction::new(
                    date(5),
                    "Fresh Market",
                    groceries,
                    usd(dec!(-10.00)),
                )])
                .closed()],
        )
        .unwrap();
        let before = ledger.snapshot();
        let old = before.accounts()[0].account().id();
        let fresh_market = transaction_id(&ledger, 1);

        let result = ledger.apply(edit(fresh_market, TransactionEdit::Memo("Late".into())));

        assert_eq!(result, Err(DomainError::AccountClosed { account: old }));
        assert_eq!(ledger.snapshot(), before);
    }

    #[test]
    fn editing_an_unknown_transaction_is_rejected() {
        let mut ledger = everyday();
        let unknown = TransactionId::new(99);

        let result = ledger.apply(edit(unknown, TransactionEdit::Memo("Lost".into())));

        assert_eq!(
            result,
            Err(DomainError::UnknownTransaction {
                transaction: unknown
            })
        );
    }

    #[test]
    fn the_opening_balance_takes_no_payee() {
        let mut ledger = everyday();
        let before = ledger.snapshot();
        let opening = transaction_id(&ledger, 0);

        let result = ledger.apply(edit(opening, TransactionEdit::Payee("Bank".into())));

        assert_eq!(
            result,
            Err(DomainError::OpeningBalancePayee {
                transaction: opening
            })
        );
        assert_eq!(ledger.snapshot(), before);
    }

    /// The `index`th transaction in the first account's register.
    fn nth_transaction(ledger: &Ledger, index: usize) -> Transaction {
        ledger.snapshot().accounts()[0].register()[index]
            .transaction()
            .clone()
    }

    #[test]
    fn undo_puts_back_each_field_an_edit_changed() {
        let mut ledger = everyday();
        let fresh_market = transaction_id(&ledger, 1);
        let before = nth_transaction(&ledger, 1);
        for change in [
            TransactionEdit::Number(None),
            TransactionEdit::Payee("Corner Shop".into()),
            TransactionEdit::Memo("Party food".into()),
        ] {
            ledger.apply(edit(fresh_market, change)).unwrap();
        }

        for _ in 0..3 {
            ledger.undo().unwrap();
        }

        assert_eq!(nth_transaction(&ledger, 1), before);
    }

    #[test]
    fn undo_undoes_the_latest_change_first() {
        let mut ledger = everyday();
        let fresh_market = transaction_id(&ledger, 1);
        ledger
            .apply(edit(fresh_market, TransactionEdit::Memo("A".into())))
            .unwrap();
        ledger
            .apply(edit(fresh_market, TransactionEdit::Memo("B".into())))
            .unwrap();

        ledger.undo().unwrap();

        assert_eq!(nth_transaction(&ledger, 1).memo(), "A");
    }

    #[test]
    fn redo_makes_each_undone_edit_again() {
        let mut ledger = everyday();
        let fresh_market = transaction_id(&ledger, 1);
        for change in [
            TransactionEdit::Number(None),
            TransactionEdit::Payee("Corner Shop".into()),
            TransactionEdit::Memo("Party food".into()),
        ] {
            ledger.apply(edit(fresh_market, change)).unwrap();
        }
        let edited = nth_transaction(&ledger, 1);
        for _ in 0..3 {
            ledger.undo().unwrap();
        }

        for _ in 0..3 {
            ledger.redo().unwrap();
        }

        assert_eq!(nth_transaction(&ledger, 1), edited);
    }

    #[test]
    fn a_new_change_leaves_nothing_to_redo() {
        let mut ledger = everyday();
        let fresh_market = transaction_id(&ledger, 1);
        ledger
            .apply(edit(fresh_market, TransactionEdit::Memo("A".into())))
            .unwrap();
        ledger.undo().unwrap();
        ledger
            .apply(edit(fresh_market, TransactionEdit::Payee("B".into())))
            .unwrap();
        let before = ledger.snapshot();

        ledger.redo().unwrap();

        assert_eq!(ledger.snapshot(), before);
    }

    /// Whether the ledger's latest snapshot says Undo and Redo can act.
    fn can_undo_and_redo(ledger: &Ledger) -> (bool, bool) {
        let view = ledger.snapshot();
        (view.can_undo(), view.can_redo())
    }

    #[test]
    fn the_snapshot_says_whether_undo_and_redo_can_act() {
        let mut ledger = everyday();
        let fresh_market = transaction_id(&ledger, 1);
        assert_eq!(can_undo_and_redo(&ledger), (false, false));

        ledger
            .apply(edit(fresh_market, TransactionEdit::Memo("A".into())))
            .unwrap();
        assert_eq!(can_undo_and_redo(&ledger), (true, false));

        ledger.undo().unwrap();
        assert_eq!(can_undo_and_redo(&ledger), (false, true));

        ledger.redo().unwrap();
        assert_eq!(can_undo_and_redo(&ledger), (true, false));
    }

    #[test]
    fn undo_and_redo_with_nothing_to_do_leave_the_ledger_as_it_was() {
        let mut ledger = everyday();
        let before = ledger.snapshot();

        ledger.undo().unwrap();
        ledger.redo().unwrap();

        assert_eq!(ledger.snapshot(), before);
    }

    #[test]
    fn a_new_date_moves_the_transaction_into_date_order_and_rebalances() {
        let groceries = CategoryId::new(0);
        let mut ledger = Ledger::with_accounts(
            usd_code(),
            [NewCategory::new("Groceries")],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(100.00))).with_transactions([
                    NewTransaction::new(date(5), "Fresh Market", groceries, usd(dec!(-30.00))),
                    NewTransaction::new(date(10), "Bean There", groceries, usd(dec!(-5.00))),
                    NewTransaction::new(date(10), "Corner Shop", groceries, usd(dec!(-1.00))),
                ]),
            ],
        )
        .unwrap();
        let fresh_market = transaction_id(&ledger, 1);

        ledger
            .apply(edit(fresh_market, TransactionEdit::Date(date(10))))
            .unwrap();

        let view = ledger.snapshot();
        let register: Vec<_> = view.accounts()[0]
            .register()
            .iter()
            .map(|entry| {
                let transaction = entry.transaction();
                (transaction.payee(), transaction.date(), entry.balance())
            })
            .collect();
        assert_eq!(
            register,
            [
                ("", date(1), usd(dec!(100.00))),
                ("Fresh Market", date(10), usd(dec!(70.00))),
                ("Bean There", date(10), usd(dec!(65.00))),
                ("Corner Shop", date(10), usd(dec!(64.00))),
            ]
        );
    }

    #[test]
    fn a_later_date_moves_the_transaction_after_its_new_neighbours() {
        let groceries = CategoryId::new(0);
        let mut ledger = Ledger::with_accounts(
            usd_code(),
            [NewCategory::new("Groceries")],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(100.00))).with_transactions([
                    NewTransaction::new(date(5), "Fresh Market", groceries, usd(dec!(-30.00))),
                    NewTransaction::new(date(10), "Bean There", groceries, usd(dec!(-5.00))),
                ]),
            ],
        )
        .unwrap();
        let fresh_market = transaction_id(&ledger, 1);

        ledger
            .apply(edit(fresh_market, TransactionEdit::Date(date(20))))
            .unwrap();

        let view = ledger.snapshot();
        let register: Vec<_> = view.accounts()[0]
            .register()
            .iter()
            .map(|entry| (entry.transaction().payee(), entry.balance()))
            .collect();
        assert_eq!(
            register,
            [
                ("", usd(dec!(100.00))),
                ("Bean There", usd(dec!(95.00))),
                ("Fresh Market", usd(dec!(65.00))),
            ]
        );
    }

    #[test]
    fn undoing_a_date_edit_puts_the_transaction_back_where_it_was() {
        let groceries = CategoryId::new(0);
        let mut ledger = Ledger::with_accounts(
            usd_code(),
            [NewCategory::new("Groceries")],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(100.00))).with_transactions([
                    NewTransaction::new(date(5), "Fresh Market", groceries, usd(dec!(-30.00))),
                    NewTransaction::new(date(10), "Bean There", groceries, usd(dec!(-5.00))),
                ]),
            ],
        )
        .unwrap();
        let before = ledger.snapshot().accounts()[0].clone();
        let fresh_market = transaction_id(&ledger, 1);
        ledger
            .apply(edit(fresh_market, TransactionEdit::Date(date(20))))
            .unwrap();

        ledger.undo().unwrap();

        assert_eq!(ledger.snapshot().accounts()[0], before);
    }

    #[test]
    fn the_opening_balance_takes_a_memo() {
        let mut ledger = everyday();
        let opening = transaction_id(&ledger, 0);

        ledger
            .apply(edit(opening, TransactionEdit::Memo("Moved banks".into())))
            .unwrap();

        let view = ledger.snapshot();
        assert_eq!(
            view.accounts()[0].register()[0].transaction().memo(),
            "Moved banks"
        );
    }

    #[test]
    fn editing_a_category_shows_in_the_next_snapshot() {
        let mut ledger = everyday();
        let fresh_market = transaction_id(&ledger, 1);
        let household = CategoryId::new(1);

        ledger
            .apply(edit(fresh_market, TransactionEdit::Category(household)))
            .unwrap();

        let view = ledger.snapshot();
        assert_eq!(
            view.accounts()[0].register()[1].transaction().allocation(),
            &Allocation::Category(household)
        );
    }

    #[test]
    fn the_opening_balance_takes_no_category() {
        let mut ledger = everyday();
        let before = ledger.snapshot();
        let opening = transaction_id(&ledger, 0);

        let result = ledger.apply(edit(opening, TransactionEdit::Category(CategoryId::new(0))));

        assert_eq!(
            result,
            Err(DomainError::NotInOneCategory {
                transaction: opening
            })
        );
        assert_eq!(ledger.snapshot(), before);
    }

    #[test]
    fn a_split_transaction_takes_no_single_category() {
        let groceries = CategoryId::new(0);
        let mut ledger = Ledger::with_accounts(
            usd_code(),
            [NewCategory::new("Groceries")],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(500.00))).with_transactions([
                    NewTransaction::split(
                        date(12),
                        "Costco",
                        vec![
                            SplitLine::new(groceries, "Food", usd(dec!(-10.00))),
                            SplitLine::new(groceries, "Snacks", usd(dec!(-5.00))),
                        ],
                    ),
                ]),
            ],
        )
        .unwrap();
        let before = ledger.snapshot();
        let costco = transaction_id(&ledger, 1);

        let result = ledger.apply(edit(costco, TransactionEdit::Category(groceries)));

        assert_eq!(
            result,
            Err(DomainError::NotInOneCategory {
                transaction: costco
            })
        );
        assert_eq!(ledger.snapshot(), before);
    }

    #[test]
    fn editing_to_an_unknown_category_is_rejected() {
        let mut ledger = everyday();
        let before = ledger.snapshot();
        let fresh_market = transaction_id(&ledger, 1);
        let unknown = CategoryId::new(99);

        let result = ledger.apply(edit(fresh_market, TransactionEdit::Category(unknown)));

        assert_eq!(
            result,
            Err(DomainError::UnknownCategory { category: unknown })
        );
        assert_eq!(ledger.snapshot(), before);
    }

    #[test]
    fn a_category_edit_is_undone_by_putting_the_old_category_back() {
        let mut ledger = everyday();
        let fresh_market = transaction_id(&ledger, 1);
        let groceries = CategoryId::new(0);

        let applied = ledger
            .apply(edit(
                fresh_market,
                TransactionEdit::Category(CategoryId::new(1)),
            ))
            .unwrap();

        assert_eq!(
            applied.inverse(),
            &edit(fresh_market, TransactionEdit::Category(groceries))
        );
    }

    #[test]
    fn the_categories_are_listed_in_the_order_they_were_added() {
        let ledger = everyday();

        let view = ledger.snapshot();

        let names: Vec<&str> = view.categories().iter().map(Category::name).collect();
        assert_eq!(names, ["Groceries", "Household"]);
    }
}
