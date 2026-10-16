//! [`DomainCommand`]s, the only way the ledger changes, and the [`DomainError`]s that
//! reject them.

use crate::{AccountId, TransactionId};

/// A change to the ledger. `Ledger::apply` applies it, or rejects it with a
/// [`DomainError`] and leaves the ledger as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainCommand {
    /// Change one field of an existing transaction.
    EditTransaction {
        transaction: TransactionId,
        edit: TransactionEdit,
    },
}

/// What [`Ledger::apply`](crate::Ledger::apply) did: the change it made, told by the
/// Command that reverses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    inverse: DomainCommand,
}

impl Applied {
    pub(crate) fn new(inverse: DomainCommand) -> Self {
        Self { inverse }
    }

    /// The Command that puts the ledger back as it was before the change.
    pub fn inverse(&self) -> &DomainCommand {
        &self.inverse
    }

    pub(crate) fn into_inverse(self) -> DomainCommand {
        self.inverse
    }
}

/// The new value of one field of a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionEdit {
    /// The cheque or reference number, or `None` to clear it.
    Number(Option<String>),
    Payee(String),
    Memo(String),
}

/// Why the ledger refused a [`DomainCommand`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DomainError {
    /// The transaction is in a closed account, whose history is read-only.
    #[error("the account is closed, so its transactions can't change: {account:?}")]
    AccountClosed { account: AccountId },
    /// No transaction has the id, perhaps because it was deleted.
    #[error("no such transaction: {transaction:?}")]
    UnknownTransaction { transaction: TransactionId },
    /// The command gave the opening balance a payee. The opening balance has none.
    #[error("the opening balance has no payee: {transaction:?}")]
    OpeningBalancePayee { transaction: TransactionId },
}
