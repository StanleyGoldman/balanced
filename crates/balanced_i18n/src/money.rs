// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Formatting and parsing money amounts in the formatting locale.
//!
//! Formatting is built on `icu_experimental`'s `CurrencyFormatter`, pinned to an exact
//! version because its API breaks with every release. This module is the only code
//! that names it.

use std::num::NonZeroUsize;

use icu_decimal::DecimalFormatter;
use icu_decimal::input::Decimal as FixedDecimal;
use icu_experimental::dimension::currency::CurrencyType;
use icu_experimental::dimension::currency::formatter::CurrencyFormatter;
use icu_experimental::dimension::currency::options::CurrencyFormatterOptions;
use icu_experimental::dimension::provider::currency::essentials::CurrencyEssentialsV1;
use icu_experimental::dimension::provider::currency::fractions::{
    CurrencyFractions, CurrencyFractionsV1, FractionInfo, Rounding,
};
use icu_experimental::dimension::provider::currency::no_currency::CurrencyPatternsNoCurrencyV1;
use icu_experimental::dimension::provider::currency::symbols::CurrencySymbolsV1;
use icu_locale::Locale;
use icu_provider::prelude::*;
use rust_decimal::Decimal;
use zerovec::ZeroMap;

use crate::{AmountParseError, I18nError};

/// A currency as amount formatting needs it: its code and its decimal places.
///
/// `balanced_ui` builds one from `balanced_core`'s `CurrencyCode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Currency {
    code: CurrencyType,
    decimal_places: u8,
}

impl Currency {
    /// The currency with this three-letter uppercase code, whose amounts carry
    /// `decimal_places` decimal places.
    pub fn new(code: &str, decimal_places: u32) -> Result<Self, I18nError> {
        let invalid = || I18nError::Currency {
            code: code.to_owned(),
        };
        if code.len() != 3 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(invalid());
        }
        Ok(Self {
            code: CurrencyType::try_from_str(code).map_err(|_| invalid())?,
            decimal_places: u8::try_from(decimal_places).map_err(|_| invalid())?,
        })
    }

    pub(crate) fn decimal_places(self) -> u32 {
        u32::from(self.decimal_places)
    }
}

/// Whether a formatted amount shows its currency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmountStyle {
    /// With the currency's symbol where the locale puts it: `$1,234.50`,
    /// `1.234,50 $`. For the Account bar, totals, and anywhere currencies mix.
    Symbol,
    /// The bare number: `1,234.50`. For Register cells.
    Bare,
}

/// Formats `amount` in `locale`, at the currency's decimal places, with the locale's
/// standard minus sign for negative amounts.
///
/// `amount` should already be at the currency's decimal places, as `Money` amounts
/// are. An amount with more significant decimal places shows all of them: display
/// never rounds.
pub(crate) fn format_amount(
    locale: &Locale,
    amount: Decimal,
    currency: Currency,
    style: AmountStyle,
) -> String {
    let significant_places =
        u8::try_from(amount.normalize().scale()).expect("a Decimal's scale is at most 28");
    let provider = IsoDigits {
        decimal_places: currency.decimal_places.max(significant_places),
    };
    let prefs = locale.into();
    let options = CurrencyFormatterOptions::default();
    let formatter = match style {
        AmountStyle::Symbol => {
            CurrencyFormatter::try_new_symbol_unstable(&provider, prefs, currency.code, options)
        }
        AmountStyle::Bare => CurrencyFormatter::try_new_no_currency_unstable(
            &provider,
            prefs,
            currency.code,
            options,
        ),
    }
    .expect("ICU4X's compiled currency data falls back to the root locale for every locale");

    // The mantissa is an integer, so a negative zero amount loses its sign here.
    let mut value = FixedDecimal::from(amount.mantissa());
    value.multiply_pow10(-i16::try_from(amount.scale()).expect("a Decimal's scale is at most 28"));
    formatter.format_fixed_decimal(&value).to_string()
}

/// ICU4X's compiled data, except that every currency's decimal places come from
/// [`Currency`] (or from the amount, when it has more) rather than from CLDR.
///
/// CLDR's display digits differ from ISO 4217's minor units for some currencies
/// (IQD has 3 in ISO and 0 in CLDR), and `CurrencyFormatter` rounds to CLDR's. A
/// posted amount must show every decimal place it has.
struct IsoDigits {
    decimal_places: u8,
}

