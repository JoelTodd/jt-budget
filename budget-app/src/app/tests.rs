use std::fs;
use std::hint::black_box;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use budget_core::{AppConfig, MonthDocument, MonthId, calculate_month};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tempfile::TempDir;

use super::App;
use crate::repository::Repository;
use crate::state::{
    CreateDialogue, DeleteDialogue, EditorState, FailureState, FieldId, InteractionState,
    MonthEntry, NavigationDialogue, NavigationState, PersistenceState, RenameDialogue, RetryTarget,
    Route, SyncState,
};

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

#[test]
fn unchanged_editor_commit_does_not_autosave_or_commit() {
    let (_temp, repo_root, repository, _navigation, month) = seeded_navigation_app("2026-03");
    let loaded = repository.load_month_by_id(month).unwrap();
    let before = git_capture(&repo_root, &["rev-list", "--count", "HEAD"]);
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Shutdown,
    };
    app.route = Route::MonthEditing(app.editor_state_from_loaded(loaded));

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    let after = git_capture(&repo_root, &["rev-list", "--count", "HEAD"]);
    assert_eq!(after, before);
    match app.route {
        Route::MonthEditing(state) => {
            assert_eq!(state.interaction, InteractionState::SheetIdle);
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::Synced);
            assert_eq!(state.message, None);
        }
        other => panic!("expected editor route, got {other:?}"),
    }
}

#[test]
fn unchanged_guided_step_advances_without_autosave_commit() {
    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, None).unwrap();
    let repository = Repository::open(&repo_root).unwrap();
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Shutdown,
    };
    app.start_guided_creation(MonthId::parse("2026-03").unwrap())
        .unwrap();
    let before = git_capture(&repo_root, &["rev-list", "--count", "HEAD"]);

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    let after = git_capture(&repo_root, &["rev-list", "--count", "HEAD"]);
    assert_eq!(after, before);
    match app.route {
        Route::GuidedCreation(state) => {
            assert_eq!(state.step_index, 1);
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::Synced);
            assert_eq!(state.message, None);
        }
        other => panic!("expected guided route, got {other:?}"),
    }
}

#[test]
fn local_guided_creation_commits_once_on_completion() {
    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, None).unwrap();
    let repository = Repository::open(&repo_root).unwrap();
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Shutdown,
    };
    let initial_commits = git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
        .parse::<usize>()
        .unwrap();

    app.start_guided_creation(MonthId::parse("2026-03").unwrap())
        .unwrap();

    assert!(repo_root.join("months/2026-03.toml").is_file());
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        initial_commits
    );

    loop {
        let state = match &app.route {
            Route::GuidedCreation(state) => {
                let mut state = state.clone();
                let field = state.steps[state.step_index].clone();
                state.input = crate::state::MoneyInput::from_value(
                    budget_core::Money::from_minor(guided_perf_value(state.step_index)),
                    field.allows_negative(),
                );
                state
            }
            Route::MonthEditing(_) => break,
            other => panic!("expected guided or editor route, got {other:?}"),
        };
        app.commit_guided_step(state).unwrap();
    }

    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        initial_commits + 1
    );
    assert_eq!(git_capture(&repo_root, &["status", "--short"]), "");
}

#[test]
fn remote_editor_autosave_blocks_failed_pending_push_and_retries_push_only() {
    let (_temp, repo_root, remote, repository, _navigation, month) =
        seeded_navigation_app_with_remote("2026-03");
    let loaded = repository.load_month_by_id(month).unwrap();
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::MonthEditing(app_state_from_loaded_for_perf(loaded)),
    };
    let before = git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
        .parse::<usize>()
        .unwrap();
    set_remote_url(&repo_root, "/definitely/missing/repo.git");

    autosave_first_editor_field_for_perf(&mut app, 0);

    let local_commit = git_capture(&repo_root, &["rev-parse", "HEAD"]);
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );
    match &app.route {
        Route::MonthEditing(state) => {
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::SyncPending);
            assert_eq!(
                state.message.as_deref(),
                Some("Month autosaved; sync pending")
            );
        }
        other => panic!("expected pending editor route, got {other:?}"),
    }

    app.process_pending_sync().unwrap();

    match &app.route {
        Route::BlockingFailure(FailureState { title, retry, .. }) => {
            assert_eq!(title, "Could not sync 2026-03");
            assert!(matches!(retry, RetryTarget::EditorPush(_)));
        }
        other => panic!("expected blocking failure, got {other:?}"),
    }
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );

    set_remote_url(&repo_root, remote.to_str().unwrap());
    let retry = match std::mem::replace(&mut app.route, Route::Shutdown) {
        Route::BlockingFailure(state) => state.retry,
        other => panic!("expected blocking failure, got {other:?}"),
    };
    app.retry_failure(retry).unwrap();

    match &app.route {
        Route::MonthEditing(state) => {
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::Synced);
        }
        other => panic!("expected editor route, got {other:?}"),
    }
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );
    assert!(
        git_capture(&repo_root, &["ls-remote", "origin", "refs/heads/main"])
            .contains(&local_commit)
    );
}

