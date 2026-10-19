#!/usr/bin/env bash
# Deletes the `target/` build output of every worktree under .claude/worktrees
# whose branch's pull request has merged or closed. Each worktree builds into
# its own `target/`, 18-84 GB once its tests have run, and a merged worktree
# keeps it until someone deletes it; on 2026-10-19 they filled the disk.
#
# Usage: clean-merged-targets.sh [--dry-run]
#   --dry-run  report what would be deleted, deleting nothing.
#
# Only `target/` goes: the worktree, its branch and its files stay. A worktree
# on a detached HEAD, on a branch with no pull request, or with an open one is
# skipped. Run it from any checkout of the repository; `gh` must be logged in.
#
# Output: one JSON line on stdout:
#   {"status": "cleaned"|"error", "cleaned": [path], "skipped": [path],
#    "freed_kb": number, "next": string?}
#   cleaned: every merged or closed worktree's `target/` was deleted (or, with
#            --dry-run, would be); "freed_kb" is their total size. "skipped"
#            lists the worktrees with a `target/` left in place.
#   error:   bad arguments, or the repository or `gh` failed. "next" says why.
# Exit codes: 0 cleaned, 2 error.
set -euo pipefail

fail() {
  jq -nc --arg next "$1" '{status: "error", next: $next}'
  exit 2
}

dry_run=false
case "${1-}" in
  "") ;;
  --dry-run) dry_run=true ;;
  *) fail "usage: clean-merged-targets.sh [--dry-run]" ;;
esac

common=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null) ||
  fail "not inside a checkout of the repository"
worktrees="$(dirname "$common")/.claude/worktrees"

cleaned=()
skipped=()
freed=0
for dir in "$worktrees"/*/; do
  dir=${dir%/}
  [[ -d "$dir/target" ]] || continue
  branch=$(git -C "$dir" branch --show-current)
  state=""
  if [[ -n "$branch" ]]; then
    state=$(gh pr list --head "$branch" --state all --json state --jq '.[0].state') ||
      fail "gh pr list failed for $branch; check \`gh auth status\`"
  fi
  if [[ "$state" != MERGED && "$state" != CLOSED ]]; then
    skipped+=("$dir")
    continue
  fi
  size=$(du -sk "$dir/target" | cut -f1)
  $dry_run || rm -rf "$dir/target"
  freed=$((freed + size))
  cleaned+=("$dir")
done

jq -nc --argjson freed "$freed" \
  --args '{status: "cleaned", freed_kb: $freed,
    cleaned: [$ARGS.positional[] | select(startswith("C:")) | .[2:]],
    skipped: [$ARGS.positional[] | select(startswith("S:")) | .[2:]]}' \
  "${cleaned[@]/#/C:}" "${skipped[@]/#/S:}"
