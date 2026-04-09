//! UI-facing state types for the TUI state machine.
//!
//! These structures are intentionally plain data so the runtime and renderer
//! can exchange route state without hidden side effects.

mod field_catalog;
mod money_input;
mod route_state;

pub use field_catalog::{FieldId, SectionId};
pub use money_input::MoneyInput;
pub use route_state::{
    CreateDialogue, DeleteDialogue, EditorState, FailureState, GuidedCreationState,
    InteractionState, MonthEntry, NavigationDialogue, NavigationState, PersistenceState,
    RenameDialogue, RetryTarget, Route, SyncState,
};
