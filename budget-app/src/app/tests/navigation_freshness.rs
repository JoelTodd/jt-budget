use super::*;

#[test]
fn rename_push_failure_retries_the_pending_push_boundary() {
    let (_temp, repo_root, remote, repository, navigation, source) =
        seeded_navigation_app_with_remote("2026-03");
    git(
        &repo_root,
        &[
            "config",
            "remote.origin.url",
            "/definitely/missing/repo.git",
        ],
    );

    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Rename(RenameDialogue {
                source,
                input: "2026-04".to_owned(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.sync, SyncState::SyncPending);
            assert_eq!(
                state.message.as_deref(),
                Some("Renamed 2026-03 to 2026-04; sync pending")
            );
        }
        other => panic!("expected pending navigation route, got {other:?}"),
    }

    app.process_pending_sync().unwrap();

    match &app.route {
        Route::BlockingFailure(FailureState { title, retry, .. }) => {
            assert_eq!(title, "Could not sync repository changes");
            assert!(matches!(
                retry,
                RetryTarget::PushNavigation(Some(month))
                    if *month == MonthId::parse("2026-04").unwrap()
            ));
        }
        other => panic!("expected blocking failure, got {other:?}"),
    }

    assert!(!repo_root.join("months/2026-03.toml").exists());
    assert!(repo_root.join("months/2026-04.toml").exists());

    git(
        &repo_root,
        &["config", "remote.origin.url", remote.to_str().unwrap()],
    );
    app.handle_key(KeyEvent::from(KeyCode::Char('r'))).unwrap();

    match app.route {
        Route::Navigation(state) => {
            assert!(state.dialogue.is_none());
            assert_eq!(
                state.months[0].document.month,
                MonthId::parse("2026-04").unwrap()
            );
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
}

#[test]
fn rename_from_fresh_navigation_updates_cached_entry_and_fingerprint() {
    let (_temp, repo_root, repository, navigation, source) = seeded_navigation_app("2026-03");
    let target = MonthId::parse("2026-04").unwrap();
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Rename(RenameDialogue {
                source,
                input: target.key(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::Navigation(state) => {
            assert!(state.dialogue.is_none());
            assert_eq!(state.months[0].document.month, target);
            assert_eq!(state.months[0].calculated.month, target);
            assert_eq!(
                state.fingerprint.as_ref().unwrap(),
                &app.repository
                    .as_ref()
                    .unwrap()
                    .month_file_fingerprint()
                    .unwrap()
            );
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
}

#[test]
fn rename_from_stale_navigation_reloads_unrelated_month_changes() {
    let (_temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(2);
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    let source = MonthId::parse("2020-02").unwrap();
    let target = MonthId::parse("2026-04").unwrap();
    fs::write(
        repo_root.join("months/2020-01.toml"),
        month_document_for_perf("2020-01", 99),
    )
    .unwrap();
    app.route = match std::mem::replace(&mut app.route, Route::Shutdown) {
        Route::Navigation(state) => Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Rename(RenameDialogue {
                source,
                input: target.key(),
                error: None,
            })),
            ..state
        }),
        other => panic!("expected navigation route, got {other:?}"),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::Navigation(state) => {
            let changed = state
                .months
                .iter()
                .find(|entry| entry.document.month == MonthId::parse("2020-01").unwrap())
                .unwrap();
            assert_eq!(
                changed.document.accounts["current_account"],
                200_000 + 99 * 750
            );
            assert_eq!(
                state.fingerprint.as_ref().unwrap(),
                &app.repository
                    .as_ref()
                    .unwrap()
                    .month_file_fingerprint()
                    .unwrap()
            );
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
}

#[test]
fn remote_rename_from_stale_navigation_reloads_then_syncs_pending_push() {
    let (_temp, repo_root, _remote, repository, navigation) =
        seeded_many_month_navigation_app_with_remote(2);
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    let source = MonthId::parse("2020-02").unwrap();
    let target = MonthId::parse("2026-04").unwrap();
    fs::write(
        repo_root.join("months/2020-01.toml"),
        month_document_for_perf("2020-01", 99),
    )
    .unwrap();
    app.route = match std::mem::replace(&mut app.route, Route::Shutdown) {
        Route::Navigation(state) => Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Rename(RenameDialogue {
                source,
                input: target.key(),
                error: None,
            })),
            ..state
        }),
        other => panic!("expected navigation route, got {other:?}"),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.sync, SyncState::SyncPending);
            assert_eq!(
                state.message.as_deref(),
                Some("Renamed 2020-02 to 2026-04; sync pending")
            );
            let changed = state
                .months
                .iter()
                .find(|entry| entry.document.month == MonthId::parse("2020-01").unwrap())
                .unwrap();
            assert_eq!(
                changed.document.accounts["current_account"],
                200_000 + 99 * 750
            );
            assert_eq!(
                state.fingerprint.as_ref().unwrap(),
                &app.repository
                    .as_ref()
                    .unwrap()
                    .month_file_fingerprint()
                    .unwrap()
            );
        }
        other => panic!("expected pending navigation route, got {other:?}"),
    }

    assert!(app.process_pending_sync().unwrap());
    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.sync, SyncState::Synced);
            assert_eq!(state.message.as_deref(), Some("Repository changes synced"));
        }
        other => panic!("expected synced navigation route, got {other:?}"),
    }
}

#[test]
fn delete_from_fresh_navigation_updates_cached_list_and_fingerprint() {
    let (_temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(2);
    let deleted = MonthId::parse("2020-02").unwrap();
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Delete(DeleteDialogue {
                month: deleted,
                confirmation: deleted.key(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::Navigation(state) => {
            assert!(state.dialogue.is_none());
            assert_eq!(state.months.len(), 1);
            assert!(
                state
                    .months
                    .iter()
                    .all(|entry| entry.document.month != deleted)
            );
            assert_eq!(
                state.fingerprint.as_ref().unwrap(),
                &app.repository
                    .as_ref()
                    .unwrap()
                    .month_file_fingerprint()
                    .unwrap()
            );
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
}

#[test]
fn delete_from_stale_navigation_reloads_unrelated_month_changes() {
    let (_temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(2);
    let deleted = MonthId::parse("2020-02").unwrap();
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Delete(DeleteDialogue {
                month: deleted,
                confirmation: deleted.key(),
                error: None,
            })),
            ..navigation
        }),
    };
    fs::write(
        repo_root.join("months/2020-01.toml"),
        month_document_for_perf("2020-01", 99),
    )
    .unwrap();

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::Navigation(state) => {
            let changed = state
                .months
                .iter()
                .find(|entry| entry.document.month == MonthId::parse("2020-01").unwrap())
                .unwrap();
            assert_eq!(
                changed.document.accounts["current_account"],
                200_000 + 99 * 750
            );
            assert_eq!(
                state.fingerprint.as_ref().unwrap(),
                &app.repository
                    .as_ref()
                    .unwrap()
                    .month_file_fingerprint()
                    .unwrap()
            );
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
}
