//! The [`Ledger`]: the financial data, and the snapshots windows read it through.

use std::collections::HashMap;
use std::sync::Arc;

use balanced_store::{Stack, Store, StoreError};
use icu_calendar::{Date, Iso};

use crate::account::NewAccount;
use crate::category::NewCategory;
use crate::data_file::{account_row, category_row, transaction_row};
use crate::transaction::{NewTransaction, TransactionId};
use crate::undo_log;
use crate::{
    Account, AccountGroup, AccountId, AccountKind, AccountStatus, Applied, Category, CategoryId,
    CurrencyCode, DomainCommand, DomainError, Money, MoneyError, MoveDirection, Transaction,
    TransactionEdit, Visibility, WriteError,
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
    pub(crate) fn new(transaction: Transaction, balance: Money) -> Self {
        Self {
            transaction,
            balance,
        }
    }

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

    /// `account` holding `register`, whose last running balance is `balance`, as the data
    /// file's views counted them.
    pub(crate) fn loaded(account: Account, register: Vec<RegisterEntry>, balance: Money) -> Self {
        Self {
            account,
            register,
            balance,
        }
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
    totals: Totals,
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

    /// The indices in [`Self::accounts`] of the accounts of `kind`. Where there are
    /// none, it is the empty range where one would go.
    fn sub_group(&self, kind: AccountKind) -> std::ops::Range<usize> {
        let start = self
            .accounts
            .partition_point(|summary| summary.account.kind() < kind);
        let end = self
            .accounts
            .partition_point(|summary| summary.account.kind() <= kind);
        start..end
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

    /// The sum of the balances of `group`'s open accounts, hidden accounts included.
    /// Fails when the sum overflows.
    pub fn group_total(&self, group: AccountGroup) -> Result<Money, MoneyError> {
        // `index` counts within `AccountGroup::ALL`, which sizes `groups`.
        self.totals.groups[group.index()].clone()
    }

    /// The sum of the balances of every closed account. Fails when the sum overflows.
    pub fn closed_total(&self) -> Result<Money, MoneyError> {
        self.totals.closed.clone()
    }

    /// Counts the group totals and the closed total again after the accounts changed,
    /// and moves to the next generation.
    fn accounts_changed(&mut self) {
        self.totals = Totals::count(&self.accounts, self.net_worth.currency());
        self.generation = self.generation.next();
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

/// The totals a [`LedgerView`] shows besides Net Worth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Totals {
    /// Each group's total, in the order of [`AccountGroup::ALL`].
    groups: [Result<Money, MoneyError>; AccountGroup::ALL.len()],
    closed: Result<Money, MoneyError>,
}

impl Totals {
    /// `groups`, in the order of [`AccountGroup::ALL`], and `closed`, as the data
    /// file's views counted them.
    pub(crate) fn new(groups: [Money; AccountGroup::ALL.len()], closed: Money) -> Self {
        Self {
            groups: groups.map(Ok),
            closed: Ok(closed),
        }
    }

    /// The totals of `accounts`, in `currency`.
    fn count(accounts: &[AccountSummary], currency: CurrencyCode) -> Self {
        let total = |include: &dyn Fn(&Account) -> bool| {
            accounts
                .iter()
                .filter(|summary| include(&summary.account))
                .try_fold(Money::zero(currency), |total, summary| {
                    total.checked_add(summary.balance)
                })
        };
        Self {
            groups: AccountGroup::ALL.map(|group| {
                total(&|account: &Account| !account.is_closed() && account.kind().group() == group)
            }),
            closed: total(&Account::is_closed),
        }
    }
}

/// The ids the next account, transaction and category take. Ids are never reused.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NextIds {
    pub(crate) account: AccountId,
    pub(crate) transaction: TransactionId,
    pub(crate) category: CategoryId,
}

/// How many changes Undo keeps. Applying another drops the oldest.
const UNDO_LIMIT: usize = 100;

/// The Undo and Redo stacks a data file holds: the inverses of the changes each can
/// make, the latest last.
#[derive(Debug, Default)]
pub(crate) struct History {
    pub(crate) undo: Vec<DomainCommand>,
    pub(crate) redo: Vec<DomainCommand>,
}

/// What [`Ledger::restore`] puts back: the ledger less its Undo and Redo history.
#[derive(Debug)]
struct Checkpoint {
    view: Arc<LedgerView>,
    next_account: AccountId,
    next_transaction: TransactionId,
    next_category: CategoryId,
}

/// The financial data. It changes only by applying a `DomainCommand`, and windows
/// read it through [`Ledger::snapshot`].
#[derive(Debug)]
pub struct Ledger {
    view: Arc<LedgerView>,
    /// The data file. Every change is written to it as memory changes.
    store: Store,
    /// The inverses of the changes Undo can reverse, the latest last.
    undo_stack: Vec<DomainCommand>,
    /// The inverses of the undone changes Redo can make again, the latest undone last.
    redo_stack: Vec<DomainCommand>,
    /// The id the next created account takes. Ids are never reused.
    next_account: AccountId,
    /// The id the next posted transaction takes. Ids are never reused.
    next_transaction: TransactionId,
    /// The id the next category created takes. Ids are never reused, so it only grows.
    next_category: CategoryId,
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
        let next_category = CategoryId::new(
            u32::try_from(categories.len()).expect("a ledger holds fewer than 2^32 categories"),
        );
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
        let mut next_account = AccountId::new(0);
        for (id, new) in (0..).zip(accounts) {
            next_account = AccountId::new(id).next();
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
        let store = in_memory_store(currency, &categories, &summaries);
        let totals = Totals::count(&summaries, currency);
        Ok(Self::from_parts(
            store,
            summaries,
            categories,
            net_worth,
            totals,
            NextIds {
                account: next_account,
                transaction: transaction_ids.next().expect("the ids never run out"),
                category: next_category,
            },
        ))
    }

    /// A ledger over `store`, which holds `accounts`, in Account bar order, and
    /// `categories`, with no Undo or Redo history. `net_worth` and `totals` are what
    /// `accounts` add up to.
    pub(crate) fn from_parts(
        store: Store,
        accounts: Vec<AccountSummary>,
        categories: Vec<Category>,
        net_worth: Money,
        totals: Totals,
        next: NextIds,
    ) -> Self {
        Self {
            view: Arc::new(LedgerView {
                generation: Generation::FIRST,
                accounts,
                categories,
                net_worth,
                totals,
                can_undo: false,
                can_redo: false,
            }),
            store,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            next_account: next.account,
            next_transaction: next.transaction,
            next_category: next.category,
        }
    }

    /// The ledger with `history` to undo and redo.
    pub(crate) fn with_history(mut self, history: History) -> Self {
        History {
            undo: self.undo_stack,
            redo: self.redo_stack,
        } = history;
        self.publish_history();
        self
    }

    /// The current snapshot. It stays valid, unchanged, after the ledger changes.
    pub fn snapshot(&self) -> Arc<LedgerView> {
        Arc::clone(&self.view)
    }

    /// Applies `command`, publishing a new snapshot with the next generation, or
    /// rejects it and leaves the ledger as it was. An applied change can be undone,
    /// and nothing undone before it can be redone any more.
    pub fn apply(&mut self, command: DomainCommand) -> Result<Applied, DomainError> {
        let full = self.undo_stack.len() >= UNDO_LIMIT;
        let applied = self.apply_logged(command, |store, entry| {
            store.push_entry(Stack::Undo, entry)?;
            if full {
                store.drop_oldest_entry(Stack::Undo)?;
            }
            store.clear_stack(Stack::Redo)
        })?;
        if full {
            self.undo_stack.remove(0);
        }
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
        let applied = self.apply_logged(inverse, |store, entry| {
            store.pop_entry(Stack::Undo)?;
            store.push_entry(Stack::Redo, entry)
        })?;
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
        let applied = self.apply_logged(command, |store, entry| {
            store.pop_entry(Stack::Redo)?;
            store.push_entry(Stack::Undo, entry)
        })?;
        self.redo_stack.pop();
        self.undo_stack.push(applied.into_inverse());
        self.publish_history();
        Ok(())
    }

    /// Applies `command` and makes `log`'s changes to the undo log, given the entry for
    /// the Command that undoes it, all in one transaction in the data file. The in-memory
    /// stacks are the caller's to change once this succeeds.
    fn apply_logged(
        &mut self,
        command: DomainCommand,
        log: impl FnOnce(&Store, &str) -> Result<(), StoreError>,
    ) -> Result<Applied, DomainError> {
        self.write(|ledger| {
            let applied = ledger.apply_unrecorded(command)?;
            log(&ledger.store, &undo_log::encode(applied.inverse())?)?;
            Ok(applied)
        })
    }

    /// Makes `change` in one transaction in the data file, which commits when `change`
    /// succeeds. When `change` or the commit fails, rolls the file back and puts the
    /// ledger back as it was, keeping its Undo and Redo history.
    fn write<T>(
        &mut self,
        change: impl FnOnce(&mut Self) -> Result<T, DomainError>,
    ) -> Result<T, DomainError> {
        let checkpoint = self.checkpoint();
        self.store.begin()?;
        let error = match change(self) {
            Ok(changed) => match self.store.commit() {
                Ok(()) => return Ok(changed),
                Err(error) => DomainError::from(error),
            },
            Err(error) => error,
        };
        let rolled_back = self.store.rollback();
        self.restore(checkpoint);
        match rolled_back {
            Ok(()) => Err(error),
            Err(source) => Err(DomainError::Store(WriteError::RollbackFailed {
                change: Box::new(error),
                source: Arc::new(source),
            })),
        }
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
            DomainCommand::CreateAccount {
                name,
                kind,
                opened,
                opening_balance,
            } => self.create_account(&name, kind, opened, opening_balance)?,
            DomainCommand::DeleteAccount { account } => self.delete_account(account)?,
            DomainCommand::RestoreAccount {
                account,
                transactions,
                place,
            } => self.restore_account(account, transactions, place)?,
            DomainCommand::RenameAccount { account, name } => self.rename_account(account, name)?,
            DomainCommand::SetAccountVisibility {
                account,
                visibility,
            } => self.set_account_visibility(account, visibility)?,
            DomainCommand::SetAccountStatus { account, status } => {
                self.set_account_status(account, status)?
            }
            DomainCommand::Compound(commands) => self.apply_compound(commands)?,
            DomainCommand::MoveAccount { account, direction } => {
                self.move_account(account, direction)?
            }
            DomainCommand::PostTransaction {
                account,
                transaction,
            } => self.post_transaction(account, transaction)?,
            DomainCommand::EditTransaction { transaction, edit } => {
                self.edit_transaction(transaction, edit)?
            }
            DomainCommand::SetFlagged {
                transaction,
                flagged,
            } => self.set_flagged(transaction, flagged)?,
            DomainCommand::DeleteTransaction { transaction } => {
                self.delete_transaction(transaction)?
            }
            DomainCommand::RestoreTransaction {
                account,
                transaction,
            } => self.restore_transaction(account, transaction)?,
            DomainCommand::CreateCategory { name, parent } => {
                self.create_category(&name, parent)?
            }
            DomainCommand::DeleteCategory { category } => self.delete_category(category)?,
            DomainCommand::RestoreCategory { category } => self.restore_category(category)?,
        };
        Ok(Applied { inverse })
    }

    /// Applies each of `commands` in order, returning the Command that undoes them all:
    /// their inverses, latest first. When one is refused, [`Self::write`] puts the
    /// ledger back as it was before the first.
    fn apply_compound(
        &mut self,
        commands: Vec<DomainCommand>,
    ) -> Result<DomainCommand, DomainError> {
        let mut inverses = Vec::with_capacity(commands.len());
        for command in commands {
            inverses.push(self.apply_unrecorded(command)?.into_inverse());
        }
        inverses.reverse();
        Ok(DomainCommand::Compound(inverses))
    }

    /// The ledger as it is now, less its Undo and Redo history.
    fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            view: Arc::clone(&self.view),
            next_account: self.next_account,
            next_transaction: self.next_transaction,
            next_category: self.next_category,
        }
    }

    /// Puts the ledger back as it was at `checkpoint`, keeping its Undo and Redo
    /// history.
    fn restore(&mut self, checkpoint: Checkpoint) {
        Checkpoint {
            view: self.view,
            next_account: self.next_account,
            next_transaction: self.next_transaction,
            next_category: self.next_category,
        } = checkpoint;
    }

    fn create_account(
        &mut self,
        name: &str,
        kind: AccountKind,
        opened: Date<Iso>,
        opening_balance: Money,
    ) -> Result<DomainCommand, DomainError> {
        if name.trim().is_empty() {
            return Err(DomainError::BlankAccountName);
        }
        let currency = self.view.net_worth.currency();
        let id = self.next_account;
        let (account, transactions) =
            NewAccount::new(name, kind, opened, opening_balance).into_account(id);
        let mut next_transaction = self.next_transaction;
        let mut posted = Vec::with_capacity(transactions.len());
        for new in transactions {
            posted.push(new.into_transaction(next_transaction, currency)?);
            next_transaction = next_transaction.next();
        }
        let summary = AccountSummary::new(account, posted, currency)?;
        let place = self.view.sub_group(kind).len();
        self.insert_account(summary, place)?;
        self.next_account = id.next();
        self.next_transaction = next_transaction;
        Ok(DomainCommand::DeleteAccount { account: id })
    }

    fn delete_account(&mut self, id: AccountId) -> Result<DomainCommand, DomainError> {
        let index = self
            .view
            .accounts
            .iter()
            .position(|summary| summary.account.id() == id)
            .ok_or(DomainError::UnknownAccount { account: id })?;
        // `position` found `index` in this same list of accounts.
        let place = index
            - self
                .view
                .sub_group(self.view.accounts[index].account.kind())
                .start;
        let currency = self.view.net_worth.currency();
        let net_worth = self
            .view
            .accounts
            .iter()
            .enumerate()
            .filter(|&(other, _)| other != index)
            .try_fold(Money::zero(currency), |sum, (_, summary)| {
                sum.checked_add(summary.balance)
            })?;
        self.store.delete_account(id.raw())?;
        let view = Arc::make_mut(&mut self.view);
        let summary = view.accounts.remove(index);
        view.net_worth = net_worth;
        view.accounts_changed();
        let transactions = summary.transactions();
        Ok(DomainCommand::RestoreAccount {
            account: summary.account,
            transactions,
            place,
        })
    }

    fn restore_account(
        &mut self,
        account: Account,
        transactions: Vec<Transaction>,
        place: usize,
    ) -> Result<DomainCommand, DomainError> {
        debug_assert!(
            self.view.account(account.id()).is_none(),
            "a restored account's id is not in use"
        );
        let id = account.id();
        let currency = self.view.net_worth.currency();
        let summary = AccountSummary::new(account, transactions, currency)?;
        self.insert_account(summary, place)?;
        Ok(DomainCommand::DeleteAccount { account: id })
    }

    /// The index in the view's accounts of account `id`.
    fn account_index(&self, id: AccountId) -> Result<usize, DomainError> {
        self.view
            .accounts
            .iter()
            .position(|summary| summary.account.id() == id)
            .ok_or(DomainError::UnknownAccount { account: id })
    }

    /// Changes the account at `index` with `change`, which leaves its balance as it
    /// was, and moves to the next generation. Returns what `change` returned.
    fn change_account<T>(&mut self, index: usize, change: impl FnOnce(&mut Account) -> T) -> T {
        let view = Arc::make_mut(&mut self.view);
        // Callers take `index` from a lookup in this same list of accounts.
        let changed = change(&mut view.accounts[index].account);
        view.accounts_changed();
        changed
    }

    fn rename_account(
        &mut self,
        id: AccountId,
        name: String,
    ) -> Result<DomainCommand, DomainError> {
        if name.trim().is_empty() {
            return Err(DomainError::BlankAccountName);
        }
        let index = self.account_index(id)?;
        self.store.rename_account(id.raw(), &name)?;
        let old = self.change_account(index, |account| account.rename(name));
        Ok(DomainCommand::RenameAccount {
            account: id,
            name: old,
        })
    }

    fn set_account_visibility(
        &mut self,
        id: AccountId,
        visibility: Visibility,
    ) -> Result<DomainCommand, DomainError> {
        let index = self.account_index(id)?;
        self.store
            .set_account_hidden(id.raw(), visibility == Visibility::Hidden)?;
        let old = self.change_account(index, |account| account.set_visibility(visibility));
        Ok(DomainCommand::SetAccountVisibility {
            account: id,
            visibility: old,
        })
    }

    fn set_account_status(
        &mut self,
        id: AccountId,
        status: AccountStatus,
    ) -> Result<DomainCommand, DomainError> {
        let index = self.account_index(id)?;
        self.store
            .set_account_closed(id.raw(), status == AccountStatus::Closed)?;
        let old = self.change_account(index, |account| account.set_status(status));
        Ok(DomainCommand::SetAccountStatus {
            account: id,
            status: old,
        })
    }

    fn move_account(
        &mut self,
        id: AccountId,
        direction: MoveDirection,
    ) -> Result<DomainCommand, DomainError> {
        let index = self.account_index(id)?;
        // `account_index` found `index` in this same list of accounts.
        let sub_group = self
            .view
            .sub_group(self.view.accounts[index].account.kind());
        let neighbour = direction
            .neighbour(index)
            .filter(|neighbour| sub_group.contains(neighbour))
            .ok_or(DomainError::SubGroupEdge { account: id })?;
        // `neighbour` is in the sub-group, so it is in this same list of accounts.
        let other = self.view.accounts[neighbour].account.id();
        self.store.swap_account_positions(id.raw(), other.raw())?;
        let view = Arc::make_mut(&mut self.view);
        view.accounts.swap(index, neighbour);
        view.generation = view.generation.next();
        Ok(DomainCommand::MoveAccount {
            account: id,
            direction: direction.opposite(),
        })
    }

    /// Puts `summary` at `place` among the accounts of its sub-group, recounting the
    /// totals, and moves to the next generation. Fails, changing nothing in memory, when
    /// Net Worth overflows or the data file refuses the account.
    fn insert_account(&mut self, summary: AccountSummary, place: usize) -> Result<(), DomainError> {
        let net_worth = self.view.net_worth.checked_add(summary.balance)?;
        write_account(&self.store, &summary, place)?;
        let index = self.view.sub_group(summary.account.kind()).start + place;
        let view = Arc::make_mut(&mut self.view);
        view.accounts.insert(index, summary);
        view.net_worth = net_worth;
        view.accounts_changed();
        Ok(())
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
        let row = transaction_row(self.view.accounts[position.account].account.id(), &edited);
        // The account is rebuilt, since a new amount moves every balance after it, and a
        // new date moves the transaction into date order.
        let moved = edited.date() != old.date();
        let currency = self.view.net_worth.currency();
        let (summary, ()) =
            self.view
                .edit_register(position.account, currency, |transactions| {
                    transactions[position.entry] = edited;
                    if moved {
                        transactions.sort_by_key(Transaction::register_order);
                    }
                })?;
        self.replace_account(position.account, summary)?;
        // The row is checked only now, so the ledger's own errors come first.
        self.store.update_transaction(&row?)?;
        Ok(DomainCommand::EditTransaction {
            transaction: id,
            edit: undo,
        })
    }

    fn set_flagged(
        &mut self,
        id: TransactionId,
        flagged: bool,
    ) -> Result<DomainCommand, DomainError> {
        let position = self.changeable(id)?;
        // Both indices were just found in this same view.
        let old = &self.view.accounts[position.account].register[position.entry].transaction;
        let undo = DomainCommand::SetFlagged {
            transaction: id,
            flagged: old.is_flagged(),
        };
        let flagged = old.clone().with_flag(flagged);
        let row = transaction_row(self.view.accounts[position.account].account.id(), &flagged)?;
        // A flag moves no balance, so the register keeps its entries and balances.
        let view = Arc::make_mut(&mut self.view);
        view.accounts[position.account].register[position.entry].transaction = flagged;
        view.accounts_changed();
        self.store.update_transaction(&row)?;
        Ok(undo)
    }

    fn post_transaction(
        &mut self,
        account: AccountId,
        transaction: NewTransaction,
    ) -> Result<DomainCommand, DomainError> {
        let index = self
            .view
            .accounts
            .iter()
            .position(|summary| summary.account.id() == account)
            .ok_or(DomainError::UnknownAccount { account })?;
        // `position` returns the index of an account in the view.
        if self.view.accounts[index].account.is_closed() {
            return Err(DomainError::AccountClosed { account });
        }
        if let Some(category) = transaction
            .categories()
            .into_iter()
            .find(|&category| self.view.category(category).is_none())
        {
            return Err(DomainError::UnknownCategory { category });
        }
        let id = self.next_transaction;
        let currency = self.view.net_worth.currency();
        let transaction = transaction.into_transaction(id, currency)?;
        let row = transaction_row(account, &transaction);
        let (summary, ()) = self.view.edit_register(index, currency, |transactions| {
            // The new id is the largest yet, so it goes after the rest of its date.
            let position = transactions.partition_point(|other| other.date() <= transaction.date());
            transactions.insert(position, transaction);
        })?;
        self.replace_account(index, summary)?;
        self.store.insert_transaction(&row?)?;
        self.next_transaction = id.next();
        Ok(DomainCommand::DeleteTransaction { transaction: id })
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
        self.store.delete_transaction(id.raw())?;
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
        let row = transaction_row(account, &transaction);
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
        self.store.insert_transaction(&row?)?;
        Ok(inverse)
    }

    fn create_category(
        &mut self,
        name: &str,
        parent: Option<CategoryId>,
    ) -> Result<DomainCommand, DomainError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::BlankCategoryName);
        }
        if let Some(parent) = parent
            && self.view.category(parent).is_none()
        {
            return Err(DomainError::UnknownCategory { category: parent });
        }
        let folded = name.to_lowercase();
        if self
            .view
            .categories
            .iter()
            .any(|sibling| sibling.parent() == parent && sibling.name().to_lowercase() == folded)
        {
            return Err(DomainError::DuplicateCategory {
                name: name.to_owned(),
            });
        }
        let id = self.next_category;
        let mut category = NewCategory::new(name);
        if let Some(parent) = parent {
            category = category.under(parent);
        }
        let category = category.into_category(id);
        self.store.insert_category(&category_row(&category))?;
        self.next_category = id.next();
        let view = Arc::make_mut(&mut self.view);
        // The new id is the highest yet, so the category lands after its parent.
        view.categories.push(category);
        view.generation = view.generation.next();
        Ok(DomainCommand::DeleteCategory { category: id })
    }

    fn delete_category(&mut self, id: CategoryId) -> Result<DomainCommand, DomainError> {
        let index = self
            .view
            .categories
            .iter()
            .position(|category| category.id() == id)
            .ok_or(DomainError::UnknownCategory { category: id })?;
        let has_child = self
            .view
            .categories
            .iter()
            .any(|category| category.parent() == Some(id));
        let in_a_transaction = self
            .view
            .accounts
            .iter()
            .flat_map(AccountSummary::register)
            .any(|entry| entry.transaction.uses_category(id));
        if has_child || in_a_transaction {
            return Err(DomainError::CategoryInUse { category: id });
        }
        self.store.delete_category(id.raw())?;
        let view = Arc::make_mut(&mut self.view);
        let category = view.categories.remove(index);
        view.generation = view.generation.next();
        Ok(DomainCommand::RestoreCategory { category })
    }

    fn restore_category(&mut self, category: Category) -> Result<DomainCommand, DomainError> {
        let id = category.id();
        // Categories stay in id order, which keeps each after the one it sits under.
        let Err(position) = self.view.categories.binary_search_by_key(&id, Category::id) else {
            panic!("restored {id:?}, which the ledger still holds");
        };
        self.store.insert_category(&category_row(&category))?;
        let view = Arc::make_mut(&mut self.view);
        view.categories.insert(position, category);
        view.generation = view.generation.next();
        Ok(DomainCommand::DeleteCategory { category: id })
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
        view.accounts_changed();
        Ok(())
    }
}

