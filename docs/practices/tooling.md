# Tooling

## Toolchain

- `rust-toolchain.toml` pins the exact Rust release, and `rust-version` in `Cargo.toml` names the same release. rustup installs the pinned toolchain the first time you build.
- Bump both together in a dedicated PR, every second stable release. Fix any new clippy lints in that same PR.

## Checks

Every check lives in `xtask` (`cargo xtask` lists them), so the same commands run locally and in CI. CI is GitHub Actions, on Linux only ([ADR 0010](../adr/0010-ci-runs-on-github-actions.md)): `.github/workflows/ci.yml` runs plain `cargo xtask ci` on every pull request and every push to `main`. On a pull request it checks out GitHub's merge of the branch into its target. When every changed path is prose or agent tooling (the `changes` step lists the globs), it runs only `cargo xtask typos` and `cargo xtask adrs` instead. Every step runs in the image `.github/Dockerfile` builds (see [ADR 0004](../adr/0004-ci-image-matches-the-snapshot-machine.md)), through `.github/in-image.sh`. Read runs with `gh run list` and `gh run view <id> --log-failed`. A job's summary on the run page lists its failed tests, links the snapshot diffs, gives each step's sccache hit rate, and for `mutants.yml` lists the issues the run updated.

- `cargo xtask ci` runs the following, stopping at the first failure:
  - `fmt`
  - `clippy`, with `-D warnings`
  - `test`: the suite under cargo-nextest, skipping the image snapshots, then the doctests with `cargo test --doc`, which nextest can't run
  - `snapshots`: the image snapshots, on Linux only (see [testing](testing.md#image-snapshots))
  - `locales`: reports how many messages each translation lacks, without failing (see [i18n](i18n.md#translations))
  - `deny`, which is `cargo deny check`
  - `layers`: every workspace crate depends only on the crates below it, as `ARCHITECTURE.md` draws them, and only `balanced_store` depends on `rusqlite`. The rule is the `LAYERS` table in `xtask/src/layers.rs`; a new crate goes there and in `ARCHITECTURE.md` in the same PR.
  - `typos`
  - `todo`: every `TODO` or `FIXME` in a `.rs` or `.toml` file is written `TODO(#123)`
  - `spdx`: every `.rs` file git doesn't ignore, untracked ones included, opens with `// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception` as its first line. The expression matches `license` in `[workspace.package]`; change both together.
  - `adrs`: no two records in `docs/adr/` share a number, and every file there is named `<four digits>-<slug>.md`. Gaps in the numbering are fine.
  - `dco`: every non-merge commit in `HEAD`'s history after `CUTOFF` in `xtask/src/dco.rs` carries a `Signed-off-by:` trailer ([ADR 0009](../adr/0009-gpl-3-or-later-with-a-plugin-exception-and-dco-sign-off.md), [CONTRIBUTING.md](../../CONTRIBUTING.md)). `git rebase --signoff <base>` repairs a branch.
- `cargo xtask coverage [--base <ref>] [--fail-under-diff <percent>]` is the CI task that stands in for `test` in the workflow's `test` job, and isn't part of `ci`: it runs the same suite and doctests with coverage measured and reports the result (see [testing](testing.md#coverage)).
- `cargo xtask adrs --against <ref>` runs the same check on this branch merged with `<ref>`, without touching the checkout. It catches a record that landed on `<ref>` with the same number after this branch was cut. Both branches merge clean, because the file names differ.
- `dco` starts from a fixed commit, not `origin/main`: CI fetches a push or cron run by SHA alone, and a stacked pull request with only the branch below it, so `origin/main` exists in neither. It needs full history; a shallow clone fails naming the cutoff. Merge commits are exempt, because GitHub's merge button and "Update branch" make unsigned ones. Each push to `main` checks every commit since the cutoff, so an unsigned commit that lands turns `main` red; history on `main` can't be rewritten, so move `CUTOFF` to that commit, with a comment naming it.
- `cargo xtask mutants [<base>] [--re <pattern>]` is the by-hand mutation check, for proving a mutant's fix (see [testing](testing.md#mutation-testing)).
- `cargo xtask mutants-shard <shard> [--dry-run]` is a CI task: `.github/workflows/mutants.yml` runs it each night on `main`, with the weekday's number, Sunday 0 to Saturday 6. It tests that round-robin shard of every mutant and opens, comments on and closes `mutant` issues. It refuses a dirty tree, because it mutates files in place, and it needs `GITHUB_TOKEN`; `--dry-run` prints the issue updates instead of making them. A missed mutant leaves it green; a run that couldn't finish or couldn't update its issues is red.
- `cargo xtask mutants-all [--dry-run]` is the same kind of CI task, run only by hand: `gh workflow run mutants.yml -f task=mutants-all` tests every mutant on `main`, all seven shards in one run, and updates the issues the same way.
- `cargo xtask mutants-landed [--dry-run]` is the same kind of CI task, run by `.github/workflows/mutants.yml` on each push to `main` that changes more than prose or agent tooling. It asks GitHub which merged pull request left `HEAD` as its merge commit, then tests the mutants in the diff that pull request landed and updates the issues the same way. A push no pull request landed, or a diff with no mutants, tests nothing and stays green; a failed lookup is red. By hand it needs a clean checkout whose `HEAD` is a real merge commit on `main`.
- The external tools need a one-time install: `cargo install --locked cargo-deny typos-cli cargo-mutants cargo-nextest`.
- Warnings are allowed while you iterate. `cargo xtask clippy` denies them.
- A new check is written as a new `xtask` task and added to `ci`, never as CI-provider config. Each task is its own module in `xtask/src/` with a `TASK` constant, listed once in `TASKS` in `main.rs`. Running programs goes through `process.rs`.
- To dry-run a branch's change to a CI task that reads `HEAD` and refuses a dirty tree, such as `mutants-shard`, on a real commit of `main` before the change lands, commit it and run `.claude/scripts/xtask-at-commit.sh <commit> <task> --dry-run` from the branch's worktree (contract in its header).
- To reproduce a CI failure locally, build the image with `docker build -t balanced-ci:latest -f .github/Dockerfile .`, then run the failing task in it with `.github/in-image.sh cargo xtask <task>`.
- Pin every action outside GitHub's own `actions/*` by its full commit SHA, with the tag in a comment (`uses: mikepenz/action-junit-report@<sha> # v6.6.1`); `gh api repos/<owner>/<repo>/commits/<tag> --jq .sha` resolves one. A tag can be moved to other code; a SHA can't. The workflows pin `actions/*` the same way, for one form throughout.
- Lint a change under `.github/` with `docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:latest`, which runs shellcheck on each `run:` step too.
- A change under `.github/` is proven only by a run on GitHub. Open the pull request and read its checks; for `mutants.yml`, run it with `gh workflow run mutants.yml -f task=<task>` once it is on `main`.

## Config files

| File | Holds |
|---|---|
| `Cargo.toml` `[workspace.lints]` | Lint levels: clippy `all` + `pedantic`, restriction cherry-picks, and allowed lints with one-line reasons |
| `clippy.toml` | Clippy options and project bans (`disallowed-methods`, `disallowed-macros`), each with a reason |
| `rustfmt.toml` | Stable options only. Import grouping isn't enforced. |
| `deny.toml` | Licences, advisories, duplicate versions and the egui layer ban |
| `.cargo/mutants.toml` | Which crates cargo-mutants mutates, and its exclusions |
| `.config/nextest.toml` | The one nextest profile, used by `test`, `snapshots`, `locales` and cargo-mutants: no fail-fast, no retries, a hung test killed after 3 minutes |
| `.typos.toml` | Words typos accepts, each with a comment, and the translations it skips |

- To add a lint or a ban, put it in config, not prose. A ban's `reason` names the replacement to use.
- A ban path into a crate that isn't a dependency yet resolves silently. Once the crate is a dependency, clippy warns if a path doesn't resolve, so check the output after adding a crate that `clippy.toml` mentions.
- `cargo deny` warns `unused-wrapper` for egui-family crates that aren't dependencies yet. The warning is expected and goes away as those crates arrive.
- When a skip, exception or allowed lint is no longer needed, delete it in the PR that made it unnecessary.
- Adding a dependency can make cargo re-resolve crates already in `Cargo.lock`. Adding `egui_extras` moved `tempfile` from getrandom 0.3.4 to 0.4.3, a second getrandom that `cargo deny` rejects as a duplicate. After adding a dependency, read `git diff Cargo.lock`. The new packages are expected. If an existing package's dependency moved to a version that duplicates another, edit that line back in `Cargo.lock`, then confirm cargo accepts it with `cargo metadata --locked --format-version 1 > /dev/null`.

## Build cache

- In CI, every run builds into an empty `target/`, and sccache caches the library crates in the GitHub Actions cache (see [ADR 0003](../adr/0003-ci-caches-with-sccache-not-a-persistent-target.md) and [ADR 0010](../adr/0010-ci-runs-on-github-actions.md)).
- Locally, sccache is optional and set up per developer, in `~/.cargo/config.toml` (`[build] rustc-wrapper = "sccache"`). The repo's `.cargo/config.toml` holds only aliases.
- Agents leave `RUSTC_WRAPPER`, `CARGO_HOME` and `CARGO_TARGET_DIR` as they find them, and add no repo-level `rustc-wrapper`. Each worktree builds into its own `target/`.
- **Verified 2026-10-13 (sccache 0.18.0, Rust 1.99.0), with egui 0.36.2 + eframe + tracing as a test dependency set:**
  - sccache works under the Claude Code sandbox. The client reaches the server on `127.0.0.1:4226`, and `sccache --show-stats` counts rise during a worktree build.
  - With the default setup, the cache key includes absolute paths. A first worktree build missed all 266 cacheable crates. Building the same tree again with a different `CARGO_TARGET_DIR` also got 0 hits.
  - The server was then restarted with `SCCACHE_BASEDIRS` listing two checkout roots:
    - Rebuilding the first worktree after `rm -rf target` got 266 hits out of 266.
    - A copy of the worktree at another path got 219 hits and 47 misses.
  - So by default **each new worktree builds its dependencies cold**, and sccache speeds up only rebuilds at the same path. Sharing across worktrees needs `SCCACHE_BASEDIRS`, set in the environment that starts the sccache server and listing every worktree root.

## Measuring performance

- Time in the `release` profile, on a quiet machine. Other agent sessions build in parallel on this machine, and a probe run at load average 18 gave medians up to 4× too high, with a wide spread (#258). Wait until `/proc/loadavg` reads below 3 (a background `until` loop), then pin the run to one core with `taskset -c 0`.
- `perf record` is refused here (`perf_event_paranoid` is 4). Break a cost down by timing its parts in the probe itself. A counting `#[global_allocator]` in a `#[cfg(test)]` module measures heap held.
- For a cold page cache without root, evict the file first: `os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)` from `python3`.
- `docs/research/data-file-performance.md` is a worked example: probes, method and machine.
