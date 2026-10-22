# State: Commands in, snapshots out

The decision and the options that were rejected are in `docs/adr/0001-command-and-snapshot-state-architecture.md`. This doc covers how to write code within that decision.

## Where each kind of state lives

| State | Owner | How it changes |
|---|---|---|
| Accounts, transactions, balances | `Ledger` in `balanced_core` | Only through `Ledger::apply(DomainCommand)` |
| What a window shows (route, selection, sort, filter) | That window's struct | In place, during `ui()` |
| Dialog form fields | `Option<FormState>` on the host window | In place while open. OK sends a Command, Cancel sets it to `None` |
| The Register's editing row: the open cell's text, and the changes to the row not yet saved | The Register view | In place. Enter saves the row's changes as one Command; Esc throws them away. Leaving a changed row (another row, account, tab or data file, Undo, Redo, closing the window) asks through the leave-row prompt, and the Register sends what left the row as an `AppCommand` once the row is saved or its changes thrown away |
| Scroll, hover, focus | egui `Memory`, kept for the run only (`persist_egui_memory` is off) | egui handles them |
| Preferences | `App`, loaded from `preferences.toml` (`PreferencesFile`) | Only through `AppCommand::SetPreferences`, or `AppCommand::SetRegisterColumns` from the column chooser, which `App::logic` applies and writes; the open Preferences Dialog's draft shows meanwhile |
| Recent files, each with the view (top-level tab and open account) its data file closed on | `App`, loaded from `recent-files.toml` (`RecentFilesFile`) | When a data file opens or is created, `App` saves the closed file's view, moves the new file to the top, writes the file, and shows the new file's saved view. On exit, `App::on_exit` saves the open file's view and writes the file |
| Window layout: Account bar width and fold, folded sections, Register column widths | `Layout` in eframe storage, built from the Main window by `App::save` | In place on the Main window; saved on eframe's timer and on exit |
| View models (sorted row indices, formatted strings) | The window, cached | Rebuilt when `(generation, sort/filter, locale)` changes |

## Rules

- **A domain change is a `DomainCommand`.** UI code sends `AppCommand::Domain(..)` through the `CommandSender`. It never calls a mutating method on domain data.
- **App-level actions are `AppCommand`s:** navigation (`Navigate(Route)`), `OpenDialog`, `OpenDetached`, `Undo`, `Redo` and `Quit`. The Menu bar and keyboard shortcuts go through the user-action list, which emits these Commands. They don't act directly.
- **Read from the snapshot.** A window reads `Arc<LedgerView>` and never holds a borrow of the `Ledger`. A Detached window, a deferred viewport or a worker thread gets its own clone of the `Arc` and of the `CommandSender`.
- **Commands are applied in one place,** the drain loop in `App::logic`. It hands each `DomainCommand` to the `Ledger`, which validates it and records its inverse for Undo. Persistence will happen there too. The one exception is the save of the Register's editing row as the window closes without having asked (`eframe::App::on_exit`): `App::save_editing_row` collects it on a channel of its own and applies it at once through `App::apply_domain`, the same path the drain loop takes, since no later drain runs.
- **Edits that undo together are one Command.** If a user action needs two domain changes that must undo as one step, add a compound `DomainCommand` rather than sending two.
- **A new `DomainCommand` variant joins the undo log's unit tests.** Add it to `variant()` and to `every_command` in `crates/balanced_core/src/undo_log.rs`, and accept the frozen wire snapshot when its diff only adds lines (ADR 0007). A `--test <name>` run compiles only that integration test, so run `cargo nextest run -p balanced_core` whole to reach these.
- **Only domain Commands are undoable.** Navigation history belongs to Back/Forward.
- **Rejected Commands come back as a `DomainError`.** A Dialog with a form stays open until its Command is applied, and shows the error inline. A confirm closes as its Command is sent, so its error goes to the Status bar, like any other.
- **Derived values come from the snapshot.** Running balances and account totals are computed in `balanced_core`. View models format and sort those values but never recompute them.
- **Rebuild caches only when the generation changes.** A view model rebuilds when the snapshot's `generation`, the window's sort or filter, or the locale changes. Nothing is sorted or formatted for every row on every frame.
- **A generation counts within one `Ledger`.** Every `Ledger`, stub, created or opened, starts at `Generation::FIRST`, so after the App swaps in another data file's `Ledger` a cached model can pass as current. Drop every cache built from the old file when the file switches (`MainWindow::data_file_switched`).
