use std::ops::Range;
use std::path::Path;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Frame, Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap};

use super::layout::{PanelChrome, centred_rect};
use super::theme::{Tone, UiTheme, month_state_style, validation_tone};
use super::widgets::{
    abbreviate_path, amount_cell_with_style, compact_summary_text, format_updated_timestamp,
    hint_lines, panel_block, status_line,
};
use crate::state::{DeleteDialogue, NavigationDialogue, NavigationState};

pub(super) fn render_navigation(
    frame: &mut Frame<'_>,
    state: &NavigationState,
    repo_root: &Path,
    theme: &UiTheme,
) {
    let compact = frame.area().width < 110 || frame.area().height < 28;
    let content_area = render_navigation_header(frame, state, repo_root, compact, theme);
    render_navigation_body(frame, state, content_area, compact, theme);

    if let Some(dialogue) = &state.dialogue {
        render_navigation_dialogue(frame, dialogue, theme);
    }
}

fn render_navigation_header(
    frame: &mut Frame<'_>,
    state: &NavigationState,
    repo_root: &Path,
    compact: bool,
    theme: &UiTheme,
) -> Rect {
    let mut header_lines = vec![Line::from(vec![
        Span::styled("Repo: ", theme.muted_style()),
        Span::styled(
            abbreviate_path(repo_root, frame.area().width.saturating_sub(8) as usize),
            theme.bright_style(),
        ),
    ])];
    header_lines.extend(hint_lines(
        frame.area().width,
        if compact {
            &[
                "Enter open",
                "n new",
                "s defaults",
                "m rename",
                "d delete",
                "q quit",
            ]
        } else {
            &[
                "Enter open",
                "n new",
                "s defaults",
                "m rename",
                "d delete",
                "r refresh",
                "q quit",
            ]
        },
        theme,
    ));
    if let Some(message) = &state.message {
        header_lines.push(Line::from(Span::styled(
            message.clone(),
            theme.bright_style(),
        )));
    }
    if state.persistence != crate::state::PersistenceState::Clean
        || state.sync != crate::state::SyncState::Synced
    {
        header_lines.push(status_line(state.persistence, state.sync, theme));
    }
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_lines.len() as u16 + 2),
            Constraint::Min(8),
        ])
        .split(frame.area());

    let header = Paragraph::new(header_lines)
        .style(theme.toned_panel_style(Tone::Navigation))
        .block(panel_block(
            "Navigation",
            PanelChrome::Boxed,
            Tone::Navigation,
            theme,
        ));
    frame.render_widget(header, layout[0]);

    layout[1]
}

fn render_navigation_body(
    frame: &mut Frame<'_>,
    state: &NavigationState,
    area: Rect,
    compact: bool,
    theme: &UiTheme,
) {
    // Compact terminals stack the table over the summary so both remain usable
    // at 80x24 without horizontal truncation.
    let body = if compact {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area)
    };
    render_month_table(frame, body[0], state, theme);
    render_navigation_summary(frame, body[1], state, theme);
}

fn render_month_table(frame: &mut Frame<'_>, area: Rect, state: &NavigationState, theme: &UiTheme) {
    let visible_months = visible_month_window(
        state.months.len(),
        state.selected,
        area.height.saturating_sub(3) as usize,
    );
    let visible_start = visible_months.start;
    let rows = state.months[visible_months].iter().map(|entry| {
        let status = if entry.calculated.validation.is_valid {
            "finalised"
        } else {
            "draft"
        };
        Row::new(vec![
            Cell::from(Span::styled(
                entry.document.month.display_label(),
                theme.bright_style(),
            )),
            Cell::from(Span::styled(
                status.to_owned(),
                month_state_style(theme, entry.calculated.validation.is_valid),
            )),
            amount_cell_with_style(
                entry.calculated.validation.overall_difference.format(),
                theme.emphasized_tone_style(validation_tone(entry.calculated.validation.is_valid)),
            ),
            Cell::from(Span::styled(
                format_updated_timestamp(entry.document.meta.updated_at.as_deref()),
                theme.muted_style(),
            )),
        ])
    });
    let months_table = Table::new(
        rows,
        [
            Constraint::Length(18),
            Constraint::Length(10),
            Constraint::Length(14),
            Constraint::Length(18),
        ],
    )
    .header(
        Row::new(vec![
            Cell::from(""),
            Cell::from("State"),
            Cell::from("Diff"),
            Cell::from("Updated"),
        ])
        .style(theme.emphasized_tone_style(Tone::Navigation)),
    )
    .style(theme.toned_panel_style(Tone::Navigation))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(theme.toned_panel_style(Tone::Navigation))
            .border_style(theme.panel_border_style(Tone::Navigation)),
    )
    .row_highlight_style(theme.selected_style());
    let mut table_state = TableState::default();
    if !state.months.is_empty() {
        table_state.select(Some(state.selected.saturating_sub(visible_start)));
    }
    frame.render_stateful_widget(months_table, area, &mut table_state);
}

