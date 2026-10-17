#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the plan's callers, mutants-landed and mutants-shard, come in #175 and #176"
    )
)]
mod key;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "mutants-landed and mutants-shard call the plan, in #175 and #176"
    )
)]
pub(crate) mod plan;

use anyhow::{Context as _, Result, bail};

use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "mutants",
    args: "[<base>] [--re <pattern>]",
    summary: "mutation-test code changed since <base> (default origin/main), or with --re \
              alone every mutant it matches; advisory",
    run: |args| Scope::parse(args).map(run),
};

/// Which mutants a by-hand run tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope<'a> {
    /// Those in code changed since `base`, narrowed to names matching `re` if given.
    ChangedSince { base: &'a str, re: Option<&'a str> },
    /// Every mutant whose name matches the pattern, wherever it is. An issue's
    /// reproduce command runs this way, since its mutant is rarely in the diff.
    Matching(&'a str),
}

impl<'a> Scope<'a> {
    fn parse(args: &[&'a str]) -> Option<Self> {
        match *args {
            [] => Some(Self::ChangedSince {
                base: "origin/main",
                re: None,
            }),
            ["--re", re] => Some(Self::Matching(re)),
            [base] if base != "--re" => Some(Self::ChangedSince { base, re: None }),
            [base, "--re", re] => Some(Self::ChangedSince { base, re: Some(re) }),
            _ => None,
        }
    }
}

/// Where a by-hand run writes the diff that `--in-diff` reads.
const DIFF_PATH: &str = "target/mutants.diff";

/// cargo-mutants' arguments for `scope`.
fn cargo_mutants_args(scope: &Scope<'_>) -> Vec<String> {
    let mut args = vec!["mutants", "--no-shuffle", "--cargo-arg=--locked"];
    let re = match *scope {
        Scope::ChangedSince { re, .. } => {
            args.extend(["--in-diff", DIFF_PATH]);
            re
        }
        Scope::Matching(re) => Some(re),
    };
    if let Some(re) = re {
        args.extend(["--re", re]);
    }
    args.into_iter().map(str::to_owned).collect()
}

fn run(scope: Scope<'_>) -> Result<()> {
    if let Scope::ChangedSince { base, .. } = scope {
        let diff = process::output("git", &["diff", &format!("{base}...")])?;
        std::fs::create_dir_all("target").context("creating target/")?;
        std::fs::write(DIFF_PATH, diff).with_context(|| format!("writing {DIFF_PATH}"))?;
    }
    let args = cargo_mutants_args(&scope);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut command = process::memory_capped("cargo", &args);
    let status = command
        .status()
        .with_context(|| format!("running {}", command.get_program().display()))?;
    match MutantsOutcome::from_exit_code(status.code()).context("cargo mutants failed")? {
        MutantsOutcome::AllCaught => {}
        MutantsOutcome::Missed => {
            eprintln!("advisory: missed mutants, see mutants.out/missed.txt");
        }
        MutantsOutcome::TimedOut => {
            eprintln!("advisory: mutants timed out, see mutants.out/timeout.txt");
        }
    }
    Ok(())
}

/// A finished cargo-mutants run. The check is advisory, so missed and timed-out
/// mutants are reported rather than failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MutantsOutcome {
    AllCaught,
    Missed,
    TimedOut,
}

impl MutantsOutcome {
    /// Reads cargo-mutants' exit code. Any code other than 0, 2 (missed) or
    /// 3 (timed out), or no code at all, means the run itself failed.
    fn from_exit_code(code: Option<i32>) -> Result<Self> {
        match code {
            Some(0) => Ok(Self::AllCaught),
            Some(2) => Ok(Self::Missed),
            Some(3) => Ok(Self::TimedOut),
            Some(code) => bail!("exit code {code}"),
            None => bail!("killed by a signal"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MutantsOutcome, Scope, cargo_mutants_args};

    fn argv(task_args: &[&str]) -> Option<Vec<String>> {
        Scope::parse(task_args).map(|scope| cargo_mutants_args(&scope))
    }

    #[test]
    fn a_pattern_alone_runs_the_matching_mutants_across_the_tree() {
        assert_eq!(
            argv(&["--re", "^a\\.rs:"]).unwrap(),
            [
                "mutants",
                "--no-shuffle",
                "--cargo-arg=--locked",
                "--re",
                "^a\\.rs:"
            ]
        );
    }

    #[test]
    fn a_base_and_a_pattern_run_the_matching_mutants_in_the_diff() {
        assert_eq!(
            argv(&["origin/main", "--re", "x"]).unwrap(),
            [
                "mutants",
                "--no-shuffle",
                "--cargo-arg=--locked",
                "--in-diff",
                "target/mutants.diff",
                "--re",
                "x"
            ]
        );
    }

    #[test]
    fn no_arguments_run_the_diff_against_origin_main() {
        assert_eq!(
            Scope::parse(&[]),
            Some(Scope::ChangedSince {
                base: "origin/main",
                re: None
            })
        );
        assert_eq!(
            argv(&[]).unwrap(),
            [
                "mutants",
                "--no-shuffle",
                "--cargo-arg=--locked",
                "--in-diff",
                "target/mutants.diff"
            ]
        );
    }

    #[test]
    fn malformed_arguments_are_a_usage_error() {
        assert_eq!(argv(&["--re"]), None);
        assert_eq!(argv(&["a", "b"]), None);
        assert_eq!(argv(&["--re", "x", "base"]), None);
    }

    #[test]
    fn missed_and_timed_out_mutants_are_advisory() {
        assert_eq!(
            MutantsOutcome::from_exit_code(Some(0)).ok(),
            Some(MutantsOutcome::AllCaught)
        );
        assert_eq!(
            MutantsOutcome::from_exit_code(Some(2)).ok(),
            Some(MutantsOutcome::Missed)
        );
        assert_eq!(
            MutantsOutcome::from_exit_code(Some(3)).ok(),
            Some(MutantsOutcome::TimedOut)
        );
    }

    #[test]
    fn usage_errors_baseline_failures_and_signals_fail_the_run() {
        // cargo-mutants exits 1 for a usage error and 4 when the unmutated tree fails.
        assert!(MutantsOutcome::from_exit_code(Some(1)).is_err());
        assert!(MutantsOutcome::from_exit_code(Some(4)).is_err());
        assert!(MutantsOutcome::from_exit_code(None).is_err());
    }
}
