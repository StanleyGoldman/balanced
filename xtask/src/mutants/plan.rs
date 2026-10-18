//! The mutant-run plan: what a CI mutant run does to the `mutant` issues, decided from
//! the run's outcomes, every mutant at the tested commit and the open issues. The
//! GitHub client only carries the actions out. See ADR 0002.

use std::collections::{HashMap, HashSet};
use std::fmt;

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;

use super::REPO;
use super::key::MutantKey;

/// An open issue labelled `mutant`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenIssue {
    pub(crate) number: u64,
    pub(crate) body: String,
}

/// The run being reconciled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Run {
    /// The tested commit's full SHA.
    pub(crate) commit: String,
    pub(crate) found_by: FoundBy,
}

/// Which run tested the mutants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FoundBy {
    /// The post-merge run for the pull request with this number.
    PullRequest(u64),
    /// The nightly run of one shard.
    Shard(Shard),
}

/// One round-robin shard of every mutant, as cargo-mutants' `--shard` counts them
/// (from 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shard {
    pub(crate) index: u32,
    pub(crate) count: u32,
}

/// One thing the run does to GitHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    Open(NewIssue),
    Comment {
        issue: u64,
        body: String,
    },
    Close {
        issue: u64,
        reason: CloseReason,
        comment: String,
    },
}

/// Why a run closes a mutant's issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseReason {
    /// The run tested the mutant and a test failed.
    Caught,
    /// The mutant is gone from the full list: its code was deleted, renamed or excluded.
    NoLongerGenerated,
}

/// The issue opened for a surviving mutant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewIssue {
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) labels: Vec<&'static str>,
}

/// Decides the run's actions. `outcomes` is cargo-mutants' `outcomes.json`, or `None`
/// for a run whose scope held no mutants, which tested nothing; `list` is
/// `cargo mutants --list --json` at the tested commit. Opens, comments and closes
/// for tested mutants come in the order the run tested them, then the closes for
/// mutants no longer generated, in the order of `issues`.
pub(crate) fn plan(
    outcomes: Option<&str>,
    list: &str,
    issues: &[OpenIssue],
    run: &Run,
) -> Result<Vec<Action>> {
    let listed: Vec<ListedMutant> =
        serde_json::from_str(list).context("reading the mutant list")?;
    let listed = KeyedMutant::key_all(listed)?;
    let by_name: HashMap<&str, &KeyedMutant> = listed
        .iter()
        .map(|keyed| (keyed.mutant.name.as_str(), keyed))
        .collect();
    let outcomes: Outcomes = match outcomes {
        Some(outcomes) => serde_json::from_str(outcomes).context("reading outcomes.json")?,
        None => Outcomes::default(),
    };
    let issues = KeyedIssue::key_all(issues)?;
    // Where two open issues share a key, the oldest is the mutant's issue.
    let mut issue_by_key: HashMap<&MutantKey, u64> = HashMap::new();
    for issue in &issues {
        let number = issue_by_key.entry(&issue.key).or_insert(issue.number);
        *number = (*number).min(issue.number);
    }
    let mut actions = Vec::new();
    for outcome in outcomes.outcomes {
        let Scenario::Mutant(tested) = outcome.scenario else {
            continue;
        };
        let Some(keyed) = by_name.get(tested.name.as_str()) else {
            bail!("tested mutant missing from the full list: {}", tested.name);
        };
        let issue = issue_by_key.get(&keyed.key).copied();
        let survival = match outcome.summary {
            Summary::MissedMutant => Survival::Missed,
            Summary::Timeout => Survival::TimedOut,
            Summary::CaughtMutant => {
                if let Some(issue) = issue {
                    actions.push(Action::Close {
                        issue,
                        reason: CloseReason::Caught,
                        comment: format!("Caught by {run}, so closing."),
                    });
                }
                continue;
            }
            Summary::Unviable => continue,
            Summary::Success | Summary::Failure => {
                bail!(
                    "mutant {} has the baseline's summary {:?}",
                    tested.name,
                    outcome.summary
                )
            }
        };
        actions.push(match issue {
            None => Action::Open(keyed.new_issue(survival, run)),
            Some(issue) => Action::Comment {
                issue,
                body: format!("Found again by {run}."),
            },
        });
    }
    let generated: HashSet<&MutantKey> = listed.iter().map(|keyed| &keyed.key).collect();
    for issue in issues
        .iter()
        .filter(|issue| !generated.contains(&issue.key))
    {
        actions.push(Action::Close {
            issue: issue.number,
            reason: CloseReason::NoLongerGenerated,
            comment: format!(
                "No longer generated, as seen by {run}: its code was deleted, renamed or \
                 excluded, so closing."
            ),
        });
    }
    Ok(actions)
}

