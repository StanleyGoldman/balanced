#!/usr/bin/env bash
# RUSTC_WRAPPER for agent sessions, exported by the SessionStart hook in
# `.claude/hooks/build-memory-cap.sh`: runs each rustc, and so the linker it spawns, in
# the user slice `balanced-builds.slice`, so several sessions building at once are
# killed one rustc at a time instead of the kernel's OOM killer taking every process
# on the machine. One fresh workspace build peaks at about 6 GB.
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
set -euo pipefail

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
