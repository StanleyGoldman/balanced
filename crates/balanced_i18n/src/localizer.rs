//! [`Localizer`]: the text languages, the formatting locale and their lookups.

use std::cmp::Ordering;

use i18n_embed::LanguageLoader as _;
use i18n_embed::fluent::FluentLanguageLoader;
use icu_calendar::types::Weekday;
use icu_calendar::{Date, Iso};
use icu_locale::{LanguageIdentifier, Locale, locale};
use rust_decimal::Decimal;
use rust_embed::Embed;

use crate::chain::{SOURCE_LANGUAGE, fallback_chain};
use crate::date::{self, DateSegments};
use crate::format::{Formatters, format_fluent_value};
use crate::missing::log_missing_messages;
use crate::money::{self, AmountStyle, Currency};
use crate::pseudo::{self, PSEUDO_LANGUAGE};
use crate::{AmountParseError, AmountReader, DateParseError, I18nError};

/// The `.ftl` files, one folder per shipped language.
#[derive(Embed)]
#[folder = "locales/"]
struct Locales;

/// Whether Fluent wraps placeables in bidi isolation marks (U+2068, U+2069).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Isolation {
    /// The app's setting: placeables keep their own direction.
    On,
    /// The test setting: strings compare equal to plain literals.
    #[cfg(any(test, feature = "test-support"))]
    Off,
}

/// The Fluent domain, which names each language's file: `balanced.ftl`.
const DOMAIN: &str = "balanced";

/// Looks up user-interface text in the text language and formats numbers and dates
/// in the formatting locale.
///
/// The two are set apart: a `de-AT` user can read German text with Swiss number
/// formatting. Text falls back along the chain the text language picks (see
/// `docs/practices/i18n.md`), and formatting always follows the formatting locale.
///
/// Building a `Localizer`, or changing its formatting locale, also sets how numbers
/// inside messages are formatted on the calling thread. Look text up on that
/// thread.
#[derive(Debug)]
pub struct Localizer {
    loader: FluentLanguageLoader,
    text_language: LanguageIdentifier,
    formatting_locale: Locale,
    formatters: Formatters,
    isolation: Isolation,
}

impl Localizer {
    /// The pseudo-locale tag. Its text is `en-US`, accented, lengthened and
    /// bracketed.
    pub const PSEUDO_LANGUAGE: LanguageIdentifier = PSEUDO_LANGUAGE;

    /// A localizer for `text_language`, formatting in `formatting_locale`.
    pub fn new(
        text_language: &LanguageIdentifier,
        formatting_locale: &Locale,
    ) -> Result<Self, I18nError> {
        Self::with_isolation(text_language, formatting_locale, Isolation::On)
    }

    /// The pseudo-locale, formatting in `de-DE` so that hard-coded `.` and `,`
    /// assumptions show up alongside untranslated text.
    pub fn pseudo() -> Result<Self, I18nError> {
        Self::new(&PSEUDO_LANGUAGE, &locale!("de-DE"))
    }

    /// A localizer for the operating system's locale, used for both text and
    /// formatting. Falls back to `en-US` when the OS reports no locale or one that
    /// doesn't parse.
    pub fn from_os() -> Result<Self, I18nError> {
        let os_locale = sys_locale::get_locale();
        let locale = match os_locale.as_deref().map(Locale::try_from_str) {
            Some(Ok(locale)) => locale,
            Some(Err(err)) => {
                tracing::warn!(locale = ?os_locale, %err, "OS locale doesn't parse; using {SOURCE_LANGUAGE}");
                Locale::from(SOURCE_LANGUAGE)
            }
            None => Locale::from(SOURCE_LANGUAGE),
        };
        Self::new(&locale.id, &locale)
    }

    /// The `en-US` localizer tests use: Fluent's bidi isolation marks are off, so
    /// strings compare equal to plain literals.
    #[cfg(feature = "test-support")]
    pub fn for_tests() -> Self {
        Self::with_isolation(
            &SOURCE_LANGUAGE,
            &Locale::from(SOURCE_LANGUAGE),
            Isolation::Off,
        )
        .expect("the embedded en-US messages and ICU4X's built-in en-US data always load")
    }

    fn with_isolation(
        text_language: &LanguageIdentifier,
        formatting_locale: &Locale,
        isolation: Isolation,
    ) -> Result<Self, I18nError> {
        let source = to_fluent_language(&SOURCE_LANGUAGE);
        let localizer = Self {
            loader: FluentLanguageLoader::new(DOMAIN, source),
            text_language: text_language.clone(),
            formatting_locale: formatting_locale.clone(),
            formatters: Formatters::new(formatting_locale)?,
            isolation,
        };
        localizer.load_text_language(text_language)?;
        // Only a localizer that is returned changes how this thread formats numbers.
        localizer.formatters.install_for_fluent();
        Ok(localizer)
    }