/// Writes `summary`'s account, at `place` in its sub-group, and its transactions.
fn write_account(store: &Store, summary: &AccountSummary, place: usize) -> Result<(), DomainError> {
    let account = &summary.account;
    store.insert_account(&account_row(account, place))?;
    for entry in &summary.register {
        store.insert_transaction(&transaction_row(account.id(), &entry.transaction)?)?;
    }
    Ok(())
}

/// An in-memory data file in `currency` holding `categories`, each after the one it sits
/// under, and `accounts`, in Account bar order.
fn in_memory_store(
    currency: CurrencyCode,
    categories: &[Category],
    accounts: &[AccountSummary],
) -> Store {
    let store = Store::create_in_memory(currency.as_str()).expect("SQLite makes an in-memory file");
    let write = || -> Result<(), DomainError> {
        store.begin()?;
        for category in categories {
            store.insert_category(&category_row(category))?;
        }
        // Accounts come in Account bar order, so each is last in its sub-group so far.
        let mut placed: HashMap<AccountKind, usize> = HashMap::new();
        for summary in accounts {
            let place = placed.entry(summary.account.kind()).or_default();
            write_account(&store, summary, *place)?;
            *place += 1;
        }
        Ok(store.commit()?)
    };
    write().expect("a new ledger's rows fit the data file's schema");
    store
}

