// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`DomainCommand`]s, the only way the ledger changes, and the [`DomainError`]s that
//! reject them.

use icu_calendar::{Date, Iso};

use crate::{
    Account, AccountId, AccountKind, AccountStatus, Category, CategoryId, CategoryType,
    ClearedStatus, EditedSplitLine, Money, MoneyError, NewTransaction, Transaction, TransactionId,
    Visibility, WriteError,
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
    /// Add a transaction to `account`, in date order after any already on its date. A
    /// transfer's other side, and the other side of each split line to an account, takes
    /// a later id than the transaction posted.
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
    /// the next category id. A top-level category is of `category_type` and starts
    /// shown; a subcategory takes its parent's type and visibility instead.
    CreateCategory {
        name: String,
        parent: Option<CategoryId>,
        category_type: CategoryType,
    },
    /// Delete a category that no transaction and no other category uses.
    DeleteCategory { category: CategoryId },
    /// Put a deleted category back, with its id: the inverse of
    /// [`DomainCommand::DeleteCategory`].
    RestoreCategory { category: Category },
    /// Give a category a new name, which no other category under its parent has,
    /// ignoring case. Every transaction in it shows the new name at once.
    RenameCategory { category: CategoryId, name: String },
    /// Move a category, with every category under it, under `parent`, or to the top of
    /// the tree. Under a parent, the category and those under it take its top-level
    /// category's type; at the top, the category keeps the type it had. Under a hidden
    /// parent, the category and those under it are hidden.
    MoveCategory {
        category: CategoryId,
        parent: Option<CategoryId>,
    },
    /// Rename a category and move it, as [`DomainCommand::MoveCategory`] does, in one
    /// step. The new name is checked against the new parent's categories only, so it
    /// may be one the old parent holds.
    EditCategory {
        category: CategoryId,
        name: String,
        parent: Option<CategoryId>,
    },
    /// Make a top-level category, with every category under it, Income or Expense. A
    /// subcategory takes its top-level category's type, so it can't be given one.
    SetCategoryType {
        category: CategoryId,
        category_type: CategoryType,
    },
    /// Show or hide a category, with every category under it. Showing a category also
    /// shows each hidden category above it, but not the others under those. A hidden
    /// category leaves the Category field's list, but keeps its transactions.
    SetCategoryVisibility {
        category: CategoryId,
        visibility: Visibility,
    },
    /// Move every transaction and split line in category `from` to category `to`, which
    /// may not be `from` or a category under it. Transactions in closed accounts move
    /// too. Categories under `from` keep theirs.
    ReassignCategory { from: CategoryId, to: CategoryId },
    /// Move exactly the transactions and split lines in `uses`, each in category
    /// `from`, to category `to`: the inverse of [`DomainCommand::ReassignCategory`],
    /// which leaves alone what was in `to` before.
    ReassignUses {
        from: CategoryId,
        to: CategoryId,
        uses: Vec<CategoryUse>,
    },
}

/// One use of a category: a transaction whose whole amount is in it, or one line of a
/// split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryUse {
    Transaction(TransactionId),
    /// The line at `line`, counting from 0, of split transaction `split`.
    SplitLine {
        split: TransactionId,
        line: usize,
    },
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
    /// transfer. Its other side in that account is new, with the payee and memo copied
    /// and a later id than the transaction edited, and any other side it had before is
    /// deleted.
    Transfer(AccountId),
    /// The whole amount, in the account's currency. A split transaction's amount is
    /// the sum of its lines, so it can't be edited here.
    Amount(Money),
    /// How far the transaction has been checked against the bank. Any status may
    /// follow any other.
    ClearedStatus(ClearedStatus),
    /// Every line of the transaction, which makes a transaction in one category or
    /// account a split. The lines must add up to `amount`, which becomes the
    /// transaction's amount. A new line to an account gets an other side there, with
    /// the line's memo and a later id than the transaction edited; a kept line's other
    /// side takes the line's new amount; and the other side of every line the edit drops
    /// is deleted. A kept line names one of the transaction's other sides, at most once,
    /// or the edit fails with [`DomainError::UnknownTransaction`]. While any of the
    /// transaction's other sides is in a closed account, its lines can't change.
    Lines {
        amount: Money,
        lines: Vec<EditedSplitLine>,
    },
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
            | Self::ClearedStatus(_)
            | Self::Lines { .. } => None,
        }
    }

    /// Whether the edit reaches a transfer's other side: a date or amount it shares, or
    /// a category, account or lines that remove or replace it.
    pub(crate) fn reaches_other_side(&self) -> bool {
        match self {
            Self::Date(_)
            | Self::Amount(_)
            | Self::Category(_)
            | Self::Transfer(_)
            | Self::Lines { .. } => true,
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
    /// Or it gave the opening balance split lines.
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
    /// The move would put the category under itself or one of its own subcategories.
    #[error("a category can't move under itself or its subcategories: {category:?}")]
    CategoryUnderItself { category: CategoryId },
    /// The category is a subcategory, which takes its top-level category's type.
    #[error("a subcategory takes its top-level category's type: {category:?}")]
    SubcategoryType { category: CategoryId },
    /// The category's name is empty or only spaces.
    #[error("a category needs a name")]
    BlankCategoryName,
    /// The command reassigned a category's transactions to the category itself or one
    /// under it, which would be deleted with it.
    #[error("a category's transactions can't move to it or a category under it: {category:?}")]
    ReassignIntoItself { category: CategoryId },
    /// The command named a transaction or split line that isn't in the category it gave.
    #[error("the transaction isn't in the category: {transaction:?} {category:?}")]
    NotInCategory {
        transaction: TransactionId,
        category: CategoryId,
    },
    /// A balance would no longer fit in an amount.
    #[error(transparent)]
    Money(#[from] MoneyError),
    /// The command gave a split transaction an amount, or split lines that don't add up
    /// to the amount it gave with them. A split's amount is the sum of its lines.
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
