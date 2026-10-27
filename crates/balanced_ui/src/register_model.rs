// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`RegisterModel`]: one account's transactions as the Register shows them, with every
//! cell's text already formatted.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::ptr;
use std::sync::Arc;

use balanced_core::{
    Account, AccountId, AccountSummary, Allocation, CategoryId, CategoryType, ClearedStatus,
    CurrencyCode, EditedSplitLine, EditedSplitTarget, Generation, LedgerView, Money, NewSplitLine,
    Register, RegisterEntry, SplitLine, SplitTarget, Transaction, TransactionEdit, TransactionId,
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

    /// The signed amount of `money` typed in this side's column. A negative typed in
    /// one column is an amount on the other.
    pub(crate) fn typed(self, money: Money) -> Money {
        match self {
            Self::Payment => -money,
            Self::Deposit => money,
        }
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
    /// A transaction's row whose category, or the account its transfer goes to, is
    /// changed and not yet saved.
    Changed(CategoryTarget),
}

/// One line of a split, as a row holds it and the Split dialog edits it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SplitLineRow {
    /// Where the line's part of the amount goes.
    target: CategoryTarget,
    /// The other side the line keeps, when it goes to another account and already has
    /// one there.
    other_side: Option<TransactionId>,
    memo: String,
    /// The line's signed amount, in the transaction's currency.
    amount: Money,
}

impl SplitLineRow {
    /// A line of `amount` to `target`, memoed `memo`, keeping `other_side` when it goes
    /// to another account and already has one there.
    pub(crate) fn new(
        target: CategoryTarget,
        other_side: Option<TransactionId>,
        memo: &str,
        amount: Money,
    ) -> Self {
        Self {
            target,
            other_side,
            memo: memo.to_owned(),
            amount,
        }
    }

    pub(crate) fn target(&self) -> CategoryTarget {
        self.target
    }

    pub(crate) fn other_side(&self) -> Option<TransactionId> {
        self.other_side
    }

    pub(crate) fn memo(&self) -> &str {
        &self.memo
    }

    pub(crate) fn amount(&self) -> Money {
        self.amount
    }

    /// The line as a [`TransactionEdit::Lines`] edit gives it.
    fn edited(&self) -> EditedSplitLine {
        match (self.target, self.other_side) {
            (CategoryTarget::Transfer(_), Some(other_side)) => {
                EditedSplitLine::kept(other_side, &self.memo, self.amount)
            }
            (CategoryTarget::Transfer(account), None) => {
                EditedSplitLine::transfer(account, &self.memo, self.amount)
            }
            (CategoryTarget::Category(category), _) => {
                EditedSplitLine::category(category, &self.memo, self.amount)
            }
        }
    }

    /// The line of a new transaction to post. A new transaction has no other sides to
    /// keep.
    fn posted(&self) -> NewSplitLine {
        match self.target {
            CategoryTarget::Category(category) => {
                NewSplitLine::category(category, &self.memo, self.amount)
            }
            CategoryTarget::Transfer(account) => {
                NewSplitLine::transfer(account, &self.memo, self.amount)
            }
        }
    }
}

/// A transaction's split lines, and its amount, which they add up to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SplitLines {
    amount: Money,
    lines: Vec<SplitLineRow>,
}

impl SplitLines {
    /// `lines`, which add up to `amount`.
    pub(crate) fn new(amount: Money, lines: Vec<SplitLineRow>) -> Self {
        Self { amount, lines }
    }

    pub(crate) fn amount(&self) -> Money {
        self.amount
    }

    pub(crate) fn lines(&self) -> &[SplitLineRow] {
        &self.lines
    }

    /// The edit that gives a transaction these lines.
    pub(crate) fn edit(&self) -> TransactionEdit {
        TransactionEdit::Lines {
            amount: self.amount,
            lines: self.lines.iter().map(SplitLineRow::edited).collect(),
        }
    }

    /// The lines of a new transaction to post.
    pub(crate) fn posted(&self) -> Vec<NewSplitLine> {
        self.lines.iter().map(SplitLineRow::posted).collect()
    }
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
    /// The row's place in date order, which breaks ties in every sort.
    place: usize,
    /// The running balance after the transaction, which is formatted only when the row
    /// is drawn.
    balance: Option<Money>,
    /// Shared by the models of successive snapshots while the transaction is unchanged.
    cells: Arc<RowCells>,
}

