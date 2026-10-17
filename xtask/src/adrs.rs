//! `cargo xtask adrs`: every decision record carries its own number.
//!
//! Two pull requests can each add a record with the same number and both merge
//! clean, because the file names differ, so the clash exists only in the merged
//! tree. A number is cited ("see ADR-0007"), so it must name one record.
//! `--against <ref>` reads that merged tree; without it, the working tree.
//! A gap in the sequence is fine: an abandoned record leaves one.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context as _, Result, bail};

use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "adrs",
    args: "[--against <ref>]",
    summary: "require every decision record to have its own number",
    run: |args| match args {
        [] => Some(run(None)),
        ["--against", against] => Some(run(Some(against))),
        _ => None,
    },
};

/// Where the decision records live, relative to the workspace root.
const ADRS: &str = "docs/adr";

/// What a record's file name looks like, for the message that says one doesn't.
const SHAPE: &str = "<four digits>-<slug>.md";

/// Checks the records of the working tree, or with `against`, of `HEAD` merged with
/// that ref.
pub(crate) fn run(against: Option<&str>) -> Result<()> {
    let names = match against {
        Some(against) => merged_names(Path::new("."), against)?,
        None => working_names(Path::new(ADRS))?,
    };
    check(&names)?;
    eprintln!("adrs: {} decision records checked", names.len());
    Ok(())
}

/// The names in the record directory of `HEAD` merged with `against`, sorted.
///
/// `git merge-tree --write-tree` builds the merged tree in the object store and
/// leaves the checkout alone. A conflict exits 1 but still writes the tree, and its
/// names are still checked. An unknown ref also exits 1, so it is resolved first.
fn merged_names(root: &Path, against: &str) -> Result<Vec<String>> {
    let root = root.to_str().context("the repository path is not UTF-8")?;
    let commit = format!("{against}^{{commit}}");
    process::output(
        "git",
        &["-C", root, "rev-parse", "--verify", "--quiet", &commit],
    )
    .with_context(|| format!("`{against}` does not resolve to a commit"))?;
    let merged = process::output_accepting(
        "git",
        &["-C", root, "merge-tree", "--write-tree", "HEAD", against],
        &[0, 1],
    )
    .with_context(|| format!("merging `{against}` into HEAD"))?;
    let tree = merged
        .lines()
        .next()
        .with_context(|| format!("merging `{against}` into HEAD printed no tree"))?;
    let listed = process::output(
        "git",
        &[
            "-C",
            root,
            "ls-tree",
            "--name-only",
            &format!("{tree}:{ADRS}"),
        ],
    )
    .with_context(|| format!("listing {ADRS} in HEAD merged with `{against}`"))?;
    let mut names: Vec<String> = listed.lines().map(str::to_owned).collect();
    names.sort();
    Ok(names)
}

/// The names in the record directory of the working tree, sorted.
fn working_names(directory: &Path) -> Result<Vec<String>> {
    let reading = || format!("reading {}", directory.display());
    let mut names = std::fs::read_dir(directory)
        .with_context(reading)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(reading)?;
    names.sort();
    Ok(names)
}

/// Fails naming every record that shares a number and every name that isn't a
/// record's, all in one message.
fn check(names: &[String]) -> Result<()> {
    let mut numbered: BTreeMap<u32, Vec<&str>> = BTreeMap::new();
    let mut misnamed = Vec::new();
    for name in names {
        match number(name) {
            Some(number) => numbered.entry(number).or_default().push(name),
            None => misnamed.push(format!("`{name}` is not named `{SHAPE}`")),
        }
    }
    let shared: Vec<String> = numbered
        .iter()
        .filter(|(_, records)| records.len() > 1)
        .map(|(number, records)| {
            format!("{number:04} is shared by:\n    {}", records.join("\n    "))
        })
        .collect();
    if shared.is_empty() && misnamed.is_empty() {
        return Ok(());
    }
    let hint = if shared.is_empty() {
        String::new()
    } else {
        // Past the highest number rather than into a gap: a gap's number may still be cited.
        let next = numbered.keys().next_back().map_or(1, |highest| highest + 1);
        format!(
            "\n\nrenumber one to {next:04}, keeping the one that landed first: its number may \
             already be cited"
        )
    };
    let problems = [shared, misnamed].concat();
    bail!(
        "{ADRS} has numbering problems:\n  - {}{hint}",
        problems.join("\n  - ")
    );
}

