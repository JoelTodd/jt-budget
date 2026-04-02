# Rust Project Handoff

Build this project as straightforward, idiomatic Rust.

## Baseline

- Use the **stable** toolchain
- Use **Rust 2024 edition**
- Set an explicit **`rust-version`** in `Cargo.toml`
- Reach for nightly only when a feature genuinely requires it

```toml
[package]
name = "my_project"
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
```

## Core principles

- Write for clarity first
- Design APIs around real ownership needs
- Use types to express intent
- Make invalid states hard to represent
- Keep public APIs predictable and easy to use
- Aim for code another Rust developer can read quickly

## Structure

- Keep `main.rs` focused on startup, wiring, and top-level error handling
- Put reusable logic in `lib.rs` and supporting modules
- Split files once they start carrying more than one concern
- Use workspaces when multiple crates improve separation

Typical layout:

```text
src/
├─ lib.rs
├─ main.rs
├─ config.rs
├─ error.rs
├─ domain/
└─ services/
tests/
```

## API design

- Take ownership when the function needs ownership
- Borrow when the function only needs access
- Use enums to model real choices
- Use newtypes for domain identifiers and meaningful values
- Use builders for configuration-heavy setup
- Keep public struct fields private unless direct access is the right API

Implement standard traits where they fit:

- `Debug`
- `Clone`
- `PartialEq`, `Eq`
- `PartialOrd`, `Ord`
- `Hash`
- `Default`

Use standard conversions:

- `From`
- `TryFrom`
- `AsRef`
- `AsMut`

## Errors

- Return `Result<T, E>` for recoverable failures
- Use clear error types that implement `Debug`, `Display`, and `std::error::Error`
- Add `From` impls where they make propagation cleaner
- Use `panic!` for broken invariants and impossible states
- Use `expect()` where failure would indicate a bug or a deliberate startup assumption

## Async and concurrency

- Choose threads for naturally thread-shaped work
- Choose async for I/O-heavy or highly concurrent workloads
- With Tokio, keep lock usage simple and short-lived
- Prefer dedicated tasks and channels for async coordination

## Style and implementation

- Use iterator-based code when it reads well
- Use loops when they make control flow clearer
- Keep functions focused
- Keep data flow obvious
- Keep trait implementations unsurprising
- Add dependencies where they meaningfully improve the codebase

## Documentation

Document public code as part of the API.

Include where relevant:

- purpose
- examples
- `# Errors`
- `# Panics`
- `# Safety` for unsafe APIs

Prefer examples that can run as doctests.

## Testing

- Write unit tests for internal logic and edge cases
- Write integration tests for public behaviour
- Test happy paths and failure paths
- Keep tests deterministic and easy to understand

## Tooling

Use these commands as standard:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo doc --no-deps
cargo build --release
```

Use these when applying compiler-guided fixes or edition migrations:

```bash
cargo fix
cargo fix --edition
```

## CI baseline

Run this in CI:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo doc --no-deps
```

## Performance

- Measure before optimising
- Benchmark in release mode
- Keep ownership and allocation decisions visible
- Optimise proven bottlenecks
- Start with code that is clear enough to tune later

## Unsafe code

Use unsafe deliberately and contain it well.

- Keep unsafe blocks small
- Wrap unsafe internals in safe APIs
- Document why the code is sound
- Add a `# Safety` section to public unsafe APIs

## Minimum starting checklist

- stable toolchain
- Rust 2024
- explicit `rust-version`
- thin `main.rs`
- real `lib.rs`
- config and error modules
- CI with fmt, clippy, test, and doc
- defined error types
- private-by-default public API
- basic crate docs

## Standard

Aim for code that is:

- correct
- clear
- maintainable
- idiomatic
- low-surprise

Write the boring, solid version first.
