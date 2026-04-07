# Large Rust Modules Review

Date: 2026-04-07

## Scope

Reviewed with emphasis on refactoring, consolidation, unification, and modularisation opportunities in the largest Rust modules:

- `budget-app/src/repository.rs` (946 lines)
- `budget-app/src/setup.rs` (936 lines)
- `budget-core/src/month.rs` (901 lines)
- `budget-app/src/ui/editor.rs` (612 lines)
- `budget-app/src/state.rs` (531 lines)
- supporting context in `budget-app/src/app/*.rs`, `budget-app/src/ui/*.rs`, `budget-app/src/lib.rs`, and project docs

Project contracts checked:

- `docs/mvp-brief.md`
- `docs/calc-rules.md`
- `docs/state-machine-lifecycle.md`
- `docs/rust-bp.md`
- `docs/tui-bp.md`

Verification at review time:

- `cargo fmt --check`: pass
- `cargo clippy --all-targets --all-features -- -D warnings`: pass
- `cargo test`: pass
- `cargo build --release`: pass

## Executive Observations

- The repo has already improved materially from the earlier `app.rs` / `ui.rs` monoliths. The runtime and UI are now split by route, and that split is paying off.
- The remaining large-file pressure is now concentrated in infrastructure-heavy modules rather than one obvious god object.
- I did not find a must-fix correctness defect in this pass. The highest-value work now is structural: reducing duplicated flow logic and making module boundaries sharper.

## Findings

### 1. High: `budget-core/src/month.rs` still combines too many change axes

Evidence:

- `MonthId` parsing/formatting lives at `budget-core/src/month.rs:13-111`
- persisted document schema and serialisation live at `budget-core/src/month.rs:116-212`
- UI-oriented summary projection lives at `budget-core/src/month.rs:262-346`
- the main calculation engine lives at `budget-core/src/month.rs:429-640`

Why it matters:

- Changes to persistence, calculation rules, month identity, and summary presentation all churn the same file.
- That broadens review scope for otherwise local edits and makes `budget-core` harder to test in slices.
- `CalculatedMonth::summary_groups` is currently exported but appears unused in the repo, which is a signal that UI projection concerns are bleeding into core types without a clear owner.

Refactor direction:

- Split into `month_id.rs`, `month_document.rs`, `month_calculation.rs`, and either `month_projection.rs` or move summary projection into `budget-app`.
- Keep `calculate_month` as the single calculation authority, but move helper structs beside it.
- Re-export the public surface from `month/mod.rs` so callers do not pay migration cost immediately.

### 2. High: guided and editor flows still duplicate the same persistence lifecycle

Evidence:

- editor commit-and-save flow: `budget-app/src/app/editor.rs:76-134`
- guided commit-and-save flow: `budget-app/src/app/guided.rs:63-125`
- guided initial draft save: `budget-app/src/app/guided.rs:127-155`

Shared pattern repeated:

- apply `MoneyInput`
- recompute with `calculate_month`
- set persistence and sync state
- call `repository.save_month`
- rebuild success message
- map failures into `BlockingFailure` with a route-specific retry target

Why it matters:

- These paths encode the app’s strict save/sync contract.
- Duplication here is risky because small policy changes can diverge between guided creation and month editing.
- The duplication is semantic, not just textual. The runtime is re-implementing the same lifecycle boundary in two modules.

Refactor direction:

- Extract a shared document-persistence helper in `app/` that owns the "recalculate -> persist -> transition state" sequence.
- Keep route-specific success transitions separate, but centralise the persistence state machine and failure mapping.
- A small `PersistedDocument<TState>` or `save_document_with_retry(...)` helper would be enough; this does not need a framework.

### 3. High: `setup.rs` is still a multi-concern coordinator and adapter layer in one file

Evidence:

- mode resolution: `budget-app/src/setup.rs:106-135`
- flow orchestration: `budget-app/src/setup.rs:137-250`
- interactive prompts: `budget-app/src/setup.rs:346-438`
- GitHub repo parsing and remote normalisation: `budget-app/src/setup.rs:446-645`
- external command execution and auth checks: `budget-app/src/setup.rs:537-699`
- repository-path classification: `budget-app/src/setup.rs:701-729`

Why it matters:

- The file mixes policy decisions, terminal I/O, GitHub-specific logic, and process execution.
- That makes it hard to test setup behaviour without effectively retesting shell integration.
- There is also duplicated repository-name normalisation logic in `parse_github_repo_ref` and `github_remote_name_with_owner`, which should share one parser.

Refactor direction:

- Extract `setup/prompts.rs` for terminal I/O.
- Extract `setup/github.rs` for GitHub repo parsing, auth, creation, and remote checks.
- Extract `setup/command.rs` for shell execution helpers.
- Keep `setup/mod.rs` as a thin coordinator that assembles these pieces into a clear setup plan.

### 4. Medium: `repository.rs` is carrying both domain repository semantics and git/filesystem plumbing

Evidence:

- public repository operations dominate `budget-app/src/repository.rs:45-430`
- git helper layer sits in the same file at `budget-app/src/repository.rs:433-566`
- file durability helper `write_atomic` is also embedded there at `budget-app/src/repository.rs:544-566`

Why it matters:

