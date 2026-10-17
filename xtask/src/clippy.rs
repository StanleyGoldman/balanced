use anyhow::Result;

use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "clippy",
    args: "",
    summary: "lint with warnings denied",
    run: |args| args.is_empty().then(run),
};

pub(crate) fn run() -> Result<()> {
    process::run(
        "cargo",
        &[
            "clippy",
            "--locked",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    )
}
