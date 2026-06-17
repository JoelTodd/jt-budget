use super::*;

pub(super) fn app_state_from_loaded_for_perf(
    loaded: crate::repository::LoadedMonth,
) -> EditorState {
    EditorState {
        document: loaded.document,
        calculated: loaded.calculated,
        fields: FieldId::editor_fields(&AppConfig::default_mvp()),
        focus_index: 0,
        edit_buffer: None,
        message: None,
        interaction: InteractionState::SheetIdle,
        persistence: PersistenceState::Clean,
        sync: SyncState::Synced,
        return_navigation: None,
    }
}

pub(super) fn format_duration(duration: Duration) -> String {
    format!("{:.3}ms", duration.as_secs_f64() * 1000.0)
}

pub(super) fn print_samples(scenario: &str, samples: &mut [Duration]) {
    samples.sort();
    let runs = samples.len();
    println!(
        "scenario={scenario} runs={runs} min={} median={} p90={} max={}",
        format_duration(samples[0]),
        format_duration(samples[runs / 2]),
        format_duration(samples[(runs * 9 / 10).min(runs - 1)]),
        format_duration(samples[runs - 1]),
    );
}

pub(super) fn print_samples_if_any(scenario: &str, samples: &mut [Duration]) {
    if !samples.is_empty() {
        print_samples(scenario, samples);
    }
}

pub(super) fn measure_guided_creation_breakdown(scenario: &str, app: &mut App, runs: usize) {
    let mut total_samples = Vec::with_capacity(runs);
    let mut start_feedback_samples = Vec::with_capacity(runs);
    let mut start_sync_samples = Vec::new();
    let mut changed_step_feedback_samples = Vec::new();
    let mut changed_step_sync_samples = Vec::new();
    let mut noop_step_samples = Vec::new();
    let mut final_step_feedback_samples = Vec::new();
    let mut final_step_sync_samples = Vec::new();

    for index in 0..runs {
        let year = 2027 + index / 12;
        let month_number = index % 12 + 1;
        let month = MonthId::parse(&format!("{year:04}-{month_number:02}")).unwrap();
        let total_started = Instant::now();

        let started = Instant::now();
        app.start_guided_creation(month).unwrap();
        start_feedback_samples.push(started.elapsed());

        let started = Instant::now();
        if app.process_pending_sync().unwrap() {
            start_sync_samples.push(started.elapsed());
        }

        loop {
            let (state, changed) = match &app.route {
                Route::GuidedCreation(state) => {
                    let mut state = state.clone();
                    let field = state.steps[state.step_index].clone();
                    let value = budget_core::Money::from_minor(guided_perf_value(state.step_index));
                    let changed = field.current_value(&state.document) != value;
                    state.input =
                        crate::state::MoneyInput::from_value(value, field.allows_negative());
                    (state, changed)
                }
                Route::MonthEditing(_) => break,
                other => panic!("expected guided or editor route, got {other:?}"),
            };

            let started = Instant::now();
            app.commit_guided_step(state).unwrap();
            let elapsed = started.elapsed();
            match (&app.route, changed) {
                (Route::MonthEditing(_), _) => final_step_feedback_samples.push(elapsed),
                (_, true) => changed_step_feedback_samples.push(elapsed),
                (_, false) => noop_step_samples.push(elapsed),
            }

            let started = Instant::now();
            if app.process_pending_sync().unwrap() {
                let elapsed = started.elapsed();
                match &app.route {
                    Route::MonthEditing(_) => final_step_sync_samples.push(elapsed),
                    Route::GuidedCreation(_) if changed => changed_step_sync_samples.push(elapsed),
                    _ => {}
                }
            }
        }

        total_samples.push(total_started.elapsed());
        app.reload_navigation(None);
        black_box(&app.route);
    }

    print_samples(scenario, &mut total_samples);
    print_samples(
        &format!("{scenario}.start.feedback"),
        &mut start_feedback_samples,
    );
    print_samples_if_any(&format!("{scenario}.start.sync"), &mut start_sync_samples);
    print_samples(
        &format!("{scenario}.changed_step.feedback"),
        &mut changed_step_feedback_samples,
    );
    print_samples_if_any(
        &format!("{scenario}.changed_step.sync"),
        &mut changed_step_sync_samples,
    );
    print_samples(&format!("{scenario}.noop_step"), &mut noop_step_samples);
    print_samples(
        &format!("{scenario}.final_step.feedback"),
        &mut final_step_feedback_samples,
    );
    print_samples_if_any(
        &format!("{scenario}.final_step.sync"),
        &mut final_step_sync_samples,
    );
}

