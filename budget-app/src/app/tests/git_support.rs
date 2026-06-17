use super::*;

pub(super) fn draw_for_perf(terminal: &mut Terminal<TestBackend>, app: &App) {
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

pub(super) fn measure_pending_navigation_feedback_for_perf(
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

pub(super) fn measure_pending_navigation_sync_for_perf(
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

pub(super) fn populate_month_files(repo_root: &std::path::Path, count: usize) {
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

pub(super) fn month_document_for_perf(month: &str, index: usize) -> String {
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

pub(super) fn git(root: &std::path::Path, args: &[&str]) {
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

pub(super) struct GitDaemon {
    child: Child,
}

impl Drop for GitDaemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub(super) fn start_git_daemon(base_path: &Path) -> (GitDaemon, u16) {
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

pub(super) fn wait_for_git_daemon(port: u16) {
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

pub(super) fn git_capture(root: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {:?} failed", args);
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
