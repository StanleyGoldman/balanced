# Data-file performance with large files

Research for [#258](https://github.com/StanleyGoldman/balanced/issues/258). It answers the open question that the spec [#236](https://github.com/StanleyGoldman/balanced/issues/236) carried from the map #223: how a data file holding decades of transactions performs. The spec set the first indexes and left tuning for later. This doc gives that later work its starting numbers.

Measured 2026-10-19 on `main` at `b9b3045`.

## Answer in brief

- **At the stub's size (20,000 transactions in one account, 1.8 MiB file), nothing needs tuning.** A file opens in 36 ms warm and 57 ms cold. `Ledger` holds 5 MiB. Each Command writes through in 4–6 ms, about half of it the SQLite commit.
- **At 10× (200,000 transactions, 17.8 MiB), opening and memory still fit.** A file opens in 0.39 s warm and 0.59 s cold, and `Ledger` holds 41 MiB.
- **Write-through is the one cost that grows with the file.** The SQLite part stays flat at about 3 ms. The in-memory part grows linearly with every transaction in the file, because each Command deep-copies the whole `LedgerView` and rebuilds the edited account's register. At 10×, a post takes about 50 ms, three frames at 60 Hz. This is the one follow-up worth a ticket (see [Recommendation](#recommendation)).
- **The first indexes pay for themselves; keep them.** Without them, the full read at open takes 1.7–1.9× as long, `account_balance` over every account 4×, and the RESTRICT check when a category is deleted about 300× (10 ms against 0.03 ms at 10×). They cost 0.7 MiB of file at 1×, and they add about 10 ms to a warm `quick_check` at 10×.
- **`quick_check` on every open is affordable.** It takes 9 ms warm and 30 ms cold at 1×, and 95 ms warm and 290 ms cold at 10×. Cold, it is half of the open time. Skipping or deferring it isn't worth it at these sizes.

## Sizes, and what "decades" means

The stub's generated account, Household Checking, posts four transactions a day, so 20,000 transactions cover 13.7 years in one account. A household posting 1,000–3,000 transactions a year across all its accounts reaches 30,000–90,000 in 30 years, or 1.5–4.5× the stub. The 10× file (200,000 transactions in one account) is therefore a stress case, not a typical one. Costs that grow linearly can be read off at any size in between.

| | 1× | 10× |
|---|---|---|
| Household Checking transactions | 20,000 | 200,000 |
| Register entries in the file (every stub account) | 20,027 | 200,027 |
| File size | 1.8 MiB | 17.8 MiB |
| File size without the first indexes | 1.1 MiB | 10.2 MiB |

## Results

All times are medians, in milliseconds. "Warm" means the file's pages were in the OS page cache, and "cold" means they were evicted first (see [Method](#method)).

### Open

`Ledger::open` is `Store::open` (which runs `PRAGMA quick_check`), then `Store::read` (every row, with balances and totals from the views), then `Ledger::load` (rows into domain types).

| Step | 1× warm | 1× cold | 10× warm | 10× cold |
|---|---|---|---|---|
| `Store::open`, including `quick_check` | 9.4 | 26.7 | 96 | 296 |
| `quick_check` alone, on a new connection | 9.4 | 33 | 95 | 297 |
| `Store::read` | 24.3 | 24.0 | 270 | 257 |
| `Ledger::load` | 2.3 | 2.6 | 29 | 30 |
| **Total `Ledger::open`** | **35.9** | **53.6** | **396** | **586** |

- Every step grows linearly: ten times the transactions takes about ten times as long.
- `quick_check` reads every page, so it is the step that pays for a cold cache. Once it has run, `Store::read` finds the pages cached, and its cold and warm times match.
- `integrity_check`, for comparison, takes 15 ms (1×) and 172 ms (10×) warm, about 1.8× as long as `quick_check`.
- `Store::read` costs the most when warm. Its `register_entry` query computes a window sum over every transaction, with a correlated subquery per row for split amounts, and sorts the result.

### Memory

These figures come from a counting global allocator: the Rust heap live after `Ledger::open`, less the heap before it. SQLite's own page cache is allocated outside Rust's allocator and isn't counted.

| | 1× | 10× |
|---|---|---|
| Heap `Ledger` holds | 5.0 MiB | 40.6 MiB |
| Per register entry | 263 B | 213 B |
| Peak extra heap during open | 9.8 MiB | 79.3 MiB |

- A `RegisterEntry` is 152 B inline (`Transaction` is 128 B), and the payee and memo `String`s add the rest. At 1×, the stub's other accounts and the categories carry fixed overhead, so the per-entry figure is higher.
- The peak during open is about twice what `Ledger` holds: `Store::read`'s `Contents` and the loaded `Ledger` are both alive until `load` returns.
- A write briefly holds a second copy of the view, and the UI keeps the old snapshot until its next frame. So during a write the process holds about twice the figure above.

### Write-through

Each Command is applied with `Ledger::apply`, which commits one SQLite transaction (`BEGIN IMMEDIATE` … `COMMIT`, rollback journal, `synchronous = FULL`). Each was timed 30 times and undone, untimed, between runs. The probe re-took the snapshot after every apply, as the UI does.

| Command on Household Checking | 1× | 10× |
|---|---|---|
| Post, dated on the register's last date | 5.8 | 52 |
| Edit the first transaction's date to the last date (reorders the register) | 5.7 | 33 |
| Delete a transaction in the middle | 5.3 | 31 |
| Flag a transaction (no register rebuild) | 3.9 | 15 |
| *Baseline: rename the only account of an empty file* | *2.1–3.4* | |

The parts of a write, timed separately:

| Part | 1× | 10× |
|---|---|---|
| SQLite: a flag's `UPDATE`, undo-log insert and redo clear, committed | 3.3 | 3.1 |
| Deep-copy the whole `LedgerView` | 0.55 | 6.4 |
| Free a copy of the `LedgerView` | 0.33 | 3.7 |
| Rebuild the account's register (`edit_register`) | 0.92 | 10.7 |
| `find_transaction` (a linear scan) | 0.015 | 0.028 |

- **The SQLite part doesn't grow.** The commit costs about 3 ms at both sizes. It is mostly the fsync, and it matches the empty-file baseline.
- **The in-memory part grows with every transaction in the file, not only the edited account's.** `Ledger::write` takes a checkpoint holding the current `Arc<LedgerView>`, and the UI holds another. So the first `Arc::make_mut` in every Command deep-copies every account's register. When the checkpoint drops, the old view is freed. A Command that changes a transaction also rebuilds that account's register in full (`AccountSummary::transactions`, then `AccountSummary::new`), and frees the old one.
- That is why a flag, which rebuilds nothing, costs SQLite plus one copy and one free (3 + 6.4 + 3.7 ≈ 13 ms at 10×, measured 15). A post adds the rebuild, the old register freed inside the copy, and a `Vec::insert` into a full vector, which reallocates it.

### Views, with and without the first indexes

The first indexes, from the spec, are `transaction_register_order (account_id, date, id)`, `transaction_category_id`, `split_line_category_id` and `category_parent_id`. Running without them means a copy of each file with those four dropped and then `VACUUM`ed. The unique indexes and `split_line`'s primary key stay, because constraints need them. Each query reads every result row, warm, on one connection.

| Query | 1× with | 1× without | 10× with | 10× without |
|---|---|---|---|---|
| `register_entry WHERE account_id = ?` (every row) | 7.5 | 9.4 | 84 | 109 |
| …its last 50 rows by date | 10.5 | 12.5 | 116 | 141 |
| `account_balance WHERE account_id = ?` | 1.4 | 1.3 | 16.7 | 13.9 |
| `account_balance`, every account | 1.4 | 5.6 | 16.6 | 71 |
| `net_worth` | 1.3 | 5.4 | 16.7 | 71 |
| Insert and delete an unused category (RESTRICT checks), rolled back | 0.03 | 0.77 | 0.03 | 10.0 |
| `Store::read` (the whole read at open) | 23.5 | 38.8 | 267 | 498 |
| `quick_check`, warm | 9.4 | 8.6 | 95 | 87 |
| `quick_check`, cold | 33 | 20 | 297 | 162 |

- **What the indexes buy.** With `transaction_register_order`, `register_entry` reads one account through a covering index and needs no sort. Without it, SQLite scans every transaction and sorts in a temporary B-tree. `account_balance` over every account becomes one index-ordered pass instead of a scan per account. Deleting a category checks `transaction.category_id` and `split_line.category_id` through their indexes instead of scanning both tables.
- **What they cost.** The file is 1.6–1.7× as large, and a cold `quick_check` reads that much more.
- **What they don't fix.** `register_entry` for one account computes the window over the account's whole partition, so its last 50 rows cost more than all of them. A paged register would need its balances from somewhere other than this view. Today `Ledger` reads the views only at open and holds the result, so no Command runs them.

## Recommendation

**No tuning is needed for the indexes, `quick_check` or loading the whole file.** One problem is worth a follow-up ticket: write-through cost that grows with the file.

1. **Follow-up ticket: make a Command's in-memory cost independent of the file's size.** At 1×, a Command's in-memory part is 1–3 ms against a 3 ms commit. Interpolating linearly, at 4.5× (a busy household's 30 years) it is about 5–20 ms, and at 10× it measures 12–50 ms. The cause is that every Command deep-copies every register and rebuilds the edited one in full. Two changes would address it, together or apart:
   - Share structure between snapshots, e.g. `Arc<AccountSummary>` (or an `Arc`-shared register) in `LedgerView`, so `make_mut` copies a list of pointers and the one account it changes.
   - Rebuild running balances only from the changed entry onward, and only when an amount or the order changed.

   Measure the result with the same probe: a post at 10× should come close to the 3 ms commit.
