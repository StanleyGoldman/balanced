// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`RowChanges`]: the changes made to a transaction's row in the Register, kept back
//! until the row is saved.

use balanced_core::{DomainCommand, Money, TransactionEdit, TransactionId};

use crate::register_model::RegisterRow;

/// The changes made to one transaction's row and not yet saved, at most one per field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RowChanges {
    transaction: TransactionId,
    /// In the order the save sends them: the order of [`Field`].
    edits: Vec<TransactionEdit>,
}

impl RowChanges {
    /// No changes yet to `transaction`'s row.
    pub(crate) fn new(transaction: TransactionId) -> Self {
        Self {
            transaction,
            edits: Vec::new(),
        }
    }

    pub(crate) fn transaction(&self) -> TransactionId {
        self.transaction
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// The edits, one per changed field, in the order the save sends them.
    pub(crate) fn edits(&self) -> &[TransactionEdit] {
        &self.edits
    }

    /// The new amount, if the amount is changed.
    pub(crate) fn amount(&self) -> Option<Money> {
        self.edits.iter().find_map(|edit| match edit {
            TransactionEdit::Amount(amount) => Some(*amount),
            TransactionEdit::Date(_)
            | TransactionEdit::Number(_)
            | TransactionEdit::Payee(_)
            | TransactionEdit::Memo(_)
            | TransactionEdit::Category(_)
            | TransactionEdit::Transfer(_)
            // TODO(#426): a split's lines can change its amount, which the Reconciled
            // confirm should ask about.
            | TransactionEdit::ClearedStatus(_)
            | TransactionEdit::Lines { .. } => None,
        })
    }

    /// Keeps `edit` in place of any earlier change to its field, or drops the field's
    /// change when `saved`, the transaction's row as the ledger has it, already holds
    /// the value.
    pub(crate) fn record(&mut self, saved: &RegisterRow, edit: TransactionEdit) {
        let field = Field::of(&edit);
        self.edits.retain(|kept| Field::of(kept) != field);
        if saved.holds(&edit) {
            return;
        }
        let at = self.edits.partition_point(|kept| Field::of(kept) < field);
        self.edits.insert(at, edit);
    }

    /// The Command that saves the changes: one edit Command per changed field, in one
    /// Compound, so the save is one undo step. `None` when nothing changed.
    pub(crate) fn saving(&self) -> Option<DomainCommand> {
        if self.edits.is_empty() {
            return None;
        }
        let edits = self
            .edits
            .iter()
            .map(|edit| DomainCommand::EditTransaction {
                transaction: self.transaction,
                edit: edit.clone(),
            });
        Some(DomainCommand::Compound(edits.collect()))
    }
}

/// The field of a transaction an edit changes, in the order a save sends them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Field {
    Date,
    Number,
    Payee,
    /// Where the amount goes: one category, one account a transfer goes to, or split
    /// lines.
    Category,
    Memo,
    Amount,
    ClearedStatus,
}

impl Field {
    fn of(edit: &TransactionEdit) -> Self {
        match edit {
            TransactionEdit::Date(_) => Self::Date,
            TransactionEdit::Number(_) => Self::Number,
            TransactionEdit::Payee(_) => Self::Payee,
            TransactionEdit::Category(_)
            | TransactionEdit::Transfer(_)
            | TransactionEdit::Lines { .. } => Self::Category,
            TransactionEdit::Memo(_) => Self::Memo,
            TransactionEdit::Amount(_) => Self::Amount,
            TransactionEdit::ClearedStatus(_) => Self::ClearedStatus,
        }
    }
}
