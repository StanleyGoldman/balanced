#!/usr/bin/env bash
# Posts the coverage report as a comment on a pull request, first deleting the
# comments earlier pushes left, so the thread holds one report: the newest.
#
# Usage: .github/post-coverage-comment.sh <pr-number> <report.md>
# Environment: GH_TOKEN (pull requests: write), GITHUB_REPOSITORY.
#
# The comments it deletes are the ones whose body contains the marker line the
# report opens with (MARKER in xtask/src/coverage/report.rs) and that GitHub
# Actions' bot wrote, so nobody else's comment is touched.
#
# Exit status: 0 once the new comment is posted; the failing command's otherwise.
set -euo pipefail

pr=$1
report=$2
marker='<!-- balanced-coverage-report -->'

gh api --paginate "repos/$GITHUB_REPOSITORY/issues/$pr/comments" \
  --jq ".[] | select(.user.login == \"github-actions[bot]\" and (.body | contains(\"$marker\"))) | .id" |
  while IFS= read -r id; do
    gh api -X DELETE "repos/$GITHUB_REPOSITORY/issues/comments/$id"
  done

gh api "repos/$GITHUB_REPOSITORY/issues/$pr/comments" -F "body=@$report" > /dev/null
