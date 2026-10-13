# egui/eframe app architecture

Research for [#7](https://github.com/StanleyGoldman/balanced/issues/7). This feeds the "State architecture" decision and the shell, Register and Dialog prototypes.

**Question:** How do the official egui/eframe docs and real egui apps structure a large app's state, UI and windows?

**Date:** 2026-10-13. Every claim below was checked against the source tree of the **egui 0.36.2** tag, which is also the eframe 0.36.2 release (both released **2026-09-08**; crates.io `max_version` = 0.36.2). Unreleased 0.37.0 work on `main` is noted where it matters. Production references come from **Rerun 0.39.0** (released 2026-10-12, built on egui/eframe 0.36.2) and the current `eframe_template` (`egui = "0.36.0"`).

Links to source use `https://github.com/emilk/egui/blob/0.36.2/...` unless stated.

---

## TL;DR for later decisions

1. **API baseline (0.34+):** the app implements `eframe::App::ui(&mut self, ui: &mut egui::Ui, frame)` plus an optional `logic(&mut self, ctx, frame)`. `App::update` is gone. `SidePanel` and `TopBottomPanel` are replaced by a single `egui::Panel` (`Panel::top/bottom/left/right`) shown *inside* the root `Ui`. Older tutorials and blog posts are out of date.
2. **State:** egui keeps no app state. The app struct owns everything, and the UI code takes `&mut` to it each frame. Official examples and the demo app mutate state directly for trivial edits. Both the demo app and Rerun route app-level actions ("reset", "open file", "close recording") through a **command enum** that is collected during the frame and run afterwards. Rerun sends commands over a channel and drains them once per frame. For Balanced: do direct mutation for widget-local edit buffers, and route domain mutations (post a transaction, create an account, reconcile) through commands.
3. **Multi-viewport** (real OS windows) works only on native eframe (not web), with both wgpu and glow, on Windows, macOS, Linux X11 and Linux Wayland. However, it has **open, platform-specific bugs**: a Wayland freeze when a child is covered by the main window, Windows repaint starvation of deferred viewports, macOS Space-switch and fullscreen problems, and frame-rate halving per *immediate* viewport. Rerun, the flagship egui app, does **not** use multi-viewport at all. Treat **Detached windows** as a risk to prototype early. Keep a fallback: embedded `egui::Window` or a tab in the Main view.
4. **Dialogs:** `egui::Modal` (since 0.30) is an in-window overlay with a backdrop. It blocks input to the rest of *that viewport* only, and egui has no OS-level modal or owned-window support. That is the right fit for Balanced's **Dialog** concept (confirm, New Account, split). For file open/save, use `rfd`, which gives true native dialogs. Rerun wraps `Modal` in a small `ModalHandler` (an `Option`-held modal with open/close flags), which is a good pattern to copy.
5. **Large lists:** egui lays out everything every frame, so only visible rows may be built. Use `ScrollArea::show_rows`, `egui_extras::TableBuilder` with `body.rows` or `body.heterogeneous_rows`, or Rerun's `egui_table` (0.10.0: millions of rows, sticky columns/header, heterogeneous heights, prefetch hook). Cache formatted strings and sorting/filtering results outside the per-frame path. Idle egui apps don't repaint at all.

---

## 1. Where app state lives and how UI code mutates it

### Official model

- egui does not store app values. "it is responsibility of the egui user to store the state (`value`) so that it persists between frames" ([crates/egui/src/lib.rs, "Understanding immediate mode"](https://github.com/emilk/egui/blob/0.36.2/crates/egui/src/lib.rs)). Widgets take `&mut T` and mutate it directly, for example `ui.add(Slider::new(&mut value, ..))`.
- egui *does* retain small UI state (scroll offsets, window positions, collapsed state) in `Memory`, keyed by `Id` ([README "IDs"](https://github.com/emilk/egui/blob/0.36.2/README.md#ids)). This is persisted when the `persistence` feature is on (`App::persist_egui_memory`).
- **`eframe::App` (0.34+)** ([crates/eframe/src/epi.rs](https://github.com/emilk/egui/blob/0.36.2/crates/eframe/src/epi.rs)):
  - `fn logic(&mut self, ctx: &egui::Context, frame)` runs before each `ui`, and also while the window is hidden if a repaint was requested. You "may NOT show any ui" here. Use it for draining background-work channels and running queued commands.
  - `fn ui(&mut self, ui: &mut egui::Ui, frame)` is the root UI. `Ui` now derefs to `Context`.
  - `fn save(&mut self, storage)` is called on shutdown and every `auto_save_interval` (feature `persistence`). It is fine for window/UI preferences, but **not** for financial data, which belongs in our own store.
  - Close handling: check `ctx.input(|i| i.viewport().close_requested())` and send `ViewportCommand::CancelClose` to veto. See [examples/confirm_exit](https://github.com/emilk/egui/tree/0.36.2/examples/confirm_exit). This is needed for "unsaved changes?" prompts.
  - Changelog: "Replace `App::update` with `fn logic` and `fn ui`" ([#7775](https://github.com/emilk/egui/pull/7775), 0.34.0, 2026-03-26).
- **Background work:** the FAQ says never `.await` in UI code. Talk to worker threads or tasks through channels with `try_recv`, `Arc<Mutex<_>>`, `poll_promise::Promise`, or `tokio::sync::watch`, and call `ctx.request_repaint()` from the worker ([README FAQ "How do I use egui with async?"](https://github.com/emilk/egui/blob/0.36.2/README.md#how-do-i-use-egui-with-async)). `Context` is `Clone + Send`.

### eframe_template

[`src/app.rs`](https://github.com/emilk/eframe_template/blob/main/src/app.rs) has one `TemplateApp` struct with `#[derive(Serialize, Deserialize)]`, loaded in `new()` from `cc.storage` and written in `save()`. `#[serde(skip)]` opts fields out of persistence. All mutation is direct (`self.value += 1.0`). This shows the minimum, not how to structure a large app.

### egui demo app: a command enum, collected and then run

[`crates/egui_demo_app/src/wrap_app.rs`](https://github.com/emilk/egui/blob/0.36.2/crates/egui_demo_app/src/wrap_app.rs):

- `WrapApp { state: State, .. }`. Persisted state is split into a `State` struct, and non-persisted runtime state stays beside it.
- `enum Command { Nothing, ResetEverything }` is marked `#[must_use]`. Inside `ui()`, a `let mut cmd = Command::Nothing;` is threaded as `&mut Command` into the top bar and side panel. After all panels are drawn, `self.run_cmd(ctx, cmd)` applies it. This avoids mutating (or replacing) `self.state` while it is borrowed by the UI code that is drawing it.
- `logic()` forwards to every sub-app's `logic`, even hidden ones.

### Rerun: typed commands over channels, drained once per frame

- [`re_viewer_context/src/command_sender.rs`](https://github.com/rerun-io/rerun/blob/0.39.0/crates/viewer_support/re_viewer_context/src/command_sender.rs): `enum SystemCommand` has dozens of variants, such as load data source, close recording, change route, append to store, and open a modal. A `CommandSender` sends these, plus `UICommand`, `RecordingCommand` and `TableCommand`, over crossbeam channels. UI code anywhere holds a cheap sender and never mutates the stores directly.
- [`re_viewer/src/app/command_handling.rs`](https://github.com/rerun-io/rerun/blob/0.39.0/crates/top/re_viewer/src/app/command_handling.rs): `run_pending_system_commands` / `run_pending_ui_commands` / `run_pending_recording_commands` each loop `while let Some(cmd) = self.command_receiver.recv_*()` and dispatch to `run_system_command` etc. once per frame.
- [`re_ui/src/command/ui_command.rs`](https://github.com/rerun-io/rerun/blob/0.39.0/crates/viewer_support/re_ui/src/command/ui_command.rs): `enum UICommand` is the single list of user-invokable actions. Each variant has `text_and_tooltip()` and per-OS `kb_shortcuts()`, and it feeds the menus, the keyboard shortcuts and the command palette. This maps directly onto a Quicken-style **Menu bar** plus accelerator keys.
- Rerun's data (recordings, blueprints) lives in a `StoreHub` that commands mutate. Per-frame UI gets a read-mostly `ViewerContext`.

### Implication for Balanced

Use a split like Rerun's. `App { ledger: <domain store>, ui: UiState, commands: Sender/Vec<Command> }`. Widgets edit *UI-local buffers* directly (for example the row currently being edited in the Register, or a Dialog's form fields). Committing ("Enter" on a register row, "OK" in New Account) emits a domain `Command`, which is applied after the frame, or in `logic()`, against the store. That gives one place for validation, undo and persistence, and keeps borrows simple. A `UICommand`-style enum can drive both the Menu bar and the shortcuts.

---

## 2. Panels and menu bar organisation

- **0.34 API change:** "`SidePanel` and `TopBottomPanel` are deprecated, replaced by a single `Panel`. … it is now deprecated to use panels directly on `Context`. Use the `show_inside` functions instead, acting on `Ui`s" ([CHANGELOG 0.34.0](https://github.com/emilk/egui/blob/0.36.2/CHANGELOG.md), [#5659](https://github.com/emilk/egui/pull/5659), [#7781](https://github.com/emilk/egui/pull/7781)). Deprecated items were removed in 0.35 ([#8105](https://github.com/emilk/egui/pull/8105)). In 0.36.2 `SidePanel`/`TopBottomPanel` no longer exist in `crates/egui/src/containers/panel.rs`.
- **Order matters:** add the outer panels first, then `CentralPanel` takes the remaining space. The template comment reads "The central panel the region left after adding TopPanel's and SidePanel's".
- **Canonical shell** (eframe_template + demo app), mapped to Balanced terms:

  ```rust
  fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
      egui::Panel::top("menu_bar").show(ui, |ui| {          // Menu bar
          egui::MenuBar::new().ui(ui, |ui| {
              ui.menu_button("File", |ui| { /* … */ });
          });
      });
      egui::Panel::bottom("status_bar").show(ui, |ui| { /* Status bar */ });
      egui::Panel::left("account_bar")                         // Account bar
          .resizable(true)
          .size_range(200..=400)
          .show(ui, |ui| { /* … */ });
      egui::CentralPanel::default().show(ui, |ui| { /* Main view */ });
  }
  ```

  `Panel::left(..).resizable(true).size_range(280..=400).show_collapsible(ui, &mut is_open, ..)` is used verbatim in `wrap_app.rs`. Panels gained drag-to-close (0.35) and drag-to-reopen (0.36), plus double-click on the edge to toggle.
- `egui::MenuBar::new().ui(ui, ..)` with `ui.menu_button(..)` is the menu-bar widget. Inside a menu, `ui.close()` closes the nearest container (menu, popup, `Modal`, `Window`).
- **Tabs / docking**, if the Main view needs it: Rerun uses its own [`egui_tiles`](https://crates.io/crates/egui_tiles) (0.17.1). [`egui_dock`](https://crates.io/crates/egui_dock) (0.21.1) is the main third-party alternative. Neither is needed for a fixed Quicken-classic shell.

---

## 3. Multi-viewport (real OS windows)

### What exists

From the [`viewport` module docs](https://github.com/emilk/egui/blob/0.36.2/crates/egui/src/viewport.rs) and [`Context::show_viewport_deferred`](https://github.com/emilk/egui/blob/0.36.2/crates/egui/src/context.rs):

- It was introduced in 0.24.0 (2023-11-23, [#3172](https://github.com/emilk/egui/pull/3172)). It is "implemented by the native `eframe` backend, but not the web one."
- Viewports are shown *every frame they should exist*, by calling `ctx.show_viewport_*(ViewportId, ViewportBuilder, cb)`. To close one, stop calling it, typically after seeing `i.viewport().close_requested()`.
- **Immediate** (`show_viewport_immediate`): takes a `FnOnce(&mut Ui, ViewportClass)` called inline, so it can borrow `&mut self` directly. "whenever the parent viewports needs to be repainted, so will the child viewport, and vice versa … immediate viewports are simpler to use, but can waste a lot of CPU time."
- **Deferred** (`show_viewport_deferred`): the callback must be `Fn(&mut Ui, ViewportClass) + Send + Sync + 'static`. It is repainted independently, so "You will need to wrap your viewport state in an `Arc<RwLock<T>>` or `Arc<Mutex<T>>`." This is "the most performant type of child viewport." See [examples/multiple_viewports](https://github.com/emilk/egui/blob/0.36.2/examples/multiple_viewports/src/main.rs).
- **Embedded:** when `ctx.embed_viewports()` is true (the web, or a forced setting), the callback is invoked with `ViewportClass::EmbeddedWindow` and drawn inside an `egui::Window` in the parent. So the same code can degrade to an in-window floating window. `ui.set_embed_viewports(true)` forces this at runtime, which is a cheap fallback switch.
- Per-window control goes through `ViewportCommand` (title, size, position, minimise, focus, close, fullscreen) and `ViewportBuilder` (`with_title`, `with_inner_size`, `with_window_level(AlwaysOnTop)`, `with_monitor`, X11 window type, Wayland `app_id`, …).
- **No parent/owner or OS-modal relationship.** `ViewportBuilder` has no `with_parent`/owner/modal option. `ViewportIdPair.parent` is only egui's logical tree. A child window won't stay above the main window or be minimised with it the way a native owned window would. The only knob is `WindowLevel::AlwaysOnTop`, which is above *all* apps. Native window handles for viewports are an open request ([#7798](https://github.com/emilk/egui/issues/7798)).
- Tracking issue [#3556](https://github.com/emilk/egui/issues/3556) (open) still lists missing pieces: automatic positioning ("they currently cover each other"), storing viewport position/size in memory, drag-and-drop between viewports, docking, web support.

### Renderers

- **wgpu is the default** since 0.34.0 ([#7615](https://github.com/emilk/egui/pull/7615)). eframe's default features are `accesskit, default_fonts, links, wayland, web_screen_reader, wgpu, winit/default, x11` ([crates/eframe/Cargo.toml](https://github.com/emilk/egui/blob/0.36.2/crates/eframe/Cargo.toml)). glow is opt-in (`glow` feature + `Renderer::Glow`). The workspace pins wgpu 30, glow 0.17 and winit 0.30.13.
- Both `wgpu_integration.rs` and `glow_integration.rs` implement immediate and deferred viewports. glow uses "one painter shared by all viewports". Both carry a Linux/Wayland resize workaround ([#4196](https://github.com/emilk/egui/issues/4196)).

### Per-platform status (open issues as of 2026-10-13)

| Platform | Status | Open problems relevant to Detached windows |
|---|---|---|
| **Linux Wayland** | Supported (default `wayland` feature). | [#5145](https://github.com/emilk/egui/issues/5145) **app freezes when a viewport is fully covered by the main window** (immediate *and* deferred; GNOME/Mutter; open since 2024, no fix). [#5836](https://github.com/emilk/egui/issues/5836) comment: `swap_buffers` hangs for a window on a non-visible KDE workspace, blocking the other viewports. [#4220](https://github.com/emilk/egui/issues/4220) drag-out-of-maximise doesn't resize. Wayland can't position windows (`with_position` "Android / Wayland: Unsupported"). Monitor targeting only works via borderless fullscreen (`apply_monitor_to_window_attributes` in egui-winit). [#3992](https://github.com/emilk/egui/issues/3992) app icon on GNOME/Wayland. Rerun ships a [Wayland decoration probe](https://github.com/rerun-io/rerun/blob/0.39.0/crates/viewer_support/re_ui/src/wayland.rs) because GNOME advertises server-side decorations but defaults to client-side ones. |
| **Linux X11** | Supported (default `x11` feature). | Fewer open reports. `with_window_type(X11WindowType)` lets a child be a `Dialog`/`Utility` hint to the WM. Some `ViewportBuilder` options are documented "Does not work on X11". The vsync issue below applies. |
| **Windows** | Supported. | [#8466](https://github.com/emilk/egui/issues/8466) **deferred viewport stops repainting while the root has focus** (root cause analysed: `WM_PAINT` goes to the topmost window, so repaint requests from lower windows starve). [#7686](https://github.com/emilk/egui/issues/7686) root viewport doesn't update while the mouse moves over a deferred viewport. [#8470](https://github.com/emilk/egui/issues/8470) showing viewports from `App::logic` while minimised: deferred doesn't appear, immediate **panics** (0.36.1). [#4842](https://github.com/emilk/egui/issues/4842) crash closing a deferred viewport. [#4918](https://github.com/emilk/egui/issues/4918) wrong placement with mixed-DPI monitors. |
| **macOS** | Supported. | [#8085](https://github.com/emilk/egui/issues/8085) Space switch or minimise **destroys immediate viewports**, which reappear centred (0.34.1, glow). [#8259](https://github.com/emilk/egui/issues/8259) opening a child viewport while the root is fullscreen or maximised flickers/aborts the fullscreen transition (wgpu). [#8246](https://github.com/emilk/egui/issues/8246) `ViewportCommand::Minimized(true)` makes the app unresponsive. Live-resize jitter with wgpu/Metal was fixed in 0.33.2 and 0.35 ([#7641](https://github.com/emilk/egui/pull/7641), [#8229](https://github.com/emilk/egui/pull/8229)). |
| **All, both renderers** | | [#5836](https://github.com/emilk/egui/issues/5836) **each immediate viewport waits for vsync separately**, so 2 windows run at ½ FPS, 3 at ⅓ ("occurs in both the glow and wgpu backends"). [#4963](https://github.com/emilk/egui/issues/4963) big FPS loss with a second immediate viewport. [#8683](https://github.com/emilk/egui/issues/8683) cross-viewport hover glitch. |
| **Web** | Not supported (embedded fallback only). | — |

**Production evidence:** a code search of `rerun-io/rerun` finds **no `show_viewport_*` calls**. Rerun is a single window, with internal tiling (`egui_tiles`) and in-window modals. No large production egui app was found that relies on multi-viewport for core workflows.

### Implication for Balanced

Detached windows (report, reconcile) are possible, but they are the riskiest part of the shell. Recommendations:

1. Prototype them early on all three OSes. Use **deferred** viewports if they must stay live independently. **Immediate** viewports are simpler with state, and acceptable for mostly idle finance windows, but they suffer from the vsync halving bug.
2. Design the window code so the same closure can run embedded (`ViewportClass::EmbeddedWindow`), or as a Main view tab, with a setting to fall back.
3. Don't rely on OS-level ownership or modality between our windows. Child windows can fall behind the Main window, which also triggers the Wayland freeze ([#5145](https://github.com/emilk/egui/issues/5145)).
4. Persist window geometry ourselves. egui doesn't yet persist it for child viewports ([#3556](https://github.com/emilk/egui/issues/3556)).

---

## 4. Modal dialogs

- **`egui::Modal`** was added in 0.30.0 (2024-12-16, [#5358](https://github.com/emilk/egui/pull/5358)). Source: [crates/egui/src/containers/modal.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui/src/containers/modal.rs).
  - It is "Similar to a `Window` but centered and with a backdrop that blocks input to the rest of the UI". It is implemented as a foreground `Area` anchored at the centre, painted over `ctx.content_rect()` of the **current viewport**. So it is an in-window overlay, not an OS window, and it does not block other viewports.
  - `Modal::new(id).show(ctx, |ui| ..)` returns a `ModalResponse { response, backdrop_response, inner, is_top_modal, any_popup_open }`. `should_close()` is true on a backdrop click, `ui.close()`, or Escape (only for the topmost modal, and not while a popup is open).
  - Modals stack ("The topmost modal will always be the most recently shown one"). Stacking covers a split dialog opened from within a transaction dialog.
  - The app owns the "is open" state: keep showing it each frame until closed.
  - Not resizable today ([#8579](https://github.com/emilk/egui/issues/8579), open). Size it to content or set sizes explicitly.
- **Rerun's wrapper** [`re_ui/src/modal.rs`](https://github.com/rerun-io/rerun/blob/0.39.0/crates/viewer_support/re_ui/src/modal.rs): `ModalHandler { modal: Option<ModalWrapper>, allow_escape, should_open, should_close }` with `open()`/`close()`/`ui(ctx, make_modal, content)`. The modal exists only while open. Rerun also has a `SystemCommand` variant to open a specific modal, so commands can trigger dialogs. Feature modals live next to their feature (`open_url_modal.rs`, `share_modal.rs`, `server_modal.rs`, `add_view_or_container_modal.rs`).
- **Alternatives:**
  - `egui::Window` is a non-modal floating window inside the viewport, which is what a Quicken "find" panel would be.
  - A child **viewport** is a real OS window, but it is non-modal and unowned (see §3), so it is a poor fit for Dialogs.
  - Native dialogs: the FAQ points to [`rfd`](https://docs.rs/rfd) (async on web) for file dialogs. These are true OS-modal dialogs, which suits import/export file pickers.
  - Confirm-on-quit: `ViewportCommand::CancelClose` + a `Modal` ([examples/confirm_exit](https://github.com/emilk/egui/tree/0.36.2/examples/confirm_exit)).

**Implication for Balanced:** the Dialog concept maps to `egui::Modal` (in the Main window, or in a Detached window for dialogs launched from it), wrapped in a `ModalHandler`-like helper and opened through commands. "Blocks the main window" holds only for the window that shows the modal. File pickers use `rfd`.

---

## 5. Keeping per-frame UI fast for large lists

- **Cost model:** "Since an immediate mode GUI does a full layout each frame … having a very large UI in a scroll area … can be slow … If you … only lay out the part that is in view … the performance hit is generally pretty small … 1-2 ms per frame … `egui` only repaints when there is interaction … or an animation, so if your app is idle, no CPU is wasted" ([README "CPU usage"](https://github.com/emilk/egui/blob/0.36.2/README.md#cpu-usage)). eframe also skips `App::ui` while the window is not visible (0.34, [#7950](https://github.com/emilk/egui/pull/7950)).
- **`ScrollArea::show_rows(ui, row_height, total_rows, |ui, row_range| ..)`**: "Efficiently show only the visible part of a large number of rows". Uniform row height is required. `ScrollArea::show_viewport` gives the visible rect for custom virtualisation ([scroll_area.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui/src/containers/scroll_area.rs)).
- **`egui_extras::TableBuilder`** ([table.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_extras/src/table.rs)): `body.rows(row_height, num_rows, |row| ..)` and `body.heterogeneous_rows(heights_iter, ..)` "will only render the visible rows". `body.row(..)` is the slow path ("⚠️ It is much more performant to use `rows` or `heterogeneous_rows`"). It supports resizable columns, striping and `scroll_to_row`.
- **`egui_table`** ([rerun-io/egui_table](https://github.com/rerun-io/egui_table), 0.10.0): Rerun's table for its dataframe and text-log views. It handles "millions of rows", sticky columns and header, hierarchical headers, expandable rows and heterogeneous heights. It has a delegate trait (`TableDelegate`) with a `prefetch` hook, which suits lazily loaded rows ([re_dataframe_ui/src/re_table.rs](https://github.com/rerun-io/rerun/blob/0.39.0/crates/viewer_support/re_dataframe_ui/src/re_table.rs)).
- **Patterns that follow from the above:**
  - Keep a *view model* for the Register: a `Vec` of row indices after sort and filter, and pre-formatted strings for dates, amounts and running balance. Recompute it only when the data or the sort changes, which a command does, not every frame.
  - Only the edited row gets `TextEdit`s. Other rows are labels or painted text.
  - Running balances are computed in the domain layer, not in the row closure.
  - Use stable `Id`s per row (transaction id) so focus and edit state survive scrolling and resorting. 0.37 deprecates `Id::new` in favour of `Id::unique`/`IdSalt*`, so expect churn there.
  - Profile with `puffin` ([examples/puffin_profiler](https://github.com/emilk/egui/tree/0.36.2/examples/puffin_profiler)).

---

## 6. Other things later decisions should know

- **Release cadence and churn:** 0.33 (2025-10-09), 0.34 (2026-03-26), 0.35 (2026-06-25), 0.36 (2026-08-05), 0.36.2 (2026-09-08), and 0.37 in progress (colour emoji, an experimental `StyleProvider` theming, accessibility trees for popups/modals/panels, and `Panel` constructors taking `impl AsIdSalt`). That is roughly one breaking release per 2–3 months. Pin versions and budget for upgrades. MSRV is 1.95 (0.36).
- **Testing:** `egui_kittest` (in the egui repo) drives apps headlessly with snapshot tests. Rerun uses it, for example `tests/snapshots/open_url_modal__*.png`. The 0.35 `egui_inspection` protocol and `egui_mcp` allow agents to drive a running app (`EGUI_INSPECTION=1`, feature `inspection`).
- **Accessibility:** AccessKit is on by default in eframe.
- **Fonts and theming:** 0.37 (unreleased) makes `default_fonts` non-default and adds system-font loading on native.

## Sources

- egui/eframe 0.36.2 source tree: <https://github.com/emilk/egui/tree/0.36.2> (CHANGELOG.md, crates/eframe/CHANGELOG.md, crates/egui/src/{lib.rs,viewport.rs,context.rs,containers/{modal.rs,panel.rs,scroll_area.rs}}, crates/eframe/src/{epi.rs,native/*_integration.rs}, crates/egui_extras/src/table.rs, crates/egui_demo_app/src/wrap_app.rs, examples/{multiple_viewports,confirm_exit}, README.md)
- egui docs: <https://docs.rs/egui/0.36.2/egui/>, <https://docs.rs/eframe/0.36.2/eframe/>
- eframe_template: <https://github.com/emilk/eframe_template/blob/main/src/app.rs>
- Rerun 0.39.0: <https://github.com/rerun-io/rerun/tree/0.39.0> (files cited inline)
- egui issues: #3556, #4220, #4842, #4918, #4963, #5145, #5836, #7686, #7798, #8085, #8246, #8259, #8466, #8470, #8579, #8683 on <https://github.com/emilk/egui/issues>
- crates.io versions: egui/eframe 0.36.2, egui_table 0.10.0, egui_tiles 0.17.1, egui_dock 0.21.1
