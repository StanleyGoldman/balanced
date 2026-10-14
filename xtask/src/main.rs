//! Workspace automation, run as `cargo xtask <task>`.
//!
//! Every check a CI runs lives here, so any CI provider only has to call
//! `cargo xtask ci`.

#![expect(
    clippy::print_stderr,
    reason = "xtask reports progress and failures on the terminal"
)]

use std::env;
use std::process::{Command, ExitCode};

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
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

type Task = fn() -> Result<(), String>;

fn ci() -> Result<(), String> {
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
        task().map_err(|message| format!("{name} failed: {message}"))?;
    }
    Ok(())
}

fn fmt() -> Result<(), String> {
    run("cargo", &["fmt", "--all", "--check"])
}

fn clippy() -> Result<(), String> {
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

fn test() -> Result<(), String> {
    run(
        "cargo",
        &["test", "--locked", "--workspace", "--all-features"],
    )
}

/// Reruns the locale-consistency test uncaptured, so its untranslated counts show.
fn locales() -> Result<(), String> {
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

fn deny() -> Result<(), String> {
    run("cargo", &["deny", "--locked", "check"])
}

fn typos() -> Result<(), String> {
    run("typos", &[])
}

fn todo() -> Result<(), String> {
    let files = output("git", &["ls-files", "*.rs", "*.toml"])?;
    let mut bare = Vec::new();
    // This file spells out bare markers in its usage text and tests.
    for path in files.lines().filter(|path| *path != "xtask/src/main.rs") {
        let text = std::fs::read_to_string(path).map_err(|err| format!("reading {path}: {err}"))?;
        for (index, line) in text.lines().enumerate() {
            if has_bare_todo(line) {
                bare.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    if bare.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "write TODO(#<issue>) instead of a bare TODO or FIXME:\n{}",
            bare.join("\n")
        ))
    }
}

/// Whether `line` holds a `TODO` or `FIXME` word that isn't followed by `(#<digits>)`.
fn has_bare_todo(line: &str) -> bool {
    ["TODO", "FIXME"].iter().any(|marker| {
        line.match_indices(marker).any(|(start, _)| {
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

/// `command` in a memory-capped systemd scope on Linux, where the cap kills only
/// the runaway test. Elsewhere, `command` as it is, with a warning.
fn memory_capped(command: &[&str]) -> Command {
    let (program, args) = command.split_first().expect("a command names its program");
    if cfg!(target_os = "linux") {
        let mut capped = Command::new("systemd-run");
        capped
            .args(["--user", "--scope", "--quiet", "-p", MUTANTS_MEMORY_MAX])
            .args(["-p", "MemorySwapMax=0", "--"])
            .args(command);
        return capped;
    }
    eprintln!("warning: no memory cap on this OS; a runaway mutant can exhaust RAM");
    let mut plain = Command::new(program);
    plain.args(args);
    plain
}

fn mutants(base: &str) -> Result<(), String> {
    let diff = output("git", &["diff", &format!("{base}...")])?;
    let diff_path = "target/mutants.diff";
    std::fs::create_dir_all("target").map_err(|err| format!("creating target/: {err}"))?;
    std::fs::write(diff_path, diff).map_err(|err| format!("writing {diff_path}: {err}"))?;
    let mutants = ["cargo", "mutants", "--in-diff", diff_path, "--no-shuffle"];
    let status = memory_capped(&mutants)
        .status()
        .map_err(|err| format!("running cargo mutants: {err}"))?;
    // Advisory until balanced_core and balanced_i18n reach zero missed mutants:
    // missed (2) and timed-out (3) mutants are reported, not failed.
    match status.code() {
        Some(0) => Ok(()),
        Some(2) => {
            eprintln!("advisory: missed mutants, see mutants.out/missed.txt");
            Ok(())
        }
        Some(3) => {
            eprintln!("advisory: mutants timed out, see mutants.out/timeout.txt");
            Ok(())
        }
        _ => Err(format!("cargo mutants failed: {status}")),
    }
}

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|err| format!("running {program}: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {} exited with {status}", args.join(" ")))
    }
}

fn output(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("running {program}: {err}"))?;
    if out.status.success() {
        String::from_utf8(out.stdout)
            .map_err(|err| format!("{program} printed invalid UTF-8: {err}"))
    } else {
        Err(format!(
            "{program} {} exited with {}: {}",
            args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::has_bare_todo;

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
}
