// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! What a Register combobox lists: the [`Choices`] a field offers, and the rows of its
//! list for the text typed so far.

use std::collections::BTreeSet;

use balanced_core::{
    AccountSummary, Allocation, CategoryId, Generation, LedgerView, RegisterEntry, Transaction,
};
use balanced_i18n::Localizer;
use icu_locale::Locale;

use crate::register_model::category_text;

/// One entry a combobox offers: the text it shows and enters, and the value it stands
/// for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Choice<T> {
    text: String,
    /// The text in lower case, which typed text is matched against.
    folded: String,
    value: T,
}

impl<T> Choice<T> {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn value(&self) -> &T {
        &self.value
    }
}

/// Everything a combobox offers, in alphabetical order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Choices<T> {
    choices: Vec<Choice<T>>,
}

impl<T> Choices<T> {
    /// `entries`, each a text and its value, sorted as `i18n` sorts text.
    pub(crate) fn new(i18n: &Localizer, entries: impl IntoIterator<Item = (String, T)>) -> Self {
        let mut choices: Vec<Choice<T>> = entries
            .into_iter()
            .map(|(text, value)| Choice {
                folded: text.to_lowercase(),
                text,
                value,
            })
            .collect();
        choices.sort_by(|a, b| i18n.compare_text(&a.text, &b.text));
        Self { choices }
    }

    /// The choices whose text holds `text`, trimmed, anywhere, ignoring case, in
    /// alphabetical order. Blank text matches every choice.
    pub(crate) fn matching(&self, text: &str) -> Vec<&Choice<T>> {
        let text = text.trim().to_lowercase();
        self.choices
            .iter()
            .filter(|choice| choice.folded.contains(&text))
            .collect()
    }

    /// The choice named `text`, trimmed, ignoring case.
    pub(crate) fn named(&self, text: &str) -> Option<&Choice<T>> {
        let text = text.trim().to_lowercase();
        self.choices.iter().find(|choice| choice.folded == text)
    }
}

/// A row of a combobox list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ListRow<'a> {
    /// A choice, by its text.
    Choice(&'a str),
    /// Create a new entry named by the typed text, which no choice has.
    Create(String),
}

impl<T> Choices<T> {
    /// The list for `text`: the choices matching it.
    pub(crate) fn list(&self, text: &str) -> Vec<ListRow<'_>> {
        self.matching(text)
            .into_iter()
            .map(|choice| ListRow::Choice(choice.text()))
            .collect()
    }

    /// The row creating `text`, trimmed, unless it is blank or a choice has that name.
    pub(crate) fn create_row(&self, text: &str) -> Option<ListRow<'static>> {
        let text = text.trim();
        (!text.is_empty() && self.named(text).is_none()).then(|| ListRow::Create(text.to_owned()))
    }
}

/// What the Register's Payee and Category comboboxes offer: every payee and every
/// category in a snapshot. It is rebuilt when the snapshot or the formatting locale
/// changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldChoices {
    generation: Generation,
    formatting_locale: Locale,
    payees: Choices<()>,
    categories: Choices<CategoryId>,
}

impl FieldChoices {
    /// The payees of every transaction in `view`, each once, and its categories by
    /// their paths, sorted as `i18n` sorts text.
    pub(crate) fn new(view: &LedgerView, i18n: &Localizer) -> Self {
        let payees: BTreeSet<&str> = view
            .accounts()
            .iter()
            .flat_map(AccountSummary::register)
            .map(RegisterEntry::transaction)
            .filter(|transaction| *transaction.allocation() != Allocation::OpeningBalance)
            .map(Transaction::payee)
            .filter(|payee| !payee.is_empty())
            .collect();
        let categories = view.categories().iter().map(|category| {
            let text = category_text(view, category.id())
                .expect("every category of a snapshot has a path in it");
            (text, category.id())
        });
        Self {
            generation: view.generation(),
            formatting_locale: i18n.formatting_locale().clone(),
            payees: Choices::new(i18n, payees.into_iter().map(|payee| (payee.to_owned(), ()))),
            categories: Choices::new(i18n, categories),
        }
    }

    /// Whether these are the choices of `view`, sorted as `i18n` sorts text, so need
    /// no rebuild.
    pub(crate) fn is_current(&self, view: &LedgerView, i18n: &Localizer) -> bool {
        self.generation == view.generation() && self.formatting_locale == *i18n.formatting_locale()
    }

    pub(crate) fn payees(&self) -> &Choices<()> {
        &self.payees
    }

    pub(crate) fn categories(&self) -> &Choices<CategoryId> {
        &self.categories
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn categories() -> Choices<u32> {
        let paths = [
            "Food & Dining:Restaurants",
            "Home:Insurance",
            "Food & Dining:Groceries",
            "Auto & Transport:Insurance",
            "Income:Salary",
        ];
        Choices::new(
            &Localizer::for_tests(),
            (0..).zip(paths).map(|(id, path)| (path.to_owned(), id)),
        )
    }

    fn texts<T>(choices: &[&Choice<T>]) -> Vec<String> {
        choices
            .iter()
            .map(|choice| choice.text().to_owned())
            .collect()
    }

    #[test]
    fn text_matches_anywhere_in_a_choice_ignoring_case_listed_alphabetically() {
        let categories = categories();

        let matches = categories.matching("INSUR");

        assert_eq!(
            texts(&matches),
            ["Auto & Transport:Insurance", "Home:Insurance"]
        );
    }

    #[test]
    fn an_unknown_name_gets_a_create_row() {
        let categories = categories();

        let row = categories.create_row(" Insur ");

        assert_eq!(row, Some(ListRow::Create("Insur".to_owned())));
    }

    #[test]
    fn a_known_name_in_any_case_gets_no_create_row() {
        let categories = categories();

        let row = categories.create_row("home:INSURANCE");

        assert_eq!(row, None);
    }

    #[test]
    fn blank_text_lists_every_choice() {
        let categories = categories();

        let list = categories.list("  ");

        assert_eq!(
            list,
            [
                ListRow::Choice("Auto & Transport:Insurance"),
                ListRow::Choice("Food & Dining:Groceries"),
                ListRow::Choice("Food & Dining:Restaurants"),
                ListRow::Choice("Home:Insurance"),
                ListRow::Choice("Income:Salary"),
            ]
        );
    }

    #[test]
    fn blank_text_gets_no_create_row() {
        let categories = categories();

        let row = categories.create_row("  ");

        assert_eq!(row, None);
    }
}