    /// Switches the text language. The change shows on the next frame.
    pub fn set_text_language(&mut self, language: &LanguageIdentifier) -> Result<(), I18nError> {
        self.load_text_language(language)?;
        self.text_language = language.clone();
        Ok(())
    }

    /// Switches the formatting locale. The change shows on the next frame.
    pub fn set_formatting_locale(&mut self, locale: &Locale) -> Result<(), I18nError> {
        self.formatters = Formatters::new(locale)?;
        self.formatters.install_for_fluent();
        self.formatting_locale = locale.clone();
        Ok(())
    }

    /// The language text is shown in, before fallback.
    pub fn text_language(&self) -> &LanguageIdentifier {
        &self.text_language
    }

    /// The locale numbers and dates are formatted in.
    pub fn formatting_locale(&self) -> &Locale {
        &self.formatting_locale
    }

    /// The Fluent loader. Only `balanced_ui`'s `fl!` wrapper calls this, and it
    /// passes each result through [`Self::finish`].
    pub fn loader(&self) -> &FluentLanguageLoader {
        &self.loader
    }

    /// Finishes a message looked up through [`Self::loader`]: brackets it in the
    /// pseudo-locale, and returns it unchanged otherwise.
    pub fn finish(&self, message: String) -> String {
        if self.text_language == PSEUDO_LANGUAGE {
            pseudo::bracket(&message)
        } else {
            message
        }
    }

    /// Formats an integer with the formatting locale's digits and grouping. Money
    /// has its own formatting: [`Self::format_amount`].
    pub fn format_number(&self, value: i64) -> String {
        self.formatters.number(value)
    }

    /// Formats a money amount in the formatting locale: `$1,234.50` in `en-US` and
    /// `1.234,50 $` in `de-DE`, whatever the currency. This is the only way an
    /// amount is shown to the user.
    ///
    /// The amount shows exactly the currency's decimal places, and a negative amount
    /// gets the locale's standard minus sign. `amount` should already be at the
    /// currency's decimal places, as a `Money` amount is. An amount with more
    /// significant decimal places shows all of them: display never rounds.
    pub fn format_amount(&self, amount: Decimal, currency: Currency, style: AmountStyle) -> String {
        money::format_amount(&self.formatting_locale, amount, currency, style)
    }

    /// Reads an amount the user typed, written the formatting locale's way, for a
    /// currency with `decimal_places` decimal places. The result is at exactly
    /// those decimal places.
    ///
    /// Accepts the locale's decimal mark and group separator (a typed space stands
    /// in for a no-break space), a leading or trailing minus, and a currency symbol
    /// or code at either end, which is ignored. Strict about the locale: `1.234` is
    /// one thousand two hundred and thirty-four in `de-DE`, and `1.23` is rejected
    /// there because its group is too short. More decimal places than the currency
    /// allows are rejected, not rounded, unless the extra digits are zeros.
    pub fn parse_amount(
        &self,
        text: &str,
        decimal_places: u32,
    ) -> Result<Decimal, AmountParseError> {
        money::parse_amount(self.formatters.symbols(), decimal_places, text)
    }

    /// A reader for amounts in `currency` typed in the current formatting locale: a
    /// plain amount, which goes through [`Self::parse_amount`], or arithmetic over
    /// amounts. The reader keeps this locale's marks if the formatting locale changes
    /// later.
    pub fn amount_reader(&self, currency: Currency) -> AmountReader {
        let formatted = self.format_amount(Decimal::ONE, currency, AmountStyle::Symbol);
        AmountReader::new(
            self.formatters.symbols().clone(),
            currency.decimal_places(),
            &formatted,
        )
    }

    /// Formats a date as the formatting locale writes a short date with a
    /// four-digit year: `10/4/2026` in `en-US`, `04.10.2026` in `de-DE`.
    pub fn format_date(&self, date: Date<Iso>) -> String {
        self.formatters.date(date)
    }

    /// How the formatting locale writes a date as numbers: the order of its segments
    /// and the separator between them.
    pub fn date_segments(&self) -> &DateSegments {
        self.formatters.date_segments()
    }

    /// Reads a full date the user typed or pasted. It takes the day, month and year
    /// in the formatting locale's order (`10/5/2026` in `en-US`, `5.10.2026` in
    /// `de-DE`), the ISO order (`2026-10-05`), or eight digits with no separators in
    /// ISO order (`20261005`). A date missing a part, or naming a day the calendar
    /// lacks, is rejected, never guessed.
    pub fn parse_date(&self, text: &str) -> Result<Date<Iso>, DateParseError> {
        date::parse_date(self.formatters.date_segments(), text)
    }

