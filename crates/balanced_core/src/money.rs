// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`Money`]: an exact amount in one currency, and the [`CurrencyCode`] it is in.

use std::fmt;
use std::str::FromStr;

use rust_decimal::{Decimal, RoundingStrategy};

/// A currency, by its code, with the number of decimal places its amounts carry.
///
/// Only ISO 4217 currencies with minor units exist today. ISO data stays inside this
/// type so that non-ISO currencies can be added without changing its users.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CurrencyCode {
    /// Three uppercase ASCII letters.
    code: [u8; 3],
    decimal_places: u8,
}

impl CurrencyCode {
    /// The currency with this ISO 4217 code, such as `USD`. The code must be
    /// uppercase, and the currency must have minor units: `XAU` (gold) has none.
    pub fn new(code: &str) -> Result<Self, MoneyError> {
        let unknown = || MoneyError::UnknownCurrency {
            code: code.to_owned(),
        };
        let bytes: [u8; 3] = code.as_bytes().try_into().map_err(|_| unknown())?;
        if !bytes.iter().all(u8::is_ascii_uppercase) {
            return Err(unknown());
        }
        let exponent = iso_currency::Currency::from_code(code)
            .and_then(iso_currency::Currency::exponent)
            .ok_or_else(unknown)?;
        let decimal_places = u8::try_from(exponent)
            .expect("ISO 4217 minor units are between 0 and 4 (SIX List One)");
        Ok(Self {
            code: bytes,
            decimal_places,
        })
    }

    /// The code, such as `USD`.
    pub fn as_str(&self) -> &str {
        str::from_utf8(&self.code).expect("`new` admits only ASCII letters")
    }

    /// The decimal places every amount in this currency carries: 2 for USD, 0 for
    /// JPY, 3 for BHD.
    pub fn decimal_places(self) -> u32 {
        u32::from(self.decimal_places)
    }
}

impl FromStr for CurrencyCode {
    type Err = MoneyError;

    fn from_str(code: &str) -> Result<Self, Self::Err> {
        Self::new(code)
    }
}

impl fmt::Display for CurrencyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How an amount with too many decimal places is rounded to its currency's.
///
/// Each operation that rounds names its policy. Nothing rounds by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundingPolicy {
    /// A half goes to the even neighbour: 0.125 becomes 0.12, 0.135 becomes 0.14.
    BankersEven,
    /// A half goes away from zero: 0.125 becomes 0.13, -0.125 becomes -0.13.
    HalfAwayFromZero,
}

impl RoundingPolicy {
    fn strategy(self) -> RoundingStrategy {
        match self {
            Self::BankersEven => RoundingStrategy::MidpointNearestEven,
            Self::HalfAwayFromZero => RoundingStrategy::MidpointAwayFromZero,
        }
    }
}

/// An exact amount in one currency, always at that currency's decimal places:
/// `5` USD is held as `5.00`.
///
/// Arithmetic is checked: adding amounts in different currencies is an error, not a
/// conversion. Unit prices, quantities and rates are plain `Decimal`s, not `Money`.
///
/// `Money` has no `Display`. Show it through `balanced_i18n`'s amount formatting,
/// which follows the user's formatting locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Money {
    amount: Decimal,
    currency: CurrencyCode,
}

impl Money {
    /// `amount` in `currency`, padded to the currency's decimal places.
    ///
    /// Fails when `amount` has more decimal places than the currency allows, unless
    /// the extra digits are zeros. Use [`Self::rounded`] to round instead.
    pub fn new(amount: Decimal, currency: CurrencyCode) -> Result<Self, MoneyError> {
        if amount.normalize().scale() > currency.decimal_places() {
            return Err(MoneyError::TooManyDecimalPlaces { currency });
        }
        Self::at_currency_scale(amount, currency)
    }

    /// `amount` in `currency`, rounded to the currency's decimal places by `policy`.
    pub fn rounded(
        amount: Decimal,
        currency: CurrencyCode,
        policy: RoundingPolicy,
    ) -> Result<Self, MoneyError> {
        let rounded = amount.round_dp_with_strategy(currency.decimal_places(), policy.strategy());
        Self::at_currency_scale(rounded, currency)
    }

    /// Zero in `currency`.
    pub fn zero(currency: CurrencyCode) -> Self {
        Self::at_currency_scale(Decimal::ZERO, currency)
            .expect("zero fits at every currency's decimal places")
    }

