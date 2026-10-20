// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! A [`Ledger`] over a data file: creating and opening one, and converting between the
//! domain's types and the store's rows.

use std::collections::HashMap;
use std::num::TryFromIntError;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use balanced_store::OpenError as StoreOpenError;
use balanced_store::{
    AccountRow, AllocationRow, CategoryRow, Contents, SplitLineRow, SplitTargetRow, Store,
    StoreError, TransactionRow, UpgradeError,
};
use icu_calendar::{Date, Iso};
use rust_decimal::Decimal;

use crate::ledger::{AccountSummary, History, NextIds, RegisterEntry, Totals};
use crate::transaction::{SplitLine, SplitTarget};
use crate::undo_log::{self, UndoEntryError};
use crate::{
    Account, AccountGroup, AccountId, AccountKind, AccountStatus, Allocation, Category, CategoryId,
    ClearedStatus, CurrencyCode, DomainError, Ledger, Money, MoneyError, Transaction,
    TransactionId, Visibility,
};

/// Why a data file couldn't be created or opened. The refusals mirror
/// `balanced_store`'s, so the UI picks a Dialog from this enum alone.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    /// No file exists at the path.
    #[error("no file exists at {0:?}")]
    Missing(PathBuf),
    /// The file isn't SQLite, or another program's SQLite file.
    #[error("the file isn't a Balanced data file")]
    NotBalanced,
    /// The file fails SQLite's integrity check, or holds an undo log entry Balanced
    /// can't read.
    #[error("the data file is damaged")]
    Damaged(#[source] Damage),
    /// The file's schema is from a newer Balanced than this one.
    #[error("the data file is at schema version {version}, above this Balanced's {latest}")]
    Newer { version: u32, latest: u32 },
    /// Another connection, in this program or another, holds the file.
    #[error("the data file is open elsewhere")]
    Locked,
    /// The file is from an older Balanced, and the backup taken before upgrading it
    /// couldn't be written, so it wasn't upgraded.
    #[error("the data file couldn't be backed up before its upgrade")]
    BackupFailed(#[source] StoreError),
    /// Upgrading the file from an older Balanced failed, so it was left at its old
    /// version. The backup taken first stays at `backup`.
    #[error("the data file couldn't be upgraded; its backup is at {backup:?}")]
    UpgradeFailed {
        backup: PathBuf,
        #[source]
        source: UpgradeError,
    },
    #[error("the data file couldn't be read or written")]
    Store(#[from] StoreError),
    #[error("the data file's currency isn't one Balanced knows")]
    Currency(#[source] MoneyError),
    /// A name the schema admits but Balanced doesn't know, such as a kind of account
    /// from a newer Balanced.
    #[error("the data file holds a {what} Balanced doesn't know: {value:?}")]
    Unknown { what: &'static str, value: String },
    /// A date the schema admits but Balanced's calendar can't hold. `reason` is ICU's,
    /// which implements `Display` but not `Error`, so it can't be a source.
    #[error("the data file holds a date Balanced can't read ({reason}): {value:?}")]
    Date {
        value: String,
        reason: icu_calendar::ParseError,
    },
}

impl From<StoreOpenError> for OpenError {
    fn from(error: StoreOpenError) -> Self {
        match error {
            StoreOpenError::Missing(path) => Self::Missing(path),
            StoreOpenError::NotBalanced => Self::NotBalanced,
            StoreOpenError::Damaged(damage) => Self::Damaged(Damage::File(damage)),
            StoreOpenError::Newer { version, latest } => Self::Newer { version, latest },
            StoreOpenError::Locked => Self::Locked,
            StoreOpenError::BackupFailed(error) => Self::BackupFailed(error),
            StoreOpenError::UpgradeFailed { backup, source } => {
                Self::UpgradeFailed { backup, source }
            }
            StoreOpenError::Store(error) => Self::Store(error),
        }
    }
}

/// What is wrong with a damaged data file.
#[derive(Debug, thiserror::Error)]
pub enum Damage {
    /// SQLite found the file damaged.
    #[error(transparent)]
    File(balanced_store::Damage),
    /// An entry in the undo log isn't a Command this Balanced can read.
    #[error("an undo log entry can't be read")]
    UndoEntry(#[source] UndoEntryError),
    /// Transaction `transaction` is a side of a transfer that disagrees with
    /// `other_side`: the other side doesn't name it back, or the two differ in their
    /// date or amount. Balanced keeps both sides in step, so only another program can
    /// have written this.
    #[error("{transaction:?} and its other side {other_side:?} disagree")]
    TransferSides {
        transaction: TransactionId,
        other_side: TransactionId,
    },
}

/// Why the data file couldn't take a change.
#[derive(Debug, Clone, thiserror::Error)]
pub enum WriteError {
    /// An amount has more minor units than the file can hold.
    #[error("the amount is too large for the data file")]
    AmountOutOfRange(#[source] TryFromIntError),
    /// SQLite refused the write or the commit.
    #[error("the data file refused the change")]
    Sqlite(#[source] Arc<StoreError>),
    /// The change failed, and so did rolling the file back, which leaves the file's
    /// transaction open.
    #[error("the data file couldn't roll back a failed change: {change}")]
    RollbackFailed {
        /// Why the change failed.
        change: Box<DomainError>,
        #[source]
        source: Arc<StoreError>,
    },
}

impl PartialEq for WriteError {
    /// Two SQLite failures are equal only when they are the same failure.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::AmountOutOfRange(a), Self::AmountOutOfRange(b)) => a == b,
            (Self::Sqlite(a), Self::Sqlite(b)) => Arc::ptr_eq(a, b),
            (
                Self::RollbackFailed { change, source },
                Self::RollbackFailed {
                    change: other_change,
                    source: other_source,
                },
            ) => change == other_change && Arc::ptr_eq(source, other_source),
            (Self::AmountOutOfRange(_) | Self::Sqlite(_) | Self::RollbackFailed { .. }, _) => false,
        }
    }
}

impl Eq for WriteError {}

impl From<StoreError> for WriteError {
    fn from(error: StoreError) -> Self {
        Self::Sqlite(Arc::new(error))
    }
}

impl Ledger {
    /// Creates an empty data file at `path` whose amounts are in `currency`, and a
    /// ledger over it that holds the file locked until it is dropped. Refuses a path that
    /// already holds a file.
    pub fn create(path: &Path, currency: CurrencyCode) -> Result<Self, OpenError> {
        let store = Store::create(path, currency.as_str())?;
        let zero = Money::zero(currency);
        Ok(Self::from_parts(
            store,
            Vec::new(),
            Vec::new(),
            zero,
            Totals::new([zero; AccountGroup::ALL.len()], zero),
            NextIds {
                account: AccountId::new(0),
                transaction: TransactionId::new(0),
                category: CategoryId::new(0),
            },
        ))
    }

    /// Opens the data file at `path`, reading all of it into a ledger that holds the file
    /// locked until it is dropped. Refuses a file that is missing, isn't a Balanced data
    /// file, is damaged, comes from a newer Balanced, or is open elsewhere.
    pub fn open(path: &Path) -> Result<Self, OpenError> {
        let store = Store::open(path)?;
        let contents = store.read()?;
        Self::load(store, contents)
    }

    /// [`Self::open`] with `migrations` in place of Balanced's own, for tests that upgrade
    /// a file through steps of their own.
    #[cfg(feature = "test-support")]
    pub fn open_with_migrations(
        path: &Path,
        migrations: &[balanced_store::Migration],
    ) -> Result<Self, OpenError> {
        let store = Store::open_with_migrations(path, migrations)?;
        let contents = store.read()?;
        Self::load(store, contents)
    }

    /// A ledger over `store` holding `contents`, which `store` just read.
    fn load(store: Store, contents: Contents) -> Result<Self, OpenError> {
        let currency = CurrencyCode::new(&contents.currency).map_err(OpenError::Currency)?;
        let money = |minor_units| money(minor_units, currency);
        let categories = contents
            .categories
            .into_iter()
            .map(|row| {
                Category::new(
                    CategoryId::new(row.id),
                    row.name,
                    row.parent_id.map(CategoryId::new),
                )
            })
            .collect();
        // Transactions come by account, in register order.
        let mut registers: HashMap<u32, Vec<RegisterEntry>> = HashMap::new();
        for stored in contents.transactions {
            let account = stored.row.account_id;
            let transaction = transaction(stored.row, money(stored.amount), currency)?;
            registers
                .entry(account)
                .or_default()
                .push(RegisterEntry::new(transaction, money(stored.balance)));
        }
        let mut accounts = contents
            .accounts
            .into_iter()
            .map(|stored| {
                let row = stored.row;
                let kind = kind(&row.kind)?;
                let account = Account::new(
                    AccountId::new(row.id),
                    row.name,
                    kind,
                    currency,
                    if row.hidden {
                        Visibility::Hidden
                    } else {
                        Visibility::Shown
                    },
                    if row.closed {
                        AccountStatus::Closed
                    } else {
                        AccountStatus::Open
                    },
                );
                let register = registers.remove(&row.id).unwrap_or_default();
                let summary = AccountSummary::loaded(
                    account,
                    register,
                    money(stored.balance),
                    money(stored.cleared_balance),
                );
                Ok(((kind, row.position), summary))
            })
            .collect::<Result<Vec<_>, OpenError>>()?;
        // The store orders kinds by name; the Account bar orders them by group.
        accounts.sort_by_key(|&(order, _)| order);
        let accounts: Vec<AccountSummary> =
            accounts.into_iter().map(|(_, summary)| summary).collect();
        check_transfers(&accounts)?;
        let mut group_totals = [Money::zero(currency); AccountGroup::ALL.len()];
        for (name, total) in contents.group_totals {
            // `index` counts within `AccountGroup::ALL`, which sizes the array.
            group_totals[group(&name)?.index()] = money(total);
        }
        let next = |last: Option<u32>| last.map_or(0, |id| id + 1);
        let decode = |entries: Vec<String>| {
            entries
                .iter()
                .map(|entry| undo_log::decode(entry, currency))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| OpenError::Damaged(Damage::UndoEntry(error)))
        };
        let history = History {
            undo: decode(contents.undo)?,
            redo: decode(contents.redo)?,
        };
        Ok(Self::from_parts(
            store,
            accounts,
            categories,
            money(contents.net_worth),
            Totals::new(group_totals, money(contents.closed_total)),
            NextIds {
                account: AccountId::new(next(contents.last_ids.account)),
                transaction: TransactionId::new(next(contents.last_ids.transaction)),
                category: CategoryId::new(next(contents.last_ids.category)),
            },
        )
        .with_history(history))
    }
}

/// Refuses `accounts` as damaged when a transfer's two sides disagree, naming the
/// disagreeing side with the lowest id. The schema guards only that each other side
/// exists.
fn check_transfers(accounts: &[AccountSummary]) -> Result<(), OpenError> {
    let transactions: HashMap<TransactionId, &Transaction> = accounts
        .iter()
        .flat_map(AccountSummary::register)
        .map(|entry| (entry.transaction().id(), entry.transaction()))
        .collect();
    let mut sides: Vec<(&Transaction, TransactionId)> = transactions
        .values()
        .filter_map(|&side| side.other_side_id().map(|other_side| (side, other_side)))
        .collect();
    sides.sort_by_key(|(side, _)| side.id());
    for (side, other_side) in sides {
        let agrees = transactions.get(&other_side).is_some_and(|other| {
            other.other_side_id() == Some(side.id())
                && other.date() == side.date()
                && other.amount() == -side.amount()
        });
        if !agrees {
            return Err(OpenError::Damaged(Damage::TransferSides {
                transaction: side.id(),
                other_side,
            }));
        }
    }
    Ok(())
}

/// `minor_units` of `currency`: cents, for USD.
pub(crate) fn money(minor_units: i64, currency: CurrencyCode) -> Money {
    Money::new(
        Decimal::new(minor_units, currency.decimal_places()),
        currency,
    )
    .expect("any i64 of minor units is an amount at its currency's decimal places")
}

/// `money` in minor units: cents, for USD. Fails when it has more than an `i64` holds.
pub(crate) fn minor_units(money: Money) -> Result<i64, WriteError> {
    // `Money` is always at its currency's decimal places, so its mantissa counts minor
    // units.
    i64::try_from(money.amount().mantissa()).map_err(WriteError::AmountOutOfRange)
}

/// The store's name for `kind`.
pub(crate) fn kind_name(kind: AccountKind) -> &'static str {
    match kind {
        AccountKind::Checking => "checking",
        AccountKind::Savings => "savings",
        AccountKind::CreditCard => "credit_card",
        AccountKind::Investment => "investment",
        AccountKind::Retirement => "retirement",
        AccountKind::Asset => "asset",
        AccountKind::Liability => "liability",
    }
}

/// The store's name for `group`.
fn group_name(group: AccountGroup) -> &'static str {
    match group {
        AccountGroup::Banking => "banking",
        AccountGroup::Investing => "investing",
        AccountGroup::PropertyDebt => "property_debt",
    }
}

fn group(name: &str) -> Result<AccountGroup, OpenError> {
    AccountGroup::ALL
        .into_iter()
        .find(|&group| group_name(group) == name)
        .ok_or_else(|| OpenError::Unknown {
            what: "group",
            value: name.to_owned(),
        })
}

/// The store's name for `status`.
fn cleared_status_name(status: ClearedStatus) -> &'static str {
    match status {
        ClearedStatus::Uncleared => "uncleared",
        ClearedStatus::Cleared => "cleared",
        ClearedStatus::Reconciled => "reconciled",
    }
}

fn cleared_status(name: &str) -> Result<ClearedStatus, OpenError> {
    ClearedStatus::ALL
        .into_iter()
        .find(|&status| cleared_status_name(status) == name)
        .ok_or_else(|| OpenError::Unknown {
            what: "cleared status",
            value: name.to_owned(),
        })
}

fn kind(name: &str) -> Result<AccountKind, OpenError> {
    AccountKind::ALL
        .into_iter()
        .find(|&kind| kind_name(kind) == name)
        .ok_or_else(|| OpenError::Unknown {
            what: "kind of account",
            value: name.to_owned(),
        })
}

/// `date` as ISO `YYYY-MM-DD` text.
pub(crate) fn date_text(date: Date<Iso>) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year().extended_year(),
        date.month().ordinal,
        date.day_of_month().0
    )
}

