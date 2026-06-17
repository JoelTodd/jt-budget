use super::*;

#[test]
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::guided_perf::guided_creation_breakdown_local -- --ignored --exact --nocapture`"]
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
#[ignore = "performance harness; run with `cargo test -p jt-budget --release app::tests::guided_perf::guided_creation_breakdown_git_daemon_remote -- --ignored --exact --nocapture`"]
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
