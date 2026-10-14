//! Messages a translation lacks, which Fluent fills from `en-US` without a word.

use std::collections::BTreeSet;

use i18n_embed::fluent::FluentLanguageLoader;
use unic_langid::LanguageIdentifier;

/// The message ids each language in `chain` lacks compared with `source`, sorted by
/// language and then id.
pub(crate) fn missing_messages(
    loader: &FluentLanguageLoader,
    chain: &[LanguageIdentifier],
    source: &LanguageIdentifier,
) -> Vec<(LanguageIdentifier, String)> {
    let source_ids = message_ids(loader, source);
    let mut missing = Vec::new();
    for language in chain.iter().filter(|language| *language != source) {
        let ids = message_ids(loader, language);
        missing.extend(
            source_ids
                .difference(&ids)
                .map(|id| (language.clone(), id.clone())),
        );
    }
    missing
}

/// Logs each message a language in `chain` lacks, once per load.
pub(crate) fn log_missing_messages(
    loader: &FluentLanguageLoader,
    chain: &[LanguageIdentifier],
    source: &LanguageIdentifier,
) {
    for (language, id) in missing_messages(loader, chain, source) {
        tracing::warn!(%language, id = ?id, "message not translated; showing the {source} text");
    }
}

fn message_ids(loader: &FluentLanguageLoader, language: &LanguageIdentifier) -> BTreeSet<String> {
    loader.with_message_iter(language, |messages| {
        messages.map(|message| message.id.name.to_owned()).collect()
    })
}
