// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Transfers: a transaction posted to `[Account]` and its other side in that account.

use std::path::PathBuf;

use balanced_core::{
    AccountId, AccountKind, AccountStatus, AccountSummary, Allocation, CategoryId, CategoryType,
    ClearedStatus, CurrencyCode, Damage, DomainCommand, DomainError, Ledger, Money, NewTransaction,
    OpenError, Transaction, TransactionEdit, TransactionId,
};
use balanced_store::{AllocationRow, Store, TransactionRow};
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

/// A household with a Groceries category and three accounts, Checking, Savings and
/// Visa, each opened on 1 September holding 1000.00, in a new data file.
struct Household {
    /// Holds the file until the test ends.
    dir: TempDir,
    path: PathBuf,
    ledger: Ledger,
    checking: AccountId,
    savings: AccountId,
    visa: AccountId,
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
            ("Visa", AccountKind::CreditCard),
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
        let visa = account_id(&ledger, "Visa");
        Self {
            dir,
            path,
            ledger,
            checking,
            savings,
            visa,
        }
    }

    fn apply(&mut self, command: DomainCommand) {
        apply(&mut self.ledger, command);
    }

    /// Posts `transaction` to Checking, returning its id.
    fn post_to_checking(&mut self, transaction: NewTransaction) -> TransactionId {
        self.apply(DomainCommand::PostTransaction {
            account: self.checking,
            transaction,
        });
        last_posted(&self.summary(self.checking))
    }

    /// Posts 200.00 from Checking to Savings on 5 September, returning the Checking
    /// side's id.
    fn save_200(&mut self) -> TransactionId {
        let savings = self.savings;
        self.post_to_checking(
            NewTransaction::transfer(date(5), "Saving up", savings, usd("-200.00"))
                .number("1042")
                .memo("September"),
        )
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
            visa,
        } = self;
        drop(ledger);
        let ledger = Ledger::open(&path).expect("the file the ledger wrote opens");
        Self {
            dir,
            path,
            ledger,
            checking,
            savings,
            visa,
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
        .map(|summary| summary.account())
        .find(|account| account.name() == name)
        .expect("the test created the account")
        .id()
}

/// The id of the transaction posted last to the account `summary` shows.
fn last_posted(summary: &AccountSummary) -> TransactionId {
    summary
        .register()
        .iter()
        .map(|entry| entry.transaction().id())
        .max()
        .expect("the account has a transaction")
}

/// The id of the household's Groceries category.
fn groceries(household: &Household) -> CategoryId {
    household
        .ledger
        .snapshot()
        .categories()
        .iter()
        .find(|category| category.name() == "Groceries")
        .expect("the household has a Groceries category")
        .id()
}

/// The one transaction `transactions` holds.
fn only(transactions: Vec<Transaction>) -> Transaction {
    let [transaction] = <[Transaction; 1]>::try_from(transactions)
        .expect("the register holds one transaction besides its opening balance");
    transaction
}

#[test]
fn a_transfer_records_its_other_side_in_the_other_account() {
    let mut household = Household::new();

    let sent = household.save_200();

    let checking = only(household.posted(household.checking));
    let savings = only(household.posted(household.savings));
    assert_eq!(checking.id(), sent);
    assert_eq!(checking.allocation(), &Allocation::Transfer(savings.id()));
    assert_eq!(savings.allocation(), &Allocation::Transfer(sent));
    assert_eq!((savings.date(), savings.amount()), (date(5), usd("200.00")));
    assert_eq!(
        (savings.payee(), savings.memo()),
        ("Saving up", "September")
    );
    assert_eq!((savings.number(), savings.is_flagged()), (None, false));
}

#[test]
fn a_posted_transfers_other_side_takes_a_later_id() {
    let mut household = Household::new();

    let sent = household.save_200();

    let received = only(household.posted(household.savings));
    assert!(received.id() > sent, "{:?} after {sent:?}", received.id());
}

#[test]
fn a_transfer_moves_both_balances_and_leaves_net_worth_as_it_was() {
    let mut household = Household::new();
    let net_worth = household.ledger.snapshot().net_worth();

    household.save_200();

    assert_eq!(household.balance(household.checking), usd("800.00"));
    assert_eq!(household.balance(household.savings), usd("1200.00"));
    assert_eq!(household.ledger.snapshot().net_worth(), net_worth);
}

