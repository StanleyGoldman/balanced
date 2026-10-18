//! A mutant's identity across runs, stored in its issue's body.

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};

/// A mutant's file, function, mutation text (original to replacement, as
/// cargo-mutants names it without its location) and its occurrence among identical
/// mutations in that function. It holds no line or column, so edits elsewhere in
/// the file leave it unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct MutantKey {
    pub(crate) file: String,
    pub(crate) function: String,
    pub(crate) mutation: String,
    /// Counts from 0, in source order.
    pub(crate) occurrence: usize,
}

const MARKER_START: &str = "<!-- mutant-key/v1: ";
const MARKER_END: &str = " -->";

impl MutantKey {
    /// The key as an HTML comment for an issue body.
    pub(crate) fn render(&self) -> String {
        let json = serde_json::to_string(self).expect("a key always serialises");
        format!("{MARKER_START}{json}{MARKER_END}")
    }

    /// The key in an issue body's marker, or `None` when the body has no marker. A
    /// marker that doesn't hold a key is an error, since the issue would otherwise
    /// look keyless and its mutant would get a second issue.
    pub(crate) fn parse(body: &str) -> Result<Option<Self>> {
        let Some((_, rest)) = body.split_once(MARKER_START) else {
            return Ok(None);
        };
        let mut values = serde_json::Deserializer::from_str(rest).into_iter::<Self>();
        let key = values
            .next()
            .context("the mutant-key marker is empty")?
            .context("reading the mutant-key marker")?;
        // The deserializer stops at the end of a JSON value, a char boundary in `rest`.
        if !rest[values.byte_offset()..].starts_with(MARKER_END) {
            bail!("the mutant-key marker doesn't end with {MARKER_END:?}");
        }
        Ok(Some(key))
    }
}

#[cfg(test)]
mod tests {
    use super::MutantKey;

    fn key(mutation: &str) -> MutantKey {
        MutantKey {
            file: "crates/balanced_core/src/ledger.rs".to_owned(),
            function: "Ledger::create_category".to_owned(),
            mutation: mutation.to_owned(),
            occurrence: 1,
        }
    }

    #[test]
    fn a_key_survives_a_render_then_parse_round_trip() {
        let key = key("replace == with != in Ledger::create_category");
        let body = format!("Intro.\n\n{}\n\nMore text.\n", key.render());

        assert_eq!(MutantKey::parse(&body).unwrap(), Some(key));
    }

    #[test]
    fn a_mutation_holding_the_marker_end_survives_the_round_trip() {
        let key = key("replace a --> b with \"x\" in f");

        assert_eq!(MutantKey::parse(&key.render()).unwrap(), Some(key));
    }

    #[test]
    fn a_body_with_no_marker_has_no_key() {
        assert_eq!(
            MutantKey::parse("Missed mutant, filed by hand.\n").unwrap(),
            None
        );
        assert_eq!(
            MutantKey::parse("<!-- mutant-key/v2: {} -->").unwrap(),
            None
        );
    }

    #[test]
    fn a_marker_that_holds_no_key_is_an_error() {
        assert!(MutantKey::parse("<!-- mutant-key/v1: {\"file\": -->").is_err());
        assert!(
            MutantKey::parse(&format!(
                "{} trailing",
                key("m").render().trim_end_matches(" -->")
            ))
            .is_err()
        );
    }
}
