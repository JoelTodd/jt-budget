use super::*;

#[test]
fn editor_navigation_visits_each_field_once_in_visible_order() {
    let config = AppConfig::default_mvp();
    let document = MonthDocument::new_draft(MonthId::parse("2026-03").unwrap(), &config, None);
    let calculated = calculate_month(&config, &document).unwrap();
    let fields = FieldId::editor_fields(&config);
    let expected = fields.clone();

    let mut app = App {
        repo_root: PathBuf::from("/tmp/budget"),
        repository: None,
        route: Route::MonthEditing(EditorState {
            document,
            calculated,
            fields,
            focus_index: 0,
            edit_buffer: None,
            message: None,
            interaction: InteractionState::SheetIdle,
            persistence: PersistenceState::Clean,
            sync: SyncState::Synced,
            return_navigation: None,
        }),
    };

    let mut visited = Vec::new();
    loop {
        match &app.route {
            Route::MonthEditing(state) => {
                visited.push(state.fields[state.focus_index].clone());
                if state.focus_index + 1 == state.fields.len() {
                    break;
                }
            }
            _ => panic!("editor route unexpectedly changed"),
        }
        app.handle_key(KeyEvent::from(KeyCode::Tab)).unwrap();
    }

    assert_eq!(visited, expected);
    let unique = visited.iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), visited.len());
}

