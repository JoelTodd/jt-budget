use anyhow::Result;
use budget_core::{CalculatedMonth, MonthDocument, calculate_month};

use super::App;
use crate::state::{
    EditorState, FailureState, GuidedCreationState, PersistenceState, RetryTarget, Route, SyncState,
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
}