#[test]
fn remote_navigation_rename_blocks_failed_pending_push_and_retries_push_only() {
    let (_temp, repo_root, remote, repository, navigation, source) =
        seeded_navigation_app_with_remote("2026-03");
    let target = MonthId::parse("2026-04").unwrap();
    let before = git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
        .parse::<usize>()
        .unwrap();
    set_remote_url(&repo_root, "/definitely/missing/repo.git");
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Rename(RenameDialogue {
                source,
                input: target.key(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    let local_commit = git_capture(&repo_root, &["rev-parse", "HEAD"]);
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );
    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::SyncPending);
            assert_eq!(
                state.message.as_deref(),
                Some("Renamed 2026-03 to 2026-04; sync pending")
            );
            assert_eq!(state.months[0].document.month, target);
        }
        other => panic!("expected pending navigation route, got {other:?}"),
    }

    assert!(app.process_pending_sync().unwrap());

    match &app.route {
        Route::BlockingFailure(FailureState { title, retry, .. }) => {
            assert_eq!(title, "Could not sync repository changes");
            assert!(matches!(
                retry,
                RetryTarget::PushNavigation(Some(month)) if *month == target
            ));
        }
        other => panic!("expected blocking failure, got {other:?}"),
    }
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );

    set_remote_url(&repo_root, remote.to_str().unwrap());
    let retry = match std::mem::replace(&mut app.route, Route::Shutdown) {
        Route::BlockingFailure(state) => state.retry,
        other => panic!("expected blocking failure, got {other:?}"),
    };
    app.retry_failure(retry).unwrap();

    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.sync, SyncState::Synced);
            assert_eq!(state.months[0].document.month, target);
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
    assert!(
        git_capture(&repo_root, &["ls-remote", "origin", "refs/heads/main"])
            .contains(&local_commit)
    );
}

#[test]
fn remote_navigation_delete_blocks_failed_pending_push_and_retries_push_only() {
    let (_temp, repo_root, remote, repository, navigation) =
        seeded_many_month_navigation_app_with_remote(2);
    let deleted = MonthId::parse("2020-02").unwrap();
    let before = git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
        .parse::<usize>()
        .unwrap();
    set_remote_url(&repo_root, "/definitely/missing/repo.git");
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

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    let local_commit = git_capture(&repo_root, &["rev-parse", "HEAD"]);
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );
    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::SyncPending);
            assert_eq!(
                state.message.as_deref(),
                Some("Deleted 2020-02; sync pending")
            );
            assert!(
                state
                    .months
                    .iter()
                    .all(|entry| entry.document.month != deleted)
            );
        }
        other => panic!("expected pending navigation route, got {other:?}"),
    }

    assert!(app.process_pending_sync().unwrap());

    match &app.route {
        Route::BlockingFailure(FailureState { title, retry, .. }) => {
            assert_eq!(title, "Could not sync repository changes");
            assert!(matches!(retry, RetryTarget::PushNavigation(Some(_))));
        }
        other => panic!("expected blocking failure, got {other:?}"),
    }
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );

    set_remote_url(&repo_root, remote.to_str().unwrap());
    let retry = match std::mem::replace(&mut app.route, Route::Shutdown) {
        Route::BlockingFailure(state) => state.retry,
        other => panic!("expected blocking failure, got {other:?}"),
    };
    app.retry_failure(retry).unwrap();

    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.sync, SyncState::Synced);
            assert!(
                state
                    .months
                    .iter()
                    .all(|entry| entry.document.month != deleted)
            );
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
    assert!(
        git_capture(&repo_root, &["ls-remote", "origin", "refs/heads/main"])
            .contains(&local_commit)
    );
}

