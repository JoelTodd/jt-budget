use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use super::App;
use super::document::is_money_input_character;
use crate::state::{DefaultsState, FailureState, MoneyInput, RetryTarget, Route};

impl App {
    pub(super) fn handle_defaults_key(
        &mut self,
        mut state: DefaultsState,
        key: KeyEvent,
    ) -> Result<()> {
        if let Some(buffer) = &mut state.edit_buffer {
            match key.code {
                KeyCode::Esc => {
                    state.edit_buffer = None;
                    state.message = None;
                }
                KeyCode::Backspace => buffer.backspace(),
                KeyCode::Char(character) if is_money_input_character(character) => {
                    buffer.push(character);
                    state.message = None;
                }
                KeyCode::Enter => match buffer.commit_value() {
                    Ok(value) => {
                        let field = state.fields[state.focus_index];
                        if field.value(&state.config) != value {
                            field.set_value(&mut state.config, value);
                            state.edit_buffer = None;
                            return self.persist_defaults(state);
                        }
                        state.edit_buffer = None;
                        state.message = None;
                    }
                    Err(error) => state.message = Some(error.to_string()),
                },
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Esc => {
                    self.route = Route::Navigation(*state.return_navigation);
                    return Ok(());
                }
                KeyCode::Char('q') => {
                    self.route = Route::Shutdown;
                    return Ok(());
                }
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                    state.focus_index =
                        (state.focus_index + 1).min(state.fields.len().saturating_sub(1));
                }
                KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                    state.focus_index = state.focus_index.saturating_sub(1);
                }
                KeyCode::Enter | KeyCode::Char('e') => {
                    if let Some(field) = state.fields.get(state.focus_index) {
                        state.edit_buffer =
                            Some(MoneyInput::from_value(field.value(&state.config), false));
                        state.message = None;
                    }
                }
                _ => {}
            }
        }
        self.route = Route::DefaultsEditing(state);
        Ok(())
    }

    pub(super) fn persist_defaults(&mut self, mut state: DefaultsState) -> Result<()> {
        let repository = self.repository.as_mut().expect("config available");
        match repository.save_monthly_defaults(&state.config) {
            Ok(()) => {
                state.message = Some(if repository.sync_enabled() {
                    "Defaults saved and synced".to_owned()
                } else {
                    "Defaults saved locally (no remote configured)".to_owned()
                });
                self.route = Route::DefaultsEditing(state);
            }
            Err(error) => {
                self.route = Route::BlockingFailure(FailureState {
                    title: "Could not save defaults".to_owned(),
                    message: format!("{error:#}"),
                    retry: RetryTarget::DefaultsSave(state),
                });
            }
        }
        Ok(())
    }
}
