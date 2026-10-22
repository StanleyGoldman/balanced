#!/usr/bin/env bash
# SessionStart hook (see `.claude/settings.json`): turns on the build memory cap,
# `.config/rustc-memory-cap.sh`, for the agent session. It appends
# `export RUSTC_WRAPPER=<this checkout>/.config/rustc-memory-cap.sh` to
# $CLAUDE_ENV_FILE, which Claude Code runs before each Bash command. The path is
# absolute, so cargo finds the wrapper from any directory and in checkouts that
# predate it.
#
# Leaves the session as it is when a wrapper is already set (RUSTC_WRAPPER,
# CARGO_BUILD_RUSTC_WRAPPER) or configured (`build.rustc-wrapper`, e.g. sccache in
# ~/.cargo/config.toml): RUSTC_WRAPPER would override it, and that session's builds
# stay uncapped. Also when there is no $CLAUDE_ENV_FILE, or when cargo can't answer,
# which it reports on stderr.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
if [[ -z ${CLAUDE_ENV_FILE:-} || -n ${RUSTC_WRAPPER:-} ]]; then
  exit 0
fi
# `cargo config get` is the only way to read cargo's merged config (files and
# CARGO_BUILD_RUSTC_WRAPPER), and it is unstable; RUSTC_BOOTSTRAP=1 lets stable cargo
# run it. An unset key is its one error that says "is not set".
if configured=$(cd "$root" && RUSTC_BOOTSTRAP=1 cargo -Zunstable-options config get \
  build.rustc-wrapper 2>&1); then
  exit 0
fi
if [[ $configured == *"is not set"* ]]; then
  printf "export RUSTC_WRAPPER='%s'\n" "$root/.config/rustc-memory-cap.sh" >>"$CLAUDE_ENV_FILE"
else
  echo "build-memory-cap: builds stay uncapped; cargo config get failed: $configured" >&2
fi
