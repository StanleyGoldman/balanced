//! [`RegisterModel`]: one account's transactions as the Register shows them, with every
//! cell's text already formatted.

use std::cmp::Ordering;
use std::collections::HashMap;

use balanced_core::{
    AccountId, AccountSummary, Allocation, CategoryId, Generation, LedgerView, TransactionId,
};
use balanced_i18n::{AmountStyle, Localizer};
use icu_calendar::{Date, Iso};
use icu_locale::LanguageIdentifier;
use icu_locale::Locale;

use crate::formatted_amount::FormattedAmount;
use crate::i18n::fl;

/// Separates a category from the one it sits under: `Food & Dining:Groceries`.
const CATEGORY_SEPARATOR: &str = ":";

/// Where a row's amount went, as far as its Payee and Category cells care.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowKind {
    OpeningBalance,
    /// The whole amount, in one category.
    Category(CategoryId),
    Split,
}

/// One transaction's row in the Register, every cell formatted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisterRow {
    id: TransactionId,
    /// The row's place in date order, which breaks ties in every sort.
    place: usize,
    kind: RowKind,
    saved_date: Date<Iso>,
    date: String,
    number: String,
    payee: String,
    category: String,
    memo: String,
    amount: FormattedAmount,
    balance: Option<FormattedAmount>,
}

impl RegisterRow {
    pub(crate) fn id(&self) -> TransactionId {
        self.id
    }

    /// Whether this is the account's opening balance, whose Payee cell shows a
    /// caption rather than a payee.
    pub(crate) fn is_opening_balance(&self) -> bool {
        self.kind == RowKind::OpeningBalance
    }

    /// The one category the whole amount is in, unless the row is the opening balance
    /// or split.
    pub(crate) fn category_id(&self) -> Option<CategoryId> {
        match self.kind {
            RowKind::Category(id) => Some(id),
            RowKind::OpeningBalance | RowKind::Split => None,
        }
    }

    /// Whether the amount is split between categories.
    pub(crate) fn is_split(&self) -> bool {
        self.kind == RowKind::Split
    }

    /// The transaction's date, which the Date cell shows formatted.
    pub(crate) fn saved_date(&self) -> Date<Iso> {
        self.saved_date
    }

    pub(crate) fn date(&self) -> &str {
        &self.date
    }

    /// The cheque or reference number, or nothing.
    pub(crate) fn number(&self) -> &str {
        &self.number
    }

    pub(crate) fn payee(&self) -> &str {
        &self.payee
    }

    /// The category's path from the top of the tree, `[Split]` for a split
    /// transaction, or nothing for the opening balance.
    pub(crate) fn category(&self) -> &str {
        &self.category
    }

    pub(crate) fn memo(&self) -> &str {
        &self.memo
    }

    /// The amount, as a bare number.
    pub(crate) fn amount(&self) -> &FormattedAmount {
        &self.amount
    }

    /// The running balance after this transaction, as a bare number. Under a sort
    /// other than by date it is `None`, since it would be out of order.
    pub(crate) fn balance(&self) -> Option<&FormattedAmount> {
        self.balance.as_ref()
    }

    /// Orders this row and `other` by `key`'s column alone.
    fn compare(&self, other: &Self, key: SortKey, i18n: &Localizer) -> Ordering {
        match key {
            SortKey::Date => self.place.cmp(&other.place),
            SortKey::Payee => i18n.compare_text(&self.payee, &other.payee),
            SortKey::Category => i18n.compare_text(&self.category, &other.category),
            SortKey::Memo => i18n.compare_text(&self.memo, &other.memo),
            SortKey::Amount => self
                .amount
                .money()
                .amount()
                .cmp(&other.amount.money().amount()),
        }
    }
}

/// A column the Register sorts by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SortKey {
    Date,
    Payee,
    Category,
    Memo,
    Amount,
}

/// Which way a sort runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortDirection {
    /// Oldest, A to Z, or smallest first.
    Ascending,
    Descending,
}

/// How the Register orders its rows. Every Register opens sorted by date, oldest
/// first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RegisterSort {
    key: SortKey,
    direction: SortDirection,
}

impl Default for RegisterSort {
    fn default() -> Self {
        Self {
            key: SortKey::Date,
            direction: SortDirection::Ascending,
        }
    }
}

impl RegisterSort {
    pub(crate) fn key(self) -> SortKey {
        self.key
    }

    pub(crate) fn direction(self) -> SortDirection {
        self.direction
    }

    /// The sort after a click on `key`'s header: the same column reverses, another
    /// sorts by it ascending.
    pub(crate) fn clicked(self, key: SortKey) -> Self {
        let direction = match (key == self.key, self.direction) {
            (true, SortDirection::Ascending) => SortDirection::Descending,
            (true, SortDirection::Descending) | (false, _) => SortDirection::Ascending,
        };
        Self { key, direction }
    }

