use super::*;

pub(super) fn seeded_navigation_app(
    month_key: &str,
) -> (TempDir, PathBuf, Repository, NavigationState, MonthId) {
    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, None).unwrap();

    let repository = Repository::open(&repo_root).unwrap();
    let month = MonthId::parse(month_key).unwrap();
    let mut document = repository.create_month_draft(month).unwrap();
    repository.save_month(&mut document).unwrap();
    let navigation = navigation_from_repository(&repository);

    (temp, repo_root, repository, navigation, month)
}

pub(super) fn seeded_navigation_app_with_remote(
    month_key: &str,
) -> (
    TempDir,
    PathBuf,
    PathBuf,
    Repository,
    NavigationState,
    MonthId,
) {
    let temp = tempfile::tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);

    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, Some(remote.to_str().unwrap())).unwrap();

    let repository = Repository::open(&repo_root).unwrap();
    let month = MonthId::parse(month_key).unwrap();
    let mut document = repository.create_month_draft(month).unwrap();
    repository.save_month(&mut document).unwrap();
    let navigation = navigation_from_repository(&repository);

    (temp, repo_root, remote, repository, navigation, month)
}

pub(super) fn seeded_many_month_navigation_app(
    count: usize,
) -> (TempDir, PathBuf, Repository, NavigationState) {
    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, None).unwrap();
    populate_month_files(&repo_root, count);
    git(&repo_root, &["add", "-A", "--", "months"]);
    git(
        &repo_root,
        &["commit", "--quiet", "-m", "Seed large month history"],
    );

    let repository = Repository::open(&repo_root).unwrap();
    let navigation = navigation_from_repository(&repository);

    (temp, repo_root, repository, navigation)
}

pub(super) fn seeded_many_month_navigation_app_with_remote(
    count: usize,
) -> (TempDir, PathBuf, PathBuf, Repository, NavigationState) {
    let temp = tempfile::tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);

    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, Some(remote.to_str().unwrap())).unwrap();
    populate_month_files(&repo_root, count);
    git(&repo_root, &["add", "-A", "--", "months"]);
    git(
        &repo_root,
        &["commit", "--quiet", "-m", "Seed large month history"],
    );
    git(&repo_root, &["push"]);

    let repository = Repository::open(&repo_root).unwrap();
    let navigation = navigation_from_repository(&repository);

    (temp, repo_root, remote, repository, navigation)
}

pub(super) fn navigation_from_repository(repository: &Repository) -> NavigationState {
    let (loaded_months, fingerprint) = repository.list_months_with_fingerprint().unwrap();
    NavigationState::with_fingerprint(
        loaded_months
            .into_iter()
            .map(|loaded| MonthEntry {
                document: loaded.document,
                calculated: loaded.calculated,
            })
            .collect(),
        fingerprint,
    )
}

pub(super) fn stage_create_dialogue_for_perf(app: &mut App, index: usize) {
    let mut state = match &app.route {
        Route::Navigation(state) => state.clone(),
        other => panic!("expected navigation route, got {other:?}"),
    };
    let year = 2070 + index / 12;
    let month = index % 12 + 1;
    state.dialogue = Some(NavigationDialogue::Create(CreateDialogue {
        input: format!("{year:04}-{month:02}"),
        error: None,
    }));
    app.route = Route::Navigation(state);
}

pub(super) fn start_guided_draft_for_perf(app: &mut App, index: usize) {
    let year = 2070 + index / 12;
    let month = index % 12 + 1;
    let month = MonthId::parse(&format!("{year:04}-{month:02}")).unwrap();
    app.start_guided_creation(month).unwrap();
    assert!(matches!(app.route, Route::GuidedCreation(_)));
}

pub(super) fn make_navigation_stale_for_perf(repo_root: &Path, index: usize) {
    fs::write(
        repo_root.join("months/2020-01.toml"),
        month_document_for_perf("2020-01", 900 + index),
    )
    .unwrap();
}

pub(super) fn stage_rename_dialogue_for_perf(app: &mut App, index: usize) {
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
}

pub(super) fn stage_delete_dialogue_for_perf(app: &mut App) {
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
}

pub(super) fn autosave_first_editor_field_for_perf(app: &mut App, index: usize) {
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
    app.commit_editor_field(state).unwrap();
    assert!(matches!(app.route, Route::MonthEditing(_)));
}

pub(super) fn set_remote_url(repo_root: &Path, remote: &str) {
    git(repo_root, &["config", "remote.origin.url", remote]);
}
