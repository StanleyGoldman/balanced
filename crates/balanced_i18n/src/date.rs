// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! How the formatting locale writes a date as numbers, and reading typed dates back.

use icu_calendar::{Date, Iso};

use crate::DateParseError;

/// One part of a date as the user types it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DateSegment {
    Day,
    Month,
    Year,
}

/// How the formatting locale writes a short date: the order of its segments, the
/// separator between them, and whether a one-digit day or month gets a leading zero.
/// `M/D/YYYY` in `en-US`, `DD.MM.YYYY` in `de-DE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateSegments {
    order: [DateSegment; 3],
    separator: String,
    zero_padded: bool,
}

impl DateSegments {
    /// The segments in the order the locale writes them.
    pub fn order(&self) -> [DateSegment; 3] {
        self.order
    }

    /// The text between two segments: `/` in `en-US`, `.` in `de-DE`.
    pub fn separator(&self) -> &str {
        &self.separator
    }

    /// Whether `c` may separate the segments of a typed date: `/`, `.`, `-` or a space,
    /// in any locale.
    pub fn is_separator(c: char) -> bool {
        c.is_whitespace() || SEPARATORS.contains(c)
    }

    /// Whether a one-digit day or month is written with a leading zero.
    pub fn zero_padded(&self) -> bool {
        self.zero_padded
    }

    /// The segments of `formatted`, a date formatted in the same locale, which writes
    /// 2 January 2026 as `formatted_probe`. Falls back to the ISO form, `YYYY-MM-DD`,
    /// when the probe doesn't hold three runs of ASCII digits.
    pub(crate) fn from_probe(formatted_probe: &str) -> Self {
        let runs = digit_runs(formatted_probe);
        let [first, second, third] = runs[..] else {
            return Self::iso();
        };
        let segment = |run: Run<'_>| match run.text.trim_start_matches('0') {
            "2" => Some(DateSegment::Day),
            "1" => Some(DateSegment::Month),
            "2026" => Some(DateSegment::Year),
            _ => None,
        };
        let (Some(a), Some(b), Some(c)) = (segment(first), segment(second), segment(third)) else {
            return Self::iso();
        };
        let order = [a, b, c];
        // The probe's day and month are one digit, so two digits mean padding.
        let zero_padded = runs
            .iter()
            .zip(order)
            .any(|(run, segment)| segment != DateSegment::Year && run.text.len() == 2);
        Self {
            order,
            separator: formatted_probe[first.end..second.start].to_owned(),
            zero_padded,
        }
    }

    /// The ISO form, `YYYY-MM-DD`.
    fn iso() -> Self {
        Self {
            order: ISO_ORDER,
            separator: "-".to_owned(),
            zero_padded: true,
        }
    }
}

/// A run of ASCII digits in a text, and where it starts and ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Run<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

/// Every run of ASCII digits in `text`, in order.
fn digit_runs(text: &str) -> Vec<Run<'_>> {
    let mut runs = Vec::new();
    let mut rest = text.char_indices().peekable();
    while let Some((start, c)) = rest.next() {
        if !c.is_ascii_digit() {
            continue;
        }
        let mut end = start + 1;
        while let Some(&(index, c)) = rest.peek()
            && c.is_ascii_digit()
        {
            end = index + 1;
            rest.next();
        }
        runs.push(Run {
            text: &text[start..end],
            start,
            end,
        });
    }
    runs
}

/// The date [`DateSegments::from_probe`] expects formatted: 2 January 2026, whose day,
/// month and year are all different numbers and the day and month one digit long.
pub(crate) fn probe_date() -> Date<Iso> {
    Date::try_new_iso(2026, 1, 2).expect("2 January 2026 is a date")
}

/// The order of an ISO date's segments: year, month, day.
const ISO_ORDER: [DateSegment; 3] = [DateSegment::Year, DateSegment::Month, DateSegment::Day];

/// How many digits a date typed with no separators has: `YYYYMMDD`.
const DIGITS_ONLY_LENGTH: usize = 8;

