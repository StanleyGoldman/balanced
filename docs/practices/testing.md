# Testing

## Pick the lowest layer that can catch the bug

1. **Unit tests in `balanced_core` and `balanced_i18n`**: plain `#[test]`, plus `insta` text snapshots for structured output (formatted amounts, exports, reports). These need no GPU and no egui, and they are what mutation testing checks.
2. **UI interaction tests in `balanced_ui`**: `egui_kittest`'s `Harness`, finding widgets by AccessKit role and label, clicking and typing, then asserting on state or node properties. These need no GPU and run on every OS.
3. **Image snapshot tests**: a few, only for layout-critical views (the Register, the Account bar). See [Image snapshots](#image-snapshots).

Prefer a core unit test to a UI test, and a UI interaction test to an image snapshot.

## Writing tests

- Each test checks one behaviour, with setup, action and checks visibly separated.
- Prefer exact comparisons (`assert_eq!`) to partial ones (`contains`).
- Keep fixtures small and tests fast. Mutation testing runs the suite once for every mutant.
- Test behaviour that could plausibly break, and skip trivial wiring.
- Tests run against a real in-memory `Ledger`, not mocks.
- `unwrap()` passes clippy only inside a `#[test]` function. A helper function in a test file writes `expect("why this can't fail")`.

## UI tests

- Drive the UI the way a user does: `node.click()`, or `focus()` then `type_text()`. Don't write into internal state to set up a UI test. `TextEdit` ignores AccessKit value-setting in egui 0.36.
- `node.click()` clicks the centre of the node's rect, even when the widget is scrolled out of view. In a heavy Dialog's scrolling body, that click lands on the dimming backdrop and cancels the Dialog, and the test fails later with "No nodes found". Scroll the widget into view first: `node.scroll_to_me()`, `harness.run()`, then `click()`.
- Find nodes by role and label (`get_by_role_and_label`), never by screen position.
- A `Label`'s text is its AccessKit value, and its label is empty, so `get_by_label` misses a message shown in a `Label` (a bubble, the Status bar). Find it by role and value: `query_all_by_role(Role::Label)` and `accesskit_node().value()`.
- `accesskit_node()` returns an `accesskit_consumer` node, whose `supports_action` takes a filter as well. To ask whether a widget accepts a click, read the raw node: `node.accesskit_node().data().supports_action(Action::Click)`.
- Labels in tests use the pinned test locale, `en-US`.
- Every run starts on Home, where auto-fold shows the Account bar as a rail. A test that clicks an account first unfolds it with `balanced_ui::testing::unfold_account_bar`, as each test file's `app()` helper does. A test of the saved fold opens the Accounts tab instead, where auto-fold doesn't apply.
- Double-clicks need `with_step_dt(0.01)`. With the default 0.25 s step, the second click arrives too late.
- `egui_kittest` never calls `eframe::App::raw_input_hook`, so a key the hook takes (Tab in the Register) reaches egui's focus navigation in a test as it never does in the app. Send such keys through the hook: take the input with `std::mem::take(harness.input_mut())`, push the event, call `harness.state_mut().raw_input_hook(&ctx, &mut input)`, put the input back, then `step()`. The `send` helper in `crates/balanced_ui/tests/register.rs` does this.
- `Harness::output()` holds only the last frame's output, and `run()` keeps stepping until repaints stop. Output that lasts one frame, such as a `ViewportCommand` sent from `App::logic`, is gone by the time `run()` returns. Step to that frame instead: after a `click()`, one `step()` plays the click, and the next runs the `logic()` that applies the Command it sent.
- `step()` plays each queued event in a frame of its own. `key_press_modifiers` queues four (modifiers down, key down, key up, modifiers up), so one `step()` runs four frames and passes the one-frame output by. To step to a chord's output, queue a single `Event::Key` that carries its modifiers with `harness.event(..)`.
- `Harness::run()` panics if the UI keeps asking for repaints. A UI that animates needs mocked time in its tests.
- Today's date comes from `balanced_ui::Clock`, which the App owns. Build every test App with `Clock::fixed(date)` (feature `test-support`) so it runs the same on any day, and give each test file `required-features = ["test-support"]` in the crate's `Cargo.toml`. `clippy.toml` bans `SystemTime::now`, `Instant::now` and jiff's `now` everywhere else; the prototype harness, which times sessions, is the one exception.

## Image snapshots

The tests live in `crates/balanced_ui/tests/snapshots.rs`, and the images in `crates/balanced_ui/tests/snapshots/`.

- **Source of truth:** Linux with lavapipe, Mesa's Vulkan software rasterizer (`mesa-vulkan-drivers` on Debian and Ubuntu). `balanced_ui::testing::app` accepts no other adapter, so every Linux machine draws the same pixels. On other OSes `cargo xtask snapshots` skips them.
- **Writing one:** build the App with `balanced_ui::testing::app(scheme)` and check it with `check_snapshot`, which holds the project thresholds. Mark the test `#[ignore]`, with a reason naming `cargo xtask snapshots`, so `cargo xtask test` skips it. After clicking, send `Event::PointerGone` and `run()`: kittest draws the pointer and its hover into the image.
- **Running:** `cargo xtask snapshots`.
- **A crash is not a diff:** the snapshot binary can die with `SIGSEGV` (signal 11) inside lavapipe, with no image compared. Rerun `cargo xtask snapshots`. If the crash repeats, investigate it as a bug.
- **Reviewing a failure:** kittest writes `<name>.new.png` and `<name>.diff.png` beside the snapshot (both gitignored). Look at both images and name every changed region. A change the code was meant to make gets updated; anything else is a bug to fix.
- **Updating:** `UPDATE_SNAPSHOTS=1 cargo xtask snapshots` rewrites the failing snapshots. Commit each new PNG with the code that changed it, and say in the PR which views changed and why. A snapshot that fails on noise is fixed in the test setup; the thresholds stay put.

## Mutation testing

`cargo-mutants` mutates only `balanced_core` and `balanced_i18n` (`.cargo/mutants.toml`). Each mutant runs only its own crate's tests, so core logic has to be caught by core's tests.

- Before opening a PR that touches those crates, run `cargo xtask mutants` (needs `cargo install --locked cargo-mutants`). It mutates only the code committed since `origin/main`: commit first, because on uncommitted work it prints `Diff file is empty` and tests nothing. It is advisory: it reports missed mutants but doesn't fail.
- Run it through `cargo xtask mutants`, never `cargo mutants` directly. On Linux the xtask runs it in a systemd scope capped at 8 GB with no swap. A mutant that makes a loop allocate forever fills RAM in seconds, before the per-mutant timeout fires, and an uncapped OOM kill takes the whole terminal session with it. On other OSes there is no cap.
- Write every loop over input so each pass provably consumes something, checked in the loop itself (see `split_number` in `balanced_i18n`'s `money.rs`). A mutant then fails the tests instead of spinning.
- Kill a missed mutant with a test, or exclude it with a reason. Don't leave it.
- Every exclusion (`exclude_re`, `#[mutants::skip]`) carries a comment saying why.
- A weekly full run on `main` is planned for when there is code to mutate.
