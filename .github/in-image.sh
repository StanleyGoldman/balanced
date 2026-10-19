#!/usr/bin/env bash
# Runs one command in the CI image, with the checkout mounted at /work, the way
# every workflow step under .github/workflows/ runs its xtask task.
#
# Usage: .github/in-image.sh <command> [<arg>...]
#   e.g. .github/in-image.sh cargo xtask ci
#
# Run it from the repository root. The image must already exist as
# `balanced-ci:latest`: the workflows build it from .github/Dockerfile, and by
# hand `docker build -t balanced-ci:latest -f .github/Dockerfile .` does.
#
# Environment, all optional:
#   CI_CARGO_HOME   a host directory mounted as the container's CARGO_HOME, so
#                   the cargo registry outlives the container (ADR 0003).
#   CI_MEMORY       a memory cap, such as 8g, with no swap beyond it, so a
#                   mutant that allocates without end is killed (ADR 0002).
#   The variables in `passed` below reach the container when set here.
#
# With RUSTC_WRAPPER=sccache, the command runs under a trap that prints
# sccache's hit rate when it ends, pass or fail, and saves it as JSON in
# target/sccache-stats.json. The server lives in the container, so a later
# step could no longer ask it. In a workflow step (RUNNER_TEMP set), the
# script then adds the step's hit rate as a row to $RUNNER_TEMP/sccache-rows.md,
# which .github/sccache-summary.sh turns into a table in the job summary.
#
# Exit status: the command's.
set -euo pipefail

passed=(
  CARGO_TERM_COLOR CARGO_INCREMENTAL GITHUB_TOKEN CI_RUN_URL
  RUSTC_WRAPPER SCCACHE_GHA_ENABLED SCCACHE_CACHE_SIZE SCCACHE_IDLE_TIMEOUT
  ACTIONS_CACHE_URL ACTIONS_RESULTS_URL ACTIONS_RUNTIME_TOKEN ACTIONS_CACHE_SERVICE_V2
)

args=(--rm -v "$PWD:/work" -w /work)
for name in "${passed[@]}"; do
  args+=(-e "$name")
done
if [ -n "${CI_CARGO_HOME:-}" ]; then
  mkdir -p "$CI_CARGO_HOME"
  args+=(-v "$CI_CARGO_HOME:/cache/cargo" -e CARGO_HOME=/cache/cargo)
fi
if [ -n "${CI_MEMORY:-}" ]; then
  args+=(--memory "$CI_MEMORY" --memory-swap "$CI_MEMORY")
fi

if [ "${RUSTC_WRAPPER:-}" != sccache ]; then
  exec docker run "${args[@]}" balanced-ci:latest "$@"
fi

# Removed and written inside the container, whose root owns `target/`.
stats=target/sccache-stats.json
status=0
docker run "${args[@]}" balanced-ci:latest sh -c '
  rm -f '"$stats"'
  trap "sccache --show-stats || true
    mkdir -p target && sccache --show-stats --stats-format=json > '"$stats"' || true" EXIT
  "$@"' sh "$@" || status=$?

if [ -n "${RUNNER_TEMP:-}" ] && [ -s "$stats" ]; then
  jq -r --arg command "$*" '
    .stats as $s
    | ([$s.cache_hits.counts[]] | add // 0) as $hits
    | ([$s.cache_misses.counts[]] | add // 0) as $misses
    | if $hits + $misses == 0 then empty else
        "| `\($command)` | \($hits) | \($misses)"
        + " | \($hits * 100 / ($hits + $misses) | floor)%"
        + " | \($s.requests_not_cacheable) |"
      end' "$stats" >> "$RUNNER_TEMP/sccache-rows.md" || true
fi
exit "$status"
