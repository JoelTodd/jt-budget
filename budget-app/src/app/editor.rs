use anyhow::Result;
use budget_core::{CalculatedMonth, MonthDocument, calculate_month};
use crossterm::event::{KeyCode, KeyEvent};

use super::App;
use super::document::{is_money_input_character, update_document_field};
use crate::repository::LoadedMonth;
use crate::state::{
    EditorState, FieldId, InteractionState, MonthEntry, PersistenceState, RetryTarget, Route,
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
                    KeyCode::Esc => self.return_to_navigation(state)?,
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

    fn return_to_navigation(&mut self, state: EditorState) -> Result<()> {
        let selected = state.document.month;
        if let Some(cached) = state.return_navigation {
            if let Some(expected) = cached.fingerprint.as_ref()
                && self
                    .repository()
                    .and_then(|repository| repository.month_file_fingerprint())
                    .is_ok_and(|current| current == *expected)
            {
                self.route = Route::Navigation(*cached);
                return Ok(());
            }
        }

        self.reload_navigation(Some(selected));
        Ok(())
    }

    pub(super) fn commit_editor_field(&mut self, mut state: EditorState) -> Result<()> {
        let field = state.fields[state.focus_index].clone();
        let Some(buffer) = state.edit_buffer.clone() else {
            self.route = Route::MonthEditing(state);
            return Ok(());
        };

        match update_document_field(&mut state.document, &field, &buffer) {
            Ok(false) => {
                state.edit_buffer = None;
                state.interaction = InteractionState::SheetIdle;
                state.message = None;
                self.route = Route::MonthEditing(state);
                Ok(())
            }
            Ok(true) => match calculate_month(self.config()?, &state.document) {
                Ok(calculated) => {
                    // Only persist documents that still satisfy the domain
                    // rules after the field edit has been applied.
                    Self::stage_document_persist(&mut state, calculated, "Autosaving changes");
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
        if self.repository()?.sync_enabled() {
            state.persistence = PersistenceState::Autosaving;
            state.sync = SyncState::SyncPending;
            let repository = self.repository()?;
            match repository.save_month_local_commit(&mut state.document) {
                Ok(()) => {
                    let calculated = calculate_month(self.config()?, &state.document)?;
                    state.calculated = calculated;
                    state.persistence = PersistenceState::Clean;
                    state.sync = SyncState::SyncPending;
                    state.message = Some("Month autosaved; sync pending".to_owned());
                    self.update_return_navigation_after_editor_save(&mut state);
                    self.route = Route::MonthEditing(state);
                }
                Err(error) => {
                    state.persistence = PersistenceState::SaveFailed;
                    state.sync = SyncState::SyncFailed;
                    self.route = Route::BlockingFailure(crate::state::FailureState {
                        title: format!("Could not save {}", state.document.month),
                        message: error.to_string(),
                        retry: RetryTarget::EditorSave(state),
                    });
                }
            }
            return Ok(());
        }

        match self.persist_route_state(
            state,
            "Month autosaved",
            |state| format!("Could not save {}", state.document.month),
            RetryTarget::EditorSave,
        )? {
            Some(mut state) => {
                self.update_return_navigation_after_editor_save(&mut state);
                self.route = Route::MonthEditing(state);
                Ok(())
            }
            None => Ok(()),
        }
    }

    fn update_return_navigation_after_editor_save(&self, state: &mut EditorState) {
        let Some(navigation) = state.return_navigation.as_mut() else {
            return;
        };
        let month = state.document.month;
        let Some(index) = navigation
            .months
            .iter()
            .position(|entry| entry.document.month == month)
        else {
            state.return_navigation = None;
            return;
        };

        navigation.months[index] = MonthEntry {
            document: state.document.clone(),
            calculated: state.calculated.clone(),
        };
        navigation.selected = index;

        let Some(fingerprint) = navigation.fingerprint.as_mut() else {
            return;
        };
        let Ok(current) = self
            .repository()
            .and_then(|repository| repository.month_file_fingerprint_for_month(month))
        else {
            state.return_navigation = None;
            return;
        };
        if let Some(existing) = fingerprint
            .iter_mut()
            .find(|(fingerprint_month, _)| *fingerprint_month == month)
        {
            *existing = current;
        } else {
            state.return_navigation = None;
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
            return_navigation: None,
        }
    }
}