#[test]
fn rename_dialogue_keeps_unchanged_month_error_inline() {
    let (_temp, repo_root, repository, navigation, source) = seeded_navigation_app("2026-03");
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Rename(RenameDialogue {
                source,
                input: source.key(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match app.route {
        Route::Navigation(state) => {
            assert_eq!(state.selected, 0);
            match state.dialogue {
                Some(NavigationDialogue::Rename(dialogue)) => {
                    assert_eq!(dialogue.source, source);
                    assert_eq!(dialogue.input, "2026-03");
                    assert_eq!(
                        dialogue.error.as_deref(),
                        Some("Month is already named 2026-03")
                    );
                }
                other => panic!("expected rename dialogue, got {other:?}"),
            }
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
}

#[test]
fn rename_dialogue_preserves_input_after_validation_error_and_allows_retry() {
    let (_temp, repo_root, repository, navigation, source) = seeded_navigation_app("2026-03");
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Rename(RenameDialogue {
                source,
                input: "2026-13".to_owned(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
    match &app.route {
        Route::Navigation(state) => match &state.dialogue {
            Some(NavigationDialogue::Rename(dialogue)) => {
                assert_eq!(dialogue.input, "2026-13");
                assert_eq!(
                    dialogue.error.as_deref(),
                    Some("invalid month id `2026-13`, expected YYYY-MM")
                );
            }
            other => panic!("expected rename dialogue, got {other:?}"),
        },
        other => panic!("expected navigation route, got {other:?}"),
    }

    app.handle_key(KeyEvent::from(KeyCode::Backspace)).unwrap();
    app.handle_key(KeyEvent::from(KeyCode::Char('2'))).unwrap();

    match &app.route {
        Route::Navigation(state) => match &state.dialogue {
            Some(NavigationDialogue::Rename(dialogue)) => {
                assert_eq!(dialogue.input, "2026-12");
                assert_eq!(dialogue.error, None);
            }
            other => panic!("expected rename dialogue, got {other:?}"),
        },
        other => panic!("expected navigation route, got {other:?}"),
    }

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match app.route {
        Route::Navigation(state) => {
            assert!(state.dialogue.is_none());
            assert_eq!(state.selected, 0);
            assert_eq!(
                state.months[0].document.month,
                MonthId::parse("2026-12").unwrap()
            );
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
}

#[test]
fn rename_dialogue_uses_blocking_failure_for_repository_faults() {
    let (_temp, repo_root, repository, navigation, source) = seeded_navigation_app("2026-03");
    fs::remove_file(repo_root.join("months/2026-03.toml")).unwrap();

    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Rename(RenameDialogue {
                source,
                input: "2026-04".to_owned(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match app.route {
        Route::BlockingFailure(FailureState { title, message, .. }) => {
            assert_eq!(title, "Could not rename 2026-03");
            assert!(message.contains("month `2026-03` does not exist"));
        }
        other => panic!("expected blocking failure, got {other:?}"),
    }
}

#[test]
fn create_dialogue_routes_duplicate_month_to_blocking_failure() {
    let (_temp, repo_root, repository, navigation, month) = seeded_navigation_app("2026-03");
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Create(CreateDialogue {
                input: month.key(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match app.route {
        Route::BlockingFailure(FailureState {
            title,
            message,
            retry,
        }) => {
            assert_eq!(title, "Could not create 2026-03");
            assert!(message.contains("month `2026-03` already exists"));
            assert!(matches!(retry, RetryTarget::CreateMonth(value) if value == month));
        }
        other => panic!("expected blocking failure, got {other:?}"),
    }
}

#[test]
fn create_dialogue_routes_repository_faults_to_blocking_failure() {
    let (_temp, repo_root, repository, navigation, _) = seeded_navigation_app("2026-03");
    fs::remove_dir_all(repo_root.join("months")).unwrap();

    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Create(CreateDialogue {
                input: "2026-04".to_owned(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match app.route {
        Route::BlockingFailure(FailureState { title, message, .. }) => {
            assert_eq!(title, "Could not create 2026-04");
            assert!(message.contains("reading `"));
            assert!(message.contains("/months`"));
        }
        other => panic!("expected blocking failure, got {other:?}"),
    }
}

#[test]
fn create_dialogue_carries_forward_from_fresh_navigation() {
    let (_temp, repo_root, repository, mut navigation) = seeded_many_month_navigation_app(2);
    let previous = navigation
        .months
        .iter_mut()
        .find(|entry| entry.document.month == MonthId::parse("2020-02").unwrap())
        .unwrap();
    previous
        .calculated
        .pot_rows
        .iter_mut()
        .find(|row| row.id == "travel_fund")
        .unwrap()
        .final_balance = budget_core::Money::from_minor(777_777);
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Create(CreateDialogue {
                input: "2020-03".to_owned(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::GuidedCreation(state) => {
            assert_eq!(state.document.month, MonthId::parse("2020-03").unwrap());
            assert_eq!(
                state.document.savings_pots["travel_fund"].carried_over,
                777_777
            );
        }
        other => panic!("expected guided creation route, got {other:?}"),
    }
}

#[test]
fn create_dialogue_reloads_previous_month_when_navigation_is_stale() {
    let (_temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(2);
    fs::write(
        repo_root.join("months/2020-02.toml"),
        month_document_for_perf("2020-02", 99),
    )
    .unwrap();
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Create(CreateDialogue {
                input: "2020-03".to_owned(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::GuidedCreation(state) => {
            assert_eq!(state.document.month, MonthId::parse("2020-03").unwrap());
            assert_eq!(
                state.document.savings_pots["travel_fund"].carried_over,
                80_000 + 99 * 200 + 9_000
            );
        }
        other => panic!("expected guided creation route, got {other:?}"),
    }
}

#[test]
fn create_dialogue_reloads_when_newer_prior_month_appears() {
    let (_temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(2);
    fs::write(
        repo_root.join("months/2020-04.toml"),
        month_document_for_perf("2020-04", 99),
    )
    .unwrap();
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Create(CreateDialogue {
                input: "2020-05".to_owned(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::GuidedCreation(state) => {
            assert_eq!(state.document.month, MonthId::parse("2020-05").unwrap());
            assert_eq!(
                state.document.savings_pots["travel_fund"].carried_over,
                80_000 + 99 * 200 + 9_000
            );
        }
        other => panic!("expected guided creation route, got {other:?}"),
    }
}

#[test]
fn create_dialogue_reloads_when_navigation_fingerprint_is_missing() {
    let (_temp, repo_root, repository, mut navigation) = seeded_many_month_navigation_app(2);
    navigation.fingerprint = None;
    fs::write(
        repo_root.join("months/2020-02.toml"),
        month_document_for_perf("2020-02", 99),
    )
    .unwrap();
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Create(CreateDialogue {
                input: "2020-03".to_owned(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::GuidedCreation(state) => {
            assert_eq!(state.document.month, MonthId::parse("2020-03").unwrap());
            assert_eq!(
                state.document.savings_pots["travel_fund"].carried_over,
                80_000 + 99 * 200 + 9_000
            );
        }
        other => panic!("expected guided creation route, got {other:?}"),
    }
}
