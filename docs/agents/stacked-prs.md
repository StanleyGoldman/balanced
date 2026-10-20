# Stacked pull requests

A **stack** is a chain of open PRs, each based on the branch below it, with the bottom one based on `main`. GitHub shows the chain on every PR in it, and each PR's diff holds only its own commits, so the user reviews them one layer at a time. The `gh stack` extension (`github/gh-stack`) is installed; `gh stack <command> --help` is the full reference.

Stack a PR whenever its branch builds on another open PR's branch, including when you rebase one PR onto another to resolve their conflicts. The exception is a prototype PR: a stack exists for merging, and a prototype PR closes unmerged, so rebase it and set its PR's base to the branch below, without `gh stack` (`docs/agents/prototypes.md`).

Offer a stack when you foresee two PRs colliding: merged one at a time, the second would conflict with the first. Check before opening a PR whose files another open PR also touches, and when the user asks about several open PRs: `git merge-tree --write-tree --name-only --no-messages origin/<branch-a> origin/<branch-b>` exits 1 and lists the conflicted files when they collide, 0 when they merge clean. Name the PRs and files in the offer; on a yes, rebase the upper branch onto the lower one and make the stack.

## Making a stack

1. Bring each branch onto the one below it, bottom first, so every layer holds the tip of the layer under it. Rebase a branch whose commits apply cleanly. Merge the lower branch in when several of the branch's commits touch the conflicted hunks: a rebase stops on each such commit to resolve the same hunks again, and a merge resolves them once.
2. From a worktree on any branch in the chain, adopt the branches, bottom to top:
   `gh stack init --base main <bottom-branch> … <top-branch>`
   This records local tracking only; it adopts existing branches and creates any that are missing.
3. Push and link: `gh stack submit --auto`. It pushes every branch, retargets each existing PR's base to the branch below, opens a PR for any branch without one (as a draft, since `--auto` skips the editor), and creates the stack on GitHub. It prints the stack number (`stack #<n>`).
4. Check the result with `gh stack view --short`, and update each PR's description to say which PR it sits on.
5. `gh stack submit` turns off auto-merge on any PR in the stack that had it, since GitHub allows no auto-merge on a stacked PR, and says so only in a `⚠` line of its output. Tell the user which PRs lost it.

To link PRs that already have the right bases, without local tracking: `gh stack link <bottom-pr> … <top-pr>`.

## Joining stacks

A PR sits in at most one stack, and while it does GitHub refuses to change its base (`Cannot change the base branch because the pull request is part of a stack`). To move PRs from stack A onto the top of stack B:

1. `gh stack unstack <A>`. It can warn that some PRs stay stacked even when none has auto-merge; the link below settles it.
2. Point the lowest moved PR at B's top branch: `gh pr edit <n> --base <branch>`.
3. `gh stack link <B> <pr> … <pr>`, bottom to top. It appends them to B, and rejects any PR still in another stack.

## Keeping it current

- When `main` moves, run `gh stack rebase` for the cascading rebase, then `gh stack push`. On a conflict it pauses: resolve it as the Merge conflicts section of `CLAUDE.md` says, then `gh stack rebase --continue`.
- `gh stack sync` and `gh stack rebase` pull trunk into the local `main` branch, which the user's main checkout holds. Ask the user before running either.
- Stack tracking lives in the clone's common git directory, so every worktree sees the same stacks.

## Merging

Merging stays with the user, so `gh stack merge` is theirs to run. They merge bottom first; when a layer merges, GitHub retargets the next PR to `main`.

A `Closes #<n>` in a PR above the bottom links nothing until that PR targets `main` (see `issue-tracker.md`).