impl fmt::Display for Run {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.found_by {
            FoundBy::PullRequest(number) => {
                write!(f, "the post-merge run of pull request #{number}")?;
            }
            FoundBy::Shard(Shard { index, count }) => {
                write!(f, "the nightly run of shard {index}/{count}")?;
            }
        }
        write!(f, " at commit {}", self.commit)
    }
}

/// An open issue with the key its body names.
#[derive(Debug)]
struct KeyedIssue {
    number: u64,
    key: MutantKey,
}

impl KeyedIssue {
    /// The issues whose bodies hold a key marker; a corrupt marker is an error.
    fn key_all(issues: &[OpenIssue]) -> Result<Vec<Self>> {
        let mut keyed = Vec::new();
        for issue in issues {
            let key = MutantKey::parse(&issue.body)
                .with_context(|| format!("reading the mutant key of issue #{}", issue.number))?;
            if let Some(key) = key {
                keyed.push(Self {
                    number: issue.number,
                    key,
                });
            }
        }
        Ok(keyed)
    }
}

/// A listed mutant with its key.
#[derive(Debug)]
struct KeyedMutant {
    mutant: ListedMutant,
    key: MutantKey,
    /// Whether another mutant in its function makes the same mutation.
    has_siblings: bool,
}

impl KeyedMutant {
    /// Keys every mutant. Identical mutations in one function are numbered in source
    /// order.
    fn key_all(mut listed: Vec<ListedMutant>) -> Result<Vec<Self>> {
        listed.sort_by(|a, b| (&a.file, a.span.start).cmp(&(&b.file, b.span.start)));
        // Counted under the key with occurrence 0, which names the whole sibling group.
        let mut group_sizes: HashMap<MutantKey, usize> = HashMap::new();
        let mut keyed = Vec::with_capacity(listed.len());
        for mutant in listed {
            let first = MutantKey {
                file: mutant.file.clone(),
                function: mutant.function.function_name.clone(),
                mutation: mutant.mutation()?.to_owned(),
                occurrence: 0,
            };
            let size = group_sizes.entry(first.clone()).or_default();
            let key = MutantKey {
                occurrence: *size,
                ..first
            };
            *size += 1;
            keyed.push(Self {
                mutant,
                key,
                has_siblings: false,
            });
        }
        for keyed in &mut keyed {
            let first = MutantKey {
                occurrence: 0,
                ..keyed.key.clone()
            };
            keyed.has_siblings = group_sizes.get(&first).is_some_and(|&size| size > 1);
        }
        Ok(keyed)
    }

    /// The issue for this mutant, which survived `run`.
    fn new_issue(&self, survival: Survival, run: &Run) -> NewIssue {
        let ListedMutant {
            file,
            function,
            span,
            replacement,
            diff,
            ..
        } = &self.mutant;
        let function = &function.function_name;
        let line = span.start.line;
        let commit = &run.commit;
        let reproduce = self.reproduce_command();
        let SurvivalText {
            kind,
            outcome,
            labels,
        } = survival.text();
        let body = format!(
            "{marker}\n\n\
             cargo-mutants made this change in `{function}` and {outcome}: \
             [{file}:{line}](https://github.com/{REPO}/blob/{commit}/{file}#L{line})\n\n\
             ```diff\n{diff}```\n\n\
             Found by {run}.\n\n\
             Reproduce it:\n\n\
             ```sh\n{reproduce}\n```\n\n\
             Kill it with a test, or exclude it with a reason.\n",
            marker = self.key.render(),
        );
        NewIssue {
            title: format!("{kind} mutant: {replacement} in {function}"),
            body,
            labels: labels.to_vec(),
        }
    }

