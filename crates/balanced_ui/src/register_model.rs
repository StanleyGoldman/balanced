// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`RegisterModel`]: one account's transactions as the Register shows them, with every
//! cell's text already formatted.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use balanced_core::{
    AccountId, AccountSummary, Allocation, CategoryId, ClearedStatus, CurrencyCode, Generation,
    LedgerView, Money, SplitTarget, TransactionId,
};
use balanced_i18n::{AmountStyle, Localizer};
use icu_calendar::{Date, Iso};
use icu_locale::LanguageIdentifier;
use icu_locale::Locale;

use crate::combobox::CategoryTarget;
use crate::draft_row::Draft;
use crate::formatted_amount::FormattedAmount;
use crate::i18n::fl;
use crate::register_filter::RegisterFilter;

/// Separates a category from the one it sits under: `Food & Dining:Groceries`.
pub(crate) const CATEGORY_SEPARATOR: &str = ":";

/// The Category field's name for a transfer to or from account `name`: `[Savings]`. The
/// brackets are the field's notation, which typed text is matched against, so like
/// [`CATEGORY_SEPARATOR`] they aren't translated.
pub(crate) fn transfer_text(name: &str) -> String {
    format!("[{name}]")
}

/// Which row of the Register a row is: a transaction's, or the draft row below them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RowId {
    Transaction(TransactionId),
    Draft,
}

/// Which way money moves, which decides whether an amount shows as a payment or a
/// deposit. The Register shows the amount unsigned in the column for its side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AmountSide {
    /// Money out: a negative amount.
    Payment,
    /// Money in: zero or a positive amount.
    Deposit,
}

impl AmountSide {
    /// The side `money` is on.
    pub(crate) fn of(money: Money) -> Self {
        if money.is_negative() {
            Self::Payment
        } else {
            Self::Deposit
        }
    }

    /// The signed amount of `money` on this side, whatever sign `money` has.
    pub(crate) fn signed(self, money: Money) -> Money {
        let unsigned = money.amount().abs();
        let amount = match self {
            Self::Payment => -unsigned,
            Self::Deposit => unsigned,
        };
        Money::new(amount, money.currency()).expect("a change of sign keeps the decimal places")
    }
}

/// An amount as the Register shows it, signed and unsigned.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RowAmount {
    signed: FormattedAmount,
    /// The amount without its sign, which its side's column shows.
    unsigned: FormattedAmount,
}

impl RowAmount {
    fn new(i18n: &Localizer, money: Money) -> Self {
        Self {
            signed: FormattedAmount::new(i18n, money, AmountStyle::Bare),
            unsigned: FormattedAmount::new(
                i18n,
                AmountSide::Deposit.signed(money),
                AmountStyle::Bare,
            ),
        }
    }
}

/// Where a row's amount went, as far as its Payee and Category cells care.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowKind {
    OpeningBalance,
    /// The whole amount, in one category.
    Category(CategoryId),
    Split,
    /// One side of a transfer, with its other side.
    Transfer(OtherSide),
    /// The draft row, with its category once one is typed.
    Draft(Option<CategoryTarget>),
}

