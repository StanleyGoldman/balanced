//! [`RegisterModel`]: one account's transactions as the Register shows them, with every
//! cell's text already formatted.

use std::collections::HashMap;

use balanced_core::{
    AccountId, AccountSummary, Allocation, CategoryId, Generation, LedgerView, TransactionId,
};
use balanced_i18n::{AmountStyle, Localizer};
use icu_locale::LanguageIdentifier;
use icu_locale::Locale;

use crate::formatted_amount::FormattedAmount;
use crate::i18n::fl;

/// Separates a category from the one it sits under: `Food & Dining:Groceries`.
const CATEGORY_SEPARATOR: &str = ":";

/// One transaction's row in the Register, every cell formatted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisterRow {
    id: TransactionId,
    date: String,
    number: String,
    payee: String,
    category: String,
    memo: String,
    amount: FormattedAmount,
    balance: FormattedAmount,
}

impl RegisterRow {
    pub(crate) fn id(&self) -> TransactionId {
        self.id
    }

    pub(crate) fn date(&self) -> &str {
        &self.date
    }

    /// The cheque or reference number, or nothing.
    pub(crate) fn number(&self) -> &str {
        &self.number
    }

    pub(crate) fn payee(&self) -> &str {
        &self.payee
    }

    /// The category's path from the top of the tree, `[Split]` for a split
    /// transaction, or nothing for the opening balance.
    pub(crate) fn category(&self) -> &str {
        &self.category
    }

    pub(crate) fn memo(&self) -> &str {
        &self.memo
    }

    /// The amount, as a bare number.
    pub(crate) fn amount(&self) -> &FormattedAmount {
        &self.amount
    }

    /// The running balance after this transaction, as a bare number.
    pub(crate) fn balance(&self) -> &FormattedAmount {
        &self.balance
    }
}

/// What the model was built from. It is rebuilt when any of it changes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Source {
    account: AccountId,
    generation: Generation,
    text_language: LanguageIdentifier,
    formatting_locale: Locale,
}

/// One account's Register rows, in date order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisterModel {
    source: Source,
    rows: Vec<RegisterRow>,
}

impl RegisterModel {
    /// The Register of `summary`, an account in `view`, formatted by `i18n`.
    pub(crate) fn new(summary: &AccountSummary, view: &LedgerView, i18n: &Localizer) -> Self {
        let mut paths = CategoryPaths {
            view,
            paths: HashMap::new(),
        };
        let split = fl!(i18n, "register-split");
        let opening_balance = fl!(i18n, "register-opening-balance");
        let rows = summary
            .register()
            .iter()
            .map(|entry| {
                let transaction = entry.transaction();
                let (payee, category) = match transaction.allocation() {
                    Allocation::OpeningBalance => (opening_balance.clone(), String::new()),
                    Allocation::Category(id) => {
                        (transaction.payee().to_owned(), paths.path(*id).to_owned())
                    }
                    Allocation::Split(_) => (transaction.payee().to_owned(), split.clone()),
                };
                RegisterRow {
                    id: transaction.id(),
                    date: i18n.format_date(transaction.date()),
                    number: transaction.number().unwrap_or_default().to_owned(),
                    payee,
                    category,
                    memo: transaction.memo().to_owned(),
                    amount: FormattedAmount::new(i18n, transaction.amount(), AmountStyle::Bare),
                    balance: FormattedAmount::new(i18n, entry.balance(), AmountStyle::Bare),
                }
            })
            .collect();
        Self {
            source: Self::source(summary.account().id(), view, i18n),
            rows,
        }
    }

    /// Whether the model shows account `account` of `view` as `i18n` formats it, so
    /// needs no rebuild.
    pub(crate) fn is_current(
        &self,
        account: AccountId,
        view: &LedgerView,
        i18n: &Localizer,
    ) -> bool {
        self.source == Self::source(account, view, i18n)
    }

    fn source(account: AccountId, view: &LedgerView, i18n: &Localizer) -> Source {
        Source {
            account,
            generation: view.generation(),
            text_language: i18n.text_language().clone(),
            formatting_locale: i18n.formatting_locale().clone(),
        }
    }

    /// The account whose rows these are.
    pub(crate) fn account(&self) -> AccountId {
        self.source.account
    }

    /// The rows, oldest transaction first.
    pub(crate) fn rows(&self) -> &[RegisterRow] {
        &self.rows
    }
}

/// Category paths, each built once while a model is built.
struct CategoryPaths<'a> {
    view: &'a LedgerView,
    paths: HashMap<CategoryId, String>,
}

impl CategoryPaths<'_> {
    /// The path from the top of the tree to category `id`: `Food & Dining:Groceries`.
    fn path(&mut self, id: CategoryId) -> &str {
        let view = self.view;
        self.paths.entry(id).or_insert_with(|| {
            view.category_path(id)
                .expect("a transaction's categories are in its snapshot")
                .join(CATEGORY_SEPARATOR)
        })
    }
}
