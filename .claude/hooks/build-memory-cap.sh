#!/usr/bin/env bash
# SessionStart hook (see `.claude/settings.json`): warns when the session's builds
# bypass the build memory cap. The repo's `.cargo/config.toml` sets
# `build.rustc-wrapper` to `.config/rustc-memory-cap.sh`, which also runs sccache, but a
# RUSTC_WRAPPER or CARGO_BUILD_RUSTC_WRAPPER env var overrides that config. When either
# is set (even empty) to anything but a `rustc-memory-cap.sh`, prints a SessionStart
# JSON result on stdout: `systemMessage` for the user, `additionalContext` for the
# agent. Otherwise prints nothing. Always exits 0.
set -euo pipefail

for var in RUSTC_WRAPPER CARGO_BUILD_RUSTC_WRAPPER; do
  if [[ -n ${!var+set} && ${!var} != *rustc-memory-cap.sh ]]; then
    message="build-memory-cap: $var='${!var}' overrides the repo's build.rustc-wrapper, so this session's builds run uncapped. Unset it and restart the session; the cap already runs sccache when it is installed."
    jq -n --arg m "$message" \
      '{systemMessage: $m, hookSpecificOutput: {hookEventName: "SessionStart", additionalContext: $m}}'
    exit 0
  fi
done
