//! [`RegisterFilter`]: which of an account's transactions the filter bar lets the
//! Register show.

use std::collections::BTreeSet;

use balanced_core::TransactionId;
use balanced_i18n::Localizer;
use eframe::egui::{self, Button, ComboBox, Response, TextEdit};
use icu_calendar::{Date, Iso};
use jiff::ToSpan as _;

use crate::clock::{to_civil, to_iso};
use crate::i18n::fl;
use crate::register_model::{AmountSide, RegisterModel, RegisterRow};
use crate::theme::Theme;

/// The dates the Register shows, relative to today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DateRange {
    #[default]
    All,
    /// Today and the 29 days before it.
    Last30Days,
    /// Today and the days back to the day after the same date a year ago.
    Last12Months,
    /// The calendar year today is in.
    ThisYear,
    /// The calendar year before it.
    LastYear,
}

impl DateRange {
    const ALL: [Self; 5] = [
        Self::All,
        Self::Last30Days,
        Self::Last12Months,
        Self::ThisYear,
        Self::LastYear,
    ];

    /// The first and last dates of the range as of `today`, or `None` for every date,
    /// or when today is beyond the years dates can be worked out in.
    fn bounds(self, today: Date<Iso>) -> Option<DateBounds> {
        let today = to_civil(today)?;
        let (first, last) = match self {
            Self::All => return None,
            Self::Last30Days => (today.checked_sub(29.days()).ok()?, today),
            Self::Last12Months => (
                today
                    .checked_sub(1.year())
                    .and_then(|date| date.checked_add(1.day()))
                    .ok()?,
                today,
            ),
            Self::ThisYear => (today.first_of_year(), today.last_of_year()),
            Self::LastYear => {
                let last_year = today.checked_sub(1.year()).ok()?;
                (last_year.first_of_year(), last_year.last_of_year())
            }
        };
        Some(DateBounds {
            first: to_iso(first),
            last: to_iso(last),
        })
    }

    fn label(self, i18n: &Localizer) -> String {
        match self {
            Self::All => fl!(i18n, "register-filter-dates-all"),
            Self::Last30Days => fl!(i18n, "register-filter-dates-last-30-days"),
            Self::Last12Months => fl!(i18n, "register-filter-dates-last-12-months"),
            Self::ThisYear => fl!(i18n, "register-filter-dates-this-year"),
            Self::LastYear => fl!(i18n, "register-filter-dates-last-year"),
        }
    }
}

/// The first and last dates of a [`DateRange`], both in it.
#[derive(Debug, Clone, Copy)]
struct DateBounds {
    first: Date<Iso>,
    last: Date<Iso>,
}

impl DateBounds {
    fn contains(self, date: Date<Iso>) -> bool {
        (self.first.to_rata_die()..=self.last.to_rata_die()).contains(&date.to_rata_die())
    }
}

/// The kind of transaction the Register shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum TransactionType {
    #[default]
    All,
    /// Money out: negative amounts.
    Payments,
    /// Money in: zero or positive amounts.
    Deposits,
    Flagged,
}

impl TransactionType {
    const ALL: [Self; 4] = [Self::All, Self::Payments, Self::Deposits, Self::Flagged];

    fn shows(self, row: &RegisterRow) -> bool {
        let side = row.amount().map(AmountSide::of);
        match self {
            Self::All => true,
            Self::Payments => side == Some(AmountSide::Payment),
            Self::Deposits => side == Some(AmountSide::Deposit),
            Self::Flagged => row.is_flagged(),
        }
    }

    fn label(self, i18n: &Localizer) -> String {
        match self {
            Self::All => fl!(i18n, "register-filter-type-all"),
            Self::Payments => fl!(i18n, "register-filter-type-payments"),
            Self::Deposits => fl!(i18n, "register-filter-type-deposits"),
            Self::Flagged => fl!(i18n, "register-filter-type-flagged"),
        }
    }
}

/// What the filter bar is set to, and the transactions added since it was last
/// changed, which it shows whatever it is set to. The open Register owns it, and
/// opening another account clears it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct RegisterFilter {
    /// The search box's text.
    search: String,
    dates: DateRange,
    kind: TransactionType,
    kept: BTreeSet<TransactionId>,
}

