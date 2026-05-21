# Developer Decision Document: Rust TUI Budgeting App

## Purpose

This document recommends a concrete technical stack for building the MVP of the personal budgeting app, explains why those choices fit the product, and records the main alternatives considered and rejected.

The app is:

- single-user
- local-first
- terminal-based
- manually updated once a month
- file-based
- synced across machines using Git
- designed around one dense editable monthly budgeting screen rather than ongoing transaction entry

The stack should reflect that reality.

---

## Recommended Stack

Use the following stack for the MVP:

- **UI:** Ratatui + Crossterm
- **Architecture:** synchronous event loop with explicit app state
- **Serialisation:** Serde
- **Config format:** TOML
- **Month data format:** TOML
- **Sync:** Git CLI invoked from the app
- **Money representation:** `i64` integer pence
- **Date/time:** `time`
- **Errors and logging:** `thiserror` + `anyhow` + `tracing`
- **Safe writes:** staged writes with `tempfile`
- **File coordination:** local file locking
- **Testing:** unit tests + property tests + snapshot tests

This is the recommended implementation direction because it is the best fit for the product rather than the most academically elaborate stack.

---

## 1. UI Layer

### Recommendation

Use **Ratatui** with **Crossterm**.

### Why

The app needs a terminal UI that can handle:

- a guided month-creation flow
- one dense editable monthly sheet
- inline value editing
- live recalculation
- visible section subtotals
- live overall balance difference
- validation and sync state indicators

Ratatui is the best fit for that kind of custom terminal layout. It gives enough control to build a spreadsheet-like or dashboard-like screen without forcing the app into a menu-and-dialogue structure.

Crossterm should be the terminal backend because it is cross-platform, pure Rust, and the default practical choice for Ratatui-based apps.

### Strengths

- good control over layout and rendering
- strong fit for a custom single-screen editor
- active and well-established in the Rust TUI ecosystem
- avoids over-abstraction

### Weaknesses

- more UI plumbing must be written by hand
- focus behaviour, field editing, and keyboard flow need explicit implementation
- not a full batteries-included app framework

### Decision

**Choose Ratatui + Crossterm.**

---

## 2. Application Architecture

### Recommendation

Use a **synchronous event loop** with **explicit app state**, split into a small workspace.

Suggested workspace structure:

- `budget-core`
- `jt-budget`
- optional `xtask` later for dev tooling or release helpers

### Why

This app mostly:

- reads and writes local files
- recalculates derived totals
- validates monthly budgets
- occasionally invokes Git
- updates a terminal screen

That is not an inherently async-heavy product.

A simple synchronous design will be easier to reason about, easier to test, and easier to debug than an async architecture.

Keeping domain logic in a separate `budget-core` crate allows the budgeting rules to remain testable and independent from the UI and sync layers.

### Strengths

- simpler control flow
- fewer moving parts
- easier testing of domain logic
- lower conceptual overhead for the team

### Weaknesses

- long-running operations must be handled carefully so the UI does not feel frozen
- some background work patterns are less convenient than in async systems

### Decision

**Choose a synchronous architecture with a small workspace split.**

---

## 3. Persistence Model

### Recommendation

Use **plain flat files in a Git-backed repository**.

### Why

This matches the product requirements directly:

- data should be inspectable
- data should be editable by hand if needed
- data should sync cleanly through Git
- the app should not depend on a database or hosted service
- the user wants to be able to continue the same draft on another machine

A file-based model is the natural fit.

### Suggested layout

    budget-repo/
      config.toml
      months/
        2026-03.toml
        2026-04.toml
      meta/
        state.toml

The exact structure can vary, but the key idea is:

- one config file
- one file per month
- small metadata files if necessary

### Strengths

- human-readable
- Git-friendly
- simple backup and recovery
- easy to inspect outside the app
- easy to debug

### Weaknesses

- the team must define stable file-writing behaviour
- the app must manage safe writes explicitly
- merge and sync rules must be kept strict

### Decision

**Choose flat files, not a database.**

---

## 4. Config Format

### Recommendation

Use **TOML** for configuration.

### Why

The config needs to be reasonably comprehensive and human-editable. It should define:

- accounts
- account ordering
- account sign behaviour
- savings pots
- default contributions
- default earmarks
- section ordering
- summary groupings
- validation tolerance
- carry-forward rules
- monthly confirmation rules
- display labels

TOML is an excellent fit for this kind of structured configuration in a Rust project.

### Strengths

- familiar in Rust projects
- pleasant to edit
- readable without being noisy
- works naturally with Serde

### Weaknesses

- deeper nested structures can get clunky
- preserving comments during programmatic rewrite is not automatic unless special tooling is used

### Decision

**Choose TOML for config.**

---

## 5. Month Data Format

### Recommendation

Start with **TOML** for month files. Keep **JSON** as a fallback option.

### Why

Month data needs to be:

- human-readable
- easy to inspect
- structured enough for typed deserialisation
- Git-diff friendly

