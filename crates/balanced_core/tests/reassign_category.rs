// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Reassigning a category: `DomainCommand::ReassignCategory`, which moves every
//! transaction and split line in one category to another, and its undo.

use std::path::PathBuf;

use balanced_core::{
    AccountId, AccountKind, Allocation, CategoryId, CategoryType, CategoryUse, CurrencyCode,
    DomainCommand, DomainError, Ledger, Money, NewSplitLine, NewTransaction, SplitTarget,
    TransactionId,
};
use icu_calendar::{Date, Iso};
use tempfile::TempDir;

fn usd(amount: &str) -> Money {
    let currency = CurrencyCode::new("USD").expect("USD is an ISO 4217 currency");
    let amount = amount.parse().expect("test amounts are decimal numbers");
    Money::new(amount, currency).expect("test amounts have two decimal places")
}

fn date(day: u8) -> Date<Iso> {
    Date::try_new_iso(2026, 9, day).expect("test dates are real dates")
}

/// A household in a new data file, with Groceries, Dining and Cleaning, and Checking,
/// opened on 1 September holding 1000.00, which holds three payments: Costco on
/// Groceries, a Target split between Groceries and Cleaning, and Bistro on Dining.
struct Household {
    /// Holds the file until the test ends.
    _dir: TempDir,
    path: PathBuf,
    ledger: Ledger,
    checking: AccountId,
    groceries: CategoryId,
    dining: CategoryId,
    cleaning: CategoryId,
}

impl Household {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("the temp dir is writable");
        let path = dir.path().join("Household.balanced");
        let currency = CurrencyCode::new("USD").expect("USD is an ISO 4217 currency");
        let mut ledger = Ledger::create(&path, currency).expect("a new path takes a data file");
        apply(
            &mut ledger,
            DomainCommand::CreateAccount {
                name: "Checking".to_owned(),
                kind: AccountKind::Checking,
                opened: date(1),
                opening_balance: usd("1000.00"),
            },
        );
        for name in ["Groceries", "Dining", "Cleaning"] {
            apply(
                &mut ledger,
                DomainCommand::CreateCategory {
                    name: name.to_owned(),
                    parent: None,
                    category_type: CategoryType::Expense,
                },
            );
        }
        let checking = ledger.snapshot().accounts()[0].account().id();
        let category = |name| category_id(&ledger, name);
        let (groceries, dining, cleaning) = (
            category("Groceries"),
            category("Dining"),
            category("Cleaning"),
        );
        for transaction in [
            NewTransaction::new(date(5), "Costco", groceries, usd("-100.00")),
            NewTransaction::split(
                date(6),
                "Target",
                vec![
                    NewSplitLine::category(groceries, "Food", usd("-40.00")),
                    NewSplitLine::category(cleaning, "Soap", usd("-10.00")),
                ],
            ),
            NewTransaction::new(date(7), "Bistro", dining, usd("-60.00")),
        ] {
            apply(
                &mut ledger,
                DomainCommand::PostTransaction {
                    account: checking,
                    transaction,
                },
            );
        }
        Self {
            _dir: dir,
            path,
            ledger,
            checking,
            groceries,
            dining,
            cleaning,
        }
    }

    fn apply(&mut self, command: DomainCommand) {
        apply(&mut self.ledger, command);
    }

    /// The household as its file holds it, opened afresh.
    fn reopened(self) -> Self {
        let Self {
            _dir,
            path,
            ledger,
            checking,
            groceries,
            dining,
            cleaning,
        } = self;
        drop(ledger);
        let ledger = Ledger::open(&path).expect("the file the ledger wrote opens");
        Self {
            _dir,
            path,
            ledger,
            checking,
            groceries,
            dining,
            cleaning,
        }
    }

    /// The id of the payment to `payee`.
    fn transaction(&self, payee: &str) -> TransactionId {
        self.ledger
            .snapshot()
            .account(self.checking)
            .expect("Checking stays in the ledger")
            .register()
            .iter()
            .map(|entry| entry.transaction())
            .find(|transaction| transaction.payee() == payee)
            .expect("the household paid the payee")
            .id()
    }

    /// Each payment's payee with where its amount went: one category's name, or each
    /// split line's category name.
    fn targets(&self) -> Vec<(String, Vec<String>)> {
        let view = self.ledger.snapshot();
        let name = |id| {
            view.category(id)
                .expect("the payments name categories the ledger holds")
                .name()
                .to_owned()
        };
        view.account(self.checking)
            .expect("Checking stays in the ledger")
            .register()
            .iter()
            .map(|entry| entry.transaction())
            .filter_map(|transaction| {
                let names = match transaction.allocation() {
                    Allocation::OpeningBalance => return None,
                    Allocation::Category(category) => vec![name(*category)],
                    Allocation::Split(lines) => lines
                        .iter()
                        .map(|line| match line.target() {
                            SplitTarget::Category(category) => name(category),
                            SplitTarget::Transfer(_) => panic!("the payments hold no transfer"),
                        })
                        .collect(),
                    Allocation::Transfer(_) => panic!("the payments hold no transfer"),
                };
                Some((transaction.payee().to_owned(), names))
            })
            .collect()
    }
}

fn apply(ledger: &mut Ledger, command: DomainCommand) {
    ledger.apply(command).expect("the test's command applies");
}

fn category_id(ledger: &Ledger, name: &str) -> CategoryId {
    ledger
        .snapshot()
        .categories()
        .iter()
        .find(|category| category.name() == name)
        .expect("the test created the category")
        .id()
}

