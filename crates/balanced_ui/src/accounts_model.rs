// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! [`AccountsModel`]: the accounts as the Account bar and the context header show them,
//! with every amount already formatted.

use std::collections::HashMap;

use balanced_core::{
    AccountGroup, AccountId, AccountKind, AccountSummary, Generation, LedgerView, Money,
};
use balanced_i18n::{AmountStyle, Localizer};
use icu_calendar::{Date, Iso};
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
    today: Date<Iso>,
}

/// The accounts of one snapshot, laid out for the Account bar, with every balance
/// and Net Worth formatted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountsModel {
    source: Source,
    sections: Vec<SectionRows>,
    /// Each account's balance with every transaction counted.
    ending_balances: HashMap<AccountId, FormattedAmount>,
    /// Each account's balance as of today.
    current_balances: HashMap<AccountId, FormattedAmount>,
    /// Each account's Cleared balance.
    cleared_balances: HashMap<AccountId, FormattedAmount>,
    /// Each section's total, for the sections whose sum doesn't overflow.
    totals: HashMap<Section, FormattedAmount>,
    net_worth: FormattedAmount,
}

impl AccountsModel {
    /// The model of `view` on `today`, formatted by `i18n`.
    ///
    /// Hidden accounts get no row, but keep their balances for the context header.
    /// Closed accounts are listed in the Closed section, in group order.
    pub(crate) fn new(view: &LedgerView, i18n: &Localizer, today: Date<Iso>) -> Self {
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
        let formatted = |balance: &dyn Fn(&AccountSummary) -> Money| {
            view.accounts()
                .iter()
                .map(|summary| {
                    let amount = FormattedAmount::new(i18n, balance(summary), AmountStyle::Symbol);
                    (summary.account().id(), amount)
                })
                .collect()
        };
        Self {
            source: Self::source(view, i18n, today),
            sections,
            ending_balances: formatted(&AccountSummary::balance),
            current_balances: formatted(&|summary| summary.balance_on(today)),
            cleared_balances: formatted(&AccountSummary::cleared_balance),
            totals: Section::ALL
                .into_iter()
                .filter_map(|section| {
                    let total = match section {
                        Section::Group(group) => view.group_total(group),
                        Section::Closed => view.closed_total(),
                    };
                    match total {
                        Ok(total) => Some((
                            section,
                            FormattedAmount::new(i18n, total, AmountStyle::Symbol),
                        )),
                        Err(error) => {
                            tracing::warn!(?error, ?section, "a section's total can't be summed");
                            None
                        }
                    }
                })
                .collect(),
            net_worth: FormattedAmount::new(i18n, view.net_worth(), AmountStyle::Symbol),
        }
    }

    /// Whether the model shows `view` on `today` as `i18n` formats it, so needs no
    /// rebuild.
    pub(crate) fn is_current(&self, view: &LedgerView, i18n: &Localizer, today: Date<Iso>) -> bool {
        self.source == Self::source(view, i18n, today)
    }

    fn source(view: &LedgerView, i18n: &Localizer, today: Date<Iso>) -> Source {
        Source {
            generation: view.generation(),
            formatting_locale: i18n.formatting_locale().clone(),
            today,
        }
    }

    /// The Account bar's sections, top to bottom.
    pub(crate) fn sections(&self) -> &[SectionRows] {
        &self.sections
    }

    /// The Ending balance of account `id`, every transaction counted, or `None` when
    /// the snapshot lacks it.
    pub(crate) fn ending_balance(&self, id: AccountId) -> Option<&FormattedAmount> {
        self.ending_balances.get(&id)
    }

    /// The Current balance of account `id`, its transactions dated today or earlier
    /// counted, or `None` when the snapshot lacks it.
    pub(crate) fn current_balance(&self, id: AccountId) -> Option<&FormattedAmount> {
        self.current_balances.get(&id)
    }

    /// The Cleared balance of account `id`, its Cleared and Reconciled transactions
    /// counted whatever their date, or `None` when the snapshot lacks it.
    pub(crate) fn cleared_balance(&self, id: AccountId) -> Option<&FormattedAmount> {
        self.cleared_balances.get(&id)
    }

    /// The sum of the balances of `section`'s accounts, hidden accounts included, or
    /// `None` when the sum overflows.
    pub(crate) fn total(&self, section: Section) -> Option<&FormattedAmount> {
        self.totals.get(&section)
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
