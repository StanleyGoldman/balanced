# Issue tracker: GitHub

Issues and specs for this repo live as GitHub issues. Use the `gh` CLI for all operations.

## Conventions

- **Create an issue**: `gh issue create --title "..." --body "..."`. Use a heredoc for multi-line bodies.
- **Read an issue**: `gh issue view <number> --json number,title,state,body,labels,comments --jq '{number,title,state,labels:[.labels[].name],body,comments:[.comments[]|{author:.author.login,body}]}'`. Outside a terminal, plain `gh issue view <number> --comments` prints only the comments, so an issue with none comes back empty.
- **List issues**: `gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'` with appropriate `--label` and `--state` filters.
- **Comment on an issue**: `gh issue comment <number> --body "..."`
- **Apply / remove labels**: `gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **Close**: `gh issue close <number> --comment "..."`
- **Edit a PR's title, body or base** through the REST API: `gh api -X PATCH repos/StanleyGoldman/balanced/pulls/<n> -F body=@<file>` (or `-f title="..."`, `-f base=<branch>`). The installed `gh` 2.46 reads Projects (classic) in `gh pr edit`, so `gh pr edit --body-file` fails with a GraphQL deprecation error naming `projectCards` and changes nothing. Check the PR afterwards when a `gh pr edit` prints that error.

`gh` infers the repo from `git remote -v` only inside a clone. From any other directory (the scratchpad, where issue bodies are written), pass `--repo StanleyGoldman/balanced` to `gh issue` and `gh pr`; without it they fail with "not a git repository". `gh api` takes the repo in its path instead.

## When git's SSH remote is refused

`origin` is an SSH URL. A shell with no SSH agent gets `Permission denied (publickey)` on `git fetch` and `git push`, while `gh` still works on its own token. Run the git command over HTTPS with `gh` as the credential helper, naming the URL in place of `origin`:

```sh
git -c credential.helper='!gh auth git-credential' fetch https://github.com/StanleyGoldman/balanced.git main:refs/remotes/origin/main
git -c credential.helper='!gh auth git-credential' push -u https://github.com/StanleyGoldman/balanced.git <branch>
```

The fetch's refspec updates `origin/main`, so `git worktree add ... origin/main` branches from the fresh tip. Leave `origin` and the global git config as they are; the user's own checkout uses SSH.

## Linking a pull request to its issue

Every PR that comes from an issue opens its body with `Closes #<n>`, and that includes prototype PRs. The keyword is what puts the PR in the issue's **Development** panel. A prototype PR is closed unmerged, so its keyword only links; the resolution comment closes the issue.

- **Keywords fire in prose too.** GitHub closes on merge every issue that follows close, closes, closed, fix, fixes, fixed, resolve, resolves or resolved anywhere in the body, so "two PRs closed #320" in a lesson PR closed #320. Refer to other issues with a verb that isn't a keyword ("#344 and #348 targeted #320"), and check `closingIssuesReferences` (below) lists only the PR's own issue.
- **When GitHub links:** only while the PR is open and targets the default branch (`main`). A keyword added after the PR closed, or on a PR stacked on another branch, links nothing until those hold. The link survives the PR closing unmerged.
- **Check a link** from the PR side; this also works for closed PRs:

  ```sh
  gh api graphql -f query='{repository(owner:"StanleyGoldman",name:"balanced"){pullRequest(number:<pr>){closingIssuesReferences(first:10){nodes{number}}}}}'
  ```

- **Fix a closed PR that lacks the link:** add the keyword to the body, `gh pr reopen <pr>`, wait a few seconds for GitHub to link it, check, then `gh pr close <pr>`.
- **No API writes the link.** GitHub's public GraphQL has no mutation for it; a link comes only from a body keyword or by hand in the issue's Development panel.

## Pull requests as a triage surface

**PRs as a request surface: no.** _(Set to `yes` if this repo treats external PRs as feature requests; `/triage` reads this flag.)_

When set to `yes`, PRs run through the same labels and states as issues, using the `gh pr` equivalents:

- **Read a PR**: `gh pr view <number> --comments` and `gh pr diff <number>` for the diff.
- **List external PRs for triage**: `gh pr list --state open --json number,title,body,labels,author,authorAssociation,comments` then keep only `authorAssociation` of `CONTRIBUTOR`, `FIRST_TIME_CONTRIBUTOR`, or `NONE` (drop `OWNER`/`MEMBER`/`COLLABORATOR`).
- **Comment / label / close**: `gh pr comment`, `gh pr edit --add-label`/`--remove-label`, `gh pr close`.

GitHub shares one number space across issues and PRs, so a bare `#42` may be either: resolve with `gh pr view 42` and fall back to `gh issue view 42`.

## When a skill says "publish to the issue tracker"

Create a GitHub issue.

## When a skill says "fetch the relevant ticket"

Read it with the **Read an issue** command above.

## Wayfinding operations

Used by `/wayfinder`. The **map** is a single issue with **child** issues as tickets.

- **Map**: a single issue labelled `wayfinder:map`, holding the Notes / Decisions-so-far / Fog body. `gh issue create --label wayfinder:map`.
- **Child ticket**: an issue linked to the map as a GitHub sub-issue (`gh api` on the sub-issues endpoint). Where sub-issues aren't enabled, add the child to a task list in the map body and put `Part of #<map>` at the top of the child body. Labels: `wayfinder:<type>` (`research`/`prototype`/`grilling`/`task`). Once claimed, the ticket is assigned to the driving dev.
- **Blocking**: GitHub's **native issue dependencies**, the canonical, UI-visible representation. Add an edge with `gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>`, where `<blocker-db-id>` is the blocker's numeric **database id** (`gh api repos/<owner>/<repo>/issues/<n> --jq .id`, _not_ the `#number` or `node_id`). GitHub reports `issue_dependencies_summary.blocked_by` (open blockers only, the live gate). Where dependencies aren't available, fall back to a `Blocked by: #<n>, #<n>` line at the top of the child body. A ticket is unblocked when every blocker is closed.
- **Frontier query**: list the map's open children (`gh issue list --state open`, scoped to the map's sub-issues / task list), drop any with an open blocker (`issue_dependencies_summary.blocked_by > 0`, or an open issue in the `Blocked by` line) or an assignee, or an open PR that closes it; first in map order wins. A research ticket stays open, unassigned, until its findings PR merges, so the PR is what marks it done. Find it with `gh pr list --state open --search "<n>"`, then confirm with the `closingIssuesReferences` query above.
- **Claim**: `gh issue edit <n> --add-assignee @me`, the session's first write.
- **Resolve**: `gh issue comment <n> --body "<answer>"`, then `gh issue close <n>`, then append a context pointer (gist + link) to the map's Decisions-so-far.
- **Backfill pointers on load**: a research ticket closes when its findings PR merges, after its session has ended, so no session appends its pointer. On loading a map, compare its closed sub-issues with Decisions-so-far and append a pointer for each one missing, linking the ticket and its merged PR.