/// A transfer's other side: the transaction, and the account holding it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OtherSide {
    pub(crate) account: AccountId,
    pub(crate) transaction: TransactionId,
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
    amount: Option<RowAmount>,
    balance: Option<FormattedAmount>,
    /// Whether the transaction is flagged. The draft row never is.
    flagged: bool,
    /// How far the transaction has been checked against the bank. The draft row is
    /// uncleared.
    cleared_status: ClearedStatus,
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
                .and_then(|target| match target {
                    CategoryTarget::Category(id) => category_text(view, id),
                    CategoryTarget::Transfer(account) => view
                        .account(account)
                        .map(|summary| transfer_text(summary.account().name())),
                })
                .unwrap_or_default(),
            memo: draft.memo().to_owned(),
            currency,
            amount: draft.amount().map(|amount| RowAmount::new(i18n, amount)),
            balance: None,
            flagged: false,
            cleared_status: ClearedStatus::Uncleared,
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
            RowKind::Category(id) | RowKind::Draft(Some(CategoryTarget::Category(id))) => Some(id),
            RowKind::OpeningBalance
            | RowKind::Split
            | RowKind::Transfer(_)
            | RowKind::Draft(None | Some(CategoryTarget::Transfer(_))) => None,
        }
    }

    /// The other account the row's transfer moves its amount to or from, unless it
    /// isn't a transfer, or is the draft row before an account is typed.
    pub(crate) fn transfer_account(&self) -> Option<AccountId> {
        match self.kind {
            RowKind::Transfer(OtherSide { account, .. })
            | RowKind::Draft(Some(CategoryTarget::Transfer(account))) => Some(account),
            RowKind::OpeningBalance
            | RowKind::Category(_)
            | RowKind::Split
            | RowKind::Draft(None | Some(CategoryTarget::Category(_))) => None,
        }
    }

    /// The other side of the transfer the row shows, unless it shows none.
    pub(crate) fn other_side(&self) -> Option<OtherSide> {
        match self.kind {
            RowKind::Transfer(other_side) => Some(other_side),
            RowKind::OpeningBalance | RowKind::Category(_) | RowKind::Split | RowKind::Draft(_) => {
                None
            }
        }
    }

    /// Whether the whole amount goes in one category or to one other account, which
    /// the Category cell edits.
    pub(crate) fn takes_category(&self) -> bool {
        match self.kind {
            RowKind::Category(_) | RowKind::Transfer(_) | RowKind::Draft(_) => true,
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

    /// The signed amount. Only the draft row can be without one.
    pub(crate) fn amount(&self) -> Option<Money> {
        self.amount.as_ref().map(|amount| amount.signed.money())
    }

    /// The signed amount's text, or nothing when the row has no amount.
    pub(crate) fn amount_text(&self) -> &str {
        self.amount
            .as_ref()
            .map_or("", |amount| amount.signed.text())
    }

    /// The amount without its sign, as a bare number, when it is on `side`.
    pub(crate) fn amount_on(&self, side: AmountSide) -> Option<&FormattedAmount> {
        self.amount
            .as_ref()
            .filter(|amount| AmountSide::of(amount.signed.money()) == side)
            .map(|amount| &amount.unsigned)
    }

    /// The text of [`RegisterRow::amount_on`], or nothing when the amount is on the
    /// other side or the row has none.
    pub(crate) fn amount_text_on(&self, side: AmountSide) -> &str {
        self.amount_on(side).map_or("", FormattedAmount::text)
    }

    /// The running balance after this transaction, as a bare number. Under a sort
    /// other than by date it is `None`, since it would be out of order.
    pub(crate) fn balance(&self) -> Option<&FormattedAmount> {
        self.balance.as_ref()
    }

    /// Whether the transaction is flagged. The draft row never is.
    pub(crate) fn is_flagged(&self) -> bool {
        self.flagged
    }

    /// How far the transaction has been checked against the bank. The draft row is
    /// uncleared.
    pub(crate) fn cleared_status(&self) -> ClearedStatus {
        self.cleared_status
    }

    /// Orders this row and `other` by `key`'s column alone.
    fn compare(&self, other: &Self, key: SortKey, i18n: &Localizer) -> Ordering {
        match key {
            SortKey::Date => self.place.cmp(&other.place),
            SortKey::Payee => i18n.compare_text(&self.payee, &other.payee),
            SortKey::Category => i18n.compare_text(&self.category, &other.category),
            SortKey::Memo => i18n.compare_text(&self.memo, &other.memo),
            SortKey::Amount(_) => {
                let amount = |row: &Self| row.amount().map(|amount| amount.amount());
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
    /// The signed amount. The side names the column whose heading was clicked, which
    /// shows the sort arrow; both sides sort alike.
    Amount(AmountSide),
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

/// What the model was built from. It is rebuilt when any of it changes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Source {
    account: AccountId,
    generation: Generation,
    sort: RegisterSort,
    filter: RegisterFilter,
    /// Today's date, which the filter's date ranges count from.
    today: Date<Iso>,
    today_divider: TodayDivider,
    text_language: LanguageIdentifier,
    formatting_locale: Locale,
}

/// One account's Register rows that its filter shows, in the order of its sort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisterModel {
    source: Source,
    rows: Vec<RegisterRow>,
    /// Every transaction in the account, shown or not.
    transactions: HashSet<TransactionId>,
    /// The index of the row the Today divider sits above, while it is drawn.
    divider: Option<usize>,
}

/// What a [`RegisterModel`] shows: its sort, its filter, today's date and whether the
/// Today divider is drawn.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Showing<'a> {
    pub(crate) sort: RegisterSort,
    pub(crate) filter: &'a RegisterFilter,
    pub(crate) today: Date<Iso>,
    pub(crate) today_divider: TodayDivider,
}

impl RegisterModel {
    /// The Register of `summary`, an account in `view`, formatted by `i18n`, narrowed
    /// and ordered as `shown` says, with the Today divider placed as it says. Each
    /// row's balance is the account's running balance, whichever rows the filter hides.
    pub(crate) fn new(
        summary: &AccountSummary,
        view: &LedgerView,
        shown: Showing<'_>,
        i18n: &Localizer,
    ) -> Self {
        let Showing {
            sort,
            filter,
            today,
            today_divider,
        } = shown;
        let matcher = filter.matcher(today);
        let mut paths = CategoryPaths {
            view,
            paths: HashMap::new(),
        };
        let mut other_accounts = OtherAccounts {
            view,
            holders: None,
        };
        let split = fl!(i18n, "register-split");
        let opening_balance = fl!(i18n, "register-opening-balance");
        let shows_balance = sort.key == SortKey::Date;
        let mut rows: Vec<RegisterRow> = summary
            .register()
            .iter()
            .enumerate()
            .filter_map(|(place, entry)| {
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
                    Allocation::Transfer(other_side) => {
                        let holder = other_accounts.holding(*other_side);
                        (
                            RowKind::Transfer(OtherSide {
                                account: holder.id,
                                transaction: *other_side,
                            }),
                            transaction.payee().to_owned(),
                            transfer_text(holder.name),
                        )
                    }
                };
                let row = RegisterRow {
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
                    amount: Some(RowAmount::new(i18n, transaction.amount())),
                    balance: shows_balance
                        .then(|| FormattedAmount::new(i18n, entry.balance(), AmountStyle::Bare)),
                    flagged: transaction.is_flagged(),
                    cleared_status: transaction.cleared_status(),
                };
                let split_lines = match transaction.allocation() {
                    Allocation::Split(lines) => lines.as_slice(),
                    Allocation::OpeningBalance
                    | Allocation::Category(_)
                    | Allocation::Transfer(_) => &[],
                };
                let split_targets: Vec<String> = split_lines
                    .iter()
                    .map(|line| match line.target() {
                        SplitTarget::Category(category) => paths.path(category).to_owned(),
                        SplitTarget::Transfer(other_side) => {
                            transfer_text(other_accounts.holding(other_side).name)
                        }
                    })
                    .collect();
                matcher
                    .shows(&row, split_targets.iter().map(String::as_str))
                    .then_some(row)
            })
            .collect();
        rows.sort_by(|a, b| sort.compare(i18n, a, b));
        let divider = match today_divider {
            TodayDivider::Shown { today } if shows_balance => divider(&rows, sort.direction, today),
            TodayDivider::Shown { .. } | TodayDivider::Hidden => None,
        };
        Self {
            source: Self::source(summary.account().id(), view, shown, i18n),
            rows,
            transactions: transaction_ids(summary).collect(),
            divider,
        }
    }

    /// Whether the model shows account `account` of `view` as `shown` says, as `i18n`
    /// formats it, so needs no rebuild.
    pub(crate) fn is_current(
        &self,
        account: AccountId,
        view: &LedgerView,
        shown: Showing<'_>,
        i18n: &Localizer,
    ) -> bool {
        self.source == Self::source(account, view, shown, i18n)
    }

    fn source(
        account: AccountId,
        view: &LedgerView,
        shown: Showing<'_>,
        i18n: &Localizer,
    ) -> Source {
        Source {
            account,
            generation: view.generation(),
            sort: shown.sort,
            filter: shown.filter.clone(),
            today: shown.today,
            today_divider: shown.today_divider,
            text_language: i18n.text_language().clone(),
            formatting_locale: i18n.formatting_locale().clone(),
        }
    }

    /// The account whose rows these are.
    pub(crate) fn account(&self) -> AccountId {
        self.source.account
    }

    /// The rows the filter shows, in the order of the model's sort.
    pub(crate) fn rows(&self) -> &[RegisterRow] {
        &self.rows
    }

    /// How many transactions the account has, shown or not.
    pub(crate) fn transaction_count(&self) -> usize {
        self.transactions.len()
    }

    /// The transactions in `summary`, the model's account as it is now, that were
    /// not in it when the model was built.
    pub(crate) fn added<'a>(
        &'a self,
        summary: &'a AccountSummary,
    ) -> impl Iterator<Item = TransactionId> + 'a {
        transaction_ids(summary).filter(|id| !self.transactions.contains(id))
    }

    /// The index of the row the Today divider sits above: the first row dated after
    /// today, or under a descending sort the first row dated today or earlier. `None`
    /// while the divider is hidden, under a sort other than by date, or when no row is
    /// dated after today.
    pub(crate) fn divider(&self) -> Option<usize> {
        self.divider
    }
}

