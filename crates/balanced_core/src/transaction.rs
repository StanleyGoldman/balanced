// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Transactions: what was paid or received, when, to whom, and which categories the
//! amount went to.

use icu_calendar::{Date, Iso};

use crate::{AccountId, CategoryId, DomainError, Money, MoneyError, TransactionEdit};

/// Identifies a transaction for as long as it exists. Ids are never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransactionId(u32);

impl TransactionId {
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

/// Where a split line's part of the amount went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitTarget {
    /// The line's part of the amount is in this category.
    Category(CategoryId),
}

/// One line of a split transaction: part of its amount, sent to one target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitLine {
    target: SplitTarget,
    memo: String,
    amount: Money,
}

impl SplitLine {
    pub(crate) fn new(target: SplitTarget, memo: &str, amount: Money) -> Self {
        Self {
            target,
            memo: memo.to_owned(),
            amount,
        }
    }

    /// A line in `category`, for tests outside the crate's own code.
    #[cfg(feature = "test-support")]
    pub fn for_tests(category: CategoryId, memo: &str, amount: Money) -> Self {
        Self::new(SplitTarget::Category(category), memo, amount)
    }

    pub fn target(&self) -> SplitTarget {
        self.target
    }

    pub fn memo(&self) -> &str {
        &self.memo
    }

    pub fn amount(&self) -> Money {
        self.amount
    }
}

/// Where a transaction's amount went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Allocation {
    /// The account's starting balance: the first transaction of every account. It has
    /// no category.
    OpeningBalance,
    /// The whole amount, in one category.
    Category(CategoryId),
    /// The amount divided between categories. The transaction's amount is the sum of
    /// the lines.
    Split(Vec<SplitLine>),
    /// One side of a transfer: the whole amount moved to or from another account, where
    /// the transaction with this id is its other side.
    Transfer(TransactionId),
}

/// How far a transaction has been checked against the bank: not yet, seen on the bank's
/// records, or matched against a statement. It belongs to the whole transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClearedStatus {
    /// The bank hasn't shown the transaction yet.
    Uncleared,
    /// The bank has shown the transaction.
    Cleared,
    /// The transaction was matched against a statement.
    Reconciled,
}

impl ClearedStatus {
    /// Every status, in the order a transaction moves through them.
    pub(crate) const ALL: [Self; 3] = [Self::Uncleared, Self::Cleared, Self::Reconciled];
}

/// A transaction in one account. A positive amount is money in, a negative one money out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    id: TransactionId,
    date: Date<Iso>,
    number: Option<String>,
    payee: String,
    memo: String,
    allocation: Allocation,
    amount: Money,
    /// Whether the user marked it to come back to.
    flagged: bool,
    cleared_status: ClearedStatus,
}

impl Transaction {
    /// A transaction as the data file holds it, whose amount is `amount`: the sum of
    /// the lines, for a split transaction.
    pub(crate) fn from_parts(
        id: TransactionId,
        date: Date<Iso>,
        number: Option<String>,
        payee: String,
        memo: String,
        allocation: Allocation,
        amount: Money,
    ) -> Self {
        Self {
            id,
            date,
            number,
            payee,
            memo,
            allocation,
            amount,
            flagged: false,
            cleared_status: ClearedStatus::Uncleared,
        }
    }

    /// The same transaction, flagged or not.
    #[must_use]
    pub(crate) fn with_flag(self, flagged: bool) -> Self {
        Self { flagged, ..self }
    }

    /// The same transaction, with `cleared_status`.
    #[must_use]
    pub(crate) fn with_cleared_status(self, cleared_status: ClearedStatus) -> Self {
        Self {
            cleared_status,
            ..self
        }
    }

    pub fn id(&self) -> TransactionId {
        self.id
    }

    pub fn date(&self) -> Date<Iso> {
        self.date
    }

    /// The cheque or reference number, when it has one.
    pub fn number(&self) -> Option<&str> {
        self.number.as_deref()
    }

