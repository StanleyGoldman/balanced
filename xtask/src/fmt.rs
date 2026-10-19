// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

use anyhow::Result;

use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "fmt",
    args: "",
    summary: "check formatting",
    run: |args| args.is_empty().then(run),
};

pub(crate) fn run() -> Result<()> {
    process::run("cargo", &["fmt", "--all", "--check"])
}
