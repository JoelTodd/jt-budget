use super::*;

fn key(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::from(code)).unwrap();
}

fn edit_default(app: &mut App, amount: &str) {
    key(app, KeyCode::Enter);
    for character in amount.chars() {
        key(app, KeyCode::Char(character));
    }
    key(app, KeyCode::Enter);
}

#[test]
fn defaults_editing_saves_prefills_without_changing_existing_months() {
    let (_temp, repo_root, repository, navigation, month) = seeded_navigation_app("2026-04");
    let before = fs::read_to_string(repo_root.join("months/2026-04.toml")).unwrap();
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    key(&mut app, KeyCode::Char('s'));
    edit_default(&mut app, "185");
    key(&mut app, KeyCode::Tab);
    edit_default(&mut app, "150.25");
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Tab);
    edit_default(&mut app, "105");
    let config = app.config().unwrap();
    assert_eq!(config.savings_pots[0].default_monthly_change_minor, 18_500);
    assert_eq!(config.savings_pots[1].default_monthly_change_minor, 15_025);
    assert_eq!(config.next_month_earmarks[0].default_amount_minor, 10_500);
    let future = MonthId::parse("2026-05").unwrap();
    let draft = app
        .repository()
        .unwrap()
        .create_month_draft(future)
        .unwrap();
    assert_eq!(draft.savings_pots["travel_fund"].monthly_change, 18_500);
    assert_eq!(draft.savings_pots["home_upkeep"].monthly_change, 15_025);
    assert_eq!(draft.next_month_earmarks["subscriptions"], 10_500);
    assert_eq!(
        fs::read_to_string(repo_root.join("months/2026-04.toml")).unwrap(),
        before
    );
    key(&mut app, KeyCode::Esc);
    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.selected_month().unwrap().document.month, month)
        }
        other => panic!("expected navigation, got {other:?}"),
    }
    let saved_config = app.config().unwrap().clone();
    drop(app);
    let reopened = Repository::open(&repo_root).unwrap();
    assert_eq!(reopened.config(), &saved_config);
}

#[test]
fn defaults_cancel_and_unchanged_confirmation_do_not_write_or_commit() {
    let (_temp, repo_root, repository, navigation, _) = seeded_navigation_app("2026-04");
    let before = fs::read_to_string(repo_root.join("config.toml")).unwrap();
    let head = git_capture(&repo_root, &["rev-parse", "HEAD"]);
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    key(&mut app, KeyCode::Char('s'));
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Char('9'));
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Esc);
    assert!(matches!(app.route, Route::Navigation(_)));
    assert_eq!(
        fs::read_to_string(repo_root.join("config.toml")).unwrap(),
        before
    );
    assert_eq!(git_capture(&repo_root, &["rev-parse", "HEAD"]), head);
}

#[test]
fn defaults_save_failure_blocks_and_retries_the_unsaved_value() {
    let (_temp, repo_root, repository, navigation, _) = seeded_navigation_app("2026-04");
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    fs::write(repo_root.join(".git/index.lock"), "busy").unwrap();
    key(&mut app, KeyCode::Char('s'));
    edit_default(&mut app, "185");
    assert!(matches!(
        &app.route,
        Route::BlockingFailure(FailureState {
            retry: RetryTarget::DefaultsSave(_),
            ..
        })
    ));
    key(&mut app, KeyCode::Char('s'));
    assert!(matches!(app.route, Route::BlockingFailure(_)));
    fs::remove_file(repo_root.join(".git/index.lock")).unwrap();
    key(&mut app, KeyCode::Char('r'));
    assert!(matches!(app.route, Route::DefaultsEditing(_)));
    assert_eq!(
        app.config().unwrap().savings_pots[0].default_monthly_change_minor,
        18_500
    );
    assert!(git_capture(&repo_root, &["status", "--porcelain"]).is_empty());
}

#[test]
fn defaults_push_failure_blocks_and_retries_without_an_empty_commit() {
    let (_temp, repo_root, remote, repository, navigation, _) =
        seeded_navigation_app_with_remote("2026-04");
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    set_remote_url(&repo_root, "/missing/defaults-remote.git");
    key(&mut app, KeyCode::Char('s'));
    edit_default(&mut app, "185");
    assert!(matches!(app.route, Route::BlockingFailure(_)));
    let committed = git_capture(&repo_root, &["rev-parse", "HEAD"]);
    set_remote_url(&repo_root, remote.to_str().unwrap());
    key(&mut app, KeyCode::Char('r'));
    assert!(matches!(app.route, Route::DefaultsEditing(_)));
    assert_eq!(git_capture(&repo_root, &["rev-parse", "HEAD"]), committed);
    assert_eq!(
        git_capture(&repo_root, &["rev-parse", "origin/main"]),
        committed
    );
}

#[test]
fn defaults_screen_handles_config_without_pots_or_earmarks() {
    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    let mut config = AppConfig::default_mvp();
    config.savings_pots.clear();
    config.next_month_earmarks.clear();
    Repository::init_with_config(&repo_root, None, &config).unwrap();
    let repository = Repository::open(&repo_root).unwrap();
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState::new(vec![])),
    };
    key(&mut app, KeyCode::Char('s'));
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Esc);
    assert!(matches!(app.route, Route::Navigation(_)));
}
