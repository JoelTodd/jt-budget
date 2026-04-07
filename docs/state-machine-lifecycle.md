# State Machine / Lifecycle Doc

## Purpose

Define the app's runtime states and allowed transitions so control flow stays explicit and predictable.

Repo discovery and first-run setup happen before this TUI lifecycle starts. Once a repo path has been chosen, launch still enters `Bootstrap` and must pass through `RepositoryGate` before normal use.

---

## Top-Level States

1. `Bootstrap`
2. `RepositoryGate`
3. `Navigation`
4. `GuidedCreation`
5. `MonthEditing`
6. `BlockingFailure`
7. `Shutdown`

Only one top-level state is active at a time.

---

## 1. Bootstrap

**Purpose:** start the app and prepare runtime state.

**Does:**
- initialise runtime
- load config
- prepare paths, logging, locks, and in-memory state

**Transitions:**
- success -> `RepositoryGate`
- recoverable failure -> `BlockingFailure`
- fatal failure -> `Shutdown`

---

## 2. RepositoryGate

**Purpose:** decide whether the repo is safe to use.

**Substates:**
- `Checking`
- `Syncing`
- `Ready`
- `Blocked`

**Transitions:**
- `Checking -> Syncing`
- `Checking -> Blocked`
- `Syncing -> Ready`
- `Syncing -> Blocked`
- `Ready -> Navigation`
- `Blocked -> BlockingFailure`

This state must be passed through before normal use.

---

## 3. Navigation

**Purpose:** non-editing state for choosing what to do next.

**Substates:**
- `MonthList`
- `OpeningMonth`
- `CreatingMonth`

**Transitions:**
- `MonthList -> OpeningMonth`
- `MonthList -> CreatingMonth`
- `OpeningMonth -> MonthEditing`
- `OpeningMonth -> BlockingFailure`
- `CreatingMonth -> GuidedCreation`
- `CreatingMonth -> BlockingFailure`

---

## 4. GuidedCreation

**Purpose:** collect initial values for a new month.

**Substates:**
- `StepActive`
- `StepError`
- `Autosaving`
- `Complete`

**Transitions:**
- `StepActive -> StepError`
- `StepError -> StepActive`
- `StepActive -> Autosaving`
- `Autosaving -> StepActive`
- `StepActive -> Complete`
- `Complete -> MonthEditing`
- any substate -> `BlockingFailure`

Notes:
- once a draft exists, leaving this flow must preserve resumability
- this is only for new months

---

## 5. MonthEditing

**Purpose:** main editing state for an existing or newly created month.

**Interaction substates:**
- `SheetIdle`
- `FieldEditing`

**Persistence substates:**
- `Clean`
- `Dirty`
- `Autosaving`
- `SaveFailed`

**Sync substates:**
- `SyncPending`
- `Syncing`
- `SyncFailed`

**Validation substates:**
- `Valid`
- `Invalid`

These are best treated as separate dimensions, not one flat enum.

**Core transitions:**
- `SheetIdle -> FieldEditing`
- `FieldEditing -> Dirty`
- `Dirty -> Autosaving`
- `Autosaving -> Clean`
- `Autosaving -> SaveFailed`
- `Clean -> SyncPending`
- `SyncPending -> Syncing`
- `Syncing -> Clean`
- `Syncing -> SyncFailed`
- `Valid <-> Invalid`

**Allowed exits:**
- `MonthEditing -> Navigation`
- `MonthEditing -> BlockingFailure`
- `MonthEditing -> Shutdown`

Rules:
- recalculation is always based on current in-memory state
- local save success does not imply sync success
- validation status does not create a special lock state

---

## 6. BlockingFailure

**Purpose:** stop normal use until the problem is fixed or the user exits.

**Typical causes:**
- config load failure
- repo unavailable or invalid
- sync failure
- month file unreadable or invalid
- lock failure
- save failure

**Substates:**
- `Shown`
- `Retrying`
- `Exiting`

**Transitions:**
- `Shown -> Retrying`
- `Retrying -> RepositoryGate`
- `Retrying -> Navigation`
- `Retrying -> MonthEditing`
- `Retrying -> Shown`
- `Shown -> Shutdown`

Rules:
- no normal editing while in this state
- recovery must re-enter through the failed boundary, not jump ahead

---

## 7. Shutdown

**Purpose:** clean process exit.

Can be entered from:
- `Navigation`
- `BlockingFailure`
- `MonthEditing`

The app must not exit mid-write or pretend sync finished when it did not.

---

## Canonical Paths

### Open existing month
`Bootstrap -> RepositoryGate -> Navigation -> OpeningMonth -> MonthEditing`

### Create new month
`Bootstrap -> RepositoryGate -> Navigation -> CreatingMonth -> GuidedCreation -> MonthEditing`

### Resume draft
`Bootstrap -> RepositoryGate -> Navigation -> OpeningMonth -> MonthEditing`

### Startup blocked
`Bootstrap -> RepositoryGate -> BlockingFailure`

### Mid-edit failure
`MonthEditing -> BlockingFailure`

---

## Transition Rules

1. Never enter `MonthEditing` without a loaded or created month model.
2. Never bypass `RepositoryGate` for repo-dependent actions.
3. Never continue silently after a blocking error.
4. Keep persistence state, sync state, and validation state separate.
5. Editing past months uses the same lifecycle as editing the current month.
6. Retry must return to the failed boundary, not skip checks.

---

## Recommended Internal State Shape

Use separate coordinated state objects for:

- `Route`
- `RepositoryState`
- `EditorState`
- `PersistenceState`
- `SyncState`
- `ValidationState`

That is clearer than a single giant enum of every possible combination.

---

## Minimum Transition Coverage

The implementation should make these transitions easy to test:

- launch -> repo ready
- repo ready -> month list
- month list -> create month
- month list -> open month
- guided creation -> editor
- dirty -> autosaved
- autosaved -> sync pending
- sync pending -> synced
- blocking failure -> retry -> safe state
