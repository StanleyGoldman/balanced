//! Accounts: what kind each is, the fixed group its kind belongs to, and its flags.

use crate::CurrencyCode;

/// One of the fixed groups the Account bar lists accounts in, in Account bar order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountGroup {
    Banking,
    Investing,
    PropertyDebt,
}

/// What an account holds. Each kind is one sub-group of one fixed [`AccountGroup`].
///
/// Kinds are declared, and ordered, the way the Account bar lists their sub-groups:
/// by group, then by their fixed order within it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AccountKind {
    Checking,
    Savings,
    CreditCard,
    Investment,
    /// Retirement savings, such as a 401(k) or an IRA.
    Retirement,
    /// Property, such as a house or a car.
    Asset,
    /// Debt, such as a mortgage or a loan.
    Liability,
}

impl AccountKind {
    /// Every kind, in Account bar order. A new kind goes here as well as in the
    /// matches below, which the compiler checks; it can't check this list.
    pub const ALL: [Self; 7] = [
        Self::Checking,
        Self::Savings,
        Self::CreditCard,
        Self::Investment,
        Self::Retirement,
        Self::Asset,
        Self::Liability,
    ];

    /// The group whose sub-group this kind is.
    pub fn group(self) -> AccountGroup {
        match self {
            Self::Checking | Self::Savings | Self::CreditCard => AccountGroup::Banking,
            Self::Investment | Self::Retirement => AccountGroup::Investing,
            Self::Asset | Self::Liability => AccountGroup::PropertyDebt,
        }
    }
}

/// Identifies an account for as long as it exists. Ids are never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AccountId(u32);

impl AccountId {
    pub(crate) fn new(id: u32) -> Self {
        Self(id)
    }
}

/// An account: its name, its kind, its currency, and whether it is hidden or closed.
///
/// Hidden and closed are independent. A hidden account is left out of the Account
/// bar but still counts in Net Worth. A closed account keeps its history, read-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    id: AccountId,
    name: String,
    kind: AccountKind,
    currency: CurrencyCode,
    hidden: bool,
    closed: bool,
}

impl Account {
    pub(crate) fn new(
        id: AccountId,
        name: String,
        kind: AccountKind,
        currency: CurrencyCode,
    ) -> Self {
        Self {
            id,
            name,
            kind,
            currency,
            hidden: false,
            closed: false,
        }
    }

    pub(crate) fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    pub(crate) fn set_closed(&mut self, closed: bool) {
        self.closed = closed;
    }

    pub fn id(&self) -> AccountId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> AccountKind {
        self.kind
    }

    /// The currency every amount in the account is in.
    pub fn currency(&self) -> CurrencyCode {
        self.currency
    }

    /// Whether the Account bar leaves the account out.
    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Whether the account is closed: its Register is read-only.
    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_belongs_to_its_fixed_group() {
        let groups = AccountKind::ALL.map(|kind| (kind, kind.group()));

        assert_eq!(
            groups,
            [
                (AccountKind::Checking, AccountGroup::Banking),
                (AccountKind::Savings, AccountGroup::Banking),
                (AccountKind::CreditCard, AccountGroup::Banking),
                (AccountKind::Investment, AccountGroup::Investing),
                (AccountKind::Retirement, AccountGroup::Investing),
                (AccountKind::Asset, AccountGroup::PropertyDebt),
                (AccountKind::Liability, AccountGroup::PropertyDebt),
            ]
        );
    }
}
