//! Formatting and parsing money amounts in the formatting locale.
//!
//! Formatting is built on `icu_experimental`'s `CurrencyFormatter`, pinned to an exact
//! version because its API breaks with every release. This module is the only code
//! that names it.

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
    /// The currency with this three-letter code, whose amounts carry
    /// `decimal_places` decimal places.
    pub fn new(code: &str, decimal_places: u32) -> Result<Self, I18nError> {
        let invalid = || I18nError::Currency {
            code: code.to_owned(),
        };
        Ok(Self {
            code: CurrencyType::try_from_str(code).map_err(|_| invalid())?,
            decimal_places: u8::try_from(decimal_places).map_err(|_| invalid())?,
        })
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
/// are. Any extra digits are rounded half away from zero, for display only.
pub(crate) fn format_amount(
    locale: &Locale,
    amount: Decimal,
    currency: Currency,
    style: AmountStyle,
) -> String {
    let provider = IsoDigits {
        decimal_places: currency.decimal_places,
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
/// [`Currency`] rather than from CLDR.
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
        let minus_prefix = take_if(&mut runs, 0, false);
        let last = runs.len().saturating_sub(1);
        let minus_suffix = take_if(&mut runs, last, false);
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
fn take_if(runs: &mut Vec<(bool, String)>, index: usize, is_digit: bool) -> String {
    match runs.get(index) {
        Some((run_is_digit, _)) if *run_is_digit == is_digit => runs.remove(index).1,
        _ => String::new(),
    }
}

/// Parses an amount typed in the locale `symbols` describe, for a currency with
/// `decimal_places` decimal places. See [`crate::Localizer::parse_amount`].
pub(crate) fn parse_amount(
    symbols: &NumberSymbols,
    decimal_places: u32,
    text: &str,
) -> Result<Decimal, AmountParseError> {
    let text: String = text.chars().filter(|&c| !is_bidi_mark(c)).collect();
    let text = strip_affixes(&text);
    if text.is_empty() {
        return Err(AmountParseError::Empty);
    }
    let (negative, text) = strip_minus(symbols, text);
    let text = strip_affixes(text);

    let (integer, fraction) = split_number(symbols, text)?;
    let places = usize::try_from(decimal_places).expect("decimal places fit in usize");
    let fraction = match fraction.get(places..) {
        Some(excess) if !excess.bytes().all(|b| b == b'0') => {
            return Err(AmountParseError::TooManyDecimalPlaces { decimal_places });
        }
        Some(_) => &fraction[..places],
        None => fraction.as_str(),
    };

    let sign = if negative { "-" } else { "" };
    let integer = if integer.is_empty() { "0" } else { &integer };
    let digits = if fraction.is_empty() {
        format!("{sign}{integer}")
    } else {
        format!("{sign}{integer}.{fraction}")
    };
    let mut amount = Decimal::from_str_exact(&digits).map_err(|_| AmountParseError::OutOfRange)?;
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
        if consumed == 0 {
            return Err(AmountParseError::Invalid);
        }
        rest = rest.get(consumed..).ok_or(AmountParseError::Invalid)?;
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
    const TYPED: [&str; 2] = ["-", "\u{2212}"];
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
/// `5,00 kr.`. The symbol is not checked against the currency.
fn strip_affixes(text: &str) -> &str {
    let mut start = 0;
    let mut previous: Option<char> = None;
    for (i, c) in text.char_indices() {
        if !is_affix(c, previous) {
            break;
        }
        start = i + c.len_utf8();
        previous = Some(c);
    }
    let text = &text[start..];

    let mut end = text.len();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (n, &(i, c)) in chars.iter().enumerate().rev() {
        let before = n.checked_sub(1).and_then(|p| chars.get(p)).map(|&(_, c)| c);
        // Read right to left, a `.` belongs to the symbol when a letter precedes it.
        if !(is_affix(c, None) || (c == '.' && before.is_some_and(char::is_alphabetic))) {
            break;
        }
        end = i;
    }
    &text[..end]
}

/// Whether `c` can be part of a currency symbol or code, given the character before
/// it. A `.` right after a letter is an abbreviation (`kr.`), not a decimal mark.
fn is_affix(c: char, previous: Option<char>) -> bool {
    c.is_alphabetic()
        || c.is_whitespace()
        || is_currency_symbol(c)
        || (c == '.' && previous.is_some_and(char::is_alphabetic))
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
        for text in ["-5", "5-", "\u{2212}5", "-$5.00", "$-5.00", "($5.00)"] {
            let expected = if text.starts_with('(') {
                Err(AmountParseError::Invalid)
            } else {
                Ok(dec!(-5.00))
            };
            assert_eq!(parse(&locale!("en-US"), 2, text), expected, "{text:?}");
        }
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

    #[test]
    fn formatted_amounts_parse_back() {
        let amounts = [dec!(0.00), dec!(0.05), dec!(-1234.50), dec!(1234567.89)];
        let currencies = [
            usd(),
            currency("JPY", 0),
            currency("BHD", 3),
            currency("IQD", 3),
        ];
        for locale in [locale!("en-US"), locale!("de-DE"), locale!("en-XA")] {
            let symbols = symbols(&locale);
            for currency in currencies {
                let places = u32::from(currency.decimal_places);
                for amount in amounts {
                    let mut amount = amount;
                    amount.rescale(places);
                    for style in [AmountStyle::Symbol, AmountStyle::Bare] {
                        let text = format_amount(&locale, amount, currency, style);

                        let parsed = parse_amount(&symbols, places, &text);

                        assert_eq!(parsed, Ok(amount), "{locale} {style:?} {text:?}");
                    }
                }
            }
        }
    }
}
