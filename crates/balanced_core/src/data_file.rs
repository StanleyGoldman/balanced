//! A [`Ledger`] over a data file: creating and opening one, and converting between the
//! domain's types and the store's rows.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use balanced_store::{
    AccountRow, AllocationRow, CategoryRow, Contents, SplitLineRow, Store, StoreError,
    TransactionRow,
};
use icu_calendar::{Date, Iso};
use rust_decimal::Decimal;

use crate::ledger::{AccountSummary, NextIds, RegisterEntry, Totals};
use crate::transaction::SplitLine;
use crate::{
    Account, AccountGroup, AccountId, AccountKind, AccountStatus, Allocation, Category, CategoryId,
    CurrencyCode, Ledger, Money, MoneyError, Transaction, TransactionId, Visibility,
};

/// Why a data file couldn't be created or opened.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("the data file couldn't be read or written")]
    Store(#[from] StoreError),
    #[error("the data file's currency isn't one Balanced knows")]
    Currency(#[source] MoneyError),
    /// A value the schema admits but Balanced can't hold, such as a date outside the
    /// calendar.
    #[error("the data file holds a {what} Balanced can't read: {value:?}")]
    Unreadable { what: &'static str, value: String },
}

/// Why the data file couldn't take a change.
#[derive(Debug, Clone, thiserror::Error)]
pub enum WriteError {
    /// An amount has more minor units than the file can hold.
    #[error("the amount is too large for the data file")]
    AmountOutOfRange,
    /// SQLite refused the write or the commit.
    #[error("the data file refused the change")]
    Store(#[source] Arc<StoreError>),
}

impl PartialEq for WriteError {
    /// Two store failures are equal only when they are the same failure.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::AmountOutOfRange, Self::AmountOutOfRange) => true,
            (Self::Store(a), Self::Store(b)) => Arc::ptr_eq(a, b),
            (Self::AmountOutOfRange | Self::Store(_), _) => false,
        }
    }
}

impl Eq for WriteError {}

impl From<StoreError> for WriteError {
    fn from(error: StoreError) -> Self {
        Self::Store(Arc::new(error))
    }
}

impl Ledger {
    /// Creates an empty data file at `path` whose amounts are in `currency`, and a
    /// ledger over it. Refuses a path that already holds a file.
    pub fn create(path: &Path, currency: CurrencyCode) -> Result<Self, OpenError> {
        let store = Store::create(path, currency.as_str())?;
        let zero = Money::zero(currency);
        Ok(Self::loaded(
            store,
            Vec::new(),
            Vec::new(),
            zero,
            Totals::new([zero; 3], zero),
            NextIds {
                account: AccountId::new(0),
                transaction: TransactionId::new(0),
                category: CategoryId::new(0),
            },
        ))
    }

    /// Opens the data file at `path`, reading all of it into a ledger.
    pub fn open(path: &Path) -> Result<Self, OpenError> {
        let store = Store::open(path)?;
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
                let summary = AccountSummary::loaded(account, register, money(stored.balance));
                Ok(((kind, row.position), summary))
            })
            .collect::<Result<Vec<_>, OpenError>>()?;
        // The store orders kinds by name; the Account bar orders them by group.
        accounts.sort_by_key(|&(order, _)| order);
        let accounts = accounts.into_iter().map(|(_, summary)| summary).collect();
        let mut group_totals = [Money::zero(currency); 3];
        for (name, total) in contents.group_totals {
            let group = match name.as_str() {
                "banking" => AccountGroup::Banking,
                "investing" => AccountGroup::Investing,
                "property_debt" => AccountGroup::PropertyDebt,
                _ => {
                    return Err(OpenError::Unreadable {
                        what: "group",
                        value: name,
                    });
                }
            };
            group_totals[group.index()] = money(total);
        }
        let next = |last: Option<u32>| last.map_or(0, |id| id + 1);
        Ok(Self::loaded(
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
        ))
    }
}

/// `minor_units` of `currency`: cents, for USD.
fn money(minor_units: i64, currency: CurrencyCode) -> Money {
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
    i64::try_from(money.amount().mantissa()).map_err(|_| WriteError::AmountOutOfRange)
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

fn kind(name: &str) -> Result<AccountKind, OpenError> {
    AccountKind::ALL
        .into_iter()
        .find(|&kind| kind_name(kind) == name)
        .ok_or_else(|| OpenError::Unreadable {
            what: "kind of account",
            value: name.to_owned(),
        })
}

/// `date` as ISO `YYYY-MM-DD` text.
fn date_text(date: Date<Iso>) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year().extended_year(),
        date.month().ordinal,
        date.day_of_month().0
    )
}

fn date(text: &str) -> Result<Date<Iso>, OpenError> {
    let unreadable = || OpenError::Unreadable {
        what: "date",
        value: text.to_owned(),
    };
    let mut parts = text.splitn(3, '-');
    let mut part = || parts.next().ok_or_else(unreadable);
    let (year, month, day) = (part()?, part()?, part()?);
    Date::try_new_iso(
        year.parse().map_err(|_| unreadable())?,
        month.parse().map_err(|_| unreadable())?,
        day.parse().map_err(|_| unreadable())?,
    )
    .map_err(|_| unreadable())
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
        AllocationRow::Split(lines) => Allocation::Split(
            lines
                .into_iter()
                .map(|line| {
                    SplitLine::new(
                        CategoryId::new(line.category_id),
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
    ))
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
        Allocation::Split(lines) => AllocationRow::Split(
            lines
                .iter()
                .map(|line| {
                    Ok(SplitLineRow {
                        category_id: line.category().raw(),
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
    })
}
