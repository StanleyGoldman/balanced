// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Times what a window rebuilds on its first frame after each Command, on Household
//! Checking in the stub, at the stub's size (20,000 transactions) and at ten times it:
//! the Register's rows, the Payee and Category comboboxes' choices, and the usage count
//! the Category List reads. Each run reads a snapshot no earlier run has read, taken
//! after an untimed Command.
//!
//! Run it in the `release` profile on a quiet machine, as `docs/practices/tooling.md`
//! describes: `taskset -c 0 cargo bench -p balanced_ui --features test-support --bench
//! rebuild`.

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use balanced_core::{AccountId, CategoryId, DomainCommand, Ledger, LedgerView, TransactionId};
use balanced_i18n::Localizer;
use balanced_ui::testing::{build_field_choices, build_register_model};
use divan::Bencher;
use icu_calendar::{Date, Iso};
use tempfile::TempDir;

fn main() {
    divan::main();
}

/// How many transactions the stub's generated account holds at 1×.
const STUB_TRANSACTIONS: u32 = 20_000;

/// The sizes timed, as multiples of the stub's.
const SCALES: [u32; 2] = [1, 10];

/// The stub at `scale` times its size, in a data file in a directory of its own, with
/// the ids its runs read.
struct Household {
    ledger: RefCell<Ledger>,
    /// Household Checking, the stub's generated account.
    account: AccountId,
    /// The transaction in the middle of Household Checking, which each run's Command
    /// flags or unflags.
    middle: TransactionId,
    /// Groceries, which the generated payments are in.
    groceries: CategoryId,
    /// Whether the middle transaction is flagged now. The stub flags none.
    flagged: Cell<bool>,
    i18n: Localizer,
    /// Holds the data file until the ledger is dropped.
    _directory: TempDir,
}

impl Household {
    fn new(scale: u32) -> Self {
        // Under the build's own directory, as the Commands benchmark does, though no
        // run here is timed with a commit in it.
        let directory =
            TempDir::new_in(env!("CARGO_TARGET_TMPDIR")).expect("a temporary directory");
        let path = directory.path().join("household.balanced");
        let ledger = Ledger::create_stub(&path, STUB_TRANSACTIONS * scale)
            .expect("the stub is written to a new file");
        let view = ledger.snapshot();
        let summary = view
            .accounts()
            .iter()
            .find(|summary| summary.account().name() == "Household Checking")
            .expect("the stub has Household Checking");
        let register = summary.register();
        let middle = register
            .get(register.len() / 2)
            .expect("the register holds the entry")
            .transaction()
            .id();
        let groceries = view
            .categories()
            .iter()
            .find(|category| category.name() == "Groceries")
            .expect("the stub has Groceries")
            .id();
        Self {
            account: summary.account().id(),
            middle,
            groceries,
            flagged: Cell::new(false),
            ledger: RefCell::new(ledger),
            i18n: Localizer::for_tests(),
            _directory: directory,
        }
    }

    /// Times `rebuild` on a new snapshot for each run: the middle transaction is
    /// flagged or unflagged first, untimed, which moves to a new generation and changes
    /// no balance.
    fn bench<T>(&self, bencher: Bencher<'_, '_>, rebuild: impl Fn(&Self, &LedgerView) -> T) {
        bencher
            .with_inputs(|| {
                let mut ledger = self.ledger.borrow_mut();
                let flagged = !self.flagged.get();
                ledger
                    .apply(DomainCommand::SetFlagged {
                        transaction: self.middle,
                        flagged,
                    })
                    .expect("the flag changes");
                self.flagged.set(flagged);
                ledger.snapshot()
            })
            .bench_local_refs(|view: &mut Arc<LedgerView>| rebuild(self, view));
    }
}

/// The day the stub's Registers are drawn on.
fn today() -> Date<Iso> {
    Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")
}

/// Formats every row of Household Checking's Register, sorted by date and unfiltered.
#[divan::bench(args = SCALES, sample_size = 1, sample_count = 30)]
fn register_rows(bencher: Bencher<'_, '_>, scale: u32) {
    let household = Household::new(scale);
    household.bench(bencher, |household, view| {
        build_register_model(view, household.account, &household.i18n, today())
    });
}

/// Collects the payees, categories and Payee recalls Household Checking's comboboxes
/// offer.
#[divan::bench(args = SCALES, sample_size = 1, sample_count = 30)]
fn field_choices(bencher: Bencher<'_, '_>, scale: u32) {
    let household = Household::new(scale);
    household.bench(bencher, |household, view| {
        build_field_choices(view, household.account, &household.i18n)
    });
}

/// Reads one category's usage, which counts every category's and account's usage, as
/// the Category List's first read after a change does.
#[divan::bench(args = SCALES, sample_size = 1, sample_count = 30)]
fn usage_count(bencher: Bencher<'_, '_>, scale: u32) {
    let household = Household::new(scale);
    household.bench(bencher, |household, view| {
        view.category_usage(household.groceries).count()
    });
}