pub(super) fn measure_editor_commit_changed_autosave(scenario: &str, app: &mut App, runs: usize) {
    let mut samples = Vec::with_capacity(runs);
    for index in 0..runs {
        let state = match &app.route {
            Route::MonthEditing(state) => {
                let mut state = state.clone();
                state.focus_index = 0;
                state.edit_buffer = Some(crate::state::MoneyInput::from_value(
                    budget_core::Money::from_minor(100_000 + index as i64),
                    false,
                ));
                state.interaction = InteractionState::FieldEditing;
                state
            }
            other => panic!("expected editor route, got {other:?}"),
        };

        let started = Instant::now();
        app.commit_editor_field(state).unwrap();
        samples.push(started.elapsed());
        app.process_pending_sync().unwrap();
        black_box(&app.route);
    }

    samples.sort();
    println!(
        "scenario={scenario} runs={runs} min={} median={} p90={} max={}",
        format_duration(samples[0]),
        format_duration(samples[runs / 2]),
        format_duration(samples[(runs * 9 / 10).min(runs - 1)]),
        format_duration(samples[runs - 1]),
    );
}

pub(super) fn measure_editor_commit_changed_autosave_breakdown(
    scenario: &str,
    app: &mut App,
    runs: usize,
) {
    let mut feedback_samples = Vec::with_capacity(runs);
    let mut sync_samples = Vec::with_capacity(runs);
    let mut total_samples = Vec::with_capacity(runs);

    for index in 0..runs {
        let state = match &app.route {
            Route::MonthEditing(state) => {
                let mut state = state.clone();
                state.focus_index = 0;
                state.edit_buffer = Some(crate::state::MoneyInput::from_value(
                    budget_core::Money::from_minor(100_000 + index as i64),
                    false,
                ));
                state.interaction = InteractionState::FieldEditing;
                state
            }
            other => panic!("expected editor route, got {other:?}"),
        };

        let total_started = Instant::now();
        let started = Instant::now();
        app.commit_editor_field(state).unwrap();
        feedback_samples.push(started.elapsed());

        let started = Instant::now();
        app.process_pending_sync().unwrap();
        sync_samples.push(started.elapsed());
        total_samples.push(total_started.elapsed());
        black_box(&app.route);
    }

    print_samples(&format!("{scenario}.feedback"), &mut feedback_samples);
    print_samples(&format!("{scenario}.sync"), &mut sync_samples);
    print_samples(&format!("{scenario}.total"), &mut total_samples);
}

pub(super) struct RenameDeleteBreakdownSamples {
    rename_repository: Vec<Duration>,
    rename_update: Vec<Duration>,
    rename_draw: Vec<Duration>,
    delete_repository: Vec<Duration>,
    delete_update: Vec<Duration>,
    delete_draw: Vec<Duration>,
}

impl RenameDeleteBreakdownSamples {
    fn with_capacity(runs: usize) -> Self {
        Self {
            rename_repository: Vec::with_capacity(runs),
            rename_update: Vec::with_capacity(runs),
            rename_draw: Vec::with_capacity(runs),
            delete_repository: Vec::with_capacity(runs),
            delete_update: Vec::with_capacity(runs),
            delete_draw: Vec::with_capacity(runs),
        }
    }
}

pub(super) fn collect_rename_delete_breakdown(
    repository: &Repository,
    navigation: &mut NavigationState,
    terminal: &mut Terminal<TestBackend>,
    runs: usize,
) -> RenameDeleteBreakdownSamples {
    let mut samples = RenameDeleteBreakdownSamples::with_capacity(runs);
    collect_rename_breakdown(repository, navigation, terminal, runs, &mut samples);
    collect_delete_breakdown(repository, navigation, terminal, runs, &mut samples);
    samples
}

