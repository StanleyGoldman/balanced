//! [`DomainCommand`]s, the only way the ledger changes, and the [`DomainError`]s that
//! reject them.

use icu_calendar::{Date, Iso};

use crate::{
    Account, AccountId, AccountKind, CategoryId, Money, MoneyError, NewTransaction, Transaction,
    TransactionId,
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
    /// Delete a transaction, from whichever account holds it.
    DeleteTransaction { transaction: TransactionId },
    /// Put a deleted transaction back into `account`, with its id, where it was in the
    /// register: the inverse of [`DomainCommand::DeleteTransaction`].
    RestoreTransaction {
        account: AccountId,
        transaction: Transaction,
    },
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
}
