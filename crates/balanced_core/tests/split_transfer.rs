// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Transfers in split lines: a split line to `[Account]` and the other side it records
//! in that account.

use std::path::PathBuf;

use balanced_core::{
    AccountId, AccountKind, AccountStatus, AccountSummary, Allocation, CategoryId, CategoryType,
    ClearedStatus, CurrencyCode, DomainCommand, DomainError, Ledger, Money, NewSplitLine,
    NewTransaction, SplitLine, SplitTarget, Transaction, TransactionEdit, TransactionId,
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

/// A household with a Groceries category and two accounts, Checking and Savings, each
/// opened on 1 September holding 1000.00, in a new data file.
struct Household {
    /// Holds the file until the test ends.
    dir: TempDir,
    path: PathBuf,
    ledger: Ledger,
    checking: AccountId,
    savings: AccountId,
    groceries: CategoryId,
}

impl Household {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("the temp dir is writable");
        let path = dir.path().join("Household.balanced");
        let currency = CurrencyCode::new("USD").expect("USD is an ISO 4217 currency");
        let mut ledger = Ledger::create(&path, currency).expect("a new path takes a data file");
        for (name, kind) in [
            ("Checking", AccountKind::Checking),
            ("Savings", AccountKind::Savings),
        ] {
            apply(
                &mut ledger,
                DomainCommand::CreateAccount {
                    name: name.to_owned(),
                    kind,
                    opened: date(1),
                    opening_balance: usd("1000.00"),
                },
            );
        }
        apply(
            &mut ledger,
            DomainCommand::CreateCategory {
                name: "Groceries".to_owned(),
                parent: None,
                category_type: CategoryType::Expense,
            },
        );
        let checking = account_id(&ledger, "Checking");
        let savings = account_id(&ledger, "Savings");
        let groceries = ledger
            .snapshot()
            .categories()
            .first()
            .expect("the test created Groceries")
            .id();
        Self {
            dir,
            path,
            ledger,
            checking,
            savings,
            groceries,
        }
    }

    fn apply(&mut self, command: DomainCommand) {
        apply(&mut self.ledger, command);
    }

    /// Posts a split of 350.00 from Checking on 5 September, returning its id: 50.00 on
    /// Groceries, then 200.00 and 100.00 to Savings, each its own line.
    fn save_from_a_split(&mut self) -> TransactionId {
        let (savings, groceries) = (self.savings, self.groceries);
        self.apply(DomainCommand::PostTransaction {
            account: self.checking,
            transaction: NewTransaction::split(
                date(5),
                "Payday",
                vec![
                    NewSplitLine::category(groceries, "Food", usd("-50.00")),
                    NewSplitLine::transfer(savings, "Rainy day", usd("-200.00")),
                    NewSplitLine::transfer(savings, "Holiday", usd("-100.00")),
                ],
            ),
        });
        only(self.posted(self.checking)).id()
    }

    fn summary(&self, account: AccountId) -> AccountSummary {
        self.ledger
            .snapshot()
            .account(account)
            .expect("the test's accounts stay in the ledger")
            .clone()
    }

    /// Every transaction in `account`'s register but the opening balance.
    fn posted(&self, account: AccountId) -> Vec<Transaction> {
        self.summary(account)
            .register()
            .iter()
            .map(|entry| entry.transaction().clone())
            .filter(|transaction| *transaction.allocation() != Allocation::OpeningBalance)
            .collect()
    }

    fn balance(&self, account: AccountId) -> Money {
        self.summary(account).balance()
    }

    /// The household as its file holds it, opened afresh.
    fn reopened(self) -> Self {
        let Self {
            dir,
            path,
            ledger,
            checking,
            savings,
            groceries,
        } = self;
        drop(ledger);
        let ledger = Ledger::open(&path).expect("the file the ledger wrote opens");
        Self {
            dir,
            path,
            ledger,
            checking,
            savings,
            groceries,
        }
    }
}

fn apply(ledger: &mut Ledger, command: DomainCommand) {
    ledger.apply(command).expect("the test's command applies");
}

fn account_id(ledger: &Ledger, name: &str) -> AccountId {
    ledger
        .snapshot()
        .accounts()
        .iter()
        .map(AccountSummary::account)
        .find(|account| account.name() == name)
        .expect("the test created the account")
        .id()
}

/// The one transaction `transactions` holds.
fn only(transactions: Vec<Transaction>) -> Transaction {
    let [transaction] = <[Transaction; 1]>::try_from(transactions)
        .expect("the register holds one transaction besides its opening balance");
    transaction
}

