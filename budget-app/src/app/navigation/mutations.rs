use super::*;

impl App {
    pub(in crate::app) fn push_and_reload_navigation(
        &mut self,
        selected: Option<MonthId>,
    ) -> Result<()> {
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

    pub(in crate::app) fn rename_month(&mut self, source: MonthId, target: MonthId) -> Result<()> {
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

    pub(super) fn rename_month_from_navigation(
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

    pub(in crate::app) fn delete_month(&mut self, month: MonthId) -> Result<()> {
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

    pub(super) fn delete_month_from_navigation(
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
