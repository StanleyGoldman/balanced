// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

use anyhow::Result;

use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "test",
    args: "",
    summary: "run every test except the image snapshots",
    run: |args| args.is_empty().then(run),
};

/// Runs the suite under nextest, then the doctests, which nextest doesn't run.
pub(crate) fn run() -> Result<()> {
    process::run(
        "cargo",
        &[
            "nextest",
            "run",
            "--locked",
            "--workspace",
            "--all-features",
        ],
    )?;
    doctests()
}

/// Runs the doctests, which nextest doesn't run.
pub(crate) fn doctests() -> Result<()> {
    process::run(
        "cargo",
        &["test", "--locked", "--workspace", "--all-features", "--doc"],
    )
}
