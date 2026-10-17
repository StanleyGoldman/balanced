# CI caches compiled crates with sccache, and starts every run with an empty target directory

Every CI step builds into a fresh `target/` inside its container. What persists between runs on the host is sccache's cache and the cargo registry. The registry lives under `/var/lib/woodpecker/cache/balanced/`. sccache's cache is one directory, `/cache/agent/balanced/sccache`, which every agent reads and writes: all three oldhermit agents mount the same host folder as `/cache/agent`. Incremental compilation is off (`CARGO_INCREMENTAL=0`), which sccache needs to cache the workspace crates as well as their dependencies. Every log ends with `sccache --show-stats`. We chose this over keeping `target/` between runs because quicken_clone's persistent target directory corrupted repeatedly: link failures such as `undefined hidden symbol` from a cached rlib that no longer matched its sources (its #992, #1026). Each one failed every pipeline on that agent until someone with shell access to the host wiped the directory. A fresh `target/` can't carry state from an earlier commit, and sccache stores whole compiler outputs keyed by a hash of their inputs.

Decided while planning Balanced's Woodpecker CI.

## Considered options

- **A persistent `target/` per agent, plus sccache:** quicken_clone's final setup (its ADR 0017). Warm runs are a little faster, because cargo skips even the sccache lookups. Rejected for the corruption above, and because builds sharing one directory concurrently invalidate each other rather than queueing (939 s against 58 s in quicken_clone's #65), which means a directory per concurrent step.
- **An sccache directory per agent:** quicken_clone's setup, which kept sccache beside each agent's own `target/`. Rejected because `ci` runs on whichever of oldhermit-1 and oldhermit-2 is free, so each agent would hold half the cache, and a pull request built on the other agent would miss crates that are already cached. The `target/` that made quicken_clone's directories per-agent no longer persists.
- **A persistent `target/` with no sccache:** rejected. Wiping a corrupted directory then costs a cold build of every dependency.

## Consequences

- Linking and test binaries are rebuilt on every run; sccache only caches library crates.
- sccache's key includes absolute paths. The checkout path is the same on every run, so CI hits. That's unlike a local worktree (see `docs/practices/tooling.md`).
- cargo-mutants is the exception. It turns incremental compilation on and sccache off for its own builds (ADR 0002). In a throwaway container its incremental state lasts one run, so it can't go stale the way quicken_clone's did.
- Two `ci` steps can run at once, so two sccache servers share one directory. Each server tracks `SCCACHE_CACHE_SIZE` on its own, so the cache can overshoot the cap. If sharing ever corrupts entries, the fallback is one sccache storage service on the host (its Redis or WebDAV backend) that every agent points at.
- `/cache/agent` is mounted into every repository's steps, so every path is namespaced under `balanced/`. Otherwise balanced and another repository on these agents would build into one directory.
