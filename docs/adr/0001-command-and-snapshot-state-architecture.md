# Commands in, snapshots out: Balanced's state architecture

Balanced's UI never mutates the financial domain directly. Domain changes, app-level actions (open a Dialog or Detached window, undo, quit) and Main view navigation are sent as Commands over a cloneable `CommandSender` (`std::sync::mpsc`, sending also requests a repaint) and applied once per frame in `App::logic`. Every window reads an immutable `Arc<LedgerView>` snapshot. Only widget-local state (edit buffers, focus, hover, scroll, sort) is mutated in place. We chose this over egui's default of `&mut` everywhere so there is one place for validation, undo and persistence, and so the same code works in the Main window, in deferred viewports and in background workers, none of which can share a borrow.

Decided in [State architecture](https://github.com/StanleyGoldman/balanced/issues/11), building on the [egui/eframe architecture research](../research/egui-app-architecture.md).

## Shape

- **Command types follow the crate boundary.** `balanced_core` owns `DomainCommand` (post/edit transaction, create account, reconcile…), which stays egui-free. `balanced_ui` owns `AppCommand`: `Domain(DomainCommand)`, `Navigate(Route)`, `OpenDialog(..)`, `OpenDetached(..)`, `Undo`, `Redo`, `Quit`. The Menu bar and shortcuts bind to a user-action list (label, accelerator, enabled state) that emits `AppCommand`s; the central keymap that list feeds is decided separately.
- **The domain seam is a concrete `Ledger`**, not a trait: `apply(DomainCommand) -> Result<Applied, DomainError>` and `snapshot() -> Arc<LedgerView>`, backed today by in-memory stub data. Real persistence later replaces its internals behind the same interface. Tests run against a real in-memory `Ledger` rather than mocks.
- **Windows own their UI state.** The Main window and each Detached window hold their own route, selection and edit buffers. A Dialog's form state lives in its host window as an `Option` that exists only while it is open; OK sends a Command, Cancel drops the state. This works whether Detached windows end up immediate or deferred viewports.
- **Errors return to the sender.** `DomainError` is a `thiserror` enum; a rejected Command is shown inline in the Dialog that sent it, otherwise in the Status bar.
- **Undo uses inverse Commands.** `Applied` carries the inverse `DomainCommand`; `balanced_core` keeps the undo/redo stacks. Only domain Commands are undoable. Navigation history belongs to Back/Forward, not undo. Edits that must undo together are a single compound Command.
- **Derived data is cached by generation.** Each snapshot carries a `generation` counter and domain-derived values (running balances, account totals). Each window keeps a view model (sorted/filtered row indices, ICU4X-formatted strings) rebuilt only when `(generation, sort/filter, locale)` changes.

## Considered options

- **Everything through Commands (Elm-style):** rejected. egui widgets take `&mut`, and egui keeps its own UI state in `Memory`, so routing keystrokes and scroll through Commands is boilerplate that can never be complete.
- **Shared `Arc<RwLock<AppState>>`:** rejected. It holds locks across UI code and makes windows contend.
- **`&mut Vec<Command>` threaded through `ui()`:** rejected. Deferred viewports (`'static + Send + Sync` closures) and worker threads can't hold the borrow.
- **Snapshot-restore undo:** rejected. It stops working once the domain persists to a real store.
- **A `Domain` trait with a stub impl:** deferred until a second implementation exists.
