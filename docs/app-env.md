# Developer Handoff: WSL2 Ubuntu Budget App Dev Environment

## Purpose

This document records what is installed and available on the WSL2 Ubuntu instance for development of the Rust TUI budgeting app.

The aim of this setup is to provide a clean baseline for local Rust development with Git-based sync support.

---

## Installed Environment

### System packages

Installed via `apt`:

- `build-essential`
- `pkg-config`
- `git`
- `openssh-client`
- `curl`
- `ca-certificates`

### Rust toolchain

Installed via `rustup`:

- stable Rust toolchain
- `cargo`
- `rustc`

### Rust components

Added via `rustup component add`:

- `rustfmt`
- `clippy`
- `rust-analyzer`

---

## Installed Tools and Their Role

## 1. Native Build Toolchain

### Installed

- `build-essential`

### What it provides

The standard Ubuntu native compilation toolchain, including:

- `gcc`
- `g++`
- `make`
- standard development headers and related build support

### Why it is available

This supports Rust builds on Linux where crates may rely on a working native toolchain during compilation.

---

## 2. Build Configuration Support

### Installed

- `pkg-config`

### What it provides

The standard Linux utility for discovering compiler and linker flags for native libraries.

### Why it is available

This supports build scripts that may need to detect system libraries during compilation.

---

## 3. Git Support

### Installed

- `git`

### What it provides

The Git command-line client is available in the environment.

### Why it is available

This supports local repository operations and the planned Git-based sync workflow.

---

## 4. SSH Support

### Installed

- `openssh-client`

### What it provides

Standard SSH client tooling, including `ssh`.

### Why it is available

This supports authentication and access to Git remotes over SSH.

---

## 5. HTTPS Fetch Support

### Installed

- `curl`
- `ca-certificates`

### What they provide

Secure HTTPS download capability and certificate trust validation.

### Why they are available

These support bootstrap and other secure remote fetch operations during development.

---

## 6. Rust Toolchain

### Installed

Via `rustup`:

- stable Rust toolchain
- `cargo`
- `rustc`

### What it provides

The standard Rust development environment for:

- compiling code
- managing dependencies
- building binaries
- running tests
- running project commands

---

## 7. Rust Development Components

### Installed

Via `rustup component add`:

- `rustfmt`
- `clippy`
- `rust-analyzer`

### What each one is for

#### `rustfmt`

The standard Rust code formatter.

#### `clippy`

The standard Rust linter.

#### `rust-analyzer`

The standard Rust language server backend used by editors for completion, diagnostics, navigation, and refactoring support.

---

## Available Commands

The following commands should be available in the shell:

- `rustc`
- `cargo`
- `rustfmt`
- `cargo fmt`
- `cargo clippy`
- `git`
- `ssh`
- `curl`
- `pkg-config`
- `gcc`
- `g++`
- `make`

---

## Environment Status

The instance is ready for:

- creating Rust projects with Cargo
- building and running the app locally
- adding Rust crate dependencies
- formatting code
- linting code
- running tests
- working with Git repositories
- authenticating to remotes over SSH
