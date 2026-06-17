use std::path::Path;

use ratatui::prelude::{Frame, Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use super::layout::{PanelChrome, centred_rect};
use super::theme::{Tone, UiTheme};
use super::widgets::{abbreviate_path, panel_block};
use crate::state::RepositoryGateState;

pub(super) fn render_repository_gate(
    frame: &mut Frame<'_>,
    state: &RepositoryGateState,
    repo_root: &Path,
    theme: &UiTheme,
) {
    let area = centred_rect(70, 34, frame.area());
    let repo = abbreviate_path(repo_root, area.width.saturating_sub(6) as usize);
    let lines = vec![
        Line::from(Span::styled(state.message.clone(), theme.bright_style())),
        Line::from(""),
        Line::from(vec![
            Span::styled("Repo: ", theme.muted_style()),
            Span::styled(repo, theme.subtle_style()),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.toned_panel_style(Tone::Status))
            .block(panel_block(
                "Repository Gate",
                PanelChrome::Boxed,
                Tone::Status,
                theme,
            ))
            .wrap(Wrap { trim: false }),
        area,
    );
}