    /// Pads `amount`, which has no more significant decimal places than the
    /// currency allows, to exactly the currency's decimal places.
    fn at_currency_scale(amount: Decimal, currency: CurrencyCode) -> Result<Self, MoneyError> {
        let mut amount = amount.normalize();
        // Padding never rounds. `rescale` stops short of the scale asked for when the
        // mantissa would overflow, so a short scale means the amount is too large.
        amount.rescale(currency.decimal_places());
        if amount.scale() != currency.decimal_places() {
            return Err(MoneyError::Overflow);
        }
        Ok(Self { amount, currency })
    }

    /// The amount, at the currency's decimal places.
    pub fn amount(&self) -> Decimal {
        self.amount
    }

    /// The currency.
    pub fn currency(&self) -> CurrencyCode {
        self.currency
    }

    /// Whether the amount is below zero.
    pub fn is_negative(&self) -> bool {
        self.amount.is_sign_negative() && !self.amount.is_zero()
    }

    /// `self + other`. Fails when the currencies differ or the sum overflows.
    pub fn checked_add(self, other: Self) -> Result<Self, MoneyError> {
        self.same_currency(other)?;
        let sum = self
            .amount
            .checked_add(other.amount)
            .ok_or(MoneyError::Overflow)?;
        Self::at_currency_scale(sum, self.currency)
    }

    /// `self - other`. Fails when the currencies differ or the difference overflows.
    pub fn checked_sub(self, other: Self) -> Result<Self, MoneyError> {
        self.same_currency(other)?;
        let difference = self
            .amount
            .checked_sub(other.amount)
            .ok_or(MoneyError::Overflow)?;
        Self::at_currency_scale(difference, self.currency)
    }

    fn same_currency(self, other: Self) -> Result<(), MoneyError> {
        if self.currency == other.currency {
            Ok(())
        } else {
            Err(MoneyError::CurrencyMismatch {
                left: self.currency,
                right: other.currency,
            })
        }
    }
}

impl std::ops::Neg for Money {
    type Output = Self;

    /// The amount with its sign flipped. Zero stays positive zero.
    fn neg(self) -> Self {
        let mut amount = -self.amount;
        if amount.is_zero() {
            amount.set_sign_positive(true);
        }
        Self {
            amount,
            currency: self.currency,
        }
    }
}

