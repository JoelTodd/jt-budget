# TUI Review

Date: 2026-04-01 10:53 BST

## Findings
- Rename error handling is too heavy for a local dialog mistake. Submitting an unchanged month name pushes the user out of navigation and into the full `Blocking Failure` screen instead of keeping the error inline in the rename flow. This breaks the app's otherwise direct, keyboard-native rhythm for a no-op validation case.
- Focus and edit state are not legible enough once a month is open. On the monthly sheet, the selected field is easy to lose in a fast scan until edit mode adds the cursor/underscore. That falls short of `docs/tui-bp.md`'s requirement that focus stay visible at all times and that focus, selection, and editing read as distinct states.
- Guided creation is the weakest supported view at `80x24`. The smallest layout keeps the current step, live preview, and status footer all visible, but the active input area ends up cramped and visually secondary. The flow remains usable, yet it loses the "primary task area usable at all times" standard more than the larger sizes do.
- The `210x48` layout is clear but over-framed. Extra width mostly turns into more bordered regions rather than stronger hierarchy, so the wide profile feels heavier than it needs to for an app whose job is fast monthly editing.

## Suggestions
- Keep rename validation inside the dialog whenever the app can decide it locally, including unchanged input and malformed month ids. Reserve `Blocking Failure` for repo, save, load, and sync faults that actually block safe continuation.
- Strengthen idle focus in the month sheet with a row-level selected treatment, not just a subtle cell background and edit cursor. A stronger highlight, accent marker, or section-local focus label would make keyboard position obvious before editing starts.
- Rebalance compact guided creation for `80x24` by demoting always-visible helper copy and shrinking the preview footprint so the active step and next action get more vertical room. If something has to compress first, it should be secondary summary, not the input path.
- Trim some chrome in the wide profile. The app already has clear sectioning and stable totals; at `210x48`, reducing bordered repetition or slimming the validation footer would make the workspace feel more deliberate without changing scope.

## MVP Check
- The reviewed flow matches `docs/mvp-brief.md`: manual month creation, guided setup, full-sheet editing, section subtotals, overall difference, validation, autosave, history actions, and strict blocking failures for real repo problems are all present.
- No review point depends on adding out-of-scope features like advice, analytics, forecasts, or transaction tracking.
- `105x48` is the most balanced baseline. `80x24` is functional but compressed in guided creation, and `210x48` improves comfort more than capability.
