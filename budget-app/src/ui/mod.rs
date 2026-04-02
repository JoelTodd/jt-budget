//! Route-oriented rendering entrypoints for the TUI.
//!
//! Each submodule owns one slice of the visual surface, while this module keeps
//! the public render API thin and stable for the runtime and tests.

mod editor;
mod failure;
mod guided;
mod layout;
mod navigation;
mod theme;
mod widgets;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

use std::path::Path;

use budget_core::AppConfig;
use ratatui::prelude::Frame;

use crate::state::Route;
use theme::UiTheme;

/// Renders the current route using the project's default theme.
pub fn render(frame: &mut Frame<'_>, route: &Route, repo_root: &Path, config: Option<&AppConfig>) {
    let theme = UiTheme::project_default();
    render_with_theme(frame, route, repo_root, config, theme);
}

fn render_with_theme(
    frame: &mut Frame<'_>,
    route: &Route,
    repo_root: &Path,
    config: Option<&AppConfig>,
    theme: &UiTheme,
) {
    frame.render_widget(
        ratatui::widgets::Block::default().style(theme.app_style()),
        frame.area(),
    );
    match route {
        Route::Navigation(state) => navigation::render_navigation(frame, state, repo_root, theme),
        Route::GuidedCreation(state) => {
            if let Some(config) = config {
                guided::render_guided_creation(frame, state, config, theme);
            }
        }
        Route::MonthEditing(state) => {
            if config.is_some() {
                editor::render_editor(frame, state, theme);
            }
        }
        Route::BlockingFailure(state) => failure::render_failure(frame, state, theme),
        Route::Shutdown => {}
    }
}