/// What a row shows, every cell formatted, and the text the filter searches.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RowCells {
    id: RowId,
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
    /// Whether the transaction is flagged. The draft row never is.
    flagged: bool,
    /// How far the transaction has been checked against the bank. The draft row is
    /// uncleared.
    cleared_status: ClearedStatus,
    /// The split lines, when the amount is split.
    lines: Vec<SplitLineRow>,
    /// What the Category field shows for each split line's target, which the filter
    /// searches: a category's path, or `[Account]` for a transfer.
    split_targets: Vec<String>,
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
        let (kind, category, lines) = match draft.split() {
            Some(split) => (
                RowKind::Split,
                fl!(i18n, "register-split"),
                split.lines().to_vec(),
            ),
            None => (
                RowKind::Draft(draft.category()),
                draft
                    .category()
                    .and_then(|target| target_text(view, target))
                    .unwrap_or_default(),
                Vec::new(),
            ),
        };
        let cells = RowCells {
            id: RowId::Draft,
            kind,
            saved_date: draft.date(),
            date: draft
                .date()
                .map(|date| i18n.format_date(date))
                .unwrap_or_default(),
            number: draft.number().to_owned(),
            payee: draft.payee().to_owned(),
            category,
            memo: draft.memo().to_owned(),
            currency,
            amount: draft.amount().map(|amount| RowAmount::new(i18n, amount)),
            flagged: false,
            cleared_status: ClearedStatus::Uncleared,
            lines,
            split_targets: Vec::new(),
        };
        Self {
            place: usize::MAX,
            balance: None,
            cells: Arc::new(cells),
        }
    }

    /// This transaction's row showing `edits`, changes not yet saved, with categories
    /// and accounts named as in `view`. The balance stays the saved one.
    pub(crate) fn changed(
        &self,
        edits: &[TransactionEdit],
        view: &LedgerView,
        i18n: &Localizer,
    ) -> Self {
        let mut row = self.clone();
        let cells = Arc::make_mut(&mut row.cells);
        for edit in edits {
            match edit {
                TransactionEdit::Date(date) => {
                    cells.saved_date = Some(*date);
                    cells.date = i18n.format_date(*date);
                }
                TransactionEdit::Number(number) => {
                    cells.number = number.clone().unwrap_or_default();
                }
                TransactionEdit::Payee(payee) => payee.clone_into(&mut cells.payee),
                TransactionEdit::Memo(memo) => memo.clone_into(&mut cells.memo),
                TransactionEdit::Category(category) => {
                    cells.kind = RowKind::Changed(CategoryTarget::Category(*category));
                    cells.category = category_text(view, *category).unwrap_or_default();
                    cells.lines.clear();
                    cells.split_targets.clear();
                }
                TransactionEdit::Transfer(account) => {
                    cells.kind = RowKind::Changed(CategoryTarget::Transfer(*account));
                    cells.category =
                        target_text(view, CategoryTarget::Transfer(*account)).unwrap_or_default();
                    cells.lines.clear();
                    cells.split_targets.clear();
                }
                TransactionEdit::Amount(amount) => {
                    cells.amount = Some(RowAmount::new(i18n, *amount));
                }
                TransactionEdit::ClearedStatus(status) => cells.cleared_status = *status,
                TransactionEdit::Lines { amount, lines } => {
                    cells.kind = RowKind::Split;
                    cells.category = fl!(i18n, "register-split");
                    cells.amount = Some(RowAmount::new(i18n, *amount));
                    cells.lines = lines.iter().map(|line| self.line_row(line)).collect();
                    cells.split_targets = cells
                        .lines
                        .iter()
                        .map(|line| target_text(view, line.target).unwrap_or_default())
                        .collect();
                }
            }
        }
        row
    }

    /// Whether the row already holds the value `edit` gives its field.
    pub(crate) fn holds(&self, edit: &TransactionEdit) -> bool {
        match edit {
            TransactionEdit::Date(date) => self.saved_date() == Some(*date),
            TransactionEdit::Number(number) => {
                self.number() == number.as_deref().unwrap_or_default()
            }
            TransactionEdit::Payee(payee) => self.payee() == payee,
            TransactionEdit::Memo(memo) => self.memo() == memo,
            TransactionEdit::Category(category) => self.category_id() == Some(*category),
            TransactionEdit::Transfer(account) => self.transfer_account() == Some(*account),
            TransactionEdit::Amount(amount) => self.amount() == Some(*amount),
            TransactionEdit::ClearedStatus(status) => self.cleared_status() == *status,
            TransactionEdit::Lines { amount, lines } => {
                self.is_split()
                    && self.amount() == Some(*amount)
                    && self
                        .cells
                        .lines
                        .iter()
                        .map(SplitLineRow::edited)
                        .eq(lines.iter().cloned())
            }
        }
    }

    /// `line`, an edited line of this transaction, as the row holds it. A kept line
    /// goes to the account of the other side it keeps, one of this row's.
    fn line_row(&self, line: &EditedSplitLine) -> SplitLineRow {
        let (target, other_side) = match line.target() {
            EditedSplitTarget::Category(category) => (CategoryTarget::Category(category), None),
            EditedSplitTarget::Transfer(account) => (CategoryTarget::Transfer(account), None),
            EditedSplitTarget::Kept(other_side) => {
                let account = self
                    .cells
                    .lines
                    .iter()
                    .find(|kept| kept.other_side == Some(other_side))
                    .map(|kept| kept.target)
                    .or_else(|| {
                        self.other_side()
                            .filter(|side| side.transaction == other_side)
                            .map(|side| CategoryTarget::Transfer(side.account))
                    })
                    .expect("a kept line keeps one of the row's other sides");
                (account, Some(other_side))
            }
        };
        SplitLineRow::new(target, other_side, line.memo(), line.amount())
    }

    pub(crate) fn id(&self) -> RowId {
        self.cells.id
    }

    /// The transaction the row shows, unless it is the draft row.
    pub(crate) fn transaction(&self) -> Option<TransactionId> {
        match self.cells.id {
            RowId::Transaction(id) => Some(id),
            RowId::Draft => None,
        }
    }

    /// Whether this is the account's opening balance, whose Payee cell shows a
    /// caption rather than a payee.
    pub(crate) fn is_opening_balance(&self) -> bool {
        self.cells.kind == RowKind::OpeningBalance
    }

    /// The one category the whole amount is in, unless the row is the opening balance
    /// or split, or the draft row before a category is typed.
    pub(crate) fn category_id(&self) -> Option<CategoryId> {
        match self.cells.kind {
            RowKind::Category(id)
            | RowKind::Draft(Some(CategoryTarget::Category(id)))
            | RowKind::Changed(CategoryTarget::Category(id)) => Some(id),
            RowKind::OpeningBalance
            | RowKind::Split
            | RowKind::Transfer(_)
            | RowKind::Draft(None | Some(CategoryTarget::Transfer(_)))
            | RowKind::Changed(CategoryTarget::Transfer(_)) => None,
        }
    }

    /// The other account the row's transfer moves its amount to or from, unless it
    /// isn't a transfer, or is the draft row before an account is typed.
    pub(crate) fn transfer_account(&self) -> Option<AccountId> {
        match self.cells.kind {
            RowKind::Transfer(OtherSide { account, .. })
            | RowKind::Draft(Some(CategoryTarget::Transfer(account)))
            | RowKind::Changed(CategoryTarget::Transfer(account)) => Some(account),
            RowKind::OpeningBalance
            | RowKind::Category(_)
            | RowKind::Split
            | RowKind::Draft(None | Some(CategoryTarget::Category(_)))
            | RowKind::Changed(CategoryTarget::Category(_)) => None,
        }
    }

    /// The other side of the transfer the row shows, unless it shows none.
    pub(crate) fn other_side(&self) -> Option<OtherSide> {
        match self.cells.kind {
            RowKind::Transfer(other_side) => Some(other_side),
            RowKind::OpeningBalance
            | RowKind::Category(_)
            | RowKind::Split
            | RowKind::Draft(_)
            | RowKind::Changed(_) => None,
        }
    }

    /// Whether the whole amount goes in one category or to one other account, which
    /// the Category cell edits.
    pub(crate) fn takes_category(&self) -> bool {
        match self.cells.kind {
            RowKind::Category(_)
            | RowKind::Transfer(_)
            | RowKind::Draft(_)
            | RowKind::Changed(_) => true,
            RowKind::OpeningBalance | RowKind::Split => false,
        }
    }

    /// Whether the amount is split between categories.
    pub(crate) fn is_split(&self) -> bool {
        self.cells.kind == RowKind::Split
    }

    /// The split lines, when the amount is split.
    pub(crate) fn split_lines(&self) -> &[SplitLineRow] {
        &self.cells.lines
    }

    /// What the Category field shows for each split line's target: a category's path,
    /// or `[Account]` for a transfer.
    pub(crate) fn split_targets(&self) -> impl Iterator<Item = &str> {
        self.cells.split_targets.iter().map(String::as_str)
    }

    /// The transaction's date, which the Date cell shows formatted. Only the draft row
    /// can be without one.
    pub(crate) fn saved_date(&self) -> Option<Date<Iso>> {
        self.cells.saved_date
    }

    pub(crate) fn date(&self) -> &str {
        &self.cells.date
    }

    /// The cheque or reference number, or nothing.
    pub(crate) fn number(&self) -> &str {
        &self.cells.number
    }

    pub(crate) fn payee(&self) -> &str {
        &self.cells.payee
    }

    /// The category's path from the top of the tree, `[Split]` for a split
    /// transaction, or nothing for the opening balance.
    pub(crate) fn category(&self) -> &str {
        &self.cells.category
    }

    pub(crate) fn memo(&self) -> &str {
        &self.cells.memo
    }

    /// The currency of the transaction's account.
    pub(crate) fn currency(&self) -> CurrencyCode {
        self.cells.currency
    }

    /// The signed amount. Only the draft row can be without one.
    pub(crate) fn amount(&self) -> Option<Money> {
        self.cells
            .amount
            .as_ref()
            .map(|amount| amount.signed.money())
    }

    /// The signed amount's text, or nothing when the row has no amount.
    pub(crate) fn amount_text(&self) -> &str {
        self.cells
            .amount
            .as_ref()
            .map_or("", |amount| amount.signed.text())
    }

    /// The amount without its sign, as a bare number, when it is on `side`. A zero
    /// amount is on neither side.
    pub(crate) fn amount_on(&self, side: AmountSide) -> Option<&FormattedAmount> {
        self.cells
            .amount
            .as_ref()
            .filter(|amount| {
                let money = amount.signed.money();
                !money.amount().is_zero() && AmountSide::of(money) == side
            })
            .map(|amount| &amount.unsigned)
    }

    /// The text of [`RegisterRow::amount_on`], or nothing when the amount is zero or
    /// on the other side, or the row has none.
    pub(crate) fn amount_text_on(&self, side: AmountSide) -> &str {
        self.amount_on(side).map_or("", FormattedAmount::text)
    }

    /// The running balance after this transaction, unformatted, since only the rows
    /// drawn need its text. Under a sort other than by date it is `None`, since it
    /// would be out of order.
    pub(crate) fn balance(&self) -> Option<Money> {
        self.balance
    }

    /// Whether the transaction is flagged. The draft row never is.
    pub(crate) fn is_flagged(&self) -> bool {
        self.cells.flagged
    }

    /// How far the transaction has been checked against the bank. The draft row is
    /// uncleared.
    pub(crate) fn cleared_status(&self) -> ClearedStatus {
        self.cells.cleared_status
    }

    /// Orders this row and `other` by `key`'s column alone.
    fn compare(&self, other: &Self, key: SortKey, i18n: &Localizer) -> Ordering {
        match key {
            SortKey::Date => self.place.cmp(&other.place),
            SortKey::Payee => i18n.compare_text(self.payee(), other.payee()),
            SortKey::Category => i18n.compare_text(self.category(), other.category()),
            SortKey::Memo => i18n.compare_text(self.memo(), other.memo()),
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

impl Source {
    /// Whether cells formatted for a model built from `self` still hold for one built
    /// from `later`, as far as the account and the locale go.
    fn formats_like(&self, later: &Self) -> bool {
        self.account == later.account
            && self.text_language == later.text_language
            && self.formatting_locale == later.formatting_locale
    }
}

/// One account's Register rows that its filter shows, in the order of its sort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisterModel {
    source: Source,
    /// The account as the model was built from it.
    summary: Arc<AccountSummary>,
    /// The names the cells show for categories and accounts.
    names: Names,
    /// Every transaction's cells, shown or not, in register order.
    cells: Vec<Arc<RowCells>>,
    rows: Vec<RegisterRow>,
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
    /// `earlier` is the model this one replaces, if any, whose cells it keeps for every
    /// transaction that is unchanged, unless a name they show or the locale changed.
    pub(crate) fn new(
        summary: &Arc<AccountSummary>,
        view: &LedgerView,
        shown: Showing<'_>,
        i18n: &Localizer,
        earlier: Option<Self>,
    ) -> Self {
        let Showing {
            sort,
            filter,
            today,
            today_divider,
        } = shown;
        let source = Self::source(summary.account().id(), view, shown, i18n);
        let names = Names::of(view);
        let earlier = earlier.filter(|earlier| {
            earlier.source.formats_like(&source) && earlier.names.still_hold_in(&names)
        });
        let cells = cells(summary, CellFormatter::new(view, i18n, &names), earlier);
        let matcher = filter.matcher(today);
        let shows_balance = sort.key == SortKey::Date;
        let mut rows: Vec<RegisterRow> = summary
            .register()
            .iter()
            .zip(&cells)
            .enumerate()
            .map(|(place, (entry, cells))| RegisterRow {
                place,
                balance: shows_balance.then(|| entry.balance()),
                cells: Arc::clone(cells),
            })
            .filter(|row| matcher.shows(row))
            .collect();
        rows.sort_by(|a, b| sort.compare(i18n, a, b));
        let divider = match today_divider {
            TodayDivider::Shown { today } if shows_balance => divider(&rows, sort.direction, today),
            TodayDivider::Shown { .. } | TodayDivider::Hidden => None,
        };
        Self {
            source,
            summary: Arc::clone(summary),
            names,
            cells,
            rows,
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

    /// The row Go to date lands on for `date`: of the rows dated on or after it, the
    /// earliest dated, and the first shown among those that share its date. When no
    /// row is, the latest dated, and the last shown among those. `None` while no row
    /// is shown.
    pub(crate) fn row_on_or_after(&self, date: Date<Iso>) -> Option<&RegisterRow> {
        // `min_by_key` keeps the first of equal rows, and `max_by_key` the last.
        self.rows
            .iter()
            .filter(|row| row.saved_date().is_some_and(|saved| saved >= date))
            .min_by_key(|row| row.saved_date())
            .or_else(|| self.rows.iter().max_by_key(|row| row.saved_date()))
    }

    /// How many transactions the account has, shown or not.
    pub(crate) fn transaction_count(&self) -> usize {
        self.cells.len()
    }

    /// The transactions in `summary`, the model's account as it is now, that were
    /// not in it when the model was built.
    pub(crate) fn added<'a>(
        &'a self,
        summary: &'a AccountSummary,
    ) -> impl Iterator<Item = TransactionId> + 'a {
        let changed = ChangedEntries::between(self.summary.register(), summary.register());
        let ids = |register: Register<'a>, entries: Range<usize>| {
            entries.filter_map(move |entry| Some(register.get(entry)?.transaction().id()))
        };
        // An id outside the changed entries is in both registers, each holding it once.
        let earlier: HashSet<TransactionId> = ids(self.summary.register(), changed.old).collect();
        ids(summary.register(), changed.new).filter(move |id| !earlier.contains(id))
    }

    /// The index of the row the Today divider sits above: the first row dated after
    /// today, or under a descending sort the first row dated today or earlier. `None`
    /// while the divider is hidden, under a sort other than by date, or when no row is
    /// dated after today.
    pub(crate) fn divider(&self) -> Option<usize> {
        self.divider
    }
}