fn date(text: &str) -> Result<Date<Iso>, OpenError> {
    Date::try_from_str(text, Iso).map_err(|reason| OpenError::Date {
        value: text.to_owned(),
        reason,
    })
}

/// The transaction `row` holds, whose amount is `amount`.
fn transaction(
    row: TransactionRow,
    amount: Money,
    currency: CurrencyCode,
) -> Result<Transaction, OpenError> {
    let allocation = match row.allocation {
        AllocationRow::OpeningBalance { .. } => Allocation::OpeningBalance,
        AllocationRow::Category { category_id, .. } => {
            Allocation::Category(CategoryId::new(category_id))
        }
        AllocationRow::Transfer { other_side_id, .. } => {
            Allocation::Transfer(TransactionId::new(other_side_id))
        }
        AllocationRow::Split(lines) => Allocation::Split(
            lines
                .into_iter()
                .map(|line| {
                    SplitLine::new(
                        match line.target {
                            SplitTargetRow::Category { category_id } => {
                                SplitTarget::Category(CategoryId::new(category_id))
                            }
                        },
                        &line.memo,
                        money(line.amount, currency),
                    )
                })
                .collect(),
        ),
    };
    Ok(Transaction::from_parts(
        TransactionId::new(row.id),
        date(&row.date)?,
        row.number,
        row.payee,
        row.memo,
        allocation,
        amount,
    )
    .with_flag(row.flagged)
    .with_cleared_status(cleared_status(&row.cleared_status)?))
}

