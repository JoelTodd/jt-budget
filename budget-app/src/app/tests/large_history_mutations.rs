use super::*;

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::large_history_mutations::guided_exit_large_history -- --ignored --exact --nocapture`"]
fn guided_exit_large_history() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 30;

    let (temp, repo_root, repository, _navigation) = seeded_many_month_navigation_app(MONTH_COUNT);
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState::new(Vec::new())),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut samples = Vec::with_capacity(RUNS);

    for index in 0..RUNS {
        start_guided_draft_for_perf(&mut app, index);

        let started = Instant::now();
        app.handle_key(KeyEvent::from(KeyCode::Esc)).unwrap();
        draw_for_perf(&mut terminal, &app);
        samples.push(started.elapsed());
        assert!(matches!(app.route, Route::Navigation(_)));
        black_box(terminal.backend().buffer());
    }

    black_box(temp.path());
    print_samples("guided_exit_large_history", &mut samples);
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::large_history_mutations::rename_month_large_history -- --ignored --exact --nocapture`"]
fn rename_month_large_history() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 20;

    let (temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(MONTH_COUNT);
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut samples = Vec::with_capacity(RUNS);

    for index in 0..RUNS {
        let mut state = match &app.route {
            Route::Navigation(state) => state.clone(),
            other => panic!("expected navigation route, got {other:?}"),
        };
        state.selected = 0;
        let source = state.selected_month().unwrap().document.month;
        let target_year = 2070 + index / 12;
        let target_month = index % 12 + 1;
        let target = MonthId::parse(&format!("{target_year:04}-{target_month:02}")).unwrap();
        state.dialogue = Some(NavigationDialogue::Rename(RenameDialogue {
            source,
            input: target.key(),
            error: None,
        }));
        app.route = Route::Navigation(state);

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
    print_samples("rename_month_large_history", &mut samples);
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::large_history_mutations::delete_month_large_history -- --ignored --exact --nocapture`"]
fn delete_month_large_history() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 20;

    let (temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(MONTH_COUNT);
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut samples = Vec::with_capacity(RUNS);

    for _ in 0..RUNS {
        let mut state = match &app.route {
            Route::Navigation(state) => state.clone(),
            other => panic!("expected navigation route, got {other:?}"),
        };
        state.selected = 0;
        let month = state.selected_month().unwrap().document.month;
        state.dialogue = Some(NavigationDialogue::Delete(DeleteDialogue {
            month,
            confirmation: month.key(),
            error: None,
        }));
        app.route = Route::Navigation(state);

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
    print_samples("delete_month_large_history", &mut samples);
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::large_history_mutations::rename_delete_large_history_breakdown -- --ignored --exact --nocapture`"]
fn rename_delete_large_history_breakdown() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 20;

    let (temp, repo_root, repository, mut navigation) =
        seeded_many_month_navigation_app(MONTH_COUNT);
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut samples =
        collect_rename_delete_breakdown(&repository, &mut navigation, &mut terminal, RUNS);

    black_box((temp.path(), repo_root));
    print_rename_delete_breakdown(&mut samples);
}
