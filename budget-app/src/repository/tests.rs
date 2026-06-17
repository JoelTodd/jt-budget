use std::fs;
use std::path::Path;
use std::process::Command;

use budget_core::{AppConfig, MonthId, calculate_month};
use tempfile::tempdir;

use super::{Repository, SyncOutcome};

#[test]
fn init_repo_and_open_repo_gate() {
    let temp = tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);

    let repo = temp.path().join("budget");
    Repository::init(&repo, Some(remote.to_str().unwrap())).unwrap();
    assert_eq!(
        git_capture(
            &repo,
            &[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}"
            ],
        ),
        "origin/main"
    );
    assert_eq!(
        git_capture(&repo, &["config", "--get", "branch.main.remote"]),
        "origin"
    );
    assert_eq!(
        git_capture(&repo, &["config", "--get", "branch.main.merge"]),
        "refs/heads/main"
    );

    let opened = Repository::open(&repo).unwrap();
    assert!(opened.sync_enabled());
    assert!(opened.root().join("config.toml").exists());
    assert!(opened.list_months().unwrap().is_empty());
}

#[test]
fn open_repo_gate_blocks_when_origin_branch_has_no_upstream() {
    let temp = tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);

    let repo = temp.path().join("budget");
    Repository::init(&repo, Some(remote.to_str().unwrap())).unwrap();
    git(&repo, &["config", "--unset", "branch.main.remote"]);
    git(&repo, &["config", "--unset", "branch.main.merge"]);

    let error = Repository::open(&repo).unwrap_err();

    assert!(error.to_string().contains("pulling latest budget data"));
}

#[test]
fn init_repo_without_remote_opens_repo_gate() {
    let temp = tempdir().unwrap();
    let repo = temp.path().join("budget");
    Repository::init(&repo, None).unwrap();

    let config_text = fs::read_to_string(repo.join("config.toml")).unwrap();
    assert!(!config_text.contains("[ui.base24]"));

    let opened = Repository::open(&repo).unwrap();
    assert!(!opened.sync_enabled());
    assert!(opened.root().join("config.toml").exists());
    assert!(opened.list_months().unwrap().is_empty());
}

#[test]
fn open_rejects_budget_shaped_folder_that_is_not_git_repo() {
    let temp = tempdir().unwrap();
    let repo = temp.path().join("budget");
    fs::create_dir_all(repo.join("months")).unwrap();
    fs::create_dir_all(repo.join("meta")).unwrap();
    fs::write(
        repo.join("config.toml"),
        toml::to_string_pretty(&AppConfig::default_mvp()).unwrap(),
    )
    .unwrap();

    let error = Repository::open(&repo).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("git remote get-url origin failed")
    );
}

#[test]
fn init_repo_persists_explicit_initial_config() {
    let temp = tempdir().unwrap();
    let repo = temp.path().join("budget");
    let mut config = AppConfig::default_mvp();
    config.accounts[0].label = "Main balance".to_owned();
    config.savings_pots[0].label = "Trip".to_owned();
    config.savings_pots[0].default_monthly_change_minor = 4_200;
    config.next_month_earmarks[0].label = "Bills".to_owned();
    config.next_month_earmarks[0].default_amount_minor = 8_500;

    Repository::init_with_config(&repo, None, &config).unwrap();

    let opened = Repository::open(&repo).unwrap();
    assert_eq!(opened.config(), &config);
}

#[test]
fn clone_from_remote_opens_repo_gate() {
    let temp = tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    git(
        temp.path(),
        &[
            "init",
            "--bare",
            "--initial-branch=main",
            remote.to_str().unwrap(),
        ],
    );

    let source = temp.path().join("source");
    Repository::init(&source, Some(remote.to_str().unwrap())).unwrap();

    let clone = temp.path().join("clone");
    Repository::clone_from_remote(remote.to_str().unwrap(), &clone).unwrap();

    let opened = Repository::open(&clone).unwrap();
    assert!(opened.sync_enabled());
    assert!(opened.root().join("config.toml").exists());
}

#[test]
fn clone_from_remote_rejects_non_empty_target() {
    let temp = tempdir().unwrap();
    let target = temp.path().join("clone");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("note.txt"), "occupied").unwrap();

    let error = Repository::clone_from_remote("https://github.com/example/example.git", &target)
        .unwrap_err();
    assert!(
        error.to_string().contains("repository path")
            && error.to_string().contains("must be empty or not exist")
    );
}

