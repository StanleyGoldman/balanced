// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Transactions: what was paid or received, when, to whom, and which categories the
//! amount went to.

use icu_calendar::{Date, Iso};

use crate::{
    AccountId, CategoryId, CategoryUse, CurrencyCode, DomainError, Money, MoneyError,
    TransactionEdit,
};

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
    /// The line's part of the amount moved to or from another account, where the
    /// transaction with this id is its other side. An other side the ledger makes for
    /// the line takes a later id than the split's.
    Transfer(TransactionId),
}

impl SplitTarget {
    /// The line's other side, when it is a transfer.
    pub(crate) fn transfer(self) -> Option<TransactionId> {
        match self {
            Self::Transfer(other_side) => Some(other_side),
            Self::Category(_) => None,
        }
    }
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

    pub fn target(&self) -> SplitTarget {
        self.target
    }

    pub fn memo(&self) -> &str {
        &self.memo
    }

    pub fn amount(&self) -> Money {
        self.amount
    }

    /// The amount of a split with `lines`, in `currency`: the sum of the lines. Fails
    /// when it overflows.
    pub(crate) fn sum(lines: &[Self], currency: CurrencyCode) -> Result<Money, MoneyError> {
        lines.iter().try_fold(Money::zero(currency), |sum, line| {
            sum.checked_add(line.amount)
        })
    }
}

/// Where a [`NewSplitLine`]'s part of the amount goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NewSplitTarget {
    /// Into this category.
    Category(CategoryId),
    /// To or from this account, where posting the split records the line's other side.
    Transfer(AccountId),
}

/// One line of a split transaction to post.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSplitLine {
    target: NewSplitTarget,
    memo: String,
    amount: Money,
}

impl NewSplitLine {
    pub(crate) fn new(target: NewSplitTarget, memo: &str, amount: Money) -> Self {
        Self {
            target,
            memo: memo.to_owned(),
            amount,
        }
    }

    /// A line in `category`.
    pub fn category(category: CategoryId, memo: &str, amount: Money) -> Self {
        Self::new(NewSplitTarget::Category(category), memo, amount)
    }

    /// A line that moves `amount` from or to `account`.
    pub fn transfer(account: AccountId, memo: &str, amount: Money) -> Self {
        Self::new(NewSplitTarget::Transfer(account), memo, amount)
    }

    pub(crate) fn target(&self) -> NewSplitTarget {
        self.target
    }

    pub(crate) fn memo(&self) -> &str {
        &self.memo
    }

    pub(crate) fn amount(&self) -> Money {
        self.amount
    }

    /// The line once posted, going to `target`.
    fn posted(self, target: SplitTarget) -> SplitLine {
        SplitLine {
            target,
            memo: self.memo,
            amount: self.amount,
        }
    }
}

/// Where an [`EditedSplitLine`]'s part of the amount goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditedSplitTarget {
    /// Into this category.
    Category(CategoryId),
    /// To or from this account, where the edit records the line's new other side.
    Transfer(AccountId),
    /// To or from another account, where this transaction, already one of the
    /// transaction's other sides, stays the line's other side.
    Kept(TransactionId),
}

/// One line of a transaction's new list of lines, in [`TransactionEdit::Lines`]: a line
/// to a category, a new line to an account, or a line to an account that keeps its
/// other side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditedSplitLine {
    target: EditedSplitTarget,
    memo: String,
    amount: Money,
}

impl EditedSplitLine {
    pub(crate) fn new(target: EditedSplitTarget, memo: &str, amount: Money) -> Self {
        Self {
            target,
            memo: memo.to_owned(),
            amount,
        }
    }

    /// A line in `category`.
    pub fn category(category: CategoryId, memo: &str, amount: Money) -> Self {
        Self::new(EditedSplitTarget::Category(category), memo, amount)
    }

    /// A new line that moves `amount` from or to `account`.
    pub fn transfer(account: AccountId, memo: &str, amount: Money) -> Self {
        Self::new(EditedSplitTarget::Transfer(account), memo, amount)
    }

    /// A line to another account whose other side stays `other_side`, one of the
    /// transaction's other sides.
    pub fn kept(other_side: TransactionId, memo: &str, amount: Money) -> Self {
        Self::new(EditedSplitTarget::Kept(other_side), memo, amount)
    }

    pub fn target(&self) -> EditedSplitTarget {
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
    /// The amount divided between categories and transfers. The transaction's amount is
    /// the sum of the lines.
    Split(Vec<SplitLine>),
    /// One side of a transfer: the whole amount moved to or from another account, where
    /// the transaction with this id is its other side. The other side of a split line
    /// names the split. An other side the ledger makes, for a posting or an edit, takes a
    /// later id than the side it was made for.
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

    /// Whether a transaction at this status counts in its account's Cleared balance.
    pub(crate) fn counts_as_cleared(self) -> bool {
        match self {
            Self::Uncleared => false,
            Self::Cleared | Self::Reconciled => true,
        }
    }
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
        self.other_side_with(id, &self.memo, self.amount)
    }

