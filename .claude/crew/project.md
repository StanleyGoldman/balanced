# Project profile

## Language

Rust (stable toolchain; `cargo 1.99`)

## Build

`cargo build`

## Test

`cargo test`

## Run

`cargo run -- <args>`

## Format and lint

`cargo fmt --check`
`cargo clippy -- -D warnings`

## CI

none yet: the verifier's local test run is the only check (Woodpecker CI comes later)

## Conventions

- Branches: `issue-<n>-<slug>`
- Squash merges only; the user merges, agents never do
- Commits and PRs reference the issue: `Closes #<n>`
- Domain language lives in `CONTEXT.md`; decisions in `docs/adr/`

## Main branch

`main`