/// The separators a typed date may use in any locale, besides spaces.
const SEPARATORS: &str = "/.-";

/// Reads a full date typed or pasted in `segments`' order.
pub(crate) fn parse_date(segments: &DateSegments, text: &str) -> Result<Date<Iso>, DateParseError> {
    if !text
        .chars()
        .all(|c| c.is_ascii_digit() || DateSegments::is_separator(c))
    {
        return Err(DateParseError::Invalid);
    }
    let runs = digit_runs(text);
    if let [run] = runs[..]
        && run.text.len() >= DIGITS_ONLY_LENGTH
    {
        // More than eight digits leave the day too long, which `calendar_date` refuses.
        let digits = run.text;
        return calendar_date(&digits[..4], &digits[4..6], &digits[6..]);
    }
    let [first, second, third] = runs[..] else {
        return Err(match runs.len() {
            0..=2 => DateParseError::Incomplete,
            _ => DateParseError::Invalid,
        });
    };
    let mut day = None;
    let mut month = None;
    let mut year = None;
    // A four-digit first part is a year, so the date is in ISO order.
    let order = if first.text.len() == 4 {
        ISO_ORDER
    } else {
        segments.order
    };
    for (run, segment) in [first, second, third].into_iter().zip(order) {
        match segment {
            DateSegment::Day => day = Some(run.text),
            DateSegment::Month => month = Some(run.text),
            DateSegment::Year => year = Some(run.text),
        }
    }
    let (Some(day), Some(month), Some(year)) = (day, month, year) else {
        return Err(DateParseError::Invalid);
    };
    calendar_date(year, month, day)
}

/// The date with these digits as its year, month and day. The year takes four digits,
/// and the month and day one or two.
fn calendar_date(year: &str, month: &str, day: &str) -> Result<Date<Iso>, DateParseError> {
    if year.len() != 4 || !(1..=2).contains(&month.len()) || !(1..=2).contains(&day.len()) {
        return Err(DateParseError::Invalid);
    }
    let year = year.parse().expect("four ASCII digits are an i32");
    let month = month.parse().expect("two ASCII digits are a u8");
    let day = day.parse().expect("two ASCII digits are a u8");
    Ok(Date::try_new_iso(year, month, day)?)
}

#[cfg(test)]
mod tests {
    use icu_calendar::types::Weekday;
    use icu_locale::{Locale, langid, locale};

    use super::*;
    use crate::Localizer;

    fn localizer(formatting_locale: &Locale) -> Localizer {
        Localizer::new(&langid!("en-US"), formatting_locale).expect(
            "the embedded messages load, and ICU4X's compiled data covers the test's locale",
        )
    }

    fn date(year: i32, month: u8, day: u8) -> Date<Iso> {
        Date::try_new_iso(year, month, day).expect("the test's date is a real calendar date")
    }

    #[test]
    fn en_us_writes_month_day_year_unpadded_with_slashes() {
        let segments = localizer(&locale!("en-US")).date_segments().clone();

        assert_eq!(
            (
                segments.order(),
                segments.separator(),
                segments.zero_padded()
            ),
            (
                [DateSegment::Month, DateSegment::Day, DateSegment::Year],
                "/",
                false
            )
        );
    }

    #[test]
    fn de_de_writes_day_month_year_padded_with_dots() {
        let segments = localizer(&locale!("de-DE")).date_segments().clone();

        assert_eq!(
            (
                segments.order(),
                segments.separator(),
                segments.zero_padded()
            ),
            (
                [DateSegment::Day, DateSegment::Month, DateSegment::Year],
                ".",
                true
            )
        );
    }

    #[test]
    fn a_locale_writing_other_digits_falls_back_to_iso() {
        let segments = localizer(&locale!("bn-BD")).date_segments().clone();

        assert_eq!(
            (
                segments.order(),
                segments.separator(),
                segments.zero_padded()
            ),
            (
                [DateSegment::Year, DateSegment::Month, DateSegment::Day],
                "-",
                true
            )
        );
    }