fn collect_rename_breakdown(
    repository: &Repository,
    navigation: &mut NavigationState,
    terminal: &mut Terminal<TestBackend>,
    runs: usize,
    samples: &mut RenameDeleteBreakdownSamples,
) {
    for index in 0..runs {
        navigation.selected = 0;
        let source = navigation.selected_month().unwrap().document.month;
        let target_year = 2070 + index / 12;
        let target_month = index % 12 + 1;
        let target = MonthId::parse(&format!("{target_year:04}-{target_month:02}")).unwrap();

        let started = Instant::now();
        let (_outcome, loaded) = repository.rename_month(source, target).unwrap();
        samples.rename_repository.push(started.elapsed());

        let started = Instant::now();
        replace_renamed_entry(navigation, source, target, loaded);
        samples.rename_update.push(started.elapsed());

        let started = Instant::now();
        draw_navigation_route(repository, navigation, terminal);
        samples.rename_draw.push(started.elapsed());
        black_box(terminal.backend().buffer());
    }
}

fn collect_delete_breakdown(
    repository: &Repository,
    navigation: &mut NavigationState,
    terminal: &mut Terminal<TestBackend>,
    runs: usize,
    samples: &mut RenameDeleteBreakdownSamples,
) {
    for _ in 0..runs {
        navigation.selected = 0;
        let month = navigation.selected_month().unwrap().document.month;

        let started = Instant::now();
        let _outcome = repository.delete_month(month).unwrap();
        samples.delete_repository.push(started.elapsed());

        let started = Instant::now();
        remove_deleted_entry(navigation, month);
        samples.delete_update.push(started.elapsed());

        let started = Instant::now();
        draw_navigation_route(repository, navigation, terminal);
        samples.delete_draw.push(started.elapsed());
        black_box(terminal.backend().buffer());
    }
}

fn replace_renamed_entry(
    navigation: &mut NavigationState,
    source: MonthId,
    target: MonthId,
    loaded: crate::repository::LoadedMonth,
) {
    let index = navigation
        .months
        .iter()
        .position(|entry| entry.document.month == source)
        .unwrap();
    navigation.months[index] = MonthEntry {
        document: loaded.document,
        calculated: loaded.calculated,
    };
    navigation
        .months
        .sort_by_key(|entry| std::cmp::Reverse(entry.document.month));
    navigation.selected = navigation
        .months
        .iter()
        .position(|entry| entry.document.month == target)
        .unwrap();
}

fn remove_deleted_entry(navigation: &mut NavigationState, month: MonthId) {
    let index = navigation
        .months
        .iter()
        .position(|entry| entry.document.month == month)
        .unwrap();
    navigation.months.remove(index);
    navigation.selected = if navigation.months.is_empty() {
        0
    } else {
        index.min(navigation.months.len() - 1)
    };
}

fn draw_navigation_route(
    repository: &Repository,
    navigation: &NavigationState,
    terminal: &mut Terminal<TestBackend>,
) {
    let route = Route::Navigation(navigation.clone());
    terminal
        .draw(|frame| {
            crate::ui::render(
                frame,
                &route,
                Path::new("/tmp/budget"),
                Some(repository.config()),
            )
        })
        .unwrap();
}

pub(super) fn print_rename_delete_breakdown(samples: &mut RenameDeleteBreakdownSamples) {
    print_samples(
        "rename_delete_large_history_breakdown.rename_repository",
        &mut samples.rename_repository,
    );
    print_samples(
        "rename_delete_large_history_breakdown.rename_update",
        &mut samples.rename_update,
    );
    print_samples(
        "rename_delete_large_history_breakdown.rename_draw",
        &mut samples.rename_draw,
    );
    print_samples(
        "rename_delete_large_history_breakdown.delete_repository",
        &mut samples.delete_repository,
    );
    print_samples(
        "rename_delete_large_history_breakdown.delete_update",
        &mut samples.delete_update,
    );
    print_samples(
        "rename_delete_large_history_breakdown.delete_draw",
        &mut samples.delete_draw,
    );
}

pub(super) fn guided_perf_value(step_index: usize) -> i64 {
    match step_index {
        0 => 200_000,
        1 => 50_000,
        2 => 10_000,
        3 => 5_000,
        4 => 1_500,
        5 => 18_000,
        6 => 80_000,
        7 => 20_000,
        8 => 10_000,
        9 => 9_000,
        10 => 5_500,
        11 => 3_500,
        12 => 12_000,
        13 => 32_000,
        _ => 0,
    }
}
