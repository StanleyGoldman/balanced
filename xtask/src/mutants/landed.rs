//! `cargo xtask mutants-landed`: the post-merge CI run that mutation-tests the diff a
//! pull request brought into `main` and reconciles the `mutant` issues (ADR 0002).

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;

use super::github::GitHub;
use super::plan::FoundBy;
use super::reconcile::{self, IssueUpdates};
use super::write_diff;
use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "mutants-landed",
    args: "[--dry-run]",
    summary: "a CI task: mutation-test the diff the pull request that landed HEAD brought \
              into main and update the mutant issues, or with --dry-run print the updates; \
              by hand, use mutants",
    run: |args| match *args {
        [] => Some(run(IssueUpdates::Apply)),
        ["--dry-run"] => Some(run(IssueUpdates::Print)),
        _ => None,
    },
};

/// The branch whose landed pull requests this run tests.
const BRANCH: &str = "main";

/// Where the run writes the landed diff that `--in-diff` reads.
const DIFF_PATH: &str = "target/mutants-landed.diff";

fn run(updates: IssueUpdates) -> Result<()> {
    let github = GitHub::from_env()?;
    let commit = Commit::head()?;
    let pulls = github
        .pulls_holding(&commit.sha)
        .context("looking up the pull request that landed HEAD")?;
    let Some(pull) = LandedPull::find(&commit.sha, &pulls)? else {
        eprintln!(
            "mutants: no pull request landed {}, so testing nothing",
            commit.sha
        );
        return Ok(());
    };
    let range = Range::landed_by(&commit, || github.commit_count(pull.number))?;
    // Resolving `before` also fails, rather than diffing the wrong range, when the
    // clone lacks the history.
    let before = process::output(
        "git",
        &[
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", range.before),
        ],
    )
    .with_context(|| format!("resolving {}", range.before))?;
    let before = before.trim();
    eprintln!(
        "mutants: pull request #{} ({}) landed {before}..{}",
        pull.number, pull.branch, range.after
    );
    write_diff(&format!("{before}..{}", range.after), DIFF_PATH)?;
    reconcile::run(
        &["--in-diff".to_owned(), DIFF_PATH.to_owned()],
        FoundBy::PullRequest(pull.number),
        updates,
    )
}

/// The pull request that landed a commit on `main`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LandedPull {
    number: u64,
    /// The pull request's head branch.
    branch: String,
}

impl LandedPull {
    /// The merged pull request into `main` whose merge commit is `commit`, read from
    /// GitHub's `commits/{commit}/pulls` response. That lists every pull request
    /// containing the commit, so a stack's upper layers show up too; they are left
    /// out. `None` means no pull request landed it.
    fn find(commit: &str, pulls: &str) -> Result<Option<Self>> {
        #[derive(Deserialize)]
        struct Listed {
            number: u64,
            merged_at: Option<String>,
            merge_commit_sha: Option<String>,
            base: Ref,
            head: Ref,
        }
        #[derive(Deserialize)]
        struct Ref {
            #[serde(rename = "ref")]
            name: String,
        }
        let listed: Vec<Listed> =
            serde_json::from_str(pulls).context("reading the pull requests holding the commit")?;
        Ok(listed
            .into_iter()
            .find(|listed| {
                listed.merged_at.is_some()
                    && listed.base.name == BRANCH
                    && listed.merge_commit_sha.as_deref() == Some(commit)
            })
            .map(|listed| Self {
                number: listed.number,
                branch: listed.head.name,
            }))
    }
}

/// The commit a run tests, with its parents in order.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Commit {
    sha: String,
    parents: Vec<String>,
}

impl Commit {
    /// `HEAD`, the commit the run tests.
    fn head() -> Result<Self> {
        let line = process::output("git", &["rev-list", "--parents", "--max-count=1", "HEAD"])?;
        let mut shas = line.split_whitespace().map(str::to_owned);
        let sha = shas.next().context("git rev-list printed no commit")?;
        Ok(Self {
            sha,
            parents: shas.collect(),
        })
    }
}