    #[test]
    fn japanese_writes_the_year_first() {
        let segments = localizer(&locale!("ja-JP")).date_segments().clone();

        assert_eq!(
            (segments.order(), segments.separator()),
            (
                [DateSegment::Year, DateSegment::Month, DateSegment::Day],
                "/"
            )
        );
    }

    #[test]
    fn reads_a_date_in_the_locale_order() {
        assert_eq!(
            localizer(&locale!("en-US")).parse_date("10/5/2026"),
            Ok(date(2026, 10, 5))
        );
        assert_eq!(
            localizer(&locale!("de-DE")).parse_date("05.10.2026"),
            Ok(date(2026, 10, 5))
        );
    }

    #[test]
    fn a_date_missing_a_part_is_incomplete() {
        let en_us = localizer(&locale!("en-US"));
        let de_de = localizer(&locale!("de-DE"));

        assert_eq!(en_us.parse_date("5"), Err(DateParseError::Incomplete));
        assert_eq!(en_us.parse_date("3/5"), Err(DateParseError::Incomplete));
        assert_eq!(de_de.parse_date("5.3."), Err(DateParseError::Incomplete));
        assert_eq!(en_us.parse_date("  "), Err(DateParseError::Incomplete));
    }

    #[test]
    fn a_date_the_calendar_lacks_or_the_locale_doesnt_write_is_refused() {
        let en_us = localizer(&locale!("en-US"));

        for text in ["2/30/2026", "13/1/2026"] {
            assert!(
                matches!(
                    en_us.parse_date(text),
                    Err(DateParseError::NotOnCalendar(_))
                ),
                "{text:?}"
            );
        }
        for text in [
            "10/5/26",
            "10/5/20260",
            "010/5/2026",
            "10/5/2026x",
            "1/2/3/2026",
        ] {
            assert_eq!(
                en_us.parse_date(text),
                Err(DateParseError::Invalid),
                "{text:?}"
            );
        }
    }

    #[test]
    fn reads_a_date_in_iso_order_in_any_locale() {
        assert_eq!(
            localizer(&locale!("en-US")).parse_date("2026-10-05"),
            Ok(date(2026, 10, 5))
        );
        assert_eq!(
            localizer(&locale!("de-DE")).parse_date("2026-10-05"),
            Ok(date(2026, 10, 5))
        );
    }

    #[test]
    fn reads_eight_digits_as_year_month_day() {
        assert_eq!(
            localizer(&locale!("en-US")).parse_date("20261005"),
            Ok(date(2026, 10, 5))
        );
        assert_eq!(
            localizer(&locale!("de-DE")).parse_date(" 20261005 "),
            Ok(date(2026, 10, 5))
        );
    }

    #[test]
    fn digits_alone_are_a_date_only_when_eight_make_one() {
        let en_us = localizer(&locale!("en-US"));

        assert_eq!(en_us.parse_date("0501"), Err(DateParseError::Incomplete));
        assert!(matches!(
            en_us.parse_date("10052026"),
            Err(DateParseError::NotOnCalendar(_))
        ));
        assert_eq!(en_us.parse_date("202610051"), Err(DateParseError::Invalid));
    }

    #[test]
    fn names_weekdays_and_months_for_a_calendar() {
        let en_us = localizer(&locale!("en-US"));
        let de_de = localizer(&locale!("de-DE"));

        assert_eq!(
            (
                en_us.format_weekday(Weekday::Sunday),
                en_us.format_month_year(date(2026, 10, 5))
            ),
            ("Sun".to_owned(), "October 2026".to_owned())
        );
        assert_eq!(
            (
                de_de.format_weekday(Weekday::Monday),
                de_de.format_month_year(date(2026, 10, 5))
            ),
            ("Mo".to_owned(), "Oktober 2026".to_owned())
        );
    }

    #[test]
    fn slashes_dots_dashes_and_spaces_separate_segments() {
        let separators: Vec<bool> = ['/', '.', '-', ' ', ',', 'x', '5']
            .into_iter()
            .map(DateSegments::is_separator)
            .collect();

        assert_eq!(separators, [true, true, true, true, false, false, false]);
    }
}
