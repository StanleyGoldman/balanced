# `Ledger` loads the data file once and writes through

`Ledger` owns its data file, through a `balanced_store::Store`. Opening a file reads all of it into memory: accounts, categories, transactions and split lines, the next ids, and the derived numbers, which come from the file's SQL views. From then on `Ledger` builds every `LedgerView` snapshot in Rust, as it did before there was a file. `apply`, `undo` and `redo` each run in one SQLite transaction. Each private operation writes its rows through the store at the point it changes memory, so a `Compound` and its parts share the transaction, and the commit comes last. The views are a test oracle: after any series of Commands, the snapshot of the reopened file equals the live one.

`Ledger::stub()` and every core test run on a real store over in-memory SQLite. There is no `Option<Store>` and no store trait.

Decided in [Store under `Ledger`](https://github.com/StanleyGoldman/balanced/issues/226) (Q1 to Q4), recorded by the spec [#236](https://github.com/StanleyGoldman/balanced/issues/236).

## Consequences

- Reading never touches SQL. Windows read the snapshot as before (ADR 0001), and the public seam (`apply`, `undo`, `redo`, `snapshot`) keeps its signatures.
- Opening a file costs one read of all of it, and memory holds the whole household's data.
- A failed write or commit rolls the file back, restores memory from the `Checkpoint`, keeps the Undo and Redo stacks as they were, and returns `DomainError::Store`, which the UI shows like any rejected Command. An amount with more minor units than an `i64` holds fails the same way.
- The same numbers are derived twice, in SQL and in Rust, so they can disagree. The reopen test exists to catch that, and it needs no test-only API: open reads the views, and the test compares snapshots.
- Ids come from `sqlite_sequence` on open, so an id is never reused, even after a restart.
- Moving an account parks one row at position `-1` before swapping. `UNIQUE (kind, position)` is checked row by row, so the spec's single `UPDATE … CASE` fails (#239). Inserting or deleting an account shifts its neighbours the same way, through negative positions.

## Considered options

- **Rebuild the snapshot from the views after every commit:** rejected. It re-reads the whole file for every Command.
- **Read through SQL on demand, with no in-memory copy:** rejected. Every frame would query SQLite, and the snapshot model of ADR 0001 would need a second implementation.
- **A store trait, with an in-memory fake for tests:** rejected. Tests would no longer cover the SQL the app runs, and the fake would be a second implementation to keep in step.
