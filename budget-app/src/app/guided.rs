use anyhow::{Result, anyhow};
use budget_core::{CalculatedMonth, MonthDocument, MonthId, calculate_month};
use crossterm::event::{KeyCode, KeyEvent};

use super::App;
use super::document::{is_money_input_character, update_document_field};
use crate::state::{
    FailureState, FieldId, GuidedCreationState, NavigationState, PersistenceState, RetryTarget,
    Route, SyncState,
};

impl App {
    pub(super) fn handle_guided_key(
        &mut self,
        mut state: GuidedCreationState,
        key: KeyEvent,
    ) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => {
                self.route = Route::Shutdown;
                Ok(())
            }
            KeyCode::Esc => {
                self.reload_navigation(Some(state.document.month));
                Ok(())
            }
            KeyCode::Backspace => {
                state.input.backspace();
                self.route = Route::GuidedCreation(state);
                Ok(())
            }
            KeyCode::Enter => self.commit_guided_step(state),
            KeyCode::Char(character) if is_money_input_character(character) => {
                state.input.push(character);
                self.route = Route::GuidedCreation(state);
                Ok(())
            }
            _ => {
                self.route = Route::GuidedCreation(state);
                Ok(())
            }
        }
    }

    pub(super) fn start_guided_creation(&mut self, month: MonthId) -> Result<()> {
        let result = self.repository()?.create_month_draft(month);
        self.start_guided_creation_from_result(month, result)
    }

    pub(super) fn start_guided_creation_from_navigation(
        &mut self,
        navigation: NavigationState,
        month: MonthId,
    ) -> Result<()> {
        let previous = previous_calculated_for_month(&navigation, month);
        let result = self
            .repository()?
            .create_month_draft_with_cached_previous(month, previous);
        self.start_guided_creation_from_result(month, result)
    }

    fn start_guided_creation_from_result(
        &mut self,
        month: MonthId,
        result: Result<MonthDocument>,
    ) -> Result<()> {
        match result {
            Ok(document) => {
                let guided =
                    self.guided_state_from_document(document, Some("Draft created".to_owned()))?;
                self.save_initial_guided_state(guided)
            }
            Err(error) => {
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not create {month}"),
                    message: error.to_string(),
                    retry: RetryTarget::CreateMonth(month),
                });
                Ok(())
            }
        }
    }

    pub(super) fn commit_guided_step(&mut self, mut state: GuidedCreationState) -> Result<()> {
        let field = state.steps[state.step_index].clone();
        match update_document_field(&mut state.document, &field, &state.input) {
            Ok(false) => {
                state.message = None;
                self.advance_guided_state(state)
            }
            Ok(true) => match calculate_month(self.config()?, &state.document) {
                Ok(calculated) => {
                    // Recalculate before persisting so validation failures stay
                    // in the guided flow instead of reaching the repository.
                    Self::stage_document_persist(&mut state, calculated, "Autosaving draft");
                    self.persist_guided_state(state)
                }
                Err(error) => {
                    state.message = Some(error.to_string());
                    self.route = Route::GuidedCreation(state);
                    Ok(())
                }
            },
            Err(error) => {
                state.message = Some(error.to_string());
                self.route = Route::GuidedCreation(state);
                Ok(())
            }
        }
    }

    fn advance_guided_state(&mut self, mut state: GuidedCreationState) -> Result<()> {
        if state.step_index + 1 >= state.steps.len() {
            if !self.repository()?.sync_enabled() {
                return self.complete_local_guided_state(state);
            }
            let mut editor =
                self.editor_state_from_document(state.document, state.message.clone())?;
            editor.persistence = state.persistence;
            editor.sync = state.sync;
            self.route = Route::MonthEditing(editor);
        } else {
            state.step_index += 1;
            state.input = crate::state::MoneyInput::from_field(
                &state.steps[state.step_index],
                &state.document,
            );
            self.route = Route::GuidedCreation(state);
        }
        Ok(())
    }

    pub(super) fn persist_guided_state(&mut self, state: GuidedCreationState) -> Result<()> {
        if !self.repository()?.sync_enabled() {
            return match self.persist_local_guided_file(
                state,
                "Draft autosaved",
                |state| format!("Could not save {}", state.document.month),
                RetryTarget::GuidedSave,
            )? {
                Some(state) => self.advance_guided_state(state),
                None => Ok(()),
            };
        }

        match self.persist_remote_guided_state(
            state,
            "Draft autosaved",
            |state| format!("Could not save {}", state.document.month),
            RetryTarget::GuidedSave,
        )? {
            Some(state) => self.advance_guided_state(state),
            None => Ok(()),
        }
    }

    pub(super) fn save_initial_guided_state(&mut self, state: GuidedCreationState) -> Result<()> {
        if !self.repository()?.sync_enabled() {
            return match self.persist_local_guided_file(
                state,
                "Draft created",
                |state| format!("Could not create {}", state.document.month),
                RetryTarget::CreateDraft,
            )? {
                Some(state) => {
                    self.route = Route::GuidedCreation(state);
                    Ok(())
                }
                None => Ok(()),
            };
        }

        // Persist the draft immediately so an interrupted guided flow can be
        // resumed from the navigation screen on the next launch.
        match self.persist_remote_guided_state(
            state,
            "Draft created",
            |state| format!("Could not create {}", state.document.month),
            RetryTarget::CreateDraft,
        )? {
            Some(state) => {
                self.route = Route::GuidedCreation(state);
                Ok(())
            }
            None => Ok(()),
        }
    }

    fn persist_remote_guided_state<F, R>(
        &mut self,
        mut state: GuidedCreationState,
        success_message: &str,
        failure_title: F,
        retry_target: R,
    ) -> Result<Option<GuidedCreationState>>
    where
        F: FnOnce(&GuidedCreationState) -> String,
        R: FnOnce(GuidedCreationState) -> RetryTarget,
    {
        state.persistence = PersistenceState::Autosaving;
        state.sync = SyncState::SyncPending;
        let repository = self.repository()?;
        match repository.save_month_local_commit(&mut state.document) {
            Ok(()) => {
                state.calculated = calculate_month(self.config()?, &state.document)?;
                state.persistence = PersistenceState::Clean;
                state.sync = SyncState::SyncPending;
                state.message = Some(format!("{success_message}; sync pending"));
                Ok(Some(state))
            }
            Err(error) => {
                let title = failure_title(&state);
                state.persistence = PersistenceState::SaveFailed;
                state.sync = SyncState::SyncFailed;
                self.route = Route::BlockingFailure(FailureState {
                    title,
                    message: error.to_string(),
                    retry: retry_target(state),
                });
                Ok(None)
            }
        }
    }

    fn persist_local_guided_file<F, R>(
        &mut self,
        mut state: GuidedCreationState,
        success_message: &str,
        failure_title: F,
        retry_target: R,
    ) -> Result<Option<GuidedCreationState>>
    where
        F: FnOnce(&GuidedCreationState) -> String,
        R: FnOnce(GuidedCreationState) -> RetryTarget,
    {
        state.persistence = PersistenceState::Autosaving;
        state.sync = SyncState::Syncing;
        let repository = self.repository()?;
        match repository.save_month_file_only(&mut state.document) {
            Ok(()) => {
                state.calculated = calculate_month(self.config()?, &state.document)?;
                state.persistence = PersistenceState::Clean;
                state.sync = SyncState::Synced;
                state.message = Some(self.autosave_message(success_message)?);
                Ok(Some(state))
            }
            Err(error) => {
                let title = failure_title(&state);
                state.persistence = PersistenceState::SaveFailed;
                state.sync = SyncState::SyncFailed;
                self.route = Route::BlockingFailure(FailureState {
                    title,
                    message: error.to_string(),
                    retry: retry_target(state),
                });
                Ok(None)
            }
        }
    }

    fn complete_local_guided_state(&mut self, mut state: GuidedCreationState) -> Result<()> {
        state.persistence = PersistenceState::Autosaving;
        state.sync = SyncState::Syncing;
        let month = state.document.month;
        let message = format!("Update budget month {month}");
        let repository = self.repository()?;
        match repository.commit_month_file(month, &message) {
            Ok(()) => {
                state.persistence = PersistenceState::Clean;
                state.sync = SyncState::Synced;
                state.message = Some(self.autosave_message("Draft autosaved")?);
                let editor =
                    self.editor_state_from_document(state.document, state.message.clone())?;
                self.route = Route::MonthEditing(editor);
                Ok(())
            }
            Err(error) => {
                state.persistence = PersistenceState::SaveFailed;
                state.sync = SyncState::SyncFailed;
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not save {month}"),
                    message: error.to_string(),
                    retry: RetryTarget::GuidedSave(state),
                });
                Ok(())
            }
        }
    }

    pub(super) fn guided_state_from_document(
        &self,
        document: MonthDocument,
        message: Option<String>,
    ) -> Result<GuidedCreationState> {
        let calculated = calculate_month(self.config()?, &document)?;
        let steps = FieldId::guided_steps(self.config()?);
        // Guided creation always restarts from the first configured step; the
        // saved draft itself is the source of truth for progress.
        let input = steps
            .first()
            .ok_or_else(|| anyhow!("guided step list is empty"))?
            .clone();
        let initial_input = crate::state::MoneyInput::from_field(&input, &document);
        Ok(GuidedCreationState {
            document,
            calculated,
            steps,
            step_index: 0,
            input: initial_input,
            message,
            persistence: crate::state::PersistenceState::Clean,
            sync: crate::state::SyncState::Synced,
        })
    }
}

fn previous_calculated_for_month(
    navigation: &NavigationState,
    month: MonthId,
) -> Option<(MonthId, &CalculatedMonth, u64)> {
    let entry = navigation
        .months
        .iter()
        .filter(|entry| entry.document.month < month)
        .max_by_key(|entry| entry.document.month)?;
    let fingerprint = navigation
        .fingerprint
        .as_ref()?
        .iter()
        .find(|(month, _)| *month == entry.document.month)?
        .1;
    Some((entry.document.month, &entry.calculated, fingerprint))
}
