//! `cargo xtask mutants-shard`: the nightly CI run that mutation-tests one shard of
//! every mutant on `main` and reconciles the `mutant` issues (ADR 0002).

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result};

use super::plan::{FoundBy, Shard};
use super::reconcile::{self, IssueUpdates};
use crate::Task;

pub(crate) const TASK: Task = Task {
    name: "mutants-shard",
    args: "[--dry-run]",
    summary: "a CI task: mutation-test tonight's shard of every mutant and update the mutant \
              issues, or with --dry-run print the updates; by hand, use mutants",
    run: |args| match *args {
        [] => Some(run(IssueUpdates::Apply)),
        ["--dry-run"] => Some(run(IssueUpdates::Print)),
        _ => None,
    },
};

/// How many shards the mutants are split into. A night's run tests one, so all of
/// `main` is tested once a week. Change it only on purpose, when a night nears the
/// step timeout: a new count reshuffles every shard.
const SHARD_COUNT: u32 = 7;

const SECONDS_PER_DAY: u64 = 86_400;

impl Shard {
    /// The shard a run starting `unix_seconds` after the Unix epoch tests: the days
    /// since the epoch, in UTC, modulo [`SHARD_COUNT`].
    fn at(unix_seconds: u64) -> Self {
        let days = unix_seconds / SECONDS_PER_DAY;
        let index = u32::try_from(days % u64::from(SHARD_COUNT))
            .expect("a remainder of the shard count fits its type");
        Self {
            index,
            count: SHARD_COUNT,
        }
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

fn run(updates: IssueUpdates) -> Result<()> {
    #[expect(
        clippy::disallowed_methods,
        reason = "the nightly shard is chosen from the date the run starts; xtask has no Clock"
    )]
    let now = SystemTime::now();
    let seconds = now
        .duration_since(UNIX_EPOCH)
        .context("the system clock is before 1970")?
        .as_secs();
    let shard = Shard::at(seconds);
    eprintln!(
        "mutants-shard: testing shard {}/{} (days since the Unix epoch mod {})",
        shard.index, shard.count, shard.count
    );
    reconcile::run(&shard.cargo_mutants_args(), FoundBy::Shard(shard), updates)
}

#[cfg(test)]
mod tests {
    use super::Shard;

    /// 2026-01-01T00:00:00Z, day 20454 since the epoch.
    const NEW_YEAR_2026: u64 = 1_767_225_600;

    fn shard(index: u32) -> Shard {
        Shard { index, count: 7 }
    }

    #[test]
    fn the_shard_turns_over_at_midnight_utc_across_the_year_boundary() {
        assert_eq!(Shard::at(NEW_YEAR_2026 - 1), shard(6));
        assert_eq!(Shard::at(NEW_YEAR_2026), shard(0));
        assert_eq!(Shard::at(NEW_YEAR_2026 + 86_399), shard(0));
        assert_eq!(Shard::at(NEW_YEAR_2026 + 86_400), shard(1));
    }

    #[test]
    fn the_shards_cycle_weekly() {
        assert_eq!(Shard::at(NEW_YEAR_2026 + 3 * 86_400), shard(3));
        assert_eq!(Shard::at(NEW_YEAR_2026 + 10 * 86_400), shard(3));
    }

    #[test]
    fn the_epoch_is_shard_zero() {
        assert_eq!(Shard::at(0), shard(0));
    }

    #[test]
    fn a_shard_is_tested_round_robin() {
        assert_eq!(
            shard(3).cargo_mutants_args(),
            ["--shard=3/7", "--sharding=round-robin"]
        );
    }
}