#[test]
fn save_month_writes_derived_cache_and_pushes() {
    let temp = tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);
    let repo_path = temp.path().join("budget");
    Repository::init(&repo_path, Some(remote.to_str().unwrap())).unwrap();

    let repository = Repository::open(&repo_path).unwrap();
    let mut month = repository
        .create_month_draft(MonthId::parse("2026-03").unwrap())
        .unwrap();
    month.accounts.insert("current_account".to_owned(), 100_000);
    month.accounts.insert("savings_account".to_owned(), 20_000);
    month.accounts.insert("credit_card_a".to_owned(), 5_000);
    month.accounts.insert("credit_card_b".to_owned(), 0);
    month
        .next_month_earmarks
        .insert("subscriptions".to_owned(), 10_000);
    month
        .next_month_earmarks
        .insert("general_spending".to_owned(), 32_000);
    repository.save_month(&mut month).unwrap();

    let opened = repository
        .load_month_by_id(MonthId::parse("2026-03").unwrap())
        .unwrap();
    let recalculated = calculate_month(repository.config(), &opened.document).unwrap();
    assert_eq!(
        opened.calculated.validation.overall_difference,
        recalculated.validation.overall_difference
    );

    let log = git_capture(&repo_path, &["log", "--oneline", "--max-count", "1"]);
    assert!(log.contains("Update budget month 2026-03"));
    let remote_log = git_capture_bare(
        &remote,
        &["log", "--oneline", "--max-count", "1", "refs/heads/main"],
    );
    assert!(remote_log.contains("Update budget month 2026-03"));

    let file_text = fs::read_to_string(repo_path.join("months/2026-03.toml")).unwrap();
    assert!(file_text.contains("[derived]"));
}

#[test]
fn save_month_without_remote_commits_locally() {
    let temp = tempdir().unwrap();
    let repo_path = temp.path().join("budget");
    Repository::init(&repo_path, None).unwrap();

    let repository = Repository::open(&repo_path).unwrap();
    let mut month = repository
        .create_month_draft(MonthId::parse("2026-03").unwrap())
        .unwrap();
    month.accounts.insert("current_account".to_owned(), 100_000);
    month.accounts.insert("savings_account".to_owned(), 20_000);
    month.accounts.insert("credit_card_a".to_owned(), 5_000);
    month.accounts.insert("credit_card_b".to_owned(), 0);
    month
        .next_month_earmarks
        .insert("subscriptions".to_owned(), 10_000);
    month
        .next_month_earmarks
        .insert("general_spending".to_owned(), 32_000);
    repository.save_month(&mut month).unwrap();

    let log = git_capture(&repo_path, &["log", "--oneline", "--max-count", "1"]);
    assert!(log.contains("Update budget month 2026-03"));
    assert!(repo_path.join("months/2026-03.toml").exists());
}

#[test]
fn save_existing_untracked_month_falls_back_to_staging() {
    let temp = tempdir().unwrap();
    let repo_path = temp.path().join("budget");
    Repository::init(&repo_path, None).unwrap();

    let repository = Repository::open(&repo_path).unwrap();
    let mut month = repository
        .create_month_draft(MonthId::parse("2026-03").unwrap())
        .unwrap();
    fs::write(
        repo_path.join("months/2026-03.toml"),
        month.to_pretty_toml(repository.config()).unwrap(),
    )
    .unwrap();
    month.accounts.insert("current_account".to_owned(), 123_456);

    repository.save_month(&mut month).unwrap();

    let log = git_capture(&repo_path, &["log", "--oneline", "--max-count", "1"]);
    assert!(log.contains("Update budget month 2026-03"));
    let changed_files = git_capture(&repo_path, &["show", "--name-only", "--format="]);
    assert!(changed_files.contains("months/2026-03.toml"));
}

#[test]
fn rename_month_updates_persisted_identifier() {
    let temp = tempdir().unwrap();
    let repo_path = temp.path().join("budget");
    Repository::init(&repo_path, None).unwrap();

    let repository = Repository::open(&repo_path).unwrap();
    let mut month = repository
        .create_month_draft(MonthId::parse("2026-03").unwrap())
        .unwrap();
    repository.save_month(&mut month).unwrap();
    let (outcome, renamed) = repository
        .rename_month(
            MonthId::parse("2026-03").unwrap(),
            MonthId::parse("2026-04").unwrap(),
        )
        .unwrap();
    assert_eq!(outcome, SyncOutcome::Synced);
    assert_eq!(renamed.document.month, MonthId::parse("2026-04").unwrap());

    assert!(!repo_path.join("months/2026-03.toml").exists());
    let renamed_path = repo_path.join("months/2026-04.toml");
    assert!(renamed_path.exists());
    let renamed_text = fs::read_to_string(&renamed_path).unwrap();
    assert!(renamed_text.contains("month = \"2026-04\""));
    let log = git_capture(&repo_path, &["log", "--oneline", "--max-count", "1"]);
    assert!(log.contains("Rename budget month 2026-03 to 2026-04"));
}

