//! The in-memory data Balanced runs on until it reads data files.

use icu_calendar::{Date, Iso};

use crate::account::NewAccount;
use crate::category::NewCategory;
use crate::transaction::{NewTransaction, SplitLine};
use crate::{AccountKind, CategoryId, CurrencyCode, Ledger, Money};

/// How many transactions the generated account holds, after its opening balance.
const GENERATED_TRANSACTIONS: u16 = 20_000;

/// The generated account's payments, each with its category and amount. With
/// one salary of 1,000.00 they repeat in a cycle that sums to zero, so the account's
/// balance always returns to its opening balance.
const GENERATED_PAYMENTS: [(&str, StubCategory, &str); 9] = [
    ("Fresh Market", StubCategory::Groceries, "-100.00"),
    ("Shell", StubCategory::Fuel, "-50.00"),
    ("Bean There", StubCategory::CoffeeShops, "-5.00"),
    ("City Power & Light", StubCategory::Utilities, "-120.00"),
    ("Trattoria Roma", StubCategory::Restaurants, "-75.00"),
    ("Fresh Market", StubCategory::Groceries, "-150.00"),
    ("Hardware Barn", StubCategory::Household, "-200.00"),
    (
        "Safe Auto Insurance",
        StubCategory::AutoInsurance,
        "-110.00",
    ),
    ("Maple Property Management", StubCategory::Home, "-190.00"),
];

/// The stub's categories, in the order they are added, so each one's id is its
/// position. The tree is three levels deep in places, and two categories share the
/// name Insurance, so a search for a name's middle finds several.
#[derive(Debug, Clone, Copy)]
enum StubCategory {
    AutoTransport,
    Fuel,
    AutoInsurance,
    FoodDining,
    Groceries,
    Restaurants,
    CoffeeShops,
    Home,
    HomeInsurance,
    Utilities,
    Income,
    Salary,
    Interest,
    Shopping,
    Household,
}

impl StubCategory {
    const ALL: [Self; 15] = [
        Self::AutoTransport,
        Self::Fuel,
        Self::AutoInsurance,
        Self::FoodDining,
        Self::Groceries,
        Self::Restaurants,
        Self::CoffeeShops,
        Self::Home,
        Self::HomeInsurance,
        Self::Utilities,
        Self::Income,
        Self::Salary,
        Self::Interest,
        Self::Shopping,
        Self::Household,
    ];

    fn id(self) -> CategoryId {
        CategoryId::new(self as u32)
    }

    fn new_category(self) -> NewCategory {
        let (name, parent) = match self {
            Self::AutoTransport => ("Auto & Transport", None),
            Self::Fuel => ("Fuel", Some(Self::AutoTransport)),
            Self::AutoInsurance => ("Insurance", Some(Self::AutoTransport)),
            Self::FoodDining => ("Food & Dining", None),
            Self::Groceries => ("Groceries", Some(Self::FoodDining)),
            Self::Restaurants => ("Restaurants", Some(Self::FoodDining)),
            Self::CoffeeShops => ("Coffee Shops", Some(Self::Restaurants)),
            Self::Home => ("Home", None),
            Self::HomeInsurance => ("Insurance", Some(Self::Home)),
            Self::Utilities => ("Utilities", Some(Self::Home)),
            Self::Income => ("Income", None),
            Self::Salary => ("Salary", Some(Self::Income)),
            Self::Interest => ("Interest", Some(Self::Income)),
            Self::Shopping => ("Shopping", None),
            Self::Household => ("Household", Some(Self::Shopping)),
        };
        let category = NewCategory::new(name);
        match parent {
            Some(parent) => category.under(parent.id()),
            None => category,
        }
    }
}

fn date(year: i32, month: u8, day: u8) -> Date<Iso> {
    Date::try_new_iso(year, month, day).expect("stub dates are real dates")
}

