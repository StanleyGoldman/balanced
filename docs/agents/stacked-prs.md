# Stacked pull requests

A **stack** is a chain of open PRs, each based on the branch below it, with the bottom one based on `main`. GitHub shows the chain on every PR in it, and each PR's diff holds only its own commits, so the user reviews them one layer at a time. The `gh stack` extension (`github/gh-stack`) is installed; `gh stack <command> --help` is the full reference.

Stack a PR whenever its branch builds on another open PR's branch, including when you rebase one PR onto another to resolve their conflicts.

## Making a stack

1. Rebase each branch onto the one below it, bottom first, so every layer holds the tip of the layer under it.
2. From a worktree on any branch in the chain, adopt the branches, bottom to top:
   `gh stack init --base main <bottom-branch> … <top-branch>`
   This records local tracking only; it adopts existing branches and creates any that are missing.
3. Push and link: `gh stack submit --auto`. It pushes every branch, retargets each existing PR's base to the branch below, opens a PR for any branch without one (as a draft, since `--auto` skips the editor), and creates the stack on GitHub. It prints the stack number (`stack #<n>`).
4. Check the result with `gh stack view --short`, and update each PR's description to say which PR it sits on.

To link PRs that already have the right bases, without local tracking: `gh stack link <bottom-pr> … <top-pr>`.

## Keeping it current

- When `main` moves, run `gh stack rebase` for the cascading rebase, then `gh stack push`. On a conflict it pauses: resolve it as the Merge conflicts section of `CLAUDE.md` says, then `gh stack rebase --continue`.
- `gh stack sync` and `gh stack rebase` pull trunk into the local `main` branch, which the user's main checkout holds. Ask the user before running either.
- Stack tracking lives in the clone's common git directory, so every worktree sees the same stacks.

## Merging

Merging stays with the user, so `gh stack merge` is theirs to run. They merge bottom first; when a layer merges, GitHub retargets the next PR to `main`.

A `Closes #<n>` in a PR above the bottom links nothing until that PR targets `main` (see `issue-tracker.md`).
