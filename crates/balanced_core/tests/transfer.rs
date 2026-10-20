// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Transfers: a transaction posted to `[Account]` and its other side in that account.

use std::path::PathBuf;

use balanced_core::{
    AccountId, AccountKind, AccountStatus, AccountSummary, Allocation, ClearedStatus, CurrencyCode,
    DomainCommand, DomainError, Ledger, Money, NewTransaction, Transaction, TransactionEdit,
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

/// A household with a Groceries category and two accounts, Checking and Savings, each
/// opened on 1 September holding 1000.00, in a new data file.
struct Household {
    /// Holds the file until the test ends.
    dir: TempDir,
    path: PathBuf,
    ledger: Ledger,
    checking: AccountId,
    savings: AccountId,
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
            },
        );
        let checking = account_id(&ledger, "Checking");
        let savings = account_id(&ledger, "Savings");
        Self {
            dir,
            path,
            ledger,
            checking,
            savings,
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
        } = self;
        drop(ledger);
        let ledger = Ledger::open(&path).expect("the file the ledger wrote opens");
        Self {
            dir,
            path,
            ledger,
            checking,
            savings,
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

/// The id of the transaction posted last to the account `summary` shows.
fn last_posted(summary: &AccountSummary) -> TransactionId {
    summary
        .register()
        .iter()
        .map(|entry| entry.transaction().id())
        .max()
        .expect("the account has a transaction")
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
