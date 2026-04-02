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
- Prefer British English spelling and conventions across prose, comments, and project-owned identifiers.

## Comments

- Follow Rust documentation guidance: document public APIs with `///` comments that explain purpose and, when relevant, `# Errors`, `# Panics`, or invariant expectations.
- Prefer comments that explain intent, invariants, safety boundaries, or why a choice was made. Do not add comments that merely restate the code.
- Add inline comments sparingly, only where control flow, layout policy, persistence semantics, or other behaviour would otherwise take time to infer.
- Keep comments accurate and local to the code they describe. If the code changes, update or remove stale comments immediately.
- Use comments to reinforce project contracts when they are easy to violate, such as derived-value recomputation, strict sync behaviour, or terminal layout constraints.
- If behaviour is unclear, inspect the surrounding code and run the relevant workflow before commenting so the comment reflects real behaviour rather than a guess.
