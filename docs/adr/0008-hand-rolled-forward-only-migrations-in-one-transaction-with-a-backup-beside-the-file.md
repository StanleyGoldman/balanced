# Hand-rolled, forward-only migrations in one transaction, with a backup beside the file

A data file's schema version is its `PRAGMA user_version`: the number of migrations it has had. A runner in `balanced_store`, written by hand, upgrades a file from an older Balanced automatically when it opens, once every refusal check has passed and before it loads. Each migration is SQL plus an optional Rust function for data changes SQL can't make, and declares whether it changes the undo log's format.

The upgrade is one transaction: `foreign_keys=OFF` → `BEGIN IMMEDIATE` → every pending migration → `PRAGMA foreign_key_check` → `user_version` set to the latest → `COMMIT` → `foreign_keys=ON`. Before it starts, `VACUUM INTO` writes `<name> (before upgrade, v<n>).balanced` beside the file. There are no down migrations. A new file is made by the same migrations, from version 0.

Decided in [Data file migrations](https://github.com/StanleyGoldman/balanced/issues/230) (Q1–Q4, Q9), recorded by the spec [#236](https://github.com/StanleyGoldman/balanced/issues/236).

## Consequences

- A file is always at its old version or the latest, never in between. A failed step, or a foreign key broken once every step has run, rolls the whole upgrade back. Open then refuses the file as Upgrade failed and keeps the backup.
- If the backup can't be written, the file isn't upgraded or written to, and open refuses it as Backup failed. Nobody is upgraded without a copy to go back to.
- The backup is an ordinary data file at the old version, which an older Balanced opens. Balanced never deletes it. When a file already has the backup's name, as after an upgrade that failed, the new backup's name gains a number (`… (before upgrade, v1) 2.balanced`), so an earlier backup is never replaced.
- With foreign keys off, a migration can rebuild a table that others refer to with `RESTRICT`, using SQLite's 12-step table rebuild. `foreign_key_check` still catches any row the rebuild orphans.
- An upgrade through a migration that changes the undo format clears both stacks. Otherwise it keeps the log. Stored entries are never rewritten (ADR 0007).
- The runner takes its migrations as a slice, so stopping at a version is a shorter slice. Tests build files at an older version, and upgrade them through steps of their own, with `Store::create_with_migrations` and `Store::open_with_migrations` behind `test-support`.

## Considered options

- **`rusqlite_migration`:** rejected. It can't turn `foreign_keys` off for a table rebuild, and it is pinned in lockstep to rusqlite's version.
- **A transaction per migration:** rejected. A failure partway would leave the file at a version no released Balanced ever made.
- **Reversible migrations with down steps:** rejected. Down steps are rarely run, so they are rarely right. The backup taken before an upgrade does their job.
- **Asking before upgrading:** rejected. Declining would leave a file Balanced can't open, and the backup makes the upgrade safe to do without asking.