/// The ids of `summary`'s transactions, in date order.
fn transaction_ids(summary: &AccountSummary) -> impl Iterator<Item = TransactionId> + '_ {
    summary
        .register()
        .iter()
        .map(|entry| entry.transaction().id())
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

/// The accounts holding transfers' other sides, gathered the first time one is asked
/// for while a model is built.
struct OtherAccounts<'a> {
    view: &'a LedgerView,
    /// Each transaction's account, by the transaction's id. Every transaction is in it,
    /// so a file whose transfer points at a transaction of another kind still shows.
    holders: Option<HashMap<TransactionId, Holder<'a>>>,
}

/// The account holding a transaction.
#[derive(Debug, Clone, Copy)]
struct Holder<'a> {
    id: AccountId,
    name: &'a str,
}

impl<'a> OtherAccounts<'a> {
    /// The account holding transaction `other_side`.
    fn holding(&mut self, other_side: TransactionId) -> Holder<'a> {
        let view = self.view;
        let holders = self.holders.get_or_insert_with(|| {
            view.accounts()
                .iter()
                .flat_map(|summary| {
                    let account = summary.account();
                    let holder = Holder {
                        id: account.id(),
                        name: account.name(),
                    };
                    summary
                        .register()
                        .iter()
                        .map(move |entry| (entry.transaction().id(), holder))
                })
                .collect()
        });
        *holders
            .get(&other_side)
            .expect("a transfer's other side is in its snapshot")
    }
}

/// The path from the top of the tree to category `id` of `view`, as the Register shows
/// it: `Food & Dining:Groceries`. `None` once the category has been deleted.
pub(crate) fn category_text(view: &LedgerView, id: CategoryId) -> Option<String> {
    Some(view.category_path(id)?.join(CATEGORY_SEPARATOR))
}
