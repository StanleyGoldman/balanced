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

- A Dialog is an overlay in the window that opened it, drawn through the project's modal helper (`balanced_ui`'s `dialog` module). Its form state is an `Option<OpenDialog<Form>>` on the host window and exists only while the Dialog is open. Escape and a backdrop click close it the same way Cancel does, except that a `LightDialog` built with `ignoring_backdrop()` stays open on a backdrop click. Only the welcome Dialog uses that, so a stray click can't quit the App.
- A light Dialog is a `LightDialog`: no title bar, a fixed width, the body above its default button and Cancel. The helper gives the default button first focus and presses it on Enter unless Cancel has focus. `with_accept_enabled(false)` disables the button, which then takes neither, so the body gives first focus to the field that would enable it (the delete confirm's target list does). The helper keeps the Dialog inside the window minus `Theme::DIALOG_MARGIN`, scrolling the body.
- A heavy Dialog is a `HeavyDialog`: an `egui::Window` (title bar with ×, movable, resizable) over a dimming backdrop `Area`. egui's own × reads "Close window" to a screen reader in every language, so the window has no `.open(..)` and `HeavyDialog` paints its own × where egui's sat, labelled through `fl!`. A spacer atom after the title keeps the title centred as it was beside egui's ×. The backdrop is egui's modal layer, so everything below it takes no input and `top_modal_layer()` sees it as it sees an `egui::Modal`. It opens centred, stays inside the window minus `Theme::DIALOG_MARGIN`, and gives the default button first focus unless its body focuses a field. Its layers sit at `Order::Middle`, so the popups in its body (a `ComboBox` list) and the light Dialogs it opens, all `Order::Foreground`, draw and take clicks above it.
- A form Dialog sends its change as `AppCommand::SubmitDialog` and stays open: `App::logic` hands the ledger's answer back through `MainWindow::dialog_applied`, which closes the Dialog or shows the `DomainError` inline.
- No Main window Shortcut acts behind a Dialog: `MainWindow::shortcuts` hands the keymap dispatcher no bindings while one is open. The exception is a Dialog whose changes apply as soon as they're confirmed (today only the Category List). Undo and Redo pass through it, so the user can undo in place, but only while nothing is open over it: its confirms rely on the ledger not changing under them. A Dialog that stages changes until OK (Manage Accounts) keeps blocking every Shortcut.
- Closing returns a `FocusReturn`. Keep it on the host and call `restore` at the start of each frame until it returns `None`. egui keeps a closed modal's layer on top for one more frame and takes focus from any widget drawn below it, so focus given back in the closing frame is lost. The same holds for any `memory.request_focus(id)` made in that frame: request it each frame until the widget's response `has_focus()`, then clear the request. To have the field ready to type over, also store a `TextEditState` whose cursor char range spans the whole text.
- A `ScrollArea` is at least 64 points tall (`min_scrolled_height`) unless told otherwise. Set it to 0 in anything that must fit a small window.
- A modal blocks input only within its own viewport. Write Dialog code so that nothing outside that window needs to be blocked.
- File open and save use `rfd`, which shows native dialogs.
- A Detached window is an egui viewport. Write its UI as a closure that can also run embedded (`ViewportClass::EmbeddedWindow`), so it can fall back to an in-window `egui::Window` while multi-viewport bugs remain open. It can't rely on staying above, or minimising with, the Main window.
- To confirm on close, check `close_requested()` and send `ViewportCommand::CancelClose`.

## Large lists

- Lay out only the visible rows: `ScrollArea::show_rows`, or `TableBuilder` `body.rows`/`heterogeneous_rows`, or `egui_table`. (`TableBody::row` is banned in `clippy.toml`.)
- Only the cell being edited gets a `TextEdit`. Every other cell is a label or painted text.
- Strings shown in a row come pre-formatted from the window's cached view model (see [state](state.md)).
- **Scroll bar over the last column:** egui's default scroll bar floats over the content, so in a `TableBuilder` with a `Column::remainder()` it covers the right-hand column (the Category List's Txns counts read "0|") and the overflow adds a sideways scroll. Set `ui.style_mut().spacing.scroll = ScrollStyle::solid()` before building the table, so the bar takes its own width beside the columns.

