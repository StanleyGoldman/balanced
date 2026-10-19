// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! A CI mutant run: test the mutants cargo-mutants is pointed at, then bring the
//! `mutant` issues in line with what it found (ADR 0002). `mutants-shard` and
//! `mutants-landed` differ only in which mutants they point it at.

use std::collections::BTreeSet;

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;

use super::MutantsOutcome;
use super::github::GitHub;
use super::plan::{self, Action, CloseReason, FoundBy, Run};
use crate::process;

/// What a run does with the issue updates its plan decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IssueUpdates {
    /// Make them on GitHub.
    Apply,
    /// Print them, and leave GitHub as it is.
    Print,
}

/// Where cargo-mutants writes `mutants.out`, so the run never adds to the
/// checkout's own files.
const OUTPUT_DIR: &str = "target";

/// The variable `.github/workflows/mutants.yml` sets to the web page of the
/// running workflow.
const PIPELINE_VAR: &str = "CI_RUN_URL";

/// Tests the mutants `scope_args` select at `HEAD`, then opens, comments on and
/// closes `mutant` issues as the plan says, or prints those updates. Missed and
/// timed-out mutants leave the run green; a run that can't finish, or can't
/// update its issues, is an error.
pub(crate) fn run(scope_args: &[String], found_by: FoundBy, updates: IssueUpdates) -> Result<()> {
    refuse_dirty_tree()?;
    let commit = process::output("git", &["rev-parse", "HEAD"])?
        .trim()
        .to_owned();
    let github = GitHub::from_env()?;
    // Read first, so a bad token fails the run before the hour of testing.
    let issues = github.open_mutant_issues()?;
    eprintln!("mutants: {} open mutant issues", issues.len());
    let list = process::output("cargo", &["mutants", "--list", "--json"])?;
    let packages = scoped_packages(scope_args)?;
    let outcomes = if packages.is_empty() {
        eprintln!("mutants: no mutants in scope, so testing none");
        None
    } else {
        Some(test(scope_args, &packages)?)
    };
    let run = Run {
        commit,
        found_by,
        pipeline: std::env::var(PIPELINE_VAR).ok(),
    };
    let actions = plan::plan(outcomes.as_deref(), &list, &issues, &run)?;
    eprintln!("mutants: {} issue updates", actions.len());
    for action in &actions {
        eprintln!("{}", describe(action));
        if updates == IssueUpdates::Apply {
            github.apply(action)?;
        }
    }
    Ok(())
}

/// The packages holding the mutants `scope_args` select. cargo-mutants prints
/// nothing at all, not an empty list, when they select none: for a diff that is
/// empty, changes no Rust, or changes only code it doesn't mutate.
fn scoped_packages(scope_args: &[String]) -> Result<BTreeSet<String>> {
    #[derive(Deserialize)]
    struct Listed {
        package: String,
    }
    let mut args = vec!["mutants", "--list", "--json"];
    args.extend(scope_args.iter().map(String::as_str));
    let list = process::output("cargo", &args)?;
    if list.trim().is_empty() {
        return Ok(BTreeSet::new());
    }
    let listed: Vec<Listed> =
        serde_json::from_str(&list).context("reading the mutants in scope")?;
    Ok(listed.into_iter().map(|listed| listed.package).collect())
}

/// Tests the mutants `scope_args` select, which `packages` hold, and returns
/// cargo-mutants' `outcomes.json`.
fn test(scope_args: &[String], packages: &BTreeSet<String>) -> Result<String> {
    warm_up(packages)?;
    let args = cargo_mutants_args(scope_args);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    // Incremental on and sccache off, the fastest per-mutant builds (ADR 0002). An
    // empty wrapper also overrides a `build.rustc-wrapper` in a developer's cargo
    // config.
    let code = process::exit_code_with_env(
        "cargo",
        &args,
        &[("CARGO_INCREMENTAL", "1"), ("RUSTC_WRAPPER", "")],
    )?;
    let outcome = MutantsOutcome::from_exit_code(code).context("cargo mutants failed")?;
    eprintln!("mutants: cargo mutants finished: {outcome:?}");
    let outcomes_path = format!("{OUTPUT_DIR}/mutants.out/outcomes.json");
    std::fs::read_to_string(&outcomes_path).with_context(|| format!("reading {outcomes_path}"))
}

/// Builds each of `packages`' tests the way cargo-mutants will, but through the
/// step's sccache with incremental compilation off, so their dependencies come from
/// the cache `ci` fills. cargo-mutants' own builds then find those dependencies
/// fresh: they rebuild only the workspace crates. The profile and features match
/// `.cargo/mutants.toml`.
fn warm_up(packages: &BTreeSet<String>) -> Result<()> {
    // One build per package, as cargo-mutants builds them: features unify only
    // across the packages a build selects.
    for package in packages {
        eprintln!("mutants: warming up {package}");
        process::run(
            "cargo",
            &[
                "nextest",
                "run",
                "--no-run",
                "--cargo-profile=mutants",
                &format!("--package={package}"),
                "--all-features",
                "--locked",
            ],
        )?;
    }
    Ok(())
}

/// cargo-mutants' arguments for a CI run over the mutants `scope_args` select:
/// mutated in place in the throwaway checkout, one mutant at a time. The run
/// starts beside the `ci` workflow, not after it, so the baseline proves the
/// unmutated tests pass first: on a broken commit every mutant would look
/// caught and close its issue. A mutant
/// whose tests run past a minute is a timeout; nextest's own kill comes later.
fn cargo_mutants_args(scope_args: &[String]) -> Vec<String> {
    let mut args: Vec<String> = [
        "mutants",
        "--no-shuffle",
        "--cargo-arg=--locked",
        "--in-place",
        "--timeout=60",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    args.push(format!("--output={OUTPUT_DIR}"));
    args.extend_from_slice(scope_args);
    args
}

/// Fails unless the checkout matches `HEAD`. The run mutates files in place, so on
/// a developer's tree it could mix their edits into its results, and a crash
/// mid-mutant would leave a mutation among them.
fn refuse_dirty_tree() -> Result<()> {
    let status = process::output("git", &["status", "--porcelain"])?;
    if !status.is_empty() {
        bail!(
            "the checkout has uncommitted changes, and this CI task mutates files in \
             place; by hand, use `cargo xtask mutants`:\n{status}"
        );
    }
    Ok(())
}

/// One line per update, then the text it posts, indented.
fn describe(action: &Action) -> String {
    let (head, text) = match action {
        Action::Open(issue) => (
            format!("open: {} [{}]", issue.title, issue.labels.join(", ")),
            &issue.body,
        ),
        Action::Comment { issue, body } => (format!("comment on #{issue}"), body),
        Action::Close {
            issue,
            reason,
            comment,
        } => {
            let reason = match reason {
                CloseReason::Caught => "caught",
                CloseReason::NoLongerGenerated => "no longer generated",
            };
            (format!("close #{issue} as {reason}"), comment)
        }
    };
    let text: Vec<String> = text.lines().map(|line| format!("    {line}")).collect();
    format!("{head}\n{}", text.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::cargo_mutants_args;

    #[test]
    fn a_ci_run_mutates_in_place_after_a_baseline_with_a_one_minute_timeout() {
        assert_eq!(
            cargo_mutants_args(&["--shard=3/7".to_owned()]),
            [
                "mutants",
                "--no-shuffle",
                "--cargo-arg=--locked",
                "--in-place",
                "--timeout=60",
                "--output=target",
                "--shard=3/7",
            ]
        );
    }
}
