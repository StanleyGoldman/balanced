//! [`DomainCommand`]s, the only way the ledger changes, and the [`DomainError`]s that
//! reject them.

use icu_calendar::{Date, Iso};

use crate::{
    Account, AccountId, AccountKind, AccountStatus, Category, CategoryId, Money, MoneyError,
    NewTransaction, Transaction, TransactionId, Visibility, WriteError,
};

/// A change to the ledger. `Ledger::apply` applies it, or rejects it with a
/// [`DomainError`] and leaves the ledger as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainCommand {
    /// Open an account named `name`, of `kind`, in the ledger's currency, holding
    /// `opening_balance` on `opened`. It lands last in its sub-group.
    CreateAccount {
        name: String,
        kind: AccountKind,
        opened: Date<Iso>,
        opening_balance: Money,
    },
    /// Delete an account with every transaction in it: the inverse of
    /// [`DomainCommand::CreateAccount`].
    DeleteAccount { account: AccountId },
    /// Put a deleted account back, with its id and its transactions' ids, at `place`
    /// among the accounts of its sub-group: the inverse of
    /// [`DomainCommand::DeleteAccount`].
    RestoreAccount {
        account: Account,
        transactions: Vec<Transaction>,
        place: usize,
    },
    /// Give an account a new name, which may not be blank.
    RenameAccount { account: AccountId, name: String },
    /// Show an account in the Account bar, or hide it there.
    SetAccountVisibility {
        account: AccountId,
        visibility: Visibility,
    },
    /// Close an account, which makes its history read-only, or open it again. An
    /// account with a balance can be closed.
    SetAccountStatus {
        account: AccountId,
        status: AccountStatus,
    },
    /// Swap an account with its neighbour above or below it in its sub-group. An
    /// account never moves out of its sub-group.
    MoveAccount {
        account: AccountId,
        direction: MoveDirection,
    },
    /// Make every change in the list, in order, as one change that undoes in one step.
    /// When the ledger refuses any of them, it makes none.
    Compound(Vec<Self>),
    /// Add a transaction to `account`, in date order after any already on its date.
    PostTransaction {
        account: AccountId,
        transaction: NewTransaction,
    },
    /// Change one field of an existing transaction.
    EditTransaction {
        transaction: TransactionId,
        edit: TransactionEdit,
    },
    /// Flag a transaction to come back to, or take its flag off.
    SetFlagged {
        transaction: TransactionId,
        flagged: bool,
    },
    /// Delete a transaction, from whichever account holds it.
    DeleteTransaction { transaction: TransactionId },
    /// Put a deleted transaction back into `account`, with its id, where it was in the
    /// register: the inverse of [`DomainCommand::DeleteTransaction`].
    RestoreTransaction {
        account: AccountId,
        transaction: Transaction,
    },
    /// Add a category named `name` under `parent`, or at the top of the tree. It takes
    /// the next category id.
    CreateCategory {
        name: String,
        parent: Option<CategoryId>,
    },
    /// Delete a category that no transaction and no other category uses.
    DeleteCategory { category: CategoryId },
    /// Put a deleted category back, with its id: the inverse of
    /// [`DomainCommand::DeleteCategory`].
    RestoreCategory { category: Category },
}

/// Which way [`DomainCommand::MoveAccount`] moves an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoveDirection {
    Up,
    Down,
}

impl MoveDirection {
    /// The direction that undoes a move this way.
    #[must_use]
    pub fn opposite(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
        }
    }

    /// The index of the item this way of `index` in a list, or `None` above the first.
    pub fn neighbour(self, index: usize) -> Option<usize> {
        match self {
            Self::Up => index.checked_sub(1),
            Self::Down => index.checked_add(1),
        }
    }
}

/// The new value of one field of a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionEdit {
    /// The date, which moves the transaction into date order in its register.
    Date(Date<Iso>),
    /// The cheque or reference number, or `None` to clear it.
    Number(Option<String>),
    Payee(String),
    Memo(String),
    /// The one category the whole amount is in.
    Category(CategoryId),
    /// The whole amount, in the account's currency. A split transaction's amount is
    /// the sum of its lines, so it can't be edited here.
    Amount(Money),
}

/// A [`DomainCommand`] the ledger applied, with the Command that undoes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub(crate) inverse: DomainCommand,
}

impl Applied {
    /// The Command that puts the ledger back as it was before this one.
    pub fn inverse(&self) -> &DomainCommand {
        &self.inverse
    }

    pub fn into_inverse(self) -> DomainCommand {
        self.inverse
    }
}

/// Why the ledger refused a [`DomainCommand`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DomainError {
    /// The transaction is in a closed account, whose history is read-only.
    #[error("the account is closed, so its transactions can't change: {account:?}")]
    AccountClosed { account: AccountId },
    /// No transaction has the id, perhaps because it was deleted.
    #[error("no such transaction: {transaction:?}")]
    UnknownTransaction { transaction: TransactionId },
    /// No account has the id, perhaps because it was deleted.
    #[error("no such account: {account:?}")]
    UnknownAccount { account: AccountId },
    /// The account is already first or last in its sub-group, so it can't move that
    /// way.
    #[error("the account can't move out of its sub-group: {account:?}")]
    SubGroupEdge { account: AccountId },
    /// The command named a new account with nothing but white space.
    #[error("an account needs a name")]
    BlankAccountName,
    /// The command gave the opening balance a payee. The opening balance has none.
    #[error("the opening balance has no payee: {transaction:?}")]
    OpeningBalancePayee { transaction: TransactionId },
    /// The command put the whole amount in one category, but the transaction is the
    /// opening balance, which has no category, or is split between categories.
    #[error("the transaction isn't in one category: {transaction:?}")]
    NotInOneCategory { transaction: TransactionId },
    /// No category has the id, perhaps because it was deleted.
    #[error("no such category: {category:?}")]
    UnknownCategory { category: CategoryId },
    /// Another category under the same parent has the name, ignoring case.
    #[error("a category under the same parent is already named {name:?}")]
    DuplicateCategory { name: String },
    /// A transaction or another category uses the category, so it can't be deleted.
    #[error("the category is in use: {category:?}")]
    CategoryInUse { category: CategoryId },
    /// The category's name is empty or only spaces.
    #[error("a category needs a name")]
    BlankCategoryName,
    /// A balance would no longer fit in an amount.
    #[error(transparent)]
    Money(#[from] MoneyError),
    /// The command gave a split transaction an amount. Its amount is the sum of its
    /// lines.
    #[error("a split transaction's amount is the sum of its lines: {transaction:?}")]
    SplitAmount { transaction: TransactionId },
    /// The command gave a transaction an amount in another currency than its account's.
    #[error("the amount is in another currency than the account: {transaction:?}")]
    CurrencyMismatch { transaction: TransactionId },
    /// The data file couldn't take the change.
    #[error(transparent)]
    Store(#[from] WriteError),
}

impl From<balanced_store::StoreError> for DomainError {
    fn from(error: balanced_store::StoreError) -> Self {
        Self::Store(error.into())
    }
}
