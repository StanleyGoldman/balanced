//! [`AccountsModel`]: the accounts as the Account bar and the context header show them,
//! with every amount already formatted.

use std::collections::HashMap;

use balanced_core::{AccountGroup, AccountId, AccountKind, Generation, LedgerView};
use balanced_i18n::{AmountStyle, Localizer};
use icu_locale::Locale;

use crate::formatted_amount::FormattedAmount;
use crate::i18n::fl;

/// A section of the Account bar, under a heading that folds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Section {
    /// The open accounts of one fixed group.
    Group(AccountGroup),
    /// Closed accounts of every kind, at the foot of the Account bar.
    Closed,
}

impl Section {
    /// Every section, in the order the Account bar shows them.
    pub(crate) const ALL: [Self; 4] = [
        Self::Group(AccountGroup::Banking),
        Self::Group(AccountGroup::Investing),
        Self::Group(AccountGroup::PropertyDebt),
        Self::Closed,
    ];

    /// The section's stable id, which the saved layout names it by.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Group(AccountGroup::Banking) => "banking",
            Self::Group(AccountGroup::Investing) => "investing",
            Self::Group(AccountGroup::PropertyDebt) => "property_debt",
            Self::Closed => "closed",
        }
    }

    /// The section's heading.
    pub(crate) fn label(self, i18n: &Localizer) -> String {
        match self {
            Self::Group(AccountGroup::Banking) => fl!(i18n, "account-bar-group-banking"),
            Self::Group(AccountGroup::Investing) => fl!(i18n, "account-bar-group-investing"),
            Self::Group(AccountGroup::PropertyDebt) => {
                fl!(i18n, "account-bar-group-property-debt")
            }
            Self::Closed => fl!(i18n, "account-bar-group-closed"),
        }
    }
}

/// One account row in the Account bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountRow {
    id: AccountId,
    name: String,
    starts_sub_group: bool,
}

impl AccountRow {
    pub(crate) fn id(&self) -> AccountId {
        self.id
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Whether a thin line separates this row from the one above, because it starts
    /// another sub-group.
    pub(crate) fn starts_sub_group(&self) -> bool {
        self.starts_sub_group
    }
}

/// A section's heading and its rows. A section with no rows is left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SectionRows {
    section: Section,
    rows: Vec<AccountRow>,
}

impl SectionRows {
    pub(crate) fn section(&self) -> Section {
        self.section
    }

    pub(crate) fn rows(&self) -> &[AccountRow] {
        &self.rows
    }
}

/// What the model was built from. It is rebuilt when any of it changes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Source {
    generation: Generation,
    formatting_locale: Locale,
}

/// The accounts of one snapshot, laid out for the Account bar, with every balance
/// and Net Worth formatted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountsModel {
    source: Source,
    sections: Vec<SectionRows>,
    balances: HashMap<AccountId, FormattedAmount>,
    net_worth: FormattedAmount,
}

impl AccountsModel {
    /// The model of `view`, formatted by `i18n`.
    ///
    /// Hidden accounts get no row, but keep a balance for the context header.
    /// Closed accounts are listed in the Closed section, in group order.
    pub(crate) fn new(view: &LedgerView, i18n: &Localizer) -> Self {
        let mut sections: Vec<SectionRows> = Vec::new();
        let mut closed = SectionRows {
            section: Section::Closed,
            rows: Vec::new(),
        };
        let mut previous_kind: HashMap<Section, AccountKind> = HashMap::new();
        for summary in view.accounts() {
            let account = summary.account();
            if account.is_hidden() {
                continue;
            }
            let section = if account.is_closed() {
                Section::Closed
            } else {
                Section::Group(account.kind().group())
            };
            let starts_sub_group = previous_kind
                .insert(section, account.kind())
                .is_some_and(|previous| previous != account.kind());
            let row = AccountRow {
                id: account.id(),
                name: account.name().to_owned(),
                starts_sub_group,
            };
            let rows = match section {
                Section::Closed => &mut closed.rows,
                Section::Group(_) => {
                    // The view lists accounts by group, so a new group is a new last section.
                    if sections.last().is_none_or(|last| last.section != section) {
                        sections.push(SectionRows {
                            section,
                            rows: Vec::new(),
                        });
                    }
                    &mut sections.last_mut().expect("a section was just pushed").rows
                }
            };
            rows.push(row);
        }
        if !closed.rows.is_empty() {
            sections.push(closed);
        }
        Self {
            source: Self::source(view, i18n),
            sections,
            balances: view
                .accounts()
                .iter()
                .map(|summary| {
                    let balance =
                        FormattedAmount::new(i18n, summary.balance(), AmountStyle::Symbol);
                    (summary.account().id(), balance)
                })
                .collect(),
            net_worth: FormattedAmount::new(i18n, view.net_worth(), AmountStyle::Symbol),
        }
    }

    /// Whether the model shows `view` as `i18n` formats it, so needs no rebuild.
    pub(crate) fn is_current(&self, view: &LedgerView, i18n: &Localizer) -> bool {
        self.source == Self::source(view, i18n)
    }

    fn source(view: &LedgerView, i18n: &Localizer) -> Source {
        Source {
            generation: view.generation(),
            formatting_locale: i18n.formatting_locale().clone(),
        }
    }

    /// The Account bar's sections, top to bottom.
    pub(crate) fn sections(&self) -> &[SectionRows] {
        &self.sections
    }

    /// The balance of account `id`, or `None` when the snapshot lacks it.
    pub(crate) fn balance(&self, id: AccountId) -> Option<&FormattedAmount> {
        self.balances.get(&id)
    }

    /// Net Worth: every balance summed, hidden and closed accounts included.
    pub(crate) fn net_worth(&self) -> &FormattedAmount {
        &self.net_worth
    }
}

/// The name of `kind`, as the New Account wizard and Manage Accounts show it.
pub(crate) fn kind_label(i18n: &Localizer, kind: AccountKind) -> String {
    match kind {
        AccountKind::Checking => fl!(i18n, "account-kind-checking"),
        AccountKind::Savings => fl!(i18n, "account-kind-savings"),
        AccountKind::CreditCard => fl!(i18n, "account-kind-credit-card"),
        AccountKind::Investment => fl!(i18n, "account-kind-investment"),
        AccountKind::Retirement => fl!(i18n, "account-kind-retirement"),
        AccountKind::Asset => fl!(i18n, "account-kind-asset"),
        AccountKind::Liability => fl!(i18n, "account-kind-liability"),
    }
}
