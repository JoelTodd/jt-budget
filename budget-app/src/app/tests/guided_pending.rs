use super::*;

#[test]
fn remote_guided_creation_blocks_failed_pending_push_and_retries_push_only() {
    let (_temp, repo_root, remote, mut app, before) = remote_guided_app();

    set_remote_url(&repo_root, "/definitely/missing/repo.git");
    app.start_guided_creation(MonthId::parse("2026-03").unwrap())
        .unwrap();

    let draft_commit = git_capture(&repo_root, &["rev-parse", "HEAD"]);
    assert_commit_count(&repo_root, before + 1);
    assert_guided_route(
        &app,
        0,
        SyncState::SyncPending,
        "Draft created; sync pending",
    );
    assert_guided_push_failure(&mut app, &repo_root, before + 1);
    retry_guided_push(
        &mut app,
        &repo_root,
        &remote,
        &draft_commit,
        "Draft created and synced",
    );

    set_remote_url(&repo_root, "/definitely/missing/repo.git");
    commit_changed_guided_step(&mut app);

    let step_commit = git_capture(&repo_root, &["rev-parse", "HEAD"]);
    assert_commit_count(&repo_root, before + 2);
    assert_guided_route(
        &app,
        1,
        SyncState::SyncPending,
        "Draft autosaved; sync pending",
    );
    assert_guided_push_failure(&mut app, &repo_root, before + 2);
    retry_guided_push(
        &mut app,
        &repo_root,
        &remote,
        &step_commit,
        "Draft autosaved and synced",
    );
}

fn remote_guided_app() -> (TempDir, PathBuf, PathBuf, App, usize) {
    let temp = tempfile::tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, Some(remote.to_str().unwrap())).unwrap();
    let repository = Repository::open(&repo_root).unwrap();
    let before = git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
        .parse::<usize>()
        .unwrap();
    let app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Shutdown,
    };

    (temp, repo_root, remote, app, before)
}

fn assert_commit_count(repo_root: &Path, expected: usize) {
    assert_eq!(
        git_capture(repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        expected
    );
}

fn assert_guided_route(app: &App, step_index: usize, sync: SyncState, message: &str) {
    match &app.route {
        Route::GuidedCreation(state) => {
            assert_eq!(state.step_index, step_index);
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, sync);
            assert_eq!(state.message.as_deref(), Some(message));
        }
        other => panic!("expected guided route, got {other:?}"),
    }
}

fn assert_guided_push_failure(app: &mut App, repo_root: &Path, expected_commits: usize) {
    app.process_pending_sync().unwrap();

    match &app.route {
        Route::BlockingFailure(FailureState { title, retry, .. }) => {
            assert_eq!(title, "Could not sync 2026-03");
            assert!(matches!(retry, RetryTarget::GuidedPush(_)));
        }
        other => panic!("expected blocking failure, got {other:?}"),
    }
    assert_commit_count(repo_root, expected_commits);
}

fn retry_guided_push(
    app: &mut App,
    repo_root: &Path,
    remote: &Path,
    expected_commit: &str,
    expected_message: &str,
) {
    set_remote_url(repo_root, remote.to_str().unwrap());
    let retry = match std::mem::replace(&mut app.route, Route::Shutdown) {
        Route::BlockingFailure(state) => state.retry,
        other => panic!("expected blocking failure, got {other:?}"),
    };
    app.retry_failure(retry).unwrap();

    let step_index = match &app.route {
        Route::GuidedCreation(state) => state.step_index,
        other => panic!("expected guided route, got {other:?}"),
    };
    assert_guided_route(app, step_index, SyncState::Synced, expected_message);
    assert!(
        git_capture(repo_root, &["ls-remote", "origin", "refs/heads/main"])
            .contains(expected_commit)
    );
}

fn commit_changed_guided_step(app: &mut App) {
    let state = match &app.route {
        Route::GuidedCreation(state) => {
            let mut state = state.clone();
            state.input = crate::state::MoneyInput::from_value(
                budget_core::Money::from_minor(guided_perf_value(state.step_index)),
                state.steps[state.step_index].allows_negative(),
            );
            state
        }
        other => panic!("expected guided route, got {other:?}"),
    };
    app.commit_guided_step(state).unwrap();
}
