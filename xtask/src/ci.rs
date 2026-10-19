// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

use anyhow::{Context as _, Result};

use crate::{
    Task, adrs, clippy, dco, deny, fmt, layers, locales, snapshots, spdx, test, todo, typos,
};

pub(crate) const TASK: Task = Task {
    name: "ci",
    args: "",
    summary: "run fmt, clippy, test, snapshots, locales, deny, layers, typos, todo, spdx, adrs and dco, stopping at the first failure",
    run: |args| args.is_empty().then(run),
};

type Step = fn() -> Result<()>;

fn run() -> Result<()> {
    let tasks: &[(&str, Step)] = &[
        (fmt::TASK.name, fmt::run),
        (clippy::TASK.name, clippy::run),
        (test::TASK.name, test::run),
        (snapshots::TASK.name, snapshots::run),
        (locales::TASK.name, locales::run),
        (deny::TASK.name, deny::run),
        (layers::TASK.name, layers::run),
        (typos::TASK.name, typos::run),
        (todo::TASK.name, todo::run),
        (spdx::TASK.name, spdx::run),
        (adrs::TASK.name, || adrs::run(None)),
        (dco::TASK.name, dco::run),
    ];
    for &(name, task) in tasks {
        eprintln!("==> {name}");
        task().with_context(|| format!("{name} failed"))?;
    }
    Ok(())
}