impl DataProvider<CurrencyFractionsV1> for IsoDigits {
    fn load(&self, _req: DataRequest<'_>) -> Result<DataResponse<CurrencyFractionsV1>, DataError> {
        Ok(DataResponse {
            metadata: DataResponseMetadata::default(),
            payload: DataPayload::from_owned(CurrencyFractions {
                fractions: ZeroMap::default(),
                default: FractionInfo {
                    digits: self.decimal_places,
                    rounding: Rounding::R1,
                    cash_digits: None,
                    cash_rounding: None,
                },
            }),
        })
    }
}

/// Passes each marker through to the crate whose compiled data holds it.
macro_rules! compiled_data {
    ($($marker:ty => $baked:path),* $(,)?) => {$(
        impl DataProvider<$marker> for IsoDigits {
            fn load(&self, req: DataRequest<'_>) -> Result<DataResponse<$marker>, DataError> {
                $baked.load(req)
            }
        }
    )*};
}

compiled_data! {
    CurrencyEssentialsV1 => icu_experimental::provider::Baked,
    CurrencySymbolsV1 => icu_experimental::provider::Baked,
    CurrencyPatternsNoCurrencyV1 => icu_experimental::provider::Baked,
    icu_decimal::provider::DecimalSymbolsV1 => icu_decimal::provider::Baked,
    icu_decimal::provider::DecimalDigitsV1 => icu_decimal::provider::Baked,
}

/// The symbols a locale writes numbers with, read back from its own formatter so
/// that the parser accepts exactly what the formatter writes.
#[derive(Debug, Clone)]
pub(crate) struct NumberSymbols {
    /// The locale's digits, zero to nine.
    digits: [char; 10],
    decimal: String,
    group: String,
    /// The size of the group next to the decimal mark.
    primary_group: usize,
    /// The size of every group further left.
    secondary_group: usize,
    minus_prefix: String,
    minus_suffix: String,
}

impl NumberSymbols {
    pub(crate) fn new(formatter: &DecimalFormatter) -> Self {
        let digits = std::array::from_fn(|n| {
            let n = i64::try_from(n).expect("digits are below ten");
            let text = formatter.format(&FixedDecimal::from(n)).to_string();
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(digit), None) => digit,
                _ => unreachable!("a single digit formats as one character: {text:?}"),
            }
        });

        // -1234567.5 shows every symbol: minus, group separators and the decimal mark.
        let mut probe = FixedDecimal::from(-12_345_675);
        probe.multiply_pow10(-1);
        let text = formatter.format(&probe).to_string();
        let mut runs: Vec<(bool, String)> = Vec::new();
        for c in text.chars() {
            let is_digit = digits.contains(&c);
            match runs.last_mut() {
                Some((run_is_digit, run)) if *run_is_digit == is_digit => run.push(c),
                _ => runs.push((is_digit, c.to_string())),
            }
        }
        let minus_prefix = take_separator_run(&mut runs, 0);
        let last = runs.len().saturating_sub(1);
        let minus_suffix = take_separator_run(&mut runs, last);
        // What is left alternates digits and separators, and ends `<decimal> 5`.
        let separators: Vec<&str> = runs
            .iter()
            .filter(|(is_digit, _)| !is_digit)
            .map(|(_, run)| run.as_str())
            .collect();
        let groups: Vec<usize> = runs
            .iter()
            .filter(|(is_digit, _)| *is_digit)
            .map(|(_, run)| run.chars().count())
            .collect();
        let (decimal, group_separators) = separators
            .split_last()
            .expect("-1234567.5 has a decimal mark");
        // The probe has digits after its decimal mark, so `groups` is not empty.
        let integer_groups = &groups[..groups.len() - 1];
        Self {
            digits,
            decimal: (*decimal).to_owned(),
            group: group_separators
                .first()
                .copied()
                .unwrap_or_default()
                .to_owned(),
            primary_group: integer_groups.last().copied().unwrap_or(3),
            secondary_group: integer_groups
                .len()
                .checked_sub(2)
                .and_then(|i| integer_groups.get(i))
                .copied()
                .unwrap_or(3),
            minus_prefix,
            minus_suffix,
        }
    }

    fn digit(&self, c: char) -> Option<char> {
        if c.is_ascii_digit() {
            return Some(c);
        }
        let n = self.digits.iter().position(|&d| d == c)?;
        char::from_digit(u32::try_from(n).ok()?, 10)
    }

    /// The decimal mark: `.` in `en-US`, `,` in `de-DE`.
    pub(crate) fn decimal(&self) -> &str {
        &self.decimal
    }

    /// The group separator: `,` in `en-US`, `.` in `de-DE`.
    pub(crate) fn group(&self) -> &str {
        &self.group
    }

    /// Whether `c` can be part of a number written with these symbols: a digit, the
    /// decimal mark, the group separator or a minus sign, the locale's own or a typed
    /// `-` or `−`.
    pub(crate) fn writes(&self, c: char) -> bool {
        self.digit(c).is_some()
            || [
                &self.decimal,
                &self.group,
                &self.minus_prefix,
                &self.minus_suffix,
            ]
            .iter()
            .any(|symbol| symbol.contains(c))
            || matches!(c, '-' | MINUS_SIGN)
    }

    /// The group separator, if `rest` starts with one. A typed space stands in for
    /// a no-break space.
    fn group_len(&self, rest: &str) -> Option<usize> {
        if self.group.is_empty() {
            return None;
        }
        if rest.starts_with(&self.group) {
            return Some(self.group.len());
        }
        let group_is_space = self.group.chars().all(char::is_whitespace);
        (group_is_space && rest.starts_with(' ')).then_some(1)
    }
}

