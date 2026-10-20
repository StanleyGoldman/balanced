# CI runs on GitHub Actions

CI runs on GitHub Actions' hosted Linux runners, which are free for the repository now that it is public. `.github/workflows/ci.yml` runs the checks of `cargo xtask ci` on every pull request and every push to `main`. `.github/workflows/mutants.yml` runs beside it: `mutants-landed` on each push to `main`, and `mutants-shard` nightly (ADR 0002). Every step runs one xtask task in the CI image (ADR 0004), and check logic stays in xtask. Caching follows ADR 0003.

Decided in the licensing grilling recorded in ADR 0009 (Q11). Built by [#300](https://github.com/StanleyGoldman/balanced/issues/300).

## Consequences

- The workflow runs each task of `cargo xtask ci` as a step of its own, over parallel jobs, so the Actions page names the failing check and `clippy` and `test` don't wait on each other. `cargo xtask ci` stays the one command to run them all locally, and a test in `xtask/src/ci.rs` fails when the workflow and the task run different checks.
- The required status check on `main` is the job `ci`, which passes when every other job passed or was skipped. A prose-only change skips every check but `typos` and `adrs`, so the check still reports.
- On a pull request, Actions checks out GitHub's merge of the branch into its target. `cargo xtask ci`'s own `adrs` therefore checks the merged tree, where two pull requests that each add a record with the same number clash.
- The mutant runs' concurrency group keeps only the newest waiting run. When a merge lands while two runs are queued, the older queued `mutants-landed` run is cancelled. The weekly round of shards still tests those mutants.
- The mutant runs use the workflow's own `GITHUB_TOKEN`, scoped by the workflow to issues write and pull requests read, so no personal token is stored.