/// The row for `account`, at `position` in its kind.
pub(crate) fn account_row(account: &Account, position: usize) -> AccountRow {
    AccountRow {
        id: account.id().raw(),
        name: account.name().to_owned(),
        kind: kind_name(account.kind()).to_owned(),
        position: u32::try_from(position).expect("a kind holds fewer than 2^32 accounts"),
        hidden: account.is_hidden(),
        closed: account.is_closed(),
    }
}

/// The row for `category`.
pub(crate) fn category_row(category: &Category) -> CategoryRow {
    CategoryRow {
        id: category.id().raw(),
        parent_id: category.parent().map(CategoryId::raw),
        name: category.name().to_owned(),
    }
}

/// The row for `transaction`, in account `account`. Fails when an amount has more minor
/// units than the file holds.
pub(crate) fn transaction_row(
    account: AccountId,
    transaction: &Transaction,
) -> Result<TransactionRow, WriteError> {
    let allocation = match transaction.allocation() {
        Allocation::OpeningBalance => AllocationRow::OpeningBalance {
            amount: minor_units(transaction.amount())?,
        },
        Allocation::Category(category) => AllocationRow::Category {
            category_id: category.raw(),
            amount: minor_units(transaction.amount())?,
        },
        Allocation::Transfer(other_side) => AllocationRow::Transfer {
            other_side_id: other_side.raw(),
            amount: minor_units(transaction.amount())?,
        },
        Allocation::Split(lines) => AllocationRow::Split(
            lines
                .iter()
                .map(|line| {
                    Ok(SplitLineRow {
                        target: match line.target() {
                            SplitTarget::Category(category) => SplitTargetRow::Category {
                                category_id: category.raw(),
                            },
                        },
                        memo: line.memo().to_owned(),
                        amount: minor_units(line.amount())?,
                    })
                })
                .collect::<Result<_, WriteError>>()?,
        ),
    };
    Ok(TransactionRow {
        id: transaction.id().raw(),
        account_id: account.raw(),
        date: date_text(transaction.date()),
        number: transaction.number().map(str::to_owned),
        payee: transaction.payee().to_owned(),
        memo: transaction.memo().to_owned(),
        allocation,
        flagged: transaction.is_flagged(),
        cleared_status: cleared_status_name(transaction.cleared_status()).to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out_of_range() -> WriteError {
        WriteError::AmountOutOfRange(
            i64::try_from(u128::MAX).expect_err("u128::MAX doesn't fit in an i64"),
        )
    }

    fn store_error() -> Arc<StoreError> {
        Arc::new(StoreError::AlreadyExists(PathBuf::from(
            "Household.balanced",
        )))
    }

    #[test]
    fn two_out_of_range_amounts_are_the_same_error() {
        assert_eq!(out_of_range(), out_of_range());
    }

    #[test]
    fn an_out_of_range_amount_is_not_a_sqlite_failure() {
        assert_ne!(out_of_range(), WriteError::Sqlite(store_error()));
    }

    #[test]
    fn sqlite_failures_are_the_same_error_only_when_they_are_one_failure() {
        let failure = store_error();

        assert_eq!(
            WriteError::Sqlite(Arc::clone(&failure)),
            WriteError::Sqlite(failure),
        );
        assert_ne!(
            WriteError::Sqlite(store_error()),
            WriteError::Sqlite(store_error()),
        );
    }

    #[test]
    fn rollback_failures_of_different_changes_are_different_errors() {
        let source = store_error();
        let rollback_failed = |change| WriteError::RollbackFailed {
            change: Box::new(change),
            source: Arc::clone(&source),
        };

        assert_ne!(
            rollback_failed(DomainError::BlankAccountName),
            rollback_failed(DomainError::UnknownAccount {
                account: AccountId::new(0),
            }),
        );
    }
}
