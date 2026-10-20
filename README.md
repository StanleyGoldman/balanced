# Balanced

A desktop personal-finance application in the tradition of Quicken classic: one main window with a menu bar, an account list, and account registers. It is written in Rust on [egui](https://github.com/emilk/egui)/eframe.

Balanced is early in development. The app runs on a built-in stub ledger and doesn't open or save data files yet.

## Running

```sh
cargo run -p balanced
```

`rust-toolchain.toml` pins the toolchain, and rustup installs it on the first build.

On Linux, the window needs the X11 and Wayland development libraries. On Debian or Ubuntu:

```sh
sudo apt install build-essential pkg-config libgl1-mesa-dev libwayland-dev \
  libx11-dev libxcursor-dev libxi-dev libxkbcommon-dev libxrandr-dev
```

Environment variables:

| Variable | Effect |
| --- | --- |
| `BALANCED_LOCALE` | Use this language tag instead of the OS locale, e.g. `de-DE`, or `en-XA` for the pseudo-locale. Empty means the OS locale. |
| `RUST_LOG` | Log filter for stderr, e.g. `debug`. The default is `info`. |

Preferences are saved to the OS config directory, such as `~/.config/balanced/` on Linux.

## Checks

```sh
cargo xtask ci
```

This runs every check CI runs: formatting, clippy, tests, image snapshots, locales, `cargo deny`, typos, TODOs, SPDX headers, ADR numbering and commit sign-offs. Run `cargo xtask` with no task to list each check on its own.

Some checks need tools beyond the toolchain: [cargo-nextest](https://nexte.st), [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) and [typos](https://github.com/crate-ci/typos), plus [cargo-mutants](https://mutants.rs) for `cargo xtask mutants`. The image snapshots draw with Vulkan, so a machine without a GPU needs a software driver such as Mesa's lavapipe (`mesa-vulkan-drivers`). `.github/Dockerfile` pins the versions CI uses.

## Documentation

- [`GLOSSARY.md`](GLOSSARY.md): the domain language (Main window, Account bar, Register, …).
- [`ARCHITECTURE.md`](ARCHITECTURE.md): the crates and the rules between them.
- [`CODING_STANDARDS.md`](CODING_STANDARDS.md): the rules for Rust code.
- [`docs/adr/`](docs/adr/): architecture decision records.
- [`CONTRIBUTING.md`](CONTRIBUTING.md): the license your contribution is under, and signing off commits.

## License

Balanced is licensed under the GNU General Public License, version 3 or later, with the Balanced Plugin Exception (`GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception`):

- [`LICENSE`](LICENSE): the GPL, version 3.
- [`LICENSE-EXCEPTION.md`](LICENSE-EXCEPTION.md): the plugin exception, which lets an extension that uses only Balanced's designated extension interface be distributed under any license. It is a draft awaiting legal review.
- [`TRADEMARKS.md`](TRADEMARKS.md): the "Balanced" name and logo aren't covered by the license.

[ADR 0009](docs/adr/0009-gpl-3-or-later-with-a-plugin-exception-and-dco-sign-off.md) records why.
