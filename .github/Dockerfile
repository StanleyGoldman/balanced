# CI's tools, baked in once instead of reinstalled by every step. Each workflow
# under .github/workflows/ builds it on its runner, with its layers in the
# GitHub Actions cache; there is no registry. Steps run in it through
# .github/in-image.sh. See docs/adr/0004-ci-image-matches-the-snapshot-machine.md
# and docs/adr/0010-ci-runs-on-github-actions.md.
#
# Ubuntu, not an official `rust:` image: the image snapshots compare pixels
# drawn by lavapipe, and the committed ones are drawn on an Ubuntu 26.04
# machine. The `rust:` images are Debian and ship a different Mesa, which draws
# different pixels. Move this line when the snapshot machine moves release,
# along with the runners (`runs-on`) in .github/workflows/.
FROM ubuntu:26.04

# - build-essential: the C linker rustc calls, and cc for build scripts.
# - git: `cargo xtask todo` lists files with `git ls-files`.
# - eframe links the windowing and GL libraries even in a build that never
#   opens a window.
# - mesa-vulkan-drivers is lavapipe, the software Vulkan device the snapshot
#   tests draw with; the agents have no GPU. libvulkan1 is its loader.
RUN apt-get update -qq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
      build-essential ca-certificates curl git pkg-config \
      libgl1-mesa-dev libvulkan1 libwayland-dev libx11-dev libxcursor-dev \
      libxi-dev libxkbcommon-dev libxrandr-dev mesa-vulkan-drivers && \
    rm -rf /var/lib/apt/lists/*

# Steps run as root, and the checkout belongs to whoever cloned it (the
# runner's user, or the user's own UID run by hand). Without this, git
# refuses the checkout with "detected dubious ownership" and `cargo xtask todo`
# fails. Each container lives for one step and checks out one repository.
RUN git config --system --add safe.directory '*'

# The layout the official `rust:` images use.
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

# The checks' own tools, and sccache, CI's compiler cache (ADR 0003), as
# pinned prebuilt binaries: an image rebuild never changes them behind our
# back, and nothing compiles. Bump a version here to move it. Installed before
# the toolchain so a toolchain bump doesn't refetch them.
ARG CARGO_DENY_VERSION=0.20.2
ARG TYPOS_VERSION=1.51.1
ARG NEXTEST_VERSION=0.9.148
ARG CARGO_MUTANTS_VERSION=27.1.0
ARG SCCACHE_VERSION=0.18.0
RUN mkdir -p "$CARGO_HOME/bin" && cd "$CARGO_HOME/bin" && \
    curl -fsSL "https://github.com/EmbarkStudios/cargo-deny/releases/download/${CARGO_DENY_VERSION}/cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl.tar.gz" | \
      tar -xz --strip-components=1 "cargo-deny-${CARGO_DENY_VERSION}-x86_64-unknown-linux-musl/cargo-deny" && \
    curl -fsSL "https://github.com/crate-ci/typos/releases/download/v${TYPOS_VERSION}/typos-v${TYPOS_VERSION}-x86_64-unknown-linux-musl.tar.gz" | \
      tar -xz ./typos && \
    curl -fsSL "https://get.nexte.st/${NEXTEST_VERSION}/linux" | tar -xz && \
    curl -fsSL "https://github.com/sourcefrog/cargo-mutants/releases/download/v${CARGO_MUTANTS_VERSION}/cargo-mutants-x86_64-unknown-linux-gnu.tar.gz" | \
      tar -xz && \
    curl -fsSL "https://github.com/mozilla/sccache/releases/download/v${SCCACHE_VERSION}/sccache-v${SCCACHE_VERSION}-x86_64-unknown-linux-musl.tar.gz" | \
      tar -xz --strip-components=1 "sccache-v${SCCACHE_VERSION}-x86_64-unknown-linux-musl/sccache" && \
    typos --version && cargo-deny --version && cargo-nextest nextest --version && cargo-mutants mutants --version && sccache --version

# rustup with no toolchain of its own, then the one rust-toolchain.toml names
# (its components included), so the version is written in one place. The build
# context is the checkout, but Dockerfile.dockerignore beside this file lets in
# that file alone, so a local build doesn't send `target/`.
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal --default-toolchain none
COPY rust-toolchain.toml /toolchain/rust-toolchain.toml
RUN cd /toolchain && rustup toolchain install && rustc --version && cargo clippy --version && cargo fmt --version
