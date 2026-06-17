use super::*;

#[test]
fn guided_snapshot() {
    let config = AppConfig::default_mvp();
    let route = guided_route(&config);
    assert_snapshot_for_size("guided_snapshot", &route, &config, 120, 40);
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
