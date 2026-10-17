# Tooling

## Toolchain

- `rust-toolchain.toml` pins the exact Rust release, and `rust-version` in `Cargo.toml` names the same release. rustup installs the pinned toolchain the first time you build.
- Bump both together in a dedicated PR, every second stable release. Fix any new clippy lints in that same PR.

## Checks

Every check lives in `xtask` (`cargo xtask` lists them), so the same commands run locally and in CI. CI is Woodpecker, on Linux only: `.woodpecker/ci.yml` runs plain `cargo xtask ci` on every pull request and every push to `main`, in the image `.woodpecker/Dockerfile` builds (see [ADR 0004](../adr/0004-ci-image-matches-the-snapshot-machine.md)). It is never GitHub Actions.

- `cargo xtask ci` runs the following, stopping at the first failure:
  - `fmt`
  - `clippy`, with `-D warnings`
  - `test`, which skips the image snapshots
  - `snapshots`: the image snapshots, on Linux only (see [testing](testing.md#image-snapshots))
  - `locales`: reports how many messages each translation lacks, without failing (see [i18n](i18n.md#translations))
  - `deny`, which is `cargo deny check`
  - `typos`
  - `todo`: every `TODO` or `FIXME` in a `.rs` or `.toml` file is written `TODO(#123)`
- `cargo xtask mutants [<base>]` is the advisory mutation check (see [testing](testing.md#mutation-testing)).
- The external tools need a one-time install: `cargo install --locked cargo-deny typos-cli cargo-mutants`.
- Warnings are allowed while you iterate. `cargo xtask clippy` denies them.
- A new check is written as a new `xtask` task and added to `ci`, never as CI-provider config. Each task is its own module in `xtask/src/` with a `TASK` constant, listed once in `TASKS` in `main.rs`. Running programs goes through `process.rs`.
- After changing a file under `.woodpecker/`, lint it with `woodpecker-cli lint --strict <file>`, then run it locally with `.claude/scripts/woodpecker-exec.sh <file>` before pushing. The script runs the committed HEAD; its contract is in its header.

One check is still to come: a periodic full mutants run. It arrives when there is code for it to check.

## Config files

| File | Holds |
|---|---|
| `Cargo.toml` `[workspace.lints]` | Lint levels: clippy `all` + `pedantic`, restriction cherry-picks, and allowed lints with one-line reasons |
| `clippy.toml` | Clippy options and project bans (`disallowed-methods`, `disallowed-macros`), each with a reason |
| `rustfmt.toml` | Stable options only. Import grouping isn't enforced. |
| `deny.toml` | Licences, advisories, duplicate versions and the egui layer ban |
| `.cargo/mutants.toml` | Which crates cargo-mutants mutates, and its exclusions |
| `.typos.toml` | Words typos accepts, each with a comment |

- To add a lint or a ban, put it in config, not prose. A ban's `reason` names the replacement to use.
- A ban path into a crate that isn't a dependency yet resolves silently. Once the crate is a dependency, clippy warns if a path doesn't resolve, so check the output after adding a crate that `clippy.toml` mentions.
- `cargo deny` warns `unused-wrapper` for egui-family crates that aren't dependencies yet. The warning is expected and goes away as those crates arrive.
- When a skip, exception or allowed lint is no longer needed, delete it in the PR that made it unnecessary.
- Adding a dependency can make cargo re-resolve crates already in `Cargo.lock`. Adding `egui_extras` moved `tempfile` from getrandom 0.3.4 to 0.4.3, a second getrandom that `cargo deny` rejects as a duplicate. After adding a dependency, read `git diff Cargo.lock`. The new packages are expected. If an existing package's dependency moved to a version that duplicates another, edit that line back in `Cargo.lock`, then confirm cargo accepts it with `cargo metadata --locked --format-version 1 > /dev/null`.

## Build cache

- sccache is optional and set up per developer, in `~/.cargo/config.toml` (`[build] rustc-wrapper = "sccache"`). The repo's `.cargo/config.toml` holds only aliases.
- Agents leave `RUSTC_WRAPPER`, `CARGO_HOME` and `CARGO_TARGET_DIR` as they find them, and add no repo-level `rustc-wrapper`. Each worktree builds into its own `target/`.
- **Verified 2026-10-13 (sccache 0.18.0, Rust 1.99.0), with egui 0.36.2 + eframe + tracing as a test dependency set:**
  - sccache works under the Claude Code sandbox. The client reaches the server on `127.0.0.1:4226`, and `sccache --show-stats` counts rise during a worktree build.
  - With the default setup, the cache key includes absolute paths. A first worktree build missed all 266 cacheable crates. Building the same tree again with a different `CARGO_TARGET_DIR` also got 0 hits.
  - The server was then restarted with `SCCACHE_BASEDIRS` listing two checkout roots:
    - Rebuilding the first worktree after `rm -rf target` got 266 hits out of 266.
    - A copy of the worktree at another path got 219 hits and 47 misses.
  - So by default **each new worktree builds its dependencies cold**, and sccache speeds up only rebuilds at the same path. Sharing across worktrees needs `SCCACHE_BASEDIRS`, set in the environment that starts the sccache server and listing every worktree root.
