use budget_core::{CalculatedMonth, MonthDocument, MonthId};

use super::field_catalog::FieldId;
use super::money_input::MoneyInput;

/// Top-level application route.
#[derive(Clone, Debug)]
pub enum Route {
    Navigation(NavigationState),
    GuidedCreation(GuidedCreationState),
    MonthEditing(EditorState),
    BlockingFailure(FailureState),
    Shutdown,
}

/// State for the month list and its modal dialogues.
#[derive(Clone, Debug)]
pub struct NavigationState {
    pub months: Vec<MonthEntry>,
    pub selected: usize,
    pub dialogue: Option<NavigationDialogue>,
}

impl NavigationState {
    pub fn new(months: Vec<MonthEntry>) -> Self {
        Self {
            months,
            selected: 0,
            dialogue: None,
        }
    }

    pub fn selected_month(&self) -> Option<&MonthEntry> {
        self.months.get(self.selected)
    }
}

/// Navigation entry containing both editable and derived month data.
#[derive(Clone, Debug)]
pub struct MonthEntry {
    pub document: MonthDocument,
    pub calculated: CalculatedMonth,
}

/// Dialogue state for creating a new month.
#[derive(Clone, Debug)]
pub struct CreateDialogue {
    pub input: String,
    pub error: Option<String>,
}

/// Dialogue state for renaming an existing month.
#[derive(Clone, Debug)]
pub struct RenameDialogue {
    pub source: MonthId,
    pub input: String,
    pub error: Option<String>,
}

/// Dialogue state for deleting a month after explicit confirmation.
#[derive(Clone, Debug)]
pub struct DeleteDialogue {
    pub month: MonthId,
    pub confirmation: String,
    pub error: Option<String>,
}

/// Any modal dialogue that can appear from the navigation route.
#[derive(Clone, Debug)]
pub enum NavigationDialogue {
    Create(CreateDialogue),
    Rename(RenameDialogue),
    Delete(DeleteDialogue),
}

/// State for the guided month-creation workflow.
#[derive(Clone, Debug)]
pub struct GuidedCreationState {
    pub document: MonthDocument,
    pub calculated: CalculatedMonth,
    pub steps: Vec<FieldId>,
    pub step_index: usize,
    pub input: MoneyInput,
    pub message: Option<String>,
    pub persistence: PersistenceState,
    pub sync: SyncState,
}

/// State for the full monthly editor.
#[derive(Clone, Debug)]
pub struct EditorState {
    pub document: MonthDocument,
    pub calculated: CalculatedMonth,
    pub fields: Vec<FieldId>,
    pub focus_index: usize,
    pub edit_buffer: Option<MoneyInput>,
    pub message: Option<String>,
    pub interaction: InteractionState,
    pub persistence: PersistenceState,
    pub sync: SyncState,
}

/// Route state for failures that must block user progress until retried or quit.
#[derive(Clone, Debug)]
pub struct FailureState {
    pub title: String,
    pub message: String,
    pub retry: RetryTarget,
}

/// Operation that should be retried from the blocking failure screen.
#[derive(Clone, Debug)]
pub enum RetryTarget {
    RepositoryGate,
    CreateMonth(MonthId),
    CreateDraft(GuidedCreationState),
    GuidedSave(GuidedCreationState),
    EditorSave(EditorState),
    OpenMonth(MonthId),
    RenameMonth { source: MonthId, target: MonthId },
    DeleteMonth(MonthId),
    PushNavigation(Option<MonthId>),
}

/// Whether the monthly sheet is navigating fields or editing one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionState {
    SheetIdle,
    FieldEditing,
}

/// Local persistence state for the current editor or guided draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistenceState {
    Clean,
    Dirty,
    Autosaving,
    SaveFailed,
}

/// Remote synchronisation state for the current editor or guided draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncState {
    SyncPending,
    Syncing,
    Synced,
    SyncFailed,
}
