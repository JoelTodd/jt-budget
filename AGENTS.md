# AGENTS.md

## Scope

- Read the relevant files in `docs/` before changing behaviour. Those docs are the project contract.
- Keep changes inside the current MVP unless the task explicitly expands scope.

## Project invariants

- Derived values are cache only. Recompute from editable state and never trust stored derived fields.
- Keep money in integer minor units end to end.
- Keep the app file-based and inspectable.
- Keep sync strict and explicit. First-run setup may prepare the repo, but normal sync/save failures must block rather than being silently tolerated.

## Working loop

- Use `cargo run -p jt-budget -- ...` for app commands.
- Before finishing code changes, run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test`
  - `cargo build --release`

## UI changes

- Preserve deliberate support for `80x24`, `105x48`, and `210x48`.
- Update snapshot coverage when rendering changes.
- Manually smoke-test the relevant workflow in a PTY when changing TUI behaviour.

## Docs

- Keep `README.md` concise.
- Update docs only when implementation meaningfully diverges from them, and prefer small focused edits.