/// The names a row's cells show: each category's path, and each account's name.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Names {
    category_paths: HashMap<CategoryId, String>,
    account_names: HashMap<AccountId, String>,
}

impl Names {
    fn of(view: &LedgerView) -> Self {
        Self {
            category_paths: view
                .categories()
                .iter()
                .map(|category| {
                    let path = category_text(view, category.id())
                        .expect("a category in the snapshot has a path in it");
                    (category.id(), path)
                })
                .collect(),
            account_names: view
                .accounts()
                .iter()
                .map(|summary| (summary.account().id(), summary.account().name().to_owned()))
                .collect(),
        }
    }

    /// Whether every name in `self` is the same in `later`, so the cells that show
    /// them still hold. A category or account only `later` has is in no earlier cell.
    fn still_hold_in(&self, later: &Self) -> bool {
        self.category_paths
            .iter()
            .all(|(id, path)| later.category_paths.get(id) == Some(path))
            && self
                .account_names
                .iter()
                .all(|(id, name)| later.account_names.get(id) == Some(name))
    }
}

/// The cells of every transaction in `summary`, in register order. Those of a
/// transaction unchanged since `earlier` was built are `earlier`'s; `formatter`
/// formats the rest.
fn cells(
    summary: &AccountSummary,
    mut formatter: CellFormatter<'_>,
    earlier: Option<RegisterModel>,
) -> Vec<Arc<RowCells>> {
    let register = summary.register();
    let Some(earlier) = earlier else {
        return register
            .iter()
            .map(|entry| Arc::new(formatter.cells(entry.transaction())))
            .collect();
    };
    let earlier_register = earlier.summary.register();
    let ChangedEntries { old, new } = ChangedEntries::between(earlier_register, register);
    let mut cells = earlier.cells;
    let suffix = cells.split_off(old.end);
    // A transaction that moved within the changed entries, such as one whose date
    // changed, keeps its cells too.
    let mut moved: HashMap<*const Transaction, Arc<RowCells>> = old
        .clone()
        .filter_map(|entry| earlier_register.get(entry))
        .map(|entry| ptr::from_ref(entry.transaction()))
        .zip(cells.split_off(old.start))
        .collect();
    cells.extend(new.filter_map(|entry| register.get(entry)).map(|entry| {
        let transaction = entry.transaction();
        moved
            .remove(&ptr::from_ref(transaction))
            .unwrap_or_else(|| Arc::new(formatter.cells(transaction)))
    }));
    cells.extend(suffix);
    cells
}

