use super::*;

#[test]
fn editor_snapshot() {
    let config = AppConfig::default_mvp();
    let route = editor_route(&config);
    assert_snapshot_for_size("editor_snapshot", &route, &config, 120, 40);
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
    let config: ThemeConfig = toml::from_str(include_str!("../../../theme.toml")).unwrap();
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
