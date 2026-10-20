// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Transfers: a transaction posted to `[Account]` and its other side in that account.

use std::path::PathBuf;

use balanced_core::{
    AccountId, AccountKind, AccountStatus, AccountSummary, Allocation, CategoryId, CurrencyCode,
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
    assert_eq!((received.date(), received.amount()), (date(5), usd("200.00")));
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

        household.ledger.undo().expect("the change of target undoes");

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
        household.ledger.undo().expect("the change of target undoes");

        household.ledger.redo().expect("the change of target redoes");

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

        household.ledger.undo().expect("the change of target undoes");
        let household = household.reopened();

        assert_eq!(household.ledger.snapshot().accounts(), before.accounts());
    }
}

/// Applies `command` to `household`, expecting the ledger to refuse it with `error`
/// and leave every account as it was.
fn assert_refused(household: &mut Household, command: DomainCommand, error: DomainError) {
    let before = household.ledger.snapshot();

    let result = household.ledger.apply(command);

    assert_eq!(result.unwrap_err(), error);
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
        DomainError::TransferToOwnAccount { account: checking },
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
        DomainError::TransferAccountClosed { account: visa },
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
            DomainError::TransferAccountClosed { account: savings },
        );
    }
}

#[test]
fn the_opening_balance_can_not_become_a_transfer() {
    let mut household = Household::new();
    let opening_balance = household.summary(household.checking).register()[0]
        .transaction()
        .id();
    let savings = household.savings;

    assert_refused(
        &mut household,
        DomainCommand::EditTransaction {
            transaction: opening_balance,
            edit: TransactionEdit::Transfer(savings),
        },
        DomainError::NotInOneCategory {
            transaction: opening_balance,
        },
    );
}
