// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Times one Command on Household Checking, in the stub written to a data file, at the
//! stub's size (20,000 transactions) and at ten times it. Each run holds the snapshot
//! taken before it, as a window does, and is undone, untimed, before the next.
//!
//! Run it in the `release` profile on a quiet machine, as `docs/practices/tooling.md`
//! describes: `taskset -c 0 cargo bench -p balanced_core --features test-support`.

use std::cell::RefCell;

use balanced_core::{
    AccountId, DomainCommand, Ledger, LedgerView, NewTransaction, Transaction, TransactionEdit,
};
use divan::Bencher;
use tempfile::TempDir;

fn main() {
    divan::main();
}

/// How many transactions the stub's generated account holds at 1×.
const STUB_TRANSACTIONS: u32 = 20_000;

/// The sizes timed, as multiples of the stub's.
const SCALES: [u32; 2] = [1, 10];

/// The stub at `scale` times its size, in a data file in a directory of its own, and the
/// id of its generated account, Household Checking.
struct Household {
    ledger: RefCell<Ledger>,
    account: AccountId,
    /// Holds the data file until the ledger is dropped.
    _directory: TempDir,
}

impl Household {
    fn new(scale: u32) -> Self {
        // Under the build's own directory, so the data file is on disk, where a commit
        // pays for its fsync as the app's does, rather than in a RAM-backed `/tmp`.
        let directory =
            TempDir::new_in(env!("CARGO_TARGET_TMPDIR")).expect("a temporary directory");
        let path = directory.path().join("household.balanced");
        let ledger = Ledger::create_stub(&path, STUB_TRANSACTIONS * scale)
            .expect("the stub is written to a new file");
        let account = ledger
            .snapshot()
            .accounts()
            .iter()
            .find(|summary| summary.account().name() == "Household Checking")
            .expect("the stub has Household Checking")
            .account()
            .id();
        Self {
            ledger: RefCell::new(ledger),
            account,
            _directory: directory,
        }
    }

    /// The transaction at `index` in Household Checking's register, counted from the
    /// end when `index` is negative.
    fn transaction(&self, index: isize) -> Transaction {
        let view = self.ledger.borrow().snapshot();
        let register = view
            .account(self.account)
            .expect("Household Checking")
            .register();
        let index = if index < 0 {
            register.len() - index.unsigned_abs()
        } else {
            index.unsigned_abs()
        };
        register
            .get(index)
            .expect("the register holds the entry")
            .transaction()
            .clone()
    }

    /// Times `command`, applied to the ledger as it was before every earlier run. The
    /// snapshot a window would hold is taken, and the earlier run undone, untimed.
    fn bench(&self, bencher: Bencher<'_, '_>, command: &DomainCommand) {
        bencher
            .with_inputs(|| {
                let mut ledger = self.ledger.borrow_mut();
                if ledger.snapshot().can_undo() {
                    ledger.undo().expect("the last run undoes");
                }
                ledger.snapshot()
            })
            .bench_local_values(|held: std::sync::Arc<LedgerView>| {
                self.ledger
                    .borrow_mut()
                    .apply(command.clone())
                    .expect("the Command applies");
                // Returned, so the snapshot is dropped after the run, as a window drops
                // it on its next frame.
                held
            });
    }
}

/// Posts a payment dated on the register's last date, so it lands at the end.
#[divan::bench(args = SCALES, sample_size = 1, sample_count = 30)]
fn post(bencher: Bencher<'_, '_>, scale: u32) {
    let household = Household::new(scale);
    let last = household.transaction(-1);
    let view = household.ledger.borrow().snapshot();
    let groceries = view
        .categories()
        .iter()
        .find(|category| category.name() == "Groceries")
        .expect("the stub has Groceries")
        .id();
    let command = DomainCommand::PostTransaction {
        account: household.account,
        transaction: NewTransaction::new(last.date(), "Fresh Market", groceries, last.amount()),
    };
    household.bench(bencher, &command);
}

/// Moves the first payment after the opening balance to the register's last date, so
/// it moves to the end and every balance changes.
#[divan::bench(args = SCALES, sample_size = 1, sample_count = 30)]
fn edit_date(bencher: Bencher<'_, '_>, scale: u32) {
    let household = Household::new(scale);
    let command = DomainCommand::EditTransaction {
        transaction: household.transaction(1).id(),
        edit: TransactionEdit::Date(household.transaction(-1).date()),
    };
    household.bench(bencher, &command);
}

/// Deletes the transaction in the middle of the register.
#[divan::bench(args = SCALES, sample_size = 1, sample_count = 30)]
fn delete(bencher: Bencher<'_, '_>, scale: u32) {
    let household = Household::new(scale);
    let middle = isize::try_from(STUB_TRANSACTIONS * scale / 2).expect("the index fits");
    let command = DomainCommand::DeleteTransaction {
        transaction: household.transaction(middle).id(),
    };
    household.bench(bencher, &command);
}

/// Flags the transaction in the middle of the register, which moves no balance.
#[divan::bench(args = SCALES, sample_size = 1, sample_count = 30)]
fn flag(bencher: Bencher<'_, '_>, scale: u32) {
    let household = Household::new(scale);
    let middle = isize::try_from(STUB_TRANSACTIONS * scale / 2).expect("the index fits");
    let command = DomainCommand::SetFlagged {
        transaction: household.transaction(middle).id(),
        flagged: true,
    };
    household.bench(bencher, &command);
}