## Text entry and editable grids

egui's own focus handling and widgets read keys and clicks before app code does. Most rules below **take the input first**: edit the frame's input ahead of whatever would consume it. These rules are egui mechanics. What a Register cell does on Enter, Tab or Esc is decided in the reference shell spec, not here.

### Keys

- **Esc, ← and → in a `TextEdit`:** build every text field's filter with `FieldKeys` (`crates/balanced_ui/src/field_keys.rs`), as in `.event_filter(FieldKeys::new().into())`, never an `EventFilter` literal. It takes Esc, so the field keeps focus for app code to cancel or revert the edit, and ← and →, so they move the text cursor. `EventFilter::default()` leaves all three false, unlike the filter a `TextEdit` uses when given none: egui then drops the field's focus on Esc, and its focus navigation moves focus to the nearest widget beside the field on ← or →, ending the edit.
- **Esc in a field without that filter:** egui has already taken the field's focus when `ui()` runs, so a handler that checks "one of my fields has focus" never fires. Read Esc while nothing has focus, or take it out of `RawInput` in `raw_input_hook`.
- **↑ and ↓ in a field with a list:** add `.with_vertical_arrows()` to the field's `FieldKeys`, then consume the arrows in app code to move the list's highlight. Otherwise egui reads a bare ↑ or ↓ at the start of the frame and moves focus to the nearest widget above or below, ending the edit. A field that handles Tab itself adds `.with_tab()` the same way.
- **Tab in a grid:** a grid that owns Tab takes it out of `RawInput` in `eframe::App::raw_input_hook`. While no widget has focus, egui's focus navigation acts on Tab before `ui()` runs and moves focus to the next widget outside the grid.
- **Tab stops in a table:** `TableBuilder::sense(Sense::click())` makes every cell a focus stop, since `Sense::click()` is `CLICK | FOCUSABLE`. In an editing row Tab then lands on each cell's background before its `TextEdit`, two Tabs per field. Use `Sense::CLICK` to keep clicks without the Tab stops.
- **Modified keys first:** `consume_key` ignores any Shift or Alt the pattern leaves out, so `consume_key(Modifiers::NONE, Key::Tab)` also eats Shift+Tab. Consume the most specific key first: Shift+Tab before Tab, Alt+↓ before ↓.
- **Bare-letter keys:** an app key with no modifier acts only while `!ctx.egui_wants_keyboard_input()`. Otherwise the letter also lands in the focused field as text.
- **Arrow keys:** while any widget has focus, egui moves focus to the nearest focusable widget in the arrow's direction, anywhere in the window and out of a popup. A popup that steps through its own items takes the arrows out of `RawInput` in `raw_input_hook` while it is open, as the Menu bar does.

### Typing and committing

- **Refusing characters:** remove refused characters from `Event::Text` and `Event::Paste` in `ui.input_mut` before the `TextEdit` is added. Filtering the buffer afterwards flickers and moves the cursor.
- **Cursor after a reformat:** when a commit rewrites the text (a calculator result, a reformatted amount) and focus stays in the field, move the cursor to the end through `TextEditState::load`, `cursor.set_char_range` and `store`. Otherwise the next keystroke lands at the old position: `100/3` becomes `33.33`, and the next `3` makes `333.33`.
- **Locale change:** commit every dirty field's text under the old locale before the locale changes. Otherwise the blur commit rereads it under the new marks, and a rejected `12.345` USD becomes 12,345.00 in `de-DE`.

### Popups and layout

