// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The `en-XA` pseudo-locale: `en-US` text, accented and lengthened, so untranslated
//! strings, truncation and fixed widths show up on screen.

use std::borrow::Cow;

use icu_locale::LanguageIdentifier;
use icu_locale::langid;

/// The pseudo-locale's language tag.
pub(crate) const PSEUDO_LANGUAGE: LanguageIdentifier = langid!("en-XA");

/// Accents ASCII letters and doubles every vowel, which makes English text about
/// 35% longer.
///
/// Every replacement lies in U+00C0..=U+024F, which egui's default font draws.
/// Fluent applies this to the text between placeables only, so formatted amounts
/// and dates pass through untouched.
pub(crate) fn transform(text: &str) -> Cow<'_, str> {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        let accented = accent(c);
        out.push(accented);
        if is_vowel(c) {
            out.push(accented);
        }
    }
    Cow::Owned(out)
}

/// Brackets a whole message so a cut-off end shows.
pub(crate) fn bracket(message: &str) -> String {
    format!("[{message}]")
}

fn is_vowel(c: char) -> bool {
    matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u')
}

fn accent(c: char) -> char {
    match c {
        'a' => 'å',
        'b' => 'ƀ',
        'c' => 'ç',
        'd' => 'ď',
        'e' => 'é',
        'f' => 'ƒ',
        'g' => 'ğ',
        'h' => 'ĥ',
        'i' => 'î',
        'j' => 'ĵ',
        'k' => 'ķ',
        'l' => 'ĺ',
        'n' => 'ñ',
        'o' => 'ö',
        'p' => 'ƥ',
        'q' => 'ǫ',
        'r' => 'ř',
        's' => 'š',
        't' => 'ť',
        'u' => 'ü',
        'w' => 'ŵ',
        'y' => 'ý',
        'z' => 'ž',
        'A' => 'Å',
        'B' => 'Ɓ',
        'C' => 'Ç',
        'D' => 'Ď',
        'E' => 'É',
        'F' => 'Ƒ',
        'G' => 'Ğ',
        'H' => 'Ĥ',
        'I' => 'Î',
        'J' => 'Ĵ',
        'K' => 'Ķ',
        'L' => 'Ĺ',
        'N' => 'Ñ',
        'O' => 'Ö',
        'P' => 'Ƥ',
        'Q' => 'Ǫ',
        'R' => 'Ř',
        'S' => 'Š',
        'T' => 'Ť',
        'U' => 'Ü',
        'W' => 'Ŵ',
        'Y' => 'Ý',
        'Z' => 'Ž',
        // m, v and x have no accented form the font draws.
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accents_letters_and_doubles_vowels() {
        assert_eq!(transform("Total: "), "Ťööťååĺ: ");
    }

    #[test]
    fn leaves_digits_and_punctuation_alone() {
        assert_eq!(transform("1,234.5 (x)"), "1,234.5 (x)");
    }

    #[test]
    fn uses_only_glyphs_egui_draws() {
        let letters: String = ('a'..='z').chain('A'..='Z').collect();

        let out = transform(&letters);

        let drawable = |c: char| c.is_ascii() || ('\u{C0}'..='\u{24F}').contains(&c);
        let missing: String = out.chars().filter(|&c| !drawable(c)).collect();
        assert_eq!(missing, "");
    }

    #[test]
    fn brackets_the_message() {
        assert_eq!(bracket("Ťööťååĺ"), "[Ťööťååĺ]");
    }
}
