# jt-budget

`jt-budget` is a terminal-first monthly budgeting app for a manual zero-based
allocation workflow. It stores inspectable TOML budget files in a separate Git
repository so budget data can stay private while this app remains shareable.

It is not a transaction tracker or bank sync tool. The main workflow is to
create a month, enter the balances and allocations that matter for that monthly
budget session, and adjust the sheet until the difference is within tolerance.

## Prerequisites

- Rust stable, version 1.85 or newer, with Cargo.
- `git` for the budget-data repository and sync workflow.
- GitHub CLI (`gh`) when using the guided GitHub setup path. Local-only setup
  does not need it.
- A terminal that supports an interactive TUI. The app is developed for Linux
  terminals and is expected to work in WSL2 and other terminal environments
  supported by Ratatui and Crossterm.

## Install

Build and install the app from this checkout:

```bash
cargo install --path budget-app
```

For development without installing the binary, run app commands from this
workspace with `cargo run -p jt-budget -- ...`.

## First Run

Start the setup flow:

```bash
jt-budget setup
```

The default setup path can create a new private GitHub-backed budget repository
or connect this machine to an existing one. The same flow also supports
local-only setup and advanced adoption of an existing local repository.

Normal launches reopen the configured budget repository:

```bash
jt-budget
```

To open a budget repository for one launch without changing the saved default:

```bash
jt-budget --repo /path/to/budget-repo
```

When running from this checkout, the Cargo alias is equivalent:

```bash
cargo budget
```

## Budget Data

The app source repository and the budget-data repository are intentionally
separate. Do not place real budget files in this checkout.

A budget-data repository contains the user-editable config and month files,
including `config.toml` and `months/YYYY-MM.toml`. Confirmed guided steps and
edited fields autosave month data. When a remote is configured, saves use the
Git-backed sync path and sync failures block instead of being silently ignored.
Derived totals written to month files are only cached inspection data; the app
recomputes them from editable state when loading.

The bundled Base24 palette lives in [`budget-app/theme.toml`](budget-app/theme.toml).

## Main keys

- Navigation: `n` create month, `Enter` open month, `m` rename month, `d` delete month, `r` refresh, `q` quit.
- Guided creation: type amount, `Enter` save/advance, `Esc` back.
- Monthly sheet: `Tab`/`Up`/`Down` move, `Enter` edit field, `Esc` back, `q` quit.

## Support

This is a personal project published for source visibility. GitHub issues are
welcome for reproducible bugs in the app, but outside contributions are not
being solicited for the first public release. The project does not provide
personal budgeting advice.

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```