/// `(payee, category names)` pairs, from string literals.
fn expected(targets: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
    targets
        .iter()
        .map(|(payee, names)| {
            (
                (*payee).to_owned(),
                names.iter().map(|&name| name.to_owned()).collect(),
            )
        })
        .collect()
}

#[test]
fn reassigning_moves_whole_transactions_and_split_lines() {
    let mut household = Household::new();

    let (groceries, dining) = (household.groceries, household.dining);
    household
        .ledger
        .apply(DomainCommand::ReassignCategory {
            from: groceries,
            to: dining,
        })
        .unwrap();

    assert_eq!(
        household.targets(),
        expected(&[
            ("Costco", &["Dining"]),
            ("Target", &["Dining", "Cleaning"]),
            ("Bistro", &["Dining"]),
        ])
    );
}

#[test]
fn undoing_a_reassign_puts_back_only_what_it_moved() {
    let mut household = Household::new();
    let (groceries, dining) = (household.groceries, household.dining);
    household.apply(DomainCommand::ReassignCategory {
        from: groceries,
        to: dining,
    });

    household.ledger.undo().unwrap();

    assert_eq!(
        household.targets(),
        expected(&[
            ("Costco", &["Groceries"]),
            ("Target", &["Groceries", "Cleaning"]),
            ("Bistro", &["Dining"]),
        ])
    );
}

#[test]
fn a_reassign_is_kept_in_the_file() {
    let mut household = Household::new();
    let (groceries, dining) = (household.groceries, household.dining);

    household.apply(DomainCommand::ReassignCategory {
        from: groceries,
        to: dining,
    });

    assert_eq!(
        household.reopened().targets(),
        expected(&[
            ("Costco", &["Dining"]),
            ("Target", &["Dining", "Cleaning"]),
            ("Bistro", &["Dining"]),
        ])
    );
}

#[test]
fn a_reassign_undoes_once_the_file_is_reopened() {
    let mut household = Household::new();
    let (groceries, dining) = (household.groceries, household.dining);
    household.apply(DomainCommand::ReassignCategory {
        from: groceries,
        to: dining,
    });
    let mut household = household.reopened();

    household.ledger.undo().unwrap();

    assert_eq!(
        household.reopened().targets(),
        expected(&[
            ("Costco", &["Groceries"]),
            ("Target", &["Groceries", "Cleaning"]),
            ("Bistro", &["Dining"]),
        ])
    );
}

#[test]
fn a_category_is_not_reassigned_to_itself() {
    let mut household = Household::new();
    let groceries = household.groceries;
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(DomainCommand::ReassignCategory {
        from: groceries,
        to: groceries,
    });

    assert_eq!(
        result,
        Err(DomainError::ReassignIntoItself {
            category: groceries
        })
    );
    assert_eq!(household.ledger.snapshot(), before);
}

#[test]
fn a_category_is_not_reassigned_to_one_under_it() {
    let mut household = Household::new();
    let groceries = household.groceries;
    household.apply(DomainCommand::CreateCategory {
        name: "Bulk".to_owned(),
        parent: Some(groceries),
        category_type: CategoryType::Expense,
    });
    let bulk = category_id(&household.ledger, "Bulk");
    household.apply(DomainCommand::CreateCategory {
        name: "Club".to_owned(),
        parent: Some(bulk),
        category_type: CategoryType::Expense,
    });
    let club = category_id(&household.ledger, "Club");
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(DomainCommand::ReassignCategory {
        from: groceries,
        to: club,
    });

    assert_eq!(
        result,
        Err(DomainError::ReassignIntoItself {
            category: groceries
        })
    );
    assert_eq!(household.ledger.snapshot(), before);
}

#[test]
fn a_use_not_in_the_category_is_not_reassigned() {
    let mut household = Household::new();
    let (dining, cleaning) = (household.dining, household.cleaning);
    let bistro = household.transaction("Bistro");
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(DomainCommand::ReassignUses {
        from: cleaning,
        to: dining,
        uses: vec![CategoryUse::Transaction(bistro)],
    });

    assert_eq!(
        result,
        Err(DomainError::NotInCategory {
            transaction: bistro,
            category: cleaning
        })
    );
    assert_eq!(household.ledger.snapshot(), before);
}

#[test]
fn a_parent_reassigned_and_deleted_with_its_subcategory_comes_back_in_one_undo() {
    let mut household = Household::new();
    let (groceries, dining, checking) = (household.groceries, household.dining, household.checking);
    household.apply(DomainCommand::CreateCategory {
        name: "Bulk".to_owned(),
        parent: Some(groceries),
        category_type: CategoryType::Expense,
    });
    let bulk = category_id(&household.ledger, "Bulk");
    household.apply(DomainCommand::PostTransaction {
        account: checking,
        transaction: NewTransaction::new(date(8), "Warehouse", bulk, usd("-80.00")),
    });
    let categories = household.ledger.snapshot().categories().to_vec();
    household.apply(DomainCommand::Compound(vec![
        DomainCommand::ReassignCategory {
            from: bulk,
            to: dining,
        },
        DomainCommand::ReassignCategory {
            from: groceries,
            to: dining,
        },
        DomainCommand::DeleteCategory { category: bulk },
        DomainCommand::DeleteCategory {
            category: groceries,
        },
    ]));

    household.ledger.undo().unwrap();

    let household = household.reopened();
    assert_eq!(household.ledger.snapshot().categories(), categories);
    assert_eq!(
        household.targets(),
        expected(&[
            ("Costco", &["Groceries"]),
            ("Target", &["Groceries", "Cleaning"]),
            ("Bistro", &["Dining"]),
            ("Warehouse", &["Bulk"]),
        ])
    );
}