/// Removes and returns run `index` when it is a separator run, as at either end of
/// the probe.
fn take_separator_run(runs: &mut Vec<(bool, String)>, index: usize) -> String {
    match runs.get(index) {
        Some((false, _)) => runs.remove(index).1,
        _ => String::new(),
    }
}

/// U+2212, the typographic minus sign, which users may type for `-`.
pub(crate) const MINUS_SIGN: char = '\u{2212}';
const MINUS_SIGN_STR: &str = "\u{2212}";

/// Parses an amount typed in the locale `symbols` describe, for a currency with
/// `decimal_places` decimal places. See [`crate::Localizer::parse_amount`].
pub(crate) fn parse_amount(
    symbols: &NumberSymbols,
    decimal_places: u32,
    text: &str,
) -> Result<Decimal, AmountParseError> {
    let Digits {
        negative,
        integer,
        fraction,
    } = read_digits(symbols, text)?;
    let places = usize::try_from(decimal_places).expect("decimal places fit in usize");
    let fraction = match fraction.get(places..) {
        Some(excess) if !excess.bytes().all(|b| b == b'0') => {
            return Err(AmountParseError::TooManyDecimalPlaces { decimal_places });
        }
        // `get(places..)` succeeded, so `places` is within the ASCII `fraction`.
        Some(_) => &fraction[..places],
        None => fraction.as_str(),
    };
    let amount = to_decimal(negative, &integer, fraction)?;
    at_scale(amount, decimal_places)
}

/// Parses a number typed in the locale `symbols` describe, as [`parse_amount`] does,
/// but keeping every decimal place it has.
pub(crate) fn parse_number(
    symbols: &NumberSymbols,
    text: &str,
) -> Result<Decimal, AmountParseError> {
    let Digits {
        negative,
        integer,
        fraction,
    } = read_digits(symbols, text)?;
    to_decimal(negative, &integer, &fraction)
}

/// `amount`, which has no more significant decimal places than `decimal_places`,
/// padded to exactly that many. Zero loses any minus sign.
pub(crate) fn at_scale(
    mut amount: Decimal,
    decimal_places: u32,
) -> Result<Decimal, AmountParseError> {
    // Padding never rounds; a short scale means the mantissa ran out of room.
    amount.rescale(decimal_places);
    if amount.scale() != decimal_places {
        return Err(AmountParseError::OutOfRange);
    }
    if amount.is_zero() {
        amount.set_sign_positive(true);
    }
    Ok(amount)
}

/// A typed number's sign and ASCII digits.
struct Digits {
    negative: bool,
    integer: String,
    fraction: String,
}

