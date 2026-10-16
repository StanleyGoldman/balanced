# State: Commands in, snapshots out

The decision and the options that were rejected are in `docs/adr/0001-command-and-snapshot-state-architecture.md`. This doc covers how to write code within that decision.

## Where each kind of state lives

| State | Owner | How it changes |
|---|---|---|
| Accounts, transactions, balances | `Ledger` in `balanced_core` | Only through `Ledger::apply(DomainCommand)` |
| What a window shows (route, selection, sort, filter) | That window's struct | In place, during `ui()` |
| Dialog form fields | `Option<FormState>` on the host window | In place while open. OK sends a Command, Cancel sets it to `None` |
| Edit buffer of the Register row being edited | The Register view | In place. Enter sends a Command |
| Scroll, hover, focus, collapsed sections | egui `Memory` | egui handles them |
| View models (sorted row indices, formatted strings) | The window, cached | Rebuilt when `(generation, sort/filter, locale)` changes |

## Rules

- **A domain change is a `DomainCommand`.** UI code sends `AppCommand::Domain(..)` through the `CommandSender`. It never calls a mutating method on domain data.
- **App-level actions are `AppCommand`s:** navigation (`Navigate(Route)`), `OpenDialog`, `OpenDetached`, `Undo`, `Redo` and `Quit`. The Menu bar and keyboard shortcuts go through the user-action list, which emits these Commands. They don't act directly.
- **Read from the snapshot.** A window reads `Arc<LedgerView>` and never holds a borrow of the `Ledger`. A Detached window, a deferred viewport or a worker thread gets its own clone of the `Arc` and of the `CommandSender`.
- **Commands are applied in one place,** the drain loop in `App::logic`. It hands each `DomainCommand` to the `Ledger`, which validates it and records its inverse for Undo. Persistence will happen there too.
- **Edits that undo together are one Command.** If a user action needs two domain changes that must undo as one step, add a compound `DomainCommand` rather than sending two.
- **Only domain Commands are undoable.** Navigation history belongs to Back/Forward.
- **Rejected Commands come back as a `DomainError`.** The Dialog that sent the Command shows the error inline. Anything else goes to the Status bar.
- **Derived values come from the snapshot.** Running balances and account totals are computed in `balanced_core`. View models format and sort those values but never recompute them.
- **Rebuild caches only when the generation changes.** A view model rebuilds when the snapshot's `generation`, the window's sort or filter, or the locale changes. Nothing is sorted or formatted for every row on every frame.