- **Lists in a cell pick on pointer press:** a combobox list or calendar shown under a focused field picks a row on `is_pointer_button_down_on()`, not `clicked()`. egui takes focus from the field when a click is released elsewhere (`SurrenderFocusOn::Clicks`, the default), early in the release frame, so a popup drawn only while the field has focus is gone before its `clicked()` is read.
- **A popup's keys belong to its field:** while the popup is open, the field's own input handling takes Enter and Esc, and the host (the Register, a Dialog) leaves them alone. A host that reads Enter or Esc first commits the field or closes itself instead, and every host of the field needs the same guard. The date field's calendar works this way (`DateEdit::press`).
- **A popup's click outside belongs to it too, in a light Dialog:** the click that closes the popup lands on the Dialog's dimmed window, and `Modal::should_close()` closes the Dialog on it. The popup closes on the press, and the click arrives on the release, often a later frame, so pass `LightDialog::with_body_popup` true while the popup is open and until that press is released. New Account's keypad does this (`BalanceKeypad::Closing`).
- **Keep the field focused across a press on its popup or button:** egui's release still takes the field's focus, and the cell commits on blur. Note a press on the popup (`any_pressed()` inside its `area_rect`) or on a button beside the field, then call `request_focus` on the field every frame until `pointer.any_down()` is false. The segmented date field's calendar and 📅 button do this.
- **A scrolling list in a popup:** give the `ScrollArea` the same `max_height` and `min_scrolled_height`. An `Area` sizes itself from last frame's content, so a list that grows (typed text widening to every entry) shows only its old height of rows. Under `show_rows`, a highlighted row moved past the view isn't laid out and can't `scroll_to_me`: keep the offset from the last frame's `ScrollAreaOutput::state` and set `vertical_scroll_offset` when the highlight moves.
- **One-line bubbles:** a hint or error in an `Area` near the window edge wraps unless its label uses `Label::extend()`.
- **`TextEdit` in a `Grid`:** a `Grid` sizes each column from last frame's widest cell, so a `TextEdit`'s `desired_width` stays narrow until something widens the column. Call `ui.set_min_width(width)` before adding the field.

### Scrolling

- **Keyboard scrolling:** when the cell cursor moves past the visible edge, set `vertical_scroll_offset` (on `TableBuilder` or `ScrollArea`) from the row index times the row pitch. It applies in the same frame, unanimated. `scroll_to_row` applies a frame late and animates, so the highlight lands on a row before the row slides into view.

## Colours and theme

- Colours come from the theme module, which supports light and dark. `clippy.toml` bans `Color32` constructors and `hex_color!`. The theme module is the one place that builds colours, under a module-level `#[expect(clippy::disallowed_methods, reason = "the theme module defines the palette")]` (and `disallowed_macros` if it uses `hex_color!`).
- How widgets take their colours and sizes from the theme is in [theme](theme.md).

## Accessibility

egui_kittest finds widgets through the same AccessKit tree a screen reader uses, so a widget that tests can't find is also invisible to a screen reader.

