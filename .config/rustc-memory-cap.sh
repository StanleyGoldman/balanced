#!/usr/bin/env bash
# `build.rustc-wrapper` in the repo's `.cargo/config.toml`, so every local build uses it,
# from agent sessions and terminals alike: runs each rustc, and so the linker it spawns,
# in the user slice `balanced-builds.slice`, so several builds at once are killed one
# rustc at a time instead of the kernel's OOM killer taking every process on the
# machine. One fresh workspace build peaks at about 6 GB.
#
# Usage: rustc-memory-cap.sh <rustc> <args...>   (cargo passes these)
#
# The cap, a systemd size (e.g. 16G), is on every rustc together, across all worktrees
# and sessions: $BALANCED_BUILDS_SLICE_MEMORY_MAX, default 20G, which leaves room for
# the 24G of `balanced-tests.slice` (`memory-cap.sh`) and about 13G in use at idle.
# It is set the first time a build runs after login and kept until logout; to change
# it, run `systemctl --user set-property --runtime balanced-builds.slice
# MemoryMax=<size>`. A rustc over the cap is killed with SIGKILL, which cargo reports
# as `(signal: 9, SIGKILL: kill)`. Without a systemd user session (CI containers,
# cargo-mutants in Docker) rustc runs uncapped.
#
# When `sccache` is on PATH, rustc runs through it with SCCACHE_CLIENT_SIDE=1: the
# sccache client compiles in its own process, inside the slice, instead of handing the
# compile to its server outside it. Verified with sccache 0.19.0; an sccache without
# client-side mode would compile in its server, uncapped. A RUSTC_WRAPPER env var overrides this script, so
# `RUSTC_WRAPPER= cargo ...` builds with neither the cap nor sccache.
set -euo pipefail

if command -v sccache >/dev/null; then
  export SCCACHE_CLIENT_SIDE=1
  set -- sccache "$@"
fi

if ! command -v systemd-run >/dev/null || [[ ! -S ${XDG_RUNTIME_DIR:-/nonexistent}/bus ]]; then
  exec "$@"
fi

# systemd nests a dashed slice name: balanced-builds.slice sits in balanced.slice.
slice_max=/sys/fs/cgroup/user.slice/user-$UID.slice/user@$UID.service/balanced.slice/balanced-builds.slice/memory.max
if [[ ! -r $slice_max || $(<"$slice_max") == max ]]; then
  systemctl --user set-property --runtime balanced-builds.slice \
    MemoryMax="${BALANCED_BUILDS_SLICE_MEMORY_MAX:-20G}" MemorySwapMax=0
fi
exec systemd-run --user --scope --quiet --collect --slice=balanced-builds -- "$@"
