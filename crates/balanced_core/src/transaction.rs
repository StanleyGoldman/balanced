//! Transactions: what was paid or received, when, to whom, and which categories the
//! amount went to.

use icu_calendar::{Date, Iso};

use crate::{CategoryId, DomainError, Money, MoneyError, TransactionEdit};

/// Identifies a transaction for as long as it exists. Ids are never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransactionId(u32);

impl TransactionId {
    pub(crate) fn new(id: u32) -> Self {
        Self(id)
    }
}

/// One line of a split transaction: part of its amount, in one category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitLine {
    category: CategoryId,
    memo: String,
    amount: Money,
}

impl SplitLine {
    pub(crate) fn new(category: CategoryId, memo: &str, amount: Money) -> Self {
        Self {
            category,
            memo: memo.to_owned(),
            amount,
        }
    }

    pub fn category(&self) -> CategoryId {
        self.category
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
}

impl Transaction {
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

    /// The transaction with `edit` made. Fails when the edit gives the opening balance
    /// a payee.
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
        }
        Ok(edited)
    }

    /// The edit that undoes `edit`: the same field, with this transaction's value.
    pub(crate) fn undoing(&self, edit: &TransactionEdit) -> TransactionEdit {
        match edit {
            TransactionEdit::Number(_) => TransactionEdit::Number(self.number.clone()),
            TransactionEdit::Payee(_) => TransactionEdit::Payee(self.payee.clone()),
            TransactionEdit::Date(_) => TransactionEdit::Date(self.date),
            TransactionEdit::Memo(_) => TransactionEdit::Memo(self.memo.clone()),
        }
    }

    /// Where the transaction sits in its account's register: by date, and on one date in
    /// the order the transactions were posted, which their ids follow.
    pub(crate) fn register_order(&self) -> (Date<Iso>, TransactionId) {
        (self.date, self.id)
    }
}

/// How a [`NewTransaction`]'s amount is given.
#[derive(Debug, Clone)]
enum NewAllocation {
    OpeningBalance(Money),
    Category(CategoryId, Money),
    Split(Vec<SplitLine>),
}

/// A transaction to post to a new account in a [`Ledger`](crate::Ledger).
#[derive(Debug, Clone)]
pub(crate) struct NewTransaction {
    date: Date<Iso>,
    number: Option<String>,
    payee: String,
    memo: String,
    allocation: NewAllocation,
}

impl NewTransaction {
    /// An account's opening balance, on `date`.
    pub(crate) fn opening_balance(date: Date<Iso>, amount: Money) -> Self {
        Self::with_allocation(date, "", NewAllocation::OpeningBalance(amount))
    }

    /// `amount` paid to or received from `payee`, all in `category`.
    pub(crate) fn new(date: Date<Iso>, payee: &str, category: CategoryId, amount: Money) -> Self {
        Self::with_allocation(date, payee, NewAllocation::Category(category, amount))
    }

    /// A transaction with `payee`, divided between `lines`.
    pub(crate) fn split(date: Date<Iso>, payee: &str, lines: Vec<SplitLine>) -> Self {
        Self::with_allocation(date, payee, NewAllocation::Split(lines))
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
    pub(crate) fn number(self, number: &str) -> Self {
        Self {
            number: Some(number.to_owned()),
            ..self
        }
    }

    /// The same transaction, with a memo.
    pub(crate) fn memo(self, memo: &str) -> Self {
        Self {
            memo: memo.to_owned(),
            ..self
        }
    }

    /// The categories the transaction names.
    pub(crate) fn categories(&self) -> Vec<CategoryId> {
        match &self.allocation {
            NewAllocation::OpeningBalance(_) => Vec::new(),
            NewAllocation::Category(category, _) => vec![*category],
            NewAllocation::Split(lines) => lines.iter().map(SplitLine::category).collect(),
        }
    }

    /// The transaction, with `id`, in an account in `currency`. Fails when an amount is
    /// in another currency, or when a split's lines overflow.
    pub(crate) fn into_transaction(
        self,
        id: TransactionId,
        currency: crate::CurrencyCode,
    ) -> Result<Transaction, MoneyError> {
        let (allocation, amount) = match self.allocation {
            NewAllocation::OpeningBalance(amount) => (Allocation::OpeningBalance, amount),
            NewAllocation::Category(category, amount) => (Allocation::Category(category), amount),
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
        })
    }
}