    /// The by-hand command that runs this mutant alone. Its name is matched without
    /// the line and column, so the command keeps working after edits elsewhere in the
    /// file. A mutant with identical siblings is told apart only by where it was
    /// tested, so its pattern holds that line and column.
    fn reproduce_command(&self) -> String {
        let location = if self.has_siblings {
            let start = self.mutant.span.start;
            format!("{}:{}", start.line, start.column)
        } else {
            r"\d+:\d+".to_owned()
        };
        let pattern = format!(
            "^{}:{location}: {}$",
            regex_escape(&self.key.file),
            regex_escape(&self.key.mutation)
        );
        format!("cargo xtask mutants --re {}", shell_quote(&pattern))
    }
}

/// How a mutant survived the tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Survival {
    /// Every test passed.
    Missed,
    /// The tests ran past cargo-mutants' timeout.
    TimedOut,
}

/// The words and labels an issue uses for a [`Survival`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SurvivalText {
    /// The title's first word.
    kind: &'static str,
    /// What happened to the tests, as the body says it.
    outcome: &'static str,
    labels: &'static [&'static str],
}

impl Survival {
    fn text(self) -> SurvivalText {
        match self {
            Self::Missed => SurvivalText {
                kind: "Missed",
                outcome: "no test failed",
                labels: &["mutant", "ready-for-agent"],
            },
            Self::TimedOut => SurvivalText {
                kind: "Timed-out",
                outcome: "the tests ran past the timeout",
                labels: &["mutant", "ready-for-agent", "mutant-timeout"],
            },
        }
    }
}

/// `text` with every regex metacharacter escaped.
fn regex_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// `text` single-quoted for a POSIX shell.
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// An entry of `cargo mutants --list --json`.
#[derive(Debug, Deserialize)]
struct ListedMutant {
    name: String,
    file: String,
    function: Function,
    span: Span,
    replacement: String,
    diff: String,
}

impl ListedMutant {
    /// The mutation text: the name without its `file:line:column: ` prefix.
    fn mutation(&self) -> Result<&str> {
        let location = format!(
            "{}:{}:{}: ",
            self.file, self.span.start.line, self.span.start.column
        );
        self.name
            .strip_prefix(&location)
            .with_context(|| format!("mutant name doesn't start with {location:?}: {}", self.name))
    }
}

#[derive(Debug, Deserialize)]
struct Function {
    function_name: String,
}