    pub fn payee(&self) -> &str {
        &self.payee
    }

    pub fn memo(&self) -> &str {
        &self.memo
    }

    pub fn allocation(&self) -> &Allocation {
        &self.allocation
    }

    /// The whole amount: the sum of the lines, for a split transaction.
    pub fn amount(&self) -> Money {
        self.amount
    }

    /// Whether the user marked the transaction to come back to.
    pub fn is_flagged(&self) -> bool {
        self.flagged
    }

    /// The other side of this transfer, with `id`, in the other account: on the same
    /// date, for the amount with its sign flipped, with the payee and memo copied, with
    /// no number and no flag, and uncleared.
    pub(crate) fn other_side(&self, id: TransactionId) -> Self {
        Self {
            id,
            date: self.date,
            number: None,
            payee: self.payee.clone(),
            memo: self.memo.clone(),
            allocation: Allocation::Transfer(self.id),
            amount: -self.amount,
            flagged: false,
            cleared_status: ClearedStatus::Uncleared,
        }
    }

    /// The transaction's other side, when it is one side of a transfer.
    pub(crate) fn other_side_id(&self) -> Option<TransactionId> {
        match self.allocation {
            Allocation::Transfer(other_side) => Some(other_side),
            Allocation::OpeningBalance | Allocation::Category(_) | Allocation::Split(_) => None,
        }
    }

    /// How far the transaction has been checked against the bank.
    pub fn cleared_status(&self) -> ClearedStatus {
        self.cleared_status
    }

    /// The transaction with `edit` made. Fails when the edit gives the opening balance
    /// a payee, or gives a split transaction or an amount in another currency.
    ///
    /// Panics on [`TransactionEdit::Transfer`], whose other side only the ledger can
    /// make.
    pub(crate) fn edited(&self, edit: TransactionEdit) -> Result<Self, DomainError> {
        let mut edited = self.clone();
        match edit {
            TransactionEdit::Date(date) => edited.date = date,
            TransactionEdit::Number(number) => edited.number = number,
            TransactionEdit::Payee(payee) => {
                if self.allocation == Allocation::OpeningBalance {
                    return Err(DomainError::OpeningBalancePayee {
                        transaction: self.id,
                    });
                }
                edited.payee = payee;
            }
            TransactionEdit::Memo(memo) => edited.memo = memo,
            TransactionEdit::Category(category) => {
                if !matches!(self.allocation, Allocation::Category(_)) {
                    return Err(DomainError::NotInOneCategory {
                        transaction: self.id,
                    });
                }
                edited.allocation = Allocation::Category(category);
            }
            TransactionEdit::Transfer(_) => panic!(
                "the ledger makes a transfer's other side, so it changes the target with `retargeted`"
            ),
            TransactionEdit::Amount(amount) => {
                if matches!(self.allocation, Allocation::Split(_)) {
                    return Err(DomainError::SplitAmount {
                        transaction: self.id,
                    });
                }
                if amount.currency() != self.amount.currency() {
                    return Err(DomainError::CurrencyMismatch {
                        transaction: self.id,
                    });
                }
                edited.amount = amount;
            }
            TransactionEdit::ClearedStatus(status) => edited.cleared_status = status,
        }
        Ok(edited)
    }

    /// The transaction with its whole amount moved to `allocation`, a category or a
    /// transfer. Fails when it is the opening balance or split.
    pub(crate) fn retargeted(&self, allocation: Allocation) -> Result<Self, DomainError> {
        match self.allocation {
            Allocation::Category(_) | Allocation::Transfer(_) => Ok(Self {
                allocation,
                ..self.clone()
            }),
            Allocation::OpeningBalance | Allocation::Split(_) => {
                Err(DomainError::NotInOneCategory {
                    transaction: self.id,
                })
            }
        }
    }

