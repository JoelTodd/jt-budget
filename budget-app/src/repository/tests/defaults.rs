use super::*;

#[test]
fn defaults_save_preserves_unknown_config_and_unrelated_staged_files() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("budget");
    Repository::init(&root, None).unwrap();
    let path = root.join("config.toml");
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("\n[custom]\nkeep = 'future setting'\n");
    fs::write(&path, text).unwrap();
    git(&root, &["add", "config.toml"]);
    git(&root, &["commit", "-m", "Extra config"]);
    fs::write(root.join("notes.txt"), "unrelated").unwrap();
    git(&root, &["add", "notes.txt"]);
    let mut repository = Repository::open(&root).unwrap();
    let mut config = repository.config().clone();
    config.next_month_earmarks[0].default_amount_minor = 10_500;
    config.savings_pots[0].default_monthly_change_minor = 0;
    repository.save_monthly_defaults(&config).unwrap();
    let saved: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["custom"]["keep"].as_str(), Some("future setting"));
    assert_eq!(repository.config(), &config);
    assert_eq!(
        git_capture(&root, &["show", "--pretty=", "--name-only", "HEAD"]),
        "config.toml"
    );
    assert!(git_capture(&root, &["diff", "--cached", "--name-only"]).contains("notes.txt"));
}

#[test]
fn defaults_save_rejects_external_edits_structure_changes_and_negative_defaults() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("budget");
    Repository::init(&root, None).unwrap();
    let mut repository = Repository::open(&root).unwrap();
    let original = fs::read_to_string(root.join("config.toml")).unwrap();
    let mut config = repository.config().clone();
    config.accounts[0].label = "Changed structure".to_owned();
    assert!(repository.save_monthly_defaults(&config).is_err());
    config = repository.config().clone();
    config.savings_pots[0].default_monthly_change_minor = -1;
    assert!(repository.save_monthly_defaults(&config).is_err());
    config = repository.config().clone();
    config.next_month_earmarks[0].default_amount_minor = -1;
    assert!(repository.save_monthly_defaults(&config).is_err());
    assert_eq!(
        fs::read_to_string(root.join("config.toml")).unwrap(),
        original
    );

    let mut external = repository.config().clone();
    external.next_month_earmarks[0].default_amount_minor = 999;
    let external_text = toml::to_string_pretty(&external).unwrap();
    fs::write(root.join("config.toml"), &external_text).unwrap();
    let error = repository
        .save_monthly_defaults(&repository.config().clone())
        .unwrap_err();
    assert!(error.to_string().contains("changed outside the app"));
    assert_eq!(
        fs::read_to_string(root.join("config.toml")).unwrap(),
        external_text
    );
}

#[test]
fn startup_uses_defaults_received_from_remote() {
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
    let first = temp.path().join("first");
    Repository::init(&first, Some(remote.to_str().unwrap())).unwrap();
    let second = temp.path().join("second");
    Repository::clone_from_remote(remote.to_str().unwrap(), &second).unwrap();
    let mut repository = Repository::open(&first).unwrap();
    let mut config = repository.config().clone();
    config.next_month_earmarks[0].default_amount_minor = 10_500;
    config.savings_pots[0].default_monthly_change_minor = 18_500;
    repository.save_monthly_defaults(&config).unwrap();
    let reopened = Repository::open(&second).unwrap();
    assert_eq!(reopened.config(), &config);
    let draft = reopened
        .create_month_draft(MonthId::parse("2026-05").unwrap())
        .unwrap();
    assert_eq!(draft.savings_pots["travel_fund"].monthly_change, 18_500);
    assert_eq!(draft.next_month_earmarks["subscriptions"], 10_500);
}
