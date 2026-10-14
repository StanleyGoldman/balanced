//! Locale formatting through ICU4X, in the formatting locale.

use std::cell::RefCell;

use fluent_bundle::FluentValue;
use icu_calendar::types::Weekday;
use icu_calendar::week::WeekInformation;
use icu_calendar::{Date, Gregorian, Iso};
use icu_datetime::FixedCalendarDateTimeFormatter;
use icu_datetime::fieldsets::YMD;
use icu_datetime::options::YearStyle;
use icu_decimal::DecimalFormatter;
use icu_decimal::input::{Decimal, FloatPrecision};
use icu_decimal::options::{DecimalFormatterOptions, GroupingStrategy};
use icu_locale::Locale;

use crate::I18nError;
use crate::money::NumberSymbols;

/// The ICU4X formatters for one formatting locale.
#[derive(Debug)]
pub(crate) struct Formatters {
    decimal: DecimalFormatter,
    ungrouped: DecimalFormatter,
    date: FixedCalendarDateTimeFormatter<Gregorian, YMD>,
    week: WeekInformation,
    symbols: NumberSymbols,
}

impl Formatters {
    pub(crate) fn new(locale: &Locale) -> Result<Self, I18nError> {
        let decimal = DecimalFormatter::try_new(locale.into(), DecimalFormatterOptions::default())?;
        Ok(Self {
            symbols: NumberSymbols::new(&decimal),
            decimal,
            ungrouped: DecimalFormatter::try_new(locale.into(), {
                let mut options = DecimalFormatterOptions::default();
                options.grouping_strategy = Some(GroupingStrategy::Never);
                options
            })?,
            date: FixedCalendarDateTimeFormatter::try_new(
                locale.into(),
                YMD::short().with_year_style(YearStyle::Full),
            )?,
            week: WeekInformation::try_new(locale.into())?,
        })
    }

    pub(crate) fn number(&self, value: i64) -> String {
        self.decimal.format(&Decimal::from(value)).to_string()
    }

    pub(crate) fn date(&self, date: Date<Iso>) -> String {
        self.date.format(&date.to_calendar(Gregorian)).to_string()
    }

    pub(crate) fn first_weekday(&self) -> Weekday {
        self.week.first_weekday
    }

    pub(crate) fn symbols(&self) -> &NumberSymbols {
        &self.symbols
    }

    /// Makes Fluent numbers looked up on this thread format with these formatters.
    pub(crate) fn install_for_fluent(&self) {
        let numbers = FluentNumbers {
            grouped: self.decimal.clone(),
            ungrouped: self.ungrouped.clone(),
        };
        FLUENT_NUMBERS.with_borrow_mut(|cell| *cell = Some(numbers));
    }
}

thread_local! {
    /// The number formatter Fluent's formatter hook uses on this thread, set by
    /// [`Formatters::install_for_fluent`]. Each thread has its own, so lookups on
    /// one thread never see another thread's formatting locale.
    static FLUENT_NUMBERS: RefCell<Option<FluentNumbers>> = const { RefCell::new(None) };
}

/// The formatters `NUMBER()` options choose between.
#[derive(Debug)]
struct FluentNumbers {
    grouped: DecimalFormatter,
    /// For `NUMBER($n, useGrouping: "false")`.
    ungrouped: DecimalFormatter,
}

/// Fluent's formatter hook: writes numbers with this thread's formatter, honouring
/// the message's `useGrouping`, `minimumFractionDigits` and `maximumFractionDigits`.
/// Currency and percent styles are not supported: money reaches messages as text.
///
/// Returns `None`, so Fluent writes the value itself, for anything other than a
/// number, and on a thread no `Localizer` set up.
pub(crate) fn format_fluent_value<M>(value: &FluentValue<'_>, _memoizer: &M) -> Option<String> {
    let FluentValue::Number(number) = value else {
        return None;
    };
    let mut decimal = Decimal::try_from_f64(number.value, FloatPrecision::RoundTrip).ok()?;
    if let Some(digits) = number.options.maximum_fraction_digits {
        decimal.round(-i16::try_from(digits).ok()?);
    }
    if let Some(digits) = number.options.minimum_fraction_digits {
        decimal.pad_end(-i16::try_from(digits).ok()?);
    }
    FLUENT_NUMBERS.with_borrow(|numbers| {
        let numbers = numbers.as_ref()?;
        let formatter = if number.options.use_grouping {
            &numbers.grouped
        } else {
            &numbers.ungrouped
        };
        Some(formatter.format(&decimal).to_string())
    })
}

#[cfg(test)]
mod tests {
    use icu_locale::locale;

    use super::*;

    fn formatters(locale: &Locale) -> Formatters {
        Formatters::new(locale).unwrap()
    }

    #[test]
    fn groups_numbers_by_locale() {
        assert_eq!(
            formatters(&locale!("en-US")).number(-1_234_567),
            "-1,234,567"
        );
        assert_eq!(
            formatters(&locale!("de-DE")).number(-1_234_567),
            "-1.234.567"
        );
    }

    #[test]
    fn writes_dates_in_locale_order_with_full_year() {
        let date = Date::try_new_iso(2026, 10, 4).unwrap();

        assert_eq!(formatters(&locale!("en-US")).date(date), "10/4/2026");
        assert_eq!(formatters(&locale!("de-DE")).date(date), "04.10.2026");
    }

    #[test]
    fn first_weekday_follows_region_and_extension() {
        assert_eq!(
            formatters(&locale!("en-US")).first_weekday(),
            Weekday::Sunday
        );
        assert_eq!(
            formatters(&locale!("de-DE")).first_weekday(),
            Weekday::Monday
        );
        assert_eq!(
            formatters(&locale!("en-US-u-fw-wed")).first_weekday(),
            Weekday::Wednesday
        );
    }

    #[test]
    fn fluent_numbers_use_the_thread_locale_and_keep_fraction_digits() {
        formatters(&locale!("de-DE")).install_for_fluent();
        let mut number = fluent_bundle::types::FluentNumber::from(1234.5);
        number.options.minimum_fraction_digits = Some(2);

        let out = format_fluent_value(&FluentValue::Number(number), &());

        assert_eq!(out.as_deref(), Some("1.234,50"));
    }

    #[test]
    fn fluent_numbers_honour_grouping_and_maximum_fraction_digits() {
        formatters(&locale!("de-DE")).install_for_fluent();
        let mut number = fluent_bundle::types::FluentNumber::from(1234.567);
        number.options.use_grouping = false;
        number.options.maximum_fraction_digits = Some(1);

        let out = format_fluent_value(&FluentValue::Number(number), &());

        assert_eq!(out.as_deref(), Some("1234,6"));
    }

    #[test]
    fn fluent_strings_are_left_to_fluent() {
        formatters(&locale!("de-DE")).install_for_fluent();

        let out = format_fluent_value(&FluentValue::from("1234"), &());

        assert_eq!(out, None);
    }

    #[test]
    fn fluent_numbers_on_a_thread_without_a_locale_are_left_to_fluent() {
        let out = std::thread::Builder::new()
            .name("no-locale".to_owned())
            .spawn(|| format_fluent_value(&FluentValue::from(1234), &()))
            .unwrap()
            .join()
            .unwrap();

        assert_eq!(out, None);
    }
}