    /// Orders two rows. Rows that tie stay in date order, oldest first, whichever way
    /// the sort runs.
    fn compare(self, i18n: &Localizer, a: &RegisterRow, b: &RegisterRow) -> Ordering {
        let ordering = a.compare(b, self.key, i18n);
        let ordering = match self.direction {
            SortDirection::Ascending => ordering,
            SortDirection::Descending => ordering.reverse(),
        };
        ordering.then(a.place.cmp(&b.place))
    }
}

/// What the model was built from. It is rebuilt when any of it changes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Source {
    account: AccountId,
    generation: Generation,
    sort: RegisterSort,
    text_language: LanguageIdentifier,
    formatting_locale: Locale,
}

/// One account's Register rows, in the order of its sort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisterModel {
    source: Source,
    rows: Vec<RegisterRow>,
}

impl RegisterModel {
    /// The Register of `summary`, an account in `view`, formatted by `i18n` and
    /// ordered by `sort`.
    pub(crate) fn new(
        summary: &AccountSummary,
        view: &LedgerView,
        sort: RegisterSort,
        i18n: &Localizer,
    ) -> Self {
        let mut paths = CategoryPaths {
            view,
            paths: HashMap::new(),
        };
        let split = fl!(i18n, "register-split");
        let opening_balance = fl!(i18n, "register-opening-balance");
        let shows_balance = sort.key == SortKey::Date;
        let mut rows: Vec<RegisterRow> = summary
            .register()
            .iter()
            .enumerate()
            .map(|(place, entry)| {
                let transaction = entry.transaction();
                let (kind, payee, category) = match transaction.allocation() {
                    Allocation::OpeningBalance => (
                        RowKind::OpeningBalance,
                        opening_balance.clone(),
                        String::new(),
                    ),
                    Allocation::Category(id) => (
                        RowKind::Category(*id),
                        transaction.payee().to_owned(),
                        paths.path(*id).to_owned(),
                    ),
                    Allocation::Split(_) => (
                        RowKind::Split,
                        transaction.payee().to_owned(),
                        split.clone(),
                    ),
                };
                RegisterRow {
                    id: transaction.id(),
                    place,
                    kind,
                    saved_date: transaction.date(),
                    date: i18n.format_date(transaction.date()),
                    number: transaction.number().unwrap_or_default().to_owned(),
                    payee,
                    category,
                    memo: transaction.memo().to_owned(),
                    amount: FormattedAmount::new(i18n, transaction.amount(), AmountStyle::Bare),
                    balance: shows_balance
                        .then(|| FormattedAmount::new(i18n, entry.balance(), AmountStyle::Bare)),
                }
            })
            .collect();
        rows.sort_by(|a, b| sort.compare(i18n, a, b));
        Self {
            source: Self::source(summary.account().id(), view, sort, i18n),
            rows,
        }
    }

    /// Whether the model shows account `account` of `view` ordered by `sort`, as `i18n`
    /// formats it, so needs no rebuild.
    pub(crate) fn is_current(
        &self,
        account: AccountId,
        view: &LedgerView,
        sort: RegisterSort,
        i18n: &Localizer,
    ) -> bool {
        self.source == Self::source(account, view, sort, i18n)
    }

    fn source(
        account: AccountId,
        view: &LedgerView,
        sort: RegisterSort,
        i18n: &Localizer,
    ) -> Source {
        Source {
            account,
            generation: view.generation(),
            sort,
            text_language: i18n.text_language().clone(),
            formatting_locale: i18n.formatting_locale().clone(),
        }
    }

    /// The account whose rows these are.
    pub(crate) fn account(&self) -> AccountId {
        self.source.account
    }

    /// The rows, in the order of the model's sort.
    pub(crate) fn rows(&self) -> &[RegisterRow] {
        &self.rows
    }
}

/// Category paths, each built once while a model is built.
struct CategoryPaths<'a> {
    view: &'a LedgerView,
    paths: HashMap<CategoryId, String>,
}

impl CategoryPaths<'_> {
    /// The path from the top of the tree to category `id`: `Food & Dining:Groceries`.
    fn path(&mut self, id: CategoryId) -> &str {
        let view = self.view;
        self.paths.entry(id).or_insert_with(|| {
            category_text(view, id).expect("a transaction's categories are in its snapshot")
        })
    }
}

/// The path from the top of the tree to category `id` of `view`, as the Register shows
/// it: `Food & Dining:Groceries`. `None` once the category has been deleted.
pub(crate) fn category_text(view: &LedgerView, id: CategoryId) -> Option<String> {
    Some(view.category_path(id)?.join(CATEGORY_SEPARATOR))
}