impl Ledger {
    /// The in-memory data Balanced runs on until it reads data files: accounts in
    /// every sub-group, with one closed account and one hidden one, a category tree,
    /// a split transaction, transactions scheduled after mid-October 2026, and one
    /// generated account with 20,000 transactions.
    pub fn stub() -> Self {
        let usd = CurrencyCode::new("USD").expect("USD is an ISO 4217 currency");
        let money = |amount: &str| {
            let amount = amount.parse().expect("stub amounts are decimal numbers");
            Money::new(amount, usd).expect("stub amounts have two decimal places")
        };
        let opened = date(2026, 1, 1);
        let account = |name, kind, amount: &str| NewAccount::new(name, kind, opened, money(amount));
        let paid = |day, payee, category: StubCategory, amount: &str| {
            NewTransaction::new(date(2026, 9, day), payee, category.id(), money(amount))
        };
        let everyday = account("Everyday Checking", AccountKind::Checking, "1200.00")
            .with_transactions([
                paid(1, "Acme Corp", StubCategory::Salary, "3200.00"),
                paid(
                    3,
                    "Maple Property Management",
                    StubCategory::Home,
                    "-1450.00",
                )
                .number("1041")
                .memo("September rent"),
                paid(5, "Fresh Market", StubCategory::Groceries, "-86.42"),
                paid(6, "Bean There", StubCategory::CoffeeShops, "-4.75"),
                paid(10, "City Power & Light", StubCategory::Utilities, "-92.10").number("1042"),
                NewTransaction::split(
                    date(2026, 9, 12),
                    "Costco",
                    vec![
                        SplitLine::new(StubCategory::Groceries.id(), "Food", money("-142.37")),
                        SplitLine::new(
                            StubCategory::Household.id(),
                            "Paper towels, detergent",
                            money("-63.90"),
                        ),
                    ],
                ),
                paid(15, "Shell", StubCategory::Fuel, "-48.20"),
                paid(18, "Acme Corp", StubCategory::Salary, "3200.00"),
                paid(20, "Trattoria Roma", StubCategory::Restaurants, "-64.80")
                    .memo("Anniversary dinner"),
            ])
            .with_transactions(october_transactions(money));
        // Emptied, then closed: its read-only Register shows its history.
        let old_checking = NewAccount::new(
            "Old Checking",
            AccountKind::Checking,
            date(2020, 1, 1),
            money("250.00"),
        )
        .with_transactions([
            NewTransaction::new(
                date(2020, 6, 30),
                "First Savings Bank",
                StubCategory::Interest.id(),
                money("0.42"),
            ),
            NewTransaction::new(
                date(2020, 7, 15),
                "Cash withdrawal",
                StubCategory::Shopping.id(),
                money("-250.42"),
            )
            .memo("Closing the account"),
        ])
        .closed();
        Self::with_accounts(
            usd,
            StubCategory::ALL.map(StubCategory::new_category),
            [
                everyday,
                account("Joint Checking", AccountKind::Checking, "1032.00"),
                old_checking,
                generated_account(money),
                account("Emergency Fund", AccountKind::Savings, "12000.00"),
                account("Holiday Fund", AccountKind::Savings, "800.00").hidden(),
                account("Visa Rewards", AccountKind::CreditCard, "-412.30"),
                account("Brokerage", AccountKind::Investment, "25310.55"),
                account("401(k)", AccountKind::Retirement, "88420.00"),
                account("Roth IRA", AccountKind::Retirement, "31200.00"),
                account("House", AccountKind::Asset, "420000.00"),
                account("Car", AccountKind::Asset, "14500.00"),
                account("Mortgage", AccountKind::Liability, "-310500.00"),
            ],
        )
        .expect("the stub amounts are all in USD, far from overflow")
    }
}

/// Everyday Checking's transactions from mid-October 2026: one on 14 October, then
/// two scheduled for later dates.
fn october_transactions(money: impl Fn(&str) -> Money) -> [NewTransaction; 3] {
    [
        NewTransaction::new(
            date(2026, 10, 14),
            "Daily Grind",
            StubCategory::CoffeeShops.id(),
            money("-5.25"),
        ),
        NewTransaction::new(
            date(2026, 10, 20),
            "City Power & Light",
            StubCategory::Utilities.id(),
            money("-95.40"),
        )
        .number("1043"),
        NewTransaction::new(
            date(2026, 11, 1),
            "Maple Property Management",
            StubCategory::Home.id(),
            money("-1475.00"),
        )
        .number("1044")
        .memo("November rent"),
    ]
}

/// An account with [`GENERATED_TRANSACTIONS`] transactions, four a day, so the
/// Register has years of history to scroll. It opens with 500.00 and ends there.
fn generated_account(money: impl Fn(&str) -> Money) -> NewAccount {
    let opened = date(2013, 1, 1);
    let first_day = opened.to_rata_die();
    let transactions = (0..GENERATED_TRANSACTIONS).map(|i| {
        let day = Date::from_rata_die(first_day + i64::from(i / 4), Iso);
        let step = usize::from(i) % (GENERATED_PAYMENTS.len() + 1);
        match GENERATED_PAYMENTS.get(step) {
            Some(&(payee, category, amount)) => {
                NewTransaction::new(day, payee, category.id(), money(amount))
            }
            None => NewTransaction::new(
                day,
                "Acme Corp",
                StubCategory::Salary.id(),
                money("1000.00"),
            ),
        }
    });
    NewAccount::new(
        "Household Checking",
        AccountKind::Checking,
        opened,
        money("500.00"),
    )
    .with_transactions(transactions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_generated_account_has_four_transactions_a_day_and_ends_where_it_opened() {
        let ledger = Ledger::stub();

        let view = ledger.snapshot();

        let generated = view
            .accounts()
            .iter()
            .find(|summary| summary.account().name() == "Household Checking")
            .unwrap();
        let register = generated.register();
        let dates = |index: usize| register[index].transaction().date();
        // The opening balance, then 20,000 transactions over 5,000 days.
        assert_eq!(register.len(), 20_001);
        assert_eq!((dates(0), dates(4)), (date(2013, 1, 1), date(2013, 1, 1)));
        assert_eq!(dates(5), date(2013, 1, 2));
        assert_eq!(dates(20_000), date(2026, 9, 9));
        let usd = CurrencyCode::new("USD").unwrap();
        let five_hundred = Money::new("500.00".parse().unwrap(), usd).unwrap();
        assert_eq!(generated.balance(), five_hundred);
    }
}
