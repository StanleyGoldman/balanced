//! Where the Main window is: the top-level tab shown in the Main view.

use balanced_i18n::Localizer;

use crate::i18n::fl;

/// Where the Main view is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    /// A top-level tab.
    Tab(TopLevelTab),
}

/// A top-level tab in the Main window's tab row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum TopLevelTab {
    #[default]
    Home,
    Accounts,
    Bills,
    Spending,
    Planning,
    Investing,
    PropertyDebt,
}

impl TopLevelTab {
    /// Every tab, in tab-row order. A new tab goes here as well as in the matches
    /// below, which the compiler checks; it can't check this list.
    pub(crate) const ALL: [Self; 7] = [
        Self::Home,
        Self::Accounts,
        Self::Bills,
        Self::Spending,
        Self::Planning,
        Self::Investing,
        Self::PropertyDebt,
    ];

    /// The tab's name in the tab row.
    pub(crate) fn label(self, i18n: &Localizer) -> String {
        match self {
            Self::Home => fl!(i18n, "tab-home"),
            Self::Accounts => fl!(i18n, "tab-accounts"),
            Self::Bills => fl!(i18n, "tab-bills"),
            Self::Spending => fl!(i18n, "tab-spending"),
            Self::Planning => fl!(i18n, "tab-planning"),
            Self::Investing => fl!(i18n, "tab-investing"),
            Self::PropertyDebt => fl!(i18n, "tab-property-debt"),
        }
    }

    /// What the Main view shows on this tab until the tab has content.
    pub(crate) fn placeholder(self, i18n: &Localizer) -> String {
        match self {
            Self::Home => fl!(i18n, "tab-home-placeholder"),
            Self::Accounts => fl!(i18n, "tab-accounts-placeholder"),
            Self::Bills => fl!(i18n, "tab-bills-placeholder"),
            Self::Spending => fl!(i18n, "tab-spending-placeholder"),
            Self::Planning => fl!(i18n, "tab-planning-placeholder"),
            Self::Investing => fl!(i18n, "tab-investing-placeholder"),
            Self::PropertyDebt => fl!(i18n, "tab-property-debt-placeholder"),
        }
    }
}
