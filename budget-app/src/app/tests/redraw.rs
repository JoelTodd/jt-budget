use super::*;

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::redraw::editor_commit_changed_local_autosave -- --ignored --exact --nocapture`"]
fn editor_commit_changed_local_autosave() {
    const RUNS: usize = 30;

    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, None).unwrap();
    let repository = Repository::open(&repo_root).unwrap();
    let month = MonthId::parse("2026-03").unwrap();
    let mut document = repository.create_month_draft(month).unwrap();
    repository.save_month(&mut document).unwrap();
    let loaded = repository.load_month_by_id(month).unwrap();
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::MonthEditing(app_state_from_loaded_for_perf(loaded)),
    };

    measure_editor_commit_changed_autosave("editor_commit_changed_local_autosave", &mut app, RUNS);
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::redraw::editor_commit_changed_git_daemon_remote_autosave -- --ignored --exact --nocapture`"]
fn editor_commit_changed_git_daemon_remote_autosave() {
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
    let month = MonthId::parse("2026-03").unwrap();
    let mut document = repository.create_month_draft(month).unwrap();
    repository.save_month(&mut document).unwrap();
    let loaded = repository.load_month_by_id(month).unwrap();
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::MonthEditing(app_state_from_loaded_for_perf(loaded)),
    };

    measure_editor_commit_changed_autosave_breakdown(
        "editor_commit_changed_git_daemon_remote_autosave",
        &mut app,
        RUNS,
    );
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::redraw::editor_navigation_keypress_redraw -- --ignored --exact --nocapture`"]
fn editor_navigation_keypress_redraw() {
    const RUNS: usize = 1_000;

    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, None).unwrap();
    let repository = Repository::open(&repo_root).unwrap();
    let month = MonthId::parse("2026-03").unwrap();
    let mut document = repository.create_month_draft(month).unwrap();
    repository.save_month(&mut document).unwrap();
    let loaded = repository.load_month_by_id(month).unwrap();
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::MonthEditing(app_state_from_loaded_for_perf(loaded)),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();

    let mut forward = true;
    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let (focus_index, field_count) = match &app.route {
            Route::MonthEditing(state) => (state.focus_index, state.fields.len()),
            other => panic!("expected editor route, got {other:?}"),
        };
        if focus_index + 1 >= field_count {
            forward = false;
        } else if focus_index == 0 {
            forward = true;
        }
        let key = if forward {
            KeyEvent::from(KeyCode::Tab)
        } else {
            KeyEvent::from(KeyCode::BackTab)
        };

        let started = Instant::now();
        app.handle_key(key).unwrap();
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

    print_samples("editor_navigation_keypress_redraw", &mut samples);
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::redraw::editor_typing_keypress_redraw -- --ignored --exact --nocapture`"]
fn editor_typing_keypress_redraw() {
    const RUNS: usize = 1_000;

    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, None).unwrap();
    let repository = Repository::open(&repo_root).unwrap();
    let month = MonthId::parse("2026-03").unwrap();
    let mut document = repository.create_month_draft(month).unwrap();
    repository.save_month(&mut document).unwrap();
    let loaded = repository.load_month_by_id(month).unwrap();
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::MonthEditing(app_state_from_loaded_for_perf(loaded)),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
    let mut samples = Vec::with_capacity(RUNS);
    for index in 0..RUNS {
        let key = if index % 2 == 0 {
            KeyEvent::from(KeyCode::Char(char::from(b'0' + (index % 10) as u8)))
        } else {
            KeyEvent::from(KeyCode::Backspace)
        };

        let started = Instant::now();
        app.handle_key(key).unwrap();
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

    print_samples("editor_typing_keypress_redraw", &mut samples);
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::redraw::editor_invalid_money_submit_redraw -- --ignored --exact --nocapture`"]
fn editor_invalid_money_submit_redraw() {
    const RUNS: usize = 1_000;

    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, None).unwrap();
    let repository = Repository::open(&repo_root).unwrap();
    let month = MonthId::parse("2026-03").unwrap();
    let mut document = repository.create_month_draft(month).unwrap();
    repository.save_month(&mut document).unwrap();
    let loaded = repository.load_month_by_id(month).unwrap();
    let mut base_state = app_state_from_loaded_for_perf(loaded);
    base_state.focus_index = base_state
        .fields
        .iter()
        .position(FieldId::allows_negative)
        .expect("perf fixture should include a signed editor field");
    let field = base_state.fields[base_state.focus_index].clone();
    let mut invalid_input =
        crate::state::MoneyInput::from_value(field.current_value(&base_state.document), true);
    invalid_input.push('-');
    base_state.edit_buffer = Some(invalid_input);
    base_state.interaction = InteractionState::FieldEditing;

    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::MonthEditing(base_state.clone()),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        app.route = Route::MonthEditing(base_state.clone());

        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
        draw_for_perf(&mut terminal, &app);
        samples.push(started.elapsed());
        match &app.route {
            Route::MonthEditing(state) => {
                assert_eq!(state.message.as_deref(), Some("invalid monetary value `-`"))
            }
            other => panic!("expected editor route, got {other:?}"),
        }
        black_box(terminal.backend().buffer());
    }

    print_samples("editor_invalid_money_submit_redraw", &mut samples);
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::redraw::guided_typing_keypress_redraw -- --ignored --exact --nocapture`"]
fn guided_typing_keypress_redraw() {
    const RUNS: usize = 1_000;

    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, None).unwrap();
    let repository = Repository::open(&repo_root).unwrap();
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Shutdown,
    };
    app.start_guided_creation(MonthId::parse("2026-03").unwrap())
        .unwrap();
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();

    let mut samples = Vec::with_capacity(RUNS);
    for index in 0..RUNS {
        let key = if index % 2 == 0 {
            KeyEvent::from(KeyCode::Char(char::from(b'0' + (index % 10) as u8)))
        } else {
            KeyEvent::from(KeyCode::Backspace)
        };

        let started = Instant::now();
        app.handle_key(key).unwrap();
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

    print_samples("guided_typing_keypress_redraw", &mut samples);
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::redraw::navigation_many_months_keypress_redraw -- --ignored --exact --nocapture`"]
fn navigation_many_months_keypress_redraw() {
    const MONTH_COUNT: usize = 120;
    const RUNS: usize = 1_000;

    let (temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(MONTH_COUNT);
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();

    let mut forward = true;
    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let (selected, month_count) = match &app.route {
            Route::Navigation(state) => (state.selected, state.months.len()),
            other => panic!("expected navigation route, got {other:?}"),
        };
        if selected + 1 >= month_count {
            forward = false;
        } else if selected == 0 {
            forward = true;
        }
        let key = if forward {
            KeyEvent::from(KeyCode::Down)
        } else {
            KeyEvent::from(KeyCode::Up)
        };

        let started = Instant::now();
        app.handle_key(key).unwrap();
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
    print_samples("navigation_many_months_keypress_redraw", &mut samples);
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::redraw::create_dialogue_typing_keypress_redraw -- --ignored --exact --nocapture`"]
fn create_dialogue_typing_keypress_redraw() {
    const RUNS: usize = 1_000;

    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, None).unwrap();
    let repository = Repository::open(&repo_root).unwrap();
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState::new(Vec::new())),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();

    app.handle_key(KeyEvent::from(KeyCode::Char('n'))).unwrap();
    let mut samples = Vec::with_capacity(RUNS);
    for index in 0..RUNS {
        let key = if index % 2 == 0 {
            KeyEvent::from(KeyCode::Char(char::from(b'0' + (index % 10) as u8)))
        } else {
            KeyEvent::from(KeyCode::Backspace)
        };

        let started = Instant::now();
        app.handle_key(key).unwrap();
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

    print_samples("create_dialogue_typing_keypress_redraw", &mut samples);
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::redraw::navigation_dialogue_validation_error_redraw -- --ignored --exact --nocapture`"]
fn navigation_dialogue_validation_error_redraw() {
    const MONTH_COUNT: usize = 120;
    const RUNS: usize = 1_000;

    let (temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(MONTH_COUNT);
    let source = navigation.selected_month().unwrap().document.month;
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(navigation.clone()),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut rename_samples = Vec::with_capacity(RUNS);
    let mut delete_samples = Vec::with_capacity(RUNS);

    for _ in 0..RUNS {
        app.route = Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Rename(RenameDialogue {
                source,
                input: "2026-13".to_owned(),
                error: None,
            })),
            ..navigation.clone()
        });

        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
        draw_for_perf(&mut terminal, &app);
        rename_samples.push(started.elapsed());
        match &app.route {
            Route::Navigation(state) => match &state.dialogue {
                Some(NavigationDialogue::Rename(dialogue)) => assert_eq!(
                    dialogue.error.as_deref(),
                    Some("invalid month id `2026-13`, expected YYYY-MM")
                ),
                other => panic!("expected rename dialogue, got {other:?}"),
            },
            other => panic!("expected navigation route, got {other:?}"),
        }
        black_box(terminal.backend().buffer());
    }

    for _ in 0..RUNS {
        app.route = Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Delete(DeleteDialogue {
                month: source,
                confirmation: "wrong".to_owned(),
                error: None,
            })),
            ..navigation.clone()
        });

        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
        draw_for_perf(&mut terminal, &app);
        delete_samples.push(started.elapsed());
        match &app.route {
            Route::Navigation(state) => match &state.dialogue {
                Some(NavigationDialogue::Delete(dialogue)) => assert_eq!(
                    dialogue.error.as_deref(),
                    Some(format!("Type {source} to confirm deletion").as_str())
                ),
                other => panic!("expected delete dialogue, got {other:?}"),
            },
            other => panic!("expected navigation route, got {other:?}"),
        }
        black_box(terminal.backend().buffer());
    }

    black_box(temp.path());
    print_samples(
        "navigation_dialogue_validation_error_redraw.rename",
        &mut rename_samples,
    );
    print_samples(
        "navigation_dialogue_validation_error_redraw.delete",
        &mut delete_samples,
    );
}
