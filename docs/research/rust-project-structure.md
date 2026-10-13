# Rust project structure and code style

Research for [#6](https://github.com/StanleyGoldman/balanced/issues/6). It feeds "Adopt coding practices" ([#12](https://github.com/StanleyGoldman/balanced/issues/12)), which writes the root `CODING_STANDARDS.md`, the `docs/practices/<topic>.md` files and the tool config.

**Question:** What do high-trust public sources recommend for structuring a large Rust desktop application and its code?

**Date:** 2026-10-13.

**Toolchain baseline:**

- Current stable is **Rust 1.99.0**, released **2026-10-01** ([announcement](https://blog.rust-lang.org/2026/10/01/Rust-1.99.0/)). 1.100.0 is due six weeks later.
- The current edition is **Rust 2024**, stabilised in **Rust 1.85.0** on **2025-02-20**. No 2027 edition has been announced. RFC 3501 sets a three-year cadence and allows a year to be skipped, and the only 2027 discussion is speculative ([internals thread](https://internals.rust-lang.org/t/we-have-only-two-years-to-prepare-the-rust-2027-edition/22494)).

**Reference repos** (all read at a tag):

| Repo | Tag | Date | Edition | `rust-version` | Workspace shape |
|---|---|---|---|---|---|
| [rust-analyzer](https://github.com/rust-lang/rust-analyzer/tree/2026-10-12) | `2026-10-12` | 2026-10-12 | 2024 | 1.98 | Virtual manifest; flat `crates/*`, plus `lib/*` and `xtask/` |
| [Rerun](https://github.com/rerun-io/rerun/tree/0.39.0) | `0.39.0` | 2026-10-12 | 2024 | 1.96 (toolchain pinned to 1.96.0) | Virtual manifest; **layered** `crates/<layer>/*`, about 100 `re_*` crates |
| [egui](https://github.com/emilk/egui/tree/0.36.2) | `0.36.2` | 2026-09-08 | 2024 | 1.95 | Virtual manifest; `crates/*` + `examples/*` |
| [ripgrep](https://github.com/BurntSushi/ripgrep/tree/15.2.0) | `15.2.0` | 2026-07-15 | 2024 | 1.85 | Root package (`rg`) + `crates/*` |

The labels below mark how each finding should land:

- **[config]** belongs in `Cargo.toml`, `clippy.toml`, `rustfmt.toml`, `deny.toml` or CI.
- **[standard]** is a judgement rule for `CODING_STANDARDS.md` or `docs/practices/`.

Egui state, panels, commands, viewports and modals are covered in [`egui-app-architecture.md`](egui-app-architecture.md) (#7). Testing tools are #8, i18n libraries #9, and money types #10. This document only says where those pieces sit in the workspace.

---

## TL;DR for later decisions

1. **Workspace:** use a virtual manifest at the root, with crates flat under `crates/`. Directory name = crate name, and internal crates get `version = "0.0.0"` and `publish = false`. Share `edition`, `rust-version`, `license`, dependency versions and lints through `[workspace.package]`, `[workspace.dependencies]` and `[workspace.lints]` (matklad, Cargo Book). Set `resolver = "3"` explicitly, because a virtual manifest has no edition to infer it from. **[config]**
2. **Crate split for Balanced (recommendation):**
   - `balanced`: the binary.
   - `balanced_ui`: egui.
   - `balanced_core`: domain and commands, with **no egui dependency**.
   - `balanced_i18n`: locale and formatting, with no egui dependency.
   - `xtask`: automation.
   - `balanced_store` later, for persistence.

   The key invariant is that nothing below `balanced_ui` depends on egui or eframe. Write it down, and enforce it with `cargo-deny` or a CI check, as Rerun does with its layer check. **[config + standard]**
3. **Errors:** use `thiserror` (2.0.21) enum errors in library crates (`core`, `i18n`, `store`), and `anyhow` (1.0.104) with `.context()` only in the binary and at the UI's top-level error sink. Rerun writes this rule down, and the anyhow README agrees. rust-analyzer disagrees (anyhow everywhere), because it is one application whose crates have no outside callers. **[standard]**
4. **Logging:** library crates depend only on `tracing` (0.1.44). The binary alone installs a `tracing-subscriber` (0.3.23). Rerun's level definitions (ERROR = data lost, WARN = recovered, INFO = rare user-relevant, DEBUG/TRACE, per-frame goes in TRACE) are a good base. Add "handle each error exactly once: log it *or* return it". **[standard]**
5. **MSRV:** an app has no downstream users, so "latest stable" is Cargo's simplest documented policy. Pin it in `rust-toolchain.toml`, set `rust-version` to the same version, and bump both deliberately. Rerun pins N-3 and rust-analyzer N-1. Note that egui 0.36 already needs 1.95. **[config]**
6. **Lints:**
   - Turn on clippy `pedantic` at `warn` (priority -1), with a commented allow-list, and cherry-pick from `restriction`: `unwrap_used`, `dbg_macro`, `todo`, `print_stdout`, `allow_attributes`, `str_to_string` and so on.
   - Deny warnings in CI only, not in the manifest.
   - Use `clippy.toml` for `disallowed-methods`/`disallowed-types`/`disallowed-macros`. For example, ban `std::thread::spawn` in favour of a named thread, and ban raw `Color32::from_rgb` outside the theme module.
   - This is Rerun's and egui's setup. rust-analyzer uses only the default groups, and ripgrep has no lint table. **[config]**
7. **rustfmt:** use defaults plus `style_edition = "2024"`. The import-grouping options (`group_imports`, `imports_granularity`) are **nightly-only**, so either run nightly rustfmt in CI or leave imports to review. **[config]**
8. **Docs:** add an `ARCHITECTURE.md` codemap with layer invariants (matklad), and a `crates/README.md` saying where a new crate goes (Rerun). **[standard]**

---

## 1. Workspace layout

### What the sources say

- **Virtual manifest at the root.** "Make the root of the workspace a virtual manifest" so that cargo commands act on every member without `--workspace` ([matklad, "Large Rust Workspaces", 2021-08-22](https://matklad.github.io/2021/08/22/large-rust-workspaces.html)). The [Cargo Book, Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html#virtual-workspace) adds two points:
  - A virtual manifest "is useful when there isn't a 'primary' package".
  - `resolver` "must be set explicitly in virtual workspaces as they have no `package.edition` to infer it from".
- **Flat, not nested.** For 10k to 1M lines, "the flat layout makes the most sense". Put every crate one level deep in `crates/`, as rust-analyzer does, because Cargo's crate namespace is flat anyway (matklad).
- **Directory name = crate name.** "Don't succumb to the temptation to strip common prefix from folder names" (matklad). Rerun follows this (`crates/utils/re_log`). rust-analyzer's crates have no prefix in name or directory.
- **Internal crates:** "Use `version = "0.0.0"` for internal crates you don't intend to publish". Few crates should have a stable semver API, and those can live in a separate `libs/` folder (matklad).
- **Automation in Rust:** use the `cargo xtask` pattern, a workspace crate run via an alias, instead of shell or Makefiles (matklad). rust-analyzer has `xtask/`.
- **Keep `src/`** even for single-file crates (matklad).
- **Inheritance tables** ([Cargo Book](https://doc.rust-lang.org/cargo/reference/workspaces.html)):
  - `[workspace.package]` holds shared `version`, `edition`, `rust-version`, `license` and similar keys. Members opt in with `edition.workspace = true`.
  - `[workspace.dependencies]` sets one version per dependency for the whole workspace. Members write `serde = { workspace = true, features = [..] }`. Features are additive. Since Rust 1.99, on edition 2024, a member's `default-features = false` overrides the workspace entry. Before that, the workspace entry also had to disable defaults.
  - `[workspace.lints]` holds shared lint levels. Members need `[lints] workspace = true`, and **forgetting it silently drops the lints for that crate**. It has been respected since Rust 1.74.
- **Resolver:** `"3"` is the edition-2024 default (Rust 1.84+). It changes `incompatible-rust-versions` to `fallback`, so the resolver "will prefer packages with a Rust version that is less than or equal to your own Rust version" ([Cargo Book, Resolver](https://doc.rust-lang.org/cargo/reference/resolver.html#resolver-versions)). rust-analyzer, Rerun and egui all still write `resolver = "2"` in their virtual manifests. They are edition 2024, but the manifest value wins.

### What large repos do beyond matklad

- **Rerun groups crates into layer folders** (`crates/{utils,build,store,data_flow,store_app,viewer_support,views,panels,top,tests}/*`). The rule is: "A crate may depend on crates in its own folder, or in any folder below it … never above." `views` and `panels` are siblings that may not depend on each other.
  - `scripts/check_crate_layers.py` "fails in CI on a dependency that points up".
  - `crates/README.md` explains "Where does my new crate go?"
  - `ARCHITECTURE.md` draws the layers.

  Sources: [crates/README.md](https://github.com/rerun-io/rerun/blob/0.39.0/crates/README.md), [ARCHITECTURE.md](https://github.com/rerun-io/rerun/blob/0.39.0/ARCHITECTURE.md). This departs from matklad's flat rule, and it only pays off at about 100 crates.
- **ripgrep** has a root *package* (`rg`), with library crates in `crates/` listed explicitly in `members` ([Cargo.toml](https://github.com/BurntSushi/ripgrep/blob/15.2.0/Cargo.toml)). It suits a single binary that publishes its libraries.
- **Dev-build speed for GUI apps:** Rerun sets `[profile.dev] opt-level = 1` and `[profile.dev.package."*"] opt-level = 2`, so dependencies (egui, wgpu) are optimised while workspace crates stay debuggable ([Cargo.toml](https://github.com/rerun-io/rerun/blob/0.39.0/Cargo.toml)). rust-analyzer instead raises `opt-level` only for named hot dependencies. **[config]**

### Recommended Balanced workspace (synthesis, not from one source)

```
Cargo.toml              # virtual manifest: resolver = "3", workspace.{package,dependencies,lints}
rust-toolchain.toml     # pinned stable + rustfmt, clippy
clippy.toml  rustfmt.toml  deny.toml
ARCHITECTURE.md         # codemap + invariants (matklad)
crates/
  balanced/             # [[bin]] main.rs: eframe::run_native, tracing-subscriber init, anyhow at top
  balanced_ui/          # egui: shell (Menu bar, Account bar, Status bar, Main view), Register view,
                        #   Dialogs, Detached windows, theme, UICommand enum (see #7)
  balanced_core/        # domain + domain Commands; money, accounts, transactions; thiserror; NO egui
  balanced_i18n/        # message lookup + locale formatting (library chosen in #9/#13); NO egui
  balanced_store/       # (later) persistence; out of scope for map #5
xtask/                  # cargo xtask: i18n key completeness, layer check, release tasks
```

- **Layering:** `balanced → balanced_ui → {balanced_core, balanced_i18n}`. `balanced_core` and `balanced_i18n` are siblings and depend on neither egui nor each other. The UI formats core values through i18n. Rerun's "knows about egui" line falls at the same place: `viewer_support` knows egui, while `store`/`store_app` do not.
- **Why a separate `balanced_core`:**
  - It enforces "domain mutations go through commands" (#7) at the type level, because the UI can only call the core's public API.
  - It keeps domain unit tests and `cargo-mutants` runs (map #5) free of a GPU stack.
  - Incremental builds of the domain don't recompile egui code.
- **Why a separate `balanced_i18n`:** the fallback chain, completeness test and pseudo-locale (map #5) are testable without a UI, and `xtask` can reuse them.
- **Start small:** five crates is enough. matklad's advice is to stay flat, and a new crate is cheap later. Split `balanced_ui` (for example `balanced_register`) only when compile times or ownership demand it.
- **Enforce the invariant [config]:** pick one of:
  - a `cargo-deny` `[bans] deny = [{ crate = "egui", wrappers = ["balanced_ui", "eframe", …] }]` entry ("allows specific crates to have a direct dependency on the banned crate but denies all transitive dependencies on it", [cargo-deny bans config](https://embarkstudios.github.io/cargo-deny/checks/bans/cfg.html));
  - an `xtask` check over `cargo metadata`, which is what Rerun's script does.

---

## 2. Module organisation

- **File layout:** the Book calls `src/foo.rs` (plus `src/foo/bar.rs`) the idiomatic form and `src/foo/mod.rs` the "older style, still supported". Its downside: "many files named *mod.rs* … can get confusing". Mixing both styles in one project "is allowed but might be confusing" ([Book ch. 7.5](https://doc.rust-lang.org/book/ch07-05-separating-modules-into-different-files.html)).
  - Pick one style and enforce it with clippy `mod_module_files` (bans `mod.rs`) or `self_named_module_files` (bans `foo.rs` + `foo/`).
  - Rerun leaves `self_named_module_files` off, citing a clippy bug ([rust-clippy#9602](https://github.com/rust-lang/rust-clippy/issues/9602)), and its style guide still mentions `mod.rs`.
  - **[config]**: `mod_module_files = "warn"` matches the Book.
- **Thin `lib.rs`:** "Keep `lib.rs` and `mod.rs` short: mostly `mod` declarations and `pub use` statements that list what the crate exports … move functions and types with real logic into their own modules" ([Rerun CODE_STYLE.md, API design](https://github.com/rerun-io/rerun/blob/0.39.0/CODE_STYLE.md)). **[standard]**
- **Visibility:** "Keep items private by default, and make only the intended API `pub`" (Rerun). rust-analyzer enables `unreachable_pub = "warn"`, which flags `pub` items not reachable from the crate root and pushes them to `pub(crate)` ([Cargo.toml](https://github.com/rust-lang/rust-analyzer/blob/2026-10-12/Cargo.toml)). **[config]**
- **Re-exports, where sources disagree:**
  - rust-analyzer: "By default, avoid re-exports", because they create two paths to one item ([style.md](https://github.com/rust-lang/rust-analyzer/blob/master/docs/book/src/contributing/style.md)).
  - Rerun: `lib.rs` *is* a `pub use` list.
  - Reconcile them by flattening the crate's public API with `pub use` in `lib.rs` once, and never re-exporting another crate's items. **[standard]**
- **Architecture doc:** matklad recommends an `ARCHITECTURE.md` with three things ([2021-02-06](https://matklad.github.io/2021/02/06/ARCHITECTURE.md.html)):
  - a bird's-eye view;
  - a codemap ("where's the thing that does X?");
  - **architectural invariants, "especially those expressed as absences"** (for example "`balanced_core` does not depend on egui").

  "Keep it short", don't link files ("links go stale"), and revisit it a couple of times a year. **[standard]**

---

## 3. Naming and API conventions

The [Rust API Guidelines checklist](https://rust-lang.github.io/api-guidelines/checklist.html) is the shared baseline. Rerun's style guide says "We follow the Rust API Guidelines". Many of its items are aimed at published libraries. The ones that matter for an application's internal crates:

| Guideline | Rule | Lands as |
|---|---|---|
| C-CASE | RFC 430 casing | rustc `nonstandard_style` (default) **[config]** |
| C-CONV | `as_` (cheap, borrowed), `to_` (expensive), `into_` (consuming) | clippy `wrong_self_convention` (style) **[config]** |
| C-GETTER | getter is `foo()`, not `get_foo()` | **[standard]** |
| C-ITER / C-ITER-TY | `iter`/`iter_mut`/`into_iter`, iterator type names match | **[standard]** |
| C-COMMON-TRAITS, C-DEBUG | derive `Debug`, `Clone`, `PartialEq`/`Eq`, `Default` eagerly | clippy `derive_partial_eq_without_eq`; `missing_debug_implementations` (rustc, allow-by-default) **[config]** |
| C-CONV-TRAITS | conversions via `From`/`TryFrom`/`AsRef` | clippy `from_over_into`, `fallible_impl_from` **[config]** |
| C-GOOD-ERR | errors implement `Error + Send + Sync`; never `()` as an error; messages "lowercase without trailing punctuation" | `result_unit_err` (style) **[config]**; message style **[standard]** |
| C-NEWTYPE, C-CUSTOM-TYPE | newtypes for static distinctions; enums, not `bool`/`Option` arguments | clippy `fn_params_excessive_bools` (pedantic) and `max-fn-params-bools` **[config]**; rest **[standard]** |
| C-CTOR, C-BUILDER | constructors are inherent `new`/`with_*`; builders for complex values | **[standard]** |
| C-STRUCT-PRIVATE | private fields | **[standard]** |
| C-FAILURE | docs have `# Errors` / `# Panics` sections | clippy `missing_errors_doc`, `missing_panics_doc` (pedantic). Rerun allows both. **[config]** |

Further judgement rules from real repos that fit Balanced. These are **[standard]** unless marked:

- **Strong types over primitives:**
  - Use a newtype over a bare integer, an enum over a `bool`, a named struct over a tuple, and `Option<NonZeroU64>` over "0 means disabled".
  - "Parse, don't validate" (Rerun CODE_STYLE). This underpins #10/#14 (money types).
- **Preconditions in types:** "Express function preconditions in types and force the caller to provide them" (rust-analyzer style.md).
- **Control flow:** "push it to the caller", so a function does not hide branching (rust-analyzer).
- **Constructors:** "Prefer `Default` to zero-argument `new`" (rust-analyzer). Rerun: "Prefer constructors and associated functions over free functions".
- **No setters on pub fields:** "Never provide setters" (rust-analyzer). "Don't make a field `pub` if it also has a setter" (Rerun).
- **Avoid global state:** no `static FOO: LazyLock<…>`; "let the caller own it" (Rerun).
- **Generics at crate boundaries:** "Avoid making a lot of code type parametric, *especially* on the boundaries between crates" (rust-analyzer). This is a compile-time argument.
- **Exhaustive matches:** "Avoid catch-all `_ =>` arms when matching on our own enums" (Rerun), so new Command or Dialog variants are flagged by the compiler. Partly enforceable with clippy `wildcard_enum_match_arm` (restriction). **[config, optional]**
- **Destructure** `let Self { a, b, c } = self;` when touching most fields, so a new field forces a review (Rerun).
- **Sorted output:** when iterating `HashMap`/`HashSet`, sort before returning, or put `unsorted` in the name (Rerun). This matters for reproducible Register and report order. Partly enforceable with clippy `iter_over_hash_type` (Rerun enables it). **[config]**
- **Intentionally ignored `Result`:** prefer `foo().ok();` over `let _ = foo();` (Rerun). This is enforced by `let_underscore_must_use`. **[config]**
- **Comments:**
  - Write comments as proper sentences (rust-analyzer).
  - "Document behavior, not implementation". Don't narrate history or justify the change to the reviewer. "A wrong comment is worse than no comment" (Rerun).
- **Test-only code:** "Don't add traits, public methods, or hooks to production code only so that tests can use them" (Rerun). Test strategy itself is #8.
- **egui IDs:** derive from the parent with a hashed tuple, `ui.make_persistent_id(("plot_legend", view_id))`, not `format!` (Rerun). This complements #7's per-row `Id` advice.

---

## 4. Error handling

**Sources:**

- **anyhow README** ([dtolnay/anyhow](https://github.com/dtolnay/anyhow)):
  - "Use Anyhow if you don't care what error type your functions return, you just want it to be easy."
  - "Use thiserror if you are a library that wants to design your own dedicated error type(s)."
  - Add context with `.context("…")`, or `.with_context(|| format!(…))` when the message is costly to build.
- **Rerun** ([CODE_STYLE.md](https://github.com/rerun-io/rerun/blob/0.39.0/CODE_STYLE.md)):
  - "We use `thiserror` for errors in our libraries, and `anyhow` for type-erased errors in applications. Don't add `anyhow` to a library crate, not even for a one-off error: add a variant to the crate's error type instead."
  - Its rules list, all **[standard]** candidates:
    - "An error should never happen in silence."
    - "Handle each error exactly once. If you log it, don't pass it on. If you pass it on, don't log it."
    - "The code should only panic if there is a bug in the code."
    - "Don't `.map_err(|err| err.to_string())`" (it throws away the type).
    - "Don't drop an error with `.map_err(|_err| …)` unless you log it or wrap it first."
    - "Invalid configuration … is an error, not a silent fallback to the default."
    - "Put any sensitive data (like URLs, file paths etc) LAST in the error message." For a finance app, that means account names and amounts too.
    - Display errors with `{:#}` (the anyhow chain), not `{:?}`.
  - Use the **warning reporter pattern** for partial failures: a `&WarningReporter` parameter that accumulates structured warnings, because "it is harder to ignore an extra parameter". This fits imports (QIF/OFX) that succeed with skipped rows.
- **rust-analyzer** ([style.md](https://github.com/rust-lang/rust-analyzer/blob/master/docs/book/src/contributing/style.md)): "Use `anyhow::Result` rather than just `Result`". Also "Assert liberally", with `stdx::never!` (a log-and-continue assertion) preferred over `assert!` in an IDE that must not crash.
- **API Guidelines C-GOOD-ERR:** see §3.

**Where they disagree:** rust-analyzer uses anyhow in internal crates, while Rerun and the anyhow README restrict anyhow to applications. rust-analyzer's crates are effectively one application, and no caller ever matches on their errors. **Balanced:** `balanced_core` errors are matched by the UI to pick a Dialog message ("account name already exists" vs "file locked"), so they need typed `thiserror` enums. Anyhow belongs only in `balanced` (startup and IO glue) and possibly `xtask`.

**Panics: [config]**

- `clippy::unwrap_used = "warn"` (restriction; Rerun) with `allow-unwrap-in-tests = true` in `clippy.toml` (Rerun, egui).
- Optionally `expect_used`. Rerun does not enable it, and allows `expect("reason")` as a documented invariant.
- `clippy::indexing_slicing` (restriction) is stricter than either repo goes. Rerun handles indexing by prose: prefer `.get()`.

**Versions (crates.io, 2026-10-13):** thiserror 2.0.21, anyhow 1.0.104. Alternatives: snafu 0.9.2 (context selectors, one crate for both roles) and eyre 0.6.14 / color-eyre 0.6.5 (anyhow-like with pretty reports). None of the reference repos uses them.

---

## 5. Logging (`tracing`)

- **Libraries vs executables** ([tracing docs, "In libraries"/"In executables"](https://docs.rs/tracing/0.1.44/tracing/)):
  - "Libraries should link only to the `tracing` crate."
  - Executables install a subscriber. "libraries should *not* call `set_global_default()`!"
  - Balanced: `balanced_core`, `balanced_ui` and `balanced_i18n` depend on `tracing`, and only `balanced/main.rs` sets up `tracing-subscriber`. **[standard]**, partly **[config]** (`disallowed-methods` on `tracing::subscriber::set_global_default` outside the binary isn't expressible per crate, so this stays a review rule).
- **Spans vs events:** a span "represents a *period of time*", and an event "a *moment* in time". `#[instrument]` creates a span per call and records arguments with `Debug`. Be careful in a finance app: `#[instrument(skip(..))]` or `skip_all` keeps amounts and account data out of logs by default. **[standard]**
- **`log` compatibility:** the tracing macros match `log`'s syntax. The `log` feature emits `log` records when no subscriber is set, and `tracing-log` bridges the other way (egui and eframe log via `log`).
- **Levels:** Rerun's definitions ([CODE_STYLE.md "Log levels"](https://github.com/rerun-io/rerun/blob/0.39.0/CODE_STYLE.md)) are the most precise found. **[standard]**:
  - **ERROR:** "unrecoverable … Use this log level whenever some data is lost". Libraries should rather return `Err`.
  - **WARN:** "recoverable … If data is lost, it is an error and NOT a warning."
  - **INFO:** "mostly … only by application code", for rare, user-interesting events.
  - **DEBUG/TRACE:** if it fires every frame, it goes in TRACE.
  - Log strings with `{:?}` so they are quoted and escaped.
- **Rerun wraps tracing** in `re_log` (with `warn_once!`/`error_once!` for per-frame paths) and `re_tracing::profile_function!()` (puffin). A thin `balanced_log` is unnecessary at first. Add a `*_once!` helper when per-frame UI code needs it.
- **Versions:** tracing 0.1.44, tracing-subscriber 0.3.23, tracing-appender 0.2.5 (rolling file output, useful for "send us your log").

---

## 6. Edition and MSRV policy

- **Edition:** 2024, the current edition, used by all four reference repos. **[config]** `edition = "2024"` in `[workspace.package]`.
- **`rust-version` semantics** ([Cargo Book](https://doc.rust-lang.org/cargo/reference/rust-version.html)):
  - Cargo errors on older toolchains, and `cargo add` and resolver 3 pick dependency versions compatible with it.
  - Clippy's `msrv` "Defaults to the `rust-version` field in `Cargo.toml`", and about 80 lints adjust to it ([lint configuration](https://doc.rust-lang.org/clippy/lint_configuration.html#msrv)).
- **Policies Cargo documents:** the simplest is "always use the latest Rust version". Examples are "N-2", "every even release" and "calendar year". "Choose a policy … so users can compare it with their own policy". Verify with CI on that version (`cargo-msrv`, `cargo-hack`).
- **What the repos do:**

  | Repo | `rust-version` | Equivalent to | Pinned toolchain |
  |---|---|---|---|
  | Rerun | 1.96 | ≈ N-3 | `rust-toolchain` channel `1.96.0`, components `rustfmt`, `clippy` |
  | rust-analyzer | 1.98 | N-1 | not pinned (its `rust-version` file is a rustc commit for subtree sync) |
  | egui | 1.95 | — | — |
  | ripgrep | 1.85 | the 2024-edition floor | — |

  Libraries keep MSRV lower for their users. Apps track recent stable.
- **Recommendation for Balanced, an app with no downstream crates:** pin an exact stable in `rust-toolchain.toml` (for reproducible clippy output, since new clippy versions add lints) and set `rust-version` to the same value. Bump both together in a dedicated PR, perhaps every second release. egui's MSRV (1.95 at 0.36) sets the floor. **[config]**

---

## 7. rustfmt

- **Defaults are the norm.** Rerun: "We use `rust fmt` with default settings". Small diffs from default elsewhere:
  - rust-analyzer: `reorder_modules = true`, `use_small_heuristics = "Max"`;
  - ripgrep: `max_width = 79`, `use_small_heuristics = "max"`.
- **Stable vs nightly options** ([rustfmt Configurations.md](https://github.com/rust-lang/rustfmt/blob/master/Configurations.md)):
  - **Stable:** `edition`, `style_edition`, `max_width`, `use_small_heuristics`, `reorder_imports`, `reorder_modules`, `newline_style`, `use_field_init_shorthand`, `use_try_shorthand`, `hard_tabs`, `tab_spaces`, and the width knobs.
  - **Nightly-only:** `group_imports` ([#5083](https://github.com/rust-lang/rustfmt/issues/5083)), `imports_granularity` ([#4991](https://github.com/rust-lang/rustfmt/issues/4991)), `wrap_comments`, `format_code_in_doc_comments`.
  - Rerun wants `StdExternalCrate` grouping and `Module` granularity. Its guide admits stable CI "does not enforce them" and re-runs nightly fmt occasionally.
- **Recommendation [config]:** a `rustfmt.toml` with `style_edition = "2024"` and, optionally, `use_field_init_shorthand = true` and `use_try_shorthand = true`. Run `cargo fmt --check` in CI. Either add a nightly `cargo +nightly fmt --check` job for import grouping, or make import grouping a non-blocking **[standard]**. Don't write prose rules for anything rustfmt does.

---

## 8. Lints: which groups real projects enable

### Clippy's own guidance ([lints](https://doc.rust-lang.org/clippy/lints.html))

| Group | Default level | Guidance |
|---|---|---|
| `correctness` | deny | — |
| `suspicious`, `complexity`, `perf`, `style` | warn | `style` is "the most opinionated" |
| `pedantic` | allow | Can be enabled whole, but expect to use `#[allow]` generously |
| `restriction` | allow | "It is **not** recommended to enable the whole group, but rather cherry-pick". Clippy warns on `#![warn(clippy::restriction)]`. |
| `nursery` | allow | Cherry-pick only ("buggy or need more work") |
| `cargo` | allow | Mainly for published crates |

### What the repos do

| | Groups | Notable cherry-picks | Allowed despite group |
|---|---|---|---|
| **Rerun** ([Cargo.toml `[workspace.lints]`](https://github.com/rerun-io/rerun/blob/0.39.0/Cargo.toml)) | `all`, `pedantic`, `cargo` at warn (priority -1) | Around 70 from restriction/nursery: `unwrap_used`, `dbg_macro`, `todo`, `unimplemented`, `allow_attributes`, `str_to_string`, `string_add`, `iter_over_hash_type`, `let_underscore_must_use`, `use_self`, `needless_pass_by_ref_mut`, `large_stack_frames`, `undocumented_unsafe_blocks`, `exit`, `mem_forget` | Around 25 pedantic lints, each with a hit count or reason (`cast_*`, `must_use_candidate` "3134 hits", `missing_errors_doc`, `missing_panics_doc`, `items_after_statements`, `similar_names`, `too_many_arguments`, …) |
| **egui** ([Cargo.toml](https://github.com/emilk/egui/blob/0.36.2/Cargo.toml)) | `all` at warn | Around 170 individual warns, many of them pedantic, picked by name instead of enabling the group | — |
| **rust-analyzer** ([Cargo.toml](https://github.com/rust-lang/rust-analyzer/blob/2026-10-12/Cargo.toml)) | `correctness` and `perf` **deny**; `complexity`, `style`, `suspicious` warn; `restriction` explicitly allow | `dbg_macro`, `todo`, `print_stdout`, `print_stderr` ("CI raises these to deny"), `rc_buffer`, `str_to_string` | `too_many_arguments`, `type_complexity`, `single_match`, `new_ret_no_self`, … |
| **ripgrep** | no lint table | — | — |

**rustc lints** that Rerun and/or rust-analyzer turn on:

- `unsafe_code = "deny"` (Rerun), `unsafe_op_in_unsafe_fn`;
- `rust_2018_idioms`, `future_incompatible`, `nonstandard_style` (groups, priority -1);
- `elided_lifetimes_in_paths`, `unused_lifetimes`, `unused_extern_crates`, `trivial_numeric_casts`;
- `unreachable_pub` (rust-analyzer);
- `unexpected_cfgs` with a `check-cfg` list;
- `unknown_lints = "deny"` (Rerun), so a typo in a lint name fails.

Rustdoc: `[workspace.lints.rustdoc] all = "warn"`, `missing_crate_level_docs` (Rerun).

**`clippy.toml` patterns: [config]**

- Rerun and egui:
  - `allow-unwrap-in-tests = true`;
  - `avoid-breaking-exported-api = false` (an app has no exported API to protect);
  - `max-fn-params-bools`;
  - `too-many-lines-threshold`;
  - `msrv`.
- **Project-specific bans with a reason**, which turns prose rules into lints. These are the most valuable for a codebase written partly by agents:
  - Rerun bans `egui::hex_color` and `Color32::from_rgb` etc. ("Do not hard-code colors - declare them design_tokens.rs instead").
  - Rerun bans `egui_extras::TableBody::row` ("doesn't scale. Use `rows` instead"), which matches #7's Register finding.
  - egui bans `std::thread::spawn` ("Use `std::thread::Builder` and name the thread") and `std::dbg`.
  - rust-analyzer bans `std::process::Command::new` (replacement `toolchain::command`).
  - Balanced candidates:
    - hard-coded colours outside the theme module (#16);
    - `f32`/`f64` arithmetic on money (`disallowed-types` can't target arithmetic, so this is a newtype rule, #10/#14);
    - raw string literals in UI widgets (an i18n completeness concern, #9; probably an xtask or test rather than clippy);
    - `std::thread::spawn`;
    - `std::time::SystemTime::now` outside an injectable clock (testability, #8).

**Where they disagree, and a recommendation:**

- Rerun and egui accept pedantic noise in exchange for catching more.
- rust-analyzer stays on the defaults and denies only `correctness`/`perf`.
- ripgrep relies on review.

A new codebase has no existing hits, which removes Rerun's main cost (its allow-list is mostly hit counts). Agents also follow lints more reliably than prose. So, for Balanced: `all` + `pedantic` at warn (priority -1), Rerun-style restriction cherry-picks, and an allow-list that gives a one-line reason per lint. `-D warnings` goes in CI (`cargo clippy --workspace --all-targets -- -D warnings`), not `deny` in the manifest, so local iteration isn't blocked (the rust-analyzer comment "CI raises these to deny"). Every member crate needs `[lints] workspace = true`. **[config]**

---

## 9. Other repo hygiene seen in the reference repos

- **`cargo-deny`** (0.20.2) checks licences, advisories, duplicate versions and bans. Rerun has `deny.toml` and allows `multiple_crate_versions` in clippy because "we handle this with `cargo deny`". **[config]**
- **`.typos.toml`** (spell-check) and **`taplo.toml`** (TOML formatting) appear in Rerun and rust-analyzer (typos only). **[config]**, optional.
- **TODO policy:** in Rerun, CI fails on a bare `TODO`/`FIXME`/`todo!()`. Use `TODO(name)` or `TODO(#123)` instead, so nothing is left behind unattributed. **[config]** (a CI grep) plus **[standard]**.
- **Agent docs:** both Rerun and rust-analyzer now ship `AGENTS.md`/`CLAUDE.md` at the root. Rerun's points agents at `CODE_STYLE.md`, which is the same shape as Balanced's planned `CLAUDE.md → CODING_STANDARDS.md`.

---

## Where sources disagree (summary)

| Topic | Position A | Position B | Suggested for Balanced |
|---|---|---|---|
| Workspace nesting | Flat `crates/*` (matklad, rust-analyzer, egui) | Layer folders + CI check (Rerun, about 100 crates) | Flat; write the layer rule in `ARCHITECTURE.md` and check it with deny or xtask |
| `resolver` in virtual manifest | `"3"` is the 2024 default (Cargo Book) | Big repos still write `"2"` | `"3"` (MSRV-aware resolution) |
| Re-exports | Avoid (rust-analyzer) | `lib.rs` as a `pub use` list (Rerun) | Flatten own API once; don't re-export other crates |
| `anyhow` in internal crates | Yes (rust-analyzer) | Applications only (Rerun, anyhow README) | Binary only |
| Panics | Assert liberally, `never!` (rust-analyzer) | Avoid `unwrap`, lint it (Rerun) | `unwrap_used` warn; `expect` with a reason allowed; `debug_assert!` invariants |
| Clippy scope | Defaults + deny perf/correctness (rust-analyzer); none (ripgrep) | `pedantic` + `cargo` + cherry-picks (Rerun); cherry-picked pedantic (egui) | `pedantic` + cherry-picks, with reasoned allows |
| rustfmt | Defaults (Rerun) | Small tweaks (rust-analyzer, ripgrep) | Defaults + `style_edition`; import grouping only if a nightly fmt job is accepted |
| MSRV | Low for libraries (ripgrep 1.85) | Recent, pinned (Rerun N-3, rust-analyzer N-1) | Pinned recent stable = `rust-version` |
| `mod.rs` | Non-`mod.rs` is idiomatic (Book) | `mod.rs` still used (Rerun) | Non-`mod.rs`, enforced by `mod_module_files` |

---

## Sources

- Rust 1.99.0 announcement (2026-10-01): <https://blog.rust-lang.org/2026/10/01/Rust-1.99.0/>; release list <https://blog.rust-lang.org/releases/>
- Rust 2027 edition discussion (non-authoritative): <https://internals.rust-lang.org/t/we-have-only-two-years-to-prepare-the-rust-2027-edition/22494>
- Cargo Book: workspaces <https://doc.rust-lang.org/cargo/reference/workspaces.html>, resolver <https://doc.rust-lang.org/cargo/reference/resolver.html>, `[lints]` <https://doc.rust-lang.org/cargo/reference/manifest.html#the-lints-section>, `rust-version` <https://doc.rust-lang.org/cargo/reference/rust-version.html>
- The Rust Book ch. 7.5: <https://doc.rust-lang.org/book/ch07-05-separating-modules-into-different-files.html>
- Rust API Guidelines: <https://rust-lang.github.io/api-guidelines/checklist.html>, C-GOOD-ERR <https://rust-lang.github.io/api-guidelines/interoperability.html#c-good-err>
- Clippy: lint groups <https://doc.rust-lang.org/clippy/lints.html>, configuration <https://doc.rust-lang.org/clippy/configuration.html>, options <https://doc.rust-lang.org/clippy/lint_configuration.html>
- rustfmt options: <https://github.com/rust-lang/rustfmt/blob/master/Configurations.md>
- matklad, "Large Rust Workspaces" (2021-08-22): <https://matklad.github.io/2021/08/22/large-rust-workspaces.html>; "ARCHITECTURE.md" (2021-02-06): <https://matklad.github.io/2021/02/06/ARCHITECTURE.md.html>
- rust-analyzer `2026-10-12`: `Cargo.toml`, `rustfmt.toml`, `clippy.toml` at <https://github.com/rust-lang/rust-analyzer/tree/2026-10-12>; style guide <https://github.com/rust-lang/rust-analyzer/blob/master/docs/book/src/contributing/style.md>
- Rerun 0.39.0: `Cargo.toml`, `clippy.toml`, `rust-toolchain`, `CODE_STYLE.md`, `crates/README.md`, `ARCHITECTURE.md` at <https://github.com/rerun-io/rerun/tree/0.39.0>
- egui 0.36.2: `Cargo.toml`, `clippy.toml` at <https://github.com/emilk/egui/tree/0.36.2>
- ripgrep 15.2.0: `Cargo.toml`, `rustfmt.toml` at <https://github.com/BurntSushi/ripgrep/tree/15.2.0>
- anyhow README: <https://github.com/dtolnay/anyhow>; tracing 0.1.44 docs: <https://docs.rs/tracing/0.1.44/tracing/>
- cargo-deny bans config: <https://embarkstudios.github.io/cargo-deny/checks/bans/cfg.html>
- crates.io versions (2026-10-13): thiserror 2.0.21, anyhow 1.0.104, tracing 0.1.44, tracing-subscriber 0.3.23, tracing-appender 0.2.5, log 0.4.34, snafu 0.9.2, eyre 0.6.14, color-eyre 0.6.5, cargo-deny 0.20.2
