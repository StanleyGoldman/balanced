use anyhow::{Context as _, Result};

use crate::{Task, clippy, deny, fmt, locales, snapshots, test, todo, typos};

pub(crate) const TASK: Task = Task {
    name: "ci",
    args: "",
    summary: "run fmt, clippy, test, snapshots, locales, deny, typos and todo, stopping at the first failure",
    run: |args| args.is_empty().then(run),
};

type Step = fn() -> Result<()>;

fn run() -> Result<()> {
    let tasks: [(&str, Step); 8] = [
        (fmt::TASK.name, fmt::run),
        (clippy::TASK.name, clippy::run),
        (test::TASK.name, test::run),
        (snapshots::TASK.name, snapshots::run),
        (locales::TASK.name, locales::run),
        (deny::TASK.name, deny::run),
        (typos::TASK.name, typos::run),
        (todo::TASK.name, todo::run),
    ];
    for (name, task) in tasks {
        eprintln!("==> {name}");
        task().with_context(|| format!("{name} failed"))?;
    }
    Ok(())
}