#[test]
fn remote_guided_creation_blocks_failed_pending_push_and_retries_push_only() {
    let temp = tempfile::tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, Some(remote.to_str().unwrap())).unwrap();
    let repository = Repository::open(&repo_root).unwrap();
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Shutdown,
    };
    let before = git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
        .parse::<usize>()
        .unwrap();

    set_remote_url(&repo_root, "/definitely/missing/repo.git");
    app.start_guided_creation(MonthId::parse("2026-03").unwrap())
        .unwrap();

    let draft_commit = git_capture(&repo_root, &["rev-parse", "HEAD"]);
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );
    match &app.route {
        Route::GuidedCreation(state) => {
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::SyncPending);
            assert_eq!(
                state.message.as_deref(),
                Some("Draft created; sync pending")
            );
        }
        other => panic!("expected pending guided route, got {other:?}"),
    }

    app.process_pending_sync().unwrap();

    match &app.route {
        Route::BlockingFailure(FailureState { title, retry, .. }) => {
            assert_eq!(title, "Could not sync 2026-03");
            assert!(matches!(retry, RetryTarget::GuidedPush(_)));
        }
        other => panic!("expected blocking failure, got {other:?}"),
    }
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 1
    );

    set_remote_url(&repo_root, remote.to_str().unwrap());
    let retry = match std::mem::replace(&mut app.route, Route::Shutdown) {
        Route::BlockingFailure(state) => state.retry,
        other => panic!("expected blocking failure, got {other:?}"),
    };
    app.retry_failure(retry).unwrap();

    match &app.route {
        Route::GuidedCreation(state) => {
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::Synced);
            assert_eq!(state.message.as_deref(), Some("Draft created and synced"));
        }
        other => panic!("expected guided route, got {other:?}"),
    }
    assert!(
        git_capture(&repo_root, &["ls-remote", "origin", "refs/heads/main"])
            .contains(&draft_commit)
    );

    set_remote_url(&repo_root, "/definitely/missing/repo.git");
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

    let step_commit = git_capture(&repo_root, &["rev-parse", "HEAD"]);
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 2
    );
    match &app.route {
        Route::GuidedCreation(state) => {
            assert_eq!(state.step_index, 1);
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::SyncPending);
            assert_eq!(
                state.message.as_deref(),
                Some("Draft autosaved; sync pending")
            );
        }
        other => panic!("expected pending guided route, got {other:?}"),
    }

    app.process_pending_sync().unwrap();

    match &app.route {
        Route::BlockingFailure(FailureState { title, retry, .. }) => {
            assert_eq!(title, "Could not sync 2026-03");
            assert!(matches!(retry, RetryTarget::GuidedPush(_)));
        }
        other => panic!("expected blocking failure, got {other:?}"),
    }
    assert_eq!(
        git_capture(&repo_root, &["rev-list", "--count", "HEAD"])
            .parse::<usize>()
            .unwrap(),
        before + 2
    );

    set_remote_url(&repo_root, remote.to_str().unwrap());
    let retry = match std::mem::replace(&mut app.route, Route::Shutdown) {
        Route::BlockingFailure(state) => state.retry,
        other => panic!("expected blocking failure, got {other:?}"),
    };
    app.retry_failure(retry).unwrap();

    match &app.route {
        Route::GuidedCreation(state) => {
            assert_eq!(state.persistence, PersistenceState::Clean);
            assert_eq!(state.sync, SyncState::Synced);
            assert_eq!(state.message.as_deref(), Some("Draft autosaved and synced"));
        }
        other => panic!("expected guided route, got {other:?}"),
    }
    assert!(
        git_capture(&repo_root, &["ls-remote", "origin", "refs/heads/main"]).contains(&step_commit)
    );
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::editor_commit_changed_local_autosave -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::editor_commit_changed_git_daemon_remote_autosave -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::editor_navigation_keypress_redraw -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::editor_typing_keypress_redraw -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::editor_invalid_money_submit_redraw -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::guided_typing_keypress_redraw -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::navigation_many_months_keypress_redraw -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::create_dialogue_typing_keypress_redraw -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::navigation_dialogue_validation_error_redraw -- --ignored --exact --nocapture`"]
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

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::create_dialogue_submit_large_history_first_draw_local -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::create_dialogue_submit_first_draw_git_daemon_remote -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::first_navigation_screen_large_history_app_breakdown -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::repository_gate_first_draw_large_history_app -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::repository_refresh_large_history_app -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::repository_refresh_git_daemon_remote_large_history -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::open_selected_month_large_history -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::editor_exit_large_history -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::editor_exit_after_autosave_large_history -- --ignored --exact --nocapture`"]
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

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::guided_exit_large_history -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::rename_month_large_history -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::delete_month_large_history -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::rename_delete_large_history_breakdown -- --ignored --exact --nocapture`"]
fn rename_delete_large_history_breakdown() {
    const MONTH_COUNT: usize = 600;
    const RUNS: usize = 20;

    let (temp, repo_root, repository, mut navigation) =
        seeded_many_month_navigation_app(MONTH_COUNT);
    let mut terminal = Terminal::new(TestBackend::new(105, 48)).unwrap();
    let mut rename_repository_samples = Vec::with_capacity(RUNS);
    let mut rename_update_samples = Vec::with_capacity(RUNS);
    let mut rename_draw_samples = Vec::with_capacity(RUNS);
    let mut delete_repository_samples = Vec::with_capacity(RUNS);
    let mut delete_update_samples = Vec::with_capacity(RUNS);
    let mut delete_draw_samples = Vec::with_capacity(RUNS);

    for index in 0..RUNS {
        navigation.selected = 0;
        let source = navigation.selected_month().unwrap().document.month;
        let target_year = 2070 + index / 12;
        let target_month = index % 12 + 1;
        let target = MonthId::parse(&format!("{target_year:04}-{target_month:02}")).unwrap();

        let started = Instant::now();
        let (_outcome, loaded) = repository.rename_month(source, target).unwrap();
        rename_repository_samples.push(started.elapsed());

        let started = Instant::now();
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
        rename_update_samples.push(started.elapsed());

        let route = Route::Navigation(navigation.clone());
        let started = Instant::now();
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
        rename_draw_samples.push(started.elapsed());
        black_box(terminal.backend().buffer());
    }

    for _ in 0..RUNS {
        navigation.selected = 0;
        let month = navigation.selected_month().unwrap().document.month;

        let started = Instant::now();
        let _outcome = repository.delete_month(month).unwrap();
        delete_repository_samples.push(started.elapsed());

        let started = Instant::now();
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
        delete_update_samples.push(started.elapsed());

        let route = Route::Navigation(navigation.clone());
        let started = Instant::now();
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
        delete_draw_samples.push(started.elapsed());
        black_box(terminal.backend().buffer());
    }

    black_box((temp.path(), repo_root));
    print_samples(
        "rename_delete_large_history_breakdown.rename_repository",
        &mut rename_repository_samples,
    );
    print_samples(
        "rename_delete_large_history_breakdown.rename_update",
        &mut rename_update_samples,
    );
    print_samples(
        "rename_delete_large_history_breakdown.rename_draw",
        &mut rename_draw_samples,
    );
    print_samples(
        "rename_delete_large_history_breakdown.delete_repository",
        &mut delete_repository_samples,
    );
    print_samples(
        "rename_delete_large_history_breakdown.delete_update",
        &mut delete_update_samples,
    );
    print_samples(
        "rename_delete_large_history_breakdown.delete_draw",
        &mut delete_draw_samples,
    );
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::guided_creation_breakdown_local -- --ignored --exact --nocapture`"]
fn guided_creation_breakdown_local() {
    let temp = tempfile::tempdir().unwrap();
    let repo_root = temp.path().join("budget");
    Repository::init(&repo_root, None).unwrap();
    let repository = Repository::open(&repo_root).unwrap();
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Shutdown,
    };

    measure_guided_creation_breakdown("guided_creation_breakdown_local", &mut app, 30);
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::guided_creation_breakdown_git_daemon_remote -- --ignored --exact --nocapture`"]
fn guided_creation_breakdown_git_daemon_remote() {
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
        route: Route::Shutdown,
    };

    measure_guided_creation_breakdown("guided_creation_breakdown_git_daemon_remote", &mut app, 6);
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::rename_month_git_daemon_remote_large_history -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::rename_push_failure_git_daemon_remote_large_history -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::rename_push_retry_git_daemon_remote_large_history -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::delete_month_git_daemon_remote_large_history -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::stale_rename_delete_git_daemon_remote_large_history -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::stale_rename_delete_local_large_history -- --ignored --exact --nocapture`"]
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

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::delete_push_failure_git_daemon_remote_large_history -- --ignored --exact --nocapture`"]
fn delete_push_failure_git_daemon_remote_large_history() {
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

    for _ in 0..RUNS {
        stage_delete_dialogue_for_perf(&mut app);
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
        "delete_push_failure_git_daemon_remote_large_history.feedback",
        &mut feedback_samples,
    );
    print_samples(
        "delete_push_failure_git_daemon_remote_large_history.failure",
        &mut failure_samples,
    );
    print_samples(
        "delete_push_failure_git_daemon_remote_large_history.total",
        &mut total_samples,
    );
}

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::delete_push_retry_git_daemon_remote_large_history -- --ignored --exact --nocapture`"]
fn delete_push_retry_git_daemon_remote_large_history() {
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

    for _ in 0..RUNS {
        stage_delete_dialogue_for_perf(&mut app);
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
        "delete_push_retry_git_daemon_remote_large_history",
        &mut samples,
    );
}

