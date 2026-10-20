// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The text-language fallback chain.

use icu_locale::LanguageIdentifier;
use icu_locale::langid;

/// The source locale: complete, and the last language in every chain.
pub(crate) const SOURCE_LANGUAGE: LanguageIdentifier = langid!("en-US");

/// The languages to look text up in, in order: the requested language, its base
/// language, then [`SOURCE_LANGUAGE`].
///
/// Only languages in `shipped` are kept, and none appears twice. Sibling regions
/// never match: `de-AT` reaches `de` but not `de-DE`.
pub(crate) fn fallback_chain(
    requested: &LanguageIdentifier,
    shipped: &[LanguageIdentifier],
) -> Vec<LanguageIdentifier> {
    let base = LanguageIdentifier::from(requested.language);
    let mut chain: Vec<LanguageIdentifier> = Vec::new();
    for candidate in [requested.clone(), base, SOURCE_LANGUAGE] {
        if shipped.contains(&candidate) && !chain.contains(&candidate) {
            chain.push(candidate);
        }
    }
    chain
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> Vec<LanguageIdentifier> {
        vec![langid!("en-US"), langid!("de"), langid!("fr-CA")]
    }

    #[test]
    fn region_falls_back_to_base_language_then_source() {
        let chain = fallback_chain(&langid!("de-AT"), &shipped());

        assert_eq!(chain, [langid!("de"), langid!("en-US")]);
    }

    #[test]
    fn exact_match_comes_first() {
        let chain = fallback_chain(&langid!("fr-CA"), &shipped());

        assert_eq!(chain, [langid!("fr-CA"), langid!("en-US")]);
    }

    #[test]
    fn sibling_region_does_not_match() {
        let chain = fallback_chain(&langid!("fr-FR"), &shipped());

        assert_eq!(chain, [langid!("en-US")]);
    }

    #[test]
    fn source_language_appears_once() {
        let chain = fallback_chain(&langid!("en-US"), &shipped());

        assert_eq!(chain, [langid!("en-US")]);
    }

    #[test]
    fn unshipped_language_gets_only_the_source() {
        let chain = fallback_chain(&langid!("ja-JP"), &shipped());

        assert_eq!(chain, [langid!("en-US")]);
    }
}
