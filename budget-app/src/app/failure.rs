use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use super::App;
use crate::state::{FailureState, RetryTarget, Route};

impl App {
    pub(super) fn handle_failure_key(&mut self, state: FailureState, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.route = Route::Shutdown;
                Ok(())
            }
            KeyCode::Char('r') => self.retry_failure(state.retry),
            _ => {
                self.route = Route::BlockingFailure(state);
                Ok(())
            }
        }
    }

    pub(super) fn retry_failure(&mut self, retry: RetryTarget) -> Result<()> {
        // Retry targets capture the minimal operation needed to continue from a
        // blocking failure without reconstructing user state by hand.
        match retry {
            RetryTarget::RepositoryGate => {
                self.enter_repository_gate();
                Ok(())
            }
            RetryTarget::CreateMonth(month) => self.start_guided_creation(month),
            RetryTarget::CreateDraft(state) => self.save_initial_guided_state(state),
            RetryTarget::OpenMonth(month) => self.open_month(month),
            RetryTarget::GuidedSave(state) => self.persist_guided_state(state),
            RetryTarget::EditorSave(state) => self.persist_editor_state(state),
            RetryTarget::RenameMonth { source, target } => self.rename_month(source, target),
            RetryTarget::DeleteMonth(month) => self.delete_month(month),
            RetryTarget::PushNavigation(selected) => self.push_and_reload_navigation(selected),
        }
    }
}