#[cfg(test)]
mod tests {
    use icu_calendar::{Date, Iso};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::transaction::{NewTransaction, SplitLine};
    use crate::{
        AccountKind, AccountStatus, Allocation, MoveDirection, TransactionEdit, Visibility,
    };

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
    fn a_new_ledgers_store_places_each_sub_groups_accounts_in_order_from_zero() {
        let ledger = Ledger::with_accounts(
            usd_code(),
            [],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(1))),
                account("Rainy Day", AccountKind::Savings, usd(dec!(1))),
                account("Joint", AccountKind::Checking, usd(dec!(1))),
                account("Bills", AccountKind::Checking, usd(dec!(1))),
            ],
        )
        .unwrap();

        let contents = ledger.store.read().unwrap();

        let places: Vec<(&str, u32)> = contents
            .accounts
            .iter()
            .map(|account| (account.row.name.as_str(), account.row.position))
            .collect();
        assert_eq!(
            places,
            [
                ("Everyday", 0),
                ("Joint", 1),
                ("Bills", 2),
                ("Rainy Day", 0)
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
    fn a_group_total_sums_its_open_accounts_hidden_ones_included() {
        let ledger = Ledger::with_accounts(
            usd_code(),
            [],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(1000.50))),
                account("Holiday Fund", AccountKind::Savings, usd(dec!(200.25))).hidden(),
                account("Old Checking", AccountKind::Checking, usd(dec!(10.00))).closed(),
                account("Visa", AccountKind::CreditCard, usd(dec!(-400.00))),
                account("Brokerage", AccountKind::Investment, usd(dec!(5000.00))),
            ],
        )
        .unwrap();

        let view = ledger.snapshot();

        assert_eq!(
            view.group_total(AccountGroup::Banking),
            Ok(usd(dec!(800.75)))
        );
    }

    #[test]
    fn a_group_with_no_open_accounts_totals_zero() {
        let ledger = Ledger::with_accounts(
            usd_code(),
            [],
            [account("Old House", AccountKind::Asset, usd(dec!(10.00))).closed()],
        )
        .unwrap();

        let view = ledger.snapshot();

        assert_eq!(
            view.group_total(AccountGroup::PropertyDebt),
            Ok(usd(dec!(0)))
        );
    }

    #[test]
    fn the_closed_total_sums_the_closed_accounts_of_every_group() {
        let ledger = Ledger::with_accounts(
            usd_code(),
            [],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(1000.50))),
                account("Old Checking", AccountKind::Checking, usd(dec!(10.00))).closed(),
                account("Old House", AccountKind::Asset, usd(dec!(2.50))).closed(),
            ],
        )
        .unwrap();

        let view = ledger.snapshot();

        assert_eq!(view.closed_total(), Ok(usd(dec!(12.50))));
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

    /// The id, name and parent of every category in `view`, in its order.
    fn category_tree(view: &LedgerView) -> Vec<(CategoryId, &str, Option<CategoryId>)> {
        view.categories()
            .iter()
            .map(|category| (category.id(), category.name(), category.parent()))
            .collect()
    }

    fn create_category(name: &str, parent: Option<CategoryId>) -> DomainCommand {
        DomainCommand::CreateCategory {
            name: name.to_owned(),
            parent,
        }
    }

    #[test]
    fn a_created_category_takes_the_next_id_under_its_parent() {
        let mut ledger = everyday();
        let household = CategoryId::new(1);

        ledger
            .apply(create_category("Garden", Some(household)))
            .unwrap();

        let view = ledger.snapshot();
        assert_eq!(
            category_tree(&view),
            [
                (CategoryId::new(0), "Groceries", None),
                (household, "Household", None),
                (CategoryId::new(2), "Garden", Some(household)),
            ]
        );
        assert_eq!(view.generation(), Generation::FIRST.next());
    }

    #[test]
    fn a_category_named_as_a_sibling_ignoring_case_is_rejected() {
        let mut ledger = everyday();
        let before = ledger.snapshot();

        let result = ledger.apply(create_category("groceries", None));

        assert_eq!(
            result,
            Err(DomainError::DuplicateCategory {
                name: "groceries".to_owned()
            })
        );
        assert_eq!(ledger.snapshot(), before);
    }

    #[test]
    fn a_category_name_is_trimmed_before_it_is_compared_and_saved() {
        let mut ledger = everyday();

        let duplicate = ledger.apply(create_category(" Groceries ", None));
        ledger.apply(create_category(" Garden ", None)).unwrap();

        assert_eq!(
            duplicate,
            Err(DomainError::DuplicateCategory {
                name: "Groceries".to_owned()
            })
        );
        assert_eq!(
            category_tree(&ledger.snapshot()).last(),
            Some(&(CategoryId::new(2), "Garden", None))
        );
    }

    #[test]
    fn a_category_may_share_its_name_with_one_under_another_parent() {
        let mut ledger = everyday();
        let household = CategoryId::new(1);

        let result = ledger.apply(create_category("Groceries", Some(household)));

        assert!(result.is_ok());
    }

    #[test]
    fn a_category_under_an_unknown_parent_is_rejected() {
        let mut ledger = everyday();
        let before = ledger.snapshot();
        let unknown = CategoryId::new(99);

        let result = ledger.apply(create_category("Garden", Some(unknown)));

        assert_eq!(
            result,
            Err(DomainError::UnknownCategory { category: unknown })
        );
        assert_eq!(ledger.snapshot(), before);
    }

    #[test]
    fn a_blank_category_name_is_rejected() {
        let mut ledger = everyday();
        let before = ledger.snapshot();

        let result = ledger.apply(create_category("  ", None));

        assert_eq!(result, Err(DomainError::BlankCategoryName));
        assert_eq!(ledger.snapshot(), before);
    }

    #[test]
    fn undo_removes_a_created_category_and_redo_brings_it_back_with_its_id() {
        let mut ledger = everyday();
        let household = CategoryId::new(1);
        let before = category_tree(&ledger.snapshot())
            .into_iter()
            .map(|(id, name, parent)| (id, name.to_owned(), parent))
            .collect::<Vec<_>>();
        ledger
            .apply(create_category("Garden", Some(household)))
            .unwrap();

        ledger.undo().unwrap();
        let undone = ledger.snapshot();
        ledger.redo().unwrap();
        let redone = ledger.snapshot();

        assert_eq!(
            category_tree(&undone)
                .into_iter()
                .map(|(id, name, parent)| (id, name.to_owned(), parent))
                .collect::<Vec<_>>(),
            before
        );
        assert_eq!(
            category_tree(&redone),
            [
                (CategoryId::new(0), "Groceries", None),
                (household, "Household", None),
                (CategoryId::new(2), "Garden", Some(household)),
            ]
        );
    }

    #[test]
    fn a_category_created_after_an_undone_one_takes_a_fresh_id() {
        let mut ledger = everyday();
        ledger.apply(create_category("Garden", None)).unwrap();
        ledger.undo().unwrap();

        ledger.apply(create_category("Pets", None)).unwrap();

        let view = ledger.snapshot();
        assert_eq!(
            category_tree(&view).last(),
            Some(&(CategoryId::new(3), "Pets", None))
        );
    }

    #[test]
    fn a_category_a_transaction_uses_is_not_deleted() {
        let mut ledger = everyday();
        let before = ledger.snapshot();
        let groceries = CategoryId::new(0);

        let result = ledger.apply(DomainCommand::DeleteCategory {
            category: groceries,
        });

        assert_eq!(
            result,
            Err(DomainError::CategoryInUse {
                category: groceries
            })
        );
        assert_eq!(ledger.snapshot(), before);
    }

    #[test]
    fn only_a_category_a_split_line_uses_is_kept_from_deletion() {
        let groceries = CategoryId::new(0);
        let household = CategoryId::new(1);
        let mut ledger = Ledger::with_accounts(
            usd_code(),
            [NewCategory::new("Groceries"), NewCategory::new("Household")],
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

        let used = ledger.apply(DomainCommand::DeleteCategory {
            category: groceries,
        });
        let unused = ledger.apply(DomainCommand::DeleteCategory {
            category: household,
        });

        assert_eq!(
            used,
            Err(DomainError::CategoryInUse {
                category: groceries
            })
        );
        assert!(unused.is_ok());
    }

    #[test]
    fn a_category_with_one_under_it_is_not_deleted() {
        let mut ledger = everyday();
        let household = CategoryId::new(1);
        ledger
            .apply(create_category("Garden", Some(household)))
            .unwrap();
        let before = ledger.snapshot();

        let result = ledger.apply(DomainCommand::DeleteCategory {
            category: household,
        });

        assert_eq!(
            result,
            Err(DomainError::CategoryInUse {
                category: household
            })
        );
        assert_eq!(ledger.snapshot(), before);
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

    /// The command that creates an account opened on 15 September 2026.
    fn create(name: &str, kind: AccountKind, opening_balance: Money) -> DomainCommand {
        DomainCommand::CreateAccount {
            name: name.to_owned(),
            kind,
            opened: date(15),
            opening_balance,
        }
    }

    #[test]
    fn a_created_account_lands_last_in_its_sub_group_with_its_balance() {
        let mut ledger = three_payments();

        ledger
            .apply(create("Joint", AccountKind::Checking, usd(dec!(250.00))))
            .unwrap();

        let view = ledger.snapshot();
        let accounts: Vec<_> = view
            .accounts()
            .iter()
            .map(|summary| (summary.account().name(), summary.balance()))
            .collect();
        assert_eq!(
            accounts,
            [
                ("Everyday", usd(dec!(565.00))),
                ("Joint", usd(dec!(250.00))),
                ("Visa", usd(dec!(-50.00))),
            ]
        );
        assert_eq!(view.net_worth(), usd(dec!(765.00)));
    }

    #[test]
    fn an_account_of_a_kind_with_no_accounts_yet_lands_where_its_sub_group_goes() {
        let mut ledger = three_payments();

        ledger
            .apply(create("Rainy Day", AccountKind::Savings, usd(dec!(10.00))))
            .unwrap();

        assert_eq!(names(&ledger.snapshot()), ["Everyday", "Rainy Day", "Visa"]);
    }

    #[test]
    fn undoing_a_create_takes_the_account_out_again() {
        let mut ledger = three_payments();
        let before = ledger.snapshot();
        ledger
            .apply(create("Joint", AccountKind::Checking, usd(dec!(250.00))))
            .unwrap();

        ledger.undo().unwrap();

        let after = ledger.snapshot();
        assert_eq!(after.accounts(), before.accounts());
        assert_eq!(after.net_worth(), before.net_worth());
    }

    #[test]
    fn redo_brings_back_a_created_account_with_the_same_ids() {
        let mut ledger = three_payments();
        ledger
            .apply(create("Joint", AccountKind::Checking, usd(dec!(250.00))))
            .unwrap();
        let created = ledger.snapshot();
        ledger.undo().unwrap();

        ledger.redo().unwrap();

        assert_eq!(ledger.snapshot().accounts(), created.accounts());
    }

    #[test]
    fn redo_puts_an_account_back_in_its_place_in_a_later_sub_group() {
        let mut ledger = three_payments();
        ledger
            .apply(create(
                "Store Card",
                AccountKind::CreditCard,
                usd(dec!(-5.00)),
            ))
            .unwrap();
        ledger.undo().unwrap();

        ledger.redo().unwrap();

        assert_eq!(
            names(&ledger.snapshot()),
            ["Everyday", "Visa", "Store Card"]
        );
    }

    #[test]
    fn an_account_with_a_blank_name_is_rejected_and_changes_nothing() {
        let mut ledger = three_payments();
        let before = ledger.snapshot();

        let result = ledger.apply(create(" \t", AccountKind::Checking, usd(dec!(1.00))));

        assert_eq!(result, Err(DomainError::BlankAccountName));
        assert_eq!(ledger.snapshot(), before);
    }

    /// The ids of the account named `name` and of its transactions.
    fn ids(view: &LedgerView, name: &str) -> (AccountId, Vec<TransactionId>) {
        let summary = view
            .accounts()
            .iter()
            .find(|summary| summary.account().name() == name)
            .unwrap();
        let transactions = summary
            .register()
            .iter()
            .map(|entry| entry.transaction().id())
            .collect();
        (summary.account().id(), transactions)
    }

    #[test]
    fn a_created_account_never_reuses_an_id() {
        let mut ledger = three_payments();
        ledger
            .apply(create("Joint", AccountKind::Checking, usd(dec!(1.00))))
            .unwrap();
        let undone = ids(&ledger.snapshot(), "Joint");
        ledger.undo().unwrap();

        ledger
            .apply(create("Shared", AccountKind::Checking, usd(dec!(1.00))))
            .unwrap();

        let view = ledger.snapshot();
        let mut accounts: Vec<_> = view
            .accounts()
            .iter()
            .map(|summary| summary.account().id())
            .collect();
        accounts.push(undone.0);
        let mut transactions: Vec<_> = view
            .accounts()
            .iter()
            .flat_map(AccountSummary::register)
            .map(|entry| entry.transaction().id())
            .collect();
        transactions.extend(undone.1);
        let (account_count, transaction_count) = (accounts.len(), transactions.len());
        accounts.sort();
        accounts.dedup();
        transactions.sort();
        transactions.dedup();
        assert_eq!(
            (accounts.len(), transactions.len()),
            (account_count, transaction_count)
        );
    }

    #[test]
    fn a_created_account_holds_its_opening_balance_on_its_opening_date() {
        let mut ledger = three_payments();

        ledger
            .apply(create("Joint", AccountKind::Checking, usd(dec!(250.00))))
            .unwrap();

        let view = ledger.snapshot();
        let joint = view.account(ids(&view, "Joint").0).unwrap();
        let register: Vec<_> = joint
            .register()
            .iter()
            .map(|entry| {
                let transaction = entry.transaction();
                (
                    transaction.date(),
                    transaction.allocation().clone(),
                    transaction.amount(),
                )
            })
            .collect();
        assert_eq!(
            register,
            [(date(15), Allocation::OpeningBalance, usd(dec!(250.00)))]
        );
        assert_eq!(joint.account().currency(), usd_code());
    }

    #[test]
    fn deleting_an_unknown_account_is_rejected() {
        let mut ledger = three_payments();
        let unknown = AccountId::new(9);

        let result = ledger.apply(DomainCommand::DeleteAccount { account: unknown });

        assert_eq!(
            result,
            Err(DomainError::UnknownAccount { account: unknown })
        );
    }

    #[test]
    fn an_account_that_takes_net_worth_out_of_range_is_rejected_and_changes_nothing() {
        let mut ledger = three_payments();
        let before = ledger.snapshot();
        let largest = usd(Decimal::MAX.trunc() / dec!(100));

        let result = ledger.apply(create("Joint", AccountKind::Checking, largest));

        assert_eq!(result, Err(DomainError::Money(MoneyError::Overflow)));
        assert_eq!(ledger.snapshot(), before);
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
            TransactionEdit::Amount(usd(dec!(-40.00))),
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

    fn set_flagged(transaction: TransactionId, flagged: bool) -> DomainCommand {
        DomainCommand::SetFlagged {
            transaction,
            flagged,
        }
    }

    #[test]
    fn a_posted_transaction_starts_unflagged() {
        let mut ledger = everyday();
        let account = ledger.snapshot().accounts()[0].account().id();

        ledger
            .apply(DomainCommand::PostTransaction {
                account,
                transaction: NewTransaction::new(
                    date(6),
                    "Shell",
                    CategoryId::new(0),
                    usd(dec!(-20.00)),
                ),
            })
            .unwrap();

        assert!(!nth_transaction(&ledger, 2).is_flagged());
    }

    #[test]
    fn flagging_a_transaction_shows_in_the_next_snapshot() {
        let mut ledger = everyday();
        let fresh_market = transaction_id(&ledger, 1);

        ledger.apply(set_flagged(fresh_market, true)).unwrap();

        assert!(nth_transaction(&ledger, 1).is_flagged());
        assert_ne!(ledger.snapshot().generation(), Generation::FIRST);
    }

    #[test]
    fn a_flagged_transaction_can_be_unflagged() {
        let mut ledger = everyday();
        let fresh_market = transaction_id(&ledger, 1);
        ledger.apply(set_flagged(fresh_market, true)).unwrap();

        ledger.apply(set_flagged(fresh_market, false)).unwrap();

        assert!(!nth_transaction(&ledger, 1).is_flagged());
    }

    #[test]
    fn undo_takes_a_flag_off_and_redo_puts_it_back() {
        let mut ledger = everyday();
        let fresh_market = transaction_id(&ledger, 1);
        ledger.apply(set_flagged(fresh_market, true)).unwrap();

        ledger.undo().unwrap();
        let undone = nth_transaction(&ledger, 1).is_flagged();
        ledger.redo().unwrap();
        let redone = nth_transaction(&ledger, 1).is_flagged();

        assert_eq!((undone, redone), (false, true));
    }

    #[test]
    fn flagging_a_closed_accounts_transaction_is_rejected() {
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

        let result = ledger.apply(set_flagged(fresh_market, true));

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

    #[test]
    fn editing_an_amount_moves_the_balances_after_it() {
        let mut ledger = everyday();
        let fresh_market = transaction_id(&ledger, 1);

        ledger
            .apply(edit(
                fresh_market,
                TransactionEdit::Amount(usd(dec!(-40.00))),
            ))
            .unwrap();

        let view = ledger.snapshot();
        let everyday = &view.accounts()[0];
        let entry = &everyday.register()[1];
        assert_eq!(entry.transaction().amount(), usd(dec!(-40.00)));
        assert_eq!(entry.balance(), usd(dec!(60.00)));
        assert_eq!(everyday.balance(), usd(dec!(60.00)));
        assert_eq!(view.net_worth(), usd(dec!(60.00)));
    }

    #[test]
    fn editing_the_opening_balance_moves_every_balance_after_it() {
        let mut ledger = everyday();
        let opening = transaction_id(&ledger, 0);

        ledger
            .apply(edit(opening, TransactionEdit::Amount(usd(dec!(200.00)))))
            .unwrap();

        let view = ledger.snapshot();
        let balances: Vec<Money> = view.accounts()[0]
            .register()
            .iter()
            .map(RegisterEntry::balance)
            .collect();
        assert_eq!(balances, [usd(dec!(200.00)), usd(dec!(169.75))]);
    }

    #[test]
    fn a_split_transaction_takes_no_amount() {
        let groceries = CategoryId::new(0);
        let mut ledger = Ledger::with_accounts(
            usd_code(),
            [NewCategory::new("Groceries")],
            [
                account("Everyday", AccountKind::Checking, usd(dec!(500.00))).with_transactions([
                    NewTransaction::split(
                        date(12),
                        "Costco",
                        vec![SplitLine::new(groceries, "Food", usd(dec!(-142.37)))],
                    ),
                ]),
            ],
        )
        .unwrap();
        let before = ledger.snapshot();
        let costco = transaction_id(&ledger, 1);

        let result = ledger.apply(edit(costco, TransactionEdit::Amount(usd(dec!(-5.00)))));

        assert_eq!(
            result,
            Err(DomainError::SplitAmount {
                transaction: costco
            })
        );
        assert_eq!(ledger.snapshot(), before);
    }

    #[test]
    fn an_amount_in_another_currency_is_rejected() {
        let mut ledger = everyday();
        let before = ledger.snapshot();
        let fresh_market = transaction_id(&ledger, 1);
        let euros = Money::new(dec!(-5.00), CurrencyCode::new("EUR").unwrap()).unwrap();

        let result = ledger.apply(edit(fresh_market, TransactionEdit::Amount(euros)));

        assert_eq!(
            result,
            Err(DomainError::CurrencyMismatch {
                transaction: fresh_market
            })
        );
        assert_eq!(ledger.snapshot(), before);
    }

    #[test]
    fn an_amount_that_takes_a_balance_out_of_range_is_rejected() {
        let mut ledger = everyday();
        let before = ledger.snapshot();
        let fresh_market = transaction_id(&ledger, 1);
        let largest = usd(Decimal::MAX.trunc() / dec!(100));

        let result = ledger.apply(edit(fresh_market, TransactionEdit::Amount(largest)));

        assert_eq!(result, Err(DomainError::Money(MoneyError::Overflow)));
        assert_eq!(ledger.snapshot(), before);
    }

    /// Posts 12.00 paid to Corner Cafe on `day`, in the ledger's first category, to
    /// its first account.
    fn post(ledger: &Ledger, day: u8) -> DomainCommand {
        DomainCommand::PostTransaction {
            account: ledger.snapshot().accounts()[0].account().id(),
            transaction: NewTransaction::new(
                date(day),
                "Corner Cafe",
                CategoryId::new(0),
                usd(dec!(-12.00)),
            ),
        }
    }

    #[test]
    fn posting_puts_the_transaction_in_date_order_with_its_running_balance() {
        let mut ledger = three_payments();

        ledger.apply(post(&ledger, 7)).unwrap();

        assert_eq!(
            register(&ledger),
            [
                (String::new(), usd(dec!(100.00))),
                ("Acme Corp".to_owned(), usd(dec!(600.00))),
                ("Corner Cafe".to_owned(), usd(dec!(588.00))),
                ("Fresh Market".to_owned(), usd(dec!(557.75))),
                ("Bean There".to_owned(), usd(dec!(553.00))),
            ]
        );
        assert_eq!(ledger.snapshot().net_worth(), usd(dec!(503.00)));
    }

    #[test]
    fn posting_to_a_closed_account_is_rejected_and_changes_nothing() {
        let mut ledger = Ledger::with_accounts(
            usd_code(),
            [NewCategory::new("Groceries")],
            [account("Old Checking", AccountKind::Checking, usd(dec!(1))).closed()],
        )
        .unwrap();
        let before = ledger.snapshot();

        let result = ledger.apply(post(&ledger, 7));

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
    fn undo_takes_a_posted_transaction_out_and_redo_puts_it_back() {
        let mut ledger = three_payments();
        let before = register(&ledger);
        ledger.apply(post(&ledger, 7)).unwrap();
        let posted = register(&ledger);

        ledger.undo().unwrap();
        let undone = register(&ledger);
        ledger.redo().unwrap();

        assert_eq!((undone, register(&ledger)), (before, posted));
    }

    #[test]
    fn a_transaction_posted_after_an_undone_one_takes_a_new_id() {
        let mut ledger = three_payments();
        ledger.apply(post(&ledger, 7)).unwrap();
        let first = transaction(&ledger, "Corner Cafe");
        ledger.undo().unwrap();

        ledger.apply(post(&ledger, 7)).unwrap();

        assert_ne!(transaction(&ledger, "Corner Cafe"), first);
    }

    #[test]
    fn posting_to_an_unknown_category_is_rejected() {
        let mut ledger = three_payments();
        let unknown = CategoryId::new(9);
        let command = DomainCommand::PostTransaction {
            account: ledger.snapshot().accounts()[0].account().id(),
            transaction: NewTransaction::new(date(7), "Corner Cafe", unknown, usd(dec!(-1))),
        };

        let result = ledger.apply(command);

        assert_eq!(
            result.unwrap_err(),
            DomainError::UnknownCategory { category: unknown }
        );
    }

    #[test]
    fn a_transaction_posted_on_a_busy_date_goes_after_the_rest_of_that_date() {
        let mut ledger = three_payments();

        // Fresh Market and Bean There are on the 10th already.
        ledger.apply(post(&ledger, 10)).unwrap();

        let payees: Vec<String> = register(&ledger)
            .into_iter()
            .map(|(payee, _)| payee)
            .collect();
        assert_eq!(
            payees,
            ["", "Acme Corp", "Fresh Market", "Bean There", "Corner Cafe"]
        );
    }

    #[test]
    fn every_posted_transaction_takes_an_id_no_other_transaction_has() {
        let mut ledger = three_payments();

        ledger.apply(post(&ledger, 7)).unwrap();
        ledger.apply(post(&ledger, 8)).unwrap();

        let view = ledger.snapshot();
        let mut ids: Vec<TransactionId> = view
            .accounts()
            .iter()
            .flat_map(AccountSummary::register)
            .map(|entry| entry.transaction().id())
            .collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }

    /// Three checking accounts, Alpha, Bravo and Charlie in that order, then a savings
    /// account, Rainy Day.
    fn four_accounts() -> Ledger {
        Ledger::with_accounts(
            usd_code(),
            [],
            [
                account("Alpha", AccountKind::Checking, usd(dec!(10.00))),
                account("Bravo", AccountKind::Checking, usd(dec!(20.00))),
                account("Charlie", AccountKind::Checking, usd(dec!(30.00))),
                account("Rainy Day", AccountKind::Savings, usd(dec!(40.00))),
            ],
        )
        .unwrap()
    }

    /// The id of the account named `name`.
    fn account_id(ledger: &Ledger, name: &str) -> AccountId {
        ledger
            .snapshot()
            .accounts()
            .iter()
            .map(AccountSummary::account)
            .find(|account| account.name() == name)
            .map(Account::id)
            .unwrap()
    }

    fn rename(ledger: &Ledger, from: &str, to: &str) -> DomainCommand {
        DomainCommand::RenameAccount {
            account: account_id(ledger, from),
            name: to.to_owned(),
        }
    }

    #[test]
    fn renaming_an_account_shows_its_new_name_and_undo_puts_the_old_one_back() {
        let mut ledger = four_accounts();

        ledger.apply(rename(&ledger, "Bravo", "Joint")).unwrap();
        let renamed = names(&ledger.snapshot()).join(",");
        ledger.undo().unwrap();

        assert_eq!(renamed, "Alpha,Joint,Charlie,Rainy Day");
        assert_eq!(
            names(&ledger.snapshot()),
            ["Alpha", "Bravo", "Charlie", "Rainy Day"]
        );
    }

    #[test]
    fn renaming_an_account_to_a_blank_name_is_rejected_and_changes_nothing() {
        let mut ledger = four_accounts();
        let before = ledger.snapshot();

        let result = ledger.apply(rename(&ledger, "Bravo", "  "));

        assert_eq!(result, Err(DomainError::BlankAccountName));
        assert_eq!(ledger.snapshot(), before);
    }

    /// Each account's name with whether it is hidden and whether it is closed.
    fn flags(ledger: &Ledger) -> Vec<(String, bool, bool)> {
        ledger
            .snapshot()
            .accounts()
            .iter()
            .map(|summary| {
                let account = summary.account();
                (
                    account.name().to_owned(),
                    account.is_hidden(),
                    account.is_closed(),
                )
            })
            .collect()
    }

    fn flag(name: &str, hidden: bool, closed: bool) -> (String, bool, bool) {
        (name.to_owned(), hidden, closed)
    }

    #[test]
    fn hiding_an_account_leaves_it_open_and_undo_shows_it_again() {
        let mut ledger = four_accounts();
        let before = flags(&ledger);

        ledger
            .apply(DomainCommand::SetAccountVisibility {
                account: account_id(&ledger, "Bravo"),
                visibility: Visibility::Hidden,
            })
            .unwrap();
        let hidden = flags(&ledger);
        ledger.undo().unwrap();

        assert_eq!(hidden[1], flag("Bravo", true, false));
        assert_eq!(flags(&ledger), before);
    }

    #[test]
    fn closing_an_account_with_a_balance_leaves_it_shown_and_undo_reopens_it() {
        let mut ledger = four_accounts();
        let before = flags(&ledger);

        ledger
            .apply(DomainCommand::SetAccountStatus {
                account: account_id(&ledger, "Bravo"),
                status: AccountStatus::Closed,
            })
            .unwrap();
        let closed = flags(&ledger);
        ledger.undo().unwrap();

        assert_eq!(closed[1], flag("Bravo", false, true));
        assert_eq!(flags(&ledger), before);
    }

    #[test]
    fn reopening_a_closed_account_lets_its_transactions_change_again() {
        let mut ledger = four_accounts();
        let bravo = account_id(&ledger, "Bravo");
        let close = |status| DomainCommand::SetAccountStatus {
            account: bravo,
            status,
        };
        ledger.apply(close(AccountStatus::Closed)).unwrap();

        ledger.apply(close(AccountStatus::Open)).unwrap();

        assert_eq!(flags(&ledger)[1], flag("Bravo", false, false));
        let opening = ledger.snapshot().account(bravo).unwrap().register()[0]
            .transaction()
            .id();
        ledger
            .apply(edit(opening, TransactionEdit::Memo("Reopened".to_owned())))
            .unwrap();
    }

    #[test]
    fn hiding_an_unknown_account_is_rejected() {
        let mut ledger = four_accounts();
        let bravo = account_id(&ledger, "Bravo");
        ledger
            .apply(DomainCommand::DeleteAccount { account: bravo })
            .unwrap();

        let result = ledger.apply(DomainCommand::SetAccountVisibility {
            account: bravo,
            visibility: Visibility::Hidden,
        });

        assert_eq!(result, Err(DomainError::UnknownAccount { account: bravo }));
    }

    fn move_account(ledger: &Ledger, name: &str, direction: MoveDirection) -> DomainCommand {
        DomainCommand::MoveAccount {
            account: account_id(ledger, name),
            direction,
        }
    }

    #[test]
    fn moving_an_account_up_swaps_it_with_the_one_above_and_undo_swaps_them_back() {
        let mut ledger = four_accounts();

        ledger
            .apply(move_account(&ledger, "Charlie", MoveDirection::Up))
            .unwrap();
        let moved = names(&ledger.snapshot()).join(",");
        ledger.undo().unwrap();

        assert_eq!(moved, "Alpha,Charlie,Bravo,Rainy Day");
        assert_eq!(
            names(&ledger.snapshot()),
            ["Alpha", "Bravo", "Charlie", "Rainy Day"]
        );
    }

    #[test]
    fn moving_an_account_down_swaps_it_with_the_one_below() {
        let mut ledger = four_accounts();

        ledger
            .apply(move_account(&ledger, "Alpha", MoveDirection::Down))
            .unwrap();

        assert_eq!(
            names(&ledger.snapshot()),
            ["Bravo", "Alpha", "Charlie", "Rainy Day"]
        );
    }

    #[test]
    fn the_first_account_of_a_sub_group_moves_no_higher() {
        let mut ledger = four_accounts();
        let before = ledger.snapshot();

        let result = ledger.apply(move_account(&ledger, "Alpha", MoveDirection::Up));

        assert_eq!(
            result,
            Err(DomainError::SubGroupEdge {
                account: account_id(&ledger, "Alpha")
            })
        );
        assert_eq!(ledger.snapshot(), before);
    }

    #[test]
    fn a_move_never_crosses_into_the_next_sub_group() {
        let mut ledger = four_accounts();
        let before = ledger.snapshot();

        let down = ledger.apply(move_account(&ledger, "Charlie", MoveDirection::Down));
        let up = ledger.apply(move_account(&ledger, "Rainy Day", MoveDirection::Up));

        assert_eq!(
            [down, up],
            [
                Err(DomainError::SubGroupEdge {
                    account: account_id(&ledger, "Charlie")
                }),
                Err(DomainError::SubGroupEdge {
                    account: account_id(&ledger, "Rainy Day")
                }),
            ]
        );
        assert_eq!(ledger.snapshot(), before);
    }

    /// Renames Alpha, hides Bravo, deletes Charlie, then closes Rainy Day and moves
    /// Alpha below Bravo.
    fn manage_accounts(ledger: &Ledger) -> DomainCommand {
        DomainCommand::Compound(vec![
            rename(ledger, "Alpha", "Everyday"),
            DomainCommand::SetAccountVisibility {
                account: account_id(ledger, "Bravo"),
                visibility: Visibility::Hidden,
            },
            DomainCommand::DeleteAccount {
                account: account_id(ledger, "Charlie"),
            },
            DomainCommand::SetAccountStatus {
                account: account_id(ledger, "Rainy Day"),
                status: AccountStatus::Closed,
            },
            move_account(ledger, "Alpha", MoveDirection::Down),
        ])
    }

    #[test]
    fn a_compound_command_makes_each_of_its_changes_in_order() {
        let mut ledger = four_accounts();

        ledger.apply(manage_accounts(&ledger)).unwrap();

        assert_eq!(
            flags(&ledger),
            [
                flag("Bravo", true, false),
                flag("Everyday", false, false),
                flag("Rainy Day", false, true),
            ]
        );
    }

    #[test]
    fn one_undo_reverts_a_whole_compound_command_and_one_redo_makes_it_again() {
        let mut ledger = four_accounts();
        let before = flags(&ledger);
        ledger.apply(manage_accounts(&ledger)).unwrap();
        let after = flags(&ledger);

        ledger.undo().unwrap();
        let undone = flags(&ledger);
        let can_undo = ledger.snapshot().can_undo();
        ledger.redo().unwrap();

        assert_eq!(undone, before);
        assert!(!can_undo);
        assert_eq!(flags(&ledger), after);
    }

    #[test]
    fn a_compound_command_with_a_rejected_change_changes_nothing() {
        let mut ledger = four_accounts();
        let before = ledger.snapshot();
        let DomainCommand::Compound(mut changes) = manage_accounts(&ledger) else {
            unreachable!("manage_accounts is a compound command");
        };
        changes.push(move_account(&ledger, "Rainy Day", MoveDirection::Up));

        let result = ledger.apply(DomainCommand::Compound(changes));

        assert_eq!(
            result,
            Err(DomainError::SubGroupEdge {
                account: account_id(&ledger, "Rainy Day")
            })
        );
        assert_eq!(ledger.snapshot(), before);
    }

    #[test]
    fn undoing_a_compound_command_reverts_its_changes_latest_first() {
        let mut ledger = four_accounts();
        let alpha = account_id(&ledger, "Alpha");
        let rename = |name: &str| DomainCommand::RenameAccount {
            account: alpha,
            name: name.to_owned(),
        };
        ledger
            .apply(DomainCommand::Compound(vec![
                rename("First"),
                rename("Second"),
            ]))
            .unwrap();

        ledger.undo().unwrap();

        assert_eq!(names(&ledger.snapshot())[0], "Alpha");
    }

    #[test]
    fn undoing_a_compound_command_puts_a_deleted_account_back_in_its_place() {
        let mut ledger = four_accounts();
        let before = flags(&ledger);
        let compound = DomainCommand::Compound(vec![
            DomainCommand::DeleteAccount {
                account: account_id(&ledger, "Alpha"),
            },
            move_account(&ledger, "Charlie", MoveDirection::Up),
        ]);
        ledger.apply(compound).unwrap();
        let applied = names(&ledger.snapshot()).join(",");

        ledger.undo().unwrap();

        assert_eq!(applied, "Charlie,Bravo,Rainy Day");
        assert_eq!(flags(&ledger), before);
    }
}
