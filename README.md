# jt-budget

Rust workspace for a terminal-first monthly budgeting app.

## Crates

- `budget-core`: money parsing/formatting, month/config models, calculations, validation, TOML persistence.
- `jt-budget`: CLI, Git-backed repository workflow, Ratatui UI, autosave, guided month creation, history/navigation.

## Run

First use:

```bash
jt-budget setup
```

GitHub-first setup expects `git` and GitHub CLI (`gh`). Local-only setup and advanced local adoption still exist inside the setup flow.

Normal launch:

```bash
jt-budget
```

One-shot or scripted launch:

```bash
jt-budget --repo /path/to/budget-repo
```

From this workspace checkout, `.cargo/config.toml` adds:

```bash
cargo budget
```

`setup` now defaults to a GitHub-first flow: it can create a new private GitHub-backed budget repo, connect this machine to an existing GitHub budget repo, or fall back to local-only or advanced local adoption. During GitHub setup it uses `gh` for authentication and repo creation, configures Git credentials for later plain `git pull` and `git push`, validates the resulting repo through the normal repository gate, and only then saves it as the default launch target. If no default has been configured yet, plain `jt-budget` starts that setup flow interactively. `init` and `run --repo ...` still exist as compatibility commands. After startup, sync remains strict and uses `git pull --ff-only` plus blocking push failures. Each confirmed guided step or edited field autosaves the month file, commits it, and pushes when a remote is configured. Month files live in `months/YYYY-MM.toml`; cached derived values are written for inspection but recomputed on load.

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
