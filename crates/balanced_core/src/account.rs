//! Accounts: what kind each is, the fixed group its kind belongs to, and its flags.

use icu_calendar::{Date, Iso};

use crate::transaction::NewTransaction;
use crate::{CategoryId, CurrencyCode, Money, MoneyError, Transaction, TransactionId};

/// One of the fixed groups the Account bar lists accounts in, in Account bar order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountGroup {
    Banking,
    Investing,
    PropertyDebt,
}

impl AccountGroup {
    /// Every group, in Account bar order. A new group goes here as well as in the
    /// matches below, which the compiler checks; it can't check this list.
    pub const ALL: [Self; 3] = [Self::Banking, Self::Investing, Self::PropertyDebt];

    /// The group's place in [`Self::ALL`].
    pub(crate) fn index(self) -> usize {
        match self {
            Self::Banking => 0,
            Self::Investing => 1,
            Self::PropertyDebt => 2,
        }
    }
}

/// What an account holds. Each kind is one sub-group of one fixed [`AccountGroup`].
///
/// Kinds are declared, and ordered, the way the Account bar lists their sub-groups:
/// by group, then by their fixed order within it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountKind {
    Checking,
    Savings,
    CreditCard,
    Investment,
    /// Retirement savings, such as a 401(k) or an IRA.
    Retirement,
    /// Property, such as a house or a car.
    Asset,
    /// Debt, such as a mortgage or a loan.
    Liability,
}

impl AccountKind {
    /// Every kind, in Account bar order. A new kind goes here as well as in the
    /// matches below, which the compiler checks; it can't check this list.
    pub const ALL: [Self; 7] = [
        Self::Checking,
        Self::Savings,
        Self::CreditCard,
        Self::Investment,
        Self::Retirement,
        Self::Asset,
        Self::Liability,
    ];

    /// The group whose sub-group this kind is.
    pub fn group(self) -> AccountGroup {
        match self {
            Self::Checking | Self::Savings | Self::CreditCard => AccountGroup::Banking,
            Self::Investment | Self::Retirement => AccountGroup::Investing,
            Self::Asset | Self::Liability => AccountGroup::PropertyDebt,
        }
    }
}

/// Whether the Account bar shows an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
    Shown,
    /// Left out of the Account bar, but still counted in Net Worth.
    Hidden,
}

/// Whether an account's history can change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccountStatus {
    Open,
    /// Kept with its history, read-only.
    Closed,
}

/// Identifies an account for as long as it exists. Ids are never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AccountId(u32);

impl AccountId {
    pub(crate) fn new(id: u32) -> Self {
        Self(id)
    }

    /// The id after this one.
    pub(crate) fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// The id as the data file stores it.
    pub(crate) fn raw(self) -> u32 {
        self.0
    }
}

/// An account: its name, its kind, its currency, and whether it is hidden or closed.
///
/// Hidden and closed are independent. A hidden account is left out of the Account
/// bar but still counts in Net Worth. A closed account keeps its history, read-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    id: AccountId,
    name: String,
    kind: AccountKind,
    currency: CurrencyCode,
    hidden: bool,
    closed: bool,
}

impl Account {
    pub(crate) fn new(
        id: AccountId,
        name: String,
        kind: AccountKind,
        currency: CurrencyCode,
        visibility: Visibility,
        status: AccountStatus,
    ) -> Self {
        Self {
            id,
            name,
            kind,
            currency,
            hidden: visibility == Visibility::Hidden,
            closed: status == AccountStatus::Closed,
        }
    }

    pub fn id(&self) -> AccountId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> AccountKind {
        self.kind
    }

    /// The currency every amount in the account is in.
    pub fn currency(&self) -> CurrencyCode {
        self.currency
    }

    /// Whether the Account bar leaves the account out.
    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Whether the account is closed: its Register is read-only.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn visibility(&self) -> Visibility {
        if self.hidden {
            Visibility::Hidden
        } else {
            Visibility::Shown
        }
    }

    pub fn status(&self) -> AccountStatus {
        if self.closed {
            AccountStatus::Closed
        } else {
            AccountStatus::Open
        }
    }

    /// Shows or hides the account, returning whether it was shown.
    pub(crate) fn set_visibility(&mut self, visibility: Visibility) -> Visibility {
        let old = self.visibility();
        self.hidden = visibility == Visibility::Hidden;
        old
    }

