// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`AmountReader`]: reads what the user types in an amount field, a plain amount or
//! arithmetic over amounts, in the formatting locale the field was opened in.

use rust_decimal::{Decimal, RoundingStrategy};

use crate::AmountParseError;
use crate::money::{
    MINUS_SIGN, NumberSymbols, at_scale, parse_amount, parse_number, strip_affixes,
};

/// Reads amounts typed in one formatting locale for one currency. It keeps that
/// locale's marks after the [`crate::Localizer`]'s formatting locale changes, so
/// unsaved typed text is never reread under different marks.
#[derive(Debug, Clone)]
pub struct AmountReader {
    symbols: NumberSymbols,
    decimal_places: u32,
    /// The characters of the currency's symbol, as the locale writes it, that are
    /// not letters and not part of a number.
    currency_symbol: String,
}

/// An amount read from typed text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Calculation {
    amount: Decimal,
    source: Source,
}

/// What a [`Calculation`]'s amount came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// A plain amount, which is never rounded.
    Plain,
    /// Arithmetic, whose result may have been rounded.
    Arithmetic { rounded: bool },
}

impl Calculation {
    /// The amount, at exactly the currency's decimal places.
    pub fn amount(&self) -> Decimal {
        self.amount
    }

    /// Whether arithmetic gave more decimal places than the currency has, so the
    /// amount was rounded half away from zero.
    pub fn rounded(&self) -> bool {
        matches!(self.source, Source::Arithmetic { rounded: true })
    }

    /// Whether the text was arithmetic rather than a plain amount.
    pub fn is_arithmetic(&self) -> bool {
        match self.source {
            Source::Plain => false,
            Source::Arithmetic { .. } => true,
        }
    }
}

impl AmountReader {
    /// A reader for numbers written with `symbols`, at `decimal_places`, where
    /// `formatted` is an amount formatted with the currency's symbol.
    pub(crate) fn new(symbols: NumberSymbols, decimal_places: u32, formatted: &str) -> Self {
        let currency_symbol = formatted
            .chars()
            .filter(|&c| !(c.is_alphabetic() || c.is_whitespace() || symbols.writes(c)))
            .collect();
        Self {
            symbols,
            decimal_places,
            currency_symbol,
        }
    }

    /// The decimal mark amounts are typed with: `.` in `en-US`, `,` in `de-DE`.
    pub fn decimal_mark(&self) -> &str {
        self.symbols.decimal()
    }

    /// The group separator amounts are typed with: `,` in `en-US`, `.` in `de-DE`.
    pub fn group_separator(&self) -> &str {
        self.symbols.group()
    }

    /// Whether `c` can be typed in an amount field: a digit, the locale's decimal
    /// mark or group separator, a space, a minus sign, the currency's symbol or an
    /// arithmetic operator. Letters never can, even in a symbol written with them.
    pub fn accepts(&self, c: char) -> bool {
        c == ' '
            || self.symbols.writes(c)
            || self.currency_symbol.contains(c)
            || Operator::from_char(c).is_some()
    }

    /// Whether `text` ends in an operator, so a sum typed in it waits for its next
    /// amount: `12+` does, `12` and `12+3` don't. Spaces after the operator count
    /// for nothing.
    pub fn awaits_operand(text: &str) -> bool {
        text.trim_end()
            .chars()
            .next_back()
            .and_then(Operator::from_char)
            .is_some()
    }

    /// Reads `text`: a plain amount, or arithmetic (`+ - * /`) over amounts.
    ///
    /// A plain amount goes through the strict parser, which rejects more decimal
    /// places than the currency has. Arithmetic multiplies and divides before it adds
    /// and subtracts, and its result is rounded half away from zero to the
    /// currency's decimal places.
    pub fn read(&self, text: &str) -> Result<Calculation, AmountParseError> {
        let Some(Expression {
            operands,
            operators,
        }) = Expression::split(text)?
        else {
            return Ok(Calculation {
                amount: parse_amount(&self.symbols, self.decimal_places, text)?,
                source: Source::Plain,
            });
        };
        let mut values = operands
            .into_iter()
            .map(|operand| parse_number(&self.symbols, operand));
        let first = values.next().expect("an expression has an operand")?;
        let mut sum = Decimal::ZERO;
        let mut sum_operator = Additive::Add;
        let mut term = first;
        for (operator, value) in operators.into_iter().zip(values) {
            let value = value?;
            match operator {
                Operator::Multiply => {
                    term = term
                        .checked_mul(value)
                        .ok_or(AmountParseError::OutOfRange)?;
                }
                Operator::Divide => {
                    if value.is_zero() {
                        return Err(AmountParseError::DivisionByZero);
                    }
                    term = term
                        .checked_div(value)
                        .ok_or(AmountParseError::OutOfRange)?;
                }
                Operator::Additive(additive) => {
                    sum = sum_operator.combine(sum, term)?;
                    sum_operator = additive;
                    term = value;
                }
            }
        }
        let exact = sum_operator.combine(sum, term)?;
        let amount = exact
            .round_dp_with_strategy(self.decimal_places, RoundingStrategy::MidpointAwayFromZero);
        Ok(Calculation {
            source: Source::Arithmetic {
                rounded: amount != exact,
            },
            amount: at_scale(amount, self.decimal_places)?,
        })
    }
}

