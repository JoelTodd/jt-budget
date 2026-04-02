use anyhow::{Result, anyhow};
use budget_core::{MonthDocument, MonthId, calculate_month};
use crossterm::event::{KeyCode, KeyEvent};

use super::App;
use super::document::{is_money_input_character, update_document_field};
use crate::state::{
    FailureState, FieldId, GuidedCreationState, PersistenceState, RetryTarget, Route, SyncState,
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
        let repository = self.repository()?;
        match repository.create_month_draft(month) {
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
            Ok(()) => match calculate_month(self.config()?, &state.document) {
                Ok(calculated) => {
                    // Recalculate before persisting so validation failures stay
                    // in the guided flow instead of reaching the repository.
                    state.calculated = calculated;
                    state.message = Some("Autosaving draft".to_owned());
                    state.persistence = PersistenceState::Dirty;
                    state.sync = SyncState::SyncPending;
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

    pub(super) fn persist_guided_state(&mut self, mut state: GuidedCreationState) -> Result<()> {
        state.persistence = PersistenceState::Autosaving;
        state.sync = SyncState::Syncing;
        let repository = self.repository()?;
        match repository.save_month(&mut state.document) {
            Ok(()) => {
                state.calculated = calculate_month(self.config()?, &state.document)?;
                state.persistence = PersistenceState::Clean;
                state.sync = SyncState::Synced;
                state.message = Some(self.autosave_message("Draft autosaved")?);
                if state.step_index + 1 >= state.steps.len() {
                    let editor =
                        self.editor_state_from_document(state.document, state.message.clone())?;
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
            Err(error) => {
                state.persistence = PersistenceState::SaveFailed;
                state.sync = SyncState::SyncFailed;
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not save {}", state.document.month),
                    message: error.to_string(),
                    retry: RetryTarget::GuidedSave(state),
                });
                Ok(())
            }
        }
    }

    pub(super) fn save_initial_guided_state(
        &mut self,
        mut state: GuidedCreationState,
    ) -> Result<()> {
        // Persist the draft immediately so an interrupted guided flow can be
        // resumed from the navigation screen on the next launch.
        state.persistence = PersistenceState::Autosaving;
        state.sync = SyncState::Syncing;
        let repository = self.repository()?;
        match repository.save_month(&mut state.document) {
            Ok(()) => {
                state.calculated = calculate_month(self.config()?, &state.document)?;
                state.persistence = PersistenceState::Clean;
                state.sync = SyncState::Synced;
                state.message = Some(self.autosave_message("Draft created")?);
                self.route = Route::GuidedCreation(state);
                Ok(())
            }
            Err(error) => {
                state.persistence = PersistenceState::SaveFailed;
                state.sync = SyncState::SyncFailed;
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not create {}", state.document.month),
                    message: error.to_string(),
                    retry: RetryTarget::CreateDraft(state),
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
            persistence: PersistenceState::Clean,
            sync: SyncState::Synced,
        })
    }
}
