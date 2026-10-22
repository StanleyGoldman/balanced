// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! What a Register combobox lists: the [`Choices`] a field offers, and the rows of its
//! list for the text typed so far.

use std::collections::{BTreeSet, HashMap};

use balanced_core::{
    Account, AccountId, Allocation, CategoryId, Generation, LedgerView, Money, RegisterEntry,
    Transaction, TransactionId, Visibility,
};
use balanced_i18n::Localizer;
use icu_locale::Locale;

use crate::register_model::{category_text, transactions_by_id, transfer_text};

/// One entry a combobox offers: the text it shows and enters, and the value it stands
/// for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Choice<T> {
    text: String,
    /// The text in lower case, which typed text is matched against.
    folded: String,
    value: T,
    /// Whether the list and Inline completion offer it. One left out is still taken
    /// when its whole text is typed.
    listed: bool,
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
                listed: true,
            })
            .collect();
        choices.sort_by(|a, b| i18n.compare_text(&a.text, &b.text));
        Self { choices }
    }

    /// The listed choices whose text holds `text`, trimmed, anywhere, ignoring case, in
    /// alphabetical order. Blank text matches every listed choice.
    pub(crate) fn matching(&self, text: &str) -> Vec<&Choice<T>> {
        let text = text.trim().to_lowercase();
        self.choices
            .iter()
            .filter(|choice| choice.listed && choice.folded.contains(&text))
            .collect()
    }

    /// These choices, then `later`'s, each in their own order.
    #[must_use]
    pub(crate) fn followed_by(mut self, later: Self) -> Self {
        self.choices.extend(later.choices);
        self
    }

    /// The same choices, left out of the list and Inline completion, but still found
    /// by name.
    #[must_use]
    pub(crate) fn unlisted(mut self) -> Self {
        for choice in &mut self.choices {
            choice.listed = false;
        }
        self
    }

    /// The choice named `text`, trimmed, ignoring case, listed or not.
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

/// The rest of `text` after `typed`, when `text` starts with `typed`, ignoring case.
pub(crate) fn rest_after<'a>(text: &'a str, typed: &str) -> Option<&'a str> {
    let mut rest = text.chars();
    for wanted in typed.chars() {
        let next = rest.next()?;
        if !next.to_lowercase().eq(wanted.to_lowercase()) {
            return None;
        }
    }
    Some(rest.as_str())
}

/// What a Category cell names: a category, or another account, for a transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CategoryTarget {
    Category(CategoryId),
    Transfer(AccountId),
}

/// What Payee recall fills in from a payee's latest transaction: the payee as that
/// transaction spells it, and what each of the draft row's empty cells takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Recalled {
    payee: String,
    /// `None` when the transaction is a split, or a transfer whose account to fill in
    /// is closed.
    category: Option<CategoryTarget>,
    memo: String,
    /// `None` when the transaction's amount is in another currency than the account
    /// whose Register recalls it.
    amount: Option<Money>,
}

impl Recalled {
    pub(crate) fn payee(&self) -> &str {
        &self.payee
    }

    pub(crate) fn category(&self) -> Option<CategoryTarget> {
        self.category
    }

    pub(crate) fn memo(&self) -> &str {
        &self.memo
    }

    pub(crate) fn amount(&self) -> Option<Money> {
        self.amount
    }
}

/// What one account's Register offers in its Payee and Category comboboxes: every payee
/// and every category in a snapshot, then the accounts it can transfer to. It also holds
/// what Payee recall fills in for each payee. It is rebuilt when the snapshot, the
/// account or the formatting locale changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldChoices {
    generation: Generation,
    account: AccountId,
    formatting_locale: Locale,
    payees: Choices<()>,
    categories: Choices<CategoryTarget>,
    /// What recalling each payee fills in, by the payee in lower case.
    recalls: HashMap<String, Recalled>,
}

impl FieldChoices {
    /// The payees of every transaction in `view`, each once, its shown categories by
    /// their paths, and then, as `[Name]`, every open account but `account`, hidden ones
    /// included. Each part is sorted as `i18n` sorts text. Opening balances have no
    /// payee to offer. Hidden categories are found by their paths, but never listed.
    pub(crate) fn new(view: &LedgerView, account: AccountId, i18n: &Localizer) -> Self {
        let transactions = || {
            view.accounts()
                .iter()
                .flat_map(|summary| summary.register())
                .map(RegisterEntry::transaction)
                .filter(|transaction| *transaction.allocation() != Allocation::OpeningBalance)
                .filter(|transaction| !transaction.payee().is_empty())
        };
        let payees: BTreeSet<&str> = transactions().map(Transaction::payee).collect();
        let categories = |visibility| {
            view.categories()
                .iter()
                .filter(move |category| category.visibility() == visibility)
                .map(|category| {
                    let text = category_text(view, category.id())
                        .expect("every category of a snapshot has a path in it");
                    (text, CategoryTarget::Category(category.id()))
                })
        };
        let accounts = view
            .accounts()
            .iter()
            .map(|summary| summary.account())
            .filter(|other| other.id() != account && !other.is_closed())
            .map(|other| {
                (
                    transfer_text(other.name()),
                    CategoryTarget::Transfer(other.id()),
                )
            });
        Self {
            generation: view.generation(),
            account,
            formatting_locale: i18n.formatting_locale().clone(),
            payees: Choices::new(i18n, payees.into_iter().map(|payee| (payee.to_owned(), ()))),
            categories: Choices::new(i18n, categories(Visibility::Shown))
                .followed_by(Choices::new(i18n, accounts))
                .followed_by(Choices::new(i18n, categories(Visibility::Hidden)).unlisted()),
            recalls: Recalled::by_payee(transactions(), view, account),
        }
    }