#[test]
fn rename_push_failure_retries_the_pending_push_boundary() {
    let (_temp, repo_root, remote, repository, navigation, source) =
        seeded_navigation_app_with_remote("2026-03");
    git(
        &repo_root,
        &[
            "config",
            "remote.origin.url",
            "/definitely/missing/repo.git",
        ],
    );

    let mut app = App {
        repo_root: repo_root.clone(),
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

    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.sync, SyncState::SyncPending);
            assert_eq!(
                state.message.as_deref(),
                Some("Renamed 2026-03 to 2026-04; sync pending")
            );
        }
        other => panic!("expected pending navigation route, got {other:?}"),
    }

    app.process_pending_sync().unwrap();

    match &app.route {
        Route::BlockingFailure(FailureState { title, retry, .. }) => {
            assert_eq!(title, "Could not sync repository changes");
            assert!(matches!(
                retry,
                RetryTarget::PushNavigation(Some(month))
                    if *month == MonthId::parse("2026-04").unwrap()
            ));
        }
        other => panic!("expected blocking failure, got {other:?}"),
    }

    assert!(!repo_root.join("months/2026-03.toml").exists());
    assert!(repo_root.join("months/2026-04.toml").exists());

    git(
        &repo_root,
        &["config", "remote.origin.url", remote.to_str().unwrap()],
    );
    app.handle_key(KeyEvent::from(KeyCode::Char('r'))).unwrap();

    match app.route {
        Route::Navigation(state) => {
            assert!(state.dialogue.is_none());
            assert_eq!(
                state.months[0].document.month,
                MonthId::parse("2026-04").unwrap()
            );
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
}

#[test]
fn rename_from_fresh_navigation_updates_cached_entry_and_fingerprint() {
    let (_temp, repo_root, repository, navigation, source) = seeded_navigation_app("2026-03");
    let target = MonthId::parse("2026-04").unwrap();
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Rename(RenameDialogue {
                source,
                input: target.key(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::Navigation(state) => {
            assert!(state.dialogue.is_none());
            assert_eq!(state.months[0].document.month, target);
            assert_eq!(state.months[0].calculated.month, target);
            assert_eq!(
                state.fingerprint.as_ref().unwrap(),
                &app.repository
                    .as_ref()
                    .unwrap()
                    .month_file_fingerprint()
                    .unwrap()
            );
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
}

#[test]
fn rename_from_stale_navigation_reloads_unrelated_month_changes() {
    let (_temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(2);
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    let source = MonthId::parse("2020-02").unwrap();
    let target = MonthId::parse("2026-04").unwrap();
    fs::write(
        repo_root.join("months/2020-01.toml"),
        month_document_for_perf("2020-01", 99),
    )
    .unwrap();
    app.route = match std::mem::replace(&mut app.route, Route::Shutdown) {
        Route::Navigation(state) => Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Rename(RenameDialogue {
                source,
                input: target.key(),
                error: None,
            })),
            ..state
        }),
        other => panic!("expected navigation route, got {other:?}"),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::Navigation(state) => {
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
        other => panic!("expected navigation route, got {other:?}"),
    }
}

#[test]
fn remote_rename_from_stale_navigation_reloads_then_syncs_pending_push() {
    let (_temp, repo_root, _remote, repository, navigation) =
        seeded_many_month_navigation_app_with_remote(2);
    let mut app = App {
        repo_root: repo_root.clone(),
        repository: Some(repository),
        route: Route::Navigation(navigation),
    };
    let source = MonthId::parse("2020-02").unwrap();
    let target = MonthId::parse("2026-04").unwrap();
    fs::write(
        repo_root.join("months/2020-01.toml"),
        month_document_for_perf("2020-01", 99),
    )
    .unwrap();
    app.route = match std::mem::replace(&mut app.route, Route::Shutdown) {
        Route::Navigation(state) => Route::Navigation(NavigationState {
            dialogue: Some(NavigationDialogue::Rename(RenameDialogue {
                source,
                input: target.key(),
                error: None,
            })),
            ..state
        }),
        other => panic!("expected navigation route, got {other:?}"),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::Navigation(state) => {
            assert_eq!(state.sync, SyncState::SyncPending);
            assert_eq!(
                state.message.as_deref(),
                Some("Renamed 2020-02 to 2026-04; sync pending")
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

#[test]
fn delete_from_fresh_navigation_updates_cached_list_and_fingerprint() {
    let (_temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(2);
    let deleted = MonthId::parse("2020-02").unwrap();
    let mut app = App {
        repo_root,
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

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::Navigation(state) => {
            assert!(state.dialogue.is_none());
            assert_eq!(state.months.len(), 1);
            assert!(
                state
                    .months
                    .iter()
                    .all(|entry| entry.document.month != deleted)
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
        other => panic!("expected navigation route, got {other:?}"),
    }
}

#[test]
fn delete_from_stale_navigation_reloads_unrelated_month_changes() {
    let (_temp, repo_root, repository, navigation) = seeded_many_month_navigation_app(2);
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
        other => panic!("expected navigation route, got {other:?}"),
    }
}

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

fn seeded_navigation_app(
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

fn seeded_navigation_app_with_remote(
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

fn seeded_many_month_navigation_app(
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

fn seeded_many_month_navigation_app_with_remote(
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

fn navigation_from_repository(repository: &Repository) -> NavigationState {
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

fn stage_create_dialogue_for_perf(app: &mut App, index: usize) {
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

fn start_guided_draft_for_perf(app: &mut App, index: usize) {
    let year = 2070 + index / 12;
    let month = index % 12 + 1;
    let month = MonthId::parse(&format!("{year:04}-{month:02}")).unwrap();
    app.start_guided_creation(month).unwrap();
    assert!(matches!(app.route, Route::GuidedCreation(_)));
}

fn make_navigation_stale_for_perf(repo_root: &Path, index: usize) {
    fs::write(
        repo_root.join("months/2020-01.toml"),
        month_document_for_perf("2020-01", 900 + index),
    )
    .unwrap();
}

fn stage_rename_dialogue_for_perf(app: &mut App, index: usize) {
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

fn stage_delete_dialogue_for_perf(app: &mut App) {
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

fn autosave_first_editor_field_for_perf(app: &mut App, index: usize) {
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

fn set_remote_url(repo_root: &Path, remote: &str) {
    git(repo_root, &["config", "remote.origin.url", remote]);
}

fn draw_for_perf(terminal: &mut Terminal<TestBackend>, app: &App) {
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
}

fn measure_pending_navigation_feedback_for_perf(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
) -> Duration {
    let started = Instant::now();
    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
    draw_for_perf(terminal, app);
    let elapsed = started.elapsed();
    match &app.route {
        Route::Navigation(state) => assert_eq!(state.sync, SyncState::SyncPending),
        other => panic!("expected pending navigation route, got {other:?}"),
    }
    black_box(terminal.backend().buffer());
    elapsed
}

fn measure_pending_navigation_sync_for_perf(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    expect_success: bool,
) -> Duration {
    let started = Instant::now();
    assert!(app.process_pending_sync().unwrap());
    draw_for_perf(terminal, app);
    let elapsed = started.elapsed();
    match (&app.route, expect_success) {
        (Route::Navigation(state), true) => assert_eq!(state.sync, SyncState::Synced),
        (Route::BlockingFailure(_), false) => {}
        (route, true) => panic!("expected synced navigation route, got {route:?}"),
        (route, false) => panic!("expected blocking failure, got {route:?}"),
    }
    black_box(terminal.backend().buffer());
    elapsed
}

fn populate_month_files(repo_root: &std::path::Path, count: usize) {
    let months = repo_root.join("months");
    for index in 0..count {
        let year = 2020 + index / 12;
        let month = index % 12 + 1;
        let month_id = format!("{year:04}-{month:02}");
        fs::write(
            months.join(format!("{month_id}.toml")),
            month_document_for_perf(&month_id, index),
        )
        .unwrap();
    }
}

fn month_document_for_perf(month: &str, index: usize) -> String {
    let current = 200_000 + index as i64 * 750;
    let savings = 50_000 + index as i64 * 500;
    let card_a = 12_000 + index as i64 * 25;
    let card_b = 7_500 + index as i64 * 15;
    let travel = 80_000 + index as i64 * 200;
    let home = 25_000 + index as i64 * 150;
    let emergency = 100_000 + index as i64 * 100;

    format!(
        r#"month = "{month}"

[accounts]
current_account = {current}
savings_account = {savings}
credit_card_a = {card_a}
credit_card_b = {card_b}

[timing_adjustments]
investment_not_yet_sent_raw = 0
previous_month_spending_correction_raw = 0

[next_month_earmarks]
subscriptions = 12000
general_spending = 32000

[savings_pots.travel_fund]
carried_over = {travel}
monthly_change = 9000

[savings_pots.home_upkeep]
carried_over = {home}
monthly_change = 5500

[savings_pots.emergency_buffer]
carried_over = {emergency}
monthly_change = 3500
"#
    )
}

fn git(root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

struct GitDaemon {
    child: Child,
}

impl Drop for GitDaemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_git_daemon(base_path: &Path) -> (GitDaemon, u16) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let child = Command::new("git")
        .args([
            "daemon",
            "--reuseaddr",
            "--export-all",
            "--enable=receive-pack",
            "--listen=127.0.0.1",
            &format!("--port={port}"),
            &format!("--base-path={}", base_path.display()),
        ])
        .arg(base_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_for_git_daemon(port);
    (GitDaemon { child }, port)
}

fn wait_for_git_daemon(port: u16) {
    let started = Instant::now();
    let timeout = Duration::from_secs(1);
    while started.elapsed() < timeout {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("git daemon did not accept connections within {timeout:?}");
}

fn git_capture(root: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {:?} failed", args);
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn app_state_from_loaded_for_perf(loaded: crate::repository::LoadedMonth) -> EditorState {
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

fn format_duration(duration: Duration) -> String {
    format!("{:.3}ms", duration.as_secs_f64() * 1000.0)
}

fn print_samples(scenario: &str, samples: &mut [Duration]) {
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

fn print_samples_if_any(scenario: &str, samples: &mut [Duration]) {
    if !samples.is_empty() {
        print_samples(scenario, samples);
    }
}

fn measure_guided_creation_breakdown(scenario: &str, app: &mut App, runs: usize) {
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

fn measure_editor_commit_changed_autosave(scenario: &str, app: &mut App, runs: usize) {
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

fn measure_editor_commit_changed_autosave_breakdown(scenario: &str, app: &mut App, runs: usize) {
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

fn guided_perf_value(step_index: usize) -> i64 {
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