/// Reads the sign and digits of a number typed in the locale `symbols` describe,
/// ignoring direction marks and a currency symbol or code at either end.
fn read_digits(symbols: &NumberSymbols, text: &str) -> Result<Digits, AmountParseError> {
    let text: String = text.chars().filter(|&c| !is_bidi_mark(c)).collect();
    let text = strip_affixes(&text);
    if text.is_empty() {
        return Err(AmountParseError::Empty);
    }
    let (negative, text) = strip_minus(symbols, text);
    let text = strip_affixes(text);
    let (integer, fraction) = split_number(symbols, text)?;
    Ok(Digits {
        negative,
        integer,
        fraction,
    })
}

/// The number with sign `negative`, ASCII `integer` digits and `fraction` digits.
fn to_decimal(negative: bool, integer: &str, fraction: &str) -> Result<Decimal, AmountParseError> {
    let sign = if negative { "-" } else { "" };
    let integer = if integer.is_empty() { "0" } else { integer };
    let digits = if fraction.is_empty() {
        format!("{sign}{integer}")
    } else {
        format!("{sign}{integer}.{fraction}")
    };
    // `digits` is ASCII digits with at most one `.`, so the only failure is a value
    // with more digits than a `Decimal` holds.
    Decimal::from_str_exact(&digits).map_err(|_| AmountParseError::OutOfRange)
}

/// Splits `text` into its integer digits and fraction digits, as ASCII, checking
/// the group separators sit where the locale puts them.
fn split_number(symbols: &NumberSymbols, text: &str) -> Result<(String, String), AmountParseError> {
    let mut groups = vec![String::new()];
    let mut fraction: Option<String> = None;
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let consumed = if let Some(digit) = symbols.digit(c) {
            match &mut fraction {
                Some(fraction) => fraction.push(digit),
                None => groups
                    .last_mut()
                    .expect("starts with one group")
                    .push(digit),
            }
            c.len_utf8()
        } else if fraction.is_none() && rest.starts_with(&symbols.decimal) {
            fraction = Some(String::new());
            symbols.decimal.len()
        } else if let Some(len) = symbols.group_len(rest).filter(|_| fraction.is_none()) {
            groups.push(String::new());
            len
        } else {
            0
        };
        // Every pass consumes something, so the loop ends even if a separator is
        // empty. Without this, a zero-length match would push groups until memory
        // ran out.
        let consumed = NonZeroUsize::new(consumed).ok_or(AmountParseError::Invalid)?;
        rest = rest
            .get(consumed.get()..)
            .ok_or(AmountParseError::Invalid)?;
    }

    let fraction = fraction.map_or(Ok(String::new()), |fraction| {
        if fraction.is_empty() {
            Err(AmountParseError::Invalid)
        } else {
            Ok(fraction)
        }
    })?;
    if !groups_fit(symbols, &groups) {
        return Err(AmountParseError::Invalid);
    }
    let integer = groups.concat();
    if integer.is_empty() && fraction.is_empty() {
        return Err(AmountParseError::Invalid);
    }
    Ok((integer, fraction))
}

/// Whether grouped integer digits are grouped as the locale groups them: `1.234`
/// is one thousand two hundred and thirty-four in `de-DE`, and `1.23` is a typo.
fn groups_fit(symbols: &NumberSymbols, groups: &[String]) -> bool {
    let len = |group: &String| group.chars().count();
    let Some((last, rest)) = groups.split_last() else {
        return true;
    };
    let Some((first, middle)) = rest.split_first() else {
        return true;
    };
    len(last) == symbols.primary_group
        && middle
            .iter()
            .all(|group| len(group) == symbols.secondary_group)
        && (1..=symbols.secondary_group).contains(&len(first))
}

/// Strips a minus sign from either end: the locale's own, or a typed `-` or `−`.
fn strip_minus<'a>(symbols: &NumberSymbols, text: &'a str) -> (bool, &'a str) {
    const TYPED: [&str; 2] = ["-", MINUS_SIGN_STR];
    let prefixes = std::iter::once(symbols.minus_prefix.as_str()).chain(TYPED);
    for prefix in prefixes.filter(|p| !p.is_empty()) {
        if let Some(rest) = text.strip_prefix(prefix) {
            return (true, rest);
        }
    }
    let suffixes = std::iter::once(symbols.minus_suffix.as_str()).chain(TYPED);
    for suffix in suffixes.filter(|s| !s.is_empty()) {
        if let Some(rest) = text.strip_suffix(suffix) {
            return (true, rest);
        }
    }
    (false, text)
}

