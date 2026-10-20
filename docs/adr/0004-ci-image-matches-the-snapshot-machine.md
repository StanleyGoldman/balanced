# The CI image builds on Ubuntu to match the snapshot machine's Mesa, with no registry

CI's tools live in `.github/Dockerfile`, built on `ubuntu:26.04` rather than an official `rust:` image. The image snapshot tests compare pixels drawn by lavapipe, Mesa's software rasterizer, and the committed snapshots are drawn on an Ubuntu 26.04 machine (Mesa 26.0.8 when this was decided). The `rust:` images are Debian-based and ship a different Mesa (bookworm's is 22.x), so their pixels would differ and every pull request would fail on images nobody changed. The Rust toolchain is installed from the repository's `rust-toolchain.toml`, copied into the build, so the version is written in one place. Each CI job builds the image as `balanced-ci:latest` with buildx and runs its steps in it through `.github/in-image.sh`. The layers live in the GitHub Actions cache, so when nothing has changed every layer hits and loading the image is the only cost. There is no registry.

Decided while planning Balanced's CI, following the "simplest start" in [egui testing tooling](../research/egui-testing-tooling.md) and quicken_clone's local-image setup (its ADR 0004).

## Considered options

- **An official `rust:` image:** rejected for the Mesa mismatch above.
- **A pinned Mesa build (`gfx-rs/ci-build`, as Rerun and wgpu use) in CI and locally:** the most reproducible, but it adds setup on every developer machine before anything has shown it's needed. It is the next step if Ubuntu's Mesa updates start causing snapshot churn.
- **Push the image to a registry:** rejected while the project has one maintainer. A registry adds a secret and a second trigger, and protects against no one yet.

## Consequences

- When Ubuntu ships a Mesa update, a rebuilt image picks it up the same way `apt upgrade` does on the snapshot machine. The two can drift between those updates. Snapshots failing in CI with no code change is that drift.
- A layer evicted from the cache rebuilds itself on the next run.
- Moving the snapshot machine to another Ubuntu release means moving this `FROM` line with it.