/// An arithmetic operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operator {
    Additive(Additive),
    Multiply,
    Divide,
}

/// An operator that combines terms: it binds less tightly than `*` and `/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Additive {
    Add,
    Subtract,
}

impl Operator {
    const SUBTRACT: Self = Self::Additive(Additive::Subtract);

    /// The operator a typed character stands for.
    fn from_char(c: char) -> Option<Self> {
        match c {
            '+' => Some(Self::Additive(Additive::Add)),
            '-' | MINUS_SIGN => Some(Self::SUBTRACT),
            '*' => Some(Self::Multiply),
            '/' => Some(Self::Divide),
            _ => None,
        }
    }
}

impl Additive {
    /// `left` combined with `right` by this operator.
    fn combine(self, left: Decimal, right: Decimal) -> Result<Decimal, AmountParseError> {
        match self {
            Self::Add => left.checked_add(right),
            Self::Subtract => left.checked_sub(right),
        }
        .ok_or(AmountParseError::OutOfRange)
    }
}

/// Typed text split into operand texts and the operators between them.
struct Expression<'a> {
    operands: Vec<&'a str>,
    /// One fewer than the operands.
    operators: Vec<Operator>,
}

impl<'a> Expression<'a> {
    /// Splits `text` at its operators, or returns `None` when it has none and so is a
    /// plain amount. A minus with no amount before it is the sign of the operand it
    /// starts, and so is a lone trailing minus (`5-`).
    fn split(text: &'a str) -> Result<Option<Self>, AmountParseError> {
        let mut operands = Vec::new();
        let mut operators = Vec::new();
        let mut start = 0;
        for (i, c) in text.char_indices() {
            let Some(operator) = Operator::from_char(c) else {
                continue;
            };
            // `start` and `i` are char boundaries, `start` never past `i`.
            let operand = &text[start..i];
            if strip_affixes(operand).is_empty() {
                if operator == Operator::SUBTRACT {
                    continue;
                }
                return Err(AmountParseError::Invalid);
            }
            operands.push(operand);
            operators.push(operator);
            start = i + c.len_utf8();
        }
        let last = &text[start..];
        let trailing_minus = operators == [Operator::SUBTRACT] && strip_affixes(last).is_empty();
        if operators.is_empty() || trailing_minus {
            return Ok(None);
        }
        if strip_affixes(last).is_empty() {
            return Err(AmountParseError::Invalid);
        }
        operands.push(last);
        Ok(Some(Self {
            operands,
            operators,
        }))
    }
}

#[cfg(test)]
mod tests {
    use icu_locale::{Locale, locale};
    use rust_decimal_macros::dec;

    use super::*;
    use crate::{Currency, Localizer};

    /// A reader for `code` amounts typed in `locale`.
    fn reader(locale: &Locale, code: &str, decimal_places: u32) -> AmountReader {
        let i18n = Localizer::new(&locale.id, locale).unwrap();
        i18n.amount_reader(Currency::new(code, decimal_places).unwrap())
    }

    fn usd() -> AmountReader {
        reader(&locale!("en-US"), "USD", 2)
    }

    #[test]
    fn a_division_rounds_to_the_currency_decimal_places() {
        let calculation = usd().read("100/3");

        assert_eq!(
            calculation.map(|c| (c.amount(), c.rounded())),
            Ok((dec!(33.33), true))
        );
    }

    fn read(reader: &AmountReader, text: &str) -> Result<(Decimal, bool), AmountParseError> {
        reader.read(text).map(|c| (c.amount(), c.rounded()))
    }

    #[test]
    fn a_sum_ending_in_an_operator_awaits_its_next_amount() {
        let awaiting = ["12+", "12 * ", "-", "3\u{2212}"].map(AmountReader::awaits_operand);

        assert_eq!(awaiting, [true; 4]);
    }

    #[test]
    fn a_sum_ending_in_an_amount_awaits_nothing() {
        let awaiting = ["", "12", "12+3", "-5"].map(AmountReader::awaits_operand);

        assert_eq!(awaiting, [false; 4]);
    }

    #[test]
    fn a_plain_amount_with_excess_decimal_places_is_rejected_not_rounded() {
        assert_eq!(
            read(&usd(), "12.345"),
            Err(AmountParseError::TooManyDecimalPlaces { decimal_places: 2 })
        );
    }

    #[test]
    fn a_plain_amount_is_not_rounded() {
        assert_eq!(read(&usd(), "-12.30"), Ok((dec!(-12.30), false)));
    }

    #[test]
    fn halves_round_away_from_zero() {
        assert_eq!(read(&usd(), "1/8"), Ok((dec!(0.13), true)));
        assert_eq!(read(&usd(), "-1/8"), Ok((dec!(-0.13), true)));
    }

