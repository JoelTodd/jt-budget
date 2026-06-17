use super::*;

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