#[derive(Debug, Deserialize)]
struct Span {
    start: Position,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
struct Position {
    line: u32,
    column: u32,
}

#[derive(Debug, Default, Deserialize)]
struct Outcomes {
    outcomes: Vec<Outcome>,
}

#[derive(Debug, Deserialize)]
struct Outcome {
    scenario: Scenario,
    summary: Summary,
}

#[derive(Debug, Deserialize)]
enum Scenario {
    Baseline,
    Mutant(TestedMutant),
}

#[derive(Debug, Deserialize)]
struct TestedMutant {
    name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum Summary {
    Success,
    CaughtMutant,
    MissedMutant,
    Timeout,
    Unviable,
    Failure,
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::super::key::MutantKey;
    use super::{Action, CloseReason, FoundBy, NewIssue, OpenIssue, Run, Shard, plan};

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    fn shard_run() -> Run {
        Run {
            commit: COMMIT.to_owned(),
            found_by: FoundBy::Shard(Shard { index: 3, count: 7 }),
        }
    }

    /// A mutant shaped like an entry of `cargo mutants --list --json`.
    fn mutant(file: &str, function: &str, line: u32, column: u32, mutation: &str) -> Value {
        let replacement = mutation.split_once(" with ").map_or(mutation, |(_, rest)| {
            rest.split(" in ").next().unwrap_or(rest)
        });
        json!({
            "diff": format!("--- {file}\n+++ {mutation}\n@@ -{line},1 +{line},1 @@\n-old\n+new\n"),
            "file": file,
            "function": {
                "function_name": function,
                "return_type": "-> Self",
                "span": {"start": {"line": line - 1, "column": 5}, "end": {"line": line + 1, "column": 6}}
            },
            "genre": "BinaryOperator",
            "name": format!("{file}:{line}:{column}: {mutation}"),
            "package": "balanced_core",
            "replacement": replacement,
            "span": {"start": {"line": line, "column": column}, "end": {"line": line, "column": column + 1}}
        })
    }

    /// `outcomes.json` for a run that tested each mutant with the given summary.
    fn outcomes(tested: &[(&Value, &str)]) -> String {
        let mut outcomes = vec![json!({"scenario": "Baseline", "summary": "Success"})];
        for (mutant, summary) in tested {
            let mut scenario = (*mutant).clone();
            if let Some(fields) = scenario.as_object_mut() {
                fields.remove("diff");
            }
            outcomes.push(json!({
                "scenario": {"Mutant": scenario},
                "summary": summary,
                "log_path": "log/x.log",
                "diff_path": "diff/x.diff",
                "phase_results": []
            }));
        }
        json!({"outcomes": outcomes, "cargo_mutants_version": "27.1.0"}).to_string()
    }

    fn list(mutants: &[&Value]) -> String {
        Value::Array(mutants.iter().map(|m| (*m).clone()).collect()).to_string()
    }

    fn next_minus() -> Value {
        mutant(
            "crates/balanced_core/src/account.rs",
            "AccountId::next",
            84,
            21,
            "replace + with - in AccountId::next",
        )
    }

    /// The open issue an earlier run filed for `mutant`.
    fn issue_for(number: u64, mutant: &Value, occurrence: usize) -> OpenIssue {
        let file = mutant["file"].as_str().expect("fixture has a file");
        let name = mutant["name"].as_str().expect("fixture has a name");
        let mutation = name
            .splitn(4, ':')
            .nth(3)
            .expect("fixture name has a location")
            .trim_start();
        let key = MutantKey {
            file: file.to_owned(),
            function: mutant["function"]["function_name"]
                .as_str()
                .expect("fixture has a function")
                .to_owned(),
            mutation: mutation.to_owned(),
            occurrence,
        };
        OpenIssue {
            number,
            body: format!("{}\n\nMissed mutant.\n", key.render()),
        }
    }

    #[test]
    fn a_missed_mutant_with_an_issue_is_commented_on() {
        let next = next_minus();

        let actions = plan(
            Some(&outcomes(&[(&next, "MissedMutant")])),
            &list(&[&next]),
            &[issue_for(12, &next, 0)],
            &shard_run(),
        )
        .unwrap();

        assert_eq!(
            actions,
            [Action::Comment {
                issue: 12,
                body: format!("Found again by the nightly run of shard 3/7 at commit {COMMIT}."),
            }]
        );
    }

    #[test]
    fn a_caught_mutant_with_an_issue_is_closed_as_caught() {
        let next = next_minus();
        let run = Run {
            commit: COMMIT.to_owned(),
            found_by: FoundBy::PullRequest(42),
        };

        let actions = plan(
            Some(&outcomes(&[(&next, "CaughtMutant")])),
            &list(&[&next]),
            &[issue_for(12, &next, 0)],
            &run,
        )
        .unwrap();

        assert_eq!(
            actions,
            [Action::Close {
                issue: 12,
                reason: CloseReason::Caught,
                comment: format!(
                    "Caught by the post-merge run of pull request #42 at commit {COMMIT}, so closing."
                ),
            }]
        );
    }

    #[test]
    fn a_timed_out_mutant_is_opened_with_the_timeout_label() {
        let next = next_minus();

        let actions = plan(
            Some(&outcomes(&[(&next, "Timeout")])),
            &list(&[&next]),
            &[],
            &shard_run(),
        )
        .unwrap();

        let [Action::Open(issue)] = actions.as_slice() else {
            panic!("expected one Open, got {actions:?}");
        };
        assert_eq!(issue.title, "Timed-out mutant: - in AccountId::next");
        assert_eq!(
            issue.labels,
            ["mutant", "ready-for-agent", "mutant-timeout"]
        );
    }

    #[test]
    fn a_timed_out_mutant_with_an_issue_is_commented_on() {
        let next = next_minus();

        let actions = plan(
            Some(&outcomes(&[(&next, "Timeout")])),
            &list(&[&next]),
            &[issue_for(12, &next, 0)],
            &shard_run(),
        )
        .unwrap();

        assert_eq!(
            actions,
            [Action::Comment {
                issue: 12,
                body: format!("Found again by the nightly run of shard 3/7 at commit {COMMIT}."),
            }]
        );
    }

    #[test]
    fn an_issue_whose_mutant_is_no_longer_generated_is_closed() {
        let next = next_minus();
        let gone = mutant(
            "crates/balanced_core/src/ledger.rs",
            "Ledger::removed",
            10,
            5,
            "replace Ledger::removed -> bool with true",
        );

        let actions = plan(
            Some(&outcomes(&[(&next, "CaughtMutant")])),
            &list(&[&next]),
            &[issue_for(31, &gone, 0)],
            &shard_run(),
        )
        .unwrap();

        assert_eq!(
            actions,
            [Action::Close {
                issue: 31,
                reason: CloseReason::NoLongerGenerated,
                comment: format!(
                    "No longer generated, as seen by the nightly run of shard 3/7 at commit \
                     {COMMIT}: its code was deleted, renamed or excluded, so closing."
                ),
            }]
        );
    }

    #[test]
    fn an_issue_for_a_mutant_outside_the_run_is_left_alone() {
        let next = next_minus();
        let untested = mutant(
            "crates/balanced_core/src/ledger.rs",
            "Ledger::is_empty",
            10,
            5,
            "replace Ledger::is_empty -> bool with true",
        );

        let actions = plan(
            Some(&outcomes(&[(&next, "CaughtMutant")])),
            &list(&[&next, &untested]),
            &[issue_for(31, &untested, 0)],
            &shard_run(),
        )
        .unwrap();

        assert_eq!(actions, []);
    }

    #[test]
    fn unviable_mutants_are_ignored() {
        let next = next_minus();
        let times = mutant(
            "crates/balanced_core/src/account.rs",
            "AccountId::next",
            84,
            21,
            "replace + with * in AccountId::next",
        );

        let actions = plan(
            Some(&outcomes(&[(&next, "Unviable"), (&times, "Unviable")])),
            &list(&[&next, &times]),
            &[issue_for(12, &next, 0)],
            &shard_run(),
        )
        .unwrap();

        assert_eq!(actions, []);
    }

    #[test]
    fn a_key_is_unchanged_when_lines_above_the_mutant_shift() {
        let before = next_minus();
        let shifted = mutant(
            "crates/balanced_core/src/account.rs",
            "AccountId::next",
            97,
            21,
            "replace + with - in AccountId::next",
        );

        let actions = plan(
            Some(&outcomes(&[(&shifted, "MissedMutant")])),
            &list(&[&shifted]),
            &[issue_for(12, &before, 0)],
            &shard_run(),
        )
        .unwrap();

        assert!(
            matches!(actions.as_slice(), [Action::Comment { issue: 12, .. }]),
            "expected a comment on #12, got {actions:?}"
        );
    }

    #[test]
    fn identical_mutations_in_one_function_get_distinct_keys() {
        let first = mutant(
            "crates/balanced_core/src/ledger.rs",
            "Ledger::create_category",
            827,
            45,
            "replace == with != in Ledger::create_category",
        );
        let second = mutant(
            "crates/balanced_core/src/ledger.rs",
            "Ledger::create_category",
            827,
            88,
            "replace == with != in Ledger::create_category",
        );

        // Listed out of source order, so the occurrence can't come from list order.
        let actions = plan(
            Some(&outcomes(&[
                (&first, "MissedMutant"),
                (&second, "MissedMutant"),
            ])),
            &list(&[&second, &first]),
            &[issue_for(12, &second, 1)],
            &shard_run(),
        )
        .unwrap();

        let [Action::Open(opened), Action::Comment { issue: 12, .. }] = actions.as_slice() else {
            panic!("expected an Open for the first and a comment on #12, got {actions:?}");
        };
        assert_eq!(
            MutantKey::parse(&opened.body)
                .unwrap()
                .map(|key| key.occurrence),
            Some(0)
        );
    }

    #[test]
    fn a_mutant_with_identical_siblings_reproduces_by_its_tested_location() {
        let first = mutant(
            "crates/balanced_core/src/ledger.rs",
            "Ledger::create_category",
            827,
            45,
            "replace == with != in Ledger::create_category",
        );
        let second = mutant(
            "crates/balanced_core/src/ledger.rs",
            "Ledger::create_category",
            827,
            88,
            "replace == with != in Ledger::create_category",
        );

        let actions = plan(
            Some(&outcomes(&[(&second, "MissedMutant")])),
            &list(&[&first, &second]),
            &[],
            &shard_run(),
        )
        .unwrap();

        let [Action::Open(opened)] = actions.as_slice() else {
            panic!("expected one Open, got {actions:?}");
        };
        let reproduce = opened
            .body
            .lines()
            .find(|line| line.starts_with("cargo xtask"));
        assert_eq!(
            reproduce,
            Some(
                r"cargo xtask mutants --re '^crates/balanced_core/src/ledger\.rs:827:88: replace == with != in Ledger::create_category$'"
            )
        );
    }

    #[test]
    fn a_missed_mutant_with_no_issue_is_opened() {
        let next = next_minus();

        let actions = plan(
            Some(&outcomes(&[(&next, "MissedMutant")])),
            &list(&[&next]),
            &[],
            &shard_run(),
        )
        .unwrap();

        assert_eq!(
            actions,
            [Action::Open(NewIssue {
                title: "Missed mutant: - in AccountId::next".to_owned(),
                body: format!(
                    r#"<!-- mutant-key/v1: {{"file":"crates/balanced_core/src/account.rs","function":"AccountId::next","mutation":"replace + with - in AccountId::next","occurrence":0}} -->

cargo-mutants made this change in `AccountId::next` and no test failed: [crates/balanced_core/src/account.rs:84](https://github.com/StanleyGoldman/balanced/blob/{COMMIT}/crates/balanced_core/src/account.rs#L84)

```diff
--- crates/balanced_core/src/account.rs
+++ replace + with - in AccountId::next
@@ -84,1 +84,1 @@
-old
+new
```

Found by the nightly run of shard 3/7 at commit {COMMIT}.

Reproduce it:

```sh
cargo xtask mutants --re '^crates/balanced_core/src/account\.rs:\d+:\d+: replace \+ with \- in AccountId::next$'
```

Kill it with a test, or exclude it with a reason.
"#
                ),
                labels: vec!["mutant", "ready-for-agent"],
            })]
        );
    }

    #[test]
    fn a_run_that_tested_nothing_closes_only_issues_no_longer_generated() {
        let listed = next_minus();
        let gone = mutant(
            "crates/balanced_core/src/ledger.rs",
            "Ledger::removed",
            10,
            5,
            "replace Ledger::removed -> bool with true",
        );

        let actions = plan(
            None,
            &list(&[&listed]),
            &[issue_for(30, &listed, 0), issue_for(31, &gone, 0)],
            &shard_run(),
        );

        assert_eq!(
            actions.ok(),
            Some(vec![Action::Close {
                issue: 31,
                reason: CloseReason::NoLongerGenerated,
                comment: format!(
                    "No longer generated, as seen by the nightly run of shard 3/7 at commit \
                     {COMMIT}: its code was deleted, renamed or excluded, so closing."
                ),
            }])
        );
    }
}
