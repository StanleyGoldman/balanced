// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! `cargo xtask mutants-shard <shard>`: the nightly CI run that mutation-tests one
//! shard of every mutant on `main` and reconciles the `mutant` issues (ADR 0002).
//! The workflow names the shard: each weekday's schedule runs its own.

use anyhow::{Context as _, Result, ensure};

use super::plan::{FoundBy, Shard};
use super::reconcile::{self, IssueUpdates};
use crate::Task;

pub(crate) const TASK: Task = Task {
    name: "mutants-shard",
    args: "<shard> [--dry-run]",
    summary: "a CI task: mutation-test one shard (0 to 6) of every mutant and update the \
              mutant issues, or with --dry-run print the updates; by hand, use mutants",
    run: |args| match *args {
        [shard] => Some(run(shard, IssueUpdates::Apply)),
        [shard, "--dry-run"] => Some(run(shard, IssueUpdates::Print)),
        _ => None,
    },
};

/// How many shards the mutants are split into: one per weekday, so all of `main`
/// is tested once a week. `.github/workflows/mutants.yml` has a schedule for each.
/// A new count reshuffles every shard and needs a schedule per shard.
const SHARD_COUNT: u32 = 7;

impl Shard {
    /// The shard `text` numbers, from 0 to one less than [`SHARD_COUNT`].
    fn numbered(text: &str) -> Result<Self> {
        let index: u32 = text
            .parse()
            .with_context(|| format!("the shard {text:?} is not a number"))?;
        ensure!(
            index < SHARD_COUNT,
            "the shard {index} is past the last, {}",
            SHARD_COUNT - 1
        );
        Ok(Self {
            index,
            count: SHARD_COUNT,
        })
    }

    /// cargo-mutants' arguments that select this shard; [`reconcile::run`] adds the
    /// rest.
    fn cargo_mutants_args(self) -> Vec<String> {
        vec![
            format!("--shard={}/{}", self.index, self.count),
            "--sharding=round-robin".to_owned(),
        ]
    }
}

fn run(shard: &str, updates: IssueUpdates) -> Result<()> {
    let shard = Shard::numbered(shard)?;
    eprintln!(
        "mutants-shard: testing shard {}/{}",
        shard.index, shard.count
    );
    reconcile::run(&shard.cargo_mutants_args(), FoundBy::Shard(shard), updates)
}

#[cfg(test)]
mod tests {
    use super::Shard;

    fn shard(index: u32) -> Shard {
        Shard { index, count: 7 }
    }

    #[test]
    fn the_shard_is_the_number_the_workflow_names() {
        assert_eq!(Shard::numbered("0").ok(), Some(shard(0)));
        assert_eq!(Shard::numbered("6").ok(), Some(shard(6)));
    }

    #[test]
    fn a_shard_past_the_last_or_not_a_number_is_an_error() {
        assert!(Shard::numbered("7").is_err());
        assert!(Shard::numbered("-1").is_err());
        assert!(Shard::numbered("monday").is_err());
    }

    #[test]
    fn a_shard_is_tested_round_robin() {
        assert_eq!(
            shard(3).cargo_mutants_args(),
            ["--shard=3/7", "--sharding=round-robin"]
        );
    }
}
