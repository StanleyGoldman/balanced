# Testing and tooling for egui

Research for [#8](https://github.com/StanleyGoldman/balanced/issues/8), a child of map [#5](https://github.com/StanleyGoldman/balanced/issues/5). Feeds "Adopt coding practices" ([#12](https://github.com/StanleyGoldman/balanced/issues/12)): the root `CODING_STANDARDS.md` and `docs/practices/<topic>.md`.

**Question:** What testing and tooling do public sources and real egui projects use, and how does each fit a GUI codebase?

**Date:** 2026-10-13. Versions checked against source, not blog posts:

| Thing | Version | Released | Read from |
|---|---|---|---|
| egui / eframe / egui_kittest | 0.36.2 (MSRV 1.95) | 2026-09-08 | [emilk/egui @ 0.36.2](https://github.com/emilk/egui/tree/0.36.2) |
| kittest | 0.4.0 (used by egui_kittest 0.36); 0.5.0 out 2026-10-05, already on egui `main` | | [rerun-io/kittest @ 0.4.0](https://github.com/rerun-io/kittest/tree/0.4.0) |
| accesskit | 0.24.1 in egui 0.36; 0.25 on egui `main` | | egui `Cargo.toml` |
| cargo-mutants | 27.1.0 (MSRV 1.88); `main` has unreleased changes | 2026-06-02 | [sourcefrog/cargo-mutants](https://github.com/sourcefrog/cargo-mutants), book at [mutants.rs](https://mutants.rs) |
| `mutants` attribute crate | 0.0.4 | 2026-01-15 | crates.io |
| cargo-deny | 0.20.2 | 2026-07-09 | crates.io |
| cargo-nextest | 0.9.148 | 2026-10-08 | crates.io |
| insta | 1.49.0 | 2026-10-03 | crates.io |
| Rerun (production reference) | 0.39.0, on egui 0.36.2 | 2026-10-12 | [rerun-io/rerun @ 0.39.0](https://github.com/rerun-io/rerun/tree/0.39.0) |
| eframe_template | `main`, `egui = "0.36.0"` | | [emilk/eframe_template](https://github.com/emilk/eframe_template) |

Builds on [`egui-app-architecture.md`](egui-app-architecture.md) (#7) and aligns with the sibling [`rust-project-structure.md`](https://github.com/StanleyGoldman/balanced/pull/24) (#24, open): crates `balanced` (bin), `balanced_ui`, `balanced_core`, `balanced_i18n`, `xtask`; pinned toolchain = `rust-version`; clippy `pedantic` + cherry-picked `restriction`; `cargo-deny` keeps egui out of the lower crates. Lints, rustfmt and MSRV policy are settled there and not repeated here.

Each finding is tagged:

- **[config]**: enforce with `Cargo.toml`, `clippy.toml`, `kittest.toml`, `.cargo/mutants.toml`, `deny.toml`, `.gitattributes` or CI. Do not write prose for it.
- **[standard]**: a review rule for `CODING_STANDARDS.md` or `docs/practices/testing.md`.

---

## TL;DR for later decisions

1. **Three test layers, split by crate.**
   - `balanced_core` and `balanced_i18n`: plain `#[test]` unit tests plus `insta` text snapshots. No GPU, no egui. These are the mutation-testing target.
   - `balanced_ui`: `egui_kittest` *interaction* tests (find widgets through AccessKit, click, type, assert on state). These need no GPU and run on every OS.
   - A small number of `egui_kittest` *image snapshot* tests for layout-critical views (Register, Account bar). These need wgpu, so they run on one "source of truth" CI platform.
2. **egui_kittest queries widgets through AccessKit**, the same tree screen readers see. A widget a test can't find by role and label is also invisible to a screen reader. Custom-painted widgets (Register rows, the Account bar) must call `Response::widget_info` and `labelled_by`. That makes accessibility a test requirement, not an extra. **[standard]**
3. **Snapshot source of truth: Linux CI with a software rasterizer (lavapipe).** This is Rerun's setup. egui itself uses macOS CI on a real GPU instead, which is costly for a private repo and isn't a first-class Balanced platform. Snapshots go in Git LFS, `*.new.png`/`*.diff.png` are git-ignored, and a `workflow_dispatch` job commits updated snapshots from a CI artifact. **[config]**
4. **cargo-mutants:**
   - Mutate only `balanced_core` and `balanced_i18n` (`--package` or `examine_globs`). Leave `balanced_ui`, `balanced` and `xtask` out.
   - On PRs, run `--in-diff` against the base branch. Weekly, run a full sharded run on `main`.
   - Use a `[profile.mutants]` with `debug = "none"` and `--in-place` in CI. **[config]**
   - The PR job's blocking policy is an open decision (neqo: non-blocking summary; vodozemac: blocking).
5. **CI (GitHub Actions):**
   - `fmt` → `clippy -D warnings` → `test` (Linux and Windows) → snapshot job (Linux + lavapipe) → `cargo deny check` → mutants.
   - Toolchain comes from `rust-toolchain.toml`. Cache with `Swatinem/rust-cache@v2`. Install tools with `taiki-e/install-action`. Pass `--locked` everywhere.
   - A separate MSRV job is redundant while toolchain = `rust-version` (#24). **[config]**
6. **egui upgrades:**
   - egui ships a breaking 0.x minor about every two months (0.34 Mar, 0.35 Jun, 0.36 Aug 2026).
   - Deprecated items are **removed one minor later** (0.35 "Remove everything that was marked `#[deprecated]`"). So never skip a minor: upgrade one minor at a time, and clear deprecation warnings before the next.
   - Pin all egui-family crates in `[workspace.dependencies]` at one version. Commit `Cargo.lock`. Let `cargo-deny` `multiple-versions = "deny"` catch a split egui/accesskit graph. **[config + standard]**

---

## 1. egui_kittest

### What it is

- egui_kittest is a "Ui testing library for egui, based on kittest (an AccessKit based testing library)" ([README](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/README.md)).
  - It lives in the egui repo and is versioned in lock-step with egui (0.36.2).
  - kittest itself is framework-agnostic and "inspired by Testing Library" ([kittest README](https://github.com/rerun-io/kittest/blob/0.4.0/README.md)).
- **Features** ([Cargo.toml](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/Cargo.toml)):
  - `wgpu`: test renderer.
  - `snapshot`: image comparison via `dify`.
  - `eframe`: test a whole `eframe::App`, which turns on `eframe/accesskit`.
  - With no features you get interaction tests only, and no GPU stack is linked.
- **Harness constructors** ([lib.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/lib.rs), [builder.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/builder.rs)):
  - `Harness::new_ui(|ui| …)` and `Harness::new_ui_state(|ui, state| …, state)`, then `harness.state()` / `state_mut()`.
  - `Harness::builder().with_size(..).with_pixels_per_point(..).with_theme(..).with_os(..).wgpu().build_ui_state(..)`.
  - `build_eframe(|cc| MyApp::new(cc))` for a full app, which needs the `eframe` feature. The egui demo app is tested this way ([test_demo_app.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_demo_app/tests/test_demo_app.rs)).
- **Driving frames:**
  - `step()` runs one frame.
  - `run()` runs until animations finish and no repaint is requested. It **panics** after `max_steps`, which defaults to 4. `try_run()` and `run_ok()` are the non-panicking versions.
  - `step_dt` defaults to 0.25 s ("low so we don't waste cpu waiting for animations").
  - A UI that requests repaints forever (a spinner, a ticking clock) will make `run()` fail. Mock time, as the demo app does with `mock_time`.
- **Input:**
  - `Node::click/hover/focus/type_text/scroll_to_me/click_accesskit`.
  - `Harness::key_press(_modifiers)`, `key_combination`, `drag_at/drop_at`, `event(..)`.
  - `with_os` defaults to `OperatingSystem::Nix`, so shortcut text ("Ctrl+S" vs "⌘S") is stable across machines ([builder.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/builder.rs)).

### Interaction tests (no GPU)

The pattern from egui's own tests ([text_edit.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_demo_lib/src/demo/text_edit.rs)):

```rust
let mut harness = Harness::new_ui_state(|ui, text| { ui.text_edit_singleline(text); }, String::new());
let edit = harness.get_by_role(accesskit::Role::TextInput);
edit.focus();
edit.type_text("Hi");
harness.run();
assert_eq!(harness.state(), "Hi");
```

- These need no display server and no GPU. `Harness` drives `egui::Context` directly and reads `platform_output.accesskit_update` ([lib.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/lib.rs)). They are ordinary `cargo test`s on Linux, Windows and macOS CI.
- **Rerun's test guidance** ([TESTING.md](https://github.com/rerun-io/rerun/blob/0.39.0/TESTING.md)), all good **[standard]** candidates:
  - "Test the viewer through its UI (e.g. clicking buttons in an `egui_kittest` harness) rather than by writing to internal state."
  - "one behavior per test, with setup, action, and checks visibly separated"
  - "Prefer an exact comparison (`assert_eq!`) over a partial one (`contains`)."
  - "Keep fixtures small … keep tests fast."
  - "Test behavior that could plausibly break. Don't add tests for … trivial UI shortcuts."
- **Known gotchas:**
  - **Double-click** can't be produced from queued events with the defaults: each event advances time by `step_dt` (0.25 s), so the second press lands at least 0.75 s after the first, past egui's 0.3 s double-click delay. Workaround: `with_step_dt(0.01)` ([egui#8697](https://github.com/emilk/egui/issues/8697), open). This matters for the Register (double-click to edit a transaction).
  - `TextEdit` ignored AccessKit `SetValue` until recently ([egui#8699](https://github.com/emilk/egui/issues/8699), closed 2026-10-12, not in 0.36.2). Use `focus()` + `type_text()`, not value-setting.

### Image snapshot tests (need wgpu)

- **How it works** ([README](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/README.md), [snapshot.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/snapshot.rs)):
  - Enable the `wgpu` and `snapshot` features and call `harness.snapshot("name")`.
  - Images go to `tests/snapshots/{name}.png` (relative to the crate). On mismatch, `{name}.new.png` and `{name}.diff.png` are written next to it.
  - `harness.fit_contents()` shrinks the window to the content, which keeps images small.
  - `harness.mask(rect)` hides volatile regions such as dates.
- **Results gathering:**
  - Since [#7647](https://github.com/emilk/egui/issues/7647), the harness collects results and **panics on drop** if any snapshot failed. All snapshots in a test are reported together.
  - Dropping more than one `SnapshotResults` in a test panics with "all snapshot results within a test should be merged in a single SnapshotResults instance". Merge them with `take_snapshot_results` + `SnapshotResults::extend`.
- **Thresholds** ([config.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/config.rs)):
  - `threshold` (default 0.6) is a per-pixel YIQ distance. `max_failed_pixels` (default 0) is an absolute count.
  - Both can be overridden per OS in a workspace-root `kittest.toml`.
  - The README warns that a `max_failed_pixels` above ~10 "is enough to hide a real change, such as a moved separator".
  - **Doc bug:** the README's example uses a `[macos]` table, but `Config` deserializes with `deny_unknown_fields` and the field is named `mac`. egui's own [`kittest.toml`](https://github.com/emilk/egui/blob/0.36.2/kittest.toml) uses `[mac]`. Use `[mac]`.
- **The README's guidelines** (adopt as **[standard]**):
  - "Whenever possible prefer regular Rust tests or `insta` snapshot tests over image comparison tests", because image tests are slow, brittle, and take repo space.
  - Images should "depict exactly what's tested and nothing else" and "have a low resolution".
  - Images are checked in, and egui uses Git LFS.
- **Rerun's guard-rail** ([re_ui/src/testing.rs](https://github.com/rerun-io/rerun/blob/0.39.0/crates/viewer_support/re_ui/src/testing.rs), [clippy.toml](https://github.com/rerun-io/rerun/blob/0.39.0/clippy.toml)):
  - Rerun wraps the builder in `re_ui::testing::new_harness(TestOptions::Gui, size)`. It always calls `.wgpu()` and sets options from one of two "blessed" functions.
  - `clippy.toml` `disallowed-methods` bans `egui_kittest::SnapshotOptions::new` with the reason "so that snapshot tests get strict thresholds on CI".
  - egui's own doc comment recommends the same idea: write a project `my_image_snapshot` helper and use `disallowed_methods` to stop callers using the raw one ([snapshot.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/snapshot.rs)).
  - **For Balanced:** a `balanced_ui::testing::harness()` helper plus a `disallowed-methods` entry. **[config]**
- **Snapshot names must be legal file names on every OS.** Rerun broke its Windows build with names that are illegal paths there ([egui#5767](https://github.com/emilk/egui/issues/5767), open). Use `[a-z0-9_]` only. **[standard]**

### GPU and platform caveats in headless CI

- **Default adapter choice** ([wgpu.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/wgpu.rs)):
  - egui_kittest creates a headless wgpu instance ("No display handle needed for headless testing").
  - It sorts adapters by backend (Metal, then Vulkan, then DX12, then GL) and then **prefers a CPU (software) adapter**: "CPU is the best for our purposes!".
  - It enables DX12 "because it always comes with a software rasterizer" (WARP on Windows).
  - It uses `RendererOptions::PREDICTABLE`.
  - The GPU wait timeout is 10 s, which guards against device loss on macOS/Metal.
- **Why rendering differs between machines** (README): MSAA sample placement, texture filtering, float rounding and denormal flushing all vary by GPU, backend and driver version. Snapshots are only bit-stable on one fixed rasterizer.
- **egui's choice:** tests run **only on `macos-latest`**, "because it will run with an actual GPU" ([rust.yml](https://github.com/emilk/egui/blob/0.36.2/.github/workflows/rust.yml)).
  - `kittest.toml` sets `threshold = 2.0` for every OS and `[mac] threshold = 0.6`, because "our CI runs snapshot tests on macOS, this is our source of truth".
  - Contributors on Linux and Windows get loose thresholds locally ([kittest.toml](https://github.com/emilk/egui/blob/0.36.2/kittest.toml), [CONTRIBUTING.md](https://github.com/emilk/egui/blob/0.36.2/CONTRIBUTING.md)).
- **Rerun's choice:** every platform uses **lavapipe** (Mesa's software Vulkan) in CI, and Linux CI is the source of truth ([setup_software_rasterizer.py](https://github.com/rerun-io/rerun/blob/0.39.0/scripts/ci/setup_software_rasterizer.py), [testing.rs](https://github.com/rerun-io/rerun/blob/0.39.0/crates/viewer_support/re_ui/src/testing.rs)):
  - Why: "Using a software rasterizer avoids GPU-related flakiness on CI runners … spurious timeouts and failure to find graphics devices".
  - **Linux:** a pinned Mesa build (`MESA_VERSION = "25.2.7"`, from `gfx-rs/ci-build`, the same as wgpu's CI). Rerun writes its own ICD JSON and copies it to `~/.config/vulkan/icd.d`, because "On CI we run with elevated privileges, therefore VK_DRIVER_FILES is ignored".
  - **Windows:** `mesa-dist-win` lavapipe, copied into `target/debug/deps`, and registered under `HKLM\SOFTWARE\Khronos\Vulkan\Drivers` for the same elevated-privilege reason. The Vulkan SDK is installed first.
  - **Per-pixel tolerance:** `strict_on_ci` keeps a small non-zero per-pixel threshold even on Linux CI, because "lavapipe JIT-compiles its shaders for the host CPU, and the CI runners are not all the same". macOS and Windows CI, and developer GPUs, get lenient thresholds.
- **For Balanced (inference):**
  - Follow Rerun, not egui. Linux is first-class, the repo is private (macOS runner minutes are billed at a multiplier), and lavapipe gives one reproducible rasterizer.
  - Simplest start: Ubuntu's `mesa-vulkan-drivers` package (lavapipe). Its Mesa version drifts with the runner image, so pin the runner image (`ubuntu-24.04`, not `ubuntu-latest`). Move to Rerun's pinned `gfx-rs/ci-build` Mesa if drift shows up as snapshot churn.
  - Windows CI runs the interaction tests. Its snapshot tests either use lenient `[windows]` thresholds or are skipped. Windows is not the source of truth. **[config]**
- No official doc covers headless rendering outside tests ([egui#5744](https://github.com/emilk/egui/issues/5744), open).

### Snapshot update workflow

- **Locally** ([snapshot.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/snapshot.rs)):
  - `UPDATE_SNAPSHOTS=1` (or `true`/`yes`/`on`) rewrites *failing* snapshots and makes those tests pass, so `cargo test` doesn't stop at the first failing crate.
  - `UPDATE_SNAPSHOTS=force` rewrites all snapshots.
  - Any other value panics.
- **From CI** ([update_snapshots_from_ci.sh](https://github.com/emilk/egui/blob/0.36.2/scripts/update_snapshots_from_ci.sh), [update_kittest_snapshots.yml](https://github.com/emilk/egui/blob/0.36.2/.github/workflows/update_kittest_snapshots.yml)). A developer's GPU won't match lavapipe, so the source-of-truth images come from CI:
  1. The test job always uploads `**/tests/snapshots` as an artifact. Rerun first prunes unchanged files, because all of them are "~80 MB".
  2. `scripts/update_snapshots_from_ci.sh` runs `gh run download <run> --name test-results` and copies the files over the tree.
  3. A `workflow_dispatch` workflow takes a `run_id`, runs that script, and commits with `stefanzweifel/git-auto-commit-action`.
     - It refuses to run on `main`.
     - It needs a PAT secret (`SNAPSHOT_COMMIT_GITHUB_TOKEN`), because commits made with the default token don't trigger further workflows.
- **Reviewing image diffs:**
  - Rerun's [kitdiff](https://github.com/rerun-io/kitdiff) shows original/new/diff for local files, or for a PR (`kitdiff pr <url>`).
  - With a `kitdiff.toml` (`update_snapshot_workflow_name`, `artifact_pattern`), it adds a "Commit the updated snapshots" button that triggers the workflow above.
  - It is installed from git, not crates.io.
- **Storage:**
  - egui and Rerun both store snapshot PNGs in **Git LFS** (`**/snapshots/**/*.png filter=lfs …`).
  - egui's CI fails any `.png` not in LFS ([png_only_on_lfs.yml](https://github.com/emilk/egui/blob/0.36.2/.github/workflows/png_only_on_lfs.yml)). An `InvalidSignature` error in snapshot tests usually means LFS wasn't installed.
  - Add `**/tests/snapshots/**/*.diff.png` and `*.new.png` to `.gitignore` (README). **[config]**
  - Note: GitHub LFS storage and bandwidth on a private repo count against a quota.

### Other egui-side tools worth knowing

- **`egui_inspection` / `egui_mcp`** (new in 0.35, [CHANGELOG](https://github.com/emilk/egui/blob/0.36.2/CHANGELOG.md)):
  - eframe's `inspection` feature and `EGUI_INSPECTION=1` let an external tool read the running app's AccessKit tree and send events.
  - `egui_mcp` exposes that as an MCP server "to have the agent use the app, reproduce bugs and verify its changes".
  - Rerun depends on it (`egui_mcp = "0.2.0"`).
  - This is relevant to agent-driven work on Balanced, but it is not a substitute for tests.
- **`insta`** for text snapshots of non-UI output (reports, CSV/QIF export, formatted amounts). Rerun uses `INSTA_FORCE_PASS=1` + `cargo insta review` ([TESTING.md](https://github.com/rerun-io/rerun/blob/0.39.0/TESTING.md)).

---

## 2. AccessKit, and how kittest queries widgets through it

- **AccessKit is always on.**
  - egui 0.34 "Remove[d] `accesskit` feature and always depend[s] on `accesskit`" ([#7701](https://github.com/emilk/egui/pull/7701), CHANGELOG 0.34.0).
  - `Context::enable_accesskit()` makes egui emit a tree update each frame in `FullOutput::platform_output.accesskit_update`.
  - The harness enables it, runs one frame, and feeds every update into kittest's `State` ([lib.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/lib.rs)).
  - On the eframe side, `accesskit` is a default feature that connects that tree to the OS (UIA on Windows, AT-SPI on Linux, NSAccessibility on macOS).
- **Queries** ([kittest query.rs](https://github.com/rerun-io/kittest/blob/0.4.0/src/query.rs), [filter.rs](https://github.com/rerun-io/kittest/blob/0.4.0/src/filter.rs)):
  - The `Queryable` trait generates four variants per selector: `get_by_*` (exactly one, else panic), `query_by_*` (`Option`), `get_all_by_*` (at least one) and `query_all_by_*`.
  - Selectors: `label`, `label_contains`, `role`, `role_and_label`, `value`, and a general `By` filter (`by().role(..).label(..).predicate(..)`).
  - Nodes expose AccessKit properties: `toggled()`, `value()`, `is_focused()`, `rect()`, and `accesskit_node()` for the rest.
- **How a widget gets its label and role:**
  - Built-in widgets call `Response::widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, "OK"))` internally.
  - `WidgetInfo` also has `selected`, `slider`, `drag_value` and `text_edit` constructors ([output.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui/src/data/output.rs)).
  - A text field next to a separate label needs `.labelled_by(label.id)`, or `get_by_label` won't find it ([response.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui/src/response.rs)).
  - Lower level: `Context::accesskit_node_builder(id, |node| …)` edits the raw `accesskit::Node`.
- **Consequences for Balanced:**
  - The Register and Account bar will be custom-painted (`egui_table` / `TableBuilder` rows, per #7), so they are invisible to both tests and screen readers unless we add `widget_info`, roles (`Row`, `Cell`, `TreeItem`) and labels.
  - Standard: "every interactive custom widget sets `widget_info`; every text field has a label or `labelled_by`; tests locate widgets by role + label, never by screen coordinates". **[standard]**
  - Test labels come from the i18n layer. Query by the label in the test's locale, which should be pinned (the default `en`). Or keep a test helper that looks the key up through `balanced_i18n`, so a copy change doesn't break tests. **[standard]**
- **Version coupling:**
  - egui's workspace holds `accesskit_consumer = "0.35.0"` because "kittest 0.4 pins accesskit_consumer 0.35, so bumping splits it into two versions" ([egui Cargo.toml](https://github.com/emilk/egui/blob/0.36.2/Cargo.toml)). egui `main` has already moved to kittest 0.5 / accesskit 0.25.
  - Use `egui_kittest::kittest` (the re-export), and never depend on `kittest` or `accesskit` directly. That way upgrades don't split the graph. **[config]** (clippy `disallowed-types` can't express this; a `deny.toml` `[bans]` `wrappers` entry for `kittest` can.)

---

## 3. Mutation testing with cargo-mutants

### What it does and costs

- cargo-mutants generates mutants (replace a function body with a default value, flip `<` to `<=`, delete `!`, and so on). It builds and tests each one, and reports **missed** mutants: the tests still passed.
- Runtime: "Most of the runtime … is spent in running the program test suite and in running incremental builds: both are done once per viable mutant" ([performance.md](https://github.com/sourcefrog/cargo-mutants/blob/main/book/src/performance.md)).
  - Cost ≈ mutants × (incremental build + test time of the mutated package).
  - A fast `balanced_core` test suite pays off many times over.
- **Speed levers** (same page):
  - A `[profile.mutants] inherits = "test", debug = "none"` profile, selected in `.cargo/mutants.toml` with `profile = "mutants"`.
  - A faster linker: mold saves about 20%, and wild "cut the time … by more than half" on one tree.
  - A tmpfs `TMPDIR`.
  - Skipping doctests (`-- --all-targets`).
- **Timeouts** ([timeouts.md](https://github.com/sourcefrog/cargo-mutants/blob/main/book/src/timeouts.md)):
  - Default: 5× the baseline test time, minimum 20 s.
  - With `--baseline=skip` or `--in-place`, the default is a flat 300 s unless `--timeout` is given.
  - Mutants that turn loops infinite need `#[mutants::skip]`.
- **nextest:** `test_tool = "nextest"` works.
  - nextest doesn't run doctests, so behaviour tested only by doctests shows as missed.
  - It is sometimes *slower*, because straggling tests run to completion ([nextest.md](https://github.com/sourcefrog/cargo-mutants/blob/main/book/src/nextest.md)).
  - Measure before choosing.

### Scoping: which code, and which tests

- **Packages** ([workspaces.md](https://github.com/sourcefrog/cargo-mutants/blob/main/book/src/workspaces.md)):
  - In a virtual workspace, cargo-mutants mutates `default-members`, or all packages if that isn't set. `--package` / `--workspace` override this.
  - **Each mutant runs only the tests of the package being mutated**, unless `test_workspace = true` or `test_package` is set.
  - That default suits Balanced: `balanced_core` logic must be caught by `balanced_core`'s own tests, not incidentally by a UI test. **[config]**
- **Excluding UI-only code** ([skip.md](https://github.com/sourcefrog/cargo-mutants/blob/main/book/src/skip.md), [skip_files.md](https://github.com/sourcefrog/cargo-mutants/blob/main/book/src/skip_files.md), [filter_mutants.md](https://github.com/sourcefrog/cargo-mutants/blob/main/book/src/filter_mutants.md)):
  - The book's own guidance: "If a whole module is untestable, use a filter by path in the config file"; for a "class of functions, such as `Debug` implementations, use a regex filter in the config file"; for single functions "use an attribute, so that the skip is visible in the code".
  - Config keys: `examine_globs` / `exclude_globs` (paths), `examine_re` / `exclude_re` (mutant names), `skip_calls` (default `with_capacity`).
  - Since 27.0.0, config and CLI filters combine.
  - `#[mutants::skip]` needs the `mutants` crate as a **normal** dependency (not a dev-dependency). The `mutants` crate is 0.0.4 on crates.io.
  - The finer-grained `#[mutants::exclude_re("…")]` attribute needs `mutants` 0.0.5, which is **unreleased** (it is under "Unreleased" in [NEWS.md](https://github.com/sourcefrog/cargo-mutants/blob/main/NEWS.md)).
  - **For Balanced:** select packages (`balanced_core`, `balanced_i18n`) rather than excluding `balanced_ui`. UI code is then out by construction, and the crate split from #24 does the work. `exclude_re = ["impl Debug", "impl fmt::Display"]` (what rust-bitcoin does) covers formatting boilerplate. **[config]**
- **`--in-diff`** ([in-diff.md](https://github.com/sourcefrog/cargo-mutants/blob/main/book/src/in-diff.md)):
  - It "tests only mutants that overlap with regions changed in the diff", applied after `--package` and the other filters.
  - Caveats from the book:
    - Edits in one place can weaken tests elsewhere, so it is "not a substitute for a full test run".
    - "a diff that only deletes or changes test code won't cause any mutants to run".
- **`--in-place`** ([in-place.md](https://github.com/sourcefrog/cargo-mutants/blob/main/book/src/in-place.md)):
  - It mutates the checkout itself. It reuses `target/` (and so the rust-cache) and is the book's CI recommendation.
  - It is incompatible with `--jobs`.
  - Locally, don't edit or commit during a run. Stray edits carry the marker `/* ~ changed by cargo-mutants ~ */`.

### CI setups in real projects

| Project | Trigger | Shape | Blocking? |
|---|---|---|---|
| cargo-mutants book ([in-diff.yml](https://github.com/sourcefrog/cargo-mutants/blob/main/examples/workflows/in-diff.yml), [sharded.yml](https://github.com/sourcefrog/cargo-mutants/blob/main/examples/workflows/sharded.yml)) | PR / push | `git diff origin/$BASE.. > git.diff`, `cargo mutants --no-shuffle -vV --in-diff git.diff`; full run sharded 8 ways with `--shard k/8 --baseline=skip --timeout 300 --in-place`, after the normal test job passes | yes (fails on missed) |
| [mozilla/neqo](https://github.com/mozilla/neqo/blob/main/.github/workflows/mutants-pr.yml) | PR + merge queue | `--in-diff pr.diff --no-shuffle --in-place --profile mutants -- --all-targets`; exit codes 2 (missed), 3 (timeout), 4 (unviable) are written to the step summary, not failed | **no** (advisory) |
| [mozilla/neqo](https://github.com/mozilla/neqo/blob/main/.github/workflows/mutants.yml) | weekly cron | baseline job measures test time and sets the timeout to 3× (30–90 s), then **64 shards** | report |
| [matrix-org/vodozemac](https://github.com/matrix-org/vodozemac/blob/main/.github/workflows/mutants.yml) | PR, path-filtered to `src/**`, `Cargo.*`, `.cargo/**` | `cargo mutants -vV --no-shuffle --in-place --in-diff git.diff`; artifact `mutants.out` | yes |
| [rust-bitcoin](https://github.com/rust-bitcoin/rust-bitcoin/blob/master/.github/workflows/cron-weekly-cargo-mutants.yml) | weekly cron | `cargo mutants --in-place --no-shuffle`; if `mutants.out/missed.txt` is non-empty, **opens a GitHub issue** with the first 10 | report |

- Common choices:
  - install with `taiki-e/install-action` (`tool: cargo-mutants`);
  - `fetch-depth: 0` for the diff;
  - `--no-shuffle` for stable ordering;
  - always upload `mutants.out` (`if: always()`);
  - GitHub annotations are automatic when `$GITHUB_ACTION` is set ([ci.md](https://github.com/sourcefrog/cargo-mutants/blob/main/book/src/ci.md)).
- rust-bitcoin's [`.cargo/mutants.toml`](https://github.com/rust-bitcoin/rust-bitcoin/blob/master/.cargo/mutants.toml) is the best example of a mature exclusion list. It uses `examine_globs` for the crates that matter, and `exclude_re` for `impl Debug`, error types, serde, `Iterator` ("can result in an infinite loop") and deprecated functions, each with a reason comment. **[standard]**: every exclusion carries a comment saying why.
- neqo excludes test-assertion helpers by glob ([.cargo/mutants.toml](https://github.com/mozilla/neqo/blob/main/.cargo/mutants.toml)), because "every mutation looks 'missed' without indicating an actual gap".
- **No egui project found runs cargo-mutants.** Neither egui nor Rerun does. So excluding UI code is our inference, supported by the book's "test the functionality by … higher-level integration tests" reason for skipping.
  - Mutating `balanced_ui` would rebuild a wgpu-linked crate per mutant and run image tests. That is slow, and it gives noisy results from tolerance-based comparisons.

### Recommended Balanced shape

```toml
# .cargo/mutants.toml  [config]
profile = "mutants"
examine_globs = ["crates/balanced_core/src/**/*.rs", "crates/balanced_i18n/src/**/*.rs"]
exclude_re = [
  "impl Debug",        # formatting boilerplate, not behaviour
  "impl fmt::Display", # covered by insta snapshots where it matters
]
# test_workspace stays false: core logic must be caught by core's own tests.
```

```toml
# Cargo.toml  [config]
[profile.mutants]
inherits = "test"
debug = "none"
```

- **PR job:**
  - `cargo mutants --in-diff git.diff --in-place --no-shuffle -vV`, path-filtered to `crates/balanced_core/**`, `crates/balanced_i18n/**`, `Cargo.*` and `.cargo/**`.
  - Start **advisory** (neqo-style step summary). Make it blocking once the baseline of missed mutants is zero.
- **Weekly job:**
  - A full run on `main`, sharded if it exceeds ~30 min, that opens an issue on missed mutants (rust-bitcoin-style).
  - It fits the repo's GitHub-Issues workflow, with a `needs-triage` label.
- **Locally:** `cargo mutants --in-diff <(git diff main)` before opening a PR that touches core. **[standard]**
- Add `mutants.out*/` to `.gitignore`. **[config]**

---

## 4. CI layout for the workspace

What egui ([rust.yml](https://github.com/emilk/egui/blob/0.36.2/.github/workflows/rust.yml)), Rerun ([reusable_checks_rust.yml](https://github.com/rerun-io/rerun/blob/0.39.0/.github/workflows/reusable_checks_rust.yml), [rust_checks.py](https://github.com/rerun-io/rerun/blob/0.39.0/scripts/ci/rust_checks.py)) and eframe_template do:

| Concern | egui 0.36.2 | Rerun 0.39.0 | Balanced proposal |
|---|---|---|---|
| Toolchain | `dtolnay/rust-toolchain@stable` with `toolchain: 1.95.0`, and `rust-toolchain` file `1.95.0` | `rustup install` from the `rust-toolchain` file | `rust-toolchain.toml` is the single source; CI runs `rustup show` / `rustup install` **[config]** |
| Warnings | `RUSTFLAGS: -D warnings`, `RUSTDOCFLAGS: -D warnings` (env) | same, `--deny warnings` | same, CI only (#24) **[config]** |
| fmt | `cargo fmt --all -- --check` | same (non-fatal on Windows due to a command-line-length bug, rustfmt#6934) | same, Linux only |
| clippy | many feature combinations, plus `--release` | `--all-targets --all-features -- --deny warnings` | `cargo clippy --locked --workspace --all-targets --all-features` on Linux **and** Windows (cfg-specific code) |
| Tests | `cargo test --all-features` on **macOS** (real GPU); doctests separately | `cargo nextest run --all-targets --all-features --no-fail-fast` + `cargo test --doc` on Linux with lavapipe; macOS/Windows on `main` only | interaction + unit tests on Linux and Windows; snapshot tests on Linux + lavapipe; `cargo test --doc` separately if nextest is used |
| Snapshot artifacts | uploads `**/tests/snapshots` always | prunes unchanged, uploads `tests/snapshots` + `tests/failures` | upload on failure only (small repo) |
| cargo-deny | `EmbarkStudios/cargo-deny-action@v2` × 12 target triples | `cargo install --locked cargo-deny@^0.19`; targets from `[graph].targets` in `deny.toml` | `cargo-deny-action@v2`; targets `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin` in `deny.toml` **[config]** |
| Unused deps | `cargo shear` (taiki-e/install-action, `cargo-shear@1.11.2`) | `cargo_shear.yml` | `cargo shear` **[config]** |
| Typos | `crate-ci/typos` | — | optional |
| Caching | `Swatinem/rust-cache@v2` | sccache on GCS (self-hosted infra) | `Swatinem/rust-cache@v2` (v2.9.2, 2026-08-06) |
| Lockfile | `--locked` on clippy | `--locked` on check | `--locked` on every cargo command **[config]** |
| Linux system deps | `libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev libgtk-3-dev` ("libgtk-3-dev is used by rfd") | via pixi | same list (Balanced uses `rfd`, per #7) **[config]** |
| Flaky-test policy | — | nextest `ci` profile `retries = 1`; local default no retries "so flakes surface"; `slow-timeout = 30s` | adopt if/when nextest is adopted |
| LFS | `actions/checkout` with `lfs: true` | same | same, on jobs that run snapshot tests |

- **MSRV job.** Cargo's guidance and #24 make `rust-version` equal the pinned toolchain. CI already builds on that exact version, so a separate `cargo msrv verify` / `cargo hack --rust-version` job adds nothing until the two diverge.
  - egui's own policy ("always _at least_ two minor release behind the latest Rust", [RELEASES.md](https://github.com/emilk/egui/blob/0.36.2/RELEASES.md)) sets Balanced's floor (1.95 today).
- **Avoid eframe_template's CI as a model.** Its [rust.yml](https://github.com/emilk/eframe_template/blob/main/.github/workflows/rust.yml) still uses the archived `actions-rs/toolchain@v1` and `actions-rs/cargo@v1`, and it has no caching.
- **Proposed job graph** (one workflow, `concurrency: cancel-in-progress` per ref):
  1. `lint` (ubuntu-24.04): fmt, clippy, `cargo doc --no-deps`, cargo-shear, xtask layer check (#24).
  2. `deny` (ubuntu): `cargo deny check`.
  3. `test` (matrix: ubuntu-24.04, windows-latest): `cargo test --locked --workspace --all-targets`, with snapshot tests either feature-gated off or set to lenient thresholds on Windows.
  4. `snapshots` (ubuntu-24.04 + lavapipe, `lfs: true`): runs the `balanced_ui` snapshot tests and uploads `tests/snapshots` on failure.
  5. `mutants-pr` (ubuntu, PR only, path-filtered, needs `test`).
  6. `macos-check` (macos, `main` only or weekly): `cargo check` + tests without snapshots. macOS is "functional", so it is not gated per PR, and it costs more minutes on a private repo.
  - Separate workflows: `update_snapshots.yml` (workflow_dispatch) and `mutants-weekly.yml` (cron).
  - **[config]**: the YAML is the rule. `CODING_STANDARDS.md` only says "CI must be green", plus the snapshot-update procedure in `docs/practices/testing.md`.

---

## 5. Pinning and upgrading egui

- **Cadence:** "usually a new major release every two months or so … often followed by one or two patch releases" ([RELEASES.md](https://github.com/emilk/egui/blob/0.36.2/RELEASES.md)).
  - Observed: 0.34.0 2026-03-26, 0.35.0 2026-06-25, 0.36.0 2026-08-05, 0.36.2 2026-09-08 ([CHANGELOG](https://github.com/emilk/egui/blob/0.36.2/CHANGELOG.md)).
  - Under Cargo's 0.x semver, each minor is breaking.
- **Lock-step crates:** "All crates under the `crates/` folder are published in lock-step, with the same version number" (RELEASES.md). That covers egui, eframe, egui_extras, egui-wgpu, egui_kittest and epaint.
  - Satellite crates in other repos have **their own** versions, which must match the egui minor they were built against: Rerun has `egui_plot = "0.37.0"`, `egui_table = "0.10.0"` and `egui_tiles = "0.17.1"` on egui 0.36.2.
  - egui's release checklist updates eframe_template, egui_plot, egui_table, egui_tiles and Rerun as part of each release, so those follow within days. Third-party crates may lag.
- **Deprecation window is one minor:**
  - 0.34.0 deprecated `App::update`, `SidePanel` / `TopBottomPanel`, and panels on `Context`.
  - 0.35.0 then did "Remove everything that was marked `#[deprecated]`" ([#8105](https://github.com/emilk/egui/pull/8105)).
  - Skipping a minor means hitting removals with no deprecation hints.
- **How the references pin:**
  - eframe_template: `egui = "0.36.0"` (caret, so `>=0.36.0, <0.37`). The commented-out alternatives are git `branch = "main"` or a local `path`.
  - Rerun: `egui = { version = "0.36.2", default-features = false, features = [..] }` in `[workspace.dependencies]`, **plus** a `[patch.crates-io]` pointing every egui crate at `git = "https://github.com/emilk/egui", branch = "main"`. Its comment: "Prefer patching with `branch` over `rev` and let `Cargo.lock` handle the commit hash" ([Cargo.toml](https://github.com/rerun-io/rerun/blob/0.39.0/Cargo.toml)). Rerun co-develops egui, so it tracks `main`. That is not a model for Balanced.
  - egui's own workspace documents *why* a version is held back, inline (`accesskit_consumer = "0.35.0" # Can't update to 0.36+: kittest 0.4 pins …`). **[standard]**: every held-back dependency gets a reason comment.
- **Detecting a split graph:** egui's `deny.toml` sets `[bans] multiple-versions = "deny"` with a commented `skip` list. Two `egui` or `accesskit` versions in `Cargo.lock` (for example, a third-party widget crate still on 0.35) fail CI instead of failing to compile with confusing trait errors. **[config]**
- **Neither egui nor Rerun uses Dependabot or Renovate** (no config in `.github/`). Upgrades are manual PRs.
- **Proposed upgrade procedure** for `docs/practices/egui-upgrades.md` **[standard]**:
  1. Wait for the first patch release (x.y.1), or about two weeks. egui often ships one or two quick patch releases.
  2. Confirm every egui-family dependency (egui_table, egui_plot, …) has a release for the new minor. If not, wait. Don't `[patch]` to git.
  3. On the *current* version, fix all `deprecated` warnings (clippy `-D warnings` already forces this).
  4. Bump the one `[workspace.dependencies]` version for all egui crates together. Read the CHANGELOG "Breaking"/"Changed" sections and the egui_kittest CHANGELOG.
  5. Run the tests. Expect snapshot churn from font or rendering changes: regenerate from CI, and review every image diff with kitdiff.
  6. One PR per egui minor. Never jump two minors.
  7. Raise `rust-version` / `rust-toolchain.toml` if the new egui MSRV requires it (#24's policy).

---

## 6. Proposed split: config vs standards

**[config]** (enforced by tools):

- `kittest.toml` at the workspace root:
  - `threshold = 2.0`, `[linux] threshold = 0.6`, `max_failed_pixels = 0`;
  - `[windows]` and `[mac]` lenient;
  - note: `[mac]`, not `[macos]`.
- `.gitattributes`: `**/tests/snapshots/**/*.png filter=lfs diff=lfs merge=lfs -text`.
- `.gitignore`: `*.new.png`, `*.diff.png`, `*.old.png` under `tests/snapshots`, and `mutants.out*/`.
- `clippy.toml` `disallowed-methods`:
  - `egui_kittest::SnapshotOptions::new` and `egui_kittest::Harness::builder` → use `balanced_ui::testing::harness`.
- `.cargo/mutants.toml` and `[profile.mutants]` (above).
- `deny.toml`:
  - `multiple-versions = "deny"`;
  - `[graph].targets` for Linux, Windows and macOS;
  - bans that keep egui out of core (#24).
- CI workflows as in section 4. `--locked` everywhere.

**[standard]** (`CODING_STANDARDS.md` / `docs/practices/testing.md`):

1. Prefer a unit test in `balanced_core` over a UI test, and a UI interaction test over an image snapshot.
2. UI tests drive the app through the UI (click, type) and assert on state or AccessKit properties, not on internals.
3. Every interactive custom widget sets `widget_info`. Every input has a label or `labelled_by`. Tests find widgets by role + label.
4. Image snapshots:
   - show only the thing under test, at a small size;
   - mask anything volatile (dates, clocks);
   - use only `[a-z0-9_]` names;
   - are regenerated from CI, not from a developer GPU;
   - any threshold override carries a comment saying why.
5. A test that animates or polls mocks time, so `Harness::run()` terminates.
6. Every cargo-mutants exclusion (`exclude_re`, `#[mutants::skip]`) carries a reason comment.
7. Missed mutants in a PR's diff are either killed with a test or excluded with a reason, not ignored.
8. Every held-back dependency version carries a reason comment.
9. egui upgrades follow `docs/practices/egui-upgrades.md`: one minor at a time, deprecations first.

---

## Open questions for "Adopt coding practices" (#12)

- **PR mutants: blocking or advisory?** neqo is advisory, and vodozemac and the book's example are blocking. The recommendation is advisory until the core crate exists and reaches zero missed.
- **Snapshot source of truth: Ubuntu's Mesa or a pinned Mesa build?** Start with Ubuntu's `mesa-vulkan-drivers` on a pinned runner image, and switch to Rerun's pinned `gfx-rs/ci-build` Mesa if snapshots churn.
- **nextest or `cargo test`?** Rerun uses nextest (retries, slow-timeouts). cargo-mutants notes nextest can be slower and skips doctests. Measure on the real suite.
- **Windows snapshot tests:** run them with lenient thresholds (catches gross breakage), or gate them off with a cargo feature (faster, less noise)?

---

## Sources

- egui 0.36.2:
  - [egui_kittest README](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/README.md)
  - [lib.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/lib.rs)
  - [builder.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/builder.rs)
  - [snapshot.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/snapshot.rs)
  - [config.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/config.rs)
  - [wgpu.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/wgpu.rs)
  - [node.rs](https://github.com/emilk/egui/blob/0.36.2/crates/egui_kittest/src/node.rs)
  - [kittest.toml](https://github.com/emilk/egui/blob/0.36.2/kittest.toml)
  - [rust.yml](https://github.com/emilk/egui/blob/0.36.2/.github/workflows/rust.yml)
  - [update_kittest_snapshots.yml](https://github.com/emilk/egui/blob/0.36.2/.github/workflows/update_kittest_snapshots.yml)
  - [update_snapshots_from_ci.sh](https://github.com/emilk/egui/blob/0.36.2/scripts/update_snapshots_from_ci.sh)
  - [png_only_on_lfs.yml](https://github.com/emilk/egui/blob/0.36.2/.github/workflows/png_only_on_lfs.yml)
  - [deny.toml](https://github.com/emilk/egui/blob/0.36.2/deny.toml)
  - [Cargo.toml](https://github.com/emilk/egui/blob/0.36.2/Cargo.toml)
  - [RELEASES.md](https://github.com/emilk/egui/blob/0.36.2/RELEASES.md)
  - [CHANGELOG.md](https://github.com/emilk/egui/blob/0.36.2/CHANGELOG.md)
  - [CONTRIBUTING.md](https://github.com/emilk/egui/blob/0.36.2/CONTRIBUTING.md)
  - [scripts/check.sh](https://github.com/emilk/egui/blob/0.36.2/scripts/check.sh)
  - docs.rs: [egui_kittest 0.36.2](https://docs.rs/egui_kittest/0.36.2/egui_kittest/)
- egui issues:
  - [#8697](https://github.com/emilk/egui/issues/8697) double-click in kittest
  - [#8699](https://github.com/emilk/egui/issues/8699) TextEdit `SetValue`
  - [#5767](https://github.com/emilk/egui/issues/5767) snapshot names
  - [#5744](https://github.com/emilk/egui/issues/5744) headless rendering docs
  - [#7647](https://github.com/emilk/egui/issues/7647) snapshot results in harness
- kittest 0.4.0:
  - [README](https://github.com/rerun-io/kittest/blob/0.4.0/README.md)
  - [query.rs](https://github.com/rerun-io/kittest/blob/0.4.0/src/query.rs)
  - [filter.rs](https://github.com/rerun-io/kittest/blob/0.4.0/src/filter.rs)
- [kitdiff README](https://github.com/rerun-io/kitdiff)
- Rerun 0.39.0:
  - [TESTING.md](https://github.com/rerun-io/rerun/blob/0.39.0/TESTING.md)
  - [reusable_checks_rust.yml](https://github.com/rerun-io/rerun/blob/0.39.0/.github/workflows/reusable_checks_rust.yml)
  - [rust_checks.py](https://github.com/rerun-io/rerun/blob/0.39.0/scripts/ci/rust_checks.py)
  - [setup_software_rasterizer.py](https://github.com/rerun-io/rerun/blob/0.39.0/scripts/ci/setup_software_rasterizer.py)
  - [re_ui/src/testing.rs](https://github.com/rerun-io/rerun/blob/0.39.0/crates/viewer_support/re_ui/src/testing.rs)
  - [clippy.toml](https://github.com/rerun-io/rerun/blob/0.39.0/clippy.toml)
  - [Cargo.toml](https://github.com/rerun-io/rerun/blob/0.39.0/Cargo.toml)
  - [.config/nextest.toml](https://github.com/rerun-io/rerun/blob/0.39.0/.config/nextest.toml)
  - [update_kittest_snapshots.yml](https://github.com/rerun-io/rerun/blob/0.39.0/.github/workflows/update_kittest_snapshots.yml)
  - [.gitattributes](https://github.com/rerun-io/rerun/blob/0.39.0/.gitattributes)
- cargo-mutants 27.1.0 book (`main`, 2026-10):
  - [in-diff](https://mutants.rs/in-diff.html)
  - [CI](https://mutants.rs/ci.html)
  - [PR diff](https://mutants.rs/pr-diff.html)
  - [sharding](https://mutants.rs/shards.html)
  - [skip](https://mutants.rs/skip.html)
  - [attrs](https://mutants.rs/attrs.html)
  - [skip files](https://mutants.rs/skip_files.html)
  - [filter mutants](https://mutants.rs/filter_mutants.html)
  - [workspaces](https://mutants.rs/workspaces.html)
  - [performance](https://mutants.rs/performance.html)
  - [timeouts](https://mutants.rs/timeouts.html)
  - [nextest](https://mutants.rs/nextest.html)
  - [in-place](https://mutants.rs/in-place.html)
  - [config file](https://mutants.rs/config-file.html)
  - [example workflows](https://github.com/sourcefrog/cargo-mutants/tree/main/examples/workflows)
  - [NEWS.md](https://github.com/sourcefrog/cargo-mutants/blob/main/NEWS.md)
- cargo-mutants in production:
  - mozilla/neqo [mutants-pr.yml](https://github.com/mozilla/neqo/blob/main/.github/workflows/mutants-pr.yml), [mutants.yml](https://github.com/mozilla/neqo/blob/main/.github/workflows/mutants.yml), [.cargo/mutants.toml](https://github.com/mozilla/neqo/blob/main/.cargo/mutants.toml)
  - matrix-org/vodozemac [mutants.yml](https://github.com/matrix-org/vodozemac/blob/main/.github/workflows/mutants.yml)
  - rust-bitcoin [cron-weekly-cargo-mutants.yml](https://github.com/rust-bitcoin/rust-bitcoin/blob/master/.github/workflows/cron-weekly-cargo-mutants.yml), [.cargo/mutants.toml](https://github.com/rust-bitcoin/rust-bitcoin/blob/master/.cargo/mutants.toml)
- eframe_template (`main`): [Cargo.toml](https://github.com/emilk/eframe_template/blob/main/Cargo.toml), [rust.yml](https://github.com/emilk/eframe_template/blob/main/.github/workflows/rust.yml)
- Versions and dates: crates.io API, queried 2026-10-13. Action versions from GitHub releases, queried 2026-10-13.
