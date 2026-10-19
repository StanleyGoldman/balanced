//! [`RegisterModel`]: one account's transactions as the Register shows them, with every
//! cell's text already formatted.

use std::cmp::Ordering;
use std::collections::HashMap;

use balanced_core::{
    AccountId, AccountSummary, Allocation, CategoryId, CurrencyCode, Generation, LedgerView,
    TransactionId,
};
use balanced_i18n::{AmountStyle, Localizer};
use icu_calendar::{Date, Iso};
use icu_locale::LanguageIdentifier;
use icu_locale::Locale;

use crate::draft_row::Draft;
use crate::formatted_amount::FormattedAmount;
use crate::i18n::fl;
use crate::preferences::Preferences;

/// Separates a category from the one it sits under: `Food & Dining:Groceries`.
pub(crate) const CATEGORY_SEPARATOR: &str = ":";

/// Which row of the Register a row is: a transaction's, or the draft row below them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RowId {
    Transaction(TransactionId),
    Draft,
}

/// Where a row's amount went, as far as its Payee and Category cells care.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowKind {
    OpeningBalance,
    /// The whole amount, in one category.
    Category(CategoryId),
    Split,
    /// The draft row, with its category once one is typed.
    Draft(Option<CategoryId>),
}

/// One row of the Register, every cell formatted: a transaction's, or the draft row's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisterRow {
    id: RowId,
    /// The row's place in date order, which breaks ties in every sort.
    place: usize,
    kind: RowKind,
    /// The date, which only the draft row can be without.
    saved_date: Option<Date<Iso>>,
    date: String,
    number: String,
    payee: String,
    category: String,
    memo: String,
    currency: CurrencyCode,
    /// The amount, which only the draft row can be without.
    amount: Option<FormattedAmount>,
    balance: Option<FormattedAmount>,
}

impl RegisterRow {
    /// The draft row of an account in `currency`, showing what `draft` holds, with
    /// categories named as in `view`.
    pub(crate) fn draft(
        draft: &Draft,
        currency: CurrencyCode,
        view: &LedgerView,
        i18n: &Localizer,
    ) -> Self {
        Self {
            id: RowId::Draft,
            place: usize::MAX,
            kind: RowKind::Draft(draft.category()),
            saved_date: draft.date(),
            date: draft
                .date()
                .map(|date| i18n.format_date(date))
                .unwrap_or_default(),
            number: draft.number().to_owned(),
            payee: draft.payee().to_owned(),
            category: draft
                .category()
                .and_then(|id| category_text(view, id))
                .unwrap_or_default(),
            memo: draft.memo().to_owned(),
            currency,
            amount: draft
                .amount()
                .map(|amount| FormattedAmount::new(i18n, amount, AmountStyle::Bare)),
            balance: None,
        }
    }

    pub(crate) fn id(&self) -> RowId {
        self.id
    }

    /// The transaction the row shows, unless it is the draft row.
    pub(crate) fn transaction(&self) -> Option<TransactionId> {
        match self.id {
            RowId::Transaction(id) => Some(id),
            RowId::Draft => None,
        }
    }

    /// Whether this is the account's opening balance, whose Payee cell shows a
    /// caption rather than a payee.
    pub(crate) fn is_opening_balance(&self) -> bool {
        self.kind == RowKind::OpeningBalance
    }

    /// The one category the whole amount is in, unless the row is the opening balance
    /// or split, or the draft row before a category is typed.
    pub(crate) fn category_id(&self) -> Option<CategoryId> {
        match self.kind {
            RowKind::Category(id) | RowKind::Draft(Some(id)) => Some(id),
            RowKind::OpeningBalance | RowKind::Split | RowKind::Draft(None) => None,
        }
    }

    /// Whether the whole amount goes in one category, which the Category cell edits.
    pub(crate) fn takes_category(&self) -> bool {
        match self.kind {
            RowKind::Category(_) | RowKind::Draft(_) => true,
            RowKind::OpeningBalance | RowKind::Split => false,
        }
    }

    /// Whether the amount is split between categories.
    pub(crate) fn is_split(&self) -> bool {
        self.kind == RowKind::Split
    }

