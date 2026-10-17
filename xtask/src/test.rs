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
    process::run(
        "cargo",
        &["test", "--locked", "--workspace", "--all-features", "--doc"],
    )
}