TOML is the best first choice because it keeps the file format consistent with config and stays readable to humans.

If month files later prove awkward to serialise or rewrite in TOML, JSON is a valid fallback because it is simpler for machines, even if less pleasant for humans.

### TOML month files

#### Strengths

- pleasant to read and inspect
- consistent with config
- better for manual inspection

#### Weaknesses

- can become verbose with nested repeated structures
- comment/order preservation may need more specialised tooling

### JSON month files

#### Strengths

- very straightforward serialisation
- universal tooling support
- deterministic structure

#### Weaknesses

- less pleasant to hand-edit
- no comments
- visually noisier

### Decision

**Start with TOML month files. Use JSON only if TOML becomes annoying in practice.**

---

## 6. Serialisation

### Recommendation

Use **Serde** across all app data models.

### Why

Serde is the standard choice for Rust data serialisation and allows the same domain models to be serialised to and from TOML and JSON with minimal duplication.

That means:

- one set of Rust structs
- one domain model
- multiple storage formats if needed

### Strengths

- mature and standard
- integrates cleanly with TOML and JSON
- minimises serialisation boilerplate

### Weaknesses

- requires care around backward compatibility if data formats evolve
- some custom formatting behaviour may require handwritten serialisers

### Decision

**Choose Serde.**

---

## 7. Git and Sync Strategy

### Recommendation

Use the **system Git CLI**, invoked from the app.

Do not embed Git via a library for the MVP.

### Why

The app’s sync behaviour should be simple and strict:

- open app
- verify repo state
- pull safely
- refuse to continue if sync is broken
- autosave locally
- commit and push at safe points

Using the system Git binary is the best fit because it naturally reuses:

- the user’s existing Git installation
- existing SSH configuration
- credential helpers
- remotes
- auth behaviour

This avoids having to rebuild Git behaviour inside the app.

### Proposed sync behaviour

At minimum:

1. On app startup, verify the repo exists and is usable.
2. Pull with strict behaviour.
3. If pull fails because of divergence, auth problems, or other sync issues, block editing and show a clear error.
4. Allow local autosave only within a known-good sync state.
5. Commit and push at controlled points.

### Strengths

- simpler than embedded Git
- benefits from normal Git setup
- easier to reason about operationally
- fits the product requirement to stop on sync weirdness

### Weaknesses

- depends on Git being installed
- stderr/output parsing must be handled carefully
- behaviour may vary slightly with user environment

### Rejected alternatives

#### `git2` / libgit2

Powerful but drags too much Git behaviour into the app. More control than the product needs, with much more complexity around authentication and remote operations.

#### `gix` / gitoxide

Interesting and promising, but more ambitious than needed for MVP.

### Decision

**Use Git CLI integration, not embedded Git libraries.**

---

## 8. Money Representation

### Recommendation

Represent money as **`i64` integer pence**.

### Why

The app deals with:

- pounds and pence
- manual monthly budgeting
- fixed amounts
- validation against a tolerance of ±£1.00

That does not require arbitrary precision decimal arithmetic.

Using integer pence gives exact arithmetic and avoids floating point errors.

Examples:

- `£180.00` becomes `18000`
- `£1.00` becomes `100`
- validation tolerance becomes `-100..=100`

### Strengths

- exact
- simple
- fast
- easy to compare and validate
- ideal for this domain

### Weaknesses

- formatting/parsing must be done at the app boundary
- developers must remember that internal values are scaled

### Rejected alternative

#### Decimal crate

A decimal crate is technically valid, but it is unnecessary for this app and adds complexity without real benefit.

### Decision

**Use `i64` pence internally.**

---

## 9. Date and Time Handling

### Recommendation

Use the **`time`** crate.

### Why

The app needs only light date handling:

- month identifiers
- labels like `March 2026`
- timestamps for saves or updates

It does not need complex timezone logic, calendaring, or scheduling rules.

The `time` crate is a good fit for lightweight, clear date/time handling without dragging in more surface area than needed.

### Strengths

- lightweight
- focused
- sufficient for the app’s needs

### Weaknesses

- fewer “everything included” conveniences than some broader libraries

### Rejected alternatives

#### `chrono`

Perfectly acceptable, but broader than needed for this app.

#### `jiff`

Interesting and modern, but excessive for the product’s date needs.

### Decision

**Use `time`.**

---

## 10. File Locations and Safe Writes

### Recommendation

Use:

- standard app directories for config or local metadata
- local file locking
- staged writes with temporary files

### Why

The app should avoid corrupting month files and should not allow two local app instances to trample one another.

The basic strategy should be:

1. acquire a local lock where appropriate
2. write changes to a temporary file
3. atomically replace the target file
4. release lock

This improves reliability without inventing an entire storage subsystem.

### Strengths

- reduces risk of partial writes
- gives local safety against concurrent access
- fits a file-based design

### Weaknesses

- locking does not solve cross-machine conflicts
- some platform-specific behaviour still needs care

### Important note

Local file locking is only local protection. It does not replace strict Git sync rules.

