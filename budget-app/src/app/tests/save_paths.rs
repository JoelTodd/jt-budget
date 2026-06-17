use super::*;

#[test]
fn unchanged_editor_commit_does_not_autosave_or_commit() {
    let (_temp, repo_root, repository, _navigation, month) = seeded_navigation_app("2026-03");
    let loaded = repository.load_month_by_id(month).unwrap();
    let before = git_capture(&repo_root, &["rev-list", "--count", "HEAD"]);
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Shutdown,
    };
    app.route = Route::MonthEditing(app.editor_state_from_loaded(loaded));

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    let after = git_capture(&repo_root, &["rev-list", "--count", "HEAD"]);
    assert_eq!(after, before);
    match app.route {
        Route::MonthEditing(state) => {
            assert_eq!(state.interaction, InteractionState::SheetIdle);
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::Synced);
            assert_eq!(state.message, None);
        }
        other => panic!("expected editor route, got {other:?}"),
    }
}

#[test]
fn unchanged_guided_step_advances_without_autosave_commit() {
    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, None).unwrap();
    let repository = Repository::open(&repo_root).unwrap();
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Shutdown,
    };
    app.start_guided_creation(MonthId::parse("2026-03").unwrap())
        .unwrap();
    let before = git_capture(&repo_root, &["rev-list", "--count", "HEAD"]);

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    let after = git_capture(&repo_root, &["rev-list", "--count", "HEAD"]);
    assert_eq!(after, before);
    match app.route {
        Route::GuidedCreation(state) => {
            assert_eq!(state.step_index, 1);
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::Synced);
            assert_eq!(state.message, None);
        }
        other => panic!("expected guided route, got {other:?}"),
    }
}

#[test]
fn local_guided_creation_commits_once_on_completion() {
    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, None).unwrap();
    let repository = Repository::open(&repo_root).unwrap();
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Shutdown,
    };
    let initial_commits = git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
        .parse::<usize>()
        .unwrap();

    app.start_guided_creation(MonthId::parse("2026-03").unwrap())
        .unwrap();

    assert!(repo_root.join("months/2026-03.toml").is_file());
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        initial_commits
    );

    loop {
        let state = match &app.route {
            Route::GuidedCreation(state) => {
                let mut state = state.clone();
                let field = state.steps[state.step_index].clone();
                state.input = crate::state::MoneyInput::from_value(
                    budget_core::Money::from_minor(guided_perf_value(state.step_index)),
                    field.allows_negative(),
                );
                state
            }
            Route::MonthEditing(_) => break,
            other => panic!("expected guided or editor route, got {other:?}"),
        };
        app.commit_guided_step(state).unwrap();
    }

    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        initial_commits + 1
    );
    assert_eq!(git_capture(&repo_root, &["status", "--short"]), "");
}

#[test]
fn remote_editor_autosave_blocks_failed_pending_push_and_retries_push_only() {
    let (_temp, repo_root, remote, repository, _navigation, month) =
        seeded_navigation_app_with_remote("2026-03");
    let loaded = repository.load_month_by_id(month).unwrap();
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::MonthEditing(app_state_from_loaded_for_perf(loaded)),
    };
    let before = git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
        .parse::<usize>()
        .unwrap();
    set_remote_url(&repo_root, "/definitely/missing/repo.git");

    autosave_first_editor_field_for_perf(&mut app, 0);

    let local_commit = git_capture(&repo_root, &["rev-parse", "HEAD"]);
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );
    match &app.route {
        Route::MonthEditing(state) => {
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::SyncPending);
            assert_eq!(
                state.message.as_deref(),
                Some("Month autosaved; sync pending")
            );
        }
        other => panic!("expected pending editor route, got {other:?}"),
    }

    app.process_pending_sync().unwrap();

    match &app.route {
        Route::BlockingFailure(FailureState { title, retry, .. }) => {
            assert_eq!(title, "Could not sync 2026-03");
            assert!(matches!(retry, RetryTarget::EditorPush(_)));
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
        Route::MonthEditing(state) => {
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::Synced);
        }
        other => panic!("expected editor route, got {other:?}"),
    }
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );
    assert!(
        git_capture(&repo_root, &["ls-remote", "origin", "refs/heads/main"])
            .contains(&local_commit)
    );
}
