# Large-Module Maintainability Addendum

Timestamp: 2026-04-02T11:28:00+01:00

Companion to: `docs/swarm-reviews/2026-04-02_112325_rust-style-best-practices-review.md`

## Purpose

`app.rs` and `ui.rs` are each doing too many jobs.

Current size markers:

- `app.rs`: 983 lines
- `ui.rs`: 2550 lines
- `state.rs`: 487 lines

The issue is not raw size. It is that these files mix concerns with different change patterns, failure modes, and test surfaces. That makes unrelated changes easier to couple and harder to review.

## Why It Matters

This MVP relies on:

- explicit state transitions
- strict handling of save and sync failures
- recomputing derived values from editable state
- usable terminal layouts at `80x24`, `105x48`, and `210x48`

When route logic, side effects, rendering, layout policy, and test scaffolding live together, local changes stop being local. That increases regression risk in exactly the parts of the app that are supposed to stay explicit and predictable.

## Current Problem

### `app.rs`

It currently mixes:

- runtime lifecycle
- input dispatch
- navigation flow
- guided flow
- editor flow
- persistence and retry handling
- mutation helpers
- route-level tests

That blurs the boundary between state transitions, side effects, and recovery behavior.

### `ui.rs`

It currently mixes:

- theme parsing and validation
- style mapping
- layout policy
- route rendering
- widget helpers
- test helpers
- snapshot and render assertions

That means theme, layout, rendering, and test infrastructure all collide in one place.

### `state.rs`

`state.rs` is not the main problem, but it should stay focused on shared durable state. It should not become a dumping ground for whatever gets extracted from the other two files.

## Refactor Goal

This is decomposition, not redesign.

The goal is to:

- preserve behavior
- preserve the current repository and file model
- preserve the explicit state-machine approach
- reduce the scope of future changes

## Recommended Direction

Split app logic by lifecycle boundary:

- runtime loop
- navigation flow
- guided creation flow
- editor flow
- failure and retry handling
- document update helpers

Split UI logic by concern:

- theme and semantic styling
- layout policy
- shared widgets/render helpers
- navigation rendering
- guided rendering
- editor rendering
- shared UI test harness

The point is not to produce a clever architecture. The point is to make each module have one dominant reason to change.

## Constraints

This cleanup should not change:

- month-file structure
- sync strictness
- guided step order
- editor field order
- supported terminal sizes
- the meaning of `BlockingFailure`

If any of that moves, the work has crossed from maintenance into feature-level redesign.

## Suggested Order

1. Split the shared UI concerns first: theme, layout, and render helpers.
2. Then split route-specific UI rendering.
3. Then split app flow by lifecycle boundary.
4. Move mutation helpers last, once flow ownership is clearer.

## Risks

Avoid:

- helpers that still mutate route state from all over the codebase
- circular dependencies between UI modules
- moving shared state types without a clear ownership boundary
- generic abstractions that hide the app’s lifecycle
- coupling UI modules to repository logic

The target is smaller and more obvious, not more abstract.

## Done Threshold

This cleanup is done only when all of the following are true:

- app flow is no longer implemented in one large mixed-concern module
- UI theme, layout, rendering, and test support are no longer implemented in one large mixed-concern module
- each extracted module has one primary reason to change
- visible behavior is unchanged across navigation, guided creation, editor flows, failure handling, and supported terminal sizes
- the existing implementation constraints listed above are still true
- standard formatting, lint, test, and release-build checks pass
- the docs still describe the implementation accurately

If those conditions are not met, the cleanup is not done.

## Recommendation

Treat this as a bounded maintenance task to schedule after the current correctness issues. It is not cosmetic. It directly reduces change risk in code that already carries too many responsibilities.