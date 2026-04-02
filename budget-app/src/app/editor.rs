use anyhow::Result;
use budget_core::{CalculatedMonth, MonthDocument, calculate_month};
use crossterm::event::{KeyCode, KeyEvent};

use super::App;
use super::document::{is_money_input_character, update_document_field};
use crate::repository::LoadedMonth;
use crate::state::{
    EditorState, FailureState, FieldId, InteractionState, PersistenceState, RetryTarget, Route,
    SyncState,
};

impl App {
    pub(super) fn handle_editor_key(
        &mut self,
        mut state: EditorState,
        key: KeyEvent,
    ) -> Result<()> {
        match state.interaction {
            InteractionState::SheetIdle => {
                match key.code {
                    KeyCode::Char('q') => self.route = Route::Shutdown,
                    KeyCode::Esc => self.reload_navigation(Some(state.document.month)),
                    KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                        state.focus_index =
                            (state.focus_index + 1).min(state.fields.len().saturating_sub(1));
                        self.route = Route::MonthEditing(state);
                    }
                    KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                        state.focus_index = state.focus_index.saturating_sub(1);
                        self.route = Route::MonthEditing(state);
                    }
                    KeyCode::Enter | KeyCode::Char('e') => {
                        let field = state.fields[state.focus_index].clone();
                        state.edit_buffer = Some(crate::state::MoneyInput::from_field(
                            &field,
                            &state.document,
                        ));
                        state.interaction = InteractionState::FieldEditing;
                        self.route = Route::MonthEditing(state);
                    }
                    _ => self.route = Route::MonthEditing(state),
                }
                Ok(())
            }
            InteractionState::FieldEditing => match key.code {
                KeyCode::Esc => {
                    state.edit_buffer = None;
                    state.interaction = InteractionState::SheetIdle;
                    self.route = Route::MonthEditing(state);
                    Ok(())
                }
                KeyCode::Backspace => {
                    if let Some(buffer) = &mut state.edit_buffer {
                        buffer.backspace();
                    }
                    self.route = Route::MonthEditing(state);
                    Ok(())
                }
                KeyCode::Enter => self.commit_editor_field(state),
                KeyCode::Char(character) if is_money_input_character(character) => {
                    if let Some(buffer) = &mut state.edit_buffer {
                        buffer.push(character);
                    }
                    self.route = Route::MonthEditing(state);
                    Ok(())
                }
                _ => {
                    self.route = Route::MonthEditing(state);
                    Ok(())
                }
            },
        }
    }

    pub(super) fn commit_editor_field(&mut self, mut state: EditorState) -> Result<()> {
        let field = state.fields[state.focus_index].clone();
        let Some(buffer) = state.edit_buffer.clone() else {
            self.route = Route::MonthEditing(state);
            return Ok(());
        };

        match update_document_field(&mut state.document, &field, &buffer) {
            Ok(()) => match calculate_month(self.config()?, &state.document) {
                Ok(calculated) => {
                    // Only persist documents that still satisfy the domain
                    // rules after the field edit has been applied.
                    state.calculated = calculated;
                    state.message = Some("Autosaving changes".to_owned());
                    state.persistence = PersistenceState::Dirty;
                    state.sync = SyncState::SyncPending;
                    state.edit_buffer = None;
                    state.interaction = InteractionState::SheetIdle;
                    self.persist_editor_state(state)
                }
                Err(error) => {
                    state.message = Some(error.to_string());
                    self.route = Route::MonthEditing(state);
                    Ok(())
                }
            },
            Err(error) => {
                state.message = Some(error.to_string());
                self.route = Route::MonthEditing(state);
                Ok(())
            }
        }
    }

    pub(super) fn persist_editor_state(&mut self, mut state: EditorState) -> Result<()> {
        state.persistence = PersistenceState::Autosaving;
        state.sync = SyncState::Syncing;
        let repository = self.repository()?;
        match repository.save_month(&mut state.document) {
            Ok(()) => {
                state.calculated = calculate_month(self.config()?, &state.document)?;
                state.persistence = PersistenceState::Clean;
                state.sync = SyncState::Synced;
                state.message = Some(self.autosave_message("Month autosaved")?);
                self.route = Route::MonthEditing(state);
                Ok(())
            }
            Err(error) => {
                state.persistence = PersistenceState::SaveFailed;
                state.sync = SyncState::SyncFailed;
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not save {}", state.document.month),
                    message: error.to_string(),
                    retry: RetryTarget::EditorSave(state),
                });
                Ok(())
            }
        }
    }

    pub(super) fn editor_state_from_loaded(&self, loaded: LoadedMonth) -> EditorState {
        self.editor_state_from_calculated(loaded.document, loaded.calculated, None)
    }

    pub(super) fn editor_state_from_document(
        &self,
        document: MonthDocument,
        message: Option<String>,
    ) -> Result<EditorState> {
        let calculated = calculate_month(self.config()?, &document)?;
        Ok(self.editor_state_from_calculated(document, calculated, message))
    }

    fn editor_state_from_calculated(
        &self,
        document: MonthDocument,
        calculated: CalculatedMonth,
        message: Option<String>,
    ) -> EditorState {
        EditorState {
            document,
            calculated,
            // Keep focus order aligned with the rendered sheet so keyboard
            // traversal feels consistent across layout variants.
            fields: FieldId::editor_fields(self.config().expect("config available")),
            focus_index: 0,
            edit_buffer: None,
            message,
            interaction: InteractionState::SheetIdle,
            persistence: PersistenceState::Clean,
            sync: SyncState::Synced,
        }
    }
}