impl RegisterFilter {
    /// Whether any control is set to narrow the rows. Search text of only spaces
    /// narrows nothing.
    pub(crate) fn is_set(&self) -> bool {
        !self.search.trim().is_empty()
            || self.dates != DateRange::All
            || self.kind != TransactionType::All
    }

    /// Keeps `added`, transactions just added, in view until the filter changes.
    pub(crate) fn keep(&mut self, added: impl IntoIterator<Item = TransactionId>) {
        self.kept.extend(added);
    }

    /// The test a row has to pass to be shown, as of `today`.
    pub(crate) fn matcher(&self, today: Date<Iso>) -> Matcher<'_> {
        Matcher {
            search: self.search.trim().to_lowercase(),
            dates: self.dates.bounds(today),
            kind: self.kind,
            kept: &self.kept,
        }
    }
}

/// A [`RegisterFilter`] made ready to test many rows.
pub(crate) struct Matcher<'a> {
    /// The search text in lower case.
    search: String,
    dates: Option<DateBounds>,
    kind: TransactionType,
    kept: &'a BTreeSet<TransactionId>,
}

impl Matcher<'_> {
    /// Whether `row` is shown. Its payee, category and memo are searched, and so are
    /// `split_categories`, the paths of a split transaction's lines' categories.
    pub(crate) fn shows<'s>(
        &self,
        row: &RegisterRow,
        split_categories: impl IntoIterator<Item = &'s str>,
    ) -> bool {
        if row.transaction().is_some_and(|id| self.kept.contains(&id)) {
            return true;
        }
        let found = |text: &str| text.to_lowercase().contains(&self.search);
        let searched = self.search.is_empty()
            || found(row.payee())
            || found(row.category())
            || found(row.memo())
            || split_categories.into_iter().any(found);
        let dated = self
            .dates
            .is_none_or(|bounds| row.saved_date().is_some_and(|date| bounds.contains(date)));
        searched && dated && self.kind.shows(row)
    }
}

impl RegisterFilter {
    /// Draws the filter bar, a row of its own above the Register, and sets the filter
    /// from its controls. With any control set, it counts how many of `model`'s
    /// account's transactions are shown. Returns whether a control changed, which
    /// stops keeping the transactions added under the old settings.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        model: &RegisterModel,
    ) -> bool {
        let before = (self.search.clone(), self.dates, self.kind);
        ui.horizontal(|ui| {
            let search = ui.add(
                TextEdit::singleline(&mut self.search)
                    .hint_text(fl!(i18n, "register-filter-search", "placeholder"))
                    .desired_width(Theme::SEARCH_BOX_WIDTH),
            );
            name_for_screen_readers(&search, fl!(i18n, "register-filter-search"));
            let dates = ComboBox::from_id_salt("register_filter_dates")
                .selected_text(self.dates.label(i18n))
                .show_ui(ui, |ui| {
                    for range in DateRange::ALL {
                        ui.selectable_value(&mut self.dates, range, range.label(i18n));
                    }
                });
            name_for_screen_readers(&dates.response, fl!(i18n, "register-filter-dates"));
            let kind = ComboBox::from_id_salt("register_filter_type")
                .selected_text(self.kind.label(i18n))
                .show_ui(ui, |ui| {
                    for kind in TransactionType::ALL {
                        ui.selectable_value(&mut self.kind, kind, kind.label(i18n));
                    }
                });
            name_for_screen_readers(&kind.response, fl!(i18n, "register-filter-type"));
            let reset = Button::new(fl!(i18n, "register-filter-reset"));
            if ui.add_enabled(self.is_set(), reset).clicked() {
                *self = Self::default();
            }
            if self.is_set() {
                ui.weak(fl!(
                    i18n,
                    "register-filter-count",
                    shown = model.rows().len(),
                    total = model.transaction_count()
                ));
            }
        });
        let changed = before != (self.search.clone(), self.dates, self.kind);
        if changed {
            self.kept.clear();
        }
        changed
    }
}

/// Gives `response`'s widget `label` as its name for screen readers, since the filter
/// bar's controls show no label of their own.
fn name_for_screen_readers(response: &Response, label: String) {
    response
        .ctx
        .accesskit_node_builder(response.id, |node| node.set_label(label));
}
