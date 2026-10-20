// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

mod diff;
mod lcov;
mod report;

use std::fs;
use std::path::Path;

use anyhow::{Context as _, Result, bail};

use crate::{Task, process, test};

pub(crate) const TASK: Task = Task {
    name: "coverage",
    args: "[--base <ref>] [--fail-under-diff <percent>]",
    summary: "CI's test: run the suite and doctests, measuring line coverage into target/coverage \
              (lcov, html, summary.md); with --base, report how much of the lines changed since \
              <base> they ran",
    run: |args| Options::parse(args).map(|options| run(&options)),
};

/// Where the run writes its files.
const OUT_DIR: &str = "target/coverage";

/// Paths left out of the report: the build tooling, and the tests' own files.
const IGNORED: &str = r"(^|/)(xtask|tests)/";

#[derive(Debug, Default, PartialEq)]
struct Options<'a> {
    /// The ref the change is measured against.
    base: Option<&'a str>,
    /// Fail when the changed lines' coverage is below this percentage.
    fail_under_diff: Option<f64>,
}

impl<'a> Options<'a> {
    fn parse(args: &[&'a str]) -> Option<Self> {
        let mut options = Self::default();
        let mut rest = args;
        while let [flag, value, tail @ ..] = rest {
            match *flag {
                "--base" => options.base = Some(value),
                "--fail-under-diff" => options.fail_under_diff = Some(value.parse().ok()?),
                _ => return None,
            }
            rest = tail;
        }
        rest.is_empty().then_some(options)
    }
}

/// Runs the suite under cargo-llvm-cov, then the doctests, and writes the report.
/// This is `test` with coverage measured, for CI. Doctests run but aren't counted:
/// measuring them needs nightly.
fn run(options: &Options<'_>) -> Result<()> {
    let lcov_path = format!("{OUT_DIR}/lcov.info");
    fs::create_dir_all(OUT_DIR).with_context(|| format!("creating {OUT_DIR}"))?;
    process::run(
        "cargo",
        &[
            "llvm-cov",
            "nextest",
            "--locked",
            "--workspace",
            "--all-features",
            "--ignore-filename-regex",
            IGNORED,
            "--lcov",
            "--output-path",
            &lcov_path,
        ],
    )?;
    test::doctests()?;
    process::run(
        "cargo",
        &[
            "llvm-cov",
            "report",
            "--ignore-filename-regex",
            IGNORED,
            "--html",
            "--output-dir",
            OUT_DIR,
        ],
    )?;

    let root = std::env::current_dir().context("finding the workspace root")?;
    let root = root.to_str().context("the workspace path isn't UTF-8")?;
    let files = lcov::parse(&fs::read_to_string(&lcov_path)?, root)?;
    let diff = match options.base {
        Some(base) => {
            let range = format!("{base}...HEAD");
            let text =
                process::output("git", &["diff", "-U0", "--no-color", &range, "--", "*.rs"])?;
            Some(report::diff_coverage(&files, &diff::added_lines(&text)))
        }
        None => None,
    };

    let summary = Path::new(OUT_DIR).join("summary.md");
    fs::write(
        &summary,
        report::render(
            &files,
            diff.as_ref(),
            "The artifact holds the full HTML report.",
        ),
    )
    .with_context(|| format!("writing {}", summary.display()))?;
    eprintln!("coverage report: {}", summary.display());

    if let (Some(min), Some(diff)) = (options.fail_under_diff, &diff) {
        // Nothing to cover passes: a change with no code on its added lines can't fall short.
        if let Some(percent) = diff.total.percent().filter(|&p| p < min) {
            bail!("changed lines are {percent:.1}% covered, under the {min}% minimum");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_parses_its_flags() {
        assert_eq!(Options::parse(&[]), Some(Options::default()));
        assert_eq!(
            Options::parse(&["--base", "abc", "--fail-under-diff", "80"]),
            Some(Options {
                base: Some("abc"),
                fail_under_diff: Some(80.0)
            })
        );
        assert_eq!(Options::parse(&["--base"]), None);
        assert_eq!(Options::parse(&["--fail-under-diff", "x"]), None);
    }
}
