//! Messages and attributes a translation lacks, which Fluent fills from `en-US`
//! without a word.

use std::collections::BTreeSet;

use i18n_embed::fluent::FluentLanguageLoader;
use unic_langid::LanguageIdentifier;

/// A message or attribute a translation lacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MissingMessage {
    pub(crate) language: LanguageIdentifier,
    /// The message id, or `id.attribute` for an attribute.
    pub(crate) key: String,
}

/// The messages and attributes each language in `chain` lacks compared with
/// `source`, sorted by language and then key.
pub(crate) fn missing_messages(
    loader: &FluentLanguageLoader,
    chain: &[LanguageIdentifier],
    source: &LanguageIdentifier,
) -> Vec<MissingMessage> {
    let source_keys = message_keys(loader, source);
    let mut missing = Vec::new();
    for language in chain.iter().filter(|language| *language != source) {
        let keys = message_keys(loader, language);
        missing.extend(source_keys.difference(&keys).map(|key| MissingMessage {
            language: language.clone(),
            key: key.clone(),
        }));
    }
    missing
}

/// Logs each message or attribute a language in `chain` lacks, once per load.
pub(crate) fn log_missing_messages(
    loader: &FluentLanguageLoader,
    chain: &[LanguageIdentifier],
    source: &LanguageIdentifier,
) {
    for MissingMessage { language, key } in missing_messages(loader, chain, source) {
        tracing::warn!(%language, key = ?key, "message not translated; showing the {source} text");
    }
}

/// Each message id in `language`, and `id.attribute` for each of its attributes.
fn message_keys(loader: &FluentLanguageLoader, language: &LanguageIdentifier) -> BTreeSet<String> {
    loader.with_message_iter(language, |messages| {
        messages
            .flat_map(|message| {
                let id = message.id.name;
                let attributes = message
                    .attributes
                    .iter()
                    .map(move |attribute| format!("{id}.{}", attribute.id.name));
                std::iter::once(id.to_owned()).chain(attributes)
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use i18n_embed::LanguageLoader as _;
    use rust_embed::Embed;

    use super::*;

    /// A source file and a translation that has every message but lacks an attribute.
    #[derive(Embed)]
    #[folder = "tests/fixtures/missing/"]
    struct Fixtures;

    #[test]
    fn lists_an_attribute_the_translation_lacks() {
        let source: LanguageIdentifier = "en-US".parse().unwrap();
        let de: LanguageIdentifier = "de".parse().unwrap();
        let loader = FluentLanguageLoader::new("balanced", source.clone());
        loader
            .load_languages(&Fixtures, &[de.clone(), source.clone()])
            .unwrap();

        let missing = missing_messages(&loader, &[de.clone(), source.clone()], &source);

        assert_eq!(
            missing,
            [MissingMessage {
                language: de,
                key: "total.tooltip".to_owned(),
            }]
        );
    }
}
