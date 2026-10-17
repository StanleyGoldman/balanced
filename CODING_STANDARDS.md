# Coding standards

How Balanced's Rust code is written. Code review checks changes against this file and the practice docs it points to.

Tools enforce whatever they can: lint levels in `[workspace.lints]` (`Cargo.toml`), bans in `clippy.toml`, formatting in `rustfmt.toml`, dependency rules in `deny.toml`, and the checks in `xtask`. Those files are the rule. The docs below hold only the judgement calls that tools can't check.

## Cross-cutting rules

- **`cargo xtask ci` passes before you push.** It runs fmt, clippy with `-D warnings`, the tests, `cargo deny check`, typos, the TODO check and the ADR numbering check (see [tooling](docs/practices/tooling.md)).
- **Silence a lint with `#[expect(lint, reason = "…")]`**, at the narrowest scope that works. The reason says why this case is the exception. An empty or generic reason fails review.
- **Respect the layers.** Nothing below `balanced_ui` knows about egui, and `balanced_core` and `balanced_i18n` don't depend on each other. See [ARCHITECTURE.md](ARCHITECTURE.md).
- **Domain changes travel as Commands.** UI code sends a `DomainCommand` or `AppCommand` and reads a snapshot back. It never mutates the domain in place. See [state](docs/practices/state.md).
- **Encode facts in types.** Use a newtype over a bare integer, an enum over a `bool` argument, a named struct over a tuple, and parse into a valid type at the boundary rather than validating later.
- **Match our own enums exhaustively.** A new Command or Dialog variant must show up as a compile error at every `match` that handles it. `wildcard_enum_match_arm` enforces this. For a foreign enum, put `#[expect(clippy::wildcard_enum_match_arm, reason = "foreign enum")]` on the `match`.
- **Write every user-visible string through the i18n layer:** `fl!` in `balanced_ui`. See [i18n](docs/practices/i18n.md).
- **Name the issue in every TODO:** `TODO(#123)`.
- **Comments describe behaviour**, as full sentences. History and justification belong in the commit message.
- **Give every held-back dependency version a comment saying why.**
- **Keep tests and test hooks apart.** Production code gets no trait, `pub` method or hook that exists only for tests. A test helper other crates need goes behind a `test-support` feature, as `Localizer::for_tests` does.

## Practice docs

Read the doc for the area you are changing:

- [workspace](docs/practices/workspace.md): crates, manifests, dependencies, modules, visibility and API naming. Read it when adding a crate, a dependency or a public item.
- [errors-and-logging](docs/practices/errors-and-logging.md): error types, panics and `tracing`. Read it when code can fail or logs.
- [state](docs/practices/state.md): Commands, snapshots and per-window UI state. Read it when code changes domain or app state.
- [egui](docs/practices/egui.md): panels, IDs, Dialogs, large lists, text entry and editable grids, colours, accessibility and egui upgrades. Read it for any change to `balanced_ui`.
- [theme](docs/practices/theme.md): the look, its colours and sizes, density, and painting Register stripes and gridlines. Read it when a widget needs a colour or a size.
- [i18n](docs/practices/i18n.md): Fluent messages, the `fl!` macro, translations, the two locales and the pseudo-locale. Read it when adding user-visible text, a number or date shown to the user, or a locale.
- [money](docs/practices/money.md): `Money`, rounding, and formatting and parsing amounts. Read it when code holds, combines, shows or reads an amount.
- [testing](docs/practices/testing.md): which test layer to use, UI tests and mutation testing. Read it when writing tests.
- [tooling](docs/practices/tooling.md): the toolchain, `cargo xtask` checks and the build cache. Read it when a check fails or before you change config.
