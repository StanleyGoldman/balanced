## Code

Read `CODING_STANDARDS.md` before writing or reviewing Rust code. It lists the cross-cutting rules and points to the practice doc for each area. `ARCHITECTURE.md` maps the crates and their layer rules.

## Agent skills

### Issue tracker

Issues are tracked in GitHub Issues on StanleyGoldman/balanced, using the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Uses the default labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.

### CI

CI is GitHub Actions (`.github/workflows/`), read and driven with `gh run` and `gh workflow`. `docs/practices/tooling.md` covers how it runs and how to reproduce a failure locally.

## Working preferences

### Grilling

- Ask one question per round; two or three only when tightly coupled. This overrides the grilling skill's whole-frontier rounds.
- Number questions `Q<n>/<total>` across the ticket's whole grilling session: `n` is the question's position in that ticket, `total` your current estimate of the questions it needs. Revise `total` as answers come in, e.g. `Q3/5`, then `Q4/7`.
- When the user has no strong preference on a ticket, answer its remaining questions with your recommendations, then summarize the decisions and flag anything surprising, such as an answer that contradicts an earlier decision.
- Tag each recommendation with its confidence, so the user knows where to spend attention:
  - `firm`: a documented fact settles it (ADR, `GLOSSARY.md`, existing code). Cite the source.
  - `leaning`: a judgment call. Give the one-line reason.
  - `coin-flip`: a real tradeoff that needs the user's input.
- Tag each question `cheap` or `expensive` to reverse. A public API, a data format, or an ADR-level choice is `expensive`.
- Settle `firm` questions yourself. List them at the end under "Assumed, shout if wrong" and ask only the rest.
- Batch `cheap` questions tagged `firm` or `leaning` into one round. The user replies "accept all" or names the exceptions. This overrides the one-question-per-round rule for these questions.
- Post the resolution comment in the shape given in `docs/agents/grilling.md`.

### Subagents

- Ask before starting research subagents, even when a skill (e.g. `wayfinder` charting) says to start them automatically.

### Wayfinder

- When charting, recommend a destination of decisions or a spec. The cleared map then hands off to `/to-spec` → `/to-tickets` → `/implement`, which does the building.
- When working a map, check its Destination and Notes against these preferences before taking a ticket. Raise any conflict with the user (e.g. a map whose Notes put execution in scope) and settle it before acting; a map charted before a preference changed keeps the old rule until someone redraws it.

### Prototypes

- Read `docs/agents/prototypes.md` before planning, building, running or closing a prototype.

### Research

- Research ends with a pull request that commits the findings to `docs/research/<topic>.md`; the PR closes the research ticket (`Closes #<n>`), so the ticket stays open until the user merges. This overrides any skill (e.g. `wayfinder`) that leaves findings on a throwaway branch.
- Look up crate versions and metadata with `cargo info` or `cargo search`, not raw crates.io API calls.

### Writing for agents

- Load the `writing-for-agents` skill before writing anything an agent reads: `CLAUDE.md`, `docs/agents/*`, skills, prompts.

### Self-improvement

- When you hit friction (a tool gotcha, a wrong assumption, a repeated workaround), record the lesson in the doc where the next agent will look, written with the `writing-for-agents` skill.
- Make each lesson fix straight away, in its own worktree and PR, and report what you changed with the PR link; the user weighs it in the PR review. A lesson exists for the user once it is in a PR, so file it rather than mention it.
- Read a sign-off message ("leave notes, I'll pick this up later") as the session's last word: finish the work it asks for, file any lessons as PRs, and end on what was done.
- Reach for a skill when repeated work needs judgement; write skills with the `writing-for-agents` skill. When it needs only commands, write a script (below).

### Agent scripts

- **When:** script a check, repair or lookup you have run more than once, or will clearly run again, then use the script from then on and name it in the doc that calls for the task. Script work you have seen recur; a failure that hasn't happened yet earns a line in a doc, not a script.
- **Where:** `.claude/scripts/`, beside the project's other agent tooling (`.claude/settings.json`, `.claude/worktrees/`). These scripts are for the agent working the project. A top-level `scripts/` would read as part of the app, or as something the user runs.
- **Audience:** only an agent ever calls or reads them. Shape each one for that caller and nothing else:
  - the contract (arguments, output, statuses, exit codes) in the header comment, so a later session learns it without reading the code;
  - one JSON result line on stdout, built with `jq -n` (installed) and read back with `jq`;
  - a `next` field naming the follow-up whenever the script can't finish.

### Worktrees

- The main checkout in the repo folder belongs to the user; keep its branch as the user left it.
- Do branch work (practice and doc updates, fixes, PR branches) in a git worktree under `.claude/worktrees/<name>` (gitignored), never beside the repo folder. A subagent in its own worktree makes such a fix in the background.
- In a worktree-isolated subagent, run each shell command as one plain call with literal paths. The isolation guard refuses any command it cannot prove keeps git inside the worktree, and backgrounding, loops, shell variables, and `sh <script>` all trip it, even for `curl` or `gh` downloads. Inline text holding backticks or `$` trips it too, so write a PR or issue body to a scratchpad file and pass `--body-file <path>`, and a commit message with such characters via `git commit -F <path>`.

### Implementation