    /// The other sides the transaction names, in order: a transfer's one, or one for
    /// each of a split's lines to an account. A split line's other side is as
    /// [`Self::other_side`] makes it, with the line's memo and amount.
    pub(crate) fn other_sides(&self) -> Vec<Self> {
        match &self.allocation {
            Allocation::Transfer(other_side) => vec![self.other_side(*other_side)],
            Allocation::Split(lines) => lines
                .iter()
                .filter_map(|line| {
                    let other_side = line.target.transfer()?;
                    Some(self.other_side_with(other_side, &line.memo, line.amount))
                })
                .collect(),
            Allocation::OpeningBalance | Allocation::Category(_) => Vec::new(),
        }
    }

    /// An other side with `id` pointing back at this transaction, holding `memo` and
    /// `amount` with its sign flipped.
    pub(crate) fn other_side_with(&self, id: TransactionId, memo: &str, amount: Money) -> Self {
        Self {
            id,
            date: self.date,
            number: None,
            payee: self.payee.clone(),
            memo: memo.to_owned(),
            allocation: Allocation::Transfer(self.id),
            amount: -amount,
            flagged: false,
            cleared_status: ClearedStatus::Uncleared,
        }
    }

    /// The transaction's other sides, in order: a transfer's one, or the other side of
    /// each of a split's lines to an account.
    pub(crate) fn other_side_ids(&self) -> Vec<TransactionId> {
        match &self.allocation {
            Allocation::Transfer(other_side) => vec![*other_side],
            Allocation::Split(lines) => lines
                .iter()
                .filter_map(|line| line.target.transfer())
                .collect(),
            Allocation::OpeningBalance | Allocation::Category(_) => Vec::new(),
        }
    }

    /// How far the transaction has been checked against the bank.
    pub fn cleared_status(&self) -> ClearedStatus {
        self.cleared_status
    }

    /// The transaction with `edit` made. Fails when the edit gives the opening balance
    /// a payee, or gives a split transaction or an amount in another currency.
    ///
    /// Panics on [`TransactionEdit::Transfer`] and [`TransactionEdit::Lines`], whose
    /// other sides only the ledger can make.
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
            TransactionEdit::Lines { .. } => panic!(
                "the ledger makes the lines' other sides, so it changes the lines with `resplit`"
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

    /// The transaction split into `lines`, which must add up to `amount`. Fails when it
    /// is the opening balance, when `amount` is in another currency, or when the lines
    /// add up to anything else.
    pub(crate) fn resplit(
        &self,
        amount: Money,
        lines: Vec<SplitLine>,
    ) -> Result<Self, DomainError> {
        if self.allocation == Allocation::OpeningBalance {
            return Err(DomainError::NotInOneCategory {
                transaction: self.id,
            });
        }
        if amount.currency() != self.amount.currency() {
            return Err(DomainError::CurrencyMismatch {
                transaction: self.id,
            });
        }
        if SplitLine::sum(&lines, amount.currency())? != amount {
            return Err(DomainError::SplitAmount {
                transaction: self.id,
            });
        }
        Ok(Self {
            allocation: Allocation::Split(lines),
            amount,
            ..self.clone()
        })
    }

    /// The edit that undoes `edit`: the same field, with this transaction's value.
    /// Only meaningful for an edit [`Transaction::edited`] accepts, so panics on
    /// [`TransactionEdit::Transfer`] and [`TransactionEdit::Lines`].
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
            TransactionEdit::Transfer(_) | TransactionEdit::Lines { .. } => {
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
            Allocation::Split(lines) => lines
                .iter()
                .any(|line| line.target() == SplitTarget::Category(category)),
        }
    }

