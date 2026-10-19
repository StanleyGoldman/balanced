//! [`RegisterColumn`], a column of the Register, and [`RegisterColumns`], which of them
//! every Register shows and in what order.

use serde::{Deserialize, Serialize};

/// A Register column, in the default left-to-right order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RegisterColumn {
    Date,
    Number,
    Payee,
    Category,
    Memo,
    /// Money out, unsigned. Its heading depends on the account's kind.
    Payment,
    /// Money in, unsigned. Its heading depends on the account's kind.
    Deposit,
    Balance,
    /// Whether the transaction is flagged, which clicking the cell changes.
    Flag,
    /// The untitled column holding each row's Actions menu.
    Actions,
}

impl RegisterColumn {
    /// Every column, in the default order. A new column goes here as well as in the
    /// matches below, which the compiler checks; it can't check this list.
    pub(crate) const ALL: [Self; 10] = [
        Self::Date,
        Self::Number,
        Self::Payee,
        Self::Category,
        Self::Memo,
        Self::Payment,
        Self::Deposit,
        Self::Balance,
        Self::Flag,
        Self::Actions,
    ];

    /// The column's stable id, which the saved layout and the preferences file name it
    /// by.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Number => "num",
            Self::Payee => "payee",
            Self::Category => "category",
            Self::Memo => "memo",
            Self::Payment => "payment",
            Self::Deposit => "deposit",
            Self::Balance => "balance",
            Self::Flag => "flag",
            Self::Actions => "actions",
        }
    }

    /// The column whose id is `id`.
    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|column| column.id() == id)
    }

    /// Whether the column chooser can hide the column. A new transaction needs a date,
    /// a category and an amount, so those columns stay (an amount goes in Payment or
    /// Deposit), and the Actions column stays last.
    pub(crate) fn can_hide(self) -> bool {
        match self {
            Self::Number | Self::Payee | Self::Memo | Self::Balance | Self::Flag => true,
            Self::Date | Self::Category | Self::Payment | Self::Deposit | Self::Actions => false,
        }
    }

    /// Whether the column chooser can move the column. The Actions column stays last.
    fn can_move(self) -> bool {
        self != Self::Actions
    }
}

/// The columns every Register shows, and their order. The Actions column is always shown
/// and always last, so neither list holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct RegisterColumns {
    /// Every column but Actions, left to right, the hidden ones included.
    #[serde(rename = "register_columns")]
    order: ColumnOrder,
    /// The columns the user hid.
    #[serde(rename = "hidden_register_columns")]
    hidden: HiddenColumns,
}

impl Default for RegisterColumns {
    fn default() -> Self {
        Self {
            order: ColumnOrder::from(Vec::new()),
            hidden: HiddenColumns::from(Vec::new()),
        }
    }
}

/// Every column but Actions, in the user's order. The file holds their ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Vec<String>", into = "Vec<&'static str>")]
struct ColumnOrder(Vec<RegisterColumn>);

impl From<Vec<String>> for ColumnOrder {
    /// The columns `ids` names, in their order. An unknown id, a repeat and the Actions
    /// column are dropped, and a column `ids` leaves out, such as one new in this
    /// release, is appended.
    fn from(ids: Vec<String>) -> Self {
        let named = ids.iter().filter_map(|id| RegisterColumn::from_id(id));
        Self(distinct(
            named.chain(RegisterColumn::ALL),
            RegisterColumn::can_move,
        ))
    }
}

impl From<ColumnOrder> for Vec<&'static str> {
    fn from(order: ColumnOrder) -> Self {
        order.0.into_iter().map(RegisterColumn::id).collect()
    }
}

/// The columns the user hid. The file holds their ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Vec<String>", into = "Vec<&'static str>")]
struct HiddenColumns(Vec<RegisterColumn>);

impl From<Vec<String>> for HiddenColumns {
    /// The columns `ids` names that can be hidden. An unknown id and a repeat are
    /// dropped.
    fn from(ids: Vec<String>) -> Self {
        let named = ids.iter().filter_map(|id| RegisterColumn::from_id(id));
        Self(distinct(named, RegisterColumn::can_hide))
    }
}

/// The columns of `columns` that `keep` accepts, each once, in their first places.
fn distinct(
    columns: impl Iterator<Item = RegisterColumn>,
    keep: fn(RegisterColumn) -> bool,
) -> Vec<RegisterColumn> {
    let mut kept: Vec<RegisterColumn> = Vec::new();
    for column in columns {
        if keep(column) && !kept.contains(&column) {
            kept.push(column);
        }
    }
    kept
}

impl From<HiddenColumns> for Vec<&'static str> {
    fn from(hidden: HiddenColumns) -> Self {
        hidden.0.into_iter().map(RegisterColumn::id).collect()
    }
}

impl RegisterColumns {
    /// The columns a Register shows, left to right, ending with Actions.
    pub(crate) fn shown(&self) -> Vec<RegisterColumn> {
        self.order
            .0
            .iter()
            .copied()
            .filter(|column| !self.hidden.0.contains(column))
            .chain([RegisterColumn::Actions])
            .collect()
    }

    /// Every column the chooser lists, left to right, with whether it is shown.
    pub(crate) fn entries(&self) -> Vec<(RegisterColumn, bool)> {
        self.order
            .0
            .iter()
            .map(|&column| (column, !self.hidden.0.contains(&column)))
            .collect()
    }

    /// Shows or hides `column`, if it can be hidden.
    pub(crate) fn set_shown(&mut self, column: RegisterColumn, shown: bool) {
        if !column.can_hide() {
            return;
        }
        self.hidden.0.retain(|&hidden| hidden != column);
        if !shown {
            self.hidden.0.push(column);
        }
    }

    /// Moves `column` one place up the list, which is left in the Register, unless it is first.
    pub(crate) fn move_up(&mut self, column: RegisterColumn) {
        if let Some(index) = self.position(column)
            && index > 0
        {
            self.order.0.swap(index - 1, index);
        }
    }

    /// Moves `column` one place down the list, which is right in the Register, unless it is last.
    pub(crate) fn move_down(&mut self, column: RegisterColumn) {
        if let Some(index) = self.position(column)
            && index + 1 < self.order.0.len()
        {
            self.order.0.swap(index, index + 1);
        }
    }

    fn position(&self, column: RegisterColumn) -> Option<usize> {
        self.order.0.iter().position(|&listed| listed == column)
    }
}
