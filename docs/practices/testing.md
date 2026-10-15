# Testing

## Pick the lowest layer that can catch the bug

1. **Unit tests in `balanced_core` and `balanced_i18n`**: plain `#[test]`, plus `insta` text snapshots for structured output (formatted amounts, exports, reports). These need no GPU and no egui, and they are what mutation testing checks.
2. **UI interaction tests in `balanced_ui`**: `egui_kittest`'s `Harness`, finding widgets by AccessKit role and label, clicking and typing, then asserting on state or node properties. These need no GPU and run on every OS.
3. **Image snapshot tests**: a few, only for layout-critical views (the Register, the Account bar). Their check (Linux with the lavapipe software rasterizer as the source of truth) doesn't exist yet. It arrives with the first layout-critical view, together with its procedure in this doc.

Prefer a core unit test to a UI test, and a UI interaction test to an image snapshot.

## Writing tests

- Each test checks one behaviour, with setup, action and checks visibly separated.
- Prefer exact comparisons (`assert_eq!`) to partial ones (`contains`).
- Keep fixtures small and tests fast. Mutation testing runs the suite once for every mutant.
- Test behaviour that could plausibly break, and skip trivial wiring.
- Tests run against a real in-memory `Ledger`, not mocks.

## UI tests

- Drive the UI the way a user does: `node.click()`, or `focus()` then `type_text()`. Don't write into internal state to set up a UI test. `TextEdit` ignores AccessKit value-setting in egui 0.36.
- Find nodes by role and label (`get_by_role_and_label`), never by screen position.
- Labels in tests use the pinned test locale, `en-US`.
- Double-clicks need `with_step_dt(0.01)`. With the default 0.25 s step, the second click arrives too late.
- `Harness::output()` holds only the last frame's output, and `run()` keeps stepping until repaints stop. Output that lasts one frame, such as a `ViewportCommand` sent from `App::logic`, is gone by the time `run()` returns. Step to that frame instead: after a `click()`, one `step()` plays the click, and the next runs the `logic()` that applies the Command it sent.
- `step()` plays each queued event in a frame of its own. `key_press_modifiers` queues four (modifiers down, key down, key up, modifiers up), so one `step()` runs four frames and passes the one-frame output by. To step to a chord's output, queue a single `Event::Key` that carries its modifiers with `harness.event(..)`.
- `Harness::run()` panics if the UI keeps asking for repaints. A UI that animates or shows a clock needs mocked time in its tests. When the injectable clock lands, `clippy.toml` will ban `SystemTime::now` and `Instant::now` outside it.
- When the project snapshot helper (`balanced_ui::testing`) lands, add a `clippy.toml` ban on the raw kittest snapshot methods so that every snapshot uses the project thresholds.

## Mutation testing

`cargo-mutants` mutates only `balanced_core` and `balanced_i18n` (`.cargo/mutants.toml`). Each mutant runs only its own crate's tests, so core logic has to be caught by core's tests.

- Before opening a PR that touches those crates, run `cargo xtask mutants` (needs `cargo install --locked cargo-mutants`). It mutates only the code committed since `origin/main`: commit first, because on uncommitted work it prints `Diff file is empty` and tests nothing. It is advisory: it reports missed mutants but doesn't fail.
- Run it through `cargo xtask mutants`, never `cargo mutants` directly. On Linux the xtask runs it in a systemd scope capped at 8 GB with no swap. A mutant that makes a loop allocate forever fills RAM in seconds, before the per-mutant timeout fires, and an uncapped OOM kill takes the whole terminal session with it. On other OSes there is no cap.
- Write every loop over input so each pass provably consumes something, checked in the loop itself (see `split_number` in `balanced_i18n`'s `money.rs`). A mutant then fails the tests instead of spinning.
- Kill a missed mutant with a test, or exclude it with a reason. Don't leave it.
- Every exclusion (`exclude_re`, `#[mutants::skip]`) carries a comment saying why.
- A weekly full run on `main` is planned for when there is code to mutate.
