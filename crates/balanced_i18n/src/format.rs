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
use icu_decimal::options::DecimalFormatterOptions;
use icu_locale::Locale;

use crate::I18nError;

/// The ICU4X formatters for one formatting locale.
#[derive(Debug)]
pub(crate) struct Formatters {
    decimal: DecimalFormatter,
    date: FixedCalendarDateTimeFormatter<Gregorian, YMD>,
    week: WeekInformation,
}

impl Formatters {
    pub(crate) fn new(locale: &Locale) -> Result<Self, I18nError> {
        Ok(Self {
            decimal: DecimalFormatter::try_new(locale.into(), DecimalFormatterOptions::default())?,
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
}

thread_local! {
    /// The number formatter Fluent's formatter hook uses on this thread.
    ///
    /// The hook is a plain `fn` that sees only the bundle's text language, so the
    /// formatting locale has to reach it through ambient state. Thread-local state
    /// keeps parallel tests apart, and egui looks text up on the UI thread.
    static FLUENT_NUMBERS: RefCell<Option<DecimalFormatter>> = const { RefCell::new(None) };
}

/// Makes Fluent numbers looked up on this thread format in `locale`.
pub(crate) fn set_fluent_number_locale(locale: &Locale) -> Result<(), I18nError> {
    let formatter = DecimalFormatter::try_new(locale.into(), DecimalFormatterOptions::default())?;
    FLUENT_NUMBERS.with_borrow_mut(|cell| *cell = Some(formatter));
    Ok(())
}

/// Fluent's formatter hook: writes numbers with this thread's formatter, keeping
/// the message's minimum fraction digits.
///
/// Returns `None`, so Fluent writes the value itself, for anything other than a
/// number, and on a thread no `Localizer` set up.
pub(crate) fn format_fluent_value<M>(value: &FluentValue<'_>, _memoizer: &M) -> Option<String> {
    let FluentValue::Number(number) = value else {
        return None;
    };
    let mut decimal = Decimal::try_from_f64(number.value, FloatPrecision::RoundTrip).ok()?;
    if let Some(digits) = number.options.minimum_fraction_digits {
        decimal.pad_end(-i16::try_from(digits).ok()?);
    }
    FLUENT_NUMBERS.with_borrow(|formatter| {
        formatter
            .as_ref()
            .map(|formatter| formatter.format(&decimal).to_string())
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
        set_fluent_number_locale(&locale!("de-DE")).unwrap();
        let mut number = fluent_bundle::types::FluentNumber::from(1234.5);
        number.options.minimum_fraction_digits = Some(2);

        let out = format_fluent_value(&FluentValue::Number(number), &());

        assert_eq!(out.as_deref(), Some("1.234,50"));
    }

    #[test]
    fn fluent_strings_are_left_to_fluent() {
        set_fluent_number_locale(&locale!("de-DE")).unwrap();

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