/// The entries that may differ between two versions of one account's register: those
/// left between the run of transactions both start with and the run both end with.
/// Transactions are told apart by identity. The caller holds the older register's
/// account, which keeps its transactions alive, so no newer one has an older one's
/// address.
struct ChangedEntries {
    /// The entries left in the older register.
    old: Range<usize>,
    /// The entries left in the newer register.
    new: Range<usize>,
}

impl ChangedEntries {
    fn between(old: Register<'_>, new: Register<'_>) -> Self {
        let same = |a: Option<RegisterEntry<'_>>, b: Option<RegisterEntry<'_>>| {
            a.zip(b)
                .is_some_and(|(a, b)| ptr::eq(a.transaction(), b.transaction()))
        };
        let shorter = old.len().min(new.len());
        let start = (0..shorter)
            .take_while(|&entry| same(old.get(entry), new.get(entry)))
            .count();
        let end = (1..=shorter - start)
            .take_while(|&back| same(old.get(old.len() - back), new.get(new.len() - back)))
            .count();
        Self {
            old: start..old.len() - end,
            new: start..new.len() - end,
        }
    }
}

/// The index of the row the Today divider sits above in `rows`, sorted by date in
/// `direction`, unless no row is dated after `today`.
fn divider(rows: &[RegisterRow], direction: SortDirection, today: Date<Iso>) -> Option<usize> {
    let scheduled = |row: &RegisterRow| row.saved_date().is_some_and(|date| date > today);
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

/// Formats rows' cells, building each category's path and finding each transfer's
/// other account once.
struct CellFormatter<'a> {
    i18n: &'a Localizer,
    names: &'a Names,
    other_accounts: OtherAccounts<'a>,
    split: String,
    opening_balance: String,
}

