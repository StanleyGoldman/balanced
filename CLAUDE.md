## Agent skills

### Issue tracker

Issues are tracked in GitHub Issues on StanleyGoldman/balanced, using the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Uses the default labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.

## Working preferences

### Grilling

- Ask one question per round; two or three only when tightly coupled. This overrides the grilling skill's whole-frontier rounds.
- Number questions `Q<n>/<total>` across the ticket's whole grilling session: `n` is the question's position in that ticket, `total` your current estimate of the questions it needs. Revise `total` as answers come in, e.g. `Q3/5`, then `Q4/7`.
- When the user has no strong preference on a ticket, answer its remaining questions with your recommendations, then summarize the decisions and flag anything surprising, such as an answer that contradicts an earlier decision.

### Subagents

- Ask before starting research subagents, even when a skill (e.g. `wayfinder` charting) says to start them automatically.

### Prototypes

- Push every prototype to its own branch and open a **draft** PR for it, titled `PROTOTYPE: <what it prototypes>`.
- The PR description states what the prototype set out to answer, plus a running Q&A about what it showed.
- When the prototype session ends, close the PR unmerged and leave a pointer to it on the prototype's issue.

### Research

- Research ends with a pull request that commits the findings to `docs/research/<topic>.md`; the PR closes the research ticket (`Closes #<n>`), so the ticket stays open until the user merges. This overrides any skill (e.g. `wayfinder`) that leaves findings on a throwaway branch.

### Writing for agents

- Load the `writing-for-agents` skill before writing anything an agent reads: `CLAUDE.md`, `docs/agents/*`, skills, prompts.

### Self-improvement

- When you hit friction (a tool gotcha, a wrong assumption, a repeated workaround), record the lesson in the doc where the next agent will look, written with the `writing-for-agents` skill.
- Write scripts or skills whenever they help you get a task done; write skills with the `writing-for-agents` skill.

### Worktrees

- The main checkout in the repo folder belongs to the user; keep its branch as the user left it.
- Do branch work (practice and doc updates, fixes, PR branches) in a git worktree. A subagent in its own worktree makes such a fix in the background.

### Pull requests

- Implementation ends with a pull request: after the review fixes are committed, push the branch and open a PR that closes the ticket (`Closes #<n>`).
- Open pull requests and leave merging to a human; never merge one yourself.
- Stage by explicit path (`git add <paths>`) and check `git status` before committing: untracked output from another branch (e.g. build or prototype output ignored only on that branch) survives `git switch`.
- Whenever you push to a pull request, update its title and description to cover everything it now contains.

### Merge conflicts

- Invoke `/mattpocock-skills:resolving-merge-conflicts` the moment a merge or rebase reports a conflict, before opening any conflicted file.
