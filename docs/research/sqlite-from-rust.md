# SQLite tooling from Rust

Research for [#225](https://github.com/StanleyGoldman/balanced/issues/225) (child of the wayfinder map #223, "Balanced data file (SQLite)"). It feeds #227 and #230.

Researched 2026-10-18. Crate versions, licences, MSRVs and features come from `cargo info` on that date; crate sources were read from the local cargo registry. Release dates come from each repo's GitHub releases or tags. SQLite facts come from sqlite.org.

## Question

Which Rust tooling should Balanced's SQLite data file use? Balanced is a synchronous egui desktop app with a single writer. Each data file is one SQLite file. The database guards integrity (constraints, foreign keys, triggers) and derived reads (views); behaviour stays in `Ledger::apply` (ADR 0001). The undo and redo stacks of inverse `DomainCommand`s live in the file, and each Command commits as one SQLite transaction. The answer covers three things:

1. **The driver:** `rusqlite` against `sqlx` and other candidates.
2. **SQL-level tests** of views, triggers and constraints, and how they run under `cargo nextest` and `cargo xtask ci`.
3. **Migrations:** crates against hand-rolled migrations on `PRAGMA user_version`, and testing migrations against fixture files.

## Answer in brief

- **Driver: `rusqlite` 0.40.2** with `bundled` (SQLite 3.53.2). It is synchronous, MIT-licensed, released every few months, and exposes everything the map needs: typed transactions with a rollback-on-drop default, Rust-defined scalar, aggregate and window functions, the session extension, and pragma helpers. `sqlx` is async-only, cannot register Rust scalar functions, and its released 0.9.0 cannot share a build with rusqlite 0.40.
- **SQL tests: the `sqllogictest` crate** (0.29.1) as a library, behind a `harness = false` test target. Its `harness!` macro builds on `libtest-mimic`, which nextest supports, so `.slt` files run under `cargo xtask test` with no new xtask step. A local probe confirmed this, including a red run. There is no pgTAP for SQLite.
- **Migrations: `rusqlite_migration`** (2.6.0, `user_version`, Apache-2.0) fits the "one integer in the header" model and the single-file app. Its catch: it runs all pending migrations in one transaction, so it cannot switch `foreign_keys` off for SQLite's 12-step table rebuild. A hand-rolled runner of about 50 lines avoids that limit and the crate's lockstep pin on rusqlite. `refinery` adds a history table and checksums that a single-user file does not need.
- **Fixture tests:** commit one data file per past schema version, migrate a temp copy to the latest version, then compare its `sqlite_schema` with a freshly created file and run `PRAGMA integrity_check` and `PRAGMA foreign_key_check`.

## 1. The driver

### 1a. `rusqlite`

- Crate: [`rusqlite`](https://docs.rs/rusqlite/0.40.2/rusqlite/) 0.40.2, MIT, repo <https://github.com/rusqlite/rusqlite> (4.4k stars, 170 open issues, last push 2026-10-18). Releases: 0.36.0 (2025-05-26), 0.37.0 (2025-07-06), 0.38.0 (2025-12-20), 0.39.0 (2026-03-15), 0.40.0 (2026-05-26), 0.40.1 (2026-06-06), 0.40.2 (2026-08-08). That is a breaking 0.x release every two to five months. The 0.40.2 release note is "Lower MSRV to 1.88.0".
- **Bundled SQLite.** The `bundled` feature compiles the amalgamation shipped in [`libsqlite3-sys`](https://docs.rs/libsqlite3-sys/0.38.2/) 0.38.2 (MIT). Its `sqlite3/sqlite3.h` defines `SQLITE_VERSION "3.53.2"`; the 0.40.1 release note reads "Bump bundled SQLite version to 3.53.2". sqlite.org's [chronology](https://www.sqlite.org/chronology.html) dates 3.53.2 to 2026-06-03; the newest SQLite is 3.54.1 (2026-10-16), not yet bundled. The bundled build (`libsqlite3-sys` `build.rs`) compiles with, among others, `-DSQLITE_DEFAULT_FOREIGN_KEYS=1`, `-DSQLITE_ENABLE_FTS5`, `-DSQLITE_ENABLE_JSON1`, `-DSQLITE_ENABLE_STAT4` and `-DSQLITE_THREADSAFE=1`. Foreign keys are therefore **on by default** in the bundled build (confirmed by the probe below). sqlite.org's [foreign key page](https://www.sqlite.org/foreignkeys.html) still advises: "Careful developers will not make any assumptions about whether or not foreign keys are enabled by default". The stock `sqlite3` CLI does not share the bundled flags.
- **Features.** Default features are `cache` (a prepared-statement cache through `hashlink`) and `ffi-sqlite-wasm-rs` (wasm only). Opt-in features relevant here: `bundled`, `functions`, `window`, `hooks`, `preupdate_hook`, `session`, `backup`, `serialize`, `trace`, `limits`, `jiff` (`ToSql`/`FromSql` for jiff types, which Balanced already uses), `serde_json`, `extra_check`. There is no `rust_decimal` feature, and `rust_decimal` 1.43.0 has no SQLite feature either (its `db-*` features cover Postgres and MySQL only).
- **Transaction API** (`src/transaction.rs`):
  - `Connection::transaction(&mut self) -> Result<Transaction<'_>>`, `transaction_with_behavior(TransactionBehavior::{Deferred, Immediate, Exclusive})`, and `unchecked_transaction(&self)`, which skips the `&mut` borrow.
  - `Transaction::commit(self)` and `rollback(self)` consume the guard. `set_drop_behavior(DropBehavior::{Rollback, Commit, Ignore, Panic})` decides what happens on drop; the default is `Rollback`. A `?` early return inside a Command therefore rolls back the Command and its undo-log row together.
  - Nested savepoints: `Transaction::savepoint()` and `savepoint_with_name()`. 0.40.1 fixed "SQL injection when SAVEPOINT name is tainted" (#1854).
  - `transaction_state()` reports whether a transaction is open.
- **Rust-defined SQL functions** (`functions` feature, `src/functions.rs`): `create_scalar_function`, `create_aggregate_function`, `create_window_function` (`window` feature) and `remove_function`, with `FunctionFlags` bits `SQLITE_DETERMINISTIC`, `SQLITE_DIRECTONLY` and `SQLITE_INNOCUOUS`. Two SQLite rules bind any function used inside a view, trigger or CHECK:
  - With [`PRAGMA trusted_schema=OFF`](https://www.sqlite.org/pragma.html#pragma_trusted_schema), SQLite refuses to call a function from the schema "unless the function is tagged with SQLITE_INNOCUOUS" ([function flags](https://www.sqlite.org/c3ref/c_deterministic.html)). The same page advises using that flag only on functions "carefully audited and found to be free of potentially security-adverse side-effects". `trusted_schema` is ON by default "to maintain backwards compatibility". [`sqlite3_create_function`](https://www.sqlite.org/c3ref/create_function.html) recommends `SQLITE_DIRECTONLY` for every function the schema does not need, so a crafted file cannot call it.
  - Functions live on the connection, not in the file. Every connection that opens the file must register them before it reads a view that uses one. The `sqlite3` CLI cannot read such a view.
- **Session extension** (`session` feature, `src/session.rs`): `Session::new`, `attach`, `table_filter`, `changeset`, `patchset`, `diff`; `Changeset::invert` and `concat`; `Changegroup`; and `Connection::apply` with a conflict callback. In `libsqlite3-sys`, `session = [preupdate_hook, buildtime_bindgen]`, so turning it on runs bindgen at build time, which needs libclang on every build machine. sqlite.org's [session introduction](https://www.sqlite.org/sessionintro.html) says: "A 'changeset' may also be inverted and used to 'undo' a session". It also says the extension "only works with tables that have a declared PRIMARY KEY", ignores rows with a NULL in a primary-key column, and ignores virtual tables. Under ADR 0001, undo is inverse `DomainCommand`s, so Balanced does not need sessions for undo.
- **`PRAGMA user_version`** (`src/pragma.rs`): `pragma_query_value(None, "user_version", |r| r.get(0))` and `pragma_update(None, "user_version", n)`; `pragma_update_and_check` also exists. The [pragma docs](https://www.sqlite.org/pragma.html#pragma_user_version) say: "The user-version is an integer that is available to applications to use however they want. SQLite makes no use of the user-version itself". The [file format](https://www.sqlite.org/fileformat2.html) puts it at offset 60 of the header. Next to it, at offset 68, is the application ID, which "identif[ies] the database as belonging to or associated with a particular application". Balanced can stamp that ID so it can recognise its own files.
- **Other things the map will need:** `Connection::backup` (`backup` feature) and `serialize`/`deserialize` (`serialize` feature) for the copy taken before a migration; `open_with_flags`; `set_db_config` (for example `SQLITE_DBCONFIG_DEFENSIVE`). `Connection` is `Send`, not `Sync`, which fits one owner, the `Ledger`, holding it.
- **cargo-deny fit.** `libsqlite3-sys` declares `links = "sqlite3"`, so only one version can sit in the graph. Every crate that touches SQLite (driver, migration crate) must agree on it.

### 1b. `sqlx`

- Crate: [`sqlx`](https://docs.rs/sqlx/0.9.0/sqlx/) 0.9.0, MIT OR Apache-2.0, MSRV 1.94.0, repo <https://github.com/launchbadge/sqlx> (17.6k stars, 786 open issues). 0.9.0 was tagged 2026-05-21 ("Release 0.9.0 (#4256)"). Default features: `any`, `macros`, `migrate`, `json`.
- **Async only.** The `sqlx-sqlite` docs (`src/connection/mod.rs`): "Because SQLite is an in-process database accessed by blocking API calls, SQLx uses a background thread and communicates with it via channels". A synchronous egui app would need an async runtime, or `block_on`, at every call.
- **Bundled SQLite and a version clash.** `sqlx-sqlite` 0.9.0 requires `libsqlite3-sys = ">=0.30.1, <0.38.0"`. rusqlite 0.40.x needs 0.38.x, and `links = "sqlite3"` forbids two copies, so sqlx 0.9.0 and rusqlite 0.40 cannot share a build. That rules out, for example, rusqlite's session support beside sqlx. The main branch relaxed the range to allow 0.38.x on 2026-09-09 (#4402), but no release carries it yet.
- **Functions.** `sqlx-sqlite` has `SqliteConnectOptions::collation` and `SqliteConnection::create_collation`, plus an optional built-in `regexp`, but no API to register a Rust scalar, aggregate or window function. The escape hatch is `lock_handle()` / `as_raw_handle()` and unsafe `libsqlite3-sys` FFI. Its docs warn that callers doing this "should pin the version of SQLx".
- **Session and hooks.** `sqlx-sqlite` exposes `preupdate-hook`, `unlock-notify`, `deserialize` and `load-extension` behind `_unstable-all-sqlite-features`. It has no session API.
- **Transactions and pragmas.** `Connection::begin()` returns a `Transaction` that rolls back on drop. Pragmas are set through `SqliteConnectOptions::pragma`, `foreign_keys` and `journal_mode`.
- **Migrations** (`migrate` feature) record applied versions in a migrations table (see §3c), not in `user_version`.
- **Strength:** compile-time-checked `query!` macros. They need a database at build time through `DATABASE_URL`, or prepared offline data. With SQLite, that build-time database can be a file.

### 1c. Other candidates

| Crate | Version, licence | Notes |
|---|---|---|
| [`diesel`](https://docs.rs/diesel/2.3.14/diesel/) | 2.3.14, MIT OR Apache-2.0, MSRV 1.86 | Synchronous ORM and query builder on `libsqlite3-sys >=0.17.2, <0.39.0`, so it can share a build with rusqlite 0.40. It adds a Rust schema DSL (`table!`) beside the SQL. The map's testable surface is SQL (views, triggers), which an ORM layer does not help test. |
| [`sqlite`](https://docs.rs/sqlite/0.37.4/sqlite/) | 0.37.4, Apache-2.0 OR MIT | A thin binding on its own `sqlite3-sys` 0.18, not `libsqlite3-sys`. It has a smaller API surface than rusqlite. |
| [`turso`](https://github.com/tursodatabase/turso) | 0.9.0-pre.1, MIT | A rewrite of SQLite in Rust. Its README: "we have not yet reached 1.0" and, on compatibility, "We are not at 100% yet". Too young to trust with a user's financial file. |

## 2. SQL-level testing of views, triggers and constraints

### 2a. `sqllogictest` (`.slt` files)

- **Format origin.** SQLite's own [sqllogictest](https://sqlite.org/sqllogictest/doc/trunk/about.wiki) "is concerned only with correct results". Its records are `statement ok`, `statement error`, `query <types> <sort-mode> [label]` (sort modes `nosort`, `rowsort` and `valuesort`), `hash-threshold`, `halt`, and `skipif`/`onlyif`. It deliberately leaves out performance, transactions and concurrency.
- **Rust crate:** [`sqllogictest`](https://docs.rs/sqllogictest/0.29.1/sqllogictest/) 0.29.1, MIT OR Apache-2.0, repo <https://github.com/risinglightdb/sqllogictest-rs> (234 stars). Releases: 0.28.3 (2025-06-06), 0.28.4 (2025-09-07), 0.29.0 (2025-12-19), 0.29.1 (2026-02-14). RisingWave, DataFusion and Databend use it.
- **Extensions over the original format** (`src/parser.rs`, README):
  - `statement error <regex>`, or a full multi-line message after `----`. A test can therefore pin the exact `RAISE(ABORT, '…')` text or the constraint name.
  - `statement count N` asserts the number of affected rows.
  - `include`, named connections, `let` (binds a query result to variables), `control substitution on` (`$__TEST_DIR__` and friends), `retry`, `system ok`, and `<slt:ignore>` for volatile output.
- **Library use.** Implement the synchronous `DB` trait (`fn run(&mut self, sql) -> Result<DBOutput<_>, _>`) over a `rusqlite::Connection`. The crate ships no SQLite engine: `sqllogictest-engines` covers Postgres, MySQL and an external process, and the `sqllogictest-bin` CLI likewise supports only PostgreSQL and MySQL. The adapter is about 30 lines (probe below).
- **`cargo nextest` fit.** `sqllogictest::harness!(make_db, "tests/slt/**/*.slt")` generates a `main` that globs the files and runs each as one `libtest_mimic::Trial`. It goes in a `[[test]]` target with `harness = false`. The nextest [custom test harness](https://nexte.st/docs/design/custom-test-harnesses/) page says it supports libtest-mimic "version 0.4.0, or 0.5.2 or above", and "Using this crate is recommended". sqllogictest 0.29.1 uses libtest-mimic 0.8.2. Each `.slt` file becomes one nextest test, named by its path. `Runner::run_file` drives the async runner with `futures::executor::block_on`, so no tokio is needed.
- **`cargo xtask ci` fit.** `cargo xtask test` already runs `cargo nextest run --workspace --all-features`, which picks the target up. No new xtask step is needed. Auto-updating expected output (`--override`) belongs to the CLI. The library exposes `update_test_file` (async) and `update_record_with_output`, which a later `cargo xtask` task could wrap if blessing output by hand gets tedious.
- **Dependency cost.** It pulls `rand` 0.8, `getrandom` 0.2, `syn` 2, `clap` (through libtest-mimic), `regex`, `similar`, `md-5`, `glob` and `futures`. Balanced's `Cargo.lock` already contains `rand` 0.8 and 0.9, `getrandom` 0.2, 0.3 and 0.4, and `syn` 2 and 3, and `deny.toml` already has a `syn@2` skip. `cargo deny check` still has to pass when the crate is added.

### 2b. Alternatives

- **Plain `#[test]` functions over rusqlite**, with `insta` snapshots of query output. `insta` 1.49.0 is already a workspace dependency. The fixtures and assertions are Rust, so the SQL surface is tested only through Rust strings. This is the natural home for round trips through `Ledger`.
- **[`datatest-stable`](https://docs.rs/datatest-stable/0.3.3/)** 0.3.3: a libtest-mimic harness that runs a Rust function per file. Nextest's docs name it as a working example. Useful only if Balanced wanted its own file format instead of `.slt`.
- **pgTAP for SQLite:** none found. Searches turned up only pgTAP itself. pgTAP is written in PL/pgSQL, and SQLite has no stored procedures or procedural language to host a comparable assertion library. Near equivalents are a Rust-defined `assert_*` function registered only in tests, or triggers that `RAISE`, driven by `.slt` files.
- **`sqldiff`** ([docs](https://www.sqlite.org/sqldiff.html)) compares two files. It "does not (currently) display differences in TRIGGERs or VIEWs" and "is not designed to support schema migrations", so it cannot check migrated schemas (see §3e).

### 2c. Local probe

A scratch crate (not committed) combined rusqlite 0.40.2 (`bundled`, `functions`), rusqlite_migration 2.6.0 and sqllogictest 0.29.1 under `cargo nextest` with the repo's rustc. Its one migration created two `STRICT` tables with a CHECK and a foreign key, a `BEFORE DELETE` trigger that `RAISE`s, and a view calling a Rust function `fmt_minor`. The connection set `trusted_schema = OFF`. A test file looked like this:

```
statement error CHECK constraint failed
INSERT INTO line(account_id, amount_minor) VALUES (1, 0)

statement error FOREIGN KEY constraint failed
INSERT INTO line(account_id, amount_minor) VALUES (99, 100)

statement error account has lines
DELETE FROM account WHERE id = 1

query TT rowsort
SELECT name, total FROM balance
----
Checking 115.00
```

Results:

- `cargo nextest run` listed and passed `sqlprobe::slt tests/slt/ledger.slt` alongside two ordinary unit tests.
- Changing the expected `115.00` to `115.01` turned the test red with a `[Diff] (-expected|+actual)` message pointing at the file and line. Editing a `.slt` file needed no rebuild: the files are read at run time.
- `rusqlite::version()` returned `"3.53.2"`, and `PRAGMA foreign_keys` read `1` on a fresh bundled connection, with no pragma set.
- Dropping `SQLITE_INNOCUOUS` from `fmt_minor` made the view query fail with `unsafe use of fmt_minor()` under `trusted_schema = OFF`.

## 3. Migrations

### 3a. What SQLite allows

- `ALTER TABLE` supports only RENAME TO, RENAME COLUMN, ADD COLUMN, DROP COLUMN and, since 3.53.0, ALTER COLUMN … SET/DROP NOT NULL ([docs](https://www.sqlite.org/lang_altertable.html)). Any other change uses the 12-step rebuild: create `new_X`, copy rows, drop `X`, rename `new_X` to `X`, then recreate indexes, triggers and views. That covers adding a CHECK, a UNIQUE or a foreign key, or changing a column's type. The page warns that renaming the old table first "might corrupt references to that table in triggers, views, and foreign key constraints".
- Step 1 of the rebuild is `PRAGMA foreign_keys=OFF` before the transaction starts. Step 10 runs `PRAGMA foreign_key_check` before commit. The pragma docs: `foreign_keys` "is a no-op within a transaction". The reason, from the [foreign key page](https://www.sqlite.org/foreignkeys.html): with enforcement on, `DROP TABLE` "performs an implicit DELETE … but may invoke foreign key actions or constraint violations". An `ON DELETE CASCADE` child would lose its rows. `PRAGMA defer_foreign_keys` delays constraint checks until commit, but foreign key actions are not constraint checks, so it does not stop a cascade.
- `user_version` sits in the header page, so setting it inside the migration's transaction commits or rolls back with the schema change. rusqlite_migration relies on this.

### 3b. `rusqlite_migration`

- Crate: [`rusqlite_migration`](https://docs.rs/rusqlite_migration/2.6.0/rusqlite_migration/) 2.6.0 (2026-05-28), **Apache-2.0**, MSRV 1.95. Repo <https://github.com/cljoly/rusqlite_migration> (115 stars, last push 2026-10-02). Releases: 2.3.0 (2025-07-09), 2.4.0 (2026-01-19), 2.4.1 (2026-01-25), 2.5.0 (2026-03-21), 2.6.0. Its description: it uses "user_version instead of an SQL table to maintain the current schema version".
- API: `M::up(sql)`, `M::up_with_hook(sql, hook)` (a Rust `Fn(&Transaction)` for data transforms), `M::down`, `M::comment`, `M::foreign_key_check()` (runs `PRAGMA foreign_key_check` before commit), `Migrations::from_slice` (const) or `from_directory` (feature `from-directory`), `current_version`, `pending_migrations`, `to_latest`, `to_version`, and `validate()`. `validate()` runs every migration on a fresh in-memory database; it is the crate's suggested test. The README also suggests `insta::assert_debug_snapshot!(migrations)`.
- **One transaction for all pending steps.** `goto_up` opens one `conn.transaction()`, runs every pending `up`, sets `user_version` once and commits. A file several versions behind therefore migrates all-or-nothing. It also means no step can run outside a transaction, so `PRAGMA foreign_keys=OFF` for the 12-step rebuild cannot be expressed. The crate's own docs discourage `PRAGMA` in migrations and point to `foreign_key_check()` instead.
- **Lockstep with rusqlite.** It depends on `rusqlite = "0.40.0"` (caret), so a rusqlite 0.41 upgrade waits for a rusqlite_migration release.
- Probe note: `Migrations::from_slice(&[M::up(..)])` written inline fails to compile as a `const` (E0493, destructor in a constant). The README's two-step form works: first `const SLICE: &[M] = &[..]`, then `from_slice(SLICE)`.

### 3c. `refinery`

- Crate: [`refinery`](https://docs.rs/refinery/0.10.0/refinery/) 0.10.0 (2026-10-02), MIT, MSRV 1.92. Repo <https://github.com/rust-db/refinery> (1.7k stars). Releases: 0.9.0 (2026-04-14), 0.9.1, 0.9.2 (2026-06-10), 0.10.0. The `rusqlite` feature accepts `rusqlite >=0.23, <=0.40`.
- Model (from its README): Flyway-style `V{n}__{name}.sql` or `.rs` files, embedded with `embed_migrations!`. Applied versions and checksums go in a `refinery_schema_history` table. It can abort on divergent or missing migrations. By default it runs "each migration in a single transaction", or all in one with `set_grouped(true)`. It has no down migrations ("you have to generate a new one"). A Rust migration is a function returning the SQL `String`, so it cannot run Rust code against the connection.
- Fit: the checksums and history table answer a team-deploys-to-a-server problem. A single-user file whose schema ships inside the binary does not have that problem.

### 3d. Others

- `sqlx migrate` (requires sqlx; uses its own migrations table), `diesel_migrations` 2.3.2 (requires diesel), `schemer-rusqlite` 0.2.2 and `migrant_lib` 1.0.0-rc.2 exist. None adds anything the two above lack for a single-file app.

### 3e. Hand-rolled on `user_version`

- Shape: a `const MIGRATIONS: &[Migration]` where each step is SQL plus an optional Rust function. The runner reads `user_version`. A file newer than the binary is refused. Otherwise each pending step runs as: `foreign_keys=OFF` when the step rebuilds tables → `BEGIN IMMEDIATE` → SQL and Rust hook → `PRAGMA foreign_key_check` → `PRAGMA user_version = n` → `COMMIT` → `foreign_keys=ON`. This is SQLite's 12-step order with no workaround.
- Cost: about 50 lines that Balanced owns and tests. There is no third-party pin on rusqlite. Balanced decides per step or per run whether a failure leaves the file at the last good version or at the start.
- What Balanced gives up: `from_directory`, `down` migrations and `validate()`. All three are short to write, and `down` migrations do not fit a user's file anyway: restoring the backup taken before the migration replaces them.

### 3f. Testing migrations against fixture files

No crate surveyed ships fixture-file testing. `rusqlite_migration::validate()` only migrates an empty in-memory database. The pieces available:

- **Fixtures:** one real data file per released schema version (`tests/fixtures/v1.balanced`, …), each holding representative rows, undo-log entries included. A test copies the fixture to a `tempfile` path (`tempfile` 3.27.0 is already a workspace dependency), opens it, and migrates to the latest version.
- **Schema equality:** compare `SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY name` between the migrated file and a freshly created one. This catches drift in views and triggers, which `sqldiff --schema` ignores. `insta` can snapshot the fresh schema, so every schema change shows up in review.
- **Integrity:** `PRAGMA integrity_check` returns `ok`. `PRAGMA foreign_key_check` returns no rows; the pragma docs note that integrity_check "does not find FOREIGN KEY errors".
- **Data survival:** open the migrated file through `Ledger` and assert the snapshot. This is the round trip the map asks for.
- **Stepwise:** `rusqlite_migration::to_version`, or the hand-rolled runner's equivalent, can stop at each version so `.slt` files can probe intermediate schemas.
- **Backup before migrating:** `rusqlite::Connection::backup` (`backup` feature), or SQL `VACUUM INTO 'path'` ([docs](https://www.sqlite.org/lang_vacuum.html)), copies the file before the first step.

## Recommendation

This section is judgement; everything above is fact.

1. **Use `rusqlite` with `bundled`**, `default-features = false`, and only the features Balanced uses: `bundled`, `functions`, `backup`, and `jiff` if dates are stored through it. Keep `cache` if prepared statements are reused. Leave out `session` until there is a use for it: it adds a libclang build dependency, and ADR 0001 already covers undo. Declare it in `[workspace.dependencies]`, depend on it only from `balanced_store`, and name the bundled SQLite version in a comment there so an upgrade is deliberate.
2. **Set connection state explicitly on open, in one function:** `foreign_keys = ON` (do not lean on the bundled default), `trusted_schema = OFF`, the Rust-defined functions with `SQLITE_DETERMINISTIC`, plus `SQLITE_INNOCUOUS` only for pure functions that views or triggers call, and `SQLITE_DIRECTONLY` for the rest, and a check of `application_id`. Each Command runs in `transaction_with_behavior(Immediate)` and relies on the rollback-on-drop default.
3. **Test the SQL surface with `.slt` files** through the `sqllogictest` library and a `harness = false` target in `balanced_store`, built on the same open function and migrations as the app. Keep `Ledger` round trips as ordinary `#[test]`s with `insta`. Both run under the existing `cargo xtask test`. Treat a view that calls a Rust function as tested only when its `.slt` file covers it.
4. **Hand-roll migrations on `user_version`**, one transaction per step, with `foreign_keys` switched off around table rebuilds. Of the crates, `rusqlite_migration` is closest, and it is the right choice if the schema never needs a 12-step rebuild under foreign keys. But adding a CHECK or a foreign key later needs exactly that rebuild, and the crate's lockstep rusqlite pin is a recurring cost. Leave `refinery` aside.
5. **Gate every schema change on fixture tests:** a committed data file per past version, migrated and compared with a fresh file's `sqlite_schema`, checked with `integrity_check` and `foreign_key_check`, and read back through `Ledger`.

## Open questions for the spec (#227, #230)

- Should a multi-step upgrade commit per step (resumable) or as one transaction (all or nothing)? Either way, the backup taken before migrating is the fallback.
- Which values cross into SQL as Rust functions (money formatting, rounding), and which stay plain SQL? Every Rust function makes the file unreadable by the `sqlite3` CLI wherever it appears.
- Should a file written by a newer Balanced (higher `user_version`) open read-only in an older one, or be refused outright?