impl<'a> CellFormatter<'a> {
    /// Formats with `i18n`, naming categories by `names`, `view`'s, and finding
    /// transfers' other sides in `view`.
    fn new(view: &'a LedgerView, i18n: &'a Localizer, names: &'a Names) -> Self {
        Self {
            i18n,
            names,
            other_accounts: OtherAccounts {
                view,
                holders: None,
            },
            split: fl!(i18n, "register-split"),
            opening_balance: fl!(i18n, "register-opening-balance"),
        }
    }

    /// The cells of `transaction`'s row.
    fn cells(&mut self, transaction: &Transaction) -> RowCells {
        let (kind, payee, category) = match transaction.allocation() {
            Allocation::OpeningBalance => (
                RowKind::OpeningBalance,
                self.opening_balance.clone(),
                String::new(),
            ),
            Allocation::Category(id) => (
                RowKind::Category(*id),
                transaction.payee().to_owned(),
                self.path(*id).to_owned(),
            ),
            Allocation::Split(_) => (
                RowKind::Split,
                transaction.payee().to_owned(),
                self.split.clone(),
            ),
            Allocation::Transfer(other_side) => {
                let holder = self.other_accounts.holding(*other_side);
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
        let split_lines = match transaction.allocation() {
            Allocation::Split(lines) => lines.as_slice(),
            Allocation::OpeningBalance | Allocation::Category(_) | Allocation::Transfer(_) => &[],
        };
        RowCells {
            id: RowId::Transaction(transaction.id()),
            kind,
            saved_date: Some(transaction.date()),
            date: self.i18n.format_date(transaction.date()),
            number: transaction.number().unwrap_or_default().to_owned(),
            payee,
            category,
            memo: transaction.memo().to_owned(),
            currency: transaction.amount().currency(),
            amount: Some(RowAmount::new(self.i18n, transaction.amount())),
            flagged: transaction.is_flagged(),
            cleared_status: transaction.cleared_status(),
            lines: self.other_accounts.line_rows(split_lines),
            split_targets: self.split_targets(split_lines),
        }
    }

    /// The path from the top of the tree to category `id`: `Food & Dining:Groceries`.
    fn path(&self, id: CategoryId) -> &str {
        self.names
            .category_paths
            .get(&id)
            .expect("a transaction's categories are in its snapshot")
    }

    /// What the Category field shows for each of `lines`, a split's: a category's
    /// path, or `[Name]` for the account holding a line's other side.
    fn split_targets(&mut self, lines: &[SplitLine]) -> Vec<String> {
        lines
            .iter()
            .map(|line| match line.target() {
                SplitTarget::Category(category) => self.path(category).to_owned(),
                SplitTarget::Transfer(other_side) => {
                    transfer_text(self.other_accounts.holding(other_side).name)
                }
            })
            .collect()
    }
}

/// The accounts holding transfers' other sides, gathered the first time one is asked
/// for while a model is built.
struct OtherAccounts<'a> {
    view: &'a LedgerView,
    /// Each transaction's account, by the transaction's id. Every transaction is in it,
    /// so a file whose transfer points at a transaction of another kind still shows.
    holders: Option<HashMap<TransactionId, (&'a Account, &'a Transaction)>>,
}

/// The account holding a transaction.
#[derive(Debug, Clone, Copy)]
struct Holder<'a> {
    id: AccountId,
    name: &'a str,
}