    /// Whether these are the choices of `view` for `account`'s Register, sorted as
    /// `i18n` sorts text, so need no rebuild.
    pub(crate) fn is_current(
        &self,
        view: &LedgerView,
        account: AccountId,
        i18n: &Localizer,
    ) -> bool {
        self.generation == view.generation()
            && self.account == account
            && self.formatting_locale == *i18n.formatting_locale()
    }

    pub(crate) fn payees(&self) -> &Choices<()> {
        &self.payees
    }

    pub(crate) fn categories(&self) -> &Choices<CategoryTarget> {
        &self.categories
    }

    /// What recalling the payee named `payee`, trimmed, ignoring case, fills in, if an
    /// earlier transaction has that payee.
    pub(crate) fn recalled(&self, payee: &str) -> Option<&Recalled> {
        self.recalls.get(&payee_key(payee))
    }
}

impl Recalled {
    /// What recalling each payee of `transactions` fills in `account`'s draft row, by
    /// the payee's key: from its latest transaction, by date and then by id. Of a
    /// transfer's two sides with one payee, it is the side that was posted, not the
    /// other side made for it.
    fn by_payee<'a>(
        transactions: impl Iterator<Item = &'a Transaction>,
        view: &'a LedgerView,
        account: AccountId,
    ) -> HashMap<String, Self> {
        let by_id = transactions_by_id(view);
        let side = |id| {
            *by_id
                .get(&id)
                .expect("a transfer's sides are in its snapshot")
        };
        let mut latest: HashMap<String, &Transaction> = HashMap::new();
        for transaction in transactions {
            if let Some(posted) = posted_side(transaction)
                && payee_key(side(posted).1.payee()) == payee_key(transaction.payee())
            {
                continue;
            }
            latest
                .entry(payee_key(transaction.payee()))
                .and_modify(|kept| {
                    if (transaction.date(), transaction.id()) > (kept.date(), kept.id()) {
                        *kept = transaction;
                    }
                })
                .or_insert(transaction);
        }
        let currency = view
            .account(account)
            .map(|summary| summary.account().currency());
        // A transfer to a closed account can't be posted.
        let transfer_to =
            |other: &Account| (!other.is_closed()).then(|| CategoryTarget::Transfer(other.id()));
        latest
            .into_iter()
            .filter_map(|(key, transaction)| {
                let (category, amount) = match *transaction.allocation() {
                    Allocation::Category(category) => (
                        Some(CategoryTarget::Category(category)),
                        transaction.amount(),
                    ),
                    // Seen from its other side, the transfer comes from the source's
                    // account, for the amount with its sign flipped.
                    Allocation::Transfer(other_side) if side(other_side).0.id() == account => {
                        (transfer_to(side(transaction.id()).0), -transaction.amount())
                    }
                    Allocation::Transfer(other_side) => {
                        (transfer_to(side(other_side).0), transaction.amount())
                    }
                    // The draft row holds one category until #212.
                    Allocation::Split(_) => (None, transaction.amount()),
                    // Opening balances have no payee to recall.
                    Allocation::OpeningBalance => return None,
                };
                let recalled = Self {
                    payee: transaction.payee().to_owned(),
                    category,
                    memo: transaction.memo().to_owned(),
                    amount: (Some(amount.currency()) == currency).then_some(amount),
                };
                Some((key, recalled))
            })
            .collect()
    }
}

/// The transaction a transfer or split line was posted as, when `transaction` is the
/// other side made for it in another account. Posting gives the other side, and
/// changing a transfer's account gives the new one, an id after the posted side's.
fn posted_side(transaction: &Transaction) -> Option<TransactionId> {
    match *transaction.allocation() {
        Allocation::Transfer(other_side) if other_side < transaction.id() => Some(other_side),
        Allocation::Transfer(_)
        | Allocation::OpeningBalance
        | Allocation::Category(_)
        | Allocation::Split(_) => None,
    }
}

/// The key a payee is remembered by: its text, trimmed, in lower case.
fn payee_key(payee: &str) -> String {
    payee.trim().to_lowercase()
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
