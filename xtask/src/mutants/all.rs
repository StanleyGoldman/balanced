// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! `cargo xtask mutants-all`: the CI run, started by hand, that mutation-tests every
//! mutant on `main` in one go and reconciles the `mutant` issues (ADR 0002).

use anyhow::Result;

use super::plan::FoundBy;
use super::reconcile::{self, IssueUpdates};
use crate::Task;

pub(crate) const TASK: Task = Task {
    name: "mutants-all",
    args: "[--dry-run]",
    summary: "a CI task: mutation-test every mutant, all seven shards in one run, and update \
              the mutant issues, or with --dry-run print the updates; by hand, use mutants",
    run: |args| match *args {
        [] => Some(run(IssueUpdates::Apply)),
        ["--dry-run"] => Some(run(IssueUpdates::Print)),
        _ => None,
    },
};

fn run(updates: IssueUpdates) -> Result<()> {
    eprintln!("mutants-all: testing every mutant");
    // No scope arguments: cargo-mutants tests every mutant `.cargo/mutants.toml`
    // generates.
    reconcile::run(&[], FoundBy::Everything, updates)
}
