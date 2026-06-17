use super::*;

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::large_history_first_draw::create_dialogue_submit_large_history_first_draw_local -- --ignored --exact --nocapture`"]
fn create_dialogue_submit_large_history_first_draw_local() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 20;

    let (temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(MONTH_COUNT);
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut feedback_samples = Vec::with_capacity(RUNS);

    for index in 0..RUNS {
        stage_create_dialogue_for_perf(&mut app, index);
        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
        draw_for_perf(&mut terminal, &app);
        feedback_samples.push(started.elapsed());
        assert!(matches!(app.route, Route::GuidedCreation(_)));
        black_box(terminal.backend().buffer());
        app.reload_navigation(None);
    }

    black_box(temp.path());
    print_samples(
        "create_dialogue_submit_large_history_first_draw_local.feedback",
        &mut feedback_samples,
    );
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::large_history_first_draw::create_dialogue_submit_first_draw_git_daemon_remote -- --ignored --exact --nocapture`"]
fn create_dialogue_submit_first_draw_git_daemon_remote() {
    const RUNS: usize = 10;

    let temp = tempfile::tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);
    git(&remote, &["config", "daemon.receivepack", "true"]);
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, Some(remote.to_str().unwrap())).unwrap();
    let (_daemon, port) = start_git_daemon(temp.path());
    let remote_url = format!("git://127.0.0.1:{port}/remote.git");
    git(&repo_root, &["remote", "set-url", "origin", &remote_url]);
    let repository = Repository::open(&repo_root).unwrap();
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState::new(Vec::new())),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut feedback_samples = Vec::with_capacity(RUNS);
    let mut sync_samples = Vec::with_capacity(RUNS);

    for index in 0..RUNS {
        stage_create_dialogue_for_perf(&mut app, index);
        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
        draw_for_perf(&mut terminal, &app);
        feedback_samples.push(started.elapsed());
        match &app.route {
            Route::GuidedCreation(state) => assert_eq!(state.sync, SyncState::SyncPending),
            other => panic!("expected pending guided creation route, got {other:?}"),
        }
        black_box(terminal.backend().buffer());

        let started = Instant::now();
        assert!(app.process_pending_sync().unwrap());
        draw_for_perf(&mut terminal, &app);
        sync_samples.push(started.elapsed());
        match &app.route {
            Route::GuidedCreation(state) => assert_eq!(state.sync, SyncState::Synced),
            other => panic!("expected synced guided creation route, got {other:?}"),
        }
        black_box(terminal.backend().buffer());
        app.reload_navigation(None);
    }

    print_samples(
        "create_dialogue_submit_first_draw_git_daemon_remote.feedback",
        &mut feedback_samples,
    );
    print_samples(
        "create_dialogue_submit_first_draw_git_daemon_remote.sync",
        &mut sync_samples,
    );
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::large_history_first_draw::first_navigation_screen_large_history_app_breakdown -- --ignored --exact --nocapture`"]
fn first_navigation_screen_large_history_app_breakdown() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 20;

    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, None).unwrap();
    populate_month_files(&repo_root, MONTH_COUNT);
    git(&repo_root, &["add", "-A", "--", "months"]);
    git(
        &repo_root,
        &["commit", "--quiet", "-m", "Seed large month history"],
    );

    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut open_samples = Vec::with_capacity(RUNS);
    let mut navigation_samples = Vec::with_capacity(RUNS);
    let mut draw_samples = Vec::with_capacity(RUNS);
    let mut total_samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let total_started = Instant::now();
        let mut app = App::bootstrap(repo_root.clone());

        let started = Instant::now();
        let repository = Repository::open(&repo_root).unwrap();
        open_samples.push(started.elapsed());

        let started = Instant::now();
        let (repository, navigation) = app
            .navigation_state_from_repository(repository, None)
            .unwrap();
        navigation_samples.push(started.elapsed());

        app.repository = Some(repository);
        app.route = Route::Navigation(navigation);

        let started = Instant::now();
        draw_for_perf(&mut terminal, &app);
        draw_samples.push(started.elapsed());

        total_samples.push(total_started.elapsed());
        black_box(terminal.backend().buffer());
    }

    black_box(temp.path());
    print_samples(
        "first_navigation_screen_large_history_app_breakdown.open",
        &mut open_samples,
    );
    print_samples(
        "first_navigation_screen_large_history_app_breakdown.navigation",
        &mut navigation_samples,
    );
    print_samples(
        "first_navigation_screen_large_history_app_breakdown.draw",
        &mut draw_samples,
    );
    print_samples(
        "first_navigation_screen_large_history_app_breakdown.total",
        &mut total_samples,
    );
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::large_history_first_draw::repository_gate_first_draw_large_history_app -- --ignored --exact --nocapture`"]
fn repository_gate_first_draw_large_history_app() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 100;

    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, None).unwrap();
    populate_month_files(&repo_root, MONTH_COUNT);
    git(&repo_root, &["add", "-A", "--", "months"]);
    git(
        &repo_root,
        &["commit", "--quiet", "-m", "Seed large month history"],
    );

    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let started = Instant::now();
        let app = App::bootstrap(repo_root.clone());
        draw_for_perf(&mut terminal, &app);
        samples.push(started.elapsed());
        black_box(terminal.backend().buffer());
    }

    black_box(temp.path());
    print_samples("repository_gate_first_draw_large_history_app", &mut samples);
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::large_history_first_draw::repository_refresh_large_history_app -- --ignored --exact --nocapture`"]
fn repository_refresh_large_history_app() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 20;

    let (temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(MONTH_COUNT);
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut feedback_samples = Vec::with_capacity(RUNS);
    let mut completion_samples = Vec::with_capacity(RUNS);
    let mut total_samples = Vec::with_capacity(RUNS);

    for _ in 0..RUNS {
        let total_started = Instant::now();

        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Char('r'))).unwrap();
        draw_for_perf(&mut terminal, &app);
        feedback_samples.push(started.elapsed());
        assert!(matches!(app.route, Route::RepositoryGate(_)));
        black_box(terminal.backend().buffer());

        let started = Instant::now();
        assert!(app.process_repository_gate());
        draw_for_perf(&mut terminal, &app);
        completion_samples.push(started.elapsed());
        assert!(matches!(app.route, Route::Navigation(_)));
        black_box(terminal.backend().buffer());

        total_samples.push(total_started.elapsed());
    }

    black_box(temp.path());
    print_samples(
        "repository_refresh_large_history_app.feedback",
        &mut feedback_samples,
    );
    print_samples(
        "repository_refresh_large_history_app.completion",
        &mut completion_samples,
    );
    print_samples(
        "repository_refresh_large_history_app.total",
        &mut total_samples,
    );
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::large_history_first_draw::repository_refresh_git_daemon_remote_large_history -- --ignored --exact --nocapture`"]
fn repository_refresh_git_daemon_remote_large_history() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 12;

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
    let mut completion_samples = Vec::with_capacity(RUNS);
    let mut total_samples = Vec::with_capacity(RUNS);

    for _ in 0..RUNS {
        let total_started = Instant::now();

        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Char('r'))).unwrap();
        draw_for_perf(&mut terminal, &app);
        feedback_samples.push(started.elapsed());
        assert!(matches!(app.route, Route::RepositoryGate(_)));
        black_box(terminal.backend().buffer());

        let started = Instant::now();
        assert!(app.process_repository_gate());
        draw_for_perf(&mut terminal, &app);
        completion_samples.push(started.elapsed());
        assert!(matches!(app.route, Route::Navigation(_)));
        black_box(terminal.backend().buffer());

        total_samples.push(total_started.elapsed());
    }

    black_box(temp.path());
    print_samples(
        "repository_refresh_git_daemon_remote_large_history.feedback",
        &mut feedback_samples,
    );
    print_samples(
        "repository_refresh_git_daemon_remote_large_history.completion",
        &mut completion_samples,
    );
    print_samples(
        "repository_refresh_git_daemon_remote_large_history.total",
        &mut total_samples,
    );
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::large_history_first_draw::open_selected_month_large_history -- --ignored --exact --nocapture`"]
fn open_selected_month_large_history() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 100;

    let (temp, repo_root, repository, mut navigation) =
        seeded_many_month_navigation_app(MONTH_COUNT);
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(navigation.clone()),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut samples = Vec::with_capacity(RUNS);

    for index in 0..RUNS {
        navigation.selected = (index * 17) % MONTH_COUNT;
        app.route = Route::Navigation(navigation.clone());

        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
        terminal
            .draw(|frame| {
                crate::ui::render(
                    frame,
                    &app.route,
                    Path::new("/tmp/budget"),
                    app.repository.as_ref().map(Repository::config),
                )
            })
            .unwrap();
        samples.push(started.elapsed());
        black_box(terminal.backend().buffer());
    }

    black_box(temp.path());
    print_samples("open_selected_month_large_history", &mut samples);
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::large_history_first_draw::editor_exit_large_history -- --ignored --exact --nocapture`"]
fn editor_exit_large_history() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 100;

    let (temp, repo_root, repository, mut navigation) =
        seeded_many_month_navigation_app(MONTH_COUNT);
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(navigation.clone()),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut samples = Vec::with_capacity(RUNS);

    for index in 0..RUNS {
        navigation.selected = (index * 17) % MONTH_COUNT;
        app.route = Route::Navigation(navigation.clone());
        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
        assert!(matches!(app.route, Route::MonthEditing(_)));

        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Esc)).unwrap();
        draw_for_perf(&mut terminal, &app);
        samples.push(started.elapsed());
        assert!(matches!(app.route, Route::Navigation(_)));
        black_box(terminal.backend().buffer());
    }

    black_box(temp.path());
    print_samples("editor_exit_large_history", &mut samples);
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::large_history_first_draw::editor_exit_after_autosave_large_history -- --ignored --exact --nocapture`"]
fn editor_exit_after_autosave_large_history() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 30;

    let (temp, repo_root, repository, mut navigation) =
        seeded_many_month_navigation_app(MONTH_COUNT);
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(navigation.clone()),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut samples = Vec::with_capacity(RUNS);

    for index in 0..RUNS {
        navigation.selected = (index * 17) % MONTH_COUNT;
        app.route = Route::Navigation(navigation.clone());
        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
        autosave_first_editor_field_for_perf(&mut app, index);

        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Esc)).unwrap();
        draw_for_perf(&mut terminal, &app);
        samples.push(started.elapsed());
        assert!(matches!(app.route, Route::Navigation(_)));
        black_box(terminal.backend().buffer());

        navigation = match &app.route {
            Route::Navigation(state) => state.clone(),
            other => panic!("expected navigation route, got {other:?}"),
        };
    }

    black_box(temp.path());
    print_samples("editor_exit_after_autosave_large_history", &mut samples);
}
