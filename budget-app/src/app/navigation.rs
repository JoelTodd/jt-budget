use anyhow::Result;
use budget_core::MonthId;
use crossterm::event::{KeyCode, KeyEvent};
use tracing::info;

use super::App;
use super::document::{is_month_id_character, validate_rename_target};
use crate::repository::{Repository, SyncOutcome};
use crate::state::{
    CreateDialog, DeleteDialog, FailureState, MonthEntry, NavigationDialog, NavigationState,
    RenameDialog, RetryTarget, Route,
};

impl App {
    pub(super) fn handle_navigation_key(
        &mut self,
        mut state: NavigationState,
        key: KeyEvent,
    ) -> Result<()> {
        if let Some(dialog) = state.dialog.clone() {
            return self.handle_navigation_dialog(state, dialog, key);
        }

        match key.code {
            KeyCode::Char('q') => self.route = Route::Shutdown,
            KeyCode::Char('n') => {
                state.dialog = Some(NavigationDialog::Create(CreateDialog {
                    input: String::new(),
                    error: None,
                }));
                self.route = Route::Navigation(state);
            }
            KeyCode::Char('m') => {
                if let Some(entry) = state.selected_month() {
                    state.dialog = Some(NavigationDialog::Rename(RenameDialog {
                        source: entry.document.month,
                        input: entry.document.month.key(),
                        error: None,
                    }));
                }
                self.route = Route::Navigation(state);
            }
            KeyCode::Char('d') => {
                if let Some(entry) = state.selected_month() {
                    state.dialog = Some(NavigationDialog::Delete(DeleteDialog {
                        month: entry.document.month,
                        confirmation: String::new(),
                        error: None,
                    }));
                }
                self.route = Route::Navigation(state);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !state.months.is_empty() {
                    state.selected = (state.selected + 1).min(state.months.len().saturating_sub(1));
                }
                self.route = Route::Navigation(state);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !state.months.is_empty() {
                    state.selected = state.selected.saturating_sub(1);
                }
                self.route = Route::Navigation(state);
            }
            KeyCode::Enter => {
                if let Some(entry) = state.selected_month() {
                    self.open_month(entry.document.month)?;
                } else {
                    self.route = Route::Navigation(state);
                }
            }
            KeyCode::Char('r') => self.enter_repository_gate(),
            _ => self.route = Route::Navigation(state),
        }
        Ok(())
    }

    pub(super) fn handle_navigation_dialog(
        &mut self,
        mut state: NavigationState,
        dialog: NavigationDialog,
        key: KeyEvent,
    ) -> Result<()> {
        match dialog {
            NavigationDialog::Create(mut dialog) => match key.code {
                KeyCode::Esc => state.dialog = None,
                KeyCode::Enter => match MonthId::parse(dialog.input.trim()) {
                    Ok(month) => return self.start_guided_creation(month),
                    Err(error) => {
                        dialog.error = Some(error.to_string());
                        state.dialog = Some(NavigationDialog::Create(dialog));
                    }
                },
                KeyCode::Backspace => {
                    dialog.input.pop();
                    dialog.error = None;
                    state.dialog = Some(NavigationDialog::Create(dialog));
                }
                KeyCode::Char(character) if is_month_id_character(character) => {
                    dialog.input.push(character);
                    dialog.error = None;
                    state.dialog = Some(NavigationDialog::Create(dialog));
                }
                _ => state.dialog = Some(NavigationDialog::Create(dialog)),
            },
            NavigationDialog::Rename(mut dialog) => match key.code {
                KeyCode::Esc => state.dialog = None,
                KeyCode::Enter => match validate_rename_target(dialog.source, &dialog.input) {
                    Ok(target) => return self.rename_month(dialog.source, target),
                    Err(error) => {
                        dialog.error = Some(error);
                        state.dialog = Some(NavigationDialog::Rename(dialog));
                    }
                },
                KeyCode::Backspace => {
                    dialog.input.pop();
                    dialog.error = None;
                    state.dialog = Some(NavigationDialog::Rename(dialog));
                }
                KeyCode::Char(character) if is_month_id_character(character) => {
                    dialog.input.push(character);
                    dialog.error = None;
                    state.dialog = Some(NavigationDialog::Rename(dialog));
                }
                _ => state.dialog = Some(NavigationDialog::Rename(dialog)),
            },
            NavigationDialog::Delete(mut dialog) => match key.code {
                KeyCode::Esc => state.dialog = None,
                KeyCode::Enter => {
                    if dialog.confirmation.trim() == dialog.month.key() {
                        return self.delete_month(dialog.month);
                    }
                    dialog.error = Some(format!("Type {} to confirm deletion", dialog.month));
                    state.dialog = Some(NavigationDialog::Delete(dialog));
                }
                KeyCode::Backspace => {
                    dialog.confirmation.pop();
                    dialog.error = None;
                    state.dialog = Some(NavigationDialog::Delete(dialog));
                }
                KeyCode::Char(character) if is_month_id_character(character) => {
                    dialog.confirmation.push(character);
                    dialog.error = None;
                    state.dialog = Some(NavigationDialog::Delete(dialog));
                }
                _ => state.dialog = Some(NavigationDialog::Delete(dialog)),
            },
        }

        self.route = Route::Navigation(state);
        Ok(())
    }

