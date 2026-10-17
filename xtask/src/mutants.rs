use anyhow::{Context as _, Result, bail};

use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "mutants",
    args: "[<base>]",
    summary: "mutation-test code changed since <base> (default origin/main); advisory",
    run: |args| match args {
        [] => Some(run("origin/main")),
        [base] => Some(run(base)),
        _ => None,
    },
};

fn run(base: &str) -> Result<()> {
    let diff = process::output("git", &["diff", &format!("{base}...")])?;
    let diff_path = "target/mutants.diff";
    std::fs::create_dir_all("target").context("creating target/")?;
    std::fs::write(diff_path, diff).with_context(|| format!("writing {diff_path}"))?;
    let args = [
        "mutants",
        "--in-diff",
        diff_path,
        "--no-shuffle",
        "--cargo-arg=--locked",
    ];
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
    use super::MutantsOutcome;

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
