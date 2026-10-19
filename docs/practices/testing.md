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
- A test that needs a write to fail breaks the path, not the permissions. CI runs as root, which writes through a read-only directory or file, so a `chmod` test passes locally and fails in CI. Make the name longer than the 255-byte limit, or put a directory where the file would go.
- `unwrap()` belongs in a `#[test]` function. A helper function in test code writes `expect("why this can't fail")`. Review enforces this, not clippy: `allow-unwrap-in-tests` (`clippy.toml`) lets `unwrap()` through anywhere under `#[cfg(test)]`, helpers included.

## UI tests

- Drive the UI the way a user does: `node.click()`, or `focus()` then `type_text()`. Don't write into internal state to set up a UI test. `TextEdit` ignores AccessKit value-setting in egui 0.36.
- `node.click()` clicks the centre of the node's rect, even when the widget is scrolled out of view. In a heavy Dialog's scrolling body, that click lands on the dimming backdrop and cancels the Dialog, and the test fails later with "No nodes found". Scroll the widget into view first: `node.scroll_to_me()`, `harness.run()`, then `click()`.
- Find nodes by role and label (`get_by_role_and_label`), never by screen position.
- The Register lays out only the rows in view, so AccessKit holds no node for a row scrolled out of it. An assertion that something is absent proves it only in view: open an account short enough to fit, such as Joint Checking, not Household Checking's 20,000 rows, and red-check that the assertion fails once the thing is drawn.
- `get_by_role_and_label` panics when two nodes match, and some labels name two buttons: the top-level tabs Investing and Property & Debt share their names with Account bar headings. Pick a label only one node has (the Banking heading), or query inside the node that holds the one you want, since a kittest node is `Queryable` too: `harness.get_by_role(Role::TabList).get_by_role_and_label(..)`.
- A `Label`'s text is its AccessKit value, and its label is empty, so `get_by_label` misses a message shown in a `Label` (a bubble, the Status bar). Find it by role and value: `query_all_by_role(Role::Label)` and `accesskit_node().value()`.
- A kittest node's `rect()` panics with "Every egui node should have a rect" on a container with no bounds of its own, such as the Status bar (`Role::Status`). Measure a widget inside it instead: `harness.get_by_role(Role::Status).get_by_role_and_label(Role::Label, ..).rect()`.
- `accesskit_node()` returns an `accesskit_consumer` node, whose `supports_action` takes a filter as well. To ask whether a widget accepts a click, read the raw node: `node.accesskit_node().data().supports_action(Action::Click)`.
- Labels in tests use the pinned test locale, `en-US`. A test of another locale uses that locale's labels (see [i18n](i18n.md#tests)).
- Double-clicks need `with_step_dt(0.01)`. With the default 0.25 s step, the second click arrives too late.
- AccessKit carries no text colour. To check a colour (a negative amount), read the last frame's text shapes: `harness.output().shapes`, each `Shape::Text`'s galley sections and `override_text_color`. The theme's negative colour is `visuals.error_fg_color`. `amount_colours` in `crates/balanced_ui/tests/main_window.rs` does this.
- Hover tooltips need the default step. After `node.hover()` and `run()` at 0.25 s, the tooltip shows as a `Label` and in a render. Under `with_step_dt(0.01)` it never rendered, even after `run_steps(100)`.
- `egui_kittest` never calls `eframe::App::raw_input_hook`, so a key the hook takes (Tab in the Register) reaches egui's focus navigation in a test as it never does in the app. Send such keys through the hook: take the input with `std::mem::take(harness.input_mut())`, push the event, call `harness.state_mut().raw_input_hook(&ctx, &mut input)`, put the input back, then `step()`. The `send` helper in `crates/balanced_ui/tests/register.rs` does this.
- `Harness::output()` holds only the last frame's output, and `run()` keeps stepping until repaints stop. Output that lasts one frame, such as a `ViewportCommand` sent from `App::logic`, is gone by the time `run()` returns. Step to that frame instead: after a `click()`, one `step()` plays the click, and the next runs the `logic()` that applies the Command it sent.
- `step()` plays each queued event in a frame of its own. `key_press_modifiers` queues four (modifiers down, key down, key up, modifiers up), so one `step()` runs four frames and passes the one-frame output by. To step to a chord's output, queue a single `Event::Key` that carries its modifiers with `harness.event(..)`.
- `Harness::run()` panics if the UI keeps asking for repaints. A UI that animates needs mocked time in its tests.
- Today's date comes from `balanced_ui::Clock`, which the App owns. Build every test App with `Clock::fixed(date)` (feature `test-support`) so it runs the same on any day, and give each test file `required-features = ["test-support"]` in the crate's `Cargo.toml`. So run one file as `cargo nextest run -p balanced_ui --features test-support --test <file>`; without the feature cargo refuses the target. `clippy.toml` bans `SystemTime::now`, `Instant::now` and jiff's `now` everywhere else; the prototype harness, which times sessions, is the one exception.

