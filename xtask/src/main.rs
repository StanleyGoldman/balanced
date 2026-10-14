//! Workspace automation, run as `cargo xtask <task>`.
//!
//! Every check a CI runs lives here, so any CI provider only has to call
//! `cargo xtask ci`.

#![expect(
    clippy::print_stderr,
    reason = "xtask reports progress and failures on the terminal"
)]

use std::env;
use std::path::Path;
use std::process::{Command, ExitCode};

use anyhow::{Context as _, Result, anyhow, bail};

const USAGE: &str = "\
usage: cargo xtask <task>

tasks:
  ci                run fmt, clippy, test, locales, deny, typos and todo, stopping at the first failure
  fmt               check formatting
  clippy            lint with warnings denied
  test              run every test
  locales           report untranslated messages per locale
  deny              check licences, advisories, duplicate versions and layer bans
  typos             spell-check the repo
  todo              require every TODO and FIXME to name an issue, as TODO(#123)
  mutants [<base>]  mutation-test code changed since <base> (default origin/main); advisory";

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
    let result = match args.as_slice() {
        ["ci"] => ci(),
        ["fmt"] => fmt(),
        ["clippy"] => clippy(),
        ["test"] => test(),
        ["locales"] => locales(),
        ["deny"] => deny(),
        ["typos"] => typos(),
        ["todo"] => todo(),
        ["mutants"] => mutants("origin/main"),
        ["mutants", base] => mutants(base),
        _ => Err(anyhow!(USAGE)),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err:#}");
            ExitCode::FAILURE
        }
    }
}

type Task = fn() -> Result<()>;

fn ci() -> Result<()> {
    let tasks: [(&str, Task); 7] = [
        ("fmt", fmt),
        ("clippy", clippy),
        ("test", test),
        ("locales", locales),
        ("deny", deny),
        ("typos", typos),
        ("todo", todo),
    ];
    for (name, task) in tasks {
        eprintln!("==> {name}");
        task().with_context(|| format!("{name} failed"))?;
    }
    Ok(())
}

fn fmt() -> Result<()> {
    run("cargo", &["fmt", "--all", "--check"])
}

fn clippy() -> Result<()> {
    run(
        "cargo",
        &[
            "clippy",
            "--locked",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    )
}

fn test() -> Result<()> {
    run(
        "cargo",
        &["test", "--locked", "--workspace", "--all-features"],
    )
}

/// Reruns the locale-consistency test uncaptured, so its untranslated counts show.
fn locales() -> Result<()> {
    run(
        "cargo",
        &[
            "test",
            "--locked",
            "--package",
            "balanced_i18n",
            "--test",
            "locales",
            "--quiet",
            "--",
            "--nocapture",
        ],
    )
}

fn deny() -> Result<()> {
    run("cargo", &["deny", "--locked", "check"])
}

fn typos() -> Result<()> {
    run("typos", &[])
}

fn todo() -> Result<()> {
    let files = output("git", &["ls-files", "*.rs", "*.toml"])?;
    let mut bare = Vec::new();
    // This file spells out bare markers in its usage text and tests.
    for path in files.lines().filter(|path| *path != "xtask/src/main.rs") {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
        for (index, line) in text.lines().enumerate() {
            if has_bare_todo(line) {
                bare.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    if !bare.is_empty() {
        bail!(
            "write TODO(#<issue>) instead of a bare TODO or FIXME:\n{}",
            bare.join("\n")
        );
    }
    Ok(())
}

/// Whether `line` holds a `TODO` or `FIXME` word that isn't followed by `(#<digits>)`.
fn has_bare_todo(line: &str) -> bool {
    ["TODO", "FIXME"].iter().any(|marker| {
        line.match_indices(marker).any(|(start, _)| {
            // `match_indices` yields char boundaries, so both slices are in range.
            let before = line[..start].chars().next_back();
            let after = &line[start + marker.len()..];
            let is_word = !before.is_some_and(is_word_char)
                && !after.chars().next().is_some_and(is_word_char);
            is_word && !names_issue(after)
        })
    })
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether `rest` starts with `(#<digits>)`.
fn names_issue(rest: &str) -> bool {
    rest.strip_prefix("(#")
        .and_then(|rest| rest.split_once(')'))
        .is_some_and(|(number, _)| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
}

/// The memory cap for a mutants run. A mutant can make a loop allocate without end,
/// filling RAM before cargo-mutants' per-mutant timeout fires, and on Linux the
/// kernel's OOM kill then takes the whole terminal session down with it.
const MUTANTS_MEMORY_MAX: &str = "MemoryMax=8G";

/// `program` with `args` in a memory-capped systemd scope on Linux, where the cap
/// kills only the runaway test. Elsewhere, the plain command, with a warning.
fn memory_capped(program: &str, args: &[&str]) -> Command {
    if cfg!(target_os = "linux") {
        let mut capped = Command::new("systemd-run");
        capped
            .args(["--user", "--scope", "--quiet", "-p", MUTANTS_MEMORY_MAX])
            .args(["-p", "MemorySwapMax=0", "--", program])
            .args(args);
        return capped;
    }
    eprintln!("warning: no memory cap on this OS; a runaway mutant can exhaust RAM");
    let mut plain = Command::new(program);
    plain.args(args);
    plain
}

fn mutants(base: &str) -> Result<()> {
    let diff = output("git", &["diff", &format!("{base}...")])?;
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
    let mut command = memory_capped("cargo", &args);
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

fn run(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("running {program}"))?;
    if !status.success() {
        bail!("{program} {} exited with {status}", args.join(" "));
    }
    Ok(())
}

fn output(program: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("running {program}"))?;
    if !out.status.success() {
        bail!(
            "{program} {} exited with {}: {}",
            args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    String::from_utf8(out.stdout).with_context(|| format!("{program} printed invalid UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::{MutantsOutcome, has_bare_todo};

    #[test]
    fn bare_markers_are_flagged() {
        assert!(has_bare_todo("// TODO fix"));
        assert!(has_bare_todo("// FIXME: x"));
        assert!(has_bare_todo("// TODO(stan) owner is not an issue"));
        assert!(has_bare_todo("// TODO(#) empty number"));
    }

    #[test]
    fn issue_markers_and_other_words_pass() {
        assert!(!has_bare_todo("// TODO(#12) tracked"));
        assert!(!has_bare_todo("// FIXME(#3) tracked"));
        assert!(!has_bare_todo("let todos = TODOS;"));
        assert!(!has_bare_todo("// todo lower case"));
    }

    #[test]
    fn a_bare_marker_after_a_tracked_one_is_flagged() {
        assert!(has_bare_todo("// TODO(#12) tracked, TODO not"));
    }

    #[test]
    fn malformed_issue_references_are_flagged() {
        assert!(has_bare_todo("// TODO(#12 unclosed"));
        assert!(has_bare_todo("// TODO(#12a) not a number"));
    }

    #[test]
    fn markers_inside_longer_words_pass() {
        assert!(!has_bare_todo("let MY_TODO = TODOS + FIXMEX;"));
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
