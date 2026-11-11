use budget_core::{AppConfig, MonthId, calculate_month};
use ratatui::prelude::{Color, Modifier};

use super::test_support::{
    buffer_to_string, draw_route, draw_route_with_theme, editor_route, editor_route_with_state,
    find_text, guided_route, navigation_route,
};
use super::theme::{
    Base24PaletteConfig, ThemeConfig, Tone, UiTheme, month_state_style, operational_status_style,
    parse_hex_triplet, rgb, status_value_style,
};
use crate::state::{
    FailureState, InteractionState, MonthEntry, NavigationState, RepositoryGateState, RetryTarget,
    Route,
};

fn assert_snapshot_for_size(
    snapshot_name: &str,
    route: &Route,
    config: &AppConfig,
    width: u16,
    height: u16,
) {
    insta::assert_snapshot!(
        snapshot_name,
        buffer_to_string(draw_route(route, Some(config), width, height))
    );
}

mod chrome;
mod defaults;
mod editor;
mod fixtures;
mod guided;
mod navigation;

use fixtures::*;
