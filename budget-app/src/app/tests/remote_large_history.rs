use super::*;

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::remote_large_history::rename_month_git_daemon_remote_large_history -- --ignored --exact --nocapture`"]
fn rename_month_git_daemon_remote_large_history() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 10;

    let (temp, repo_root, remote, repository, navigation) =
        seeded_many_month_navigation_app_with_remote(MONTH_COUNT);
    git(&remote, &["config", "daemon.receivepack", "true"]);
    let (_daemon, port) = start_git_daemon(temp.path());
    let remote_url = format!("git://127.0.0.1:{port}/remote.git");
    set_remote_url(&repo_root, &remote_url);
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut feedback_samples = Vec::with_capacity(RUNS);
    let mut sync_samples = Vec::with_capacity(RUNS);
    let mut total_samples = Vec::with_capacity(RUNS);

    for index in 0..RUNS {
        stage_rename_dialogue_for_perf(&mut app, index);

        let total_started = Instant::now();
        feedback_samples.push(measure_pending_navigation_feedback_for_perf(
            &mut app,
            &mut terminal,
        ));
        sync_samples.push(measure_pending_navigation_sync_for_perf(
            &mut app,
            &mut terminal,
            true,
        ));
        total_samples.push(total_started.elapsed());
    }

    black_box(temp.path());
    print_samples(
        "rename_month_git_daemon_remote_large_history.feedback",
        &mut feedback_samples,
    );
    print_samples(
        "rename_month_git_daemon_remote_large_history.sync",
        &mut sync_samples,
    );
    print_samples(
        "rename_month_git_daemon_remote_large_history.total",
        &mut total_samples,
    );
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::remote_large_history::rename_push_failure_git_daemon_remote_large_history -- --ignored --exact --nocapture`"]
fn rename_push_failure_git_daemon_remote_large_history() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 10;

    let (temp, repo_root, remote, repository, navigation) =
        seeded_many_month_navigation_app_with_remote(MONTH_COUNT);
    git(&remote, &["config", "daemon.receivepack", "true"]);
    let (_daemon, port) = start_git_daemon(temp.path());
    let remote_url = format!("git://127.0.0.1:{port}/remote.git");
    set_remote_url(&repo_root, &remote_url);
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut feedback_samples = Vec::with_capacity(RUNS);
    let mut failure_samples = Vec::with_capacity(RUNS);
    let mut total_samples = Vec::with_capacity(RUNS);

    for index in 0..RUNS {
        stage_rename_dialogue_for_perf(&mut app, index);
        set_remote_url(&repo_root, "git://127.0.0.1:9/missing.git");

        let total_started = Instant::now();
        feedback_samples.push(measure_pending_navigation_feedback_for_perf(
            &mut app,
            &mut terminal,
        ));
        failure_samples.push(measure_pending_navigation_sync_for_perf(
            &mut app,
            &mut terminal,
            false,
        ));
        total_samples.push(total_started.elapsed());

        set_remote_url(&repo_root, &remote_url);
        app.handle_key(KeyEvent::from(KeyCode::Char('r'))).unwrap();
        assert!(matches!(app.route, Route::Navigation(_)));
    }

    black_box(temp.path());
    print_samples(
        "rename_push_failure_git_daemon_remote_large_history.feedback",
        &mut feedback_samples,
    );
    print_samples(
        "rename_push_failure_git_daemon_remote_large_history.failure",
        &mut failure_samples,
    );
    print_samples(
        "rename_push_failure_git_daemon_remote_large_history.total",
        &mut total_samples,
    );
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::remote_large_history::rename_push_retry_git_daemon_remote_large_history -- --ignored --exact --nocapture`"]
fn rename_push_retry_git_daemon_remote_large_history() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 10;

    let (temp, repo_root, remote, repository, navigation) =
        seeded_many_month_navigation_app_with_remote(MONTH_COUNT);
    git(&remote, &["config", "daemon.receivepack", "true"]);
    let (_daemon, port) = start_git_daemon(temp.path());
    let remote_url = format!("git://127.0.0.1:{port}/remote.git");
    set_remote_url(&repo_root, &remote_url);
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut samples = Vec::with_capacity(RUNS);

    for index in 0..RUNS {
        stage_rename_dialogue_for_perf(&mut app, index);
        set_remote_url(&repo_root, "git://127.0.0.1:9/missing.git");
        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
        assert!(matches!(
            app.route,
            Route::Navigation(NavigationState {
                sync: SyncState::SyncPending,
                ..
            })
        ));
        app.process_pending_sync().unwrap();
        assert!(matches!(app.route, Route::BlockingFailure(_)));
        set_remote_url(&repo_root, &remote_url);

        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Char('r'))).unwrap();
        draw_for_perf(&mut terminal, &app);
        samples.push(started.elapsed());
        assert!(matches!(app.route, Route::Navigation(_)));
        black_box(terminal.backend().buffer());
    }

    black_box(temp.path());
    print_samples(
        "rename_push_retry_git_daemon_remote_large_history",
        &mut samples,
    );
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::remote_large_history::delete_month_git_daemon_remote_large_history -- --ignored --exact --nocapture`"]
fn delete_month_git_daemon_remote_large_history() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 10;

    let (temp, repo_root, remote, repository, navigation) =
        seeded_many_month_navigation_app_with_remote(MONTH_COUNT);
    git(&remote, &["config", "daemon.receivepack", "true"]);
    let (_daemon, port) = start_git_daemon(temp.path());
    let remote_url = format!("git://127.0.0.1:{port}/remote.git");
    set_remote_url(&repo_root, &remote_url);
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut feedback_samples = Vec::with_capacity(RUNS);
    let mut sync_samples = Vec::with_capacity(RUNS);
    let mut total_samples = Vec::with_capacity(RUNS);

    for _ in 0..RUNS {
        stage_delete_dialogue_for_perf(&mut app);

        let total_started = Instant::now();
        feedback_samples.push(measure_pending_navigation_feedback_for_perf(
            &mut app,
            &mut terminal,
        ));
        sync_samples.push(measure_pending_navigation_sync_for_perf(
            &mut app,
            &mut terminal,
            true,
        ));
        total_samples.push(total_started.elapsed());
    }

    black_box(temp.path());
    print_samples(
        "delete_month_git_daemon_remote_large_history.feedback",
        &mut feedback_samples,
    );
    print_samples(
        "delete_month_git_daemon_remote_large_history.sync",
        &mut sync_samples,
    );
    print_samples(
        "delete_month_git_daemon_remote_large_history.total",
        &mut total_samples,
    );
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::remote_large_history::stale_rename_delete_git_daemon_remote_large_history -- --ignored --exact --nocapture`"]
fn stale_rename_delete_git_daemon_remote_large_history() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 10;

    let (temp, repo_root, remote, repository, navigation) =
        seeded_many_month_navigation_app_with_remote(MONTH_COUNT);
    git(&remote, &["config", "daemon.receivepack", "true"]);
    let (_daemon, port) = start_git_daemon(temp.path());
    let remote_url = format!("git://127.0.0.1:{port}/remote.git");
    set_remote_url(&repo_root, &remote_url);
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut rename_samples = Vec::with_capacity(RUNS);
    let mut rename_sync_samples = Vec::with_capacity(RUNS);
    let mut delete_samples = Vec::with_capacity(RUNS);
    let mut delete_sync_samples = Vec::with_capacity(RUNS);

    for index in 0..RUNS {
        make_navigation_stale_for_perf(&repo_root, index);
        stage_rename_dialogue_for_perf(&mut app, index);

        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
        draw_for_perf(&mut terminal, &app);
        rename_samples.push(started.elapsed());
        assert!(matches!(
            app.route,
            Route::Navigation(NavigationState {
                sync: SyncState::SyncPending,
                ..
            })
        ));
        black_box(terminal.backend().buffer());
        rename_sync_samples.push(measure_pending_navigation_sync_for_perf(
            &mut app,
            &mut terminal,
            true,
        ));
    }

    for index in 0..RUNS {
        make_navigation_stale_for_perf(&repo_root, RUNS + index);
        stage_delete_dialogue_for_perf(&mut app);

        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
        draw_for_perf(&mut terminal, &app);
        delete_samples.push(started.elapsed());
        assert!(matches!(
            app.route,
            Route::Navigation(NavigationState {
                sync: SyncState::SyncPending,
                ..
            })
        ));
        black_box(terminal.backend().buffer());
        delete_sync_samples.push(measure_pending_navigation_sync_for_perf(
            &mut app,
            &mut terminal,
            true,
        ));
    }

    black_box(temp.path());
    print_samples(
        "stale_rename_delete_git_daemon_remote_large_history.rename",
        &mut rename_samples,
    );
    print_samples(
        "stale_rename_delete_git_daemon_remote_large_history.rename_sync",
        &mut rename_sync_samples,
    );
    print_samples(
        "stale_rename_delete_git_daemon_remote_large_history.delete",
        &mut delete_samples,
    );
    print_samples(
        "stale_rename_delete_git_daemon_remote_large_history.delete_sync",
        &mut delete_sync_samples,
    );
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::remote_large_history::stale_rename_delete_local_large_history -- --ignored --exact --nocapture`"]
fn stale_rename_delete_local_large_history() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 20;

    let (temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(MONTH_COUNT);
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut rename_samples = Vec::with_capacity(RUNS);
    let mut delete_samples = Vec::with_capacity(RUNS);

    for index in 0..RUNS {
        make_navigation_stale_for_perf(&repo_root, index);
        stage_rename_dialogue_for_perf(&mut app, index);

        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
        draw_for_perf(&mut terminal, &app);
        rename_samples.push(started.elapsed());
        assert!(matches!(app.route, Route::Navigation(_)));
        black_box(terminal.backend().buffer());
    }

    for index in 0..RUNS {
        make_navigation_stale_for_perf(&repo_root, RUNS + index);
        stage_delete_dialogue_for_perf(&mut app);

        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
        draw_for_perf(&mut terminal, &app);
        delete_samples.push(started.elapsed());
        assert!(matches!(app.route, Route::Navigation(_)));
        black_box(terminal.backend().buffer());
    }

    black_box(temp.path());
    print_samples(
        "stale_rename_delete_local_large_history.rename",
        &mut rename_samples,
    );
    print_samples(
        "stale_rename_delete_local_large_history.delete",
        &mut delete_samples,
    );
}
