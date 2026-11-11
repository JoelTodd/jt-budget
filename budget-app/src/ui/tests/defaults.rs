use super::*;
use crate::state::{DefaultsState, MoneyInput};

fn defaults_route(config: &AppConfig) -> Route {
    Route::DefaultsEditing(DefaultsState::new(
        config.clone(),
        NavigationState::new(vec![]),
    ))
}

#[test]
fn defaults_snapshots_at_supported_sizes() {
    let config = AppConfig::default_mvp();
    let route = defaults_route(&config);
    for (name, width, height) in [
        ("defaults_80x24", 80, 24),
        ("defaults_105x48", 105, 48),
        ("defaults_210x48", 210, 48),
    ] {
        assert_snapshot_for_size(name, &route, &config, width, height);
    }
}

#[test]
fn defaults_editing_snapshot() {
    let config = AppConfig::default_mvp();
    let mut state = DefaultsState::new(config.clone(), NavigationState::new(vec![]));
    state.focus_index = 3;
    state.edit_buffer = Some(MoneyInput::from_value(
        budget_core::Money::from_minor(10_500),
        false,
    ));
    assert_snapshot_for_size(
        "defaults_editing_80x24",
        &Route::DefaultsEditing(state),
        &config,
        80,
        24,
    );
}

#[test]
fn defaults_scrolls_to_selected_row_in_compact_terminal() {
    let mut config = AppConfig::default_mvp();
    for index in 0..40 {
        let mut pot = config.savings_pots[0].clone();
        pot.id = format!("pot_{index}");
        pot.label = format!("Extra savings {index}");
        config.savings_pots.push(pot);
    }
    let mut state = DefaultsState::new(config.clone(), NavigationState::new(vec![]));
    state.focus_index = state.fields.len() - 1;
    let rendered = buffer_to_string(draw_route(
        &Route::DefaultsEditing(state),
        Some(&config),
        80,
        24,
    ));
    assert!(rendered.contains("General spending"));
    assert!(rendered.contains("£320.00"));
    assert!(rendered.contains("Esc months"));
}
