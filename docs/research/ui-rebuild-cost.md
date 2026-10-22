# The UI's rebuild after each Command

Research for [#504](https://github.com/StanleyGoldman/balanced/issues/504). [`data-file-performance.md`](data-file-performance.md) timed the core's write path, and [#442](https://github.com/StanleyGoldman/balanced/issues/442) brought a Command at 10× down to 5–8 ms. This doc times what the UI rebuilds after each Command, which the user waits for too.

Measured 2026-10-22 on the branch for #504, cut from `main` at `ef01571`.

## Answer in brief

- **The UI's rebuild, not the core, sets how long a Command takes.** Before this work, the frame that applied a flag took 52 ms at the stub's size (20,000 transactions) and 499 ms at 10×. The Command itself was 3–5 ms of that.
- **The Register's rows were 85–95% of the frame.** `RegisterModel::new` took 44.5 ms at 1× and 475 ms at 10×. About 90% of that went to building a new ICU4X `CurrencyFormatter` for each of the three amounts on every row.
- **Fixed here: each formatting locale keeps the amount formatters it builds.** The Register now takes 12.4 ms at 1× and 154 ms at 10×, and the frame after a Command 19 ms and 182 ms.
- **10× still needs the Register to rebuild less than every row.** 154 ms is about ten frames. [#580](https://github.com/StanleyGoldman/balanced/issues/580) tracks it.
- **The comboboxes' choices and the usage count are small beside it.** `FieldChoices::new` takes 1.2 ms at 1× and 15.5 ms at 10×, and the usage count 1.3 ms and 13.4 ms. Only an open Category List pays for the usage count. [#581](https://github.com/StanleyGoldman/balanced/issues/581) and [#582](https://github.com/StanleyGoldman/balanced/issues/582) track them.

## Results

All times are medians of 30 runs, in milliseconds. "Before" is the branch's first commit, with the benchmarks and without the formatter cache; "after" adds the cache. Both binaries ran back to back in the same quiet window.

### One frame of the App

`benches/frame.rs` drives the App through `egui_kittest`, showing Household Checking's Register. One frame runs with nothing to do. The other applies a Command that flags or unflags the middle transaction, which moves no balance, and rebuilds what the Command changed.

| Frame | 1× before | 1× after | 10× before | 10× after |
|---|---|---|---|---|
| Idle | 0.45 | 0.46 | 0.46 | 0.45 |
| After a Command | 52.3 | 19.3 | 499 | 182 |

- **An idle frame doesn't grow with the file.** The Register lays out only the rows on screen, and nothing is rebuilt.
- **The frame after a Command is the sum of its parts.** At 10× after: the Command (about 4.6 ms, from the [Commands benchmark](data-file-performance.md#after-442)), the Register's rows (154), the comboboxes' choices (15.5), and an idle frame's work. The Account bar's model and the rest add about 2 ms at 1×.

### Each rebuild alone

`benches/rebuild.rs` times each rebuild on a snapshot no earlier run has read, taken after an untimed flag. Each run includes dropping what it built, as a window drops the old model when it builds a new one.

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

Each row formats three amounts: the signed amount, the unsigned one its column shows, and the running balance. Before the cache that was about 1.95 µs of the 2.2 µs a row took; after it, a row takes about 0.62 µs. The rest is the date, the payee, memo and category text the row copies, and the filter's check.

## What changed

`balanced_i18n`'s `Formatters`, which hold the ICU4X formatters for one formatting locale, now also hold an `AmountFormatters` cache (`crates/balanced_i18n/src/money.rs`). It builds each `CurrencyFormatter` the first time an amount needs it and keeps it, by currency, decimal places shown and style. Decimal places are part of the key because an amount with more significant places than its currency shows all of them. A switch of formatting locale builds new `Formatters`, so the cache follows the locale.

## What still grows with the file

- **The Register's rows: 154 ms at 10×.** It formats every row when one changed, and when only the filter or sort changed. [#580](https://github.com/StanleyGoldman/balanced/issues/580) weighs reformatting only the changed rows, reusing the rest by `Arc<Transaction>` identity, against formatting only the visible rows. The filter matches on formatted text, and three sorts compare it, so either way some text is needed for every row.
- **The comboboxes' choices: 15.5 ms at 10×.** They walk every transaction in the file for payees and Payee recalls. [#581](https://github.com/StanleyGoldman/balanced/issues/581).
- **The usage count: 13.4 ms at 10×, while the Category List is open.** [#582](https://github.com/StanleyGoldman/balanced/issues/582). Adjusting it by each Command's changes meets the two snags #511 found: a `Usage` total's overflow error is sticky, so a subtraction can't undo it, and a transfer's target account needs a lookup.

At 1×, after the cache, a Command's frame is 19 ms, just over one frame at 60 Hz. At a busy household's 30 years (4.5×, see [Sizes](data-file-performance.md#sizes-and-what-decades-means)), the frame comes to about 80 ms, interpolating linearly.

## Method

### Machine

The same machine as [`data-file-performance.md`](data-file-performance.md#machine): Intel Core Ultra 9 275HX, cpufreq governor `powersave`, Linux 7.0.0-38-generic, rustc 1.99.0, `icu_experimental` 0.6.0. `release` profile, each run pinned to one core with `taskset -c 0`, started after the load average fell below 3. It read between 1.5 and 2 across the runs.

### Benchmarks

Both are committed, in `crates/balanced_ui/benches/`, and need `--features test-support`:

- `taskset -c 0 cargo bench -p balanced_ui --features test-support --bench rebuild` times `RegisterModel::new`, `FieldChoices::new` and the usage count, through `balanced_ui::testing::build_register_model` and `build_field_choices`. The Register is sorted by date, unfiltered, with the Today divider shown.
- `taskset -c 0 cargo bench -p balanced_ui --features test-support --bench frame` times one `Harness::step` of the App, idle and after a Command, at 1280×720, with no renderer. A `Harness` builds an AccessKit tree every frame, which the app does only while a screen reader is on, so an idle frame here costs more than in the app. That cost doesn't grow with the file.

Each builds the stub with `Ledger::create_stub` into a data file under `target/tmp`, at 20,000 and 200,000 transactions in Household Checking, and formats in `en-US` (`Localizer::for_tests`).

For before and after, the base commit's bench binaries were copied out of `target/release/deps` before the change was built, then each pair ran back to back.