/// Strips a currency symbol or code, and spaces, from both ends: `US$ 5`, `5 €`,
/// `5,00 kr.`, `S/ 5`, `B/.5`. The symbol is not checked against the currency.
pub(crate) fn strip_affixes(text: &str) -> &str {
    let mut start = 0;
    let mut previous: Option<char> = None;
    for (i, c) in text.char_indices() {
        if !is_affix(c, previous) {
            break;
        }
        start = i + c.len_utf8();
        previous = Some(c);
    }
    // `start` is the end of a char, so it is a char boundary within `text`.
    let text = &text[start..];

    let mut end = text.len();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (n, &(i, c)) in chars.iter().enumerate().rev() {
        let before = n.checked_sub(1).and_then(|p| chars.get(p)).map(|&(_, c)| c);
        if !is_affix(c, before) {
            break;
        }
        end = i;
    }
    // `end` is the start of a char, or the length, so it is a char boundary.
    &text[..end]
}

/// Whether `c` can be part of a currency symbol or code, given `neighbour`, the
/// character next to it on the symbol's side. A `.` or `/` right after a letter or
/// a `/` belongs to the symbol (`kr.`, `S/`, `B/.`), not to the number.
fn is_affix(c: char, neighbour: Option<char>) -> bool {
    c.is_alphabetic()
        || c.is_whitespace()
        || is_currency_symbol(c)
        || (matches!(c, '.' | '/') && neighbour.is_some_and(|n| n.is_alphabetic() || n == '/'))
}

/// Unicode's currency symbols (general category Sc).
fn is_currency_symbol(c: char) -> bool {
    matches!(c,
        '$' | '\u{A2}'..='\u{A5}' | '\u{58F}' | '\u{60B}' | '\u{7FE}'..='\u{7FF}'
        | '\u{9F2}'..='\u{9F3}' | '\u{9FB}' | '\u{AF1}' | '\u{BF9}' | '\u{E3F}'
        | '\u{17DB}' | '\u{20A0}'..='\u{20C0}' | '\u{A838}' | '\u{FDFC}' | '\u{FE69}'
        | '\u{FF04}' | '\u{FFE0}'..='\u{FFE1}' | '\u{FFE5}'..='\u{FFE6}'
        | '\u{11FDD}'..='\u{11FE0}' | '\u{1E2FF}' | '\u{1ECB0}')
}

