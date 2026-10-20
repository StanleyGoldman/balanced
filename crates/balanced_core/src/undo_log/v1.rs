// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Version 1 of the undo log's wire format: a `DomainCommand` as JSON. The format is part
//! of the data file's schema version. Adding a Command or an optional field keeps it;
//! any other change needs a migration that clears the log (ADR 0007).
//!
//! Ids are integers, amounts integer minor units in the data file's currency, and dates
//! ISO `YYYY-MM-DD` text. Enums are `snake_case`, and a Command is an object whose one key
//! names it.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Command {
    CreateAccount {
        name: String,
        kind: AccountKind,
        opened: String,
        opening_balance: i64,
    },
    DeleteAccount {
        account: u32,
    },
    RestoreAccount {
        account: Account,
        transactions: Vec<Transaction>,
        place: u32,
    },
    RenameAccount {
        account: u32,
        name: String,
    },
    SetAccountVisibility {
        account: u32,
        visibility: Visibility,
    },
    SetAccountStatus {
        account: u32,
        status: AccountStatus,
    },
    MoveAccount {
        account: u32,
        direction: MoveDirection,
    },
    /// The Commands in order, as an array.
    Compound(Vec<Self>),
    PostTransaction {
        account: u32,
        transaction: NewTransaction,
    },
    EditTransaction {
        transaction: u32,
        edit: TransactionEdit,
    },
    SetFlagged {
        transaction: u32,
        flagged: bool,
    },
    DeleteTransaction {
        transaction: u32,
    },
    /// A transaction with no other side leaves `other_sides` out.
    RestoreTransaction {
        account: u32,
        transaction: Transaction,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        other_sides: Vec<OtherSide>,
    },
    CreateCategory {
        name: String,
        parent: Option<u32>,
    },
    DeleteCategory {
        category: u32,
    },
    RestoreCategory {
        category: Category,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AccountKind {
    Checking,
    Savings,
    CreditCard,
    Investment,
    Retirement,
    Asset,
    Liability,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Visibility {
    Shown,
    Hidden,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AccountStatus {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MoveDirection {
    Up,
    Down,
}

/// An account, in the data file's currency.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Account {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) kind: AccountKind,
    pub(crate) visibility: Visibility,
    pub(crate) status: AccountStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Category {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) parent: Option<u32>,
}

/// A transaction with its id: a [`NewTransaction`]'s fields after `id`, in one object,
/// a transfer and a split line to an account naming its other side, then `flagged`, which an unflagged transaction
/// leaves out, and `cleared_status`, which an uncleared one leaves out. An entry written
/// before the status existed has none, and reads as uncleared.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Transaction {
    pub(crate) id: u32,
    #[serde(flatten)]
    pub(crate) fields: NewTransaction<TransferSide, LineSide>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) flagged: bool,
    #[serde(default, skip_serializing_if = "ClearedStatus::is_uncleared")]
    pub(crate) cleared_status: ClearedStatus,
}

/// A transfer's other side, or a split line's, in account `account`.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct OtherSide {
    pub(crate) account: u32,
    pub(crate) transaction: Transaction,
}

/// A transaction to post, which the ledger gives an id. A split's amount is the sum of
/// its lines. `T` is how a transfer is given, and `L` how a split line's transfer is: by
/// the account it goes to, for a transaction to post.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct NewTransaction<T = TransferTo, L = LineTo> {
    pub(crate) date: String,
    pub(crate) number: Option<String>,
    pub(crate) payee: String,
    pub(crate) memo: String,
    pub(crate) allocation: Allocation<T, L>,
}

/// Where a transaction's amount went, with the amount where it isn't the lines' sum. A
/// transfer is a `T`, and a split line's transfer an `L`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Allocation<T, L> {
    OpeningBalance {
        amount: i64,
    },
    Category {
        category: u32,
        amount: i64,
    },
    /// The lines in order, as an array.
    Split(Vec<SplitLine<L>>),
    Transfer(T),
}

/// A transfer to post, to or from `account`.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TransferTo {
    pub(crate) account: u32,
    pub(crate) amount: i64,
}

/// One side of a transfer, whose other side is transaction `other_side`.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TransferSide {
    pub(crate) other_side: u32,
    pub(crate) amount: i64,
}

/// A split line to post's transfer, to or from `account`.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct LineTo {
    pub(crate) account: u32,
}

/// A posted split line's transfer, whose other side is transaction `other_side`.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct LineSide {
    pub(crate) other_side: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SplitLine<L> {
    /// Written as the line's own key, `"category": 3` or `"transfer": {…}`, beside its
    /// memo and amount.
    #[serde(flatten)]
    pub(crate) target: SplitTarget<L>,
    pub(crate) memo: String,
    pub(crate) amount: i64,
}

/// Where a split line's part of the amount went. A transfer is an `L`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SplitTarget<L> {
    Category(u32),
    Transfer(L),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TransactionEdit {
    Date(String),
    Number(Option<String>),
    Payee(String),
    Memo(String),
    Category(u32),
    Amount(i64),
    ClearedStatus(ClearedStatus),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ClearedStatus {
    #[default]
    Uncleared,
    Cleared,
    Reconciled,
}

impl ClearedStatus {
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde's skip_serializing_if passes the field by reference"
    )]
    fn is_uncleared(&self) -> bool {
        *self == Self::Uncleared
    }
}