#[test]
fn a_split_records_an_other_side_for_each_line_to_an_account() {
    let mut household = Household::new();

    let split = household.save_from_a_split();

    let savings = household.posted(household.savings);
    let other_sides: Vec<_> = savings
        .iter()
        .map(|side| {
            (
                side.allocation().clone(),
                side.date(),
                side.payee(),
                side.memo(),
                side.amount(),
            )
        })
        .collect();
    assert_eq!(
        other_sides,
        [
            (
                Allocation::Transfer(split),
                date(5),
                "Payday",
                "Rainy day",
                usd("200.00")
            ),
            (
                Allocation::Transfer(split),
                date(5),
                "Payday",
                "Holiday",
                usd("100.00")
            ),
        ]
    );
    let Allocation::Split(lines) = only(household.posted(household.checking))
        .allocation()
        .clone()
    else {
        panic!("the split stays a split");
    };
    let targets: Vec<_> = lines.iter().map(SplitLine::target).collect();
    assert_eq!(
        targets,
        [
            SplitTarget::Category(household.groceries),
            SplitTarget::Transfer(savings[0].id()),
            SplitTarget::Transfer(savings[1].id()),
        ]
    );
    assert_eq!(household.balance(household.checking), usd("650.00"));
    assert_eq!(household.balance(household.savings), usd("1300.00"));
}

#[test]
fn a_split_and_its_lines_other_sides_survive_a_reopen() {
    let mut household = Household::new();
    household.save_from_a_split();
    let live = household.ledger.snapshot();

    let household = household.reopened();

    assert_eq!(household.ledger.snapshot().accounts(), live.accounts());
}

/// Posting to Checking a split with a line of 50.00 on Groceries and one of 200.00 to
/// `account`.
fn post_split_to(household: &mut Household, account: AccountId) -> Result<(), DomainError> {
    let groceries = household.groceries;
    household
        .ledger
        .apply(DomainCommand::PostTransaction {
            account: household.checking,
            transaction: NewTransaction::split(
                date(5),
                "Payday",
                vec![
                    NewSplitLine::category(groceries, "Food", usd("-50.00")),
                    NewSplitLine::transfer(account, "Rainy day", usd("-200.00")),
                ],
            ),
        })
        .map(drop)
}

