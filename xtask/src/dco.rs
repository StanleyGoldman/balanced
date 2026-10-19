// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! `cargo xtask dco`: every commit since the project took up the Developer
//! Certificate of Origin carries a `Signed-off-by:` trailer (ADR 0009).
//!
//! The range starts at a fixed commit, not at `origin/main`. CI fetches a push or a
//! cron run by its SHA alone, and a stacked pull request with only the branch below
//! it, so `origin/main` exists in neither. Every commit a check passed stays passed,
//! so `main` itself is checked on each push. Merge commits are exempt: GitHub's merge
//! button and its "Update branch" make them, unsigned, and no one can amend those.

use std::path::Path;

use anyhow::{Context as _, Result, bail};

use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "dco",
    args: "",
    summary: "require a Signed-off-by trailer on every non-merge commit since the DCO began",
    run: |args| args.is_empty().then(run),
};

/// The last commit before the DCO: `main` when ADR 0009 was adopted. Every non-merge
/// commit after it needs a sign-off. When an unsigned commit lands on `main`
/// anyway, move this to that commit, naming it in a comment, since history on
/// `main` can't be rewritten.
const CUTOFF: &str = "b9b304523194e4444021c04b8b05b2f11784a7bf";

pub(crate) fn run() -> Result<()> {
    let commits = commits(Path::new("."), CUTOFF)?;
    check(&commits)?;
    eprintln!("dco: {} commits checked", commits.len());
    Ok(())
}

/// One commit in the range, as the check reads it.
#[derive(Debug)]
struct Commit {
    /// The abbreviated hash and the subject, as the failure names it.
    summary: String,
    signed_off: bool,
}

/// The non-merge commits after `cutoff` in `HEAD`'s history, newest first.
///
/// A shallow clone can lack `cutoff`, and `cutoff..HEAD` would then fail with
/// git's own message, so it is resolved first.
fn commits(root: &Path, cutoff: &str) -> Result<Vec<Commit>> {
    let root = root.to_str().context("the repository path is not UTF-8")?;
    process::output(
        "git",
        &[
            "-C",
            root,
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{cutoff}^{{commit}}"),
        ],
    )
    .with_context(|| {
        format!(
            "the DCO cutoff {cutoff} is not in this history; fetch the full history \
             (`git fetch --unshallow`)"
        )
    })?;
    // One record per commit, ended by NUL: the sign-off lines, a tab, then the
    // summary. The subject is last, so a tab inside it can't shift a field.
    let log = process::output(
        "git",
        &[
            "-C",
            root,
            "log",
            "--no-merges",
            "--format=%(trailers:key=Signed-off-by,valueonly,separator=%x2C)%x09%h %s%x00",
            &format!("{cutoff}..HEAD"),
        ],
    )?;
    log.split('\0')
        // git puts a newline between records; the tab after empty sign-offs stays.
        .map(|record| record.trim_start_matches('\n'))
        .filter(|record| !record.is_empty())
        .map(|record| {
            let (sign_offs, summary) = record
                .split_once('\t')
                .with_context(|| format!("git log printed a record without a tab: {record}"))?;
            Ok(Commit {
                summary: summary.to_owned(),
                signed_off: !sign_offs.trim().is_empty(),
            })
        })
        .collect()
}

