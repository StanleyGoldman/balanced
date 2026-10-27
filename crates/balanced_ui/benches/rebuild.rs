// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Times what a window rebuilds on its first frame after each Command, on Household
//! Checking in the stub, at the stub's size (20,000 transactions), at a busy household's
//! 30 years (90,000) and at ten times the stub (200,000): the Register's rows, from
//! scratch and from the rows before the Command, the Payee and Category comboboxes'
//! choices, and the usage count the Category List reads. Each run reads a snapshot no
//! earlier run has read, taken after an untimed Command.
//!
//! Run it in the `release` profile on a quiet machine, as `docs/practices/tooling.md`
//! describes: `taskset -c 0 cargo bench -p balanced_ui --features test-support --bench
//! rebuild`.

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use balanced_core::{AccountId, CategoryId, DomainCommand, Ledger, LedgerView, TransactionId};
use balanced_i18n::Localizer;
use balanced_ui::testing::{RegisterRows, build_field_choices, build_register_model};
use divan::Bencher;
use icu_calendar::{Date, Iso};
use tempfile::TempDir;

fn main() {
    divan::main();
}

/// The sizes timed: how many transactions the stub's generated account holds.
const SIZES: [u32; 3] = [20_000, 90_000, 200_000];

/// The stub with `size` transactions in its generated account, in a data file in a directory of its own, with
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
    fn new(size: u32) -> Self {
        let directory =
            TempDir::new_in(env!("CARGO_TARGET_TMPDIR")).expect("a temporary directory");
        let path = directory.path().join("household.balanced");
        let ledger = Ledger::create_stub(&path, size).expect("the stub is written to a new file");
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
            .with_inputs(|| self.flag())
            .bench_local_refs(|view: &mut Arc<LedgerView>| rebuild(self, view));
    }

    /// Flags or unflags the middle transaction, and returns the snapshot after.
    fn flag(&self) -> Arc<LedgerView> {
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
    }

    /// Household Checking's Register rows in `view`, from scratch or from `earlier`.
    fn rows(&self, view: &LedgerView, earlier: Option<RegisterRows>) -> RegisterRows {
        build_register_model(view, self.account, &self.i18n, today(), earlier)
    }
}

/// The day the stub's Registers are drawn on.
fn today() -> Date<Iso> {
    Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date")
}

/// Formats every row of Household Checking's Register, sorted by date and unfiltered,
/// as on opening the account.
#[divan::bench(args = SIZES, sample_size = 1, sample_count = 30)]
fn register_rows(bencher: Bencher<'_, '_>, size: u32) {
    let household = Household::new(size);
    household.bench(bencher, |household, view| household.rows(view, None));
}

/// Rebuilds Household Checking's Register rows from the rows before the Command, as an
/// open Register does.
#[divan::bench(args = SIZES, sample_size = 1, sample_count = 30)]
fn register_rows_after_a_command(bencher: Bencher<'_, '_>, size: u32) {
    let household = Household::new(size);
    bencher
        .with_inputs(|| {
            let earlier = household.rows(&household.ledger.borrow().snapshot(), None);
            (household.flag(), earlier)
        })
        .bench_local_values(|(view, earlier)| household.rows(&view, Some(earlier)));
}

/// Collects the payees, categories and Payee recalls Household Checking's comboboxes
/// offer.
#[divan::bench(args = SIZES, sample_size = 1, sample_count = 30)]
fn field_choices(bencher: Bencher<'_, '_>, size: u32) {
    let household = Household::new(size);
    household.bench(bencher, |household, view| {
        build_field_choices(view, household.account, &household.i18n)
    });
}

/// Reads one category's usage, which counts every category's and account's usage, as
/// the Category List's first read after a change does.
#[divan::bench(args = SIZES, sample_size = 1, sample_count = 30)]
fn usage_count(bencher: Bencher<'_, '_>, size: u32) {
    let household = Household::new(size);
    household.bench(bencher, |household, view| {
        view.category_usage(household.groceries).count()
    });
}
