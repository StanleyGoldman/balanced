// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`Draft`]: the transaction typed into the draft row at the bottom of a Register,
//! before it is added.

use balanced_core::{
    AccountId, CurrencyCode, DomainCommand, Money, NewTransaction, TransactionEdit,
};
use balanced_i18n::Localizer;
use icu_calendar::{Date, Iso};

use crate::combobox::{CategoryTarget, Recalled};
use crate::i18n::fl;
use crate::register_model::SplitLines;

/// What the draft row holds so far. Every cell starts empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Draft {
    date: Option<Date<Iso>>,
    number: String,
    payee: String,
    /// Where the amount goes, once a category or the split lines are given.
    allocation: Option<DraftAllocation>,
    memo: String,
    amount: Option<Money>,
}

impl Draft {
    pub(crate) fn date(&self) -> Option<Date<Iso>> {
        self.date
    }

    pub(crate) fn number(&self) -> &str {
        &self.number
    }

    pub(crate) fn payee(&self) -> &str {
        &self.payee
    }

    /// The one category or account the whole amount goes to, once one is typed.
    pub(crate) fn category(&self) -> Option<CategoryTarget> {
        match self.allocation {
            Some(DraftAllocation::One(target)) => Some(target),
            Some(DraftAllocation::Split(_)) | None => None,
        }
    }

    /// The split lines, when the draft is split.
    pub(crate) fn split(&self) -> Option<&SplitLines> {
        match &self.allocation {
            Some(DraftAllocation::Split(split)) => Some(split),
            Some(DraftAllocation::One(_)) | None => None,
        }
    }

    /// Splits the draft into `split`'s lines, in place of any category, with the
    /// amount they add up to.
    pub(crate) fn set_split(&mut self, split: SplitLines) {
        self.amount = Some(split.amount());
        self.allocation = Some(DraftAllocation::Split(split));
    }

    pub(crate) fn memo(&self) -> &str {
        &self.memo
    }

    pub(crate) fn amount(&self) -> Option<Money> {
        self.amount
    }

    /// Saves `edit` in its cell.
    pub(crate) fn edit(&mut self, edit: TransactionEdit) {
        match edit {
            TransactionEdit::Date(date) => self.date = Some(date),
            TransactionEdit::Number(number) => self.number = number.unwrap_or_default(),
            TransactionEdit::Payee(payee) => self.payee = payee,
            TransactionEdit::Memo(memo) => self.memo = memo,
            TransactionEdit::Category(category) => {
                self.allocation = Some(DraftAllocation::One(CategoryTarget::Category(category)));
            }
            TransactionEdit::Transfer(account) => {
                self.allocation = Some(DraftAllocation::One(CategoryTarget::Transfer(account)));
            }
            TransactionEdit::Amount(amount) => self.amount = Some(amount),
            // The draft row's Clr cell is empty: a posted transaction starts uncleared.
            // Its split lines come from the Split dialog, through `set_split`.
            TransactionEdit::ClearedStatus(_) | TransactionEdit::Lines { .. } => {}
        }
    }

    /// Empties the amount cells, as if no amount had been typed.
    pub(crate) fn clear_amount(&mut self) {
        self.amount = None;
    }

    /// Takes `recalled`'s spelling of the payee, and fills the Category, Memo and amount
    /// cells from it where they are empty.
    pub(crate) fn recall(&mut self, recalled: &Recalled) {
        recalled.payee().clone_into(&mut self.payee);
        if self.allocation.is_none() {
            self.allocation = recalled.category().map(DraftAllocation::One);
        }
        if self.memo.is_empty() {
            recalled.memo().clone_into(&mut self.memo);
        }
        if self.amount.is_none() {
            self.amount = recalled.amount();
        }
    }

    /// The first cell, left to right, that the transaction can't do without and the
    /// draft leaves empty, if any.
    pub(crate) fn missing(&self) -> Option<Missing> {
        if self.date.is_none() {
            Some(Missing::Date)
        } else if self.allocation.is_none() {
            Some(Missing::Category)
        } else {
            None
        }
    }

    /// The Command that adds the draft to `account`, whose currency is `currency`, or
    /// the first cell, left to right, that the transaction can't do without. With no
    /// amount typed, the transaction's amount is zero.
    pub(crate) fn posting(
        &self,
        account: AccountId,
        currency: CurrencyCode,
    ) -> Result<DomainCommand, Missing> {
        let date = self.date.ok_or(Missing::Date)?;
        let amount = self.amount.unwrap_or_else(|| Money::zero(currency));
        let mut transaction = match self.allocation.as_ref().ok_or(Missing::Category)? {
            DraftAllocation::Split(split) => {
                NewTransaction::split(date, &self.payee, split.posted())
            }
            DraftAllocation::One(CategoryTarget::Category(category)) => {
                NewTransaction::new(date, &self.payee, *category, amount)
            }
            DraftAllocation::One(CategoryTarget::Transfer(other)) => {
                NewTransaction::transfer(date, &self.payee, *other, amount)
            }
        };
        if !self.number.is_empty() {
            transaction = transaction.number(&self.number);
        }
        if !self.memo.is_empty() {
            transaction = transaction.memo(&self.memo);
        }
        Ok(DomainCommand::PostTransaction {
            account,
            transaction,
        })
    }
}

/// Where the draft's amount goes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DraftAllocation {
    /// The whole amount, to one category or account, as typed in the Category cell.
    One(CategoryTarget),
    /// The split lines from the Split dialog, which add up to the amount.
    Split(SplitLines),
}

/// A draft row cell a transaction can't do without, left empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Missing {
    Date,
    Category,
}

impl Missing {
    /// Why the draft row can't be added.
    pub(crate) fn message(self, i18n: &Localizer) -> String {
        match self {
            Self::Date => fl!(i18n, "register-draft-date-missing"),
            Self::Category => fl!(i18n, "register-draft-category-missing"),
        }
    }
}
