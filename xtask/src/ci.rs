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

/// The tasks `ci` runs, in order. `.github/workflows/ci.yml` runs each as a step
/// of its own, so the Actions page shows which one failed.
const STEPS: &[(&str, Step)] = &[
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

fn run() -> Result<()> {
    for &(name, task) in STEPS {
        eprintln!("==> {name}");
        task().with_context(|| format!("{name} failed"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn the_workflow_runs_the_tasks_ci_runs() {
        let workflow = include_str!("../../.github/workflows/ci.yml");
        let in_workflow: BTreeSet<&str> = workflow
            .lines()
            .filter_map(|line| {
                line.trim()
                    .strip_prefix("run: .github/in-image.sh cargo xtask ")
            })
            .collect();
        let in_ci: BTreeSet<&str> = STEPS.iter().map(|&(name, _)| name).collect();
        assert_eq!(in_workflow, in_ci);
    }
}
