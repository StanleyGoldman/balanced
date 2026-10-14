# egui and eframe

This doc applies to egui 0.36. Tutorials and examples written before 0.34 use APIs that no longer exist, so check anything copied from them against the 0.36 docs. The background is in `docs/research/egui-app-architecture.md`.

## App shape

- The app implements `eframe::App::logic` (drain Commands, poll workers; no UI) and `eframe::App::ui` (draw). `App::update` is gone.
- Panels are `egui::Panel::top/bottom/left/right(..).show(ui, ..)`, shown inside the root `Ui`. Add the outer panels first (Menu bar, Status bar, Account bar) and the `CentralPanel` (Main view) last. `SidePanel` and `TopBottomPanel` no longer exist.
- The Menu bar is `egui::MenuBar` with `ui.menu_button`, drawn inside the Main window on every platform. Menu items come from the user-action list (see [state](state.md)).
- Code in `ui()` never blocks or `.await`s. Slow work goes to a named worker thread (`std::thread::Builder`), which reports back through a channel and calls `ctx.request_repaint()`.
- Financial data never goes into eframe's `save()` storage. That storage is for window and UI preferences only.

## IDs

- Derive widget IDs from stable data with a hashed tuple: `ui.make_persistent_id(("register_row", txn_id))`. Never build them with `format!`.
- Give a list row an ID based on the record's ID, not its position, so focus and edit state survive scrolling and re-sorting.

## Dialogs and Detached windows

- A Dialog is an `egui::Modal` in the window that opened it, wrapped in the project's modal helper. Its form state is an `Option` on the host window and exists only while the Dialog is open. Escape and a backdrop click close it the same way Cancel does.
- A modal blocks input only within its own viewport. Write Dialog code so that nothing outside that window needs to be blocked.
- File open and save use `rfd`, which shows native dialogs.
- A Detached window is an egui viewport. Write its UI as a closure that can also run embedded (`ViewportClass::EmbeddedWindow`), so it can fall back to an in-window `egui::Window` while multi-viewport bugs remain open. It can't rely on staying above, or minimising with, the Main window.
- To confirm on close, check `close_requested()` and send `ViewportCommand::CancelClose`.

## Large lists

- Lay out only the visible rows: `ScrollArea::show_rows`, or `TableBuilder` `body.rows`/`heterogeneous_rows`, or `egui_table`. (`TableBody::row` is banned in `clippy.toml`.)
- Only the row being edited gets `TextEdit`s. Other rows are labels or painted text.
- Strings shown in a row come pre-formatted from the window's cached view model (see [state](state.md)).

## Colours and theme

- Colours come from the theme module, which supports light and dark. `clippy.toml` bans `Color32` constructors and `hex_color!`. The theme module is the one place that builds colours, under a module-level `#[expect(clippy::disallowed_methods, reason = "the theme module defines the palette")]` (and `disallowed_macros` if it uses `hex_color!`).

## Accessibility

egui_kittest finds widgets through the same AccessKit tree a screen reader uses, so a widget that tests can't find is also invisible to a screen reader.

- Every interactive custom-painted widget (Register rows, Account bar entries) calls `Response::widget_info` with the correct role and label.
- Every text field has a visible label, or is connected to one with `.labelled_by(label.id)`.

## Upgrading egui

egui releases a breaking minor version about every two months, and it removes deprecated items one minor later. To upgrade:

1. Wait for the first patch release (x.y.1), or about two weeks after the minor release.
2. Check that every egui-family dependency (`egui_table`, `egui_plot`, …) has a release for the new minor. Don't `[patch]` to git.
3. On the current version, clear every deprecation warning.
4. Bump all egui-family crates together in `[workspace.dependencies]`. Read the "Breaking" and "Changed" sections of the egui and egui_kittest changelogs.
5. Run the tests. Expect snapshot changes, regenerate them on the snapshot source-of-truth machine, and review every image diff.
6. Make one PR per egui minor, and never skip a minor.
7. If the new egui needs a newer Rust, bump the toolchain first (see [tooling](tooling.md#toolchain)).

## Gotchas

- egui's default proportional fonts (Ubuntu-Light, NotoEmoji, emoji-icon-font) lack ▸ ▾ ▼ ⋯ and the arrows ← →, and draw a box in their place; ⏵ ⏷ ◀ ▶ render. Check a glyph with `fc-query --format='%{charset}' <font>` against the fonts in the `epaint_default_fonts` crate.
- `egui::MenuBar` takes the whole row. Anything sharing the row (tabs, a search box) goes inside its closure.
- To pin a footer under a scrolling list in a side panel, nest a `Panel::bottom` and then a `CentralPanel` inside the side panel.
- On Wayland, winit 0.30 ignores window levels and parent windows, so no second OS window can be kept above the Main window there.
