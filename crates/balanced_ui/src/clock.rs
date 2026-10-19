// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`Clock`]: where today's date comes from. It is the one place that reads the time,
//! so tests can fix the date.

use icu_calendar::{Date, Iso};

/// Today's date, as the App reads it for the date field's `t` shortcut, its calendar
/// and its format message.
#[derive(Debug, Clone, Default)]
pub struct Clock {
    source: Source,
}

#[derive(Debug, Clone, Default)]
enum Source {
    /// The system's clock, in the system's time zone.
    #[default]
    System,
    /// The same date on every read, for tests.
    #[cfg(any(test, feature = "test-support"))]
    Fixed(Date<Iso>),
}

impl Clock {
    /// A clock stopped on `today`, so tests run the same on any day.
    #[cfg(any(test, feature = "test-support"))]
    pub fn fixed(today: Date<Iso>) -> Self {
        Self {
            source: Source::Fixed(today),
        }
    }

    /// Today's date where the user is.
    pub(crate) fn today(&self) -> Date<Iso> {
        match self.source {
            Source::System => {
                #[expect(
                    clippy::disallowed_methods,
                    reason = "the clock is the one place that reads the time"
                )]
                let now = jiff::Zoned::now();
                to_iso(now.date())
            }
            #[cfg(any(test, feature = "test-support"))]
            Source::Fixed(today) => today,
        }
    }
}

/// `date` as the ICU4X date the rest of Balanced uses.
pub(crate) fn to_iso(date: jiff::civil::Date) -> Date<Iso> {
    Date::try_new_iso(
        i32::from(date.year()),
        u8::try_from(date.month()).expect("a month is 1 to 12"),
        u8::try_from(date.day()).expect("a day is 1 to 31"),
    )
    .expect("every jiff date is an ISO date")
}

/// `date` as a jiff date, for date arithmetic, or `None` beyond jiff's years, -9999
/// to 9999.
pub(crate) fn to_civil(date: Date<Iso>) -> Option<jiff::civil::Date> {
    jiff::civil::Date::new(
        i16::try_from(date.year().extended_year()).ok()?,
        i8::try_from(date.month().ordinal).ok()?,
        i8::try_from(date.day_of_month().0).ok()?,
    )
    .ok()
}
