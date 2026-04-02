# jt-budget

Rust workspace for a terminal-first monthly budgeting app.

## Crates

- `budget-core`: money parsing/formatting, month/config models, calculations, validation, TOML persistence.
- `jt-budget`: CLI, Git-backed repository workflow, Ratatui UI, autosave, guided month creation, history/navigation.

## Run

Initialise a budget data repo:

```bash
cargo run -p jt-budget -- init /path/to/budget-repo --remote /path/to/remote.git
```

Run the app:

```bash
cargo run -p jt-budget -- run --repo /path/to/budget-repo
```

`init` creates a ready-to-run data repo. With `--remote`, it publishes `main` and configures `origin/main` so `run` passes the repo gate immediately. Without a remote, the repo runs in local-only mode. After startup, sync remains strict and uses `git pull --ff-only` plus blocking push failures. Each confirmed guided step or edited field autosaves the month file, commits it, and pushes when a remote is configured. Month files live in `months/YYYY-MM.toml`; cached derived values are written for inspection but recomputed on load.

The bundled Base24 palette lives in [`budget-app/theme.toml`](budget-app/theme.toml).

## Main keys

- Navigation: `n` create month, `Enter` open month, `m` rename month, `d` delete month, `r` refresh, `q` quit.
- Guided creation: type amount, `Enter` save/advance, `Esc` back.
- Monthly sheet: `Tab`/`Up`/`Down` move, `Enter` edit field, `Esc` back, `q` quit.

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo doc --no-deps
cargo build --release
```
