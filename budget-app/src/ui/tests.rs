use budget_core::AppConfig;
use ratatui::prelude::{Color, Modifier};

use super::test_support::{
    buffer_to_string, draw_route, draw_route_with_theme, editor_route, editor_route_with_state,
    find_text, guided_route, navigation_route,
};
use super::theme::{
    Base24PaletteConfig, ThemeConfig, Tone, UiTheme, month_state_style, operational_status_style,
    parse_hex_triplet, rgb, status_value_style,
};
use crate::state::{InteractionState, NavigationState, Route};

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

#[test]
fn navigation_snapshot() {
    let config = AppConfig::default_mvp();
    let route = navigation_route(&config);

    insta::assert_snapshot!(buffer_to_string(draw_route(&route, Some(&config), 120, 40)));
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
fn editor_snapshot() {
    let config = AppConfig::default_mvp();
    let route = editor_route(&config);
    insta::assert_snapshot!(buffer_to_string(draw_route(&route, Some(&config), 120, 40)));
}

#[test]
fn editor_snapshot_80x24() {
    let config = AppConfig::default_mvp();
    let route = editor_route(&config);
    assert_snapshot_for_size("editor_snapshot_80x24", &route, &config, 80, 24);
}

#[test]
fn editor_snapshot_105x48() {
    let config = AppConfig::default_mvp();
    let route = editor_route(&config);
    assert_snapshot_for_size("editor_snapshot_105x48", &route, &config, 105, 48);
}

#[test]
fn editor_snapshot_210x48() {
    let config = AppConfig::default_mvp();
    let route = editor_route(&config);
    assert_snapshot_for_size("editor_snapshot_210x48", &route, &config, 210, 48);
}

#[test]
fn editor_render_omits_redundant_validation_and_focus_copy() {
    let config = AppConfig::default_mvp();
    let route = editor_route(&config);
    let rendered = buffer_to_string(draw_route(&route, Some(&config), 105, 48));

    assert!(!rendered.contains("validation:"));
    assert!(!rendered.contains("Focused field"));
    assert!(!rendered.contains("Previous month spending correction"));
    assert!(!rendered.contains("Previous month correction"));
    assert!(rendered.contains("General spending over/under"));
}

#[test]
fn editor_render_shows_idle_focus_marker_and_active_section_emphasis() {
    let config = AppConfig::default_mvp();
    let theme = UiTheme::project_default();
    let buffer = draw_route(&editor_route(&config), Some(&config), 105, 48);
    let rendered = buffer_to_string(buffer.clone());

    assert!(rendered.contains("› Travel fund"));

    let (marker_x, marker_y) = find_text(&buffer, "› Travel fund").unwrap();
    let marker = &buffer[(marker_x, marker_y)];
    assert_eq!(
        marker.fg,
        theme
            .selected_style()
            .fg
            .expect("selected style sets a foreground colour")
    );
    assert_eq!(
        marker.bg,
        theme
            .selected_style()
            .bg
            .expect("selected style sets a background colour")
    );
    assert!(!marker.modifier.contains(Modifier::UNDERLINED));

    let (title_x, title_y) = find_text(&buffer, "Savings Pots").unwrap();
    let title = &buffer[(title_x, title_y)];
    assert_eq!(
        title.fg,
        theme
            .emphasized_tone_style(Tone::Pots)
            .fg
            .expect("section title style sets a foreground colour")
    );
    assert!(title.modifier.contains(Modifier::BOLD));
}

#[test]
fn editor_render_distinguishes_editing_from_idle_focus() {
    let config = AppConfig::default_mvp();
    let theme = UiTheme::project_default();
    let buffer = draw_route(
        &editor_route_with_state(&config, 8, InteractionState::FieldEditing),
        Some(&config),
        105,
        48,
    );
    let rendered = buffer_to_string(buffer.clone());

    assert!(rendered.contains("✎ Travel fund"));
    assert!(rendered.contains("£800.00_"));

    let (value_x, value_y) = find_text(&buffer, "£800.00_").unwrap();
    let value = &buffer[(value_x, value_y)];
    assert_eq!(
        value.fg,
        theme
            .editing_style()
            .fg
            .expect("editing style sets a foreground colour")
    );
    assert_eq!(
        value.bg,
        theme
            .editing_style()
            .bg
            .expect("editing style sets a background colour")
    );
    assert!(value.modifier.contains(Modifier::BOLD));
    assert!(!value.modifier.contains(Modifier::UNDERLINED));

    let (title_x, title_y) = find_text(&buffer, "Savings Pots").unwrap();
    let title = &buffer[(title_x, title_y)];
    assert_eq!(
        title.fg,
        theme
            .emphasized_tone_style(Tone::Warning)
            .fg
            .expect("editing title style sets a foreground colour")
    );
    assert!(title.modifier.contains(Modifier::BOLD));
}

#[test]
fn navigation_selected_row_does_not_use_underlined_modifier() {
    let config = AppConfig::default_mvp();
    let buffer = draw_route(&navigation_route(&config), Some(&config), 105, 48);

    let (month_x, month_y) = find_text(&buffer, "August 2026").unwrap();
    let month = &buffer[(month_x, month_y)];
    assert!(!month.modifier.contains(Modifier::UNDERLINED));
}

#[test]
fn editor_render_uses_dark_neutral_section_surfaces_in_wide_layout() {
    let config = AppConfig::default_mvp();
    let theme = UiTheme::project_default();
    let buffer = draw_route(&editor_route(&config), Some(&config), 210, 48);

    for label in ["Accounts", "Timing Adjustments", "Savings Pots"] {
        let (x, y) = find_text(&buffer, label).unwrap();
        let cell = &buffer[(x, y)];
        assert_eq!(
            cell.bg,
            theme
                .toned_panel_style(match label {
                    "Accounts" => Tone::Accounts,
                    "Timing Adjustments" => Tone::Timing,
                    _ => Tone::Pots,
                })
                .bg
                .expect("panel style sets background colour")
        );
    }
}

#[test]
fn editor_render_can_use_a_project_theme_override() {
    let config = AppConfig::default_mvp();
    let theme = UiTheme::from_palette(&Base24PaletteConfig {
        base00: "#101112".to_owned(),
        base01: "#1a1c1f".to_owned(),
        base02: "#212529".to_owned(),
        base03: "#495057".to_owned(),
        base04: "#adb5bd".to_owned(),
        base05: "#dee2e6".to_owned(),
        base06: "#f8f9fa".to_owned(),
        base07: "#ffffff".to_owned(),
        base08: "#ff6b6b".to_owned(),
        base09: "#ff922b".to_owned(),
        base0_a: "#ffd43b".to_owned(),
        base0_b: "#11ee44".to_owned(),
        base0_c: "#bed6ff".to_owned(),
        base0_d: "#6c99bb".to_owned(),
        base0_e: "#d197d9".to_owned(),
        base0_f: "#692828".to_owned(),
    });

    let buffer = draw_route_with_theme(&editor_route(&config), Some(&config), 105, 48, &theme);

    let (title_x, title_y) = find_text(&buffer, "Savings Pots").unwrap();
    let title = &buffer[(title_x, title_y)];
    assert_eq!(title.fg, Color::Rgb(0x11, 0xee, 0x44));

    let (input_x, input_y) = find_text(&buffer, "£800.00").unwrap();
    let input = &buffer[(input_x, input_y)];
    assert_eq!(input.bg, Color::Rgb(0x10, 0x11, 0x12));
}

#[test]
fn bundled_project_theme_is_valid() {
    let theme = UiTheme::project_default();
    let config: ThemeConfig = toml::from_str(include_str!("../../theme.toml")).unwrap();
    assert_eq!(theme.base00, rgb(parse_hex_triplet(&config.base24.base00)));
    assert_eq!(theme.base0b, rgb(parse_hex_triplet(&config.base24.base0_b)));
}

#[test]
fn editor_render_separates_liability_cues_from_validation_failure() {
    let config = AppConfig::default_mvp();
    let theme = UiTheme::project_default();
    let buffer = draw_route(&editor_route(&config), Some(&config), 105, 48);

    let (minus_x, minus_y) = find_text(&buffer, "-£200.00").unwrap();
    let liability = &buffer[(minus_x, minus_y)];
    assert_eq!(
        liability.fg,
        theme
            .tone_style(Tone::Liability)
            .fg
            .expect("liability tone sets a foreground colour")
    );

    let (validation_x, validation_y) = find_text(&buffer, "outside tolerance").unwrap();
    let validation = &buffer[(validation_x, validation_y)];
    assert_eq!(
        validation.fg,
        status_value_style(theme, "outside tolerance")
            .fg
            .expect("validation status sets a foreground colour")
    );

    let (sync_x, sync_y) = find_text(&buffer, "synced").unwrap();
    let sync = &buffer[(sync_x, sync_y)];
    assert_eq!(
        sync.fg,
        operational_status_style(theme, "synced")
            .fg
            .expect("sync status sets a foreground colour")
    );
}

#[test]
fn editor_large_layout_keeps_section_titles_and_omits_titular_column_headers() {
    let config = AppConfig::default_mvp();
    let route = editor_route(&config);

    for (width, height) in [(105, 48), (210, 48)] {
        let rendered = buffer_to_string(draw_route(&route, Some(&config), width, height));
        assert!(rendered.contains("Accounts"));
        assert!(rendered.contains("Timing Adjustments"));
        assert!(rendered.contains("Next Month Earmarks"));
        assert!(rendered.contains("Savings Pots"));
        assert!(!rendered.contains("│Account"));
        assert!(!rendered.contains("│Adjustment"));
        assert!(!rendered.contains("│Earmark"));
        assert!(!rendered.contains("│Pot"));
    }
}

#[test]
fn guided_snapshot() {
    let config = AppConfig::default_mvp();
    let route = guided_route(&config);

    insta::assert_snapshot!(buffer_to_string(draw_route(&route, Some(&config), 120, 40)));
}

#[test]
fn guided_snapshot_80x24() {
    let config = AppConfig::default_mvp();
    let route = guided_route(&config);
    assert_snapshot_for_size("guided_snapshot_80x24", &route, &config, 80, 24);
}

#[test]
fn guided_snapshot_105x48() {
    let config = AppConfig::default_mvp();
    let route = guided_route(&config);
    assert_snapshot_for_size("guided_snapshot_105x48", &route, &config, 105, 48);
}

#[test]
fn guided_snapshot_210x48() {
    let config = AppConfig::default_mvp();
    let route = guided_route(&config);
    assert_snapshot_for_size("guided_snapshot_210x48", &route, &config, 210, 48);
}

#[test]
fn guided_render_omits_redundant_draft_status_copy_and_uses_compact_preview() {
    let config = AppConfig::default_mvp();
    let route = guided_route(&config);
    let rendered = buffer_to_string(draw_route(&route, Some(&config), 80, 24));

    assert!(!rendered.contains("Draft created and synced"));
    assert!(!rendered.contains("Draft autosaved and synced"));
    assert!(!rendered.contains("Subscriptions:"));
    assert!(!rendered.contains("Savings account:"));
    assert!(!rendered.contains("Type digits or decimals, then press Enter to autosave."));
    assert!(!rendered.contains("Validation: outside tolerance"));
    assert!(rendered.contains("Amount"));
    assert!(rendered.contains("Next: General spending"));
    assert!(rendered.contains("Earmarks £440.00  |  Diff -£620.00"));
}

#[test]
fn guided_render_gives_current_input_more_focus_than_preview_context() {
    let config = AppConfig::default_mvp();
    let theme = UiTheme::project_default();
    let route = guided_route(&config);
    let input = match &route {
        Route::GuidedCreation(state) => state.input.display_text(),
        _ => unreachable!(),
    };
    let buffer = draw_route(&route, Some(&config), 105, 48);

    let (input_x, input_y) = find_text(&buffer, &input).unwrap();
    let input_cell = &buffer[(input_x, input_y)];
    assert_eq!(
        input_cell.fg,
        theme
            .bright_style()
            .fg
            .expect("bright style sets a foreground colour")
    );
    assert_eq!(
        input_cell.bg,
        theme
            .toned_panel_style(Tone::Guided)
            .bg
            .expect("guided panel style sets a background colour")
    );

    let (preview_x, preview_y) = find_text(&buffer, "Accounts").unwrap();
    let preview = &buffer[(preview_x, preview_y)];
    assert_eq!(
        preview.bg,
        theme
            .toned_panel_style(Tone::Summary)
            .bg
            .expect("summary panel style sets a background colour")
    );
}

#[test]
fn guided_status_prioritises_validation_above_operational_metadata() {
    let config = AppConfig::default_mvp();
    let buffer = draw_route(&guided_route(&config), Some(&config), 105, 48);

    let (_, validation_y) = find_text(&buffer, "outside tolerance").unwrap();
    let (_, persistence_y) = find_text(&buffer, "Persistence").unwrap();
    assert!(validation_y < persistence_y);
}

#[test]
fn wide_layout_uses_lighter_chrome_for_guided_and_editor_views() {
    let config = AppConfig::default_mvp();
    let guided = buffer_to_string(draw_route(&guided_route(&config), Some(&config), 210, 48));
    let editor = buffer_to_string(draw_route(&editor_route(&config), Some(&config), 210, 48));

    assert!(guided.contains("Current Step"));
    assert!(guided.contains("Live Preview"));
    assert!(!guided.contains("┌Current Step"));
    assert!(!guided.contains("┌Status"));

    assert!(editor.contains("Accounts"));
    assert!(editor.contains("Validation"));
    assert!(!editor.contains("┌Accounts"));
    assert!(!editor.contains("┌Validation"));
}
