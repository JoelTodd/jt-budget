use super::*;

#[test]
fn navigation_snapshot() {
    let config = AppConfig::default_mvp();
    let route = navigation_route(&config);
    assert_snapshot_for_size("navigation_snapshot", &route, &config, 120, 40);
}

#[test]
fn navigation_snapshot_80x24() {
    let config = AppConfig::default_mvp();
    let route = navigation_route(&config);
    assert_snapshot_for_size("navigation_snapshot_80x24", &route, &config, 80, 24);
}

#[test]
fn navigation_snapshot_105x48() {
    let config = AppConfig::default_mvp();
    let route = navigation_route(&config);
    assert_snapshot_for_size("navigation_snapshot_105x48", &route, &config, 105, 48);
}

#[test]
fn navigation_snapshot_210x48() {
    let config = AppConfig::default_mvp();
    let route = navigation_route(&config);
    assert_snapshot_for_size("navigation_snapshot_210x48", &route, &config, 210, 48);
}

#[test]
fn repository_gate_render_shows_sync_context() {
    let config = AppConfig::default_mvp();
    let route = repository_gate_route();
    let rendered = buffer_to_string(draw_route(&route, Some(&config), 80, 24));

    assert!(rendered.contains("Repository Gate"));
    assert!(rendered.contains("Checking repository and syncing latest changes"));
    assert!(rendered.contains("Repo:"));
}

#[test]
fn repository_gate_snapshot_80x24() {
    let config = AppConfig::default_mvp();
    let route = repository_gate_route();
    assert_snapshot_for_size("repository_gate_snapshot_80x24", &route, &config, 80, 24);
}

#[test]
fn repository_gate_snapshot_105x48() {
    let config = AppConfig::default_mvp();
    let route = repository_gate_route();
    assert_snapshot_for_size("repository_gate_snapshot_105x48", &route, &config, 105, 48);
}

#[test]
fn repository_gate_snapshot_210x48() {
    let config = AppConfig::default_mvp();
    let route = repository_gate_route();
    assert_snapshot_for_size("repository_gate_snapshot_210x48", &route, &config, 210, 48);
}

#[test]
fn blocking_failure_snapshot_80x24() {
    let config = AppConfig::default_mvp();
    let route = blocking_failure_route();
    assert_snapshot_for_size("blocking_failure_snapshot_80x24", &route, &config, 80, 24);
}

#[test]
fn blocking_failure_snapshot_105x48() {
    let config = AppConfig::default_mvp();
    let route = blocking_failure_route();
    assert_snapshot_for_size("blocking_failure_snapshot_105x48", &route, &config, 105, 48);
}

#[test]
fn blocking_failure_snapshot_210x48() {
    let config = AppConfig::default_mvp();
    let route = blocking_failure_route();
    assert_snapshot_for_size("blocking_failure_snapshot_210x48", &route, &config, 210, 48);
}

#[test]
fn navigation_render_omits_redundant_month_action_helper_copy() {
    let config = AppConfig::default_mvp();
    let route = Route::Navigation(NavigationState::new(Vec::new()));
    let rendered = buffer_to_string(draw_route(&route, Some(&config), 105, 48));
    assert!(!rendered.contains("Month actions are explicit"));
}

#[test]
fn navigation_render_uses_compact_summary_and_short_updated_text() {
    let config = AppConfig::default_mvp();
    let route = navigation_route(&config);
    let rendered = buffer_to_string(draw_route(&route, Some(&config), 105, 48));

    assert!(rendered.contains("2026-03-31 | 14:30"));
    assert!(!rendered.contains("14:30:06"));
    assert!(!rendered.contains("Current account:"));
    assert!(rendered.contains("Accounts"));
    assert!(rendered.contains("Timing Adjustments"));
    assert!(rendered.contains("Next Month Earmarks"));
    assert!(rendered.contains("Savings Pots"));
    assert!(rendered.contains("Total allocated"));
    assert!(rendered.contains("Overall difference"));
    assert!(rendered.contains("Status invalid"));
    assert!(!rendered.contains("Months"));
    assert!(!rendered.contains("│Month"));
}

#[test]
fn navigation_render_uses_semantic_status_and_difference_colours() {
    let config = AppConfig::default_mvp();
    let theme = UiTheme::project_default();
    let mut state = match navigation_route(&config) {
        Route::Navigation(state) => state,
        _ => unreachable!(),
    };
    state.months.push(state.months[0].clone());
    state.selected = 1;
    let route = Route::Navigation(state);
    let buffer = draw_route(&route, Some(&config), 105, 48);

    let (status_x, status_y) = find_text(&buffer, "draft").unwrap();
    let status = &buffer[(status_x, status_y)];
    assert_eq!(
        status.fg,
        month_state_style(theme, false)
            .fg
            .expect("draft state sets a foreground colour")
    );

    let (diff_x, diff_y) = find_text(&buffer, "£1340.00").unwrap();
    let diff = &buffer[(diff_x, diff_y)];
    assert_eq!(
        diff.fg,
        theme
            .emphasized_tone_style(Tone::Danger)
            .fg
            .expect("difference tone sets a foreground colour")
    );
}

#[test]
fn navigation_render_keeps_far_selected_month_visible_in_long_history() {
    let config = AppConfig::default_mvp();
    let template = match navigation_route(&config) {
        Route::Navigation(state) => state.months[0].document.clone(),
        _ => unreachable!(),
    };
    let months = (0..120)
        .map(|index| {
            let year = 2020 + index / 12;
            let month = index % 12 + 1;
            let mut document = template.clone();
            document.month = MonthId::new(year, month as u8).unwrap();
            let calculated = calculate_month(&config, &document).unwrap();
            MonthEntry {
                document,
                calculated,
            }
        })
        .collect();
    let mut state = NavigationState::new(months);
    state.selected = 119;
    let route = Route::Navigation(state);
    let rendered = buffer_to_string(draw_route(&route, Some(&config), 80, 24));

    assert!(rendered.contains("December 2029"));
    assert!(!rendered.contains("January 2020"));
}
