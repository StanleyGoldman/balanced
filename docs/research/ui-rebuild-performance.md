# The UI's rebuild after each Command

Research for [#504](https://github.com/StanleyGoldman/balanced/issues/504). [`data-file-performance.md`](data-file-performance.md) timed the core's write path, and [#442](https://github.com/StanleyGoldman/balanced/issues/442) brought a Command at 10× down to 5–8 ms. This doc times what the UI rebuilds after each Command, which the user waits for too.

Measured 2026-10-22 on the branch for #504, cut from `main` at `ef01571`.

## Answer in brief

- **The UI's rebuild, not the core, sets how long a Command takes.** Before this work, the frame that applied a flag took 52 ms at the stub's size (20,000 transactions) and 499 ms at 10×. The Command itself was 3–5 ms of that.
- **The Register's rows were 85–95% of the frame.** `RegisterModel::new` took 44.5 ms at 1× and 475 ms at 10×. About three quarters of that went to building a new ICU4X `CurrencyFormatter` for each of the three amounts on every row.
- **Fixed here: each formatting locale keeps the amount formatters it builds.** The Register now takes 12.4 ms at 1× and 154 ms at 10×, and the frame after a Command 19 ms and 182 ms.
- **10× still needed the Register to rebuild less than every row.** 154 ms is about ten frames. [#580](https://github.com/StanleyGoldman/balanced/issues/580) now reformats only the rows that changed: the rows take 4.6 ms after a Command at 4.5× (90,000 transactions in one account), and a filter keystroke's frames 4.8 ms. The whole frame after a Command still misses #580's one-frame target there, at 36 ms: the Command's commit and the comboboxes' choices are most of it ([#621](https://github.com/StanleyGoldman/balanced/issues/621), [#581](https://github.com/StanleyGoldman/balanced/issues/581)). See [After #580](#after-580).
- **The comboboxes' choices and the usage count are small beside it.** `FieldChoices::new` takes 1.2 ms at 1× and 15.5 ms at 10×, and the usage count 1.3 ms and 13.4 ms. Only an open Category List pays for the usage count. [#581](https://github.com/StanleyGoldman/balanced/issues/581) tracks the choices, after #580; [#582](https://github.com/StanleyGoldman/balanced/issues/582), the usage count, closed as not worth its code for now.

## Results

All times are medians of 30 runs, in milliseconds. "Before" is the branch's first commit, with the benchmarks and without the formatter cache; "after" adds the cache. Both binaries ran back to back in the same quiet window.

### One frame of the App

`benches/frame.rs` drives the App through `egui_kittest`, showing Household Checking's Register. One frame runs with nothing to do. The other applies a Command that flags or unflags the middle transaction, which moves no balance, and rebuilds what the Command changed.

| Frame | 1× before | 1× after | 10× before | 10× after |
|---|---|---|---|---|
| Idle | 0.45 | 0.46 | 0.46 | 0.45 |
| After a Command | 52.3 | 19.3 | 499 | 182 |

- **An idle frame doesn't grow with the file.** The Register lays out only the rows on screen, and nothing is rebuilt.
- **The frame after a Command is the sum of its parts.** At 10× after: the Command (about 4.6 ms, from the [Commands benchmark](data-file-performance.md#after-442)), the Register's rows (154), the comboboxes' choices (15.5), and an idle frame's work. The Account bar's model and the rest add about 2 ms at 1× and 7 ms at 10×.

### Each rebuild alone

`benches/rebuild.rs` times each rebuild on a snapshot no earlier run has read, taken after an untimed flag. The Register's rows and the choices are dropped inside each run, as a window drops the old model when it builds a new one. The usage count is kept on the snapshot, so its drop falls outside the run.

| Rebuild | 1× before | 1× after | 10× before | 10× after |
|---|---|---|---|---|
| `RegisterModel::new`, every row of Household Checking | 44.5 | 12.4 | 475 | 154 |
| `FieldChoices::new` | 1.19 | 1.18 | 15.3 | 15.5 |
| Usage count (`UsageCounts::count`, on the first `category_usage` read) | 1.30 | 1.32 | 13.1 | 13.4 |

- **Every rebuild grows linearly with the file.** Each walks every transaction, of the account or of the whole file.
- **When each runs.** A Register rebuilds its rows and choices on its first frame after the generation changes. It rebuilds the rows also when its filter, sort, today's date or locale changes, so each keystroke in the filter bar pays the same cost. The Category List reads usage only while it is open.

### What a Register row costs

Throwaway probes, not kept, timed each formatting call on its own, as divan benchmarks and a release-mode test in `balanced_i18n`:

| Call, en-US | ns |
|---|---|
| `Localizer::format_amount`, building its `CurrencyFormatter` each time | 635–647 |
| The same with a `CurrencyFormatter` built once and reused | 96 |
| `Localizer::format_date` (its formatter is built with the locale) | 124 |

Each row formats three amounts: the signed amount, the unsigned one its column shows, and the running balance. Before the cache that was about 1.95 µs of the 2.2 µs a row took, 1.6 µs of it building formatters; after it, a row takes about 0.62 µs. The rest is the date, the payee, memo and category text the row copies, and the filter's check.

## What changed

`balanced_i18n`'s `Formatters`, which hold the ICU4X formatters for one formatting locale, now also hold an `AmountFormatters` cache (`crates/balanced_i18n/src/money.rs`). It builds each `CurrencyFormatter` the first time an amount needs it and keeps it, by currency, decimal places shown and style. Decimal places are part of the key because an amount with more significant places than its currency shows all of them. A switch of formatting locale builds new `Formatters`, so the cache follows the locale.

## What still grows with the file

- **The Register's rows: 154 ms at 10×.** It formatted every row when one changed, and when only the filter or sort changed. [#580](https://github.com/StanleyGoldman/balanced/issues/580) fixed it; see [After #580](#after-580).
- **The comboboxes' choices: 15.5 ms at 10×.** They walk every transaction in the file for payees and Payee recalls. [#581](https://github.com/StanleyGoldman/balanced/issues/581), measured again once #580 lands: about 7 ms at 4.5× is part of the same one-frame budget.
- **The usage count: 13.4 ms at 10×, while the Category List is open.** [#582](https://github.com/StanleyGoldman/balanced/issues/582) closed it as `wontfix` for now: only an open Category List pays, and adjusting it by each Command's changes meets the two snags #511 found: a `Usage` total's overflow error is sticky, so a subtraction can't undo it, and a transfer's target account needs a lookup.

At 1×, after the cache, a Command's frame is 19 ms, just over one frame at 60 Hz. At a busy household's 30 years (4.5×, see [Sizes](data-file-performance.md#sizes-and-what-decades-means)), the frame comes to about 80 ms, interpolating linearly.

## After #580

Measured 2026-10-27 on the branch for #580, cut from `main` at `964d7a1`, with the base commit's bench binaries and the branch's run back to back. The base ran the branch's `frame.rs`, and its `rebuild.rs` with the same three sizes. Same machine and method as below; the load average read between 1.1 and 1.2.

### What changed

`RegisterModel` keeps the `Arc<AccountSummary>` it was built from and each transaction's formatted cells behind an `Arc`. A rebuild walks the old and new registers from both ends while their `Arc<Transaction>`s are the same allocation, and looks up the transactions left between by pointer, so a transaction moved by a new date keeps its cells too. Only a transaction not in the old register is formatted. Holding the old summary keeps the old transactions alive, so no new one can take an old one's address. A change of locale, of a category's path or of an account's name formats every row again; a new category or account, or a change that leaves every path and name as it was, formats none. The running balance stays a `Money` on the row and is formatted as the row is drawn. A filter or sort change reuses every row's cells, and only filters and sorts again.

### Results

Medians of 30 runs, in milliseconds.

| Bench | Transactions | Before | After |
|---|---|---|---|
| `frame` `after_a_command` | 20,000 | 41.5 | 31.8 |
| | 90,000 | 99.2 | 36.4 |
| | 200,000 | 207 | 46.4 |
| `frame` `filter_keystroke` | 20,000 | 15.8 | 2.03 |
| | 90,000 | 74.4 | 4.81 |
| | 200,000 | 161 | 9.28 |
| `frame` `sort_by_payee` | 20,000 | 20.9 | 7.71 |
| | 90,000 | 102 | 34.1 |
| | 200,000 | 235 | 87.8 |
| `rebuild` `register_rows`, from scratch | 20,000 | 18.9 | 16.8 |
| | 90,000 | 72.6 | 49.0 |
| | 200,000 | 168 | 132 |
| `rebuild` `register_rows_after_a_command` | 20,000 | | 2.00 |
| | 90,000 | | 4.65 |
| | 200,000 | | 12.7 |
| `rebuild` `field_choices` | 90,000 | 14.3 | 14.6 |

- **The rows now fit the frame at 4.5×, but the frame after a Command doesn't: 36.4 ms against #580's target of 16.** The Command's commit is the largest part. Run alone in this window, the `commands` benchmark's flag took 19.8 ms at 1× and 25.8 ms at 10× (4.5× wasn't run), against 3–5 ms in #442's window; [#621](https://github.com/StanleyGoldman/balanced/issues/621) tracks it. The comboboxes' choices are next: `field_choices` took 14.6 ms at 4.5× as its median, but 8.7 ms at its fastest, and its runs split between the two ([#581](https://github.com/StanleyGoldman/balanced/issues/581)). These parts come from separate runs and don't add up to the frame exactly.
- **10× got faster, not slower:** 207 ms to 46 ms.
- **A filter keystroke fits one frame at every size measured.** It takes two frames: the one that takes the key, and the one that filters.
- **A text sort is about two frames at 4.5×.** The collation compares dominate it; [#620](https://github.com/StanleyGoldman/balanced/issues/620) tracks per-row sort keys.
- **Formatting from scratch got cheaper too**, since the running balance is no longer formatted for every row: 72.6 ms to 49.0 ms at 4.5×. Opening an account pays this once.
- **Editing a transfer row costs about 1.8 ms more at 4.5×.** Formatting a transfer's row finds its other side's account through a map of every transaction in the file, which a throwaway probe built in 1.7–1.9 ms at 90,000.
- **This window ran slower than #504's.** The base's `field_choices` took 6.2 ms at 20,000 here against 1.18 ms in #504's run, and the base's frame after a Command 41.5 ms against 19.3 ms. Compare only numbers from the same window.

## Method

### Machine

The same machine as [`data-file-performance.md`](data-file-performance.md#machine): Intel Core Ultra 9 275HX, cpufreq governor `powersave`, Linux 7.0.0-38-generic, rustc 1.99.0, `icu_experimental` 0.6.0. `release` profile, each run pinned to one core with `taskset -c 0`, started after the load average fell below 3. It read between 1.5 and 2 across the runs.

### Benchmarks

Both are committed, in `crates/balanced_ui/benches/`, and need `--features test-support`:

- `taskset -c 0 cargo bench -p balanced_ui --features test-support --bench rebuild` times `RegisterModel::new`, from scratch and from the model before the Command, `FieldChoices::new` and the usage count, through `balanced_ui::testing::build_register_model` and `build_field_choices`. The Register is sorted by date, unfiltered, with the Today divider shown.
- `taskset -c 0 cargo bench -p balanced_ui --features test-support --bench frame` times one `Harness::step` of the App, idle and after a Command, at 1280×720, with no renderer. A `Harness` builds an AccessKit tree every frame, which the app does only while a screen reader is on, so an idle frame here costs more than in the app. That cost doesn't grow with the file.

Each builds the stub with `Ledger::create_stub` into a data file under `target/tmp`, at 20,000 and 200,000 transactions (and 90,000 since #580) in Household Checking, and formats in `en-US` (`Localizer::for_tests`).

For before and after, the base commit's bench binaries were copied out of `target/release/deps` before the change was built, then each pair ran back to back.
