// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! Workspace automation, run as `cargo xtask <task>`.
//!
//! Every check a CI runs lives here, so any CI provider only has to call
//! `cargo xtask ci`. Each task is a module with a `TASK`, listed once in [`TASKS`].

#![expect(
    clippy::print_stderr,
    reason = "xtask reports progress and failures on the terminal"
)]

mod adrs;
mod ci;
mod clippy;
mod coverage;
mod dco;
mod deny;
mod fmt;
mod layers;
mod locales;
mod mutants;
mod process;
mod snapshots;
mod spdx;
mod test;
mod todo;
mod typos;

use std::env;
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Result, anyhow};

/// One `cargo xtask` task, as the dispatcher and the usage text see it.
struct Task {
    name: &'static str,
    /// The arguments after the name, as the usage text shows them; empty for none.
    args: &'static str,
    summary: &'static str,
    /// Runs the task with the arguments after its name, or returns `None` when they
    /// don't fit its usage.
    run: fn(&[&str]) -> Option<Result<()>>,
}

/// Every task, in the order the usage text lists them.
const TASKS: &[Task] = &[
    ci::TASK,
    fmt::TASK,
    clippy::TASK,
    test::TASK,
    snapshots::TASK,
    coverage::TASK,
    locales::TASK,
    deny::TASK,
    layers::TASK,
    typos::TASK,
    todo::TASK,
    spdx::TASK,
    adrs::TASK,
    dco::TASK,
    mutants::TASK,
    mutants::shard::TASK,
    mutants::landed::TASK,
    mutants::all::TASK,
];

fn main() -> ExitCode {
    // Every task resolves paths from the workspace root, wherever cargo was run.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits one level below the workspace root");
    if let Err(err) = env::set_current_dir(root) {
        eprintln!("entering the workspace root: {err}: {}", root.display());
        return ExitCode::FAILURE;
    }
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = args
        .split_first()
        .and_then(|(name, rest)| {
            let task = TASKS.iter().find(|task| task.name == *name)?;
            (task.run)(rest)
        })
        .unwrap_or_else(|| Err(anyhow!(usage())));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err:#}");
            ExitCode::FAILURE
        }
    }
}

/// The usage text: every task with its arguments, then its summary in an aligned column.
fn usage() -> String {
    let synopsis = |task: &Task| {
        if task.args.is_empty() {
            task.name.to_owned()
        } else {
            format!("{} {}", task.name, task.args)
        }
    };
    let width = TASKS
        .iter()
        .map(|task| synopsis(task).len())
        .max()
        .unwrap_or(0)
        + 2;
    let tasks: Vec<String> = TASKS
        .iter()
        .map(|task| format!("  {:<width$}{}", synopsis(task), task.summary))
        .collect();
    format!("usage: cargo xtask <task>\n\ntasks:\n{}", tasks.join("\n"))
}
