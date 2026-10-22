// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Times one frame of the App showing Household Checking's Register, in the stub written
//! to a data file, at the stub's size (20,000 transactions) and at ten times it: a frame
//! with nothing to do, and the frame that applies a Command and rebuilds what it
//! changed. The Command flags or unflags the transaction in the middle, which moves no
//! balance, so the difference between the two is the Command's cost, which the Commands
//! benchmark in `balanced_core` times, and the rebuilds.
//!
//! Run it in the `release` profile on a quiet machine, as `docs/practices/tooling.md`
//! describes: `taskset -c 0 cargo bench -p balanced_ui --features test-support --bench
//! frame`.

use std::cell::{Cell, RefCell};

use balanced_core::{DomainCommand, Ledger, TransactionId};
use balanced_i18n::Localizer;
use balanced_ui::testing::send_domain;
use balanced_ui::{App, Clock, PreferencesFile};
use divan::Bencher;
use eframe::egui::accesskit::Role;
use eframe::egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use icu_calendar::Date;
use tempfile::TempDir;

fn main() {
    divan::main();
}

/// How many transactions the stub's generated account holds at 1×.
const STUB_TRANSACTIONS: u32 = 20_000;

/// The sizes timed, as multiples of the stub's.
const SCALES: [u32; 2] = [1, 10];

/// The App on the stub at `scale` times its size, in a data file in a directory of its
/// own, showing Household Checking's Register.
struct Window {
    harness: RefCell<Harness<'static, App>>,
    /// The transaction in the middle of Household Checking, which each run's Command
    /// flags or unflags.
    middle: TransactionId,
    /// Whether the middle transaction is flagged now. The stub flags none.
    flagged: Cell<bool>,
    /// Holds the data file until the App is dropped.
    _directory: TempDir,
}

impl Window {
    fn new(scale: u32) -> Self {
        // Under the build's own directory, so the data file is on disk, where a commit
        // pays for its fsync as the app's does, rather than in a RAM-backed `/tmp`.
        let directory =
            TempDir::new_in(env!("CARGO_TARGET_TMPDIR")).expect("a temporary directory");
        let path = directory.path().join("household.balanced");
        let ledger = Ledger::create_stub(&path, STUB_TRANSACTIONS * scale)
            .expect("the stub is written to a new file");
        let view = ledger.snapshot();
        let register = view
            .accounts()
            .iter()
            .find(|summary| summary.account().name() == "Household Checking")
            .expect("the stub has Household Checking")
            .register();
        let middle = register
            .get(register.len() / 2)
            .expect("the register holds the entry")
            .transaction()
            .id();
        let preferences = PreferencesFile::in_dir(directory.path());
        let mut harness = Harness::builder()
            .with_size(vec2(1280.0, 720.0))
            .build_eframe(move |cc| {
                App::on_ledger(
                    cc,
                    Localizer::for_tests(),
                    ledger,
                    Clock::fixed(
                        Date::try_new_iso(2026, 10, 14).expect("14 October 2026 is a date"),
                    ),
                    preferences,
                )
            });
        harness.run();
        harness
            .get_by_role_and_label(Role::Button, "Household Checking")
            .click();
        harness.run();
        Self {
            harness: RefCell::new(harness),
            middle,
            flagged: Cell::new(false),
            _directory: directory,
        }
    }
}

/// A frame with no input and nothing changed.
#[divan::bench(args = SCALES, sample_size = 1, sample_count = 30)]
fn idle(bencher: Bencher<'_, '_>, scale: u32) {
    let window = Window::new(scale);
    bencher.bench_local(|| window.harness.borrow_mut().step());
}

/// The frame that applies a Command flagging or unflagging the middle transaction, and
/// rebuilds what the Register shows.
#[divan::bench(args = SCALES, sample_size = 1, sample_count = 30)]
fn after_a_command(bencher: Bencher<'_, '_>, scale: u32) {
    let window = Window::new(scale);
    bencher
        .with_inputs(|| {
            let flagged = !window.flagged.get();
            send_domain(
                &window.harness.borrow(),
                DomainCommand::SetFlagged {
                    transaction: window.middle,
                    flagged,
                },
            );
            window.flagged.set(flagged);
        })
        .bench_local_values(|()| window.harness.borrow_mut().step());
}