    /// The transaction's date, which the Date cell shows formatted. Only the draft row
    /// can be without one.
    pub(crate) fn saved_date(&self) -> Option<Date<Iso>> {
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

    /// The currency of the transaction's account.
    pub(crate) fn currency(&self) -> CurrencyCode {
        self.currency
    }

    /// The amount, as a bare number. Only the draft row can be without one.
    pub(crate) fn amount(&self) -> Option<&FormattedAmount> {
        self.amount.as_ref()
    }

    /// The amount's text, or nothing when the row has no amount.
    pub(crate) fn amount_text(&self) -> &str {
        self.amount.as_ref().map_or("", FormattedAmount::text)
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
            SortKey::Amount => {
                let amount = |row: &Self| row.amount.as_ref().map(|amount| amount.money().amount());
                amount(self).cmp(&amount(other))
            }
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

/// Whether the Register sets its rows dated after today apart with the Today divider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TodayDivider {
    Hidden,
    /// Drawn above the first row dated after `today`, under a sort by date.
    Shown {
        today: Date<Iso>,
    },
}

impl TodayDivider {
    /// The Today divider as `preferences` say, on `today`.
    pub(crate) fn of(preferences: &Preferences, today: Date<Iso>) -> Self {
        if preferences.today_divider {
            Self::Shown { today }
        } else {
            Self::Hidden
        }
    }
}

/// What the model was built from. It is rebuilt when any of it changes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Source {
    account: AccountId,
    generation: Generation,
    sort: RegisterSort,
    today_divider: TodayDivider,
    text_language: LanguageIdentifier,
    formatting_locale: Locale,
}

/// One account's Register rows, in the order of its sort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisterModel {
    source: Source,
    rows: Vec<RegisterRow>,
    /// The index of the row the Today divider sits above, while it is drawn.
    divider: Option<usize>,
}

impl RegisterModel {
    /// The Register of `summary`, an account in `view`, formatted by `i18n` and
    /// ordered by `sort`, with the Today divider placed as `today_divider` says.
    pub(crate) fn new(
        summary: &AccountSummary,
        view: &LedgerView,
        sort: RegisterSort,
        today_divider: TodayDivider,
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
                    id: RowId::Transaction(transaction.id()),
                    place,
                    kind,
                    saved_date: Some(transaction.date()),
                    date: i18n.format_date(transaction.date()),
                    number: transaction.number().unwrap_or_default().to_owned(),
                    payee,
                    category,
                    memo: transaction.memo().to_owned(),
                    currency: transaction.amount().currency(),
                    amount: Some(FormattedAmount::new(
                        i18n,
                        transaction.amount(),
                        AmountStyle::Bare,
                    )),
                    balance: shows_balance
                        .then(|| FormattedAmount::new(i18n, entry.balance(), AmountStyle::Bare)),
                }
            })
            .collect();
        rows.sort_by(|a, b| sort.compare(i18n, a, b));
        let divider = match today_divider {
            TodayDivider::Shown { today } if shows_balance => divider(&rows, sort.direction, today),
            TodayDivider::Shown { .. } | TodayDivider::Hidden => None,
        };
        Self {
            source: Self::source(summary.account().id(), view, sort, today_divider, i18n),
            rows,
            divider,
        }
    }

    /// Whether the model shows account `account` of `view` ordered by `sort`, with
    /// the Today divider as `today_divider` says, as `i18n` formats it, so needs no
    /// rebuild.
    pub(crate) fn is_current(
        &self,
        account: AccountId,
        view: &LedgerView,
        sort: RegisterSort,
        today_divider: TodayDivider,
        i18n: &Localizer,
    ) -> bool {
        self.source == Self::source(account, view, sort, today_divider, i18n)
    }

    fn source(
        account: AccountId,
        view: &LedgerView,
        sort: RegisterSort,
        today_divider: TodayDivider,
        i18n: &Localizer,
    ) -> Source {
        Source {
            account,
            generation: view.generation(),
            sort,
            today_divider,
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

    /// The index of the row the Today divider sits above: the first row dated after
    /// today, or under a descending sort the first row dated today or earlier. `None`
    /// while the divider is hidden, under a sort other than by date, or when no row is
    /// dated after today.
    pub(crate) fn divider(&self) -> Option<usize> {
        self.divider
    }
}

/// The index of the row the Today divider sits above in `rows`, sorted by date in
/// `direction`, unless no row is dated after `today`.
fn divider(rows: &[RegisterRow], direction: SortDirection, today: Date<Iso>) -> Option<usize> {
    let scheduled = |row: &RegisterRow| row.saved_date.is_some_and(|date| date > today);
    match direction {
        SortDirection::Ascending => rows.iter().position(scheduled),
        // The scheduled rows come first, so there are some only if the first is one.
        SortDirection::Descending => scheduled(rows.first()?).then(|| {
            rows.iter()
                .position(|row| !scheduled(row))
                .unwrap_or(rows.len())
        }),
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