/// The commits a pull request brought into `main`, as git's `before..after`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Range {
    before: String,
    after: String,
}

impl Range {
    /// What the pull request whose merge commit is `commit` landed. A merge commit
    /// landed everything since its first parent. A rebase merge left one commit on
    /// `main` per commit in the pull request, so the range starts that many back;
    /// `commit_count` asks for that count, and only a rebase merge calls it.
    fn landed_by(commit: &Commit, commit_count: impl FnOnce() -> Result<u32>) -> Result<Self> {
        let before = match commit.parents.as_slice() {
            [first, _] => first.clone(),
            [_] => format!("{}~{}", commit.sha, commit_count()?),
            parents => bail!("{} has {} parents", commit.sha, parents.len()),
        };
        Ok(Self {
            before,
            after: commit.sha.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use anyhow::bail;
    use serde_json::{Value, json};

    use super::{Commit, LandedPull, Range};

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    /// A pull request shaped like an entry of `commits/{sha}/pulls`.
    fn pull(number: u64, base: &str, merge_commit_sha: &str, merged: bool) -> Value {
        json!({
            "number": number,
            "merged_at": merged.then_some("2026-10-18T15:44:15Z"),
            "merge_commit_sha": merge_commit_sha,
            "base": {"ref": base},
            "head": {"ref": format!("feat/{number}")},
        })
    }

    #[test]
    fn the_landed_pull_request_is_found_among_the_stack_that_contains_its_commit() {
        let pulls = json!([
            // The layer above, still open and based on the landed one's branch.
            pull(
                7,
                "feat/5",
                "fedcba9876543210fedcba9876543210fedcba98",
                false
            ),
            pull(5, "main", COMMIT, true),
        ]);

        let landed = LandedPull::find(COMMIT, &pulls.to_string()).unwrap();

        assert_eq!(
            landed,
            Some(LandedPull {
                number: 5,
                branch: "feat/5".to_owned()
            })
        );
    }

    #[test]
    fn a_push_no_pull_request_landed_finds_none() {
        // A commit pushed straight to `main` that an open pull request also holds,
        // and an earlier pull request that merged into `main` with another commit.
        let pulls = json!([
            pull(8, "main", "1111111111111111111111111111111111111111", false),
            pull(4, "main", "2222222222222222222222222222222222222222", true),
        ]);

        assert_eq!(LandedPull::find(COMMIT, &pulls.to_string()).unwrap(), None);
        assert_eq!(LandedPull::find(COMMIT, "[]").unwrap(), None);
    }

    #[test]
    fn a_lookup_that_returns_no_list_of_pull_requests_is_an_error() {
        let error = json!({
            "message": "Bad credentials",
            "documentation_url": "https://docs.github.com/rest",
            "status": "401"
        });

        assert!(LandedPull::find(COMMIT, &error.to_string()).is_err());
    }

    #[test]
    fn a_merge_commit_landed_everything_since_its_first_parent() {
        let commit = Commit {
            sha: COMMIT.to_owned(),
            parents: vec!["aaaa".to_owned(), "bbbb".to_owned()],
        };

        let range = Range::landed_by(&commit, || bail!("a merge commit needs no count"));

        assert_eq!(
            range.ok(),
            Some(Range {
                before: "aaaa".to_owned(),
                after: COMMIT.to_owned()
            })
        );
    }

    #[test]
    fn a_rebase_merge_landed_as_many_commits_as_the_pull_request_holds() {
        let commit = Commit {
            sha: COMMIT.to_owned(),
            parents: vec!["aaaa".to_owned()],
        };

        let range = Range::landed_by(&commit, || Ok(3));

        assert_eq!(
            range.ok(),
            Some(Range {
                before: format!("{COMMIT}~3"),
                after: COMMIT.to_owned()
            })
        );
    }
}
