#!/usr/bin/env bash
# Runs this worktree's xtask against another commit's tree: a clean checkout
# detached at <commit>, with the committed xtask from HEAD laid over it. Use it
# to dry-run a CI task that reads HEAD and refuses a dirty tree
# (`mutants-shard`, `mutants-landed`) on a real commit of `main` before the
# task's own change has landed.
#
# Usage: xtask-at-commit.sh <commit> <task> [<arg>...]
#   <commit>  any revision, e.g. a pull request's merge commit on main.
#   <task>    the xtask task and its arguments, e.g. mutants-landed --dry-run.
#
# Only committed work is laid over, so commit first. The overlay is `xtask/`
# and `Cargo.lock` from HEAD, hidden from `git status` in the checkout, so the
# task sees a clean tree whose HEAD is <commit>. The task runs as
# `cargo run -p xtask --`, since an old commit may predate the `cargo xtask`
# alias. A task that talks to GitHub needs GITHUB_TOKEN in the environment,
# e.g. `GITHUB_TOKEN=$(gh auth token) xtask-at-commit.sh ...`.
#
# The checkout is .claude/worktrees/at-<short sha> beside the other worktrees.
# It stays after the run, so its `target/` (mutants.out, logs) can be read; a
# later run at the same commit replaces it.
#
# Output: one JSON line on stdout:
#   {"status": "green"|"red"|"error", "commit": sha, "checkout": path,
#    "log": path, "tail": [string]?, "next": string?}
#   green: the task exited 0. "log" holds everything it printed.
#   red:   the task failed. "tail" holds the last 15 lines of the log.
#   error: the checkout couldn't be made (bad arguments, unknown commit, git
#          failed). "next" says why.
# Exit codes: 0 green, 1 red, 2 error.
set -euo pipefail

fail() {
  jq -nc --arg next "$1" '{status: "error", next: $next}'
  exit 2
}

[[ $# -ge 2 ]] || fail "usage: xtask-at-commit.sh <commit> <task> [<arg>...]"
revision=$1
shift
source=$(git rev-parse --show-toplevel 2>/dev/null) || fail "run this from inside the repository"
commit=$(git rev-parse --verify --quiet "$revision^{commit}") || fail "no commit $revision"
common=$(git rev-parse --path-format=absolute --git-common-dir)
checkout=$(dirname "$common")/.claude/worktrees/at-${commit:0:12}
exclude=$common/info/exclude
marker="# xtask-at-commit $commit"

if [[ -e $checkout ]]; then
  git worktree remove --force "$checkout" >/dev/null 2>&1 || fail "removing the old checkout $checkout failed"
fi
git worktree add -q --detach "$checkout" "$commit" >/dev/null 2>&1 || fail "git worktree add at $commit failed"
git -C "$source" archive HEAD xtask Cargo.lock | tar -x -C "$checkout" || fail "laying HEAD's xtask over $checkout failed"

# New files are hidden through the repository's shared exclude file, between
# markers removed when the run ends; changed tracked files through skip-worktree,
# which lives in the checkout's own index.
mkdir -p "$(dirname "$exclude")"
trap 'sed -i "\|^$marker begin\$|,\|^$marker end\$|d" "$exclude"' EXIT
{
  echo "$marker begin"
  git -C "$checkout" ls-files --others --exclude-standard | sed 's|^|/|'
  echo "$marker end"
} >>"$exclude"
git -C "$checkout" ls-files -m -z | xargs -0 -r git -C "$checkout" update-index --skip-worktree
[[ -z $(git -C "$checkout" status --porcelain) ]] || fail "the overlay still shows in git status in $checkout"

mkdir -p "$checkout/target"
log=$checkout/target/xtask-at-commit.log
if (cd "$checkout" && cargo run -q -p xtask -- "$@") >"$log" 2>&1; then
  jq -nc --arg commit "$commit" --arg checkout "$checkout" --arg log "$log" \
    '{status: "green", commit: $commit, checkout: $checkout, log: $log}'
  exit 0
fi
jq -nc --arg commit "$commit" --arg checkout "$checkout" --arg log "$log" \
  --rawfile tail <(tail -15 "$log" | sed 's/\x1b\[[0-9;]*m//g') \
  '{status: "red", commit: $commit, checkout: $checkout, log: $log,
    tail: ($tail | rtrimstr("\n") | split("\n"))}'
exit 1