    /// The transaction's uses of `category`: the whole transaction, or each of its split
    /// lines in it.
    pub(crate) fn category_uses(&self, category: CategoryId) -> Vec<CategoryUse> {
        match &self.allocation {
            Allocation::Category(own) if *own == category => {
                vec![CategoryUse::Transaction(self.id)]
            }
            Allocation::OpeningBalance | Allocation::Category(_) | Allocation::Transfer(_) => {
                Vec::new()
            }
            Allocation::Split(lines) => lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.target() == SplitTarget::Category(category))
                .map(|(line, _)| CategoryUse::SplitLine {
                    split: self.id,
                    line,
                })
                .collect(),
        }
    }

    /// The transaction with each of `uses`, which are its own and in category `from`,
    /// moved to category `to`. Fails when one names the transaction or a line of it
    /// that isn't in `from`.
    pub(crate) fn reassigned(
        &self,
        from: CategoryId,
        to: CategoryId,
        uses: &[CategoryUse],
    ) -> Result<Self, DomainError> {
        let not_in_from = DomainError::NotInCategory {
            transaction: self.id,
            category: from,
        };
        let mut reassigned = self.clone();
        for &category_use in uses {
            match (category_use, &mut reassigned.allocation) {
                (CategoryUse::Transaction(id), Allocation::Category(category))
                    if id == self.id && *category == from =>
                {
                    *category = to;
                }
                (CategoryUse::SplitLine { split, line }, Allocation::Split(lines))
                    if split == self.id =>
                {
                    let line = lines.get_mut(line).ok_or(not_in_from.clone())?;
                    if line.target != SplitTarget::Category(from) {
                        return Err(not_in_from);
                    }
                    line.target = SplitTarget::Category(to);
                }
                (CategoryUse::Transaction(_) | CategoryUse::SplitLine { .. }, _) => {
                    return Err(not_in_from);
                }
            }
        }
        Ok(reassigned)
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
    Split(Vec<NewSplitLine>),
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
    pub fn split(date: Date<Iso>, payee: &str, lines: Vec<NewSplitLine>) -> Self {
        Self::with_allocation(date, payee, NewAllocation::Split(lines))
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
                .filter_map(|line| match line.target {
                    NewSplitTarget::Category(category) => Some(category),
                    NewSplitTarget::Transfer(_) => None,
                })
                .collect(),
        }
    }

    /// The accounts the amount moves to or from, in order: a transfer's one, or the
    /// account of each of a split's lines to an account.
    pub(crate) fn transfer_accounts(&self) -> Vec<AccountId> {
        match &self.allocation {
            NewAllocation::Transfer(account, _) => vec![*account],
            NewAllocation::Split(lines) => lines
                .iter()
                .filter_map(|line| match line.target {
                    NewSplitTarget::Transfer(account) => Some(account),
                    NewSplitTarget::Category(_) => None,
                })
                .collect(),
            NewAllocation::OpeningBalance(_) | NewAllocation::Category(..) => Vec::new(),
        }
    }

    /// The transaction, with `id`, in an account in `currency`. Its other sides, in the
    /// order of [`Self::transfer_accounts`], are to take the ids after `id`. Fails when an
    /// amount is in another currency, or when a split's lines overflow.
    pub(crate) fn into_transaction(
        self,
        id: TransactionId,
        currency: CurrencyCode,
    ) -> Result<Transaction, MoneyError> {
        let cleared_status = self.starting_status();
        let (allocation, amount) = match self.allocation {
            NewAllocation::OpeningBalance(amount) => (Allocation::OpeningBalance, amount),
            NewAllocation::Category(category, amount) => (Allocation::Category(category), amount),
            NewAllocation::Transfer(_, amount) => (Allocation::Transfer(id.next()), amount),
            NewAllocation::Split(lines) => {
                let mut other_side = id;
                let lines: Vec<SplitLine> = lines
                    .into_iter()
                    .map(|line| {
                        let target = match line.target {
                            NewSplitTarget::Category(category) => SplitTarget::Category(category),
                            NewSplitTarget::Transfer(_) => {
                                other_side = other_side.next();
                                SplitTarget::Transfer(other_side)
                            }
                        };
                        line.posted(target)
                    })
                    .collect();
                let amount = SplitLine::sum(&lines, currency)?;
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

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;

    /// The ledger hands each transaction only its own uses, so only a direct call
    /// reaches this check.
    #[test]
    fn a_split_line_of_another_transaction_is_not_reassigned() {
        let usd = CurrencyCode::new("USD").expect("USD is an ISO 4217 currency");
        let food = Money::new(dec!(-40.00), usd).expect("-40.00 fits in Money");
        let (groceries, dining) = (CategoryId::new(0), CategoryId::new(1));
        let target = TransactionId::new(1);
        let split = Transaction::from_parts(
            target,
            Date::try_new_iso(2026, 9, 6).expect("6 September 2026 is a real date"),
            None,
            "Target".to_owned(),
            String::new(),
            Allocation::Split(vec![SplitLine::new(
                SplitTarget::Category(groceries),
                "Food",
                food,
            )]),
            food,
        );

        let result = split.reassigned(
            groceries,
            dining,
            &[CategoryUse::SplitLine {
                split: TransactionId::new(2),
                line: 0,
            }],
        );

        assert_eq!(
            result,
            Err(DomainError::NotInCategory {
                transaction: target,
                category: groceries
            })
        );
    }
}
