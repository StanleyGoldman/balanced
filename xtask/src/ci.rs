use anyhow::{Context as _, Result};

use crate::{Task, adrs, clippy, deny, fmt, layers, locales, snapshots, test, todo, typos};

pub(crate) const TASK: Task = Task {
    name: "ci",
    args: "",
    summary: "run fmt, clippy, test, snapshots, locales, deny, layers, typos, todo and adrs, stopping at the first failure",
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
        (adrs::TASK.name, || adrs::run(None)),
    ];
    for &(name, task) in tasks {
        eprintln!("==> {name}");
        task().with_context(|| format!("{name} failed"))?;
    }
    Ok(())
}
