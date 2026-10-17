use anyhow::Result;

use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "typos",
    args: "",
    summary: "spell-check the repo",
    run: |args| args.is_empty().then(run),
};

pub(crate) fn run() -> Result<()> {
    process::run("typos", &[])
}
