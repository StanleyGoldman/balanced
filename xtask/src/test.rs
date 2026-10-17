use anyhow::Result;

use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "test",
    args: "",
    summary: "run every test except the image snapshots",
    run: |args| args.is_empty().then(run),
};

pub(crate) fn run() -> Result<()> {
    process::run(
        "cargo",
        &["test", "--locked", "--workspace", "--all-features"],
    )
}