### Decision

**Use local locking and staged writes.**

---

## 11. Error Handling and Logging

### Recommendation

Use:

- **`thiserror`** for typed internal errors
- **`anyhow`** at app boundaries
- **`tracing`** for structured logging

### Why

The app has a few clear error domains:

- config errors
- month file parse or validation errors
- file IO failures
- Git sync failures
- UI/runtime state errors

Typed errors make lower-level code clear and testable. `anyhow` keeps top-level app flow ergonomic. `tracing` gives useful structured diagnostics for sync, autosave, and file operations.

### Strengths

- clear separation between typed and top-level errors
- good developer visibility
- easier debugging of Git and file issues

### Weaknesses

- logging setup still needs deliberate design
- error messages must be curated for user clarity

### Decision

**Use `thiserror` + `anyhow` + `tracing`.**

---

## 12. Testing Strategy

### Recommendation

Use a combination of:

- **unit tests**
- **property tests**
- **snapshot tests**

### Why

This app has domain rules that are ideal for automated invariant testing.

### Unit tests should cover

- sign behaviour for asset vs liability accounts
- month total calculations
- carry-forward logic
- savings pot delta calculations
- validation tolerance logic
- config parsing
- month parsing and serialisation

### Property tests should cover

- arithmetic invariants
- carry-forward plus delta equals final
- validation behaves correctly around thresholds
- roundtrip serialisation for month data
- reshuffling does not break core accounting rules

### Snapshot tests should cover

- summary rendering
- key TUI states
- validation banners
- sync error views
- history list rendering

### Strengths

- catches logic bugs early
- good fit for financial calculations
- snapshot tests help keep UI regressions visible

### Weaknesses

- snapshot tests can become noisy if overused
- property tests require some care to keep generators useful

### Decision

**Use unit tests + property tests + limited snapshot tests.**

---

## 13. Packaging and Distribution

### Recommendation

Start with **plain Cargo-based builds** and keep release engineering simple.

### Why

This is an MVP. The team should not spend early time over-engineering build distribution.

Native builds are fine to start with. If release automation becomes annoying later, distribution tooling can be added.

### Strengths

- lowest setup cost
- keeps focus on product
- avoids premature release-pipeline complexity

### Weaknesses

- manual release steps may become irritating later
- cross-platform packaging will need more thought if distribution broadens

### Future option

If the project becomes something regularly distributed as binaries, release tooling can be added later.

### Decision

**Keep packaging simple at first.**

---

## Final Recommended Stack Summary

### Core stack

- **Ratatui**
- **Crossterm**
- **synchronous event loop**
- **small workspace split**
- **Serde**
- **TOML config**
- **TOML month files**
- **Git CLI integration**
- **`i64` pence**
- **`time`**
- **`thiserror` + `anyhow` + `tracing`**
- **local file locking**
- **staged writes**
- **unit + property + snapshot tests**

---

## Rejected Options

The following options were considered but are not recommended for MVP:

### Async-first architecture
Rejected because the app is not fundamentally a concurrency-heavy or network-heavy system.

### Cursive as the primary UI framework
Rejected because the app wants a custom editable monthly sheet more than a traditional dialogue-driven interface.

### Raw terminal handling without a TUI layer
Rejected because it would recreate solved problems and waste time.

### SQLite storage
Rejected because it clashes with the requirement for inspectable, editable, Git-friendly data.

### Embedded Git via `git2`
Rejected because it increases complexity around Git operations and auth without enough product benefit.

### Gitoxide-based sync for MVP
Rejected because it is more ambitious than necessary.

### Decimal money types
Rejected because integer pence is simpler and sufficient.

### YAML for config or data
Rejected because it is not the best fit here and adds unnecessary parsing ambiguity.

---

## Main Risks and Mitigations

### Risk: Git sync failures confuse users
**Mitigation:** make sync strict, blocking, and explicit. Do not attempt clever conflict resolution in-app.

### Risk: file corruption from interrupted writes
**Mitigation:** use staged writes and atomic replacement where possible.

### Risk: UI complexity grows messy
**Mitigation:** keep domain logic in a separate core crate and treat the TUI as a thin interaction layer.

### Risk: month file structure becomes awkward
**Mitigation:** start with TOML, but keep JSON as an acceptable fallback if real-world editing proves clumsy.

### Risk: too much abstraction too early
**Mitigation:** avoid extra UI frameworks and avoid async unless proven necessary.

---

## Final Conclusion

The recommended MVP stack is:

**Ratatui + Crossterm + synchronous app state + Serde + TOML config + flat month files in a Git repo + Git CLI sync + integer pence + `time` + `thiserror`/`anyhow`/`tracing` + local file locking + staged writes + strong automated tests.**

This is the right stack because it matches the actual product:

- local-first
- single-user
- inspectable
- Git-synced
- TUI-based
- monthly and manual
- small enough to stay understandable

Anything more elaborate at MVP stage would mostly be technical self-entertainment.
