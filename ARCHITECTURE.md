# Architecture

Balanced is a desktop personal-finance app in the tradition of Quicken classic, built in Rust on egui/eframe. A single **Main window** holds a Menu bar, an Account bar, a Main view and a Status bar. **Dialogs** are short-lived modal overlays, and **Detached windows** are separate OS windows. `CONTEXT.md` defines these terms.

## Bird's-eye view

The UI never mutates the domain. Each frame:

1. Widgets edit their own local state (edit buffers, focus, scroll).
2. Committing an edit sends a Command over a `CommandSender`.
3. `App::logic` drains the Commands and applies them to the `Ledger`.
4. The `Ledger` publishes a new immutable `Arc<LedgerView>` snapshot, which every window reads.

The reasons are recorded in `docs/adr/0001-command-and-snapshot-state-architecture.md`.

## Codemap

The workspace is flat. Each crate sits in `crates/<name>/`, except `xtask/`.

- **`balanced`**: the binary. It installs the `tracing` subscriber, calls `eframe::run_native`, and turns startup failures into `anyhow` errors. It holds no logic of its own.
- **`balanced_ui`**: everything egui. It holds the Main window and its panels, the Register view, Dialogs, Detached windows, the theme (the one place that builds colours), `AppCommand`, and the user-action list that feeds the Menu bar and shortcuts.
- **`balanced_core`**: the financial domain. It holds accounts, transactions, money, `DomainCommand`, the `Ledger` (`apply` and `snapshot`), undo and redo, and `DomainError`.
- **`balanced_i18n`**: message lookup, the locale fallback chain, and locale-aware formatting of numbers, dates and money.
- **`balanced_prototype_harness`**: tooling for prototypes, not part of the app. It provides the event log, screenshots, F2/F3 notes and markers, and the agent's control file (see `docs/agents/prototypes.md`). It depends on egui but knows nothing of Balanced's domain. Only prototype examples use it, as a dev-dependency.
- **`xtask`**: repo automation, run as `cargo xtask <task>`, including every check (`cargo xtask ci`).

A persistence crate, `balanced_store`, comes later, below `balanced_core`.

## Invariants

- **`balanced_core` and `balanced_i18n` don't depend on egui or eframe.** `deny.toml` bans both crates, and lists in `wrappers` the only crates allowed to depend on them directly.
- **`balanced_core` and `balanced_i18n` are siblings.** Neither depends on the other. `balanced_ui` formats core values through `balanced_i18n`.
- **Dependencies point down only:** `balanced` → `balanced_ui` → {`balanced_core`, `balanced_i18n`}. When a second layer rule appears (for example, for `balanced_store`), add an `xtask` layer check over `cargo metadata`.
- **The domain changes only inside `Ledger::apply`.** No UI code holds `&mut` access to domain data.
- **`balanced_prototype_harness` stays out of the app.** Only `examples/` take it, as a dev-dependency. No crate lists it under `[dependencies]`.
- **Only the binary installs a tracing subscriber.** `anyhow` appears only in `balanced` and `xtask`.
- **Colours are built only in the theme module.**
