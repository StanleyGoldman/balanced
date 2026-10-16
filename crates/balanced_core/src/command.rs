//! [`DomainCommand`]s, the only way the ledger changes, and the [`DomainError`]s that
//! reject them.

use crate::{AccountId, MoneyError, Transaction, TransactionId};

/// A change to the ledger. `Ledger::apply` applies it, or rejects it with a
/// [`DomainError`] and leaves the ledger as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::enum_variant_names,
    reason = "every Command so far acts on a transaction; account and category ones come next"
)]
pub enum DomainCommand {
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
    /// The cheque or reference number, or `None` to clear it.
    Number(Option<String>),
    Payee(String),
    Memo(String),
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
    /// The command gave the opening balance a payee. The opening balance has none.
    #[error("the opening balance has no payee: {transaction:?}")]
    OpeningBalancePayee { transaction: TransactionId },
    /// A balance would no longer fit in an amount.
    #[error(transparent)]
    Money(#[from] MoneyError),
}