    /// Opens or closes the account, returning whether it was open.
    pub(crate) fn set_status(&mut self, status: AccountStatus) -> AccountStatus {
        let old = self.status();
        self.closed = status == AccountStatus::Closed;
        old
    }

    /// Renames the account, returning its old name.
    pub(crate) fn rename(&mut self, name: String) -> String {
        std::mem::replace(&mut self.name, name)
    }
}

/// An account to open in a new [`Ledger`](crate::Ledger).
#[derive(Debug, Clone)]
pub(crate) struct NewAccount {
    name: String,
    kind: AccountKind,
    currency: CurrencyCode,
    /// The opening balance first, then the rest in the order given.
    transactions: Vec<NewAccountTransaction>,
    hidden: bool,
    closed: bool,
}

impl NewAccount {
    /// An open, shown account in the currency of `opening_balance`, opened on `opened`.
    pub(crate) fn new(
        name: &str,
        kind: AccountKind,
        opened: Date<Iso>,
        opening_balance: Money,
    ) -> Self {
        Self {
            name: name.to_owned(),
            kind,
            currency: opening_balance.currency(),
            transactions: vec![NewAccountTransaction::unflagged(
                NewTransaction::opening_balance(opened, opening_balance),
            )],
            hidden: false,
            closed: false,
        }
    }

    /// The same account, with `transactions` posted after its opening balance.
    pub(crate) fn with_transactions(
        mut self,
        transactions: impl IntoIterator<Item = NewTransaction>,
    ) -> Self {
        self.transactions.extend(
            transactions
                .into_iter()
                .map(NewAccountTransaction::unflagged),
        );
        self
    }

    /// The same account, with `transaction` posted after the rest, flagged.
    pub(crate) fn with_flagged_transaction(mut self, transaction: NewTransaction) -> Self {
        self.transactions.push(NewAccountTransaction {
            transaction,
            flagged: true,
        });
        self
    }

    /// The same account, hidden from the Account bar.
    pub(crate) fn hidden(self) -> Self {
        Self {
            hidden: true,
            ..self
        }
    }

    /// The same account, closed.
    pub(crate) fn closed(self) -> Self {
        Self {
            closed: true,
            ..self
        }
    }

    pub(crate) fn kind(&self) -> AccountKind {
        self.kind
    }

    /// The account, with `id`, in its opening balance's currency, and the transactions
    /// to post to it, opening balance first.
    pub(crate) fn into_account(self, id: AccountId) -> (Account, Vec<NewAccountTransaction>) {
        let account = Account {
            id,
            name: self.name,
            kind: self.kind,
            currency: self.currency,
            hidden: self.hidden,
            closed: self.closed,
        };
        (account, self.transactions)
    }
}

/// A transaction to post to a [`NewAccount`], which may start flagged.
#[derive(Debug, Clone)]
pub(crate) struct NewAccountTransaction {
    transaction: NewTransaction,
    flagged: bool,
}

impl NewAccountTransaction {
    fn unflagged(transaction: NewTransaction) -> Self {
        Self {
            transaction,
            flagged: false,
        }
    }

    /// The categories the transaction names.
    pub(crate) fn categories(&self) -> Vec<CategoryId> {
        self.transaction.categories()
    }

    /// The transaction, with `id`, in an account in `currency`. Fails when an amount is
    /// in another currency, or when a split's lines overflow.
    pub(crate) fn into_transaction(
        self,
        id: TransactionId,
        currency: CurrencyCode,
    ) -> Result<Transaction, MoneyError> {
        Ok(self
            .transaction
            .into_transaction(id, currency)?
            .with_flag(self.flagged))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_belongs_to_its_fixed_group() {
        let groups = AccountKind::ALL.map(|kind| (kind, kind.group()));

        assert_eq!(
            groups,
            [
                (AccountKind::Checking, AccountGroup::Banking),
                (AccountKind::Savings, AccountGroup::Banking),
                (AccountKind::CreditCard, AccountGroup::Banking),
                (AccountKind::Investment, AccountGroup::Investing),
                (AccountKind::Retirement, AccountGroup::Investing),
                (AccountKind::Asset, AccountGroup::PropertyDebt),
                (AccountKind::Liability, AccountGroup::PropertyDebt),
            ]
        );
    }
}
