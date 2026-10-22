// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Editing a transaction's split lines: `TransactionEdit::Lines`, which replaces every
//! line at once, with the other side of each line to an account.

use std::path::PathBuf;

use balanced_core::{
    AccountId, AccountKind, AccountStatus, AccountSummary, Allocation, CategoryId, CategoryType,
    CurrencyCode, DomainCommand, DomainError, EditedSplitLine, Ledger, Money, NewSplitLine,
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

/// A household with Groceries and Cleaning categories and two accounts, Checking and
/// Savings, each opened on 1 September holding 1000.00, in a new data file.
struct Household {
    /// Holds the file until the test ends.
    dir: TempDir,
    path: PathBuf,
    ledger: Ledger,
    checking: AccountId,
    savings: AccountId,
    groceries: CategoryId,
    cleaning: CategoryId,
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
        for name in ["Groceries", "Cleaning"] {
            apply(
                &mut ledger,
                DomainCommand::CreateCategory {
                    name: name.to_owned(),
                    parent: None,
                    category_type: CategoryType::Expense,
                },
            );
        }
        let checking = account_id(&ledger, "Checking");
        let savings = account_id(&ledger, "Savings");
        let category = |name| {
            ledger
                .snapshot()
                .categories()
                .iter()
                .find(|category| category.name() == name)
                .expect("the test created the category")
                .id()
        };
        let (groceries, cleaning) = (category("Groceries"), category("Cleaning"));
        Self {
            dir,
            path,
            ledger,
            checking,
            savings,
            groceries,
            cleaning,
        }
    }

    fn apply(&mut self, command: DomainCommand) {
        apply(&mut self.ledger, command);
    }

    /// Posts a payment of 100.00 from Checking to Costco on 5 September, all on
    /// Groceries, returning its id.
    fn shop(&mut self) -> TransactionId {
        self.apply(DomainCommand::PostTransaction {
            account: self.checking,
            transaction: NewTransaction::new(date(5), "Costco", self.groceries, usd("-100.00")),
        });
        only(self.posted(self.checking)).id()
    }

    /// Posts a split of 350.00 from Checking to Payday on 5 September, returning its
    /// id: 50.00 on Groceries, then 200.00 and 100.00 to Savings, each its own line.
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

    /// The ids of the other sides in Savings of Payday's lines, Rainy day then Holiday.
    fn payday_sides(&self) -> [TransactionId; 2] {
        let ids: Vec<_> = self
            .posted(self.savings)
            .iter()
            .map(Transaction::id)
            .collect();
        <[TransactionId; 2]>::try_from(ids).expect("Savings holds Payday's two other sides")
    }

    /// Gives `transaction` `lines`, totalling `amount`.
    fn split(&mut self, transaction: TransactionId, amount: &str, lines: Vec<EditedSplitLine>) {
        self.apply(lines_edit(transaction, amount, lines));
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
            cleaning,
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
            cleaning,
        }
    }
}

fn apply(ledger: &mut Ledger, command: DomainCommand) {
    ledger.apply(command).expect("the test's command applies");
}

/// The Command giving `transaction` `lines`, totalling `amount`.
fn lines_edit(
    transaction: TransactionId,
    amount: &str,
    lines: Vec<EditedSplitLine>,
) -> DomainCommand {
    DomainCommand::EditTransaction {
        transaction,
        edit: TransactionEdit::Lines {
            amount: usd(amount),
            lines,
        },
    }
}

