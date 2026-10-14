//! Checks every translation against the `en-US` source.
//!
//! Fails on parse errors, on ids, attributes and terms a translation has but `en-US`
//! lacks (stale keys), on a translated pattern whose `$variables` differ from its
//! `en-US` source, and on a term reference with no definition in the same file.
//! Messages a translation lacks are only reported: they fall back to `en-US`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use fluent_syntax::ast::{
    CallArguments, Entry, Expression, InlineExpression, Pattern, PatternElement, Resource,
};

const SOURCE: &str = "en-US";

/// What the check needs from one `.ftl` file.
#[derive(Debug, Default)]
struct Locale {
    /// Each message's patterns by key (`id` or `id.attribute`), with the variables
    /// each pattern uses.
    patterns: BTreeMap<String, BTreeSet<String>>,
    /// Message ids.
    messages: BTreeSet<String>,
    /// Term ids, without the leading `-`.
    terms: BTreeSet<String>,
    /// Term ids referenced anywhere in the file.
    term_references: BTreeSet<String>,
}

#[test]
fn translations_match_the_source() {
    let locales = read_locales();
    let source = locales.get(SOURCE).expect("en-US is the source locale");
    let mut failures = Vec::new();
    let mut report = Vec::new();

    for (name, locale) in &locales {
        failures.extend(undefined_terms(name, locale));
        if name == SOURCE {
            continue;
        }
        failures.extend(stale_keys(name, locale, source));
        failures.extend(variable_mismatches(name, locale, source));
        let missing = source.messages.difference(&locale.messages).count();
        report.push(format!(
            "{name}: {missing} of {} messages untranslated",
            source.messages.len()
        ));
    }

    #[expect(
        clippy::print_stderr,
        reason = "the test reports untranslated counts without failing"
    )]
    for line in report {
        eprintln!("{line}");
    }
    assert!(
        failures.is_empty(),
        "locale files disagree with en-US:\n{}",
        failures.join("\n")
    );
}

fn read_locales() -> BTreeMap<String, Locale> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
    let mut locales = BTreeMap::new();
    for dir in fs::read_dir(&root).expect("locales/ is readable") {
        let dir = dir.expect("locales/ lists its folders");
        let name = dir
            .file_name()
            .into_string()
            .expect("locale folder names are UTF-8");
        assert!(
            name.parse::<unic_langid::LanguageIdentifier>().is_ok(),
            "locales/{name} is not named by a language tag"
        );
        let path = dir.path().join("balanced.ftl");
        let source =
            fs::read_to_string(&path).expect("every locale folder holds a UTF-8 balanced.ftl");
        let resource = fluent_syntax::parser::parse(source.as_str())
            .unwrap_or_else(|(_, errors)| panic!("{} doesn't parse: {errors:?}", path.display()));
        locales.insert(name, Locale::from(&resource));
    }
    locales
}

fn undefined_terms(name: &str, locale: &Locale) -> Vec<String> {
    locale
        .term_references
        .difference(&locale.terms)
        .map(|term| format!("{name}: uses term -{term} but doesn't define it"))
        .collect()
}

fn stale_keys(name: &str, locale: &Locale, source: &Locale) -> Vec<String> {
    let stale_patterns = locale
        .patterns
        .keys()
        .filter(|key| !source.patterns.contains_key(*key))
        .map(|key| format!("{name}: {key} is not in en-US"));
    let stale_terms = locale
        .terms
        .difference(&source.terms)
        .map(|term| format!("{name}: term -{term} is not in en-US"));
    stale_patterns.chain(stale_terms).collect()
}

fn variable_mismatches(name: &str, locale: &Locale, source: &Locale) -> Vec<String> {
    locale
        .patterns
        .iter()
        .filter_map(|(key, variables)| {
            let expected = source.patterns.get(key)?;
            (variables != expected).then(|| {
                format!("{name}: {key} uses variables {variables:?}, en-US uses {expected:?}")
            })
        })
        .collect()
}

impl From<&Resource<&str>> for Locale {
    fn from(resource: &Resource<&str>) -> Self {
        let mut locale = Self::default();
        for entry in &resource.body {
            #[expect(clippy::wildcard_enum_match_arm, reason = "foreign enum")]
            match entry {
                Entry::Message(message) => {
                    let id = message.id.name.to_owned();
                    if let Some(value) = &message.value {
                        let key = id.clone();
                        locale.add_pattern(key, value);
                    }
                    for attribute in &message.attributes {
                        let key = format!("{id}.{}", attribute.id.name);
                        locale.add_pattern(key, &attribute.value);
                    }
                    locale.messages.insert(id);
                }
                Entry::Term(term) => {
                    let id = term.id.name.to_owned();
                    // Terms take no `$variables`; the walk collects the terms they reference.
                    let mut ignored = BTreeSet::new();
                    locale.walk_pattern(&term.value, &mut ignored);
                    for attribute in &term.attributes {
                        locale.walk_pattern(&attribute.value, &mut ignored);
                    }
                    locale.terms.insert(id);
                }
                _ => {}
            }
        }
        locale
    }
}

impl Locale {
    fn add_pattern(&mut self, key: String, pattern: &Pattern<&str>) {
        let mut variables = BTreeSet::new();
        self.walk_pattern(pattern, &mut variables);
        self.patterns.insert(key, variables);
    }

    fn walk_pattern(&mut self, pattern: &Pattern<&str>, variables: &mut BTreeSet<String>) {
        for element in &pattern.elements {
            if let PatternElement::Placeable { expression } = element {
                self.walk_expression(expression, variables);
            }
        }
    }

    fn walk_expression(&mut self, expression: &Expression<&str>, variables: &mut BTreeSet<String>) {
        match expression {
            Expression::Select { selector, variants } => {
                self.walk_inline(selector, variables);
                for variant in variants {
                    self.walk_pattern(&variant.value, variables);
                }
            }
            Expression::Inline(inline) => self.walk_inline(inline, variables),
        }
    }

    fn walk_inline(&mut self, inline: &InlineExpression<&str>, variables: &mut BTreeSet<String>) {
        match inline {
            InlineExpression::VariableReference { id } => {
                variables.insert(id.name.to_owned());
            }
            InlineExpression::TermReference { id, arguments, .. } => {
                self.term_references.insert(id.name.to_owned());
                if let Some(arguments) = arguments {
                    self.walk_arguments(arguments, variables);
                }
            }
            InlineExpression::FunctionReference { arguments, .. } => {
                self.walk_arguments(arguments, variables);
            }
            InlineExpression::Placeable { expression } => {
                self.walk_expression(expression, variables);
            }
            InlineExpression::StringLiteral { .. }
            | InlineExpression::NumberLiteral { .. }
            | InlineExpression::MessageReference { .. } => {}
        }
    }

    fn walk_arguments(
        &mut self,
        arguments: &CallArguments<&str>,
        variables: &mut BTreeSet<String>,
    ) {
        for argument in &arguments.positional {
            self.walk_inline(argument, variables);
        }
        for argument in &arguments.named {
            self.walk_inline(&argument.value, variables);
        }
    }
}
