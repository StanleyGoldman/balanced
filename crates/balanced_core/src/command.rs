// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`DomainCommand`]s, the only way the ledger changes, and the [`DomainError`]s that
//! reject them.

use icu_calendar::{Date, Iso};

use crate::{
    Account, AccountId, AccountKind, AccountStatus, Category, CategoryId, ClearedStatus, Money,
    MoneyError, NewTransaction, Transaction, TransactionId, Visibility, WriteError,
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
    /// Change one field of an existing transaction. A new date or amount on one side of
    /// a transfer changes its other side too, and a split's new date moves the other side
    /// of each of its lines to an account.
    EditTransaction {
        transaction: TransactionId,
        edit: TransactionEdit,
    },
    /// Flag a transaction to come back to, or take its flag off.
    SetFlagged {
        transaction: TransactionId,
        flagged: bool,
    },
    /// Delete a transaction, from whichever account holds it. Deleting one side of a
    /// transfer deletes its other side too, and deleting a split deletes the other side of
    /// each of its lines to an account.
    DeleteTransaction { transaction: TransactionId },
    /// Put a deleted transaction back into `account`, with its id, where it was in the
    /// register, and each of its `other_sides` back into its own: the inverse of
    /// [`DomainCommand::DeleteTransaction`].
    RestoreTransaction {
        account: AccountId,
        transaction: Transaction,
        other_sides: Vec<OtherSide>,
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

/// A transfer's other side, or a split line's, in the account that holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtherSide {
    account: AccountId,
    transaction: Transaction,
}

impl OtherSide {
    pub(crate) fn new(account: AccountId, transaction: Transaction) -> Self {
        Self {
            account,
            transaction,
        }
    }

    /// The account that holds the other side.
    pub fn account(&self) -> AccountId {
        self.account
    }

    pub fn transaction(&self) -> &Transaction {
        &self.transaction
    }

    /// The account and the transaction, taken apart.
    pub(crate) fn into_parts(self) -> (AccountId, Transaction) {
        (self.account, self.transaction)
    }
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
    /// The one category the whole amount is in. On a transfer, it deletes the other
    /// side.
    Category(CategoryId),
    /// The account the whole amount moves to or from, which makes the transaction a
    /// transfer. Its other side in that account is new, with the payee and memo copied,
    /// and any other side it had before is deleted.
    Transfer(AccountId),
    /// The whole amount, in the account's currency. A split transaction's amount is
    /// the sum of its lines, so it can't be edited here.
    Amount(Money),
    /// How far the transaction has been checked against the bank. Any status may
    /// follow any other.
    ClearedStatus(ClearedStatus),
}

impl TransactionEdit {
    /// The edit a transfer's other side takes along with this one: the same date, or
    /// the amount with its sign flipped. The other fields belong to each side alone.
    pub(crate) fn for_other_side(&self) -> Option<Self> {
        match self {
            Self::Date(date) => Some(Self::Date(*date)),
            Self::Amount(amount) => Some(Self::Amount(-*amount)),
            Self::Number(_)
            | Self::Payee(_)
            | Self::Memo(_)
            | Self::Category(_)
            | Self::Transfer(_)
            | Self::ClearedStatus(_) => None,
        }
    }

    /// Whether the edit reaches a transfer's other side: a date or amount it shares, or
    /// a category or account that removes or replaces it.
    pub(crate) fn reaches_other_side(&self) -> bool {
        match self {
            Self::Date(_) | Self::Amount(_) | Self::Category(_) | Self::Transfer(_) => true,
            Self::Number(_) | Self::Payee(_) | Self::Memo(_) | Self::ClearedStatus(_) => false,
        }
    }
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
    /// The command put the whole amount in one category or account, but the transaction
    /// is the opening balance, which has no category, or is split between categories.
    #[error("the transaction isn't in one category or account: {transaction:?}")]
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
    /// The command made a transfer to or from the account the transaction is in.
    #[error("a transfer moves money to another account: {account:?}")]
    TransferToOwnAccount { account: AccountId },
    /// The command would post, change or delete the side of a transfer that is in
    /// another account, which is closed, so its history is read-only.
    #[error("the transfer's other account is closed: {account:?}")]
    TransferAccountClosed { account: AccountId },
    /// The command would change the date, amount or target of a split line's other
    /// side, or delete it. Those change only with the split, in `account`.
    #[error("the transaction is a split line's other side; change the split in {account:?}")]
    SplitLineOtherSide { account: AccountId },
    /// The command would delete an account that holds a side of a transfer, which
    /// would leave the other side in another account with nothing to point at. The
    /// account can be closed instead.
    #[error("the account holds a transfer, so it can't be deleted: {account:?}")]
    AccountHasTransfers { account: AccountId },
    /// The data file couldn't take the change.
    #[error(transparent)]
    Store(#[from] WriteError),
}

impl From<balanced_store::StoreError> for DomainError {
    fn from(error: balanced_store::StoreError) -> Self {
        Self::Store(error.into())
    }
}