impl<'a> OtherAccounts<'a> {
    /// `lines`, a split's, as its row holds them.
    fn line_rows(&mut self, lines: &[SplitLine]) -> Vec<SplitLineRow> {
        lines
            .iter()
            .map(|line| {
                let (target, other_side) = match line.target() {
                    SplitTarget::Category(category) => (CategoryTarget::Category(category), None),
                    SplitTarget::Transfer(other_side) => (
                        CategoryTarget::Transfer(self.holding(other_side).id),
                        Some(other_side),
                    ),
                };
                SplitLineRow::new(target, other_side, line.memo(), line.amount())
            })
            .collect()
    }

    /// The account holding transaction `other_side`.
    fn holding(&mut self, other_side: TransactionId) -> Holder<'a> {
        let view = self.view;
        let (account, _) = self
            .holders
            .get_or_insert_with(|| transactions_by_id(view))
            .get(&other_side)
            .expect("a transfer's other side is in its snapshot");
        Holder {
            id: account.id(),
            name: account.name(),
        }
    }
}

/// Account `id` in `view`, shared with the snapshot, which a [`RegisterModel`] holds on
/// to. `None` once it has been deleted.
pub(crate) fn shared_summary(view: &LedgerView, id: AccountId) -> Option<&Arc<AccountSummary>> {
    view.accounts()
        .iter()
        .find(|summary| summary.account().id() == id)
}

/// Every transaction of `view`, with the account holding it, by the transaction's id.
pub(crate) fn transactions_by_id(
    view: &LedgerView,
) -> HashMap<TransactionId, (&Account, &Transaction)> {
    view.accounts()
        .iter()
        .flat_map(|summary| {
            let account = summary.account();
            summary.register().iter().map(move |entry| {
                let transaction = entry.transaction();
                (transaction.id(), (account, transaction))
            })
        })
        .collect()
}

/// What a Category field shows for `target` in `view`: a category's path, or `[Name]`
/// for an account. `None` once it has been deleted.
pub(crate) fn target_text(view: &LedgerView, target: CategoryTarget) -> Option<String> {
    match target {
        CategoryTarget::Category(id) => category_text(view, id),
        CategoryTarget::Transfer(account) => view
            .account(account)
            .map(|summary| transfer_text(summary.account().name())),
    }
}

/// The name of `category_type`, as the New Category Dialog and the Category List show
/// it.
pub(crate) fn type_label(i18n: &Localizer, category_type: CategoryType) -> String {
    match category_type {
        CategoryType::Income => fl!(i18n, "category-type-income"),
        CategoryType::Expense => fl!(i18n, "category-type-expense"),
    }
}

/// The path from the top of the tree to category `id` of `view`, as the Register shows
/// it: `Food & Dining:Groceries`. `None` once the category has been deleted.
pub(crate) fn category_text(view: &LedgerView, id: CategoryId) -> Option<String> {
    Some(view.category_path(id)?.join(CATEGORY_SEPARATOR))
}

#[cfg(test)]
mod tests {
    use balanced_core::{DomainCommand, Ledger, NewTransaction, TransactionEdit};
    use icu_locale::{langid, locale};
    use rust_decimal_macros::dec;

    use super::*;

    /// `amount` US dollars.
    fn usd(amount: rust_decimal::Decimal) -> Money {
        Money::new(
            amount,
            CurrencyCode::new("USD").expect("USD has minor units"),
        )
        .expect("cents fit USD")
    }

    /// The day the stub's Registers are drawn on.
    fn today() -> Date<Iso> {
        Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")
    }

