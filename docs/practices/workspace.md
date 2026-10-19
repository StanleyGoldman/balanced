# Workspace, crates and API conventions

`ARCHITECTURE.md` is the crate map and its layer rules. This doc covers how to change the workspace and how to shape a crate's code.

## Adding a crate

- Put it in `crates/<name>/`, with the directory named exactly like the crate (`balanced_<thing>`). Keep `src/`, even for a one-file crate.
- Copy an existing member's `Cargo.toml`. Every member inherits `version`, `edition`, `rust-version` and `publish` from the workspace and ends with:

  ```toml
  [lints]
  workspace = true
  ```

  Without that table the crate silently loses every workspace lint.
- Decide which layer the crate sits in, and in the same PR add it to `LAYERS` in `xtask/src/layers.rs` and update the codemap and invariants in `ARCHITECTURE.md`. `cargo xtask layers` fails on a crate missing from `LAYERS`.
- Start with few crates. Split a crate only when compile times or ownership demand it.

## Dependencies

- Declare each dependency's version once, in `[workspace.dependencies]`. Members write `name.workspace = true`, adding `features = [..]` as needed.
- Pin every egui-family crate (`egui`, `eframe`, `egui_extras`, `egui_kittest`, …) to the same version, and upgrade them together as described in [egui](egui.md#upgrading-egui).
- Use the re-exports `eframe::egui` and `egui_kittest::kittest`, so `kittest` and `accesskit` never become direct dependencies that could split the graph.
- A version held below the latest gets a comment on its line saying why, as egui's own workspace does:

  ```toml
  foo = "1.2"  # 1.3 needs accesskit 0.25; egui 0.36 is on 0.24
  ```

- `Cargo.lock` is committed, and the `xtask` checks build with `--locked`.
- Adding a dependency can move crates it never touches to newer versions in `Cargo.lock`, and the `multiple-versions` ban in `deny.toml` then fails on a split the new crate didn't need. Read the whole `git diff Cargo.lock` after adding one, the dependency lists of existing packages too: where several locked versions satisfy a requirement, cargo can re-point an existing package to the newest (`tempfile` from `"getrandom 0.3.4"` to `"getrandom 0.4.3"`), which no `name` or `version` line shows. Edit any unrelated change back by hand; `cargo metadata --locked` confirms the lockfile still resolves.

## Modules

- Use `foo.rs` + `foo/bar.rs`, never `foo/mod.rs` (clippy `mod_module_files` enforces this).
- Keep `lib.rs` short: `mod` declarations and the `pub use` list that is the crate's public API. Logic lives in modules.
- Flatten the crate's own public API with `pub use` in `lib.rs`, once. Don't re-export another crate's items.

## Visibility

- Items are private by default. Make `pub` only what another crate calls, and use `pub(crate)` for everything shared inside the crate (`unreachable_pub` flags the rest).
- Struct fields are private. A struct with a setter keeps that field private.

## Naming and API shape

These follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html):

- Getters are `foo()`, not `get_foo()`.
- Conversions are named `as_` (cheap, borrowed), `to_` (expensive) or `into_` (consuming), and use `From`/`TryFrom`/`AsRef` where a trait fits.
- Derive `Debug`, `Clone`, `PartialEq`/`Eq` and `Default` wherever they make sense.
- Constructors are inherent `new` or `with_*`. Prefer `Default` to a zero-argument `new`, and a builder for values with many optional parts.
- Prefer associated functions over free functions.
- Express preconditions in argument types so the caller has to satisfy them. Push branching up to the caller instead of hiding it behind a flag argument.
- Keep generics off crate boundaries. Type-parametric public APIs cost every downstream crate compile time.
- Let the caller own state. Pass it in rather than keeping it in a `static` `LazyLock` or `OnceLock`.
- When a function touches most of a struct's fields, destructure it (`let Self { a, b, c } = self;`) so a new field forces a look.
- Return `HashMap`/`HashSet` contents in sorted order, or put `unsorted` in the function's name. The Register and reports must come out in a reproducible order.
- To drop a `Result` on purpose, write `foo().ok();`, not `let _ = foo();`.
