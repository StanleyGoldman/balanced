// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`Draft`]: the transaction typed into the draft row at the bottom of a Register,
//! before it is added.

use balanced_core::{AccountId, DomainCommand, Money, NewTransaction, TransactionEdit};
use balanced_i18n::Localizer;
use icu_calendar::{Date, Iso};

use crate::combobox::{CategoryTarget, Recalled};
use crate::i18n::fl;

/// What the draft row holds so far. Every cell starts empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Draft {
    date: Option<Date<Iso>>,
    number: String,
    payee: String,
    category: Option<CategoryTarget>,
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

    pub(crate) fn category(&self) -> Option<CategoryTarget> {
        self.category
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
                self.category = Some(CategoryTarget::Category(category));
            }
            TransactionEdit::Transfer(account) => {
                self.category = Some(CategoryTarget::Transfer(account));
            }
            TransactionEdit::Amount(amount) => self.amount = Some(amount),
            // The draft row's Clr cell is empty: a posted transaction starts uncleared.
            TransactionEdit::ClearedStatus(_) => {}
        }
    }

    /// Takes `recalled`'s spelling of the payee, and fills the Category, Memo and amount
    /// cells from it where they are empty.
    pub(crate) fn recall(&mut self, recalled: &Recalled) {
        recalled.payee().clone_into(&mut self.payee);
        if self.category.is_none() {
            self.category = recalled.category();
        }
        if self.memo.is_empty() {
            recalled.memo().clone_into(&mut self.memo);
        }
        if self.amount.is_none() {
            self.amount = recalled.amount();
        }
    }

    /// Whether the cell `missing` names is still empty.
    pub(crate) fn lacks(&self, missing: Missing) -> bool {
        match missing {
            Missing::Date => self.date.is_none(),
            Missing::Category => self.category.is_none(),
            Missing::Amount => self.amount.is_none(),
        }
    }

    /// The Command that adds the draft to `account`, or the first cell, left to right,
    /// that the transaction can't do without.
    pub(crate) fn posting(&self, account: AccountId) -> Result<DomainCommand, Missing> {
        let date = self.date.ok_or(Missing::Date)?;
        let category = self.category.ok_or(Missing::Category)?;
        let amount = self.amount.ok_or(Missing::Amount)?;
        let mut transaction = match category {
            CategoryTarget::Category(category) => {
                NewTransaction::new(date, &self.payee, category, amount)
            }
            CategoryTarget::Transfer(other) => {
                NewTransaction::transfer(date, &self.payee, other, amount)
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

/// A draft row cell a transaction can't do without, left empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Missing {
    Date,
    Category,
    Amount,
}

impl Missing {
    /// Why the draft row was refused, as its bubble says it.
    pub(crate) fn message(self, i18n: &Localizer) -> String {
        match self {
            Self::Date => fl!(i18n, "register-draft-date-missing"),
            Self::Category => fl!(i18n, "register-draft-category-missing"),
            Self::Amount => fl!(i18n, "register-draft-amount-missing"),
        }
    }
}