    /// The edit that undoes `edit`: the same field, with this transaction's value.
    /// Only meaningful for an edit [`Transaction::edited`] accepts, so panics on
    /// [`TransactionEdit::Transfer`].
    pub(crate) fn undoing(&self, edit: &TransactionEdit) -> TransactionEdit {
        match edit {
            TransactionEdit::Number(_) => TransactionEdit::Number(self.number.clone()),
            TransactionEdit::Payee(_) => TransactionEdit::Payee(self.payee.clone()),
            TransactionEdit::Date(_) => TransactionEdit::Date(self.date),
            TransactionEdit::Memo(_) => TransactionEdit::Memo(self.memo.clone()),
            TransactionEdit::Category(category) => match self.allocation {
                Allocation::Category(old) => TransactionEdit::Category(old),
                // `edited` refuses a category for any other allocation.
                Allocation::OpeningBalance | Allocation::Split(_) | Allocation::Transfer(_) => {
                    TransactionEdit::Category(*category)
                }
            },
            TransactionEdit::Transfer(_) => {
                panic!("the ledger undoes a change of target by putting the transaction back whole")
            }
            TransactionEdit::Amount(_) => TransactionEdit::Amount(self.amount),
            TransactionEdit::ClearedStatus(_) => {
                TransactionEdit::ClearedStatus(self.cleared_status)
            }
        }
    }

    /// Whether any of the transaction's amount is in `category`.
    pub(crate) fn uses_category(&self, category: CategoryId) -> bool {
        match &self.allocation {
            Allocation::OpeningBalance | Allocation::Transfer(_) => false,
            Allocation::Category(own) => *own == category,
            Allocation::Split(lines) => lines.iter().any(|line| match line.target() {
                SplitTarget::Category(own) => own == category,
            }),
        }
    }

    /// Where the transaction sits in its account's register: by date, and on one date in
    /// the order the transactions were posted, which their ids follow.
    pub(crate) fn register_order(&self) -> (Date<Iso>, TransactionId) {
        (self.date, self.id)
    }
}

/// How a [`NewTransaction`]'s amount is given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NewAllocation {
    OpeningBalance(Money),
    Category(CategoryId, Money),
    Split(Vec<SplitLine>),
    /// To or from another account.
    Transfer(AccountId, Money),
}

/// A transaction to post to an account, which gives it its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTransaction {
    date: Date<Iso>,
    number: Option<String>,
    payee: String,
    memo: String,
    allocation: NewAllocation,
}

/// A [`NewTransaction`]'s fields, borrowed.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NewTransactionParts<'a> {
    pub(crate) date: Date<Iso>,
    pub(crate) number: Option<&'a str>,
    pub(crate) payee: &'a str,
    pub(crate) memo: &'a str,
    pub(crate) allocation: &'a NewAllocation,
}

impl NewTransaction {
    /// An account's opening balance, on `date`.
    pub(crate) fn opening_balance(date: Date<Iso>, amount: Money) -> Self {
        Self::with_allocation(date, "", NewAllocation::OpeningBalance(amount))
    }

    /// `amount` paid to or received from `payee`, all in `category`.
    pub fn new(date: Date<Iso>, payee: &str, category: CategoryId, amount: Money) -> Self {
        Self::with_allocation(date, payee, NewAllocation::Category(category, amount))
    }

    /// `amount` paid to or received from `payee`, moved from or to account `account`: a
    /// transfer, whose other side the ledger records in that account.
    pub fn transfer(date: Date<Iso>, payee: &str, account: AccountId, amount: Money) -> Self {
        Self::with_allocation(date, payee, NewAllocation::Transfer(account, amount))
    }

    /// A transaction with `payee`, divided between `lines`.
    pub(crate) fn split(date: Date<Iso>, payee: &str, lines: Vec<SplitLine>) -> Self {
        Self::with_allocation(date, payee, NewAllocation::Split(lines))
    }

    /// A split transaction, for tests outside the crate's own code.
    #[cfg(feature = "test-support")]
    pub fn split_for_tests(date: Date<Iso>, payee: &str, lines: Vec<SplitLine>) -> Self {
        Self::split(date, payee, lines)
    }