    /// The short name of `weekday` in the formatting locale, for a calendar's column
    /// headings: `Sun` in `en-US`, `So` in `de-DE`.
    pub fn format_weekday(&self, weekday: Weekday) -> String {
        self.formatters.weekday(weekday)
    }

    /// The month and year of `date` in the formatting locale, for a calendar's title:
    /// `October 2026` in `en-US`, `Oktober 2026` in `de-DE`.
    pub fn format_month_year(&self, date: Date<Iso>) -> String {
        self.formatters.month_year(date)
    }

    /// The day weeks start on in the formatting locale. A `-u-fw-` extension on
    /// the locale overrides the region's default.
    pub fn first_weekday(&self) -> Weekday {
        self.formatters.first_weekday()
    }

    /// Orders `a` and `b` as the formatting locale sorts text.
    pub fn compare_text(&self, a: &str, b: &str) -> Ordering {
        self.formatters.compare(a, b)
    }

    fn load_text_language(&self, language: &LanguageIdentifier) -> Result<(), I18nError> {
        let is_pseudo = *language == PSEUDO_LANGUAGE;
        let chain = if is_pseudo {
            vec![SOURCE_LANGUAGE]
        } else {
            fallback_chain(language, &shipped_languages())
        };
        let chain: Vec<_> = chain.iter().map(to_fluent_language).collect();
        self.loader.load_languages(&Locales, &chain)?;
        let isolating = self.isolation == Isolation::On;
        self.loader.with_bundles_mut(|bundle| {
            bundle.set_formatter(Some(format_fluent_value));
            bundle.set_use_isolating(isolating);
            bundle.set_transform(is_pseudo.then_some(pseudo::transform as _));
        });
        if cfg!(debug_assertions) {
            log_missing_messages(&self.loader, &chain, self.loader.fallback_language());
        }
        Ok(())
    }
}

/// The languages with a folder under `locales/`, sorted.
fn shipped_languages() -> Vec<LanguageIdentifier> {
    let mut languages: Vec<LanguageIdentifier> = Locales::iter()
        .filter_map(|path| {
            let (folder, _) = path.split_once('/')?;
            let language = LanguageIdentifier::try_from_str(folder)
                .expect("the locales test checks every folder is named by a language tag");
            Some(language)
        })
        .collect();
    languages.sort_by_cached_key(ToString::to_string);
    languages.dedup();
    languages
}

