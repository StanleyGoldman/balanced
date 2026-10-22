#!/usr/bin/env bash
# nextest wrapper script (see `.config/nextest.toml`): runs one test process with its
# memory capped, so a runaway test is killed alone instead of the kernel's OOM killer
# taking every process on the machine. A tree walk that looped on a parent cycle once
# reached 32 GB in two minutes, before the 3-minute slow-timeout could fire.
#
# Usage: memory-cap.sh <test binary> <args...>   (nextest passes these)
#
# Two caps, both systemd sizes (e.g. 4G):
#   - each test process: $BALANCED_TEST_MEMORY_MAX, default 8G;
#   - every test process together, across all worktrees and sessions, in the user slice
#     `balanced-tests.slice`: $BALANCED_TESTS_SLICE_MEMORY_MAX, default 24G. It is set
#     the first time a test runs after login and kept until logout; to change it, run
#     `systemctl --user set-property --runtime balanced-tests.slice MemoryMax=<size>`.
# A test over either cap is killed with SIGKILL, which nextest reports as `SIGKILL`.
# Without a systemd user session (CI containers, cargo-mutants in Docker) the test runs
# uncapped.
set -euo pipefail

if ! command -v systemd-run >/dev/null || [[ ! -S ${XDG_RUNTIME_DIR:-/nonexistent}/bus ]]; then
  exec "$@"
fi

# systemd nests a dashed slice name: balanced-tests.slice sits in balanced.slice.
slice_max=/sys/fs/cgroup/user.slice/user-$UID.slice/user@$UID.service/balanced.slice/balanced-tests.slice/memory.max
if [[ ! -r $slice_max || $(<"$slice_max") == max ]]; then
  systemctl --user set-property --runtime balanced-tests.slice \
    MemoryMax="${BALANCED_TESTS_SLICE_MEMORY_MAX:-24G}" MemorySwapMax=0
fi
exec systemd-run --user --scope --quiet --collect --slice=balanced-tests \
  -p MemoryMax="${BALANCED_TEST_MEMORY_MAX:-8G}" -p MemorySwapMax=0 -- "$@"