/// Invisible direction marks CLDR puts around signs in some locales.
fn is_bidi_mark(c: char) -> bool {
    matches!(
        c,
        '\u{200E}' | '\u{200F}' | '\u{061C}' | '\u{2066}'..='\u{2069}'
    )
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use icu_decimal::options::DecimalFormatterOptions;
    use icu_locale::locale;
    use rust_decimal_macros::dec;

    use super::*;

    fn currency(code: &str, decimal_places: u32) -> Currency {
        Currency::new(code, decimal_places).unwrap()
    }

    fn usd() -> Currency {
        currency("USD", 2)
    }

    fn symbols(locale: &Locale) -> NumberSymbols {
        let formatter =
            DecimalFormatter::try_new(locale.into(), DecimalFormatterOptions::default()).unwrap();
        NumberSymbols::new(&formatter)
    }

    fn parse(
        locale: &Locale,
        decimal_places: u32,
        text: &str,
    ) -> Result<Decimal, AmountParseError> {
        parse_amount(&symbols(locale), decimal_places, text)
    }

    #[test]
    fn rejects_currency_codes_that_are_not_three_uppercase_letters() {
        for code in ["usd", "US", "USDX", "U1D"] {
            let result = Currency::new(code, 2);

            assert!(
                matches!(result, Err(I18nError::Currency { code: ref given }) if given == code),
                "{code:?}: {result:?}"
            );
        }
    }

    /// One line per amount and currency, both styles, for a snapshot.
    fn table(locale: &Locale) -> String {
        let cases = [
            (dec!(1234.50), usd()),
            (dec!(-1234.50), usd()),
            (dec!(0.00), usd()),
            (dec!(1234567.89), currency("EUR", 2)),
            (dec!(1234), currency("JPY", 0)),
            (dec!(1.234), currency("BHD", 3)),
            (dec!(1234.567), currency("IQD", 3)),
            (dec!(1.2345), currency("CLF", 4)),
        ];
        let mut out = String::new();
        for (amount, currency) in cases {
            let symbol = format_amount(locale, amount, currency, AmountStyle::Symbol);
            let bare = format_amount(locale, amount, currency, AmountStyle::Bare);
            let code = currency.code.iso_code();
            writeln!(out, "{amount:>12} {code}  {symbol:?}  {bare:?}").unwrap();
        }
        out
    }

    #[test]
    fn formats_amounts_in_en_us() {
        insta::assert_snapshot!(table(&locale!("en-US")), @r#"
             1234.50 USD  "$1,234.50"  "1,234.50"
            -1234.50 USD  "-$1,234.50"  "-1,234.50"
                0.00 USD  "$0.00"  "0.00"
          1234567.89 EUR  "€1,234,567.89"  "1,234,567.89"
                1234 JPY  "¥1,234"  "1,234"
               1.234 BHD  "BHD\u{a0}1.234"  "1.234"
            1234.567 IQD  "IQD\u{a0}1,234.567"  "1,234.567"
              1.2345 CLF  "CLF\u{a0}1.2345"  "1.2345"
        "#);
    }

    #[test]
    fn formats_amounts_in_de_de() {
        insta::assert_snapshot!(table(&locale!("de-DE")), @r#"
             1234.50 USD  "1.234,50\u{a0}$"  "1.234,50"
            -1234.50 USD  "-1.234,50\u{a0}$"  "-1.234,50"
                0.00 USD  "0,00\u{a0}$"  "0,00"
          1234567.89 EUR  "1.234.567,89\u{a0}€"  "1.234.567,89"
                1234 JPY  "1.234\u{a0}¥"  "1.234"
               1.234 BHD  "1,234\u{a0}BHD"  "1,234"
            1234.567 IQD  "1.234,567\u{a0}IQD"  "1.234,567"
              1.2345 CLF  "1,2345\u{a0}CLF"  "1,2345"
        "#);
    }

    #[test]
    fn the_locale_places_the_symbol_whatever_the_currency() {
        let amount = dec!(1234.50);

        assert_eq!(
            format_amount(&locale!("de-DE"), amount, usd(), AmountStyle::Symbol),
            "1.234,50\u{a0}$"
        );
    }

    #[test]
    fn negative_zero_has_no_minus_sign() {
        let amount = -dec!(0.00);

        assert_eq!(
            format_amount(&locale!("en-US"), amount, usd(), AmountStyle::Bare),
            "0.00"
        );
    }

    #[test]
    fn formatting_pads_to_the_currency_decimal_places() {
        assert_eq!(
            format_amount(&locale!("en-US"), dec!(5), usd(), AmountStyle::Bare),
            "5.00"
        );
    }

    #[test]
    fn formatting_shows_every_decimal_place_the_amount_has() {
        let amount = dec!(1.005);

        assert_eq!(
            format_amount(&locale!("en-US"), amount, usd(), AmountStyle::Bare),
            "1.005"
        );
    }

    #[test]
    fn reads_the_symbols_each_locale_writes() {
        let en = symbols(&locale!("en-US"));
        let de = symbols(&locale!("de-DE"));

        assert_eq!((en.decimal.as_str(), en.group.as_str()), (".", ","));
        assert_eq!((de.decimal.as_str(), de.group.as_str()), (",", "."));
        assert_eq!((en.primary_group, en.secondary_group), (3, 3));
        assert_eq!(
            (en.minus_prefix.as_str(), en.minus_suffix.as_str()),
            ("-", "")
        );
    }

    #[test]
    fn reads_indian_grouping() {
        let hi = symbols(&locale!("en-IN"));

        assert_eq!((hi.primary_group, hi.secondary_group), (3, 2));
    }

    #[test]
    fn parses_by_indian_grouping() {
        assert_eq!(
            parse(&locale!("en-IN"), 2, "12,34,567.89"),
            Ok(dec!(1234567.89))
        );
        assert_eq!(
            parse(&locale!("en-IN"), 2, "1,234,567.89"),
            Err(AmountParseError::Invalid)
        );
    }

    #[test]
    fn parses_by_the_locale_separators() {
        assert_eq!(parse(&locale!("en-US"), 2, "1,234.5"), Ok(dec!(1234.50)));
        assert_eq!(parse(&locale!("de-DE"), 2, "1.234,5"), Ok(dec!(1234.50)));
        assert_eq!(parse(&locale!("de-DE"), 2, "1.234"), Ok(dec!(1234.00)));
        assert_eq!(parse(&locale!("en-US"), 2, "1234"), Ok(dec!(1234.00)));
    }

    #[test]
    fn parsed_amounts_carry_the_currency_decimal_places() {
        assert_eq!(
            parse(&locale!("en-US"), 2, "5").unwrap().to_string(),
            "5.00"
        );
        assert_eq!(parse(&locale!("en-US"), 0, "5").unwrap().to_string(), "5");
        assert_eq!(
            parse(&locale!("en-US"), 3, ".5").unwrap().to_string(),
            "0.500"
        );
    }

    #[test]
    fn rejects_the_other_locale_decimal_mark() {
        assert_eq!(
            parse(&locale!("en-US"), 2, "1,5"),
            Err(AmountParseError::Invalid)
        );
        assert_eq!(
            parse(&locale!("de-DE"), 2, "1.23"),
            Err(AmountParseError::Invalid)
        );
    }

    #[test]
    fn rejects_misplaced_group_separators() {
        for text in ["1,23,456", ",123", "1,,234", "12345,678", "1.234,5"] {
            assert_eq!(
                parse(&locale!("en-US"), 2, text),
                Err(AmountParseError::Invalid),
                "{text:?}"
            );
        }
    }

    #[test]
    fn rejects_excess_decimal_places_without_rounding() {
        assert_eq!(
            parse(&locale!("en-US"), 2, "12.345"),
            Err(AmountParseError::TooManyDecimalPlaces { decimal_places: 2 })
        );
        assert_eq!(
            parse(&locale!("en-US"), 0, "12.5"),
            Err(AmountParseError::TooManyDecimalPlaces { decimal_places: 0 })
        );
    }

    #[test]
    fn accepts_excess_zeros() {
        assert_eq!(parse(&locale!("en-US"), 2, "12.300"), Ok(dec!(12.30)));
    }

    #[test]
    fn accepts_a_leading_or_trailing_minus() {
        for text in ["-5", "5-", "\u{2212}5", "-$5.00", "$-5.00"] {
            assert_eq!(
                parse(&locale!("en-US"), 2, text),
                Ok(dec!(-5.00)),
                "{text:?}"
            );
        }
    }

    #[test]
    fn rejects_accounting_parentheses() {
        assert_eq!(
            parse(&locale!("en-US"), 2, "($5.00)"),
            Err(AmountParseError::Invalid)
        );
    }

    #[test]
    fn ignores_direction_marks() {
        assert_eq!(parse(&locale!("en-US"), 2, "\u{200E}-5"), Ok(dec!(-5.00)));
    }

    #[test]
    fn a_typed_space_is_no_separator_where_the_locale_groups_with_another() {
        assert_eq!(
            parse(&locale!("en-US"), 2, "1 234"),
            Err(AmountParseError::Invalid)
        );
    }

    #[test]
    fn rejects_two_minus_signs() {
        assert_eq!(
            parse(&locale!("en-US"), 2, "-5-"),
            Err(AmountParseError::Invalid)
        );
    }

    #[test]
    fn ignores_a_surrounding_currency_symbol_or_code() {
        let cases = [
            (locale!("en-US"), "$1,234.50"),
            (locale!("en-US"), "USD 1,234.50"),
            (locale!("en-US"), "USD1,234.50"),
            (locale!("en-US"), "1,234.50 EUR"),
            (locale!("en-US"), "US$1,234.50"),
            (locale!("de-DE"), "1.234,50 €"),
            (locale!("da-DK"), "1.234,50 kr."),
        ];
        for (locale, text) in cases {
            assert_eq!(
                parse(&locale, 2, text),
                Ok(dec!(1234.50)),
                "{locale} {text:?}"
            );
        }
    }

    #[test]
    fn ignores_currency_symbols_written_with_a_slash() {
        let cases = [
            (locale!("es-PE"), "S/\u{a0}1,234.50"),
            (locale!("es-PA"), "B/.1,234.50"),
            (locale!("es-PA"), "1,234.50 B/."),
        ];
        for (locale, text) in cases {
            assert_eq!(
                parse(&locale, 2, text),
                Ok(dec!(1234.50)),
                "{locale} {text:?}"
            );
        }
    }

    #[test]
    fn a_typed_space_stands_in_for_a_no_break_space() {
        assert_eq!(parse(&locale!("fr-FR"), 2, "1 234,50"), Ok(dec!(1234.50)));
    }

    #[test]
    fn rejects_text_with_no_amount() {
        for text in ["", "  ", "$", "USD"] {
            assert_eq!(
                parse(&locale!("en-US"), 2, text),
                Err(AmountParseError::Empty),
                "{text:?}"
            );
        }
        for text in ["-", ".", "1a2", "1.", "1.2.3", "1e5", "1 2"] {
            assert_eq!(
                parse(&locale!("en-US"), 2, text),
                Err(AmountParseError::Invalid),
                "{text:?}"
            );
        }
    }

    #[test]
    fn rejects_amounts_out_of_range() {
        let text = "79228162514264337593543950335";

        assert_eq!(
            parse(&locale!("en-US"), 2, text),
            Err(AmountParseError::OutOfRange)
        );
    }

    #[test]
    fn negative_zero_parses_as_zero() {
        let zero = parse(&locale!("en-US"), 2, "-0").unwrap();

        assert!(zero.is_sign_positive());
    }

    /// Amounts at `decimal_places`, from their minor units, so that no fixture rounds.
    fn amounts(decimal_places: u32) -> [Decimal; 4] {
        [0, 5, -123_450, 123_456_789].map(|minor_units| Decimal::new(minor_units, decimal_places))
    }

    /// Formats `amount` in both styles and parses each back.
    fn assert_parses_back(locale: &Locale, amount: Decimal, currency: Currency) {
        let symbols = symbols(locale);
        let places = u32::from(currency.decimal_places);
        for style in [AmountStyle::Symbol, AmountStyle::Bare] {
            let text = format_amount(locale, amount, currency, style);

            let parsed = parse_amount(&symbols, places, &text);

            assert_eq!(parsed, Ok(amount), "{locale} {style:?} {text:?}");
        }
    }

    #[test]
    fn formatted_amounts_parse_back() {
        let currencies = [
            usd(),
            currency("JPY", 0),
            currency("BHD", 3),
            currency("IQD", 3),
        ];
        // `en-XA` formats in `de-DE`; `Localizer`'s tests round-trip it.
        for locale in [locale!("en-US"), locale!("de-DE")] {
            for currency in currencies {
                for amount in amounts(u32::from(currency.decimal_places)) {
                    assert_parses_back(&locale, amount, currency);
                }
            }
        }
    }

    #[test]
    fn amounts_in_each_locale_own_currency_parse_back() {
        let cases = [
            (locale!("fr-FR"), currency("EUR", 2)),
            (locale!("da-DK"), currency("DKK", 2)),
            (locale!("de-CH"), currency("CHF", 2)),
            (locale!("en-IN"), currency("INR", 2)),
            (locale!("es-PE"), currency("PEN", 2)),
            (locale!("es-PA"), currency("PAB", 2)),
            (locale!("pt-BR"), currency("BRL", 2)),
            (locale!("ar-EG"), currency("EGP", 2)),
            (locale!("fa-IR"), currency("IRR", 0)),
            (locale!("he-IL"), currency("ILS", 2)),
            (locale!("ja-JP"), currency("JPY", 0)),
            (locale!("ar-KW"), currency("KWD", 3)),
        ];
        for (locale, currency) in cases {
            for amount in amounts(u32::from(currency.decimal_places)) {
                assert_parses_back(&locale, amount, currency);
            }
        }
    }
}