- Month CRUD, repo gate semantics, git command execution, branch/upstream validation, and atomic file persistence are separate concerns with different test surfaces.
- Rename/delete/save currently share a lot of commit/push choreography, but that choreography is spread across method bodies and helpers rather than being modelled once.
- `create_month_draft` also calls `list_months()` and recalculates all prior months just to find the latest predecessor at `budget-app/src/repository.rs:282-295`, which is acceptable for MVP scale but couples draft creation to the heaviest repository read path.

Refactor direction:

- Extract a small `GitRepo` helper for command execution, upstream checks, and remote inspection.
- Extract a `MonthStore` for month file listing/loading/writing.
- Keep `Repository` as the façade that enforces the product contract.
- Consider a shared local-mutation helper for `save_month`, `rename_month`, and `delete_month` so commit-and-push policy is defined once.

### 5. Medium: `state.rs` mixes route state, field catalogue policy, and money-input widget behaviour

Evidence:

- route and retry state: `budget-app/src/state.rs:13-153`
- field identity and traversal policy: `budget-app/src/state.rs:155-323`
- editable money buffer behaviour: `budget-app/src/state.rs:325-464`

Why it matters:

- Almost every TUI module depends on this file, so it has become a gravitational centre.
- `FieldId::guided_steps` and `FieldId::editor_fields` duplicate config-order traversal logic at `budget-app/src/state.rs:166-203`.
- `FieldId` also owns labels, document reads, negativity rules, and section mapping, which is convenient but broad.

Refactor direction:

- Split into `route_state.rs`, `field_catalog.rs`, and `money_input.rs`.
- Introduce one canonical field descriptor list derived from config order, then have guided/editor traversal filter or reshape that list rather than rebuilding it separately.
- Keep `MoneyInput` independent of route state so its tests and future reuse stay local.

### 6. Medium: `ui/editor.rs` has healthy route ownership, but section rendering is still mostly copy-assembled

Evidence:

- layout switching: `budget-app/src/ui/editor.rs:15-60`
- section renderers: `budget-app/src/ui/editor.rs:291-612`

Patterns repeated across sections:

- build a table
- choose compact vs standard column widths
- create the same style/block/header/subtotal structure
- wire focus state into row styling

Why it matters:

- The route split is correct, but the next layer of duplication is now inside the editor renderer itself.
- Changes to section chrome, header treatment, or subtotal presentation will likely touch four separate renderers.

Refactor direction:

- Do not over-abstract the row content.
- Do extract a small shared helper for section table framing: header row, widths, block, tone, subtotal, and compact/full title behaviour.
- Keep row-building closures local per section so readability stays high.

### 7. Medium: `budget-core` currently exports UI projection types that look unowned

Evidence:

- `SummaryGroup` and `SummaryItem` are exported from `budget-core/src/lib.rs:11-15`
- their implementation appears only in `budget-core/src/month.rs:280-346` and no repo usage turned up in search

Why it matters:

- Unused exported API increases the surface the project feels obliged to preserve.
- These types are presentation-shaped rather than rule-shaped.

Refactor direction:

- Either remove them if they are dead, or move them behind a narrower projection module that explicitly serves the UI.
- If they are kept, add a concrete consumer so the ownership boundary is obvious.

### 8. Low: UI test modules are large mostly because snapshot matrices and fixtures are repeated

Evidence:

- repeated snapshot width/height cases in `budget-app/src/ui/tests.rs:14-41`, `105-130`, and `358-384`
- repeated fixture assembly in `budget-app/src/ui/test_support.rs:15-141`

Why it matters:

- Test intent is good, but the amount of repeated setup increases noise when UI expectations change.
- This is a maintainability issue, not a correctness issue.

Refactor direction:

- Introduce a small snapshot helper or macro for the standard terminal sizes: `80x24`, `105x48`, `210x48`.
- Replace ad hoc document construction with a shared sample-month builder plus per-test overrides.

## Positive Notes

- The runtime split across `app/navigation.rs`, `app/guided.rs`, `app/editor.rs`, and `app/failure.rs` matches the lifecycle doc far better than the previous monolith.
- `calculate_month` remains the single calculation authority, which preserves the "derived values are cache only" invariant well.
- Repository sync behaviour is still explicit and blocking, which is aligned with the project contract.
- UI coverage across `80x24`, `105x48`, and `210x48` is present and currently passing.

## Suggested Refactor Order

1. Extract shared persistence logic from `app/editor.rs` and `app/guided.rs`.
2. Split `budget-core/src/month.rs` into identity, document, calculation, and projection modules.
3. Break `setup.rs` into prompt, GitHub, and command-adapter modules.
4. Split `repository.rs` into façade + git/filesystem helpers.
5. Split `state.rs` into route state, field catalogue, and `MoneyInput`.
6. Tidy `ui/editor.rs` and the UI test scaffolding last, once the domain and runtime seams are clearer.

## Bottom Line

The repo is no longer suffering from one obvious monolith, but several large modules still combine adjacent concerns that change at different speeds. The next cleanup should not be a broad rewrite. It should be a targeted second pass that extracts shared lifecycle logic first, then sharpens the remaining boundaries around core month calculation, setup integration, repository plumbing, and UI field metadata.
