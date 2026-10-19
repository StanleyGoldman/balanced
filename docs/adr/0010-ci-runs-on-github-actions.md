# CI runs on GitHub Actions

CI moved from the self-hosted Woodpecker agents to GitHub Actions' hosted runners once the repository went public, which made the runners free for it. `.github/workflows/ci.yml` runs `cargo xtask ci` on every pull request and every push to `main`. `.github/workflows/mutants.yml` runs beside it: `mutants-landed` on each push to `main`, and `mutants-shard` nightly. Every step still runs one xtask task, and check logic stays in xtask. The decisions of ADRs 0002, 0003 and 0004 carry over; what changes is only how a hosted runner provides each one.

Decided in the licensing grilling recorded in ADR 0009 (Q11). Built by [#300](https://github.com/StanleyGoldman/balanced/issues/300).

## How each earlier decision maps

- **The CI image (ADR 0004).** Still `.github/Dockerfile` on `ubuntu:26.04`, so the snapshots draw with the snapshot machine's Mesa, not the runner's. A hosted runner has no daemon that persists between runs, so each job builds the image with buildx and keeps its layers in the GitHub Actions cache. There is still no registry. Steps run in it through `.github/in-image.sh`, which also runs it by hand.
- **Caching (ADR 0003).** `target/` still starts empty on every run. sccache keeps the compiler outputs in the GitHub Actions cache (its `ghac` backend), not a directory the agents share. The cargo registry is an `actions/cache` entry keyed on `Cargo.lock`. The one cache is 10 GB per repository, shared by sccache, the image layers and the registry, and evicted least recently used first.
- **Mutant runs (ADR 0002).** The run no longer waits for `ci` on the same commit, as Woodpecker's `depends_on` made it. It starts beside `ci`, on the push or the nightly schedule, and cargo-mutants runs its baseline instead: the unmutated tests of the crates in scope, which fail the run before any issue changes. That keeps a broken `main` from reading every mutant as caught, which is what the wait was for, and a merge's mutant run no longer waits on clippy and the image snapshots. A concurrency group of one replaces the lane agent of capacity one. The 8 GB memory cap, with no swap, is now a `docker run --memory` limit on the container, not the agent's. The token is the workflow's own `GITHUB_TOKEN`, scoped by the workflow to issues write and pull requests read, not a stored personal token.

## Consequences

- A concurrency group keeps only the newest waiting run. When a merge lands while two mutant runs are queued, the older queued `mutants-landed` run is cancelled. The weekly round of shards still tests those mutants.
- The workflow runs each task of `cargo xtask ci` as a step of its own, over parallel jobs, so the Actions page names the failing check and `clippy` and `test` don't wait on each other. `cargo xtask ci` stays the one command to run them all locally, and a test in `xtask/src/ci.rs` fails when the workflow and the task run different checks.
- The required status check on `main` is the job `ci`, which passes when every other job passed or was skipped. A prose-only change skips every check but `typos` and `adrs`, so the check still reports.
- On a pull request, Actions checks out GitHub's merge of the branch into its target. `cargo xtask ci`'s own `adrs` therefore checks the merged tree, and no separate `adrs --against` step is needed.
