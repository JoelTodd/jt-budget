use ratatui::prelude::{Frame, Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};

use super::layout::{PanelChrome, centered_rect};
use super::theme::{Tone, UiTheme, key_hint_spans};
use super::widgets::panel_block;
use crate::state::FailureState;

pub(super) fn render_failure(frame: &mut Frame<'_>, state: &FailureState, theme: &UiTheme) {
    let area = centered_rect(70, 40, frame.area());
    frame.render_widget(Clear, area);
    // Failures intentionally cover the active route so the user cannot keep
    // editing after a save or sync operation has become unsafe.
    let lines = vec![
        Line::from(Span::styled(
            state.title.clone(),
            theme.emphasized_tone_style(Tone::Danger),
        )),
        Line::from(""),
        Line::from(Span::styled(state.message.clone(), theme.bright_style())),
        Line::from(""),
        Line::from(key_hint_spans(theme, "r retry")),
        Line::from(key_hint_spans(theme, "q quit")),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.toned_panel_style(Tone::Danger))
            .block(panel_block(
                "Blocking Failure",
                PanelChrome::Boxed,
                Tone::Danger,
                theme,
            ))
            .wrap(Wrap { trim: false }),
        area,
    );
}