/// A failure to build or combine [`Money`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MoneyError {
    /// The code names no ISO 4217 currency with minor units.
    #[error("unknown currency {code:?}")]
    UnknownCurrency {
        /// The code as given.
        code: String,
    },
    /// The amount has more decimal places than its currency allows.
    #[error("{currency} amounts have {} decimal places", currency.decimal_places())]
    TooManyDecimalPlaces {
        /// The currency whose decimal places were exceeded.
        currency: CurrencyCode,
    },
    /// The two amounts are in different currencies.
    #[error("cannot combine {left} and {right} amounts")]
    CurrencyMismatch {
        /// The currency of the left-hand amount.
        left: CurrencyCode,
        /// The currency of the right-hand amount.
        right: CurrencyCode,
    },
    /// The result is too large to hold at the currency's decimal places.
    #[error("amount out of range")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use std::hash::BuildHasher as _;

    use rust_decimal_macros::dec;

    use super::*;

    fn currency(code: &str) -> CurrencyCode {
        CurrencyCode::new(code).expect("the test names an ISO 4217 currency with minor units")
    }

    fn money(amount: Decimal, code: &str) -> Money {
        Money::new(amount, currency(code))
            .expect("the test's amount has no more decimal places than its currency")
    }

    /// Spot-checks `iso_currency`, which takes its data from Wikipedia, against the
    /// ISO 4217 maintenance agency's SIX List One (published 2026-09-17).
    #[test]
    fn decimal_places_match_six_list_one() {
        let places = |code| currency(code).decimal_places();

        assert_eq!(places("USD"), 2);
        assert_eq!(places("JPY"), 0);
        assert_eq!(places("BHD"), 3);
        assert_eq!(places("CLF"), 4);
    }

    #[test]
    fn rejects_codes_that_are_not_iso_currencies_with_minor_units() {
        for code in ["usd", "US", "USDX", "ABC", "XAU", ""] {
            assert_eq!(
                CurrencyCode::new(code),
                Err(MoneyError::UnknownCurrency {
                    code: code.to_owned()
                }),
                "{code:?}"
            );
        }
    }

    #[test]
    fn currency_code_round_trips_through_text() {
        let usd: CurrencyCode = "USD".parse().unwrap();

        assert_eq!(usd.as_str(), "USD");
        assert_eq!(usd.to_string(), "USD");
    }

    #[test]
    fn pads_to_the_currency_decimal_places() {
        assert_eq!(money(dec!(5), "USD").amount().to_string(), "5.00");
        assert_eq!(money(dec!(5), "JPY").amount().to_string(), "5");
        assert_eq!(money(dec!(5.1), "BHD").amount().to_string(), "5.100");
    }

    #[test]
    fn accepts_excess_trailing_zeros() {
        assert_eq!(money(dec!(5.1000), "USD").amount().to_string(), "5.10");
    }

    #[test]
    fn rejects_more_decimal_places_than_the_currency_allows() {
        let usd = currency("USD");

        let err = Money::new(dec!(12.345), usd).unwrap_err();

        assert_eq!(err, MoneyError::TooManyDecimalPlaces { currency: usd });
        assert_eq!(err.to_string(), "USD amounts have 2 decimal places");
    }

    #[test]
    fn rounds_only_by_the_named_policy() {
        let usd = currency("USD");
        let round = |amount, policy| Money::rounded(amount, usd, policy).unwrap().amount();

        assert_eq!(round(dec!(0.125), RoundingPolicy::BankersEven), dec!(0.12));
        assert_eq!(round(dec!(0.135), RoundingPolicy::BankersEven), dec!(0.14));
        assert_eq!(
            round(dec!(0.125), RoundingPolicy::HalfAwayFromZero),
            dec!(0.13)
        );
        assert_eq!(
            round(dec!(-0.125), RoundingPolicy::HalfAwayFromZero),
            dec!(-0.13)
        );
    }

    #[test]
    fn rounded_amounts_are_padded() {
        let rounded = Money::rounded(dec!(7), currency("USD"), RoundingPolicy::BankersEven);

        assert_eq!(rounded.unwrap().amount().to_string(), "7.00");
    }

    #[test]
    fn rejects_amounts_too_large_for_the_decimal_places() {
        assert_eq!(
            Money::new(Decimal::MAX, currency("USD")),
            Err(MoneyError::Overflow)
        );
    }

    #[test]
    fn rounding_an_amount_too_large_for_the_decimal_places_is_an_error() {
        let rounded = Money::rounded(Decimal::MAX, currency("USD"), RoundingPolicy::BankersEven);

        assert_eq!(rounded, Err(MoneyError::Overflow));
    }

    #[test]
    fn negated_zero_is_the_same_value_as_zero() {
        let zero = Money::zero(currency("USD"));
        let hasher = std::hash::RandomState::new();

        let negated = -zero;

        assert_eq!(negated, zero);
        assert_eq!(hasher.hash_one(negated), hasher.hash_one(zero));
        assert_eq!(negated.amount().to_string(), "0.00");
    }

    #[test]
    fn zero_is_at_the_currency_decimal_places() {
        assert_eq!(Money::zero(currency("BHD")).amount().to_string(), "0.000");
    }

    #[test]
    fn adds_and_subtracts_in_one_currency() {
        let a = money(dec!(10.50), "USD");
        let b = money(dec!(0.75), "USD");

        assert_eq!(a.checked_add(b), Ok(money(dec!(11.25), "USD")));
        assert_eq!(a.checked_sub(b), Ok(money(dec!(9.75), "USD")));
    }

    #[test]
    fn subtracting_to_zero_keeps_the_decimal_places() {
        let a = money(dec!(1), "USD");

        assert_eq!(a.checked_sub(a).unwrap().amount().to_string(), "0.00");
    }

    #[test]
    fn mixing_currencies_is_an_error() {
        let usd = money(dec!(1), "USD");
        let eur = money(dec!(1), "EUR");
        let mismatch = Err(MoneyError::CurrencyMismatch {
            left: usd.currency(),
            right: eur.currency(),
        });

        assert_eq!(usd.checked_add(eur), mismatch);
        assert_eq!(usd.checked_sub(eur), mismatch);
    }

    #[test]
    fn overflow_is_an_error() {
        let big = Money::new(Decimal::MAX, currency("JPY")).unwrap();
        let one = money(dec!(1), "JPY");

        assert_eq!(big.checked_add(one), Err(MoneyError::Overflow));
        assert_eq!((-big).checked_sub(one), Err(MoneyError::Overflow));
    }

    #[test]
    fn negation_flips_the_sign() {
        let amount = money(dec!(2.50), "USD");

        assert_eq!((-amount).amount(), dec!(-2.50));
        assert!((-amount).is_negative());
        assert!(!amount.is_negative());
        assert!(!(-Money::zero(amount.currency())).is_negative());
    }
}