#[test]
fn delete_month_removes_file_and_commits() {
    let temp = tempdir().unwrap();
    let repo_path = temp.path().join("budget");
    Repository::init(&repo_path, None).unwrap();

    let repository = Repository::open(&repo_path).unwrap();
    let mut month = repository
        .create_month_draft(MonthId::parse("2026-03").unwrap())
        .unwrap();
    repository.save_month(&mut month).unwrap();
    let outcome = repository
        .delete_month(MonthId::parse("2026-03").unwrap())
        .unwrap();
    assert_eq!(outcome, SyncOutcome::Synced);

    assert!(!repo_path.join("months/2026-03.toml").exists());
    assert!(repository.list_months().unwrap().is_empty());
    let log = git_capture(&repo_path, &["log", "--oneline", "--max-count", "1"]);
    assert!(log.contains("Delete budget month 2026-03"));
}

#[test]
fn rename_month_keeps_local_commit_when_push_fails() {
    let temp = tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);

    let repo_path = temp.path().join("budget");
    Repository::init(&repo_path, Some(remote.to_str().unwrap())).unwrap();

    let repository = Repository::open(&repo_path).unwrap();
    let mut month = repository
        .create_month_draft(MonthId::parse("2026-03").unwrap())
        .unwrap();
    repository.save_month(&mut month).unwrap();

    git(
        &repo_path,
        &[
            "config",
            "remote.origin.url",
            "/definitely/missing/repo.git",
        ],
    );

    let (outcome, renamed) = repository
        .rename_month(
            MonthId::parse("2026-03").unwrap(),
            MonthId::parse("2026-04").unwrap(),
        )
        .unwrap();
    assert_eq!(renamed.document.month, MonthId::parse("2026-04").unwrap());

    match outcome {
        SyncOutcome::PushFailed(message) => {
            assert!(message.contains("pushing repository changes"));
        }
        other => panic!("expected push failure, got {other:?}"),
    }

    assert!(!repo_path.join("months/2026-03.toml").exists());
    assert!(repo_path.join("months/2026-04.toml").exists());
    let log = git_capture(&repo_path, &["log", "--oneline", "--max-count", "1"]);
    assert!(log.contains("Rename budget month 2026-03 to 2026-04"));
}

#[test]
fn delete_month_keeps_local_commit_when_push_fails() {
    let temp = tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);

    let repo_path = temp.path().join("budget");
    Repository::init(&repo_path, Some(remote.to_str().unwrap())).unwrap();

    let repository = Repository::open(&repo_path).unwrap();
    let mut month = repository
        .create_month_draft(MonthId::parse("2026-03").unwrap())
        .unwrap();
    repository.save_month(&mut month).unwrap();

    git(
        &repo_path,
        &[
            "config",
            "remote.origin.url",
            "/definitely/missing/repo.git",
        ],
    );

    let outcome = repository
        .delete_month(MonthId::parse("2026-03").unwrap())
        .unwrap();

    match outcome {
        SyncOutcome::PushFailed(message) => {
            assert!(message.contains("pushing repository changes"));
        }
        other => panic!("expected push failure, got {other:?}"),
    }

    assert!(!repo_path.join("months/2026-03.toml").exists());
    let log = git_capture(&repo_path, &["log", "--oneline", "--max-count", "1"]);
    assert!(log.contains("Delete budget month 2026-03"));
}

#[test]
fn connect_remote_publishes_existing_local_repo() {
    let temp = tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);

    let repo_path = temp.path().join("budget");
    Repository::init(&repo_path, None).unwrap();

    Repository::connect_remote(&repo_path, remote.to_str().unwrap()).unwrap();

    assert!(Repository::has_origin_remote(&repo_path).unwrap());
    assert_eq!(
        git_capture(
            &repo_path,
            &[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}"
            ],
        ),
        "origin/main"
    );
}

#[test]
fn looks_like_budget_repo_accepts_initialised_repository() {
    let temp = tempdir().unwrap();
    let repo_path = temp.path().join("budget");
    Repository::init(&repo_path, None).unwrap();

    assert!(Repository::looks_like_budget_repo(&repo_path).unwrap());
}

fn git(root: &Path, args: &[&str]) {
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

fn git_capture(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {:?} failed", args);
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn git_capture_bare(git_dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(git_dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git --git-dir {:?} {:?} failed",
        git_dir,
        args
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}
