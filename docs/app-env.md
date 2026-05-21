# App Environment

## Purpose

This document gives generic environment guidance for building and running
`jt-budget`. It is not a handoff record for one development machine.

## Supported And Expected Environments

`jt-budget` is developed for interactive Linux terminals. WSL2 Ubuntu is a
supported development path when it provides the tools below. The terminal UI is
built with Ratatui and Crossterm, so other terminal environments supported by
that stack may work, but should be verified before relying on them.

The app expects:

- a terminal session with interactive stdin and stdout
- a filesystem location for a separate budget-data repository
- Git access for repository-backed persistence and sync

## Required Tools

Install:

- stable Rust 1.85 or newer through `rustup` or an equivalent toolchain source
- Cargo
- `git`

For development, also install the Rust components used by the project checks:

```bash
rustup component add rustfmt clippy
```

`rust-analyzer` is useful for editors, but is not required to build or run the
app.

## Optional Tools

- GitHub CLI (`gh`) is required only for the guided GitHub setup path that
  creates or connects a private GitHub-backed budget repository.
- SSH client tooling is needed when the chosen Git remote uses SSH.
- Linux native build tools such as a C compiler and `pkg-config` may be needed
  if the local Rust toolchain or dependency build environment requires them.

On Ubuntu or WSL2 Ubuntu, a practical baseline is:

```bash
sudo apt install build-essential pkg-config git openssh-client curl ca-certificates
```

Install Rust separately, then verify `cargo`, `rustc`, and `git` are available.

## Project Commands

Run app commands from the workspace with:

```bash
cargo run -p jt-budget -- setup
cargo run -p jt-budget -- --repo /path/to/budget-repo
```

Run the project checks with:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```
