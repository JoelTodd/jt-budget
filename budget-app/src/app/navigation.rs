use std::cmp::Reverse;
use std::collections::BTreeMap;

use anyhow::Result;
use budget_core::MonthId;
use crossterm::event::{KeyCode, KeyEvent};
use tracing::info;

use super::App;
use super::document::{is_month_id_character, validate_rename_target};
use crate::repository::{Repository, SyncOutcome};
use crate::state::{
    CreateDialogue, DeleteDialogue, FailureState, MonthEntry, NavigationDialogue, NavigationState,
    RenameDialogue, RetryTarget, Route, SyncState,
};

impl App {
    pub(super) fn handle_navigation_key(
        &mut self,
        mut state: NavigationState,
        key: KeyEvent,
    ) -> Result<()> {
        if let Some(dialogue) = state.dialogue.clone() {
            return self.handle_navigation_dialogue(state, dialogue, key);
        }

        match key.code {
            KeyCode::Char('q') => self.route = Route::Shutdown,
            KeyCode::Char('n') => {
                state.dialogue = Some(NavigationDialogue::Create(CreateDialogue {
                    input: String::new(),
                    error: None,
                }));
                self.route = Route::Navigation(state);
            }
            KeyCode::Char('m') => {
                if let Some(entry) = state.selected_month() {
                    state.dialogue = Some(NavigationDialogue::Rename(RenameDialogue {
                        source: entry.document.month,
                        input: entry.document.month.key(),
                        error: None,
                    }));
                }
                self.route = Route::Navigation(state);
            }
            KeyCode::Char('d') => {
                if let Some(entry) = state.selected_month() {
                    state.dialogue = Some(NavigationDialogue::Delete(DeleteDialogue {
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
                if let Some(month) = state.selected_month().map(|entry| entry.document.month) {
                    self.open_month_from_navigation(state, month)?;
                } else {
                    self.route = Route::Navigation(state);
                }
            }
            KeyCode::Char('r') => self.show_repository_gate(),
            _ => self.route = Route::Navigation(state),
        }
        Ok(())
    }

    pub(super) fn handle_navigation_dialogue(
        &mut self,
        mut state: NavigationState,
        dialogue: NavigationDialogue,
        key: KeyEvent,
    ) -> Result<()> {
        match dialogue {
            NavigationDialogue::Create(mut dialogue) => match key.code {
                KeyCode::Esc => state.dialogue = None,
                KeyCode::Enter => match MonthId::parse(dialogue.input.trim()) {
                    Ok(month) => return self.start_guided_creation_from_navigation(state, month),
                    Err(error) => {
                        dialogue.error = Some(error.to_string());
                        state.dialogue = Some(NavigationDialogue::Create(dialogue));
                    }
                },
                KeyCode::Backspace => {
                    dialogue.input.pop();
                    dialogue.error = None;
                    state.dialogue = Some(NavigationDialogue::Create(dialogue));
                }
                KeyCode::Char(character) if is_month_id_character(character) => {
                    dialogue.input.push(character);
                    dialogue.error = None;
                    state.dialogue = Some(NavigationDialogue::Create(dialogue));
                }
                _ => state.dialogue = Some(NavigationDialogue::Create(dialogue)),
            },
            NavigationDialogue::Rename(mut dialogue) => match key.code {
                KeyCode::Esc => state.dialogue = None,
                KeyCode::Enter => match validate_rename_target(dialogue.source, &dialogue.input) {
                    Ok(target) => {
                        return self.rename_month_from_navigation(state, dialogue.source, target);
                    }
                    Err(error) => {
                        dialogue.error = Some(error);
                        state.dialogue = Some(NavigationDialogue::Rename(dialogue));
                    }
                },
                KeyCode::Backspace => {
                    dialogue.input.pop();
                    dialogue.error = None;
                    state.dialogue = Some(NavigationDialogue::Rename(dialogue));
                }
                KeyCode::Char(character) if is_month_id_character(character) => {
                    dialogue.input.push(character);
                    dialogue.error = None;
                    state.dialogue = Some(NavigationDialogue::Rename(dialogue));
                }
                _ => state.dialogue = Some(NavigationDialogue::Rename(dialogue)),
            },
            NavigationDialogue::Delete(mut dialogue) => match key.code {
                KeyCode::Esc => state.dialogue = None,
                KeyCode::Enter => {
                    if dialogue.confirmation.trim() == dialogue.month.key() {
                        return self.delete_month_from_navigation(state, dialogue.month);
                    }
                    dialogue.error = Some(format!("Type {} to confirm deletion", dialogue.month));
                    state.dialogue = Some(NavigationDialogue::Delete(dialogue));
                }
                KeyCode::Backspace => {
                    dialogue.confirmation.pop();
                    dialogue.error = None;
                    state.dialogue = Some(NavigationDialogue::Delete(dialogue));
                }
                KeyCode::Char(character) if is_month_id_character(character) => {
                    dialogue.confirmation.push(character);
                    dialogue.error = None;
                    state.dialogue = Some(NavigationDialogue::Delete(dialogue));
                }
                _ => state.dialogue = Some(NavigationDialogue::Delete(dialogue)),
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
        let (loaded_months, fingerprint) = repo.list_months_with_fingerprint()?;
        let months = loaded_months
            .into_iter()
            .map(|loaded| MonthEntry {
                document: loaded.document,
                calculated: loaded.calculated,
            })
            .collect::<Vec<_>>();
        let mut navigation = NavigationState::with_fingerprint(months, fingerprint);
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
        self.open_month_with_return(month, None)
    }

    pub(super) fn open_month_from_navigation(
        &mut self,
        navigation: NavigationState,
        month: MonthId,
    ) -> Result<()> {
        self.open_month_with_return(month, Some(navigation))
    }

    fn open_month_with_return(
        &mut self,
        month: MonthId,
        return_navigation: Option<NavigationState>,
    ) -> Result<()> {
        let repository = self.repository()?;
        match repository.load_month_by_id(month) {
            Ok(loaded) => {
                let mut editor = self.editor_state_from_loaded(loaded);
                editor.return_navigation = return_navigation.map(Box::new);
                self.route = Route::MonthEditing(editor);
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
            Ok((SyncOutcome::Synced, _)) => {
                self.reload_navigation(Some(target));
                Ok(())
            }
            Ok((SyncOutcome::PushFailed(message), _)) => {
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

    fn rename_month_from_navigation(
        &mut self,
        navigation: NavigationState,
        source: MonthId,
        target: MonthId,
    ) -> Result<()> {
        let can_reuse_navigation = self.navigation_matches_disk(&navigation);
        let repository = self.repository()?;
        let sync_enabled = repository.sync_enabled();
        if can_reuse_navigation {
            match repository.rename_month_local_commit(source, target) {
                Ok(loaded) => {
                    if let Some(mut navigation) =
                        self.navigation_after_successful_rename(navigation, source, loaded)
                    {
                        if sync_enabled {
                            navigation.sync = SyncState::SyncPending;
                            navigation.message =
                                Some(format!("Renamed {source} to {target}; sync pending"));
                        } else {
                            navigation.message = Some(format!("Renamed {source} to {target}"));
                        }
                        self.route = Route::Navigation(navigation);
                        return Ok(());
                    }
                    self.reload_navigation(Some(target));
                    self.mark_navigation_local_mutation_message(
                        format!("Renamed {source} to {target}"),
                        format!("Renamed {source} to {target}; sync pending"),
                        sync_enabled,
                    );
                    return Ok(());
                }
                Err(error) => {
                    self.route = Route::BlockingFailure(FailureState {
                        title: format!("Could not rename {source}"),
                        message: error.to_string(),
                        retry: RetryTarget::RenameMonth { source, target },
                    });
                    return Ok(());
                }
            }
        }
        match repository.rename_month_local_commit(source, target) {
            Ok(_) => {
                if let Some(navigation) =
                    self.navigation_after_disk_reconciliation(navigation, Some(target))
                {
                    self.route = Route::Navigation(navigation);
                } else {
                    self.reload_navigation(Some(target));
                }
                self.mark_navigation_local_mutation_message(
                    format!("Renamed {source} to {target}"),
                    format!("Renamed {source} to {target}; sync pending"),
                    sync_enabled,
                );
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

    fn delete_month_from_navigation(
        &mut self,
        navigation: NavigationState,
        month: MonthId,
    ) -> Result<()> {
        let can_reuse_navigation = self.navigation_matches_disk(&navigation);
        let repository = self.repository()?;
        let sync_enabled = repository.sync_enabled();
        if can_reuse_navigation {
            match repository.delete_month_local_commit(month) {
                Ok(()) => {
                    if let Some(mut navigation) =
                        Self::navigation_after_successful_delete(navigation, month)
                    {
                        if sync_enabled {
                            navigation.sync = SyncState::SyncPending;
                            navigation.message = Some(format!("Deleted {month}; sync pending"));
                        } else {
                            navigation.message = Some(format!("Deleted {month}"));
                        }
                        self.route = Route::Navigation(navigation);
                        return Ok(());
                    }
                    self.reload_navigation(None);
                    self.mark_navigation_local_mutation_message(
                        format!("Deleted {month}"),
                        format!("Deleted {month}; sync pending"),
                        sync_enabled,
                    );
                    return Ok(());
                }
                Err(error) => {
                    self.route = Route::BlockingFailure(FailureState {
                        title: format!("Could not delete {month}"),
                        message: error.to_string(),
                        retry: RetryTarget::DeleteMonth(month),
                    });
                    return Ok(());
                }
            }
        }
        match repository.delete_month_local_commit(month) {
            Ok(()) => {
                if let Some(navigation) =
                    self.navigation_after_disk_reconciliation(navigation, None)
                {
                    self.route = Route::Navigation(navigation);
                } else {
                    self.reload_navigation(None);
                }
                self.mark_navigation_local_mutation_message(
                    format!("Deleted {month}"),
                    format!("Deleted {month}; sync pending"),
                    sync_enabled,
                );
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

    fn mark_navigation_local_mutation_message(
        &mut self,
        synced_message: String,
        pending_message: String,
        sync_enabled: bool,
    ) {
        if let Route::Navigation(navigation) = &mut self.route {
            if sync_enabled {
                navigation.sync = SyncState::SyncPending;
                navigation.message = Some(pending_message);
            } else {
                navigation.message = Some(synced_message);
            }
        }
    }

    pub(super) fn navigation_matches_disk(&self, navigation: &NavigationState) -> bool {
        navigation.fingerprint.as_ref().is_some_and(|expected| {
            self.repository()
                .and_then(|repository| repository.month_file_fingerprint())
                .is_ok_and(|current| current == *expected)
        })
    }

    fn navigation_after_disk_reconciliation(
        &self,
        mut navigation: NavigationState,
        selected: Option<MonthId>,
    ) -> Option<NavigationState> {
        let previous_fingerprint = navigation.fingerprint.as_ref()?;
        let current_fingerprint = self.repository().ok()?.month_file_fingerprint().ok()?;
        let previous_by_month = previous_fingerprint
            .iter()
            .copied()
            .collect::<BTreeMap<_, _>>();
        let mut cached_entries = navigation
            .months
            .into_iter()
            .map(|entry| (entry.document.month, entry))
            .collect::<BTreeMap<_, _>>();
        let repository = self.repository().ok()?;
        let mut months = Vec::with_capacity(current_fingerprint.len());

        for (month, current_entry_fingerprint) in &current_fingerprint {
            let cached_entry = previous_by_month
                .get(month)
                .filter(|previous_entry_fingerprint| {
                    *previous_entry_fingerprint == current_entry_fingerprint
                })
                .and_then(|_| cached_entries.remove(month));

            if let Some(entry) = cached_entry {
                months.push(entry);
                continue;
            }

            let loaded = repository.load_month_by_id(*month).ok()?;
            if loaded.document.month != *month {
                return None;
            }
            months.push(MonthEntry {
                document: loaded.document,
                calculated: loaded.calculated,
            });
        }

        months.sort_by_key(|entry| Reverse(entry.document.month));
        navigation.months = months;
        navigation.selected = selected
            .and_then(|selected_month| {
                navigation
                    .months
                    .iter()
                    .position(|entry| entry.document.month == selected_month)
            })
            .unwrap_or(0);
        navigation.dialogue = None;
        navigation.fingerprint = Some(current_fingerprint);
        Some(navigation)
    }

    fn navigation_after_successful_rename(
        &self,
        mut navigation: NavigationState,
        source: MonthId,
        loaded: crate::repository::LoadedMonth,
    ) -> Option<NavigationState> {
        let target = loaded.document.month;
        let index = navigation
            .months
            .iter()
            .position(|entry| entry.document.month == source)?;
        navigation.months[index] = MonthEntry {
            document: loaded.document,
            calculated: loaded.calculated,
        };
        navigation
            .months
            .sort_by_key(|entry| Reverse(entry.document.month));
        navigation.selected = navigation
            .months
            .iter()
            .position(|entry| entry.document.month == target)
            .unwrap_or(0);
        navigation.dialogue = None;

        if let Some(fingerprint) = navigation.fingerprint.as_mut() {
            let target_fingerprint = self
                .repository()
                .ok()?
                .month_file_fingerprint_for_month(target)
                .ok()?;
            fingerprint.retain(|(month, _)| *month != source);
            fingerprint.push(target_fingerprint);
            fingerprint.sort_by_key(|(month, _)| Reverse(*month));
        }

        Some(navigation)
    }

    fn navigation_after_successful_delete(
        mut navigation: NavigationState,
        month: MonthId,
    ) -> Option<NavigationState> {
        let index = navigation
            .months
            .iter()
            .position(|entry| entry.document.month == month)?;
        navigation.months.remove(index);
        navigation.selected = if navigation.months.is_empty() {
            0
        } else {
            index.min(navigation.months.len() - 1)
        };
        navigation.dialogue = None;

        if let Some(fingerprint) = navigation.fingerprint.as_mut() {
            fingerprint.retain(|(entry_month, _)| *entry_month != month);
        }

        Some(navigation)
    }
}
