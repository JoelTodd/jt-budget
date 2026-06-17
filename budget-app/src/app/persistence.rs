use anyhow::Result;
use budget_core::{CalculatedMonth, MonthDocument, calculate_month};

use super::App;
use crate::state::{
    EditorState, FailureState, GuidedCreationState, NavigationState, PersistenceState, RetryTarget,
    Route, SyncState,
};

pub(super) trait PersistableRouteState: Sized {
    fn document(&self) -> &MonthDocument;
    fn document_mut(&mut self) -> &mut MonthDocument;
    fn set_calculated(&mut self, calculated: CalculatedMonth);
    fn set_message(&mut self, message: Option<String>);
    fn set_persistence(&mut self, persistence: PersistenceState);
    fn set_sync(&mut self, sync: SyncState);
}

impl PersistableRouteState for GuidedCreationState {
    fn document(&self) -> &MonthDocument {
        &self.document
    }

    fn document_mut(&mut self) -> &mut MonthDocument {
        &mut self.document
    }

    fn set_calculated(&mut self, calculated: CalculatedMonth) {
        self.calculated = calculated;
    }

    fn set_message(&mut self, message: Option<String>) {
        self.message = message;
    }

    fn set_persistence(&mut self, persistence: PersistenceState) {
        self.persistence = persistence;
    }

    fn set_sync(&mut self, sync: SyncState) {
        self.sync = sync;
    }
}

impl PersistableRouteState for EditorState {
    fn document(&self) -> &MonthDocument {
        &self.document
    }

    fn document_mut(&mut self) -> &mut MonthDocument {
        &mut self.document
    }

    fn set_calculated(&mut self, calculated: CalculatedMonth) {
        self.calculated = calculated;
    }

    fn set_message(&mut self, message: Option<String>) {
        self.message = message;
    }

    fn set_persistence(&mut self, persistence: PersistenceState) {
        self.persistence = persistence;
    }

    fn set_sync(&mut self, sync: SyncState) {
        self.sync = sync;
    }
}

impl App {
    pub(super) fn stage_document_persist<T: PersistableRouteState>(
        state: &mut T,
        calculated: CalculatedMonth,
        message: &str,
    ) {
        state.set_calculated(calculated);
        state.set_message(Some(message.to_owned()));
        state.set_persistence(PersistenceState::Dirty);
        state.set_sync(SyncState::SyncPending);
    }

    pub(super) fn persist_route_state<T, F, R>(
        &mut self,
        mut state: T,
        success_message: &str,
        failure_title: F,
        retry_target: R,
    ) -> Result<Option<T>>
    where
        T: PersistableRouteState,
        F: FnOnce(&T) -> String,
        R: FnOnce(T) -> RetryTarget,
    {
        state.set_persistence(PersistenceState::Autosaving);
        state.set_sync(SyncState::Syncing);
        let repository = self.repository()?;
        match repository.save_month(state.document_mut()) {
            Ok(()) => {
                let calculated = calculate_month(self.config()?, state.document())?;
                state.set_calculated(calculated);
                state.set_persistence(PersistenceState::Clean);
                state.set_sync(SyncState::Synced);
                state.set_message(Some(self.autosave_message(success_message)?));
                Ok(Some(state))
            }
            Err(error) => {
                let title = failure_title(&state);
                state.set_persistence(PersistenceState::SaveFailed);
                state.set_sync(SyncState::SyncFailed);
                self.route = Route::BlockingFailure(FailureState {
                    title,
                    message: error.to_string(),
                    retry: retry_target(state),
                });
                Ok(None)
            }
        }
    }

    pub(super) fn process_pending_sync(&mut self) -> Result<bool> {
        let should_sync = match &self.route {
            Route::Navigation(state) => {
                state.persistence == PersistenceState::Clean && state.sync == SyncState::SyncPending
            }
            Route::GuidedCreation(state) => {
                state.persistence == PersistenceState::Clean && state.sync == SyncState::SyncPending
            }
            Route::MonthEditing(state) => {
                state.persistence == PersistenceState::Clean && state.sync == SyncState::SyncPending
            }
            _ => false,
        };
        if !should_sync {
            return Ok(false);
        }

        match std::mem::replace(&mut self.route, Route::Shutdown) {
            Route::Navigation(state) => {
                self.push_navigation_state(state)?;
                Ok(true)
            }
            Route::GuidedCreation(state) => {
                self.push_guided_state(state)?;
                Ok(true)
            }
            Route::MonthEditing(state) => {
                self.push_editor_state(state)?;
                Ok(true)
            }
            route => {
                self.route = route;
                Ok(false)
            }
        }
    }

    pub(super) fn push_navigation_state(&mut self, mut state: NavigationState) -> Result<()> {
        state.sync = SyncState::Syncing;
        let selected = state.selected_month().map(|entry| entry.document.month);
        let repository = self.repository()?;
        match repository.retry_pending_push() {
            Ok(()) => {
                state.sync = SyncState::Synced;
                state.message = Some("Repository changes synced".to_owned());
                self.route = Route::Navigation(state);
            }
            Err(error) => {
                state.sync = SyncState::SyncFailed;
                self.route = Route::BlockingFailure(FailureState {
                    title: "Could not sync repository changes".to_owned(),
                    message: error.to_string(),
                    retry: RetryTarget::PushNavigation(selected),
                });
            }
        }
        Ok(())
    }

    pub(super) fn push_guided_state(&mut self, mut state: GuidedCreationState) -> Result<()> {
        state.set_sync(SyncState::Syncing);
        let repository = self.repository()?;
        match repository.retry_pending_push() {
            Ok(()) => {
                let message =
                    self.autosave_message(sync_success_message(&state.message, "Draft autosaved"))?;
                state.set_sync(SyncState::Synced);
                state.set_message(Some(message));
                self.route = Route::GuidedCreation(state);
            }
            Err(error) => {
                state.set_sync(SyncState::SyncFailed);
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not sync {}", state.document.month),
                    message: error.to_string(),
                    retry: RetryTarget::GuidedPush(state),
                });
            }
        }
        Ok(())
    }

    pub(super) fn push_editor_state(&mut self, mut state: EditorState) -> Result<()> {
        state.set_sync(SyncState::Syncing);
        let repository = self.repository()?;
        match repository.retry_pending_push() {
            Ok(()) => {
                let message =
                    self.autosave_message(sync_success_message(&state.message, "Month autosaved"))?;
                state.set_sync(SyncState::Synced);
                state.set_message(Some(message));
                self.route = Route::MonthEditing(state);
            }
            Err(error) => {
                state.set_sync(SyncState::SyncFailed);
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not sync {}", state.document.month),
                    message: error.to_string(),
                    retry: RetryTarget::EditorPush(state),
                });
            }
        }
        Ok(())
    }
}

fn sync_success_message<'a>(pending_message: &'a Option<String>, fallback: &'a str) -> &'a str {
    pending_message
        .as_deref()
        .and_then(|message| message.strip_suffix("; sync pending"))
        .unwrap_or(fallback)
}