2. **Keep the first indexes, and add none yet.** They nearly halve the read at open and turn the RESTRICT checks from table scans into lookups. No Command queries the views, so no other index has a query to serve. Revisit when a feature queries the file directly (reports, search).
3. **Keep `quick_check` on every open.** It costs 30 ms cold at 1× and 300 ms cold at the 10× stress size. Revisit by deferring it to after the first frame only if open time becomes a complaint. Running it in the background would need a second connection, which the exclusive lock forbids.
4. **Keep loading the whole file.** 41 MiB at 10× is no reason for partial loading. If opening a very large file ever needs to be faster, the cheaper lever is `Store::read`: read plain rows and let `AccountSummary::new` compute the running balances, which it already does for new ledgers. That would keep the views as the test oracle but take them off the open path, which spends 270 ms on them at 10×.

## After #442

[#442](https://github.com/StanleyGoldman/balanced/issues/442) acted on recommendation 1. Measured 2026-10-21 with the committed benchmark, `crates/balanced_core/benches/commands.rs` (see [tooling](../practices/tooling.md#measuring-performance)), on the same machine at load average 1.5, pinned to one core. Each Command ran 30 times, undone untimed between runs, with a snapshot held as a window holds one. The data file was on disk, so each time includes the commit's fsync.

| Command on Household Checking, median ms | 1× before | 1× after | 10× before | 10× after |
|---|---|---|---|---|
| Post, dated on the register's last date | 6.0 | 2.8 | 41.8 | 5.1 |
| Edit the first payment's date to the last date (reorders the register) | 6.4 | 3.9 | 55.6 | 8.3 |
| Delete the transaction in the middle | 7.4 | 4.2 | 35.3 | 6.2 |
| Flag the transaction in the middle | 5.3 | 3.0 | 22.9 | 4.6 |

The "before" numbers differ from the [Write-through](#write-through) table above, because this benchmark is not that probe: the probe was not kept, so the benchmark picks its own transactions and file location. Compare within this table.

What changed:

- **Snapshots share structure.** `LedgerView` holds each account as an `Arc<AccountSummary>`, so a Command copies a list of pointers and the accounts it changes. Each register holds its transactions as `Arc<Transaction>`, with the running balances in a separate `Vec<Money>`, so a changed account copies pointers and numbers, not transactions.
- **Running balances are counted from the changed entry on.** An edit that keeps the amount and the place counts none. The Cleared balance moves by the changed transactions' amounts.
- **Category and transfer usage is counted when first read.** `UsageCounts::count`, added after this research, walked every transaction on every Command. It took about 45 of the 48 ms a post cost at 10× once the rest was shared. Now only the Category List pays it, once per generation it reads.

What still grows with the file:

- A changed account copies its pointer and balance vectors: about 1.5–2 ms at 10× on top of the 1× cost. A post at 10× lands within 6 ms. Recommendation 1's "close to the 3 ms commit" holds at 1×.
- A date edit that moves a transaction from the start of the register to the end counts every balance again. Back-dated edits pay for what they move.
- `find_transaction` still scans every account's register.
- The Category List's first read after a change counts usage over the whole file. [#504](https://github.com/StanleyGoldman/balanced/issues/504) times the UI's per-generation rebuilds, and this is one of them.

## Method

### Machine

- Intel Core Ultra 9 275HX (24 cores), 61 GiB RAM, cpufreq governor `powersave`.
- KIOXIA KXG80ZNV1T02 NVMe SSD, ext4 on LVM (99% full during the runs).
- Linux 7.0.0-38-generic; rustc 1.99.0; rusqlite 0.40.2 with bundled SQLite 3.53.2.
- `release` profile. Each probe was pinned to one core (`taskset -c 0`) and run after the load average fell below 3. A first run at load 18, with other builds running, gave medians up to 4× higher and a wide spread, so it was discarded.

### Building the files

The probe set the stub's generated-transaction count (`GENERATED_TRANSACTIONS` in `crates/balanced_core/src/stub.rs`) to 20,000 or 200,000 and built `Ledger::stub()`. It then wrote the stub's categories and accounts, in Account bar order, into a new file from `Store::create`. This is the same row writing `in_memory_store` does (`write_account`), in one transaction. So each file holds the whole stub, with Household Checking at the chosen size.

### Timing

- **Open:** one warm-up open, then 10 warm runs and 5 cold runs, each timing `Store::open`, `Store::read` and `Ledger::load` (made `pub(crate)` for the probe) separately. A cold run first evicts the file's pages with `posix_fadvise(POSIX_FADV_DONTNEED)`. That empties the OS page cache for the file without root, and each open starts with an empty SQLite cache.
- **Memory:** a `#[global_allocator]` that counts live and peak bytes, read before and after `Ledger::open`.
- **Write-through:** 30 rounds on Household Checking. Each round times one post, one date edit, one delete and one flag, each followed by an untimed `Ledger::undo`.
- **Views:** a separate probe in `balanced_store` used `rusqlite` on the same files and on index-less copies. It stepped through every result row, ran one warm-up, then took 10 timed runs (5 for cold `quick_check` and `integrity_check`), and recorded `EXPLAIN QUERY PLAN` for the filtered queries.

The probes were throwaway code, kept off `main`: a `#[cfg(test)]` module beside `ledger.rs`, an integration test in `balanced_store`, and the temporary hooks described above. A follow-up tuning ticket should land its before-and-after measurement as a committed benchmark rather than rebuild them.
