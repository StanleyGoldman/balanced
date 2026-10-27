# Tooling

## Toolchain

- `rust-toolchain.toml` pins the exact Rust release, and `rust-version` in `Cargo.toml` names the same release. rustup installs the pinned toolchain the first time you build.
- Bump both together in a dedicated PR, every second stable release. Fix any new clippy lints in that same PR.

## Checks

Every check lives in `xtask` (`cargo xtask` lists them), so the same commands run locally and in CI. CI is GitHub Actions, on Linux only ([ADR 0010](../adr/0010-ci-runs-on-github-actions.md)): `.github/workflows/ci.yml` runs plain `cargo xtask ci` on every pull request and every push to `main`. On a pull request it checks out GitHub's merge of the branch into its target. When every changed path is prose or agent tooling (the `changes` step lists the globs), it runs only `cargo xtask typos` and `cargo xtask adrs` instead. Every step runs in the image `.github/Dockerfile` builds (see [ADR 0004](../adr/0004-ci-image-matches-the-snapshot-machine.md)), through `.github/in-image.sh`. Read runs with `gh run list` and `gh run view <id> --log-failed`. A job's summary on the run page lists its failed tests, links the snapshot diffs, gives each step's sccache hit rate (or says the cache was unreachable and the step built without it), and for `mutants.yml` lists the issues the run updated.

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
- Read a `cargo xtask ci` run's verdict from its exit status: send its output to a scratchpad file and check `$?` (`cargo xtask ci > <scratchpad>/ci.log 2>&1; echo $?`), then read the log from its last `==>` step. Piping it through `tail` or `grep` reports that filter's status, and the passing tests' lines can bury the failing step, such as a `typos` error after 1600 passing tests.
- `cargo xtask coverage [--base <ref>] [--fail-under-diff <percent>]` is the CI task that stands in for `test` in the workflow's `test` job, and isn't part of `ci`: it runs the same suite and doctests with coverage measured and reports the result (see [testing](testing.md#coverage)).
- `cargo xtask adrs --against <ref>` runs the same check on this branch merged with `<ref>`, without touching the checkout. It catches a record that landed on `<ref>` with the same number after this branch was cut. Both branches merge clean, because the file names differ.
- `dco` starts from a fixed commit, not `origin/main`: CI fetches a push or cron run by SHA alone, and a stacked pull request with only the branch below it, so `origin/main` exists in neither. It needs full history; a shallow clone fails naming the cutoff. Merge commits are exempt, because GitHub's merge button and "Update branch" make unsigned ones. CI runs it on every change, prose-only ones included, since those make commits too. Each push to `main` checks every commit since the cutoff, so an unsigned commit that lands turns `main` red; history on `main` can't be rewritten, so move `CUTOFF` to that commit, with a comment naming it.
- `cargo xtask mutants [<base>] [--re <pattern>]` is the by-hand mutation check, for proving a mutant's fix (see [testing](testing.md#mutation-testing)).
- `cargo xtask mutants-shard <shard> [--dry-run]` is a CI task: `.github/workflows/mutants.yml` runs it each night on `main`, with the weekday's number, Sunday 0 to Saturday 6. It tests that round-robin shard of every mutant and opens, comments on and closes `mutant` issues. It refuses a dirty tree, because it mutates files in place, and it needs `GITHUB_TOKEN`; `--dry-run` prints the issue updates instead of making them. A missed mutant leaves it green; a run that couldn't finish or couldn't update its issues is red.
- `cargo xtask mutants-all [--dry-run]` is the same kind of CI task, run only by hand: `gh workflow run mutants.yml -f task=mutants-all` tests every mutant on `main`, all seven shards in one run, and updates the issues the same way.
- `cargo xtask mutants-landed [--dry-run]` is the same kind of CI task, run by `.github/workflows/mutants.yml` on each push to `main` that changes more than prose or agent tooling. It asks GitHub which merged pull request left `HEAD` as its merge commit, then tests the mutants in the diff that pull request landed and updates the issues the same way. A push no pull request landed, or a diff with no mutants, tests nothing and stays green; a failed lookup is red. By hand it needs a clean checkout whose `HEAD` is a real merge commit on `main`.
- The external tools need a one-time install: `cargo install --locked cargo-deny typos-cli cargo-mutants cargo-nextest cargo-insta cargo-llvm-cov`, and `rustup component add llvm-tools` for coverage. `.github/Dockerfile` installs the same tools for CI. When one is missing (`no such command`), tell the user straight away, with this install command, before working around it.
- Warnings are allowed while you iterate. `cargo xtask clippy` denies them.
- A test nextest reports as `SIGKILL` hit the memory cap in `.config/memory-cap.sh`. Look for a loop that allocates without end, such as a walk up parent links that meets a cycle. A test-first run before the guard exists, or a red-check that breaks the guard, triggers it on purpose. Raise the cap (`BALANCED_TEST_MEMORY_MAX=16G`) only for a test that needs the memory.
- A build that fails with `rustc-memory-cap.sh ... (signal: 9, SIGKILL: kill)` hit the build cap: every local rustc together, from agent sessions and terminals, went over the shared cap on `balanced-builds.slice` (`.config/rustc-memory-cap.sh`). Other builds were running at the same time, so rerun the build once they finish. If one build alone needs more, raise the cap for the rest of the login with `systemctl --user set-property --runtime balanced-builds.slice MemoryMax=<size>`.
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
| `.config/nextest.toml` | The one nextest profile, used by `test`, `snapshots`, `locales` and cargo-mutants: no fail-fast, no retries, a hung test killed after 3 minutes, and each test process run through `.config/memory-cap.sh` |
| `.config/memory-cap.sh` | The memory cap on each test process (8G) and on every test process from every session together (24G, in `balanced-tests.slice`), so a runaway test is killed alone instead of the OOM killer taking every session down. The header names the overrides. Uncapped where there is no systemd user session, as in CI |
| `.config/rustc-memory-cap.sh` | The `build.rustc-wrapper` (set in `.cargo/config.toml`) that caps every local rustc and linker together (20G, in `balanced-builds.slice`), and runs them through sccache when it is installed. The header names the overrides. Uncapped where there is no systemd user session |
| `.claude/hooks/build-memory-cap.sh` | The SessionStart hook that warns, to the user and the agent, when a `RUSTC_WRAPPER` or `CARGO_BUILD_RUSTC_WRAPPER` env var overrides the build cap |
| `.typos.toml` | Words typos accepts, each with a comment, and the translations it skips |

- To add a lint or a ban, put it in config, not prose. A ban's `reason` names the replacement to use.
- A ban path into a crate that isn't a dependency yet resolves silently. Once the crate is a dependency, clippy warns if a path doesn't resolve, so check the output after adding a crate that `clippy.toml` mentions.
- `cargo deny` warns `unused-wrapper` for egui-family crates that aren't dependencies yet. The warning is expected and goes away as those crates arrive.
- When a skip, exception or allowed lint is no longer needed, delete it in the PR that made it unnecessary.
- Adding a dependency can make cargo re-resolve crates already in `Cargo.lock`. Adding `egui_extras` moved `tempfile` from getrandom 0.3.4 to 0.4.3, a second getrandom that `cargo deny` rejects as a duplicate. After adding a dependency, read `git diff Cargo.lock`. The new packages are expected. If an existing package's dependency moved to a version that duplicates another, edit that line back in `Cargo.lock`, then confirm cargo accepts it with `cargo metadata --locked --format-version 1 > /dev/null`.

## Build cache

- In CI, every run builds into an empty `target/`, and sccache caches the library crates in the GitHub Actions cache (see [ADR 0003](../adr/0003-ci-caches-with-sccache-not-a-persistent-target.md) and [ADR 0010](../adr/0010-ci-runs-on-github-actions.md)).
- Locally, the repo's `.cargo/config.toml` sets `build.rustc-wrapper` to the build memory cap, `.config/rustc-memory-cap.sh`, for every build. Installing sccache is all a developer does to get the cache: when `sccache` is on `PATH`, the cap runs rustc through it with `SCCACHE_CLIENT_SIDE=1`, so the sccache client compiles in its own process, inside `balanced-builds.slice`. A `build.rustc-wrapper` in `~/.cargo/config.toml` is overridden by the repo's.
- A `RUSTC_WRAPPER` or `CARGO_BUILD_RUSTC_WRAPPER` env var overrides the repo's config and drops the cap; the SessionStart hook warns when one is set. CI sets `RUSTC_WRAPPER=sccache` on purpose, and cargo-mutants sets it empty for its own builds (ADR 0002).
- Agents leave `RUSTC_WRAPPER`, `CARGO_HOME` and `CARGO_TARGET_DIR` as the session starts them. Each worktree builds into its own `target/`.
- **Verified 2026-10-27 (sccache 0.19.0, Rust 1.99.0), building balanced at `origin/main` in two fresh worktrees, each with an empty cache:**
  - A new worktree reuses about two thirds of another worktree's cache with no extra setup: `cargo build --workspace --tests` got 257 hits out of 378, and the build took 53 s against 61 s cold.
  - `SCCACHE_BASEDIRS` listing both worktrees changed nothing, whether set for the client or for the server: 257 of 378 again, and 216 of 331 for `--lib --bins`. The misses are crates whose cache key still holds the worktree's path. In a smaller probe these were the crates with build scripts (`serde`, `serde_core`), even though their build-script output was identical in both worktrees. The likely cause is their `OUT_DIR`, which sits under each worktree's `target/`; the crates built on them then miss in turn. Unconfirmed.
  - Under client-side mode each rustc is a child of the sccache client, in `balanced-builds.slice`, when the cap runs sccache.
  - sccache 0.18.0 (2026-10-13) differed: a new worktree got 0 hits, and `SCCACHE_BASEDIRS` was what let worktrees share.

## Measuring performance

- Time in the `release` profile, on a quiet machine. Other agent sessions build in parallel on this machine, and a probe run at load average 18 gave medians up to 4× too high, with a wide spread (#258). Wait until `/proc/loadavg` reads below 3 (a background `until` loop), then pin the run to one core with `taskset -c 0`.
- A Command's cost has a committed benchmark: `taskset -c 0 cargo bench -p balanced_core --features test-support` times a post, a reordering date edit, a delete and a flag at 1× and 10× the stub (`crates/balanced_core/benches/commands.rs`). For before-and-after numbers, build the base commit's bench in a throwaway worktree and run both binaries in the same quiet window. Its data file goes under `target/tmp`, on disk: `/tmp` here is tmpfs, where a commit skips the fsync that is about 3 ms of a real one.
- What a window rebuilds after a Command has two committed benchmarks in `crates/balanced_ui/benches/`, each run with `taskset -c 0 cargo bench -p balanced_ui --features test-support --bench <name>`: `rebuild` times the Register's rows (from scratch, and rebuilt from the rows before a Command), the comboboxes' choices and the usage count on their own, and `frame` times App frames: idle, after a Command, after a filter keystroke and after a Payee sort click. Both run at 20,000, 90,000 and 200,000 transactions. The frame after a Command includes the Command's commit and its fsync, so read it beside the `commands` benchmark from the same window. For before and after, copy the base's bench binaries out of `target/release/deps` before building the change; the rebuild writes over them under the same name.
- `perf record` is refused here (`perf_event_paranoid` is 4). Break a cost down by timing its parts in the probe itself. A counting `#[global_allocator]` in a `#[cfg(test)]` module measures heap held.
- For a cold page cache without root, evict the file first: `os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)` from `python3`.
- To save building the base's dependencies cold, build its benches in a throwaway worktree with `CARGO_TARGET_DIR` set to the change's `target/`, then copy the binaries out. The change's sources are then older than that build's output, so cargo calls the change fresh and its next `cargo bench --no-run` hands back the base binary under the same name. `touch` the change's edited sources (`git diff --name-only <base> -- crates`) before building it, then `cmp` the two binaries: the same bytes mean the "after" run times the base.
- `docs/research/data-file-performance.md` and `docs/research/ui-rebuild-performance.md` are worked examples: probes, method and machine.