## Image snapshots

The tests live in `crates/balanced_ui/tests/snapshots.rs`, and the images in `crates/balanced_ui/tests/snapshots/`.

- **Source of truth:** Linux with lavapipe, Mesa's Vulkan software rasterizer (`mesa-vulkan-drivers` on Debian and Ubuntu). `balanced_ui::testing::app` accepts no other adapter, so every Linux machine draws the same pixels. On other OSes `cargo xtask snapshots` skips them.
- **Writing one:** build the App with `balanced_ui::testing::app(scheme)` and check it with `check_snapshot`, which holds the project thresholds. Mark the test `#[ignore]`, with a reason naming `cargo xtask snapshots`, so `cargo xtask test` skips it. After clicking, send `Event::PointerGone` and `run()`: kittest draws the pointer and its hover into the image.
- **Running:** `cargo xtask snapshots`.
- **A crash is not a diff:** a snapshot test can die with `SIGSEGV` (signal 11) inside lavapipe, with no image compared. nextest runs each test in its own process, so the crash fails only that test, reported as `SIGSEGV` rather than `FAIL`, and the rest still pass or fail on their images. Rerun `cargo xtask snapshots`. A test that crashes again is a bug to investigate.
- **Reviewing a failure:** kittest writes `<name>.new.png` and `<name>.diff.png` beside the snapshot (both gitignored). Look at both images and name every changed region. A change the code was meant to make gets updated; anything else is a bug to fix.
- **Updating:** `UPDATE_SNAPSHOTS=1 cargo xtask snapshots` rewrites the failing snapshots. Commit each new PNG with the code that changed it, and say in the PR which views changed and why. A snapshot that fails on noise is fixed in the test setup; the thresholds stay put.
- **A merge conflict:** git can't merge a PNG, so a snapshot that both sides redrew stops a merge whenever `main` moves. Check out either side (`git checkout --theirs <png>`), regenerate it with `UPDATE_SNAPSHOTS=1 cargo xtask snapshots`, and open the new image to confirm it shows both sides' changes before you `git add` it. A PR whose snapshots overlap a busy area of `main`, such as the Register, can conflict again after each merge, so check `gh pr view <n> --json mergeable` after pushing.

## Mutation testing

`cargo-mutants` mutates only `balanced_core` and `balanced_i18n` (`.cargo/mutants.toml`). Each mutant runs only its own crate's tests, so core logic has to be caught by core's tests.

- CI runs mutation testing after the merge, never on a pull request (ADR 0002). After each merge the `mutants` workflow tests the mutants in the diff that pull request brought into `main` (`cargo xtask mutants-landed`). Each night it tests one of seven round-robin shards of every mutant on `main` (`cargo xtask mutants-shard`), so all of it is tested once a week. Your pre-push gate is `cargo xtask ci` alone.
- Each surviving mutant gets one GitHub issue, labelled `mutant` and `ready-for-agent` (plus `mutant-timeout` for a timeout). The body holds the mutation, a permalink, a link to the pipeline that found it, and the command that runs that one mutant. Later runs comment "found again" with a link to their own pipeline and the line where the mutant now sits, close it when a test catches the mutant, or close it when the mutant is no longer generated. The issue is keyed by file, function and mutation, never by line, so edits elsewhere leave it in place.
- `cargo xtask mutants` is the by-hand tool for proving a mutant's fix: run the issue's reproduce command before and after your test, and the mutant goes from missed to caught. `--re <pattern>` tests every mutant whose name matches, wherever it is; `<base>` limits it to code committed since `<base>` (default `origin/main`). It needs `cargo install --locked cargo-mutants cargo-nextest`, and it reports missed mutants without failing. cargo-mutants 27.1.0 also tests every struct-field deletion mutant (`delete field … from struct …`) on any `--re` run without a diff, because those ignore name filters (cargo-mutants #632, fixed after 27.1.0). Ignore those extra results.
- Run it through `cargo xtask mutants`, never `cargo mutants` directly. On Linux the xtask runs it in a systemd scope capped at 8 GB with no swap. A mutant that makes a loop allocate forever fills RAM in seconds, before the per-mutant timeout fires, and an uncapped OOM kill takes the whole terminal session with it. On other OSes there is no cap.
- Write every loop over input so each pass provably consumes something, checked in the loop itself (see `split_number` in `balanced_i18n`'s `money.rs`). A mutant then fails the tests instead of spinning. Hold that check in a type, as `NonZeroUsize::new(consumed).ok_or(…)?`: a comparison such as `if consumed == 0` is itself a mutant, and flipping it spins the loop until the timeout.
- Kill a missed mutant with a test, or exclude it with a reason. Don't leave it.
- Every exclusion (`exclude_re`, `#[mutants::skip]`) carries a comment saying why.