/// Converts an ICU4X language identifier to the `unic-langid` one Fluent uses.
fn to_fluent_language(language: &LanguageIdentifier) -> unic_langid::LanguageIdentifier {
    language
        .to_string()
        .parse()
        .expect("a valid ICU4X language identifier is a valid unic-langid one")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use fluent_bundle::FluentValue;
    use icu_locale::langid;

    use super::*;
    use crate::missing::missing_messages;

    /// Looks `id` up with one argument.
    fn text_with(
        i18n: &Localizer,
        id: &str,
        name: &'static str,
        value: FluentValue<'static>,
    ) -> String {
        i18n.loader()
            .get_args_concrete(id, HashMap::from([(name, value)]))
    }

    /// A localizer whose strings compare equal to plain literals.
    fn localizer(text_language: &LanguageIdentifier, formatting_locale: &Locale) -> Localizer {
        Localizer::with_isolation(text_language, formatting_locale, Isolation::Off).unwrap()
    }

    #[test]
    fn regional_german_reads_the_german_file() {
        let i18n = localizer(&langid!("de-AT"), &locale!("de-AT"));

        assert_eq!(i18n.loader().get("menu-file"), "Datei");
    }

    #[test]
    fn untranslated_message_falls_back_to_en_us() {
        let i18n = localizer(&langid!("de-AT"), &locale!("de-AT"));

        assert_eq!(i18n.loader().get("register-column-amount"), "Amount");
    }

    #[test]
    fn german_plurals_select_by_count() {
        let i18n = localizer(&langid!("de"), &locale!("de-DE"));
        let count = |n: i64| text_with(&i18n, "register-transaction-count", "count", n.into());

        assert_eq!(count(1), "1 Buchung");
        assert_eq!(count(2), "2 Buchungen");
    }

    #[test]
    fn numbers_in_text_follow_the_formatting_locale_when_text_falls_back() {
        let i18n = localizer(&langid!("ja-JP"), &locale!("de-DE"));
        let text = text_with(&i18n, "register-transaction-count", "count", 1234.into());

        assert_eq!(text, "1.234 transactions");
    }

    #[test]
    fn pseudo_locale_transforms_text_but_not_arguments() {
        let i18n =
            Localizer::with_isolation(&PSEUDO_LANGUAGE, &locale!("de-DE"), Isolation::Off).unwrap();

        let text = i18n.finish(text_with(
            &i18n,
            "account-bar-total",
            "amount",
            "1.234,50 €".into(),
        ));

        assert_eq!(text, "[Ťööťååĺ: 1.234,50 €]");
    }

    #[test]
    fn switching_text_language_takes_effect() {
        let mut i18n = localizer(&langid!("en-US"), &locale!("en-US"));

        i18n.set_text_language(&langid!("de-DE")).unwrap();

        assert_eq!(i18n.loader().get("menu-file"), "Datei");
        assert_eq!(i18n.text_language(), &langid!("de-DE"));
    }

    #[test]
    fn switching_from_the_pseudo_locale_drops_accents_and_brackets() {
        let mut i18n =
            Localizer::with_isolation(&PSEUDO_LANGUAGE, &locale!("de-DE"), Isolation::Off).unwrap();

        i18n.set_text_language(&langid!("en-US")).unwrap();

        assert_eq!(i18n.finish(i18n.loader().get("menu-file")), "File");
    }

    #[test]
    fn switching_formatting_locale_takes_effect() {
        let mut i18n = localizer(&langid!("en-US"), &locale!("en-US"));

        i18n.set_formatting_locale(&locale!("de-DE")).unwrap();

        assert_eq!(i18n.format_number(1234), "1.234");
        assert_eq!(i18n.formatting_locale(), &locale!("de-DE"));
    }

    #[test]
    fn bidi_isolation_is_on_in_the_app() {
        let i18n = Localizer::new(&langid!("en-US"), &locale!("en-US")).unwrap();

        let text = text_with(&i18n, "account-bar-total", "amount", "$5.00".into());

        assert_eq!(text, "Total: \u{2068}$5.00\u{2069}");
    }

    #[test]
    fn finish_leaves_real_languages_unchanged() {
        let i18n = localizer(&langid!("de"), &locale!("de-DE"));

        assert_eq!(i18n.finish("Datei".to_owned()), "Datei");
    }

    #[test]
    fn lists_messages_the_german_file_lacks() {
        let i18n = localizer(&langid!("de"), &locale!("de-DE"));
        let chain: Vec<_> = [langid!("de"), SOURCE_LANGUAGE]
            .iter()
            .map(to_fluent_language)
            .collect();

        let missing =
            missing_messages(i18n.loader(), &chain, &to_fluent_language(&SOURCE_LANGUAGE));

        let de = to_fluent_language(&langid!("de"));
        let ids: Vec<_> = missing
            .iter()
            .map(|message| (&message.language, message.key.as_str()))
            .collect();
        // The German file grows with each feature, so check one message it lacks
        // and one it has rather than the whole list.
        assert!(ids.contains(&(&de, "register-column-amount")), "{ids:?}");
        assert!(!ids.contains(&(&de, "menu-file")), "{ids:?}");
    }

    #[test]
    fn formats_and_parses_amounts_in_the_formatting_locale() {
        let i18n = localizer(&langid!("en-US"), &locale!("de-DE"));
        let usd = Currency::new("USD", 2).unwrap();
        let amount = Decimal::new(-123_450, 2);

        let text = i18n.format_amount(amount, usd, AmountStyle::Symbol);

        assert_eq!(text, "-1.234,50\u{a0}$");
        assert_eq!(i18n.parse_amount(&text, 2), Ok(amount));
    }

    #[test]
    fn pseudo_locale_amounts_parse_back() {
        let i18n = Localizer::pseudo().unwrap();
        let usd = Currency::new("USD", 2).unwrap();
        let amount = Decimal::new(-123_450, 2);

        let text = i18n.format_amount(amount, usd, AmountStyle::Symbol);

        assert_eq!(i18n.parse_amount(&text, 2), Ok(amount), "{text:?}");
    }

    #[test]
    fn formats_dates_and_weeks_in_the_formatting_locale() {
        let i18n = localizer(&langid!("en-US"), &locale!("de-DE"));
        let date = Date::try_new_iso(2026, 10, 4).unwrap();

        assert_eq!(i18n.format_date(date), "04.10.2026");
        assert_eq!(i18n.first_weekday(), Weekday::Monday);
    }

    #[test]
    fn compares_text_in_the_formatting_locale() {
        let i18n = localizer(&langid!("en-US"), &locale!("de-DE"));
        let mut payees = ["Zebra", "Öl", "apple"];

        payees.sort_by(|a, b| i18n.compare_text(a, b));

        assert_eq!(payees, ["apple", "Öl", "Zebra"]);
    }

    #[cfg(feature = "test-support")]
    #[test]
    fn test_localizer_is_en_us_without_isolation_marks() {
        let i18n = Localizer::for_tests();

        let text = text_with(&i18n, "account-bar-total", "amount", "$5.00".into());

        assert_eq!(text, "Total: $5.00");
    }

    #[test]
    fn ships_en_us_and_german() {
        assert_eq!(shipped_languages(), [langid!("de"), langid!("en-US")]);
    }
}