fn account_id(ledger: &Ledger, name: &str) -> AccountId {
    ledger
        .snapshot()
        .accounts()
        .iter()
        .map(|summary| summary.account())
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

/// The lines of split `transaction`, as target, memo and amount.
fn lines(transaction: &Transaction) -> Vec<(SplitTarget, String, Money)> {
    let Allocation::Split(lines) = transaction.allocation() else {
        panic!("the transaction is split: {:?}", transaction.allocation());
    };
    lines
        .iter()
        .map(|line: &SplitLine| (line.target(), line.memo().to_owned(), line.amount()))
        .collect()
}

#[test]
fn lines_split_a_transaction_in_one_category_whose_amount_is_their_sum() {
    let mut household = Household::new();
    let costco = household.shop();
    let (groceries, cleaning) = (household.groceries, household.cleaning);

    household.split(
        costco,
        "-100.00",
        vec![
            EditedSplitLine::category(groceries, "Food", usd("-80.00")),
            EditedSplitLine::category(cleaning, "Soap", usd("-20.00")),
        ],
    );

    let split = only(household.posted(household.checking));
    assert_eq!(
        lines(&split),
        [
            (
                SplitTarget::Category(groceries),
                "Food".to_owned(),
                usd("-80.00")
            ),
            (
                SplitTarget::Category(cleaning),
                "Soap".to_owned(),
                usd("-20.00")
            ),
        ]
    );
    assert_eq!(split.amount(), usd("-100.00"));
    assert_eq!(household.balance(household.checking), usd("900.00"));
}

/// Savings' transactions besides its opening balance, as what each points back at,
/// memo and amount.
fn other_sides(household: &Household) -> Vec<(Allocation, String, Money)> {
    household
        .posted(household.savings)
        .iter()
        .map(|side| {
            (
                side.allocation().clone(),
                side.memo().to_owned(),
                side.amount(),
            )
        })
        .collect()
}

#[test]
fn a_kept_lines_other_side_keeps_its_id_and_own_memo_and_takes_the_lines_new_amount() {
    let mut household = Household::new();
    let payday = household.save_from_a_split();
    let [rainy_day, holiday] = household.payday_sides();
    let groceries = household.groceries;

    household.split(
        payday,
        "-350.00",
        vec![
            EditedSplitLine::category(groceries, "Food", usd("-100.00")),
            EditedSplitLine::kept(rainy_day, "Emergencies", usd("-150.00")),
            EditedSplitLine::kept(holiday, "Holiday", usd("-100.00")),
        ],
    );

    assert_eq!(household.payday_sides(), [rainy_day, holiday]);
    assert_eq!(
        other_sides(&household),
        [
            (
                Allocation::Transfer(payday),
                "Rainy day".to_owned(),
                usd("150.00")
            ),
            (
                Allocation::Transfer(payday),
                "Holiday".to_owned(),
                usd("100.00")
            ),
        ]
    );
    assert_eq!(household.balance(household.savings), usd("1250.00"));
}

#[test]
fn a_kept_line_whose_amount_changes_sign_flips_its_other_sides_sign() {
    let mut household = Household::new();
    let payday = household.save_from_a_split();
    let [rainy_day, holiday] = household.payday_sides();
    let groceries = household.groceries;

    household.split(
        payday,
        "50.00",
        vec![
            EditedSplitLine::category(groceries, "Food", usd("-50.00")),
            EditedSplitLine::kept(rainy_day, "Rainy day", usd("200.00")),
            EditedSplitLine::kept(holiday, "Holiday", usd("-100.00")),
        ],
    );

    assert_eq!(household.payday_sides(), [rainy_day, holiday]);
    assert_eq!(
        other_sides(&household),
        [
            (
                Allocation::Transfer(payday),
                "Rainy day".to_owned(),
                usd("-200.00")
            ),
            (
                Allocation::Transfer(payday),
                "Holiday".to_owned(),
                usd("100.00")
            ),
        ]
    );
    assert_eq!(household.balance(household.savings), usd("900.00"));
}

#[test]
fn a_line_to_an_account_the_edit_drops_loses_its_other_side() {
    let mut household = Household::new();
    let payday = household.save_from_a_split();
    let [rainy_day, _] = household.payday_sides();
    let groceries = household.groceries;

    household.split(
        payday,
        "-350.00",
        vec![
            EditedSplitLine::category(groceries, "Food", usd("-150.00")),
            EditedSplitLine::kept(rainy_day, "Rainy day", usd("-200.00")),
        ],
    );

    let ids: Vec<_> = household
        .posted(household.savings)
        .iter()
        .map(Transaction::id)
        .collect();
    assert_eq!(ids, [rainy_day]);
    assert_eq!(household.balance(household.savings), usd("1200.00"));
}

#[test]
fn a_new_line_to_an_account_gains_an_other_side_there_taking_the_lines_memo() {
    let mut household = Household::new();
    let costco = household.shop();
    let (groceries, savings) = (household.groceries, household.savings);

    household.split(
        costco,
        "-100.00",
        vec![
            EditedSplitLine::category(groceries, "Food", usd("-60.00")),
            EditedSplitLine::transfer(savings, "Change jar", usd("-40.00")),
        ],
    );

    let [side] = <[Transaction; 1]>::try_from(household.posted(savings))
        .expect("the new line has one other side");
    assert_eq!(
        (
            side.allocation(),
            side.date(),
            side.payee(),
            side.memo(),
            side.amount()
        ),
        (
            &Allocation::Transfer(costco),
            date(5),
            "Costco",
            "Change jar",
            usd("40.00")
        )
    );
    assert_eq!(
        lines(&only(household.posted(household.checking)))[1].0,
        SplitTarget::Transfer(side.id())
    );
}

#[test]
fn a_new_lines_other_side_takes_a_later_id_than_the_transfers_other_side_it_splits() {
    let mut household = Household::new();
    let (groceries, checking, savings) =
        (household.groceries, household.checking, household.savings);
    household.apply(DomainCommand::PostTransaction {
        account: checking,
        transaction: NewTransaction::transfer(date(6), "Saving up", savings, usd("-300.00")),
    });
    let received = only(household.posted(savings)).id();

    household.split(
        received,
        "300.00",
        vec![
            EditedSplitLine::transfer(checking, "Saving up", usd("250.00")),
            EditedSplitLine::category(groceries, "Refund", usd("50.00")),
        ],
    );

    let new_side = only(household.posted(checking)).id();
    assert!(new_side > received, "{new_side:?} after {received:?}");
}

#[test]
fn a_replaced_split_has_exactly_one_other_side_for_each_line_to_an_account() {
    let mut household = Household::new();
    let payday = household.save_from_a_split();
    let [_, holiday] = household.payday_sides();
    let savings = household.savings;

    household.split(
        payday,
        "-350.00",
        vec![
            EditedSplitLine::transfer(savings, "Car", usd("-250.00")),
            EditedSplitLine::kept(holiday, "Holiday", usd("-100.00")),
        ],
    );

    let car = household
        .posted(savings)
        .iter()
        .map(Transaction::id)
        .find(|&side| side != holiday)
        .expect("the new line has an other side");
    let targets: Vec<_> = lines(&only(household.posted(household.checking)))
        .into_iter()
        .map(|(target, _, _)| target)
        .collect();
    assert_eq!(
        targets,
        [SplitTarget::Transfer(car), SplitTarget::Transfer(holiday)]
    );
    assert_eq!(household.posted(savings).len(), 2);
    assert!(
        household
            .posted(savings)
            .iter()
            .all(|side| *side.allocation() == Allocation::Transfer(payday))
    );
}

#[test]
fn lines_that_dont_add_up_to_the_amount_are_refused_and_change_nothing() {
    let mut household = Household::new();
    let payday = household.save_from_a_split();
    let [rainy_day, _] = household.payday_sides();
    let (groceries, savings) = (household.groceries, household.savings);
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(lines_edit(
        payday,
        "-350.00",
        vec![
            EditedSplitLine::category(groceries, "Food", usd("-50.00")),
            EditedSplitLine::kept(rainy_day, "Rainy day", usd("-200.00")),
            EditedSplitLine::transfer(savings, "Car", usd("-90.00")),
        ],
    ));

    assert_eq!(
        result.unwrap_err(),
        DomainError::SplitAmount {
            transaction: payday
        }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn a_line_keeping_an_other_side_the_transaction_doesnt_have_is_refused() {
    let mut household = Household::new();
    let costco = household.shop();
    let savings = household.savings;
    household.apply(DomainCommand::PostTransaction {
        account: household.checking,
        transaction: NewTransaction::transfer(date(6), "Saving up", savings, usd("-10.00")),
    });
    let saving_up = household.posted(savings)[0].id();
    let groceries = household.groceries;
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(lines_edit(
        costco,
        "-100.00",
        vec![
            EditedSplitLine::category(groceries, "Food", usd("-90.00")),
            EditedSplitLine::kept(saving_up, "Saving up", usd("-10.00")),
        ],
    ));

    assert_eq!(
        result.unwrap_err(),
        DomainError::UnknownTransaction {
            transaction: saving_up
        }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn a_line_keeping_one_other_side_twice_is_refused() {
    let mut household = Household::new();
    let payday = household.save_from_a_split();
    let [rainy_day, _] = household.payday_sides();
    let groceries = household.groceries;
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(lines_edit(
        payday,
        "-350.00",
        vec![
            EditedSplitLine::category(groceries, "Food", usd("-150.00")),
            EditedSplitLine::kept(rainy_day, "Rainy day", usd("-100.00")),
            EditedSplitLine::kept(rainy_day, "Rainy day", usd("-100.00")),
        ],
    ));

    assert_eq!(
        result.unwrap_err(),
        DomainError::UnknownTransaction {
            transaction: rainy_day
        }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn a_new_line_to_a_closed_account_is_refused_naming_it() {
    let mut household = Household::new();
    let costco = household.shop();
    let (groceries, savings) = (household.groceries, household.savings);
    household.apply(DomainCommand::SetAccountStatus {
        account: savings,
        status: AccountStatus::Closed,
    });
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(lines_edit(
        costco,
        "-100.00",
        vec![
            EditedSplitLine::category(groceries, "Food", usd("-60.00")),
            EditedSplitLine::transfer(savings, "Change jar", usd("-40.00")),
        ],
    ));

    assert_eq!(
        result.unwrap_err(),
        DomainError::TransferAccountClosed { account: savings }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn a_line_in_an_unknown_category_is_refused() {
    let mut household = Household::new();
    let costco = household.shop();
    let cleaning = household.cleaning;
    household.apply(DomainCommand::DeleteCategory { category: cleaning });
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(lines_edit(
        costco,
        "-100.00",
        vec![EditedSplitLine::category(cleaning, "Soap", usd("-100.00"))],
    ));

    assert_eq!(
        result.unwrap_err(),
        DomainError::UnknownCategory { category: cleaning }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn lines_on_the_opening_balance_are_refused() {
    let mut household = Household::new();
    let opening = household
        .summary(household.checking)
        .register()
        .get(0)
        .unwrap()
        .transaction()
        .id();
    let groceries = household.groceries;
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(lines_edit(
        opening,
        "1000.00",
        vec![EditedSplitLine::category(groceries, "", usd("1000.00"))],
    ));

    assert_eq!(
        result.unwrap_err(),
        DomainError::NotInOneCategory {
            transaction: opening
        }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn lines_on_a_split_lines_other_side_are_refused_naming_the_splits_account() {
    let mut household = Household::new();
    household.save_from_a_split();
    let [rainy_day, _] = household.payday_sides();
    let groceries = household.groceries;
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(lines_edit(
        rainy_day,
        "200.00",
        vec![EditedSplitLine::category(groceries, "", usd("200.00"))],
    ));

    assert_eq!(
        result.unwrap_err(),
        DomainError::SplitLineOtherSide {
            account: household.checking
        }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

/// Gives Payday a new line to Savings and drops Holiday, keeping Rainy day at a new
/// amount.
fn resplit_payday(household: &mut Household, payday: TransactionId) {
    let [rainy_day, _] = household.payday_sides();
    let (groceries, savings) = (household.groceries, household.savings);
    household.split(
        payday,
        "-400.00",
        vec![
            EditedSplitLine::category(groceries, "Food", usd("-50.00")),
            EditedSplitLine::kept(rainy_day, "Rainy day", usd("-250.00")),
            EditedSplitLine::transfer(savings, "Car", usd("-100.00")),
        ],
    );
}

#[test]
fn one_undo_puts_back_the_old_lines_and_other_sides() {
    let mut household = Household::new();
    let payday = household.save_from_a_split();
    let before = household.ledger.snapshot();
    resplit_payday(&mut household, payday);

    household.ledger.undo().expect("the edit undoes");

    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn one_undo_puts_back_a_transaction_in_one_category() {
    let mut household = Household::new();
    let costco = household.shop();
    let (groceries, savings) = (household.groceries, household.savings);
    let before = household.ledger.snapshot();
    household.split(
        costco,
        "-100.00",
        vec![
            EditedSplitLine::category(groceries, "Food", usd("-60.00")),
            EditedSplitLine::transfer(savings, "Change jar", usd("-40.00")),
        ],
    );

    household.ledger.undo().expect("the edit undoes");

    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn redo_gives_the_transaction_its_new_lines_and_other_sides_again() {
    let mut household = Household::new();
    let payday = household.save_from_a_split();
    resplit_payday(&mut household, payday);
    let after = household.ledger.snapshot();
    household.ledger.undo().expect("the edit undoes");

    household.ledger.redo().expect("the edit redoes");

    assert_eq!(household.ledger.snapshot().accounts(), after.accounts());
}

#[test]
fn lines_in_a_compound_with_other_edits_undo_in_one_step() {
    let mut household = Household::new();
    let payday = household.save_from_a_split();
    let [rainy_day, _] = household.payday_sides();
    let groceries = household.groceries;
    let before = household.ledger.snapshot();
    household.apply(DomainCommand::Compound(vec![
        DomainCommand::EditTransaction {
            transaction: payday,
            edit: TransactionEdit::Payee("Employer".to_owned()),
        },
        lines_edit(
            payday,
            "-300.00",
            vec![
                EditedSplitLine::category(groceries, "Food", usd("-100.00")),
                EditedSplitLine::kept(rainy_day, "Rainy day", usd("-200.00")),
            ],
        ),
        DomainCommand::EditTransaction {
            transaction: payday,
            edit: TransactionEdit::Date(date(9)),
        },
    ]));
    let split = only(household.posted(household.checking));
    assert_eq!(
        (split.payee(), split.date(), split.amount()),
        ("Employer", date(9), usd("-300.00"))
    );

    household.ledger.undo().expect("the Compound undoes");

    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn a_refused_lines_edit_in_a_compound_leaves_the_other_edits_unmade() {
    let mut household = Household::new();
    let costco = household.shop();
    let groceries = household.groceries;
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(DomainCommand::Compound(vec![
        DomainCommand::EditTransaction {
            transaction: costco,
            edit: TransactionEdit::Payee("Fresh Market".to_owned()),
        },
        lines_edit(
            costco,
            "-100.00",
            vec![EditedSplitLine::category(groceries, "Food", usd("-90.00"))],
        ),
    ]));

    assert_eq!(
        result.unwrap_err(),
        DomainError::SplitAmount {
            transaction: costco
        }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn new_lines_and_their_other_sides_survive_a_reopen() {
    let mut household = Household::new();
    let payday = household.save_from_a_split();
    resplit_payday(&mut household, payday);
    let live = household.ledger.snapshot();

    let household = household.reopened();

    assert_eq!(household.ledger.snapshot().accounts(), live.accounts());
}

#[test]
fn undo_after_a_reopen_puts_back_the_old_lines_and_redo_the_new() {
    let mut household = Household::new();
    let payday = household.save_from_a_split();
    let before = household.ledger.snapshot();
    resplit_payday(&mut household, payday);
    let after = household.ledger.snapshot();
    let mut household = household.reopened();

    household.ledger.undo().expect("the edit undoes");
    let undone = household.ledger.snapshot();
    let mut household = household.reopened();
    household.ledger.redo().expect("the edit redoes");

    assert_eq!(undone.accounts(), before.accounts());
    assert_eq!(household.ledger.snapshot().accounts(), after.accounts());
}

#[test]
fn a_new_line_to_the_transactions_own_account_is_refused() {
    let mut household = Household::new();
    let costco = household.shop();
    let (groceries, checking) = (household.groceries, household.checking);
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(lines_edit(
        costco,
        "-100.00",
        vec![
            EditedSplitLine::category(groceries, "Food", usd("-60.00")),
            EditedSplitLine::transfer(checking, "Change jar", usd("-40.00")),
        ],
    ));

    assert_eq!(
        result.unwrap_err(),
        DomainError::TransferToOwnAccount { account: checking }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

/// Asserts that giving Payday, with Savings closed, the lines `lines` makes from the
/// ids of Rainy day's and Holiday's other sides and Groceries is refused, naming Savings,
/// and changes nothing.
fn assert_lines_refused_while_savings_is_closed(
    lines: impl Fn([TransactionId; 2], CategoryId) -> Vec<EditedSplitLine>,
) {
    let mut household = Household::new();
    let payday = household.save_from_a_split();
    let sides = household.payday_sides();
    let savings = household.savings;
    household.apply(DomainCommand::SetAccountStatus {
        account: savings,
        status: AccountStatus::Closed,
    });
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(lines_edit(
        payday,
        "-350.00",
        lines(sides, household.groceries),
    ));

    assert_eq!(
        result.expect_err("the ledger refuses the lines"),
        DomainError::TransferAccountClosed { account: savings }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn dropping_a_line_whose_other_side_is_in_a_closed_account_is_refused_naming_it() {
    assert_lines_refused_while_savings_is_closed(|_, groceries| {
        vec![EditedSplitLine::category(groceries, "Food", usd("-350.00"))]
    });
}

#[test]
fn keeping_a_line_whose_other_side_is_in_a_closed_account_is_refused_naming_it() {
    assert_lines_refused_while_savings_is_closed(|[rainy_day, holiday], groceries| {
        vec![
            EditedSplitLine::category(groceries, "Snacks", usd("-50.00")),
            EditedSplitLine::kept(rainy_day, "Rainy day", usd("-200.00")),
            EditedSplitLine::kept(holiday, "Holiday", usd("-100.00")),
        ]
    });
}

#[test]
fn lines_split_a_transfer_keeping_its_other_side() {
    let mut household = Household::new();
    let (groceries, savings) = (household.groceries, household.savings);
    household.apply(DomainCommand::PostTransaction {
        account: household.checking,
        transaction: NewTransaction::transfer(date(6), "Saving up", savings, usd("-300.00")),
    });
    let sent = only(household.posted(household.checking)).id();
    let received = only(household.posted(savings)).id();

    household.split(
        sent,
        "-300.00",
        vec![
            EditedSplitLine::kept(received, "Saving up", usd("-250.00")),
            EditedSplitLine::category(groceries, "Lunch", usd("-50.00")),
        ],
    );

    let side = only(household.posted(savings));
    assert_eq!(
        (side.id(), side.allocation(), side.amount()),
        (received, &Allocation::Transfer(sent), usd("250.00"))
    );
    assert_eq!(
        lines(&only(household.posted(household.checking)))[0].0,
        SplitTarget::Transfer(received)
    );
}
