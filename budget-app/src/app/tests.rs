use std::fs;
use std::path::PathBuf;
use std::process::Command;

use budget_core::{AppConfig, MonthDocument, MonthId, calculate_month};
use crossterm::event::{KeyCode, KeyEvent};
use tempfile::TempDir;

use super::App;
use crate::repository::Repository;
use crate::state::{
    CreateDialog, EditorState, FailureState, FieldId, InteractionState, MonthEntry,
    NavigationDialog, NavigationState, PersistenceState, RenameDialog, RetryTarget, Route,
    SyncState,
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
fn rename_dialog_keeps_unchanged_month_error_inline() {
    let (_temp, repo_root, repository, navigation, source) = seeded_navigation_app("2026-03");
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialog: Some(NavigationDialog::Rename(RenameDialog {
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
            match state.dialog {
                Some(NavigationDialog::Rename(dialog)) => {
                    assert_eq!(dialog.source, source);
                    assert_eq!(dialog.input, "2026-03");
                    assert_eq!(
                        dialog.error.as_deref(),
                        Some("Month is already named 2026-03")
                    );
                }
                other => panic!("expected rename dialog, got {other:?}"),
            }
        }
        other => panic!("expected navigation route, got {other:?}"),
    }
}

#[test]
fn rename_dialog_preserves_input_after_validation_error_and_allows_retry() {
    let (_temp, repo_root, repository, navigation, source) = seeded_navigation_app("2026-03");
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialog: Some(NavigationDialog::Rename(RenameDialog {
                source,
                input: "2026-13".to_owned(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
    match &app.route {
        Route::Navigation(state) => match &state.dialog {
            Some(NavigationDialog::Rename(dialog)) => {
                assert_eq!(dialog.input, "2026-13");
                assert_eq!(
                    dialog.error.as_deref(),
                    Some("invalid month id `2026-13`, expected YYYY-MM")
                );
            }
            other => panic!("expected rename dialog, got {other:?}"),
        },
        other => panic!("expected navigation route, got {other:?}"),
    }

    app.handle_key(KeyEvent::from(KeyCode::Backspace)).unwrap();
    app.handle_key(KeyEvent::from(KeyCode::Char('2'))).unwrap();

    match &app.route {
        Route::Navigation(state) => match &state.dialog {
            Some(NavigationDialog::Rename(dialog)) => {
                assert_eq!(dialog.input, "2026-12");
                assert_eq!(dialog.error, None);
            }
            other => panic!("expected rename dialog, got {other:?}"),
        },
        other => panic!("expected navigation route, got {other:?}"),
    }

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match app.route {
        Route::Navigation(state) => {
            assert!(state.dialog.is_none());
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
fn rename_dialog_uses_blocking_failure_for_repository_faults() {
    let (_temp, repo_root, repository, navigation, source) = seeded_navigation_app("2026-03");
    fs::remove_file(repo_root.join("months/2026-03.toml")).unwrap();

    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialog: Some(NavigationDialog::Rename(RenameDialog {
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
fn create_dialog_routes_duplicate_month_to_blocking_failure() {
    let (_temp, repo_root, repository, navigation, month) = seeded_navigation_app("2026-03");
    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialog: Some(NavigationDialog::Create(CreateDialog {
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
fn create_dialog_routes_repository_faults_to_blocking_failure() {
    let (_temp, repo_root, repository, navigation, _) = seeded_navigation_app("2026-03");
    fs::remove_dir_all(repo_root.join("months")).unwrap();

    let mut app = App {
        repo_root,
        repository: Some(repository),
        route: Route::Navigation(NavigationState {
            dialog: Some(NavigationDialog::Create(CreateDialog {
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
            dialog: Some(NavigationDialog::Rename(RenameDialog {
                source,
                input: "2026-04".to_owned(),
                error: None,
            })),
            ..navigation
        }),
    };

    app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

    match &app.route {
        Route::BlockingFailure(FailureState {
            title,
            message,
            retry,
        }) => {
            assert_eq!(title, "Could not sync rename 2026-03 -> 2026-04");
            assert!(message.contains("Renamed 2026-03 to 2026-04 locally and committed it"));
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
            assert!(state.dialog.is_none());
            assert_eq!(
                state.months[0].document.month,
                MonthId::parse("2026-04").unwrap()
            );
        }
        other => panic!("expected navigation route, got {other:?}"),
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
    let navigation = NavigationState::new(
        repository
            .list_months()
            .unwrap()
            .into_iter()
            .map(|loaded| MonthEntry {
                document: loaded.document,
                calculated: loaded.calculated,
            })
            .collect(),
    );

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
    let navigation = NavigationState::new(
        repository
            .list_months()
            .unwrap()
            .into_iter()
            .map(|loaded| MonthEntry {
                document: loaded.document,
                calculated: loaded.calculated,
            })
            .collect(),
    );

    (temp, repo_root, remote, repository, navigation, month)
}

fn git(root: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success(), "git {:?} failed", args);
}