/// The number a record's file name carries, or `None` when the name isn't
/// `<four digits>-<slug>.md`.
fn number(name: &str) -> Option<u32> {
    let (digits, slug) = name.strip_suffix(".md")?.split_at_checked(4)?;
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    match slug.strip_prefix('-') {
        Some("") | None => None,
        Some(_) => digits.parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    /// The message a check that should fail fails with.
    fn failure(result: Result<()>) -> String {
        format!("{:#}", result.expect_err("the check should fail"))
    }

    fn check_named(names: &[&str]) -> Result<()> {
        check(&named(names))
    }

    /// Checks the records of `scratch`'s `HEAD` merged with its `main`.
    fn check_merged_with_main(scratch: &Scratch) -> Result<()> {
        check(&merged_names(scratch.root(), "main")?)
    }

    const MINE_AND_THEIRS_SHARE_0002: &str = "docs/adr has numbering problems:
  - 0002 is shared by:
    0002-mine.md
    0002-theirs.md

renumber one to 0003, keeping the one that landed first: its number may already be cited";

    /// A throwaway repository, so no test depends on this one's history.
    struct Scratch(tempfile::TempDir);

    impl Scratch {
        /// A repository whose `main` holds one record, `0001-first.md`.
        fn new() -> Self {
            let scratch = Self(tempfile::tempdir().expect("a scratch directory"));
            scratch.git(&["init", "-q", "-b", "main"]);
            scratch.add("0001-first.md");
            scratch
        }

        fn git(&self, args: &[&str]) {
            let output = std::process::Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
                .args(["-c", "commit.gpgsign=false"])
                .args(args)
                .current_dir(self.root())
                .output()
                .expect("git runs");
            assert!(output.status.success(), "git {args:?}: {output:?}");
        }

        /// Commits a record named `name`, holding its name as its text.
        fn add(&self, name: &str) {
            let directory = self.root().join(ADRS);
            std::fs::create_dir_all(&directory).expect("the record directory is created");
            std::fs::write(directory.join(name), name).expect("the record is written");
            self.git(&["add", "-A"]);
            self.git(&["commit", "-q", "-m", name]);
        }

        fn root(&self) -> &Path {
            self.0.path()
        }
    }

    /// The other branch's record landed on `main` after this branch was cut, with
    /// the same number. Only the merged tree holds both.
    #[test]
    fn a_clash_only_in_the_merged_tree_is_caught_against_the_ref() {
        let scratch = Scratch::new();
        scratch.git(&["checkout", "-q", "-b", "mine"]);
        scratch.add("0002-mine.md");
        scratch.git(&["checkout", "-q", "main"]);
        scratch.add("0002-theirs.md");
        scratch.git(&["checkout", "-q", "mine"]);

        let message = failure(check_merged_with_main(&scratch));

        assert_eq!(message, MINE_AND_THEIRS_SHARE_0002);
    }

    /// The repair the message asks for. A check that merged this branch's names
    /// with the ref's rather than reading the merged tree would still see the old name.
    #[test]
    fn renumbering_clears_the_clash_in_the_merged_tree() {
        let scratch = Scratch::new();
        scratch.add("0002-mine.md");
        scratch.add("0002-theirs.md");
        scratch.git(&["checkout", "-q", "-b", "renumber"]);
        scratch.git(&["mv", "docs/adr/0002-theirs.md", "docs/adr/0003-theirs.md"]);
        scratch.git(&["commit", "-q", "-m", "renumber"]);

        let names = merged_names(scratch.root(), "main").expect("the merged tree is read");

        assert_eq!(
            names,
            named(&["0001-first.md", "0002-mine.md", "0003-theirs.md"])
        );
        check(&names).expect("the renumbered record no longer clashes in the merged tree");
    }

    /// A merge with conflicts still writes a tree, and its names are still checked.
    #[test]
    fn a_merge_with_conflicts_is_still_checked() {
        let scratch = Scratch::new();
        let first = scratch.root().join(ADRS).join("0001-first.md");
        scratch.git(&["checkout", "-q", "-b", "mine"]);
        std::fs::write(&first, "mine").expect("the record is written");
        scratch.add("0002-mine.md");
        scratch.git(&["checkout", "-q", "main"]);
        std::fs::write(&first, "theirs").expect("the record is written");
        scratch.add("0002-theirs.md");
        scratch.git(&["checkout", "-q", "mine"]);

        let message = failure(check_merged_with_main(&scratch));

        assert_eq!(message, MINE_AND_THEIRS_SHARE_0002);
    }

    #[test]
    fn a_ref_that_does_not_resolve_fails_naming_it() {
        let scratch = Scratch::new();

        let message = format!(
            "{:#}",
            merged_names(scratch.root(), "origin/no-such-branch")
                .expect_err("there is no such ref")
        );

        // The rest of the message is git's report, which names the scratch path.
        assert!(
            message.contains("`origin/no-such-branch` does not resolve to a commit"),
            "the ref should be named:\n{message}"
        );
    }

    /// The check passes on this repository's own records, or nothing lands.
    #[test]
    fn the_records_in_this_repository_pass() {
        let records = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask sits one level below the workspace root")
            .join(ADRS);

        check(&working_names(&records).expect("the records are listed"))
            .expect("this repository's records are well numbered");
    }

    #[test]
    fn distinct_numbers_pass() {
        check_named(&["0001-first.md", "0002-second.md"]).expect("every number is distinct");
    }

    #[test]
    fn a_gap_in_the_sequence_passes() {
        check_named(&["0001-first.md", "0004-second.md", "0009-third.md"])
            .expect("a gap is an abandoned record, not a fault");
    }

    #[test]
    fn two_records_sharing_a_number_are_both_named() {
        let message = failure(check_named(&["0007-first.md", "0007-second.md"]));

        assert_eq!(
            message,
            "docs/adr has numbering problems:
  - 0007 is shared by:
    0007-first.md
    0007-second.md

renumber one to 0008, keeping the one that landed first: its number may already be cited"
        );
    }

    #[test]
    fn three_records_sharing_a_number_are_all_named() {
        let message = failure(check_named(&["0007-a.md", "0007-b.md", "0007-c.md"]));

        assert_eq!(
            message,
            "docs/adr has numbering problems:
  - 0007 is shared by:
    0007-a.md
    0007-b.md
    0007-c.md

renumber one to 0008, keeping the one that landed first: its number may already be cited"
        );
    }

    /// Past the highest rather than into a gap, so renumbering never reissues a
    /// number something already cites.
    #[test]
    fn the_hint_renumbers_to_one_past_the_highest() {
        let message = failure(check_named(&["0002-a.md", "0007-b.md", "0007-c.md"]));

        assert_eq!(
            message,
            "docs/adr has numbering problems:
  - 0007 is shared by:
    0007-b.md
    0007-c.md

renumber one to 0008, keeping the one that landed first: its number may already be cited"
        );
    }

    /// A name the check can't read is a file it isn't looking at, so it is named
    /// rather than skipped.
    #[test]
    fn a_name_that_is_not_a_records_is_named() {
        let message = failure(check_named(&["0001-first.md", "notes.md"]));

        assert_eq!(
            message,
            "docs/adr has numbering problems:
  - `notes.md` is not named `<four digits>-<slug>.md`"
        );
    }

    #[test]
    fn both_kinds_of_problem_are_reported_together() {
        let message = failure(check_named(&["0007-a.md", "0007-b.md", "README.md"]));

        assert_eq!(
            message,
            "docs/adr has numbering problems:
  - 0007 is shared by:
    0007-a.md
    0007-b.md
  - `README.md` is not named `<four digits>-<slug>.md`

renumber one to 0008, keeping the one that landed first: its number may already be cited"
        );
    }

    #[test]
    fn a_name_needs_four_digits_a_hyphen_a_slug_and_md() {
        assert_eq!(number("0007-the-slug.md"), Some(7));
        for refused in [
            "007-three-digits.md",
            "00007-five-digits.md",
            "0007the-slug.md",
            "0007-.md",
            "0007.md",
            "0007-the-slug.markdown",
            "README.md",
            "",
        ] {
            assert_eq!(
                number(refused),
                None,
                "`{refused}` should not read as a record"
            );
        }
    }
}