#[test]
fn a_split_line_to_the_splits_own_account_is_refused() {
    let mut household = Household::new();
    let before = household.ledger.snapshot();
    let checking = household.checking;

    let result = post_split_to(&mut household, checking);

    assert_eq!(
        result.unwrap_err(),
        DomainError::TransferToOwnAccount { account: checking }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn a_split_line_to_a_closed_account_is_refused_naming_it() {
    let mut household = Household::new();
    let savings = household.savings;
    household.apply(DomainCommand::SetAccountStatus {
        account: savings,
        status: AccountStatus::Closed,
    });
    let before = household.ledger.snapshot();

    let result = post_split_to(&mut household, savings);

    assert_eq!(
        result.unwrap_err(),
        DomainError::TransferAccountClosed { account: savings }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn a_date_or_amount_change_on_a_split_lines_other_side_is_refused_naming_the_splits_account() {
    for edit in [
        TransactionEdit::Date(date(9)),
        TransactionEdit::Amount(usd("250.00")),
    ] {
        let mut household = Household::new();
        household.save_from_a_split();
        let other_side = household.posted(household.savings)[0].id();
        let before = household.ledger.snapshot();

        let result = household.ledger.apply(DomainCommand::EditTransaction {
            transaction: other_side,
            edit,
        });

        assert_eq!(
            result.unwrap_err(),
            DomainError::SplitLineOtherSide {
                account: household.checking
            }
        );
        assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
    }
}

#[test]
fn a_target_change_on_a_split_lines_other_side_is_refused_naming_the_splits_account() {
    let household = Household::new();
    for edit in [
        TransactionEdit::Category(household.groceries),
        TransactionEdit::Transfer(household.checking),
    ] {
        let mut household = Household::new();
        household.save_from_a_split();
        let other_side = household.posted(household.savings)[0].id();
        let before = household.ledger.snapshot();

        let result = household.ledger.apply(DomainCommand::EditTransaction {
            transaction: other_side,
            edit,
        });

        assert_eq!(
            result.unwrap_err(),
            DomainError::SplitLineOtherSide {
                account: household.checking
            }
        );
        assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
    }
}

#[test]
fn deleting_a_split_lines_other_side_is_refused_naming_the_splits_account() {
    let mut household = Household::new();
    household.save_from_a_split();
    let other_side = household.posted(household.savings)[0].id();
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(DomainCommand::DeleteTransaction {
        transaction: other_side,
    });

    assert_eq!(
        result.unwrap_err(),
        DomainError::SplitLineOtherSide {
            account: household.checking
        }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn a_split_lines_other_side_keeps_its_own_payee_memo_number_status_and_flag() {
    let mut household = Household::new();
    let split = household.save_from_a_split();
    let other_side = household.posted(household.savings)[0].id();
    let split_before = only(household.posted(household.checking));

    for edit in [
        TransactionEdit::Payee("Employer".to_owned()),
        TransactionEdit::Memo("Emergency fund".to_owned()),
        TransactionEdit::Number(Some("77".to_owned())),
        TransactionEdit::ClearedStatus(ClearedStatus::Cleared),
    ] {
        household.apply(DomainCommand::EditTransaction {
            transaction: other_side,
            edit,
        });
    }
    household.apply(DomainCommand::SetFlagged {
        transaction: other_side,
        flagged: true,
    });

    let edited = &household.posted(household.savings)[0];
    assert_eq!(
        (
            edited.payee(),
            edited.memo(),
            edited.number(),
            edited.cleared_status(),
            edited.is_flagged()
        ),
        (
            "Employer",
            "Emergency fund",
            Some("77"),
            ClearedStatus::Cleared,
            true
        )
    );
    assert_eq!(edited.allocation(), &Allocation::Transfer(split));
    assert_eq!(only(household.posted(household.checking)), split_before);
}

/// The date of each transaction `account` holds besides its opening balance.
fn dates(household: &Household, account: AccountId) -> Vec<Date<Iso>> {
    household
        .posted(account)
        .iter()
        .map(Transaction::date)
        .collect()
}

#[test]
fn changing_a_splits_date_moves_every_lines_other_side_and_undo_moves_them_back() {
    let mut household = Household::new();
    let split = household.save_from_a_split();

    household.apply(DomainCommand::EditTransaction {
        transaction: split,
        edit: TransactionEdit::Date(date(9)),
    });
    let moved = dates(&household, household.savings);
    household.ledger.undo().expect("the edit undoes");

    assert_eq!(moved, [date(9), date(9)]);
    assert_eq!(dates(&household, household.savings), [date(5), date(5)]);
}

#[test]
fn deleting_a_split_deletes_every_lines_other_side() {
    let mut household = Household::new();
    let split = household.save_from_a_split();

    household.apply(DomainCommand::DeleteTransaction { transaction: split });

    assert_eq!(household.posted(household.checking), []);
    assert_eq!(household.posted(household.savings), []);
    assert_eq!(household.balance(household.savings), usd("1000.00"));
}

#[test]
fn one_undo_puts_back_a_deleted_split_and_its_lines_other_sides_with_their_ids() {
    let mut household = Household::new();
    let split = household.save_from_a_split();
    let before = household.ledger.snapshot();
    household.apply(DomainCommand::DeleteTransaction { transaction: split });

    household.ledger.undo().expect("the delete undoes");

    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn undo_after_a_reopen_puts_back_a_deleted_split_and_its_lines_other_sides() {
    let mut household = Household::new();
    let split = household.save_from_a_split();
    let before = household.ledger.snapshot();
    household.apply(DomainCommand::DeleteTransaction { transaction: split });
    let mut household = household.reopened();

    household.ledger.undo().expect("the delete undoes");

    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn deleting_a_split_whose_line_goes_to_a_closed_account_is_refused_naming_it() {
    let mut household = Household::new();
    let split = household.save_from_a_split();
    let savings = household.savings;
    household.apply(DomainCommand::SetAccountStatus {
        account: savings,
        status: AccountStatus::Closed,
    });
    let before = household.ledger.snapshot();

    let result = household
        .ledger
        .apply(DomainCommand::DeleteTransaction { transaction: split });

    assert_eq!(
        result.unwrap_err(),
        DomainError::TransferAccountClosed { account: savings }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

/// Asserts that deleting `account` from a household holding a split with lines to
/// Savings is refused, naming it, and changes nothing.
fn assert_account_with_split_lines_is_kept(account: impl Fn(&Household) -> AccountId) {
    let mut household = Household::new();
    household.save_from_a_split();
    let account = account(&household);
    let before = household.ledger.snapshot();

    let result = household
        .ledger
        .apply(DomainCommand::DeleteAccount { account });

    assert_eq!(
        result.expect_err("the ledger keeps the account"),
        DomainError::AccountHasTransfers { account }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn deleting_the_account_holding_a_split_with_a_line_to_another_is_refused_naming_it() {
    assert_account_with_split_lines_is_kept(|household| household.checking);
}

#[test]
fn deleting_an_account_holding_a_split_lines_other_side_is_refused_naming_it() {
    assert_account_with_split_lines_is_kept(|household| household.savings);
}

#[test]
fn changing_a_splits_date_when_a_lines_account_is_closed_is_refused_naming_it() {
    let mut household = Household::new();
    let split = household.save_from_a_split();
    let savings = household.savings;
    household.apply(DomainCommand::SetAccountStatus {
        account: savings,
        status: AccountStatus::Closed,
    });
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(DomainCommand::EditTransaction {
        transaction: split,
        edit: TransactionEdit::Date(date(9)),
    });

    assert_eq!(
        result.unwrap_err(),
        DomainError::TransferAccountClosed { account: savings }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn redo_moves_a_splits_lines_other_sides_again() {
    let mut household = Household::new();
    let split = household.save_from_a_split();
    household.apply(DomainCommand::EditTransaction {
        transaction: split,
        edit: TransactionEdit::Date(date(9)),
    });
    household.ledger.undo().expect("the edit undoes");

    household.ledger.redo().expect("the edit redoes");

    assert_eq!(dates(&household, household.savings), [date(9), date(9)]);
}

#[test]
fn redo_deletes_an_undeleted_split_and_its_lines_other_sides_again() {
    let mut household = Household::new();
    let split = household.save_from_a_split();
    household.apply(DomainCommand::DeleteTransaction { transaction: split });
    household.ledger.undo().expect("the delete undoes");

    household.ledger.redo().expect("the delete redoes");

    assert_eq!(household.posted(household.checking), []);
    assert_eq!(household.posted(household.savings), []);
}