    /// Everyday Checking in `view`.
    fn everyday_checking(view: &LedgerView) -> &Arc<AccountSummary> {
        view.accounts()
            .iter()
            .find(|summary| summary.account().name() == "Everyday Checking")
            .expect("the stub has Everyday Checking")
    }

    /// Everyday Checking's Register in `view`, sorted by date and narrowed by
    /// `filter`, formatted by `i18n`, replacing `earlier`.
    fn register(
        view: &LedgerView,
        filter: &RegisterFilter,
        i18n: &Localizer,
        earlier: Option<RegisterModel>,
    ) -> RegisterModel {
        let shown = Showing {
            sort: RegisterSort::default(),
            filter,
            today: today(),
            today_divider: TodayDivider::Shown { today: today() },
        };
        RegisterModel::new(everyday_checking(view), view, shown, i18n, earlier)
    }

    /// Whether `row` shares its cells with the row showing the same transaction in
    /// `model`.
    fn reused(model: &RegisterModel, row: &RegisterRow) -> bool {
        model
            .rows()
            .iter()
            .any(|earlier| earlier.id() == row.id() && Arc::ptr_eq(&earlier.cells, &row.cells))
    }

    #[test]
    fn after_a_command_only_the_changed_transactions_row_is_formatted_again() {
        let mut ledger = Ledger::stub();
        let i18n = Localizer::for_tests();
        let filter = RegisterFilter::default();
        let before = register(&ledger.snapshot(), &filter, &i18n, None);
        let flagged = before.rows()[2].transaction().expect("a transaction's row");

        ledger
            .apply(DomainCommand::SetFlagged {
                transaction: flagged,
                flagged: true,
            })
            .expect("the flag changes");
        let after = register(&ledger.snapshot(), &filter, &i18n, Some(before.clone()));

        let formatted: Vec<_> = after
            .rows()
            .iter()
            .filter(|row| !reused(&before, row))
            .map(|row| (row.transaction(), row.is_flagged()))
            .collect();
        assert_eq!(formatted, [(Some(flagged), true)]);
    }

    #[test]
    fn a_new_amount_formats_only_its_row_again_and_moves_every_later_balance() {
        let mut ledger = Ledger::stub();
        let i18n = Localizer::for_tests();
        let filter = RegisterFilter::default();
        let before = register(&ledger.snapshot(), &filter, &i18n, None);
        let edited = &before.rows()[1];
        let raised = edited
            .amount()
            .and_then(|amount| amount.checked_add(usd(dec!(100))).ok())
            .expect("the row has an amount with room for 100 more");

        ledger
            .apply(DomainCommand::EditTransaction {
                transaction: edited.transaction().expect("a transaction's row"),
                edit: TransactionEdit::Amount(raised),
            })
            .expect("the amount changes");
        let after = register(&ledger.snapshot(), &filter, &i18n, Some(before.clone()));

        let formatted: Vec<_> = after
            .rows()
            .iter()
            .filter(|row| !reused(&before, row))
            .map(RegisterRow::transaction)
            .collect();
        assert_eq!(formatted, [edited.transaction()]);
        let raised_by_100 = |row: (&RegisterRow, &RegisterRow)| {
            row.0
                .balance()
                .and_then(|old| old.checked_add(usd(dec!(100))).ok())
                == row.1.balance()
        };
        assert!(
            before.rows()[1..]
                .iter()
                .zip(&after.rows()[1..])
                .all(raised_by_100)
        );
    }

    #[test]
    fn a_new_date_formats_only_the_row_it_moves_again() {
        let mut ledger = Ledger::stub();
        let i18n = Localizer::for_tests();
        let filter = RegisterFilter::default();
        let before = register(&ledger.snapshot(), &filter, &i18n, None);
        let moved = before.rows()[1].transaction().expect("a transaction's row");

        ledger
            .apply(DomainCommand::EditTransaction {
                transaction: moved,
                edit: TransactionEdit::Date(today()),
            })
            .expect("the date changes");
        let after = register(&ledger.snapshot(), &filter, &i18n, Some(before.clone()));

        let formatted: Vec<_> = after
            .rows()
            .iter()
            .filter(|row| !reused(&before, row))
            .map(RegisterRow::transaction)
            .collect();
        assert_eq!(formatted, [Some(moved)]);
    }

    #[test]
    fn a_new_category_formats_no_row_again() {
        let mut ledger = Ledger::stub();
        let i18n = Localizer::for_tests();
        let filter = RegisterFilter::default();
        let before = register(&ledger.snapshot(), &filter, &i18n, None);

        ledger
            .apply(DomainCommand::CreateCategory {
                name: "Pets".to_owned(),
                parent: None,
                category_type: CategoryType::Expense,
            })
            .expect("the category is created");
        let after = register(&ledger.snapshot(), &filter, &i18n, Some(before.clone()));

        assert!(after.rows().iter().all(|row| reused(&before, row)));
    }

    #[test]
    fn a_new_sort_formats_no_row_again() {
        let view = Ledger::stub().snapshot();
        let i18n = Localizer::for_tests();
        let filter = RegisterFilter::default();
        let before = register(&view, &filter, &i18n, None);
        let by_payee = Showing {
            sort: RegisterSort::default().clicked(SortKey::Payee),
            filter: &filter,
            today: today(),
            today_divider: TodayDivider::Hidden,
        };

        let after = RegisterModel::new(
            everyday_checking(&view),
            &view,
            by_payee,
            &i18n,
            Some(before.clone()),
        );

        assert!(after.rows().iter().all(|row| reused(&before, row)));
    }