    pub(super) fn enter_repository_gate(&mut self) {
        match Repository::open(&self.repo_root)
            .and_then(|repo| self.navigation_state_from_repository(repo, None))
        {
            Ok((repo, navigation)) => {
                info!("repository ready");
                self.repository = Some(repo);
                self.route = Route::Navigation(navigation);
            }
            Err(error) => {
                self.route = Route::BlockingFailure(FailureState {
                    title: "Repository blocked".to_owned(),
                    message: error.to_string(),
                    retry: RetryTarget::RepositoryGate,
                });
            }
        }
    }

    pub(super) fn navigation_state_from_repository(
        &self,
        repo: Repository,
        selected: Option<MonthId>,
    ) -> Result<(Repository, NavigationState)> {
        let months = repo
            .list_months()?
            .into_iter()
            .map(|loaded| MonthEntry {
                document: loaded.document,
                calculated: loaded.calculated,
            })
            .collect::<Vec<_>>();
        let mut navigation = NavigationState::new(months);
        if let Some(selected_month) = selected {
            if let Some(index) = navigation
                .months
                .iter()
                .position(|entry| entry.document.month == selected_month)
            {
                navigation.selected = index;
            }
        }
        Ok((repo, navigation))
    }

    pub(super) fn reload_navigation(&mut self, selected: Option<MonthId>) {
        let Some(repo) = self.repository.take() else {
            self.enter_repository_gate();
            return;
        };

        // Rebuild navigation from disk so the list reflects the repository
        // exactly after creates, renames, deletes, or retry-only pushes.
        match self.navigation_state_from_repository(repo, selected) {
            Ok((repo, navigation)) => {
                self.repository = Some(repo);
                self.route = Route::Navigation(navigation);
            }
            Err(error) => {
                self.route = Route::BlockingFailure(FailureState {
                    title: "Repository blocked".to_owned(),
                    message: error.to_string(),
                    retry: RetryTarget::RepositoryGate,
                });
            }
        }
    }

    pub(super) fn open_month(&mut self, month: MonthId) -> Result<()> {
        let repository = self.repository()?;
        match repository.load_month_by_id(month) {
            Ok(loaded) => {
                self.route = Route::MonthEditing(self.editor_state_from_loaded(loaded));
            }
            Err(error) => {
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not open {month}"),
                    message: error.to_string(),
                    retry: RetryTarget::OpenMonth(month),
                });
            }
        }
        Ok(())
    }

    pub(super) fn push_and_reload_navigation(&mut self, selected: Option<MonthId>) -> Result<()> {
        let repository = self.repository()?;
        match repository.retry_pending_push() {
            Ok(()) => {
                // Rename and delete may already be committed locally when a
                // push fails, so the retry path only needs to push and refresh.
                self.reload_navigation(selected);
            }
            Err(error) => {
                self.route = Route::BlockingFailure(FailureState {
                    title: "Could not sync repository changes".to_owned(),
                    message: error.to_string(),
                    retry: RetryTarget::PushNavigation(selected),
                });
            }
        }
        Ok(())
    }

    pub(super) fn rename_month(&mut self, source: MonthId, target: MonthId) -> Result<()> {
        let repository = self.repository()?;
        match repository.rename_month(source, target) {
            Ok(SyncOutcome::Synced) => {
                self.reload_navigation(Some(target));
                Ok(())
            }
            Ok(SyncOutcome::PushFailed(message)) => {
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not sync rename {source} -> {target}"),
                    message: format!(
                        "Renamed {source} to {target} locally and committed it, but push failed: {message}"
                    ),
                    retry: RetryTarget::PushNavigation(Some(target)),
                });
                Ok(())
            }
            Err(error) => {
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not rename {source}"),
                    message: error.to_string(),
                    retry: RetryTarget::RenameMonth { source, target },
                });
                Ok(())
            }
        }
    }

    pub(super) fn delete_month(&mut self, month: MonthId) -> Result<()> {
        let repository = self.repository()?;
        match repository.delete_month(month) {
            Ok(SyncOutcome::Synced) => {
                self.reload_navigation(None);
                Ok(())
            }
            Ok(SyncOutcome::PushFailed(message)) => {
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not sync deletion of {month}"),
                    message: format!(
                        "Deleted {month} locally and committed it, but push failed: {message}"
                    ),
                    retry: RetryTarget::PushNavigation(None),
                });
                Ok(())
            }
            Err(error) => {
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not delete {month}"),
                    message: error.to_string(),
                    retry: RetryTarget::DeleteMonth(month),
                });
                Ok(())
            }
        }
    }
}
