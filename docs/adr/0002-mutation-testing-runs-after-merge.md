# Mutation testing runs after the merge, never as a pull request gate

Mutation testing never blocks a pull request, and agents don't run it before pushing. CI runs it in two places instead. After each merge, `cargo xtask mutants-landed` tests the mutants in the diff that pull request brought into `main`. Each night, `cargo xtask mutants-shard` tests one of seven round-robin shards of all mutants. Both run in `.github/workflows/mutants.yml`, one at a time. A surviving mutant becomes a GitHub issue of its own, which later runs comment on or close. We chose this because quicken_clone, the earlier attempt at this project, gated pull requests on mutants and found it too slow: the wait held up review, while the finding is about where a later regression would go unnoticed, not about the change under review. A gap found after the merge is a to-do item, not a red pull request.

Decided while planning Balanced's CI, carrying over what quicken_clone learned (its #1243, #1311, #1239, #1242).

## Shape

- **The post-merge run finds its diff by asking GitHub** (`commits/{sha}/pulls`) which merged pull request left the pushed commit as its `merge_commit_sha`. It takes `before` from the first parent for a merge commit, and from `<sha>~<the pull request's commit count>` for a rebase merge. CI's own record of the previous commit isn't used: it can be a commit `main` never held. A failed lookup is red. A push no pull request landed tests nothing and stays green.
- **The nightly shard is the weekday,** Sunday 0 to Saturday 6. The workflow has one schedule per weekday and passes that day's number to `mutants-shard`, so the run's title in the Actions list names its shard. It's stateless. A lost night means that shard waits a week. N stays 7, one per weekday; more shards would need more than one run a day, and a changing N would reshuffle every shard.
- **One issue per mutant, not per run.** A mutant's key is its file, function, original-to-replacement text and its occurrence within the function. The key has no line number, so edits elsewhere in the file don't make it a new mutant. Every issue body stores the key as `<!-- mutant-key/v1: … -->`. Missed mutants are labelled `mutant` and `ready-for-agent`; timed-out ones also `mutant-timeout`. Unviable mutants never become issues.
- **Issues reconcile themselves.** A run first lists every mutant at its commit (`cargo mutants --list`), then, for each open issue: the run tested its key and caught it, so it closes the issue; the run tested it and missed it, so it comments "found again"; the key is in the full list but outside this run, so it leaves the issue alone; the key isn't in the full list, so it closes the issue as no longer generated. The workflow's concurrency group runs one at a time, so two runs never edit one issue at once.
- **A baseline guards against a broken `main`.** A run starts beside `ci`, not after it, so cargo-mutants first runs the unmutated tests of the crates in scope, and a failure there fails the run before any issue changes. Without it, every mutant on a broken commit would look caught and close its issue.
- **A missed or timed-out mutant leaves the step green.** A run that couldn't finish, or couldn't update its issues, is red.
- **Speed settings, measured in quicken_clone:** cargo-mutants runs its tests with nextest, mutates the throwaway CI checkout in place, keeps incremental compilation on and runs without sccache. Per-mutant builds there took a median of 6–9 s this way, against 34 s under sccache without incremental.
- **By hand, `cargo xtask mutants [<base>]` stays** for proving the fix for a mutant's issue. It keeps the 8 GB systemd memory cap and copy mode. In CI a `docker run --memory` limit on the container caps memory instead, 8 GB with no swap.

## Considered options

- **Gate pull requests on mutants over their diff:** rejected, as above. quicken_clone tried it and moved off it.
- **One issue per run, listing its survivors:** rejected. quicken_clone filed that way, and the post-merge run and the nightly then filed separate issues for the same mutant. A fix closed one and left the others open, and sessions wrote duplicate tests for mutants already killed.
- **Key issues by cargo-mutants' full mutant name:** rejected. The name includes the line number, so any edit above a mutant would make it look new.