/// Fails naming every commit without a sign-off, all in one message.
fn check(commits: &[Commit]) -> Result<()> {
    let unsigned: Vec<&str> = commits
        .iter()
        .filter(|commit| !commit.signed_off)
        .map(|commit| commit.summary.as_str())
        .collect();
    if !unsigned.is_empty() {
        bail!(
            "these commits have no Signed-off-by trailer:\n  {}\n\nsign off each one, as \
             CONTRIBUTING.md says: `git rebase --signoff <base>` adds the trailer to every \
             commit after <base>",
            unsigned.join("\n  ")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway repository, so no test depends on this one's history.
    struct Scratch(tempfile::TempDir);

    impl Scratch {
        /// A repository on `main` holding one unsigned commit, `before`, the cutoff.
        fn new() -> Self {
            let scratch = Self(tempfile::tempdir().expect("a scratch directory"));
            scratch.git(&["init", "-q", "-b", "main"]);
            scratch.commit("before", "before");
            scratch
        }

        fn git(&self, args: &[&str]) -> String {
            let output = std::process::Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
                .args(["-c", "commit.gpgsign=false"])
                .args(args)
                .current_dir(self.root())
                .output()
                .expect("git runs");
            assert!(output.status.success(), "git {args:?}: {output:?}");
            String::from_utf8(output.stdout).expect("git prints UTF-8")
        }

        /// Commits a change to `file` with `message` as the whole commit message.
        fn commit(&self, file: &str, message: &str) {
            std::fs::write(self.root().join(file), message).expect("the file is written");
            self.git(&["add", "-A"]);
            self.git(&["commit", "-q", "-m", message]);
        }

        fn head(&self) -> String {
            self.git(&["rev-parse", "HEAD"]).trim().to_owned()
        }

        fn root(&self) -> &Path {
            self.0.path()
        }
    }

    const SIGNED: &str = "Signed\n\nSigned-off-by: t <t@example.com>";

    /// The summaries of the commits after `cutoff` that lack a sign-off.
    fn unsigned(scratch: &Scratch, cutoff: &str) -> Vec<String> {
        commits(scratch.root(), cutoff)
            .expect("the commits are read")
            .into_iter()
            .filter(|commit| !commit.signed_off)
            .map(|commit| {
                commit
                    .summary
                    .split_once(' ')
                    .expect("hash and subject")
                    .1
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn a_commit_after_the_cutoff_needs_a_sign_off() {
        let scratch = Scratch::new();
        let cutoff = scratch.head();
        scratch.commit("a", SIGNED);
        scratch.commit("b", "Unsigned");

        assert_eq!(unsigned(&scratch, &cutoff), ["Unsigned"]);
        assert_eq!(
            commits(scratch.root(), &cutoff)
                .expect("the commits are read")
                .len(),
            2
        );
    }

    #[test]
    fn the_cutoff_and_commits_before_it_are_not_checked() {
        let scratch = Scratch::new();
        scratch.commit("a", "Also before");
        let cutoff = scratch.head();
        scratch.commit("b", SIGNED);

        assert_eq!(unsigned(&scratch, &cutoff), Vec::<String>::new());
    }

    /// The trailer counts only in the closing block, where `git commit -s` puts it.
    #[test]
    fn a_sign_off_inside_the_body_is_not_a_trailer() {
        let scratch = Scratch::new();
        let cutoff = scratch.head();
        scratch.commit(
            "a",
            "Quoted\n\nSigned-off-by: t <t@example.com>\n\nMore text after it.",
        );

        assert_eq!(unsigned(&scratch, &cutoff), ["Quoted"]);
    }

    #[test]
    fn a_merge_commit_needs_no_sign_off() {
        let scratch = Scratch::new();
        let cutoff = scratch.head();
        scratch.git(&["checkout", "-q", "-b", "topic"]);
        scratch.commit("a", SIGNED);
        scratch.git(&["checkout", "-q", "main"]);
        scratch.commit("b", SIGNED);
        scratch.git(&["merge", "-q", "--no-ff", "-m", "Merge topic", "topic"]);

        assert_eq!(unsigned(&scratch, &cutoff), Vec::<String>::new());
        assert_eq!(
            commits(scratch.root(), &cutoff)
                .expect("the commits are read")
                .len(),
            2
        );
    }

    #[test]
    fn a_cutoff_missing_from_history_fails_naming_it() {
        let scratch = Scratch::new();
        let missing = "0123456789abcdef0123456789abcdef01234567";

        let message = format!(
            "{:#}",
            commits(scratch.root(), missing).expect_err("the cutoff is not in this history")
        );

        assert!(
            message.starts_with(
                "the DCO cutoff 0123456789abcdef0123456789abcdef01234567 is not in this \
                 history; fetch the full history (`git fetch --unshallow`)"
            ),
            "{message}"
        );
    }

    #[test]
    fn the_failure_lists_every_unsigned_commit_and_the_fix() {
        let commit = |summary: &str, signed_off| Commit {
            summary: summary.to_owned(),
            signed_off,
        };

        let message = format!(
            "{:#}",
            check(&[
                commit("abc1234 Second", false),
                commit("def5678 Signed", true),
                commit("0a1b2c3 First", false),
            ])
            .expect_err("two commits lack a sign-off")
        );

        assert_eq!(
            message,
            "these commits have no Signed-off-by trailer:
  abc1234 Second
  0a1b2c3 First

sign off each one, as CONTRIBUTING.md says: `git rebase --signoff <base>` adds the \
trailer to every commit after <base>"
        );
        check(&[commit("def5678 Signed", true)]).expect("every commit is signed off");
    }
}
