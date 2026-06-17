use super::*;

#[test]
fn editor_exit_restores_cached_navigation_when_month_files_are_unchanged() {
    let (_temp, repo_root, repository, navigation, _month) = seeded_navigation_app("2026-03");
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
    let Route::MonthEditing(mut state) = std::mem::replace(&mut app.route, Route::Shutdown) else {
        panic!("expected editor route");
    };
    let cached = state.return_navigation.as_mut().unwrap();
    cached.dialogue = Some(NavigationDialogue::Create(CreateDialogue {
        input: "cached-marker".to_owned(),
        error: None,
    }));
    app.route = Route::MonthEditing(state);

    app.handle_key(KeyEvent::from(KeyCode::Esc)).unwrap();

    match app.route {
        Route::Navigation(state) => match state.dialogue {
            Some(NavigationDialogue::Create(dialogue)) => {
                assert_eq!(dialogue.input, "cached-marker");
            }
            other => panic!("expected cached create dialogue marker, got {other:?}"),
        },
        other => panic!("expected navigation route, got {other:?}"),
    }
}

#[test]
fn editor_exit_reloads_navigation_when_month_file_changes_externally() {
    let (_temp, repo_root, repository, navigation, _month) = seeded_navigation_app("2026-03");
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
    fs::write(
        repo_root.join("months/2026-03.toml"),
        month_document_for_perf("2026-03", 99),
    )
    .unwrap();
    app.handle_key(KeyEvent::from(KeyCode::Esc)).unwrap();

    match app.route {
        Route::Navigation(state) => {
            assert_eq!(
                state.months[0].document.accounts["current_account"],
                200_000 + 99 * 750
            );
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
}

#[test]
fn editor_exit_after_autosave_restores_updated_cached_navigation() {
    let (_temp, repo_root, repository, navigation, _month) = seeded_navigation_app("2026-03");
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
    let Route::MonthEditing(mut state) = std::mem::replace(&mut app.route, Route::Shutdown) else {
        panic!("expected editor route");
    };
    let cached = state.return_navigation.as_mut().unwrap();
    cached.dialogue = Some(NavigationDialogue::Create(CreateDialogue {
        input: "cached-marker".to_owned(),
        error: None,
    }));
    state.focus_index = 0;
    state.edit_buffer = Some(crate::state::MoneyInput::from_value(
        budget_core::Money::from_minor(123_456),
        false,
    ));
    state.interaction = InteractionState::FieldEditing;
    app.commit_editor_field(state).unwrap();
    app.handle_key(KeyEvent::from(KeyCode::Esc)).unwrap();

    match app.route {
        Route::Navigation(state) => {
            assert_eq!(
                state.months[0].document.accounts["current_account"],
                123_456
            );
            match state.dialogue {
                Some(NavigationDialogue::Create(dialogue)) => {
                    assert_eq!(dialogue.input, "cached-marker");
                }
                other => panic!("expected cached create dialogue marker, got {other:?}"),
            }
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
}

#[test]
fn editor_exit_after_autosave_reloads_when_another_month_changes() {
    let (_temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(2);
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
    let Route::MonthEditing(mut state) = std::mem::replace(&mut app.route, Route::Shutdown) else {
        panic!("expected editor route");
    };
    let cached = state.return_navigation.as_mut().unwrap();
    cached.dialogue = Some(NavigationDialogue::Create(CreateDialogue {
        input: "cached-marker".to_owned(),
        error: None,
    }));
    state.focus_index = 0;
    state.edit_buffer = Some(crate::state::MoneyInput::from_value(
        budget_core::Money::from_minor(123_456),
        false,
    ));
    state.interaction = InteractionState::FieldEditing;
    app.commit_editor_field(state).unwrap();
    fs::write(
        repo_root.join("months/2020-01.toml"),
        month_document_for_perf("2020-01", 99),
    )
    .unwrap();
    app.handle_key(KeyEvent::from(KeyCode::Esc)).unwrap();

    match app.route {
        Route::Navigation(state) => {
            assert!(state.dialogue.is_none());
            assert_eq!(
                state
                    .months
                    .iter()
                    .find(|entry| entry.document.month == MonthId::parse("2020-01").unwrap())
                    .unwrap()
                    .document
                    .accounts["current_account"],
                200_000 + 99 * 750
            );
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
}

#[test]
fn remote_delete_from_stale_navigation_reloads_then_syncs_pending_push() {
    let (_temp, repo_root, _remote, repository, navigation) =
        seeded_many_month_navigation_app_with_remote(2);
    let deleted = MonthId::parse("2020-02").unwrap();
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
    fs::write(
        repo_root.join("months/2020-01.toml"),
        month_document_for_perf("2020-01", 99),
    )
    .unwrap();

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.sync, SyncState::SyncPending);
            assert_eq!(
                state.message.as_deref(),
                Some("Deleted 2020-02; sync pending")
            );
            let changed = state
                .months
                .iter()
                .find(|entry| entry.document.month == MonthId::parse("2020-01").unwrap())
                .unwrap();
            assert_eq!(
                changed.document.accounts["current_account"],
                200_000 + 99 * 750
            );
            assert_eq!(
                state.fingerprint.as_ref().unwrap(),
                &app.repository
                    .as_ref()
                    .unwrap()
                    .month_file_fingerprint()
                    .unwrap()
            );
        }
        other => panic!("expected pending navigation route, got {other:?}"),
    }

    assert!(app.process_pending_sync().unwrap());
    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.sync, SyncState::Synced);
            assert_eq!(state.message.as_deref(), Some("Repository changes synced"));
        }
        other => panic!("expected synced navigation route, got {other:?}"),
    }
}