    #[test]
    fn an_exact_result_is_not_marked_rounded() {
        assert_eq!(read(&usd(), "10+5.5"), Ok((dec!(15.50), false)));
    }

    #[test]
    fn multiplication_and_division_come_before_addition_and_subtraction() {
        assert_eq!(read(&usd(), "2+3*4"), Ok((dec!(14.00), false)));
        assert_eq!(read(&usd(), "10-4/2"), Ok((dec!(8.00), false)));
    }

    #[test]
    fn operators_of_one_precedence_work_left_to_right() {
        assert_eq!(read(&usd(), "10-3-2"), Ok((dec!(5.00), false)));
        assert_eq!(read(&usd(), "100/10/5"), Ok((dec!(2.00), false)));
    }

    #[test]
    fn operands_are_written_with_the_locale_marks() {
        let de = reader(&locale!("de-DE"), "USD", 2);

        assert_eq!(read(&usd(), "1,000.5+1"), Ok((dec!(1001.50), false)));
        assert_eq!(read(&de, "1.000,5+1"), Ok((dec!(1001.50), false)));
        assert_eq!(read(&de, "1.5*2"), Err(AmountParseError::Invalid));
    }

    #[test]
    fn a_minus_with_no_amount_before_it_is_a_sign() {
        assert_eq!(read(&usd(), "-5*-2"), Ok((dec!(10.00), false)));
        assert_eq!(read(&usd(), "$-5+$3"), Ok((dec!(-2.00), false)));
    }

    #[test]
    fn a_trailing_minus_makes_a_plain_amount_negative() {
        assert_eq!(read(&usd(), "5-"), Ok((dec!(-5.00), false)));
    }

    #[test]
    fn dividing_by_zero_is_an_error() {
        assert_eq!(read(&usd(), "5/0"), Err(AmountParseError::DivisionByZero));
        assert_eq!(
            read(&usd(), "5/0.00"),
            Err(AmountParseError::DivisionByZero)
        );
    }

    #[test]
    fn an_operator_missing_an_operand_is_invalid() {
        for text in ["100/", "*5", "5+*2", "5+"] {
            assert_eq!(
                read(&usd(), text),
                Err(AmountParseError::Invalid),
                "{text:?}"
            );
        }
    }

    #[test]
    fn an_operand_that_is_not_a_number_is_invalid() {
        assert_eq!(read(&usd(), "5+1.2.3"), Err(AmountParseError::Invalid));
    }

    #[test]
    fn a_result_too_large_to_hold_is_out_of_range() {
        let text = "79228162514264337593543950335*10";

        assert_eq!(read(&usd(), text), Err(AmountParseError::OutOfRange));
    }

    #[test]
    fn a_zero_result_has_no_minus_sign() {
        let zero = usd().read("-5+5").unwrap().amount();

        assert!(zero.is_sign_positive());
    }

    #[test]
    fn a_reader_keeps_the_locale_it_was_made_in() {
        let mut i18n = Localizer::new(&locale!("en-US").id, &locale!("en-US")).unwrap();
        let reader = i18n.amount_reader(Currency::new("USD", 2).unwrap());

        i18n.set_formatting_locale(&locale!("de-DE")).unwrap();

        assert_eq!(read(&reader, "1.5"), Ok((dec!(1.50), false)));
    }

    #[test]
    fn accepts_only_what_an_amount_or_arithmetic_is_written_with() {
        let reader = usd();
        let typed = "12a,3b4.5$+-*/ €x\u{2212}";

        let kept: String = typed.chars().filter(|&c| reader.accepts(c)).collect();

        assert_eq!(kept, "12,34.5$+-*/ \u{2212}");
    }

    #[test]
    fn accepts_the_symbol_where_the_locale_writes_it() {
        let de_euro = reader(&locale!("de-DE"), "EUR", 2);

        assert_eq!((de_euro.accepts('€'), de_euro.accepts('$')), (true, false));
    }

    #[test]
    fn refuses_the_letters_of_a_symbol_written_in_letters() {
        let dinar = reader(&locale!("en-US"), "BHD", 3);

        assert!(!dinar.accepts('B'));
    }

    #[test]
    fn names_the_marks_amounts_are_typed_with() {
        let de = reader(&locale!("de-DE"), "USD", 2);

        assert_eq!((usd().decimal_mark(), usd().group_separator()), (".", ","));
        assert_eq!((de.decimal_mark(), de.group_separator()), (",", "."));
    }

    #[test]
    fn says_whether_the_text_was_arithmetic() {
        let plain = usd().read("-12.30").unwrap();
        let arithmetic = usd().read("10+5").unwrap();

        assert_eq!(
            (plain.is_arithmetic(), arithmetic.is_arithmetic()),
            (false, true)
        );
    }

    #[test]
    fn a_single_subtraction_is_arithmetic() {
        assert_eq!(read(&usd(), "10-4"), Ok((dec!(6.00), false)));
    }
}
