#!/usr/bin/env bash
# Adds a table of sccache's hit rate per step to the job summary, from the rows
# .github/in-image.sh wrote to $RUNNER_TEMP/sccache-rows.md. Each workflow job
# runs it as its last step, pass or fail. Steps that compiled nothing have no
# row, and a job with no rows gets no table.
#
# Usage: .github/sccache-summary.sh   (no arguments; reads RUNNER_TEMP and
#                                      GITHUB_STEP_SUMMARY)
# Exit status: 0.
set -euo pipefail

rows=$RUNNER_TEMP/sccache-rows.md
[ -s "$rows" ] || exit 0
{
  echo "### ⚡ sccache"
  echo
  echo "| Step | Hits | Misses | Hit rate | Not cacheable |"
  echo "|---|--:|--:|--:|--:|"
  cat "$rows"
} >> "$GITHUB_STEP_SUMMARY"