    /// A transaction with every field given, as the undo log holds it.
    pub(crate) fn from_parts(
        date: Date<Iso>,
        number: Option<String>,
        payee: String,
        memo: String,
        allocation: NewAllocation,
    ) -> Self {
        Self {
            date,
            number,
            payee,
            memo,
            allocation,
        }
    }

    /// Every field, borrowed.
    pub(crate) fn parts(&self) -> NewTransactionParts<'_> {
        let Self {
            date,
            number,
            payee,
            memo,
            allocation,
        } = self;
        NewTransactionParts {
            date: *date,
            number: number.as_deref(),
            payee,
            memo,
            allocation,
        }
    }

    fn with_allocation(date: Date<Iso>, payee: &str, allocation: NewAllocation) -> Self {
        Self {
            date,
            number: None,
            payee: payee.to_owned(),
            memo: String::new(),
            allocation,
        }
    }

    /// The same transaction, with a cheque or reference number.
    #[must_use]
    pub fn number(self, number: &str) -> Self {
        Self {
            number: Some(number.to_owned()),
            ..self
        }
    }

    /// The same transaction, with a memo.
    #[must_use]
    pub fn memo(self, memo: &str) -> Self {
        Self {
            memo: memo.to_owned(),
            ..self
        }
    }

    /// The status the transaction starts with: an opening balance comes from a
    /// statement, so it starts Reconciled, and the rest start Uncleared.
    pub(crate) fn starting_status(&self) -> ClearedStatus {
        match self.allocation {
            NewAllocation::OpeningBalance(_) => ClearedStatus::Reconciled,
            NewAllocation::Category(..) | NewAllocation::Transfer(..) | NewAllocation::Split(_) => {
                ClearedStatus::Uncleared
            }
        }
    }

    /// The categories the transaction names.
    pub(crate) fn categories(&self) -> Vec<CategoryId> {
        match &self.allocation {
            NewAllocation::OpeningBalance(_) | NewAllocation::Transfer(..) => Vec::new(),
            NewAllocation::Category(category, _) => vec![*category],
            NewAllocation::Split(lines) => lines
                .iter()
                .map(|line| match line.target() {
                    SplitTarget::Category(category) => category,
                })
                .collect(),
        }
    }

    /// The account a transfer moves the amount to or from.
    pub(crate) fn transfer_account(&self) -> Option<AccountId> {
        match self.allocation {
            NewAllocation::Transfer(account, _) => Some(account),
            NewAllocation::OpeningBalance(_)
            | NewAllocation::Category(..)
            | NewAllocation::Split(_) => None,
        }
    }

    /// The transaction, with `id`, in an account in `currency`. A transfer's other side
    /// is to take the id after `id`. Fails when an amount is in another currency, or when
    /// a split's lines overflow.
    pub(crate) fn into_transaction(
        self,
        id: TransactionId,
        currency: crate::CurrencyCode,
    ) -> Result<Transaction, MoneyError> {
        let cleared_status = self.starting_status();
        let (allocation, amount) = match self.allocation {
            NewAllocation::OpeningBalance(amount) => (Allocation::OpeningBalance, amount),
            NewAllocation::Category(category, amount) => (Allocation::Category(category), amount),
            NewAllocation::Transfer(_, amount) => (Allocation::Transfer(id.next()), amount),
            NewAllocation::Split(lines) => {
                let amount = lines.iter().try_fold(Money::zero(currency), |sum, line| {
                    sum.checked_add(line.amount)
                })?;
                (Allocation::Split(lines), amount)
            }
        };
        if amount.currency() != currency {
            return Err(MoneyError::CurrencyMismatch {
                left: currency,
                right: amount.currency(),
            });
        }
        Ok(Transaction {
            id,
            date: self.date,
            number: self.number,
            payee: self.payee,
            memo: self.memo,
            allocation,
            amount,
            flagged: false,
            cleared_status,
        })
    }
}