fn render_navigation_summary(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &NavigationState,
    theme: &UiTheme,
) {
    let summary_text = if let Some(entry) = state.selected_month() {
        compact_summary_text(&entry.calculated, area.width, theme)
    } else {
        Text::from("No months yet.\nPress `n` to create the first month.")
    };
    let summary = Paragraph::new(summary_text)
        .style(theme.toned_panel_style(Tone::Summary))
        .block(panel_block(
            "Summary",
            PanelChrome::Boxed,
            Tone::Summary,
            theme,
        ))
        .wrap(Wrap { trim: false });
    frame.render_widget(summary, area);
}

fn visible_month_window(total: usize, selected: usize, visible_rows: usize) -> Range<usize> {
    if total == 0 {
        return 0..0;
    }
    let visible_rows = visible_rows.max(1).min(total);
    let selected = selected.min(total - 1);
    let before = visible_rows / 2;
    let mut start = selected.saturating_sub(before);
    start = start.min(total - visible_rows);
    start..start + visible_rows
}

fn render_navigation_dialogue(
    frame: &mut Frame<'_>,
    dialogue: &NavigationDialogue,
    theme: &UiTheme,
) {
    let area = centred_rect(68, 36, frame.area());
    frame.render_widget(Clear, area);
    match dialogue {
        NavigationDialogue::Create(dialogue) => {
            render_dialogue(
                frame,
                area,
                "New Month",
                Tone::Navigation,
                &[
                    "Create month".to_owned(),
                    "".to_owned(),
                    "Enter YYYY-MM".to_owned(),
                    dialogue.input.clone(),
                    "".to_owned(),
                    dialogue
                        .error
                        .clone()
                        .unwrap_or_else(|| "Enter confirms. Esc cancels.".to_owned()),
                ],
                theme,
            );
        }
        NavigationDialogue::Rename(dialogue) => {
            render_dialogue(
                frame,
                area,
                "Rename Month",
                Tone::Navigation,
                &[
                    format!("Rename {}", dialogue.source.display_label()),
                    "".to_owned(),
                    "Enter the new month id (YYYY-MM)".to_owned(),
                    dialogue.input.clone(),
                    "".to_owned(),
                    dialogue
                        .error
                        .clone()
                        .unwrap_or_else(|| "Enter confirms. Esc cancels.".to_owned()),
                ],
                theme,
            );
        }
        NavigationDialogue::Delete(dialogue) => {
            render_delete_dialogue(frame, area, dialogue, theme);
        }
    }
}

fn render_delete_dialogue(
    frame: &mut Frame<'_>,
    area: Rect,
    dialogue: &DeleteDialogue,
    theme: &UiTheme,
) {
    render_dialogue(
        frame,
        area,
        "Delete Month",
        Tone::Danger,
        &[
            format!("Delete {}?", dialogue.month.display_label()),
            "".to_owned(),
            format!("Type {} to confirm deletion.", dialogue.month),
            dialogue.confirmation.clone(),
            "".to_owned(),
            dialogue
                .error
                .clone()
                .unwrap_or_else(|| "Enter confirms. Esc cancels.".to_owned()),
        ],
        theme,
    );
}

fn render_dialogue(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    tone: Tone,
    lines: &[String],
    theme: &UiTheme,
) {
    // Dialogue content is preassembled as lines so the create, rename, and
    // delete flows can share one renderer with route-specific copy.
    let text = Text::from(lines.iter().cloned().map(Line::from).collect::<Vec<_>>());
    frame.render_widget(
        Paragraph::new(text)
            .style(theme.toned_panel_style(tone))
            .block(panel_block(title, PanelChrome::Boxed, tone, theme))
            .wrap(Wrap { trim: false }),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::visible_month_window;

    #[test]
    fn visible_month_window_keeps_selection_in_rendered_slice() {
        assert_eq!(visible_month_window(0, 0, 10), 0..0);
        assert_eq!(visible_month_window(5, 0, 10), 0..5);
        assert_eq!(visible_month_window(120, 0, 8), 0..8);
        assert_eq!(visible_month_window(120, 60, 8), 56..64);
        assert_eq!(visible_month_window(120, 119, 8), 112..120);
    }
}