- Every interactive custom-painted widget (Register rows, Account bar entries) calls `Response::widget_info` with the correct role and label.
- Every text field has a visible label, or is connected to one with `.labelled_by(label.id)`.
- egui gives a widget's AccessKit node its bounds only when the widget is focusable. A widget interacted with `Sense::CLICK`, which is clickable but no Tab stop (a tree row other than the selected one, a fold triangle), has a node with no rect, so a screen reader can't locate it and kittest's `click()` panics with "Every egui node should have a rect". Give such a node its bounds in `accesskit_node_builder` with `node.set_bounds(..)` from `response.rect`, or call `widget_info`, which fills them. `name_cell` in `crates/balanced_ui/src/category_list.rs` does the first.
- Mark the chosen item of a set (a calendar's marked day, a list's highlighted row) with `ctx.accesskit_node_builder(response.id, |node| node.set_selected(true))`. egui writes `WidgetInfo::selected` to AccessKit as *toggled*, a checkbox's checked state, so a test reading `is_selected()` sees `None`.
- A right-to-left layout (`Sides`' right side, `ui.horizontal` inside it) reaches AccessKit in the order widgets are added, so a screen reader reads its widgets in reverse of what the screen shows. Draw a name and its value as one `Label`, a `LayoutJob` built from one Fluent message, as the context header does with Current and Ending (`header_balance` in `main_window.rs`).

## Upgrading egui

egui releases a breaking minor version about every two months, and it removes deprecated items one minor later. To upgrade:

1. Wait for the first patch release (x.y.1), or about two weeks after the minor release.
2. Check that every egui-family dependency (`egui_table`, `egui_plot`, …) has a release for the new minor. Don't `[patch]` to git.
3. On the current version, clear every deprecation warning.
4. Bump all egui-family crates together in `[workspace.dependencies]`. Read the "Breaking" and "Changed" sections of the egui and egui_kittest changelogs.
5. Run `cargo xtask ci`. Expect image snapshot changes, and review and update them as [testing](testing.md#image-snapshots) says.
6. Make one PR per egui minor, and never skip a minor.
7. If the new egui needs a newer Rust, bump the toolchain first (see [tooling](tooling.md#toolchain)).

## Gotchas

- An auto-sized `Window` (`.auto_sized()`) offers its contents all the height it may grow to. A `Layout::right_to_left(Align::Center)` button row centres itself in that height, and the window stretches to the bottom of the screen. Give the row its own height with `ui.allocate_ui(vec2(ui.available_width(), row_height), ..)`, as a fitted `HeavyDialog` does.
- egui's default proportional fonts (Ubuntu-Light, NotoEmoji, emoji-icon-font) lack ▸ ▾ ▼ ⋯, the ✕ close cross, the arrows ← → ↑ ↓, braille such as the ⠿ grip, and box-drawing such as ─, and draw a box in their place; ⏵ ⏶ ⏷ ◀ ▶ ⚙ ⚑ render. For a missing glyph, paint the shape with the `Painter` (`Shape::convex_polygon` for a triangle, `circle_filled` dots for a grip, `hline` for a rule, two `line_segment`s for a close cross). Check a glyph with `fc-query --format='%{charset}' <font>` against the fonts in the `epaint_default_fonts` crate.
- A layer raised with `ctx.move_to_top` every frame lands above every other layer of its `Order` that didn't ask this frame. A popup asks only in the frame it opens, so from its second frame it sinks under a same-order layer raised every frame, and kittest's `click()`, which clicks at the node's position, hits the widget beneath. Raise every frame only a layer whose children open at a higher `Order`, as `HeavyDialog` does at `Order::Middle`; a `LightDialog`, at `Order::Foreground` with its date calendar, stays where egui puts it.
- Read every `ctx` value a closure needs before calling `ctx.fonts_mut`, `ctx.memory` or `ctx.input`: `let heading = TextStyle::Heading.resolve(&ctx.global_style());`, then `ctx.fonts_mut(|f| f.row_height(&heading))`. A `ctx` call inside such a closure takes the context lock again, and the test panics after 10 s with `Failed to acquire RwLock read after 10s. Deadlock?`.
- A `Ui::new(ctx, id, ..)` registers a widget under its own `id`. Give each widget you `interact` inside it a different id (`heavy.with("close_ui")` for the `Ui`, `heavy.with("close")` for the button). A shared id leaves the button unclickable, with no warning.
- `egui::MenuBar` takes the whole row. Anything sharing the row (tabs, a search box) goes inside its closure.
- An egui menu closes on a pointer click only. Enter or Space on a focused menu item clicks it and leaves the menu open, so a menu item calls `ui.close()` when clicked.
- A focused widget that isn't drawn in a frame loses focus in that frame. A menu item that switches the view away from its menu (the Register's Go to) leaves nothing focused, with no `surrender_focus` call. A test of such a call can't go red, since removing it changes nothing.
- A disabled widget never holds focus: egui drops focus from it in the frame it gets it. A Tab order the app steps through itself skips its disabled widgets, as the Register's editing row passes over the draft row's disabled More. Focus moved onto one lands on nothing, and the next Tab acts as if nothing had focus.
- Tab follows the order widgets are drawn in, not their place on screen. A `Panel::bottom` footer drawn before its `CentralPanel` list comes before the list in Tab order. To pin a focusable footer under a scrolling list, draw the list first into a `scope_builder` rect that leaves room for the footer's height from the last frame, then the footer, and `request_discard` when that height changes (the Account bar's `open_bar`). A footer with nothing to focus can stay a nested `Panel::bottom` followed by a `CentralPanel`.
- On Wayland, winit 0.30 ignores window levels and parent windows, so no second OS window can be kept above the Main window there.