#[test]
fn both_sides_of_a_transfer_survive_a_reopen() {
    let mut household = Household::new();
    household.save_200();
    let live = household.ledger.snapshot();

    let household = household.reopened();

    let reopened = household.ledger.snapshot();
    assert_eq!(reopened.accounts(), live.accounts());
    assert_eq!(reopened.net_worth(), live.net_worth());
}

#[test]
fn a_transfer_to_its_own_account_is_refused() {
    let mut household = Household::new();
    let before = household.ledger.snapshot();
    let checking = household.checking;

    let result = household.ledger.apply(DomainCommand::PostTransaction {
        account: checking,
        transaction: NewTransaction::transfer(date(5), "Nowhere", checking, usd("-200.00")),
    });

    assert_eq!(
        result.unwrap_err(),
        DomainError::TransferToOwnAccount { account: checking }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn a_transfer_to_a_closed_account_is_refused_naming_it() {
    let mut household = Household::new();
    let savings = household.savings;
    household.apply(DomainCommand::SetAccountStatus {
        account: savings,
        status: AccountStatus::Closed,
    });
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(DomainCommand::PostTransaction {
        account: household.checking,
        transaction: NewTransaction::transfer(date(5), "Saving up", savings, usd("-200.00")),
    });

    assert_eq!(
        result.unwrap_err(),
        DomainError::TransferAccountClosed { account: savings }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

/// Each transaction `account` holds besides its opening balance, by id.
fn ids(household: &Household, account: AccountId) -> Vec<TransactionId> {
    household
        .posted(account)
        .iter()
        .map(Transaction::id)
        .collect()
}

#[test]
fn deleting_either_side_of_a_transfer_deletes_both() {
    for delete_savings_side in [false, true] {
        let mut household = Household::new();
        let sent = household.save_200();
        let received = only(household.posted(household.savings)).id();
        let deleted = if delete_savings_side { received } else { sent };
        let net_worth = household.ledger.snapshot().net_worth();

        household.apply(DomainCommand::DeleteTransaction {
            transaction: deleted,
        });

        assert_eq!(household.posted(household.checking), []);
        assert_eq!(household.posted(household.savings), []);
        assert_eq!(household.balance(household.checking), usd("1000.00"));
        assert_eq!(household.balance(household.savings), usd("1000.00"));
        assert_eq!(household.ledger.snapshot().net_worth(), net_worth);
    }
}

#[test]
fn one_undo_puts_back_both_sides_of_a_deleted_transfer_with_their_ids() {
    let mut household = Household::new();
    let sent = household.save_200();
    let before = household.ledger.snapshot();
    household.apply(DomainCommand::DeleteTransaction { transaction: sent });

    household.ledger.undo().expect("the delete undoes");

    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn redo_deletes_both_sides_of_an_undeleted_transfer_again() {
    let mut household = Household::new();
    let sent = household.save_200();
    household.apply(DomainCommand::DeleteTransaction { transaction: sent });
    household.ledger.undo().expect("the delete undoes");

    household.ledger.redo().expect("the delete redoes");

    assert_eq!(ids(&household, household.checking), []);
    assert_eq!(ids(&household, household.savings), []);
}

#[test]
fn undoing_a_transfer_takes_out_both_sides_and_redo_puts_them_back_with_their_ids() {
    let mut household = Household::new();
    household.save_200();
    let posted = household.ledger.snapshot();

    household.ledger.undo().expect("the post undoes");
    let undone = (
        ids(&household, household.checking),
        ids(&household, household.savings),
    );
    household.ledger.redo().expect("the post redoes");

    assert_eq!(undone, (vec![], vec![]));
    assert_eq!(household.ledger.snapshot().accounts(), posted.accounts());
}

#[test]
fn undo_after_a_reopen_puts_back_both_sides_of_a_deleted_transfer() {
    let mut household = Household::new();
    let sent = household.save_200();
    let before = household.ledger.snapshot();
    household.apply(DomainCommand::DeleteTransaction { transaction: sent });
    let mut household = household.reopened();

    household.ledger.undo().expect("the delete undoes");
    let household = household.reopened();

    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn deleting_a_transfer_whose_other_side_is_in_a_closed_account_is_refused_naming_it() {
    let mut household = Household::new();
    let sent = household.save_200();
    let savings = household.savings;
    household.apply(DomainCommand::SetAccountStatus {
        account: savings,
        status: AccountStatus::Closed,
    });
    let before = household.ledger.snapshot();

    let result = household
        .ledger
        .apply(DomainCommand::DeleteTransaction { transaction: sent });

    assert_eq!(
        result.unwrap_err(),
        DomainError::TransferAccountClosed { account: savings }
    );
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn changing_either_sides_amount_changes_the_other_with_its_sign_flipped() {
    for edit_savings_side in [false, true] {
        let mut household = Household::new();
        let sent = household.save_200();
        let received = only(household.posted(household.savings)).id();
        let (edited, amount) = if edit_savings_side {
            (received, usd("350.00"))
        } else {
            (sent, usd("-350.00"))
        };
        let net_worth = household.ledger.snapshot().net_worth();

        household.apply(DomainCommand::EditTransaction {
            transaction: edited,
            edit: TransactionEdit::Amount(amount),
        });

        let checking = only(household.posted(household.checking));
        let savings = only(household.posted(household.savings));
        assert_eq!(checking.amount(), usd("-350.00"));
        assert_eq!(savings.amount(), usd("350.00"));
        assert_eq!(household.balance(household.checking), usd("650.00"));
        assert_eq!(household.balance(household.savings), usd("1350.00"));
        assert_eq!(household.ledger.snapshot().net_worth(), net_worth);
    }
}

#[test]
fn changing_either_sides_date_moves_both_into_date_order() {
    for edit_savings_side in [false, true] {
        let mut household = Household::new();
        let sent = household.save_200();
        let received = only(household.posted(household.savings)).id();
        let groceries = household.ledger.snapshot().categories()[0].id();
        let shopping = household.post_to_checking(NewTransaction::new(
            date(10),
            "Corner Shop",
            groceries,
            usd("-40.00"),
        ));
        let edited = if edit_savings_side { received } else { sent };

        household.apply(DomainCommand::EditTransaction {
            transaction: edited,
            edit: TransactionEdit::Date(date(12)),
        });

        let dates = |account| {
            household
                .posted(account)
                .iter()
                .map(|transaction| (transaction.id(), transaction.date()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            dates(household.checking),
            [(shopping, date(10)), (sent, date(12))]
        );
        assert_eq!(dates(household.savings), [(received, date(12))]);
    }
}

#[test]
fn changing_one_sides_own_fields_leaves_the_other_side_as_it_was() {
    let edits = [
        TransactionEdit::Payee("Rainy day".to_owned()),
        TransactionEdit::Memo("October".to_owned()),
        TransactionEdit::Number(Some("1043".to_owned())),
        TransactionEdit::ClearedStatus(ClearedStatus::Cleared),
    ];
    for edit in edits {
        let mut household = Household::new();
        let sent = household.save_200();
        let savings = household.posted(household.savings);

        household.apply(DomainCommand::EditTransaction {
            transaction: sent,
            edit: edit.clone(),
        });

        assert_eq!(household.posted(household.savings), savings, "{edit:?}");
    }
}

#[test]
fn flagging_one_side_leaves_the_other_side_unflagged() {
    let mut household = Household::new();
    let sent = household.save_200();
    let savings = household.posted(household.savings);

    household.apply(DomainCommand::SetFlagged {
        transaction: sent,
        flagged: true,
    });

    assert!(only(household.posted(household.checking)).is_flagged());
    assert_eq!(household.posted(household.savings), savings);
}

#[test]
fn one_undo_puts_both_sides_back_and_redo_changes_both_again() {
    let edits = [
        TransactionEdit::Date(date(12)),
        TransactionEdit::Amount(usd("350.00")),
    ];
    for edit in edits {
        let mut household = Household::new();
        household.save_200();
        let received = only(household.posted(household.savings)).id();
        let before = household.ledger.snapshot();
        household.apply(DomainCommand::EditTransaction {
            transaction: received,
            edit: edit.clone(),
        });
        let edited = household.ledger.snapshot();

        household.ledger.undo().expect("the edit undoes");
        let undone = household.ledger.snapshot();
        household.ledger.redo().expect("the edit redoes");

        assert_eq!(undone.accounts(), before.accounts(), "{edit:?}");
        assert_eq!(
            household.ledger.snapshot().accounts(),
            edited.accounts(),
            "{edit:?}"
        );
    }
}

#[test]
fn undo_after_a_reopen_puts_both_sides_of_an_edited_transfer_back() {
    let mut household = Household::new();
    let sent = household.save_200();
    let before = household.ledger.snapshot();
    household.apply(DomainCommand::EditTransaction {
        transaction: sent,
        edit: TransactionEdit::Amount(usd("-350.00")),
    });
    let mut household = household.reopened();

    household.ledger.undo().expect("the edit undoes");
    let household = household.reopened();

    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn changing_the_date_or_amount_when_the_other_side_is_closed_is_refused_naming_it() {
    let edits = [
        TransactionEdit::Date(date(12)),
        TransactionEdit::Amount(usd("-350.00")),
    ];
    for edit in edits {
        let mut household = Household::new();
        let sent = household.save_200();
        let savings = household.savings;
        household.apply(DomainCommand::SetAccountStatus {
            account: savings,
            status: AccountStatus::Closed,
        });
        let before = household.ledger.snapshot();

        let result = household.ledger.apply(DomainCommand::EditTransaction {
            transaction: sent,
            edit: edit.clone(),
        });

        assert_eq!(
            result.unwrap_err(),
            DomainError::TransferAccountClosed { account: savings },
            "{edit:?}"
        );
        assert_eq!(
            household.ledger.snapshot().accounts(),
            before.accounts(),
            "{edit:?}"
        );
    }
}

#[test]
fn changing_the_date_or_amount_of_a_side_in_a_closed_account_is_refused_naming_it() {
    let edits = [
        TransactionEdit::Date(date(12)),
        TransactionEdit::Amount(usd("350.00")),
    ];
    for edit in edits {
        let mut household = Household::new();
        household.save_200();
        let received = only(household.posted(household.savings)).id();
        let savings = household.savings;
        household.apply(DomainCommand::SetAccountStatus {
            account: savings,
            status: AccountStatus::Closed,
        });
        let before = household.ledger.snapshot();

        let result = household.ledger.apply(DomainCommand::EditTransaction {
            transaction: received,
            edit: edit.clone(),
        });

        assert_eq!(
            result.unwrap_err(),
            DomainError::AccountClosed { account: savings },
            "{edit:?}"
        );
        assert_eq!(
            household.ledger.snapshot().accounts(),
            before.accounts(),
            "{edit:?}"
        );
    }
}

#[test]
fn a_refused_change_leaves_both_sides_as_they_were_in_the_file() {
    let mut household = Household::new();
    let sent = household.save_200();
    household.apply(DomainCommand::SetAccountStatus {
        account: household.savings,
        status: AccountStatus::Closed,
    });
    let before = household.ledger.snapshot();
    household
        .ledger
        .apply(DomainCommand::EditTransaction {
            transaction: sent,
            edit: TransactionEdit::Amount(usd("-350.00")),
        })
        .expect_err("the other side is in a closed account");

    let household = household.reopened();

    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn changing_ones_own_payee_when_the_other_side_is_closed_still_applies() {
    let mut household = Household::new();
    let sent = household.save_200();
    household.apply(DomainCommand::SetAccountStatus {
        account: household.savings,
        status: AccountStatus::Closed,
    });

    household.apply(DomainCommand::EditTransaction {
        transaction: sent,
        edit: TransactionEdit::Payee("Rainy day".to_owned()),
    });

    assert_eq!(
        only(household.posted(household.checking)).payee(),
        "Rainy day"
    );
}

#[test]
fn both_sides_of_an_edited_transfer_survive_a_reopen() {
    let mut household = Household::new();
    let sent = household.save_200();
    household.apply(DomainCommand::EditTransaction {
        transaction: sent,
        edit: TransactionEdit::Amount(usd("-350.00")),
    });
    let live = household.ledger.snapshot();

    let household = household.reopened();

    assert_eq!(household.ledger.snapshot().accounts(), live.accounts());
}

#[test]
fn deleting_an_account_holding_either_side_of_a_transfer_is_refused_naming_it() {
    for delete_savings in [false, true] {
        let mut household = Household::new();
        household.save_200();
        let account = if delete_savings {
            household.savings
        } else {
            household.checking
        };
        let before = household.ledger.snapshot();

        let result = household
            .ledger
            .apply(DomainCommand::DeleteAccount { account });

        assert_eq!(
            result.unwrap_err(),
            DomainError::AccountHasTransfers { account }
        );
        assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
    }
}

#[test]
fn an_account_without_transfers_still_deletes_and_undo_restores_it() {
    let mut household = Household::new();
    household.save_200();
    household.apply(DomainCommand::CreateAccount {
        name: "Car".to_owned(),
        kind: AccountKind::Asset,
        opened: date(1),
        opening_balance: usd("5000.00"),
    });
    let car = account_id(&household.ledger, "Car");
    let before = household.ledger.snapshot();

    household.apply(DomainCommand::DeleteAccount { account: car });
    let deleted = household.ledger.snapshot().account(car).is_none();
    household.ledger.undo().expect("the delete undoes");

    assert!(deleted);
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn changing_a_transfers_account_moves_its_other_side_there() {
    let mut household = Household::new();
    let sent = household.save_200();
    let old_side = only(household.posted(household.savings)).id();
    let visa = household.visa;
    household.apply(DomainCommand::SetFlagged {
        transaction: old_side,
        flagged: true,
    });

    household.apply(DomainCommand::EditTransaction {
        transaction: sent,
        edit: TransactionEdit::Transfer(visa),
    });

    let checking = only(household.posted(household.checking));
    let received = only(household.posted(visa));
    assert_eq!(household.posted(household.savings), []);
    assert_ne!(received.id(), old_side);
    assert_eq!(checking.allocation(), &Allocation::Transfer(received.id()));
    assert_eq!(received.allocation(), &Allocation::Transfer(sent));
    assert_eq!(
        (received.date(), received.amount()),
        (date(5), usd("200.00"))
    );
    assert_eq!(
        (received.payee(), received.memo()),
        ("Saving up", "September")
    );
    assert_eq!((received.number(), received.is_flagged()), (None, false));
    assert_eq!(household.balance(household.savings), usd("1000.00"));
    assert_eq!(household.balance(visa), usd("1200.00"));
}

#[test]
fn changing_the_account_of_a_transfers_other_side_gives_its_new_other_side_a_later_id() {
    let mut household = Household::new();
    household.save_200();
    let received = only(household.posted(household.savings)).id();
    let visa = household.visa;

    household.apply(DomainCommand::EditTransaction {
        transaction: received,
        edit: TransactionEdit::Transfer(visa),
    });

    let new_side = only(household.posted(visa)).id();
    assert!(new_side > received, "{new_side:?} after {received:?}");
}

#[test]
fn changing_a_transfer_to_a_category_deletes_its_other_side() {
    let mut household = Household::new();
    let sent = household.save_200();
    let groceries = groceries(&household);

    household.apply(DomainCommand::EditTransaction {
        transaction: sent,
        edit: TransactionEdit::Category(groceries),
    });

    let checking = only(household.posted(household.checking));
    assert_eq!(checking.allocation(), &Allocation::Category(groceries));
    assert_eq!(household.posted(household.savings), []);
    assert_eq!(household.balance(household.checking), usd("800.00"));
    assert_eq!(household.balance(household.savings), usd("1000.00"));
}

#[test]
fn changing_a_category_to_an_account_makes_the_transaction_a_transfer() {
    let mut household = Household::new();
    let groceries = groceries(&household);
    let savings = household.savings;
    let paid = household.post_to_checking(
        NewTransaction::new(date(5), "Saving up", groceries, usd("-200.00")).memo("September"),
    );

    household.apply(DomainCommand::EditTransaction {
        transaction: paid,
        edit: TransactionEdit::Transfer(savings),
    });

    let checking = only(household.posted(household.checking));
    let received = only(household.posted(savings));
    assert_eq!(checking.allocation(), &Allocation::Transfer(received.id()));
    assert_eq!(received.allocation(), &Allocation::Transfer(paid));
    assert_eq!(
        (received.date(), received.amount()),
        (date(5), usd("200.00"))
    );
    assert_eq!(
        (received.payee(), received.memo()),
        ("Saving up", "September")
    );
    assert_eq!(household.balance(savings), usd("1200.00"));
}

/// Each change of target from Checking: a transfer to Savings sent to Visa, a transfer
/// to Savings put in Groceries, and a payment in Groceries sent to Savings. Each sets up
/// its transaction in `household` and returns the edit Command.
fn target_changes() -> [fn(&mut Household) -> DomainCommand; 3] {
    [
        |household| DomainCommand::EditTransaction {
            transaction: household.save_200(),
            edit: TransactionEdit::Transfer(household.visa),
        },
        |household| DomainCommand::EditTransaction {
            transaction: household.save_200(),
            edit: TransactionEdit::Category(groceries(household)),
        },
        |household| {
            let groceries = groceries(household);
            DomainCommand::EditTransaction {
                transaction: household.post_to_checking(NewTransaction::new(
                    date(5),
                    "Saving up",
                    groceries,
                    usd("-200.00"),
                )),
                edit: TransactionEdit::Transfer(household.savings),
            }
        },
    ]
}

#[test]
fn one_undo_puts_back_a_change_of_target_with_the_old_other_side_and_its_id() {
    for change in target_changes() {
        let mut household = Household::new();
        let edit = change(&mut household);
        let before = household.ledger.snapshot();
        household.apply(edit);

        household
            .ledger
            .undo()
            .expect("the change of target undoes");

        assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
    }
}

#[test]
fn redo_makes_a_change_of_target_again_with_the_same_ids() {
    for change in target_changes() {
        let mut household = Household::new();
        let edit = change(&mut household);
        household.apply(edit);
        let changed = household.ledger.snapshot();
        household
            .ledger
            .undo()
            .expect("the change of target undoes");

        household
            .ledger
            .redo()
            .expect("the change of target redoes");

        assert_eq!(household.ledger.snapshot().accounts(), changed.accounts());
    }
}

#[test]
fn undo_after_a_reopen_puts_back_a_change_of_target() {
    for change in target_changes() {
        let mut household = Household::new();
        let edit = change(&mut household);
        let before = household.ledger.snapshot();
        household.apply(edit);
        let mut household = household.reopened();

        household
            .ledger
            .undo()
            .expect("the change of target undoes");
        let household = household.reopened();

        assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
    }
}

/// Applies `command` to `household`, expecting the ledger to refuse it with `error`
/// and leave every account as it was.
fn assert_refused(household: &mut Household, command: DomainCommand, error: &DomainError) {
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(command);

    assert_eq!(&result.expect_err("the ledger refuses the command"), error);
    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

#[test]
fn changing_a_transfer_to_its_own_account_is_refused() {
    let mut household = Household::new();
    let sent = household.save_200();
    let checking = household.checking;

    assert_refused(
        &mut household,
        DomainCommand::EditTransaction {
            transaction: sent,
            edit: TransactionEdit::Transfer(checking),
        },
        &DomainError::TransferToOwnAccount { account: checking },
    );
}

#[test]
fn changing_a_transfer_to_a_closed_account_is_refused_naming_it() {
    let mut household = Household::new();
    let sent = household.save_200();
    let visa = household.visa;
    household.apply(DomainCommand::SetAccountStatus {
        account: visa,
        status: AccountStatus::Closed,
    });

    assert_refused(
        &mut household,
        DomainCommand::EditTransaction {
            transaction: sent,
            edit: TransactionEdit::Transfer(visa),
        },
        &DomainError::TransferAccountClosed { account: visa },
    );
}

#[test]
fn changing_a_transfer_away_from_a_closed_account_is_refused_naming_it() {
    let edits: [fn(&Household) -> TransactionEdit; 2] = [
        |household| TransactionEdit::Transfer(household.visa),
        |household| TransactionEdit::Category(groceries(household)),
    ];
    for edit in edits {
        let mut household = Household::new();
        let sent = household.save_200();
        let savings = household.savings;
        household.apply(DomainCommand::SetAccountStatus {
            account: savings,
            status: AccountStatus::Closed,
        });
        let edit = edit(&household);

        assert_refused(
            &mut household,
            DomainCommand::EditTransaction {
                transaction: sent,
                edit,
            },
            &DomainError::TransferAccountClosed { account: savings },
        );
    }
}

#[test]
fn the_opening_balance_can_not_become_a_transfer() {
    let mut household = Household::new();
    let opening_balance = household
        .summary(household.checking)
        .register()
        .get(0)
        .unwrap()
        .transaction()
        .id();
    let savings = household.savings;

    assert_refused(
        &mut household,
        DomainCommand::EditTransaction {
            transaction: opening_balance,
            edit: TransactionEdit::Transfer(savings),
        },
        &DomainError::NotInOneCategory {
            transaction: opening_balance,
        },
    );
}

#[test]
fn changing_a_transfer_to_the_account_it_already_names_changes_nothing() {
    let mut household = Household::new();
    let sent = household.save_200();
    let received = only(household.posted(household.savings)).id();
    let savings = household.savings;
    household.apply(DomainCommand::SetFlagged {
        transaction: received,
        flagged: true,
    });
    let before = household.ledger.snapshot();

    household.apply(DomainCommand::EditTransaction {
        transaction: sent,
        edit: TransactionEdit::Transfer(savings),
    });

    assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
}

/// The data file of a household after 200.00 was saved from Checking to Savings, with
/// the Savings side's row then rewritten by `change` below the ledger, as another
/// program might. Returns the directory holding the file, the file's path, and the
/// Checking side's id, then the Savings side's.
fn rewritten_file(
    change: impl FnOnce(&mut TransactionRow, &[TransactionRow]),
) -> (TempDir, PathBuf, TransactionId, TransactionId) {
    let mut household = Household::new();
    let sent = household.save_200();
    let received = only(household.posted(household.savings)).id();
    let Household {
        dir, path, ledger, ..
    } = household;
    drop(ledger);
    let store = Store::open(&path).expect("the file the ledger wrote opens");
    let rows: Vec<TransactionRow> = store
        .read()
        .expect("the file reads")
        .transactions
        .into_iter()
        .map(|stored| stored.row)
        .collect();
    let mut row = rows
        .iter()
        .find(|row| {
            matches!(
                row.allocation,
                AllocationRow::Transfer { amount: 20_000, .. }
            )
        })
        .expect("the file holds the Savings side")
        .clone();
    change(&mut row, &rows);
    store.begin().expect("the file takes a change");
    store.update_transaction(&row).expect("the row rewrites");
    store.commit().expect("the change commits");
    (dir, path, sent, received)
}

/// A change to a transfer side's row, given every transaction row in the file.
type RowChange = fn(&mut TransactionRow, &[TransactionRow]);

/// Moves `row` to 7 September.
fn redate(row: &mut TransactionRow) {
    "2026-09-07".clone_into(&mut row.date);
}

/// Makes `row`, a transfer side, 250.00.
fn reamount(row: &mut TransactionRow) {
    let AllocationRow::Transfer { amount, .. } = &mut row.allocation else {
        panic!("the row is a transfer side");
    };
    *amount = 25_000;
}

/// Points `row`, a transfer side, at the opening balance of its own account, among
/// `rows`, which doesn't point back.
fn repoint(row: &mut TransactionRow, rows: &[TransactionRow]) {
    let opening_balance = rows
        .iter()
        .find(|other| {
            other.account_id == row.account_id
                && matches!(other.allocation, AllocationRow::OpeningBalance { .. })
        })
        .expect("the account has an opening balance")
        .id;
    let AllocationRow::Transfer { other_side_id, .. } = &mut row.allocation else {
        panic!("the row is a transfer side");
    };
    *other_side_id = opening_balance;
}

#[test]
fn a_file_whose_transfer_sides_disagree_is_refused_as_damaged_and_left_as_it_was() {
    let changes: [(&str, RowChange); 4] = [
        ("date", |row, _| redate(row)),
        ("amount", |row, _| reamount(row)),
        ("date and amount", |row, _| {
            redate(row);
            reamount(row);
        }),
        ("other side", repoint),
    ];
    for (changed, change) in changes {
        let (_dir, path, sent, received) = rewritten_file(change);
        let before = std::fs::read(&path).expect("the file reads");

        let result = Ledger::open(&path);

        let Err(OpenError::Damaged(Damage::TransferSides {
            transaction,
            other_side,
        })) = result
        else {
            panic!("the file whose {changed} was changed opened as {result:?}");
        };
        assert_eq!((transaction, other_side), (sent, received), "{changed}");
        assert_eq!(
            std::fs::read(&path).expect("the file reads"),
            before,
            "{changed}"
        );
    }
}