    #[test]
    fn a_posted_transaction_is_added() {
        let mut ledger = Ledger::stub();
        let view = ledger.snapshot();
        let model = register(
            &view,
            &RegisterFilter::default(),
            &Localizer::for_tests(),
            None,
        );
        let groceries = model.rows()[3].category_id().expect("a row in a category");
        let posted = NewTransaction::new(today(), "Corner Shop", groceries, usd(dec!(-5)));

        let applied = ledger
            .apply(DomainCommand::PostTransaction {
                account: model.account(),
                transaction: posted,
            })
            .expect("the transaction posts");

        let &DomainCommand::DeleteTransaction { transaction: id } = applied.inverse() else {
            panic!("a post is undone by deleting what it posted");
        };
        let view = ledger.snapshot();
        let added: Vec<_> = model.added(everyday_checking(&view)).collect();
        assert_eq!(added, [id]);
    }

    /// Applies `command` to `ledger`, then checks that Everyday Checking's Register
    /// rebuilt from the one before shows what one built from scratch does.
    fn rebuilds_as_new(ledger: &mut Ledger, command: DomainCommand) {
        let i18n = Localizer::for_tests();
        let filter = RegisterFilter::default();
        let before = register(&ledger.snapshot(), &filter, &i18n, None);

        ledger.apply(command).expect("the command applies");
        let view = ledger.snapshot();
        let rebuilt = register(&view, &filter, &i18n, Some(before));

        assert_eq!(rebuilt.rows(), register(&view, &filter, &i18n, None).rows());
    }

    #[test]
    fn a_renamed_category_shows_in_every_row_in_it() {
        let mut ledger = Ledger::stub();
        let view = ledger.snapshot();
        let category = register(
            &view,
            &RegisterFilter::default(),
            &Localizer::for_tests(),
            None,
        )
        .rows()
        .iter()
        .find_map(RegisterRow::category_id)
        .expect("a row in a category");

        rebuilds_as_new(
            &mut ledger,
            DomainCommand::RenameCategory {
                category,
                name: "Renamed".to_owned(),
            },
        );
    }

    #[test]
    fn a_renamed_account_shows_in_every_row_transferring_to_it() {
        let mut ledger = Ledger::stub();
        let view = ledger.snapshot();
        let fund = view
            .accounts()
            .iter()
            .find(|summary| summary.account().name() == "Emergency Fund")
            .expect("the stub has Emergency Fund")
            .account()
            .id();
        let top_up = NewTransaction::transfer(today(), "Top-up", fund, usd(dec!(-10)));
        ledger
            .apply(DomainCommand::PostTransaction {
                account: everyday_checking(&view).account().id(),
                transaction: top_up,
            })
            .expect("the transfer posts");

        rebuilds_as_new(
            &mut ledger,
            DomainCommand::RenameAccount {
                account: fund,
                name: "Renamed".to_owned(),
            },
        );
    }

    /// Checks that Everyday Checking's Register, rebuilt for `i18n` from one built
    /// for the test locale, shows what one built from scratch does.
    fn formats_again_for(i18n: &Localizer) {
        let view = Ledger::stub().snapshot();
        let filter = RegisterFilter::default();
        let before = register(&view, &filter, &Localizer::for_tests(), None);

        let rebuilt = register(&view, &filter, i18n, Some(before));

        assert_eq!(rebuilt.rows(), register(&view, &filter, i18n, None).rows());
    }

    #[test]
    fn a_new_formatting_locale_formats_every_row_again() {
        formats_again_for(
            &Localizer::for_tests_in(&langid!("en-US"), &locale!("de-DE"))
                .expect("the formatting data loads"),
        );
    }

    #[test]
    fn a_new_text_language_names_every_row_again() {
        formats_again_for(
            &Localizer::for_tests_in(&Localizer::PSEUDO_LANGUAGE, &locale!("en-US"))
                .expect("the pseudo-locale's messages load"),
        );
    }

    /// Everyday Checking's Register, sorted by `sort`, on a day before every row in
    /// the stub.
    fn all_after_today(sort: RegisterSort) -> RegisterModel {
        let view = Ledger::stub().snapshot();
        let summary = view
            .accounts()
            .iter()
            .find(|summary| summary.account().name() == "Everyday Checking")
            .expect("the stub has Everyday Checking");
        let today = Date::try_new_iso(2000, 1, 1).expect("1 January 2000 is a date");
        let shown = Showing {
            sort,
            filter: &RegisterFilter::default(),
            today,
            today_divider: TodayDivider::Shown { today },
        };
        RegisterModel::new(summary, &view, shown, &Localizer::for_tests(), None)
    }

    #[test]
    fn when_every_row_is_after_today_the_divider_sits_above_every_row() {
        let model = all_after_today(RegisterSort::default());

        assert_eq!(model.divider(), Some(0));
    }

    #[test]
    fn under_a_descending_date_sort_when_every_row_is_after_today_the_divider_sits_below_every_row()
    {
        // The Register opens sorted by date, so clicking Date reverses it.
        let descending = RegisterSort::default().clicked(SortKey::Date);

        let model = all_after_today(descending);

        assert_eq!(model.divider(), Some(model.rows().len()));
    }
}
