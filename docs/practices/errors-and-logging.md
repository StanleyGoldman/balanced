# Errors, panics and logging

## Error types

- **Library crates** (`balanced_core`, `balanced_i18n`, `balanced_ui`, and later `balanced_store`) return typed errors: a `thiserror` enum per crate or per area. The UI matches on these to pick the message, for example "account name already exists" versus "file locked". When a new failure appears, add a variant for it.
- **`anyhow`** is for `balanced` (startup and IO glue) and `xtask`. Add context at each step with `.context("…")`, or `.with_context(|| format!(…))` when building the message costs something.
- Error types implement `std::error::Error + Send + Sync`. A function that can fail returns a real error type, never `()`.
- Error messages are lowercase, with no trailing punctuation.
- Put sensitive data last in an error message, after the description. That covers paths, URLs, account names and amounts.
- Keep the source error. Wrap it in a variant (`#[from]` or `#[source]`) rather than `.map_err(|e| e.to_string())`, and never discard it with `.map_err(|_| …)`.
- Display an `anyhow` error with `{:#}`, which prints the whole chain.
- Invalid configuration or input is an error. Report it rather than falling back to a default. `preferences.toml` is the exception, so that upgrades need no action: a missing key takes its default, an unknown key is ignored, and an unknown font face id reads as egui's face. A file that can't be read at all is still an error, which the App reports in the Status bar.
- When an operation partly succeeds, as an import with skipped rows does, take a `&mut` warnings collector parameter and push structured warnings into it. The extra parameter makes the warnings hard to ignore.

## Handle each error once

Every error is either returned or logged, never both and never neither. The code that finally handles it decides how to show it: inline in the Dialog that sent the Command, or in the Status bar (see [state](state.md)).

## Panics

- A panic means a bug in Balanced. Bad user input, IO and data from disk are errors.
- `unwrap()` is linted outside tests. Where a value truly can't be missing, write `expect("why this can't fail")`. The message states the invariant, and an empty or vague message fails review.
- Prefer `.get()` to indexing (`v[i]`, `&s[a..b]`) unless the index is guaranteed in range by construction. A short comment then says why.
- Use `debug_assert!` for invariants that are expensive to check in release builds.

## Logging

- Libraries emit through `tracing` and nothing else. Only `balanced`'s `main` installs the subscriber. `clippy.toml` bans the subscriber-install functions, and `main` carries the one `#[expect(clippy::disallowed_methods, reason = "…")]`.
- Levels:
  - **ERROR**: something failed and data was lost, or the app can't continue. Libraries usually return `Err` instead.
  - **WARN**: something failed and the app recovered with no data lost. If data was lost, it is an ERROR.
  - **INFO**: rare events a user would care about, mostly logged by application code.
  - **DEBUG**: developer detail.
  - **TRACE**: anything that can fire every frame.
- Log strings with `{:?}` so they come out quoted and escaped.
- Keep financial data out of logs. `#[instrument]` records every argument with `Debug`, so write `#[instrument(skip_all)]` or `skip(..)` and list only harmless fields.
- Use a span for a period of time (an import, a save) and an event for a moment.