- Assign the ticket to the user as an `/implement` run starts (`gh issue edit <n> --add-assignee @me`), so the tracker shows it is being worked.
- Before branching, look for an earlier run on the ticket: `gh pr list --search "<n> in:body" --state open` and `git worktree list`. If one exists, resume its worktree and PR. A run can stop mid-merge, with conflicts resolved but not yet staged. Then run `ListAgents`: a live peer session named for the ticket (`implement 318`) is building it, with no PR or pushed branch yet, so leave the ticket to that session and tell the user.
- Start an `/implement` run with `git fetch origin main` and branch the work from `origin/main`: a worktree can predate merged blockers, so their code looks missing.
- Then check that everything the ticket reuses (a field, a clock, a Dialog) is already built. When one is still an open ticket that isn't among its blockers, ask the user whether to add it as a blocker or build an interim stand-in.
- A ticket naming quicken_clone code as its reference means `StanleyGoldman/quicken_clone`, an archived private repo with no local checkout. Read a file with `gh api repos/StanleyGoldman/quicken_clone/contents/<path> -H 'Accept: application/vnd.github.raw'`. It used clap and other crates Balanced lacks, so port its behaviour and tests into the local idiom.
- Load the `tdd` skill before writing the first test or line of code of an `/implement` run, seams agreed or not; the `implement` skill calls it only "where possible, at pre-agreed seams".
- Run each slice red before green. A test that has only failed to compile, or was written after its code, has never been red: break the code it covers, watch the test fail, then restore the code. Do the break with `.claude/scripts/red-check.sh <file> <old> <new> <test command...>`, which restores the file byte for byte (contract in its header).
- Restore broken code by editing it back, so its mtime moves forward. A file moved back from a backup copy keeps the copy's older mtime, so cargo skips the rebuild and the rerun still tests the broken build.
- A red counts only when the failing assertion is the behaviour under test. Read the panic in the `cargo nextest run` output; a setup assertion failing, or a `get_by_role_and_label` that finds no node, proves nothing.
- Break the code so the data stays valid: send the row to the wrong account, or change a copied field. A break that skips writing a linked row (a transfer's other side) fails SQLite's foreign keys at commit, so the Command's error panics in `apply` before the test's assertion runs.
- A test that removed behaviour stays gone (Ctrl+G no longer goes to the other side) has no code left to break. Take it red on the code from before the change: copy the changed source aside, `git restore -s <merge-base> -W <paths>`, run the test, then `cp` your source back (plain `cp`, so mtimes move forward). `git restore -W` leaves the index alone, where `git checkout <ref> -- <paths>` stages the old files. Use the merge-base, since another session's fetch can move `origin/main` past it.

### Pull requests

- Implementation ends with a pull request: after the review fixes are committed, push the branch and open a PR that closes the ticket (`Closes #<n>`). This extends the `implement` skill, which stops at the commit; it is user-invoked, so it never appears in your skill list.
- Unassign the ticket (`gh issue edit <n> --remove-assignee @me`) once its PR is ready for review: when you open it as ready, or when you mark a draft ready. The open PR now shows the ticket's state, and an empty assignee says the next move is the user's review.
- Open pull requests and leave merging to a human; never merge one yourself.
- Stage by explicit path (`git add <paths>`) and check `git status` before committing: untracked output from another branch (e.g. build or prototype output ignored only on that branch) survives `git switch`.
- Leave remote branches to GitHub, which deletes a PR's head branch when it merges; cleanup after a merge covers only local branches and worktrees.
- Each worktree's `target/` grows to tens of GB and outlives the merge. Run `.claude/scripts/clean-merged-targets.sh` at the start of an `/implement` run, and whenever a build fails with `No space left on device`; it deletes `target/` only for worktrees whose PR merged or closed (contract in its header).
- Before pushing to an existing PR's branch, check the PR is still open (`gh pr view <n> --json state`); once it has merged, put follow-ups on a fresh branch from `origin/main` and open a new PR.
- Whenever you push to a pull request, update its title and description to cover everything it now contains.
- After each push, read `gh pr view <n> --json mergeable` once (`UNKNOWN` for a few seconds after a push, so ask again) and resolve a `CONFLICTING` (see **Merge conflicts** below) before reporting the PR ready. `cargo xtask ci` already stands in for the PR's checks.
- **Stack** a PR whose branch builds on another open PR's branch, with `gh stack`, and offer a stack when you foresee two PRs conflicting if merged one at a time. See `docs/agents/stacked-prs.md`.

### Merge conflicts

When a merge or rebase stops on a conflict, resolve it as two intents, not two blocks of text:

1. **Read each side's intent before editing a hunk.** For each conflicted file, read `git diff <merge-base> <side> -- <file>` for both sides, and find the commit, PR or ticket behind each change (`git log <merge-base>..<side> -- <file>`, then `gh pr view`).
2. **Keep both intents where they fit together.** Where they clash, keep the side that matches the merge's goal, and name what the other side loses in the merge commit message. A side that has already merged into `main` usually describes the current state, so text it made stale on your branch goes.
3. **Check the parts that merged cleanly too.** Two branches can add the same thing in different places, such as two entries for one crate in a list, with no conflict marker. Search the merged files for each name both sides added.
4. **Run `cargo xtask ci` before committing.** A test that merged cleanly can still assert the old shape, such as a full `preferences.toml` text or a harness step the other side added.
