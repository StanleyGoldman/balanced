# CI caches compiled crates with sccache, and starts every run with an empty target directory

Every CI step builds into a fresh `target/` inside its container. What persists between runs is sccache's cache and the cargo registry, both in the repository's GitHub Actions cache: sccache through its `ghac` backend, the registry as an `actions/cache` entry keyed on `Cargo.lock`. Every job reads and writes the one cache. Incremental compilation is off (`CARGO_INCREMENTAL=0`), which sccache needs to cache the workspace crates as well as their dependencies. Every log ends with `sccache --show-stats`. We chose this over keeping `target/` between runs because quicken_clone's persistent target directory corrupted repeatedly: link failures such as `undefined hidden symbol` from a cached rlib that no longer matched its sources (its #992, #1026). Each one failed every pipeline on that machine until someone with shell access to the host wiped the directory. A fresh `target/` can't carry state from an earlier commit, and sccache stores whole compiler outputs keyed by a hash of their inputs.

Decided while planning Balanced's CI.

## Considered options

- **A persistent `target/` per machine, plus sccache:** quicken_clone's final setup (its ADR 0017). Warm runs are a little faster, because cargo skips even the sccache lookups. Rejected for the corruption above, and because builds sharing one directory concurrently invalidate each other rather than queueing (939 s against 58 s in quicken_clone's #65), which means a directory per concurrent step.
- **An sccache directory per machine:** quicken_clone's setup, which kept sccache beside each machine's own `target/`. Rejected because a run lands on whichever machine is free, so each would hold part of the cache, and a pull request built on another would miss crates that are already cached. A hosted runner keeps nothing between runs at all.
- **A persistent `target/` with no sccache:** rejected. Wiping a corrupted directory then costs a cold build of every dependency.

## Consequences

- Linking and test binaries are rebuilt on every run; sccache only caches library crates.
- sccache's key includes absolute paths. The checkout is mounted at the same path in the image on every run, so CI hits. That's unlike a local worktree (see `docs/practices/tooling.md`).
- cargo-mutants is the exception. It turns incremental compilation on and sccache off for its own builds (ADR 0002). In a throwaway container its incremental state lasts one run, so it can't go stale the way quicken_clone's did.
- Parallel jobs run separate sccache servers against the one cache. An entry is written once and never changed, so they can't corrupt each other.
- The cache holds 10 GB per repository, shared by sccache, the CI image's layers and the registry, and evicts the least recently used first.
