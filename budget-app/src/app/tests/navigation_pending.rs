use super::*;

#[test]
fn remote_navigation_rename_blocks_failed_pending_push_and_retries_push_only() {
    let (_temp, repo_root, remote, repository, navigation, source) =
        seeded_navigation_app_with_remote("2026-03");
    let target = MonthId::parse("2026-04").unwrap();
    let before = git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
        .parse::<usize>()
        .unwrap();
    set_remote_url(&repo_root, "/definitely/missing/repo.git");
    let mut app = App {
        repo_root: repo_root.clone(),
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

    let local_commit = git_capture(&repo_root, &["rev-parse", "HEAD"]);
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );
    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::SyncPending);
            assert_eq!(
                state.message.as_deref(),
                Some("Renamed 2026-03 to 2026-04; sync pending")
            );
            assert_eq!(state.months[0].document.month, target);
        }
        other => panic!("expected pending navigation route, got {other:?}"),
    }

    assert!(app.process_pending_sync().unwrap());

    match &app.route {
        Route::BlockingFailure(FailureState { title, retry, .. }) => {
            assert_eq!(title, "Could not sync repository changes");
            assert!(matches!(
                retry,
                RetryTarget::PushNavigation(Some(month)) if *month == target
            ));
        }
        other => panic!("expected blocking failure, got {other:?}"),
    }
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );

    set_remote_url(&repo_root, remote.to_str().unwrap());
    let retry = match std::mem::replace(&mut app.route, Route::Shutdown) {
        Route::BlockingFailure(state) => state.retry,
        other => panic!("expected blocking failure, got {other:?}"),
    };
    app.retry_failure(retry).unwrap();

    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.sync, SyncState::Synced);
            assert_eq!(state.months[0].document.month, target);
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
    assert!(
        git_capture(&repo_root, &["ls-remote", "origin", "refs/heads/main"])
            .contains(&local_commit)
    );
}

#[test]
fn remote_navigation_delete_blocks_failed_pending_push_and_retries_push_only() {
    let (_temp, repo_root, remote, repository, navigation) =
        seeded_many_month_navigation_app_with_remote(2);
    let deleted = MonthId::parse("2020-02").unwrap();
    let before = git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
        .parse::<usize>()
        .unwrap();
    set_remote_url(&repo_root, "/definitely/missing/repo.git");
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

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    let local_commit = git_capture(&repo_root, &["rev-parse", "HEAD"]);
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );
    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::SyncPending);
            assert_eq!(
                state.message.as_deref(),
                Some("Deleted 2020-02; sync pending")
            );
            assert!(
                state
                    .months
                    .iter()
                    .all(|entry| entry.document.month != deleted)
            );
        }
        other => panic!("expected pending navigation route, got {other:?}"),
    }

    assert!(app.process_pending_sync().unwrap());

    match &app.route {
        Route::BlockingFailure(FailureState { title, retry, .. }) => {
            assert_eq!(title, "Could not sync repository changes");
            assert!(matches!(retry, RetryTarget::PushNavigation(Some(_))));
        }
        other => panic!("expected blocking failure, got {other:?}"),
    }
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );

    set_remote_url(&repo_root, remote.to_str().unwrap());
    let retry = match std::mem::replace(&mut app.route, Route::Shutdown) {
        Route::BlockingFailure(state) => state.retry,
        other => panic!("expected blocking failure, got {other:?}"),
    };
    app.retry_failure(retry).unwrap();

    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.sync, SyncState::Synced);
            assert!(
                state
                    .months
                    .iter()
                    .all(|entry| entry.document.month != deleted)
            );
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
    assert!(
        git_capture(&repo_root, &["ls-remote", "origin", "refs/heads/main"])
            .contains(&local_commit)
    );
}
