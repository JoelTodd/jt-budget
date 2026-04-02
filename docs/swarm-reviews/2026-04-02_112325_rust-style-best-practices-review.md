# Rust Style And Best-Practices Review

Timestamp: 2026-04-02T11:23:25+01:00

## Baselines

- Rust Style Guide: <https://doc.rust-lang.org/style-guide/>
- Rust API Guidelines: <https://rust-lang.github.io/api-guidelines/>
- Clippy: <https://doc.rust-lang.org/clippy/>
- Project contracts: `docs/mvp-brief.md`, `docs/calc-rules.md`, `docs/state-machine-lifecycle.md`, `docs/tui-bp.md`, `docs/rust-bp.md`

## Verification

- `cargo fmt --check`: pass
- `cargo clippy --all-targets --all-features -- -D warnings`: pass
- `cargo test`: fail
- `cargo build --release`: pass

`cargo test` currently fails in 7 UI assertions tied to the bundled theme palette.

## Findings

### 1. High: create-month errors can escape the state machine instead of entering `BlockingFailure`

- Files: `budget-app/src/app.rs:161`, `budget-app/src/app.rs:458`
- `NavigationDialogue::Create` returns `self.start_guided_creation(month)` directly.
- `start_guided_creation` uses `create_month_draft(month)?`, so duplicate-month and repository faults can bubble out of the input handler and unwind the TUI loop instead of being converted into explicit recovery state.
- That conflicts with `docs/state-machine-lifecycle.md`, which requires repo-dependent failures to stop normal flow at the failed boundary.

Recommendation:
- Catch draft-creation errors at the dialogue boundary.
- Route repository faults into `BlockingFailure`.
- Keep only user-correctable input issues inline in the dialogue.
- Add regression coverage for "month already exists" and repository-failure creation paths.

### 2. High: rename/delete retry targets replay the mutation instead of retrying the failed boundary

- Files: `budget-app/src/app.rs:324`, `budget-app/src/app.rs:334`, `budget-app/src/app.rs:335`, `budget-app/src/app.rs:433`, `budget-app/src/app.rs:451`, `budget-app/src/repository.rs:169`, `budget-app/src/repository.rs:195`, `budget-app/src/repository.rs:243`, `budget-app/src/repository.rs:270`
- `rename_month` and `delete_month` mutate the worktree before `git push`.
- If the local file change and commit succeed but `push` fails, the stored retry target is `RetryTarget::RenameMonth` or `RetryTarget::DeleteMonth`.
- Retrying then re-runs the mutation against already-mutated state, so the retry can fail with "month does not exist" instead of retrying the pending push.

Recommendation:
- Separate "mutate locally" from "publish sync".
- Store a push-only retry target after a successful local commit.
- Add tests that simulate push failure after successful local rename/delete.

### 3. Medium: the repo does not currently meet its own test-health bar because theme assertions are stale

- Files: `budget-app/theme.toml:1`, `budget-app/src/ui.rs:2071`, `budget-app/src/ui.rs:2129`, `budget-app/src/ui.rs:2155`, `budget-app/src/ui.rs:2184`, `budget-app/src/ui.rs:2231`, `budget-app/src/ui.rs:2242`, `budget-app/src/ui.rs:2329`
- The bundled palette now uses values like `base00 = "#0c0d0e"` and `base09 = "#e6550d"`.
- Multiple UI tests still assert older raw RGB literals, which is why `cargo test` fails despite snapshots and runtime code otherwise building cleanly.
- This is not an MVP feature gap, but it is a concrete verification failure.

Recommendation:
- Decide whether `theme.toml` or the hard-coded expectations are the intended contract.
- Update the failing assertions in one pass.
- Prefer semantic assertions derived from the theme under test over duplicating raw RGB constants throughout the suite.

### 4. Medium: `MonthId` exposes invalid states through public fields and later relies on `expect`

- Files: `budget-core/src/month.rs:13`, `budget-core/src/month.rs:14`, `budget-core/src/month.rs:15`, `budget-core/src/month.rs:48`, `budget-core/src/month.rs:49`
- `MonthId` has public `year` and `month` fields, so external callers can construct `MonthId { year: 2026, month: 99 }` without using `new` or `parse`.
- `display_label` then calls `Month::try_from(self.month).expect("validated month id")`.
- That breaks the Rust API-guidelines preference for making invalid states hard to represent.

Recommendation:
- Make `MonthId` fields private.
- Keep construction behind validated constructors or `TryFrom`.
- Remove the runtime `expect` from normal public API paths.

### 5. Medium: `write_atomic` is atomic for visibility, but not durable against crash/power-loss scenarios

- Files: `budget-app/src/repository.rs:345`, `budget-app/src/repository.rs:351`, `budget-app/src/repository.rs:355`
- The implementation writes a temp file, flushes it, and renames it into place.
- It does not sync the temp file or parent directory, so a sudden crash can still lose a "successful" autosave on some filesystems.
- Given the MVP emphasis on resumable drafts and explicit save safety, this is worth calling out.

Recommendation:
- `sync_all` the temp file before persist and sync the parent directory after persist, or document that the current guarantee is atomic replacement only.

### 6. Low: logging setup hides all failure modes

- Files: `budget-app/src/logging.rs:9`, `budget-app/src/logging.rs:15`, `budget-app/src/logging.rs:18`, `budget-app/src/logging.rs:23`, `budget-app/src/logging.rs:29`
- Logging is intentionally non-blocking, which is fine for this MVP.
- The issue is that every failure path is swallowed silently, which weakens diagnosis when startup or repository-gate issues happen.

Recommendation:
- Keep logging non-blocking, but return a `Result` or emit a fallback `eprintln!` on setup failure.

## MVP Alignment Check

The findings above were trimmed against `docs/mvp-brief.md`.

Still aligned with the MVP:

- Money is kept in integer minor units end to end.
- Derived values are recomputed on load and treated as cache only.
- The app remains file-based and inspectable.
- Sync/save behaviour is intentionally strict and blocking.
- Guided creation order and main editing shape match the documented monthly workflow.

Advice intentionally excluded:

- No transaction tracking, bank sync, audit trail, automation, or daily-spend logic.
- No recommendation to weaken blocking save/sync failures into best-effort background behaviour.
- No push toward a non-file-backed data model.

## Positive Notes

- `main.rs` is appropriately thin.
- `budget-core::calculate_month` is the single calculation authority, which matches the calc-rules doc well.
- The repo gate is explicit and enforced on open.
- Snapshot coverage exists for `80x24`, `105x48`, and `210x48`.
- The tree is already clean under strict `clippy`.

## Residual Risks

- `budget-core/src/money.rs:31`, `budget-core/src/money.rs:44`, `budget-core/src/money.rs:58`, `budget-core/src/money.rs:72`, and `budget-core/src/money.rs:164` use unchecked `i64` arithmetic and `abs`; that is acceptable for normal MVP-sized values, but hand-edited extreme inputs can still panic or overflow.
- `budget-app/src/app.rs` and `budget-app/src/ui.rs` are both large multi-concern modules. That is more of a maintainability pressure than an immediate MVP defect, so it is better treated as follow-up cleanup after the functional issues above.

## Suggested Order

1. Fix create-month error routing.
2. Fix rename/delete retry semantics after push failure.
3. Reconcile `theme.toml` with the failing UI assertions.
4. Harden save durability.
5. Tighten `MonthId` invariants and quiet failure handling.
