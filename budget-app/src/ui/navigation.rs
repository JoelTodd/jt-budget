use std::path::Path;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Frame, Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap};

use super::layout::{PanelChrome, centred_rect};
use super::theme::{Tone, UiTheme, month_state_style, validation_tone};
use super::widgets::{
    abbreviate_path, amount_cell_with_style, compact_summary_text, format_updated_timestamp,
    hint_lines, panel_block,
};
use crate::state::{DeleteDialogue, NavigationDialogue, NavigationState};

pub(super) fn render_navigation(
    frame: &mut Frame<'_>,
    state: &NavigationState,
    repo_root: &Path,
    theme: &UiTheme,
) {
    let compact = frame.area().width < 110 || frame.area().height < 28;
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
            &["Enter open", "n new", "m rename", "d delete", "q quit"]
        } else {
            &[
                "Enter open",
                "n new",
                "m rename",
                "d delete",
                "r refresh",
                "q quit",
            ]
        },
        theme,
    ));
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

    // Compact terminals stack the table over the summary so both remain usable
    // at 80x24 without horizontal truncation.
    let body = if compact {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(layout[1])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(layout[1])
    };

    let rows = state.months.iter().map(|entry| {
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
        table_state.select(Some(state.selected));
    }
    frame.render_stateful_widget(months_table, body[0], &mut table_state);

    let summary_text = if let Some(entry) = state.selected_month() {
        compact_summary_text(&entry.calculated, body[1].width, theme)
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
    frame.render_widget(summary, body[1]);

    if let Some(dialogue) = &state.dialogue {
        render_navigation_dialogue(frame, dialogue, theme);
    }
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
