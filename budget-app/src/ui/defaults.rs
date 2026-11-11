use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::{Frame, Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState, Wrap};

use super::layout::PanelChrome;
use super::theme::{Tone, UiTheme};
use super::widgets::{amount_cell_with_style, hint_lines, panel_block};
use crate::state::DefaultsState;

pub(super) fn render_defaults(frame: &mut Frame<'_>, state: &DefaultsState, theme: &UiTheme) {
    let editing = state.edit_buffer.is_some();
    let mut header = vec![Line::from(
        "Prefills for new months. Existing months keep their amounts.",
    )];
    header.extend(hint_lines(
        frame.area().width,
        if editing {
            &["Type amount", "Enter save", "Esc cancel"]
        } else {
            &["Tab/Up/Down move", "Enter edit", "Esc months", "q quit"]
        },
        theme,
    ));
    let layout = Layout::vertical([
        Constraint::Length(header.len() as u16 + 2),
        Constraint::Min(5),
        Constraint::Length(4),
    ])
    .split(frame.area());
    frame.render_widget(
        Paragraph::new(header)
            .style(theme.toned_panel_style(Tone::Navigation))
            .block(panel_block(
                "Defaults",
                PanelChrome::Boxed,
                Tone::Navigation,
                theme,
            )),
        layout[0],
    );

    let visible = usize::from(layout[1].height.saturating_sub(3)).max(1);
    let start = state.focus_index.saturating_sub(visible - 1);
    let rows = state
        .fields
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .map(|(index, field)| {
            let value = if index == state.focus_index {
                state
                    .edit_buffer
                    .as_ref()
                    .map(|buffer| buffer.display_text())
            } else {
                None
            }
            .unwrap_or_else(|| field.value(&state.config).format());
            Row::new(vec![
                Cell::from(Span::styled(field.group(), theme.muted_style())),
                Cell::from(field.label(&state.config).to_owned()),
                amount_cell_with_style(
                    value,
                    if editing && index == state.focus_index {
                        theme.emphasized_tone_style(Tone::Warning)
                    } else {
                        theme.bright_style()
                    },
                ),
            ])
        });
    let table = Table::new(
        rows,
        [
            Constraint::Length(22),
            Constraint::Min(12),
            Constraint::Length(18),
        ],
    )
    .header(
        Row::new(["Default", "Name", "Amount"])
            .style(theme.emphasized_tone_style(Tone::Navigation)),
    )
    .style(theme.toned_panel_style(Tone::Navigation))
    .block(panel_block(
        if editing {
            "Monthly defaults - Editing"
        } else {
            "Monthly defaults"
        },
        PanelChrome::Boxed,
        if editing {
            Tone::Warning
        } else {
            Tone::Navigation
        },
        theme,
    ))
    .row_highlight_style(theme.selected_style())
    .highlight_symbol("> ");
    let mut selection = TableState::default();
    if !state.fields.is_empty() {
        selection.select(Some(state.focus_index.saturating_sub(start)));
    }
    frame.render_stateful_widget(table, layout[1], &mut selection);

    let message = state.message.as_deref().unwrap_or(if editing {
        "Typing replaces the amount. Enter saves and syncs this default."
    } else if state.fields.is_empty() {
        "No savings pots or earmarks configured. Esc returns to months."
    } else {
        "Each confirmed change is saved to config.toml."
    });
    frame.render_widget(
        Paragraph::new(message)
            .style(theme.bright_style())
            .block(panel_block(
                "Status",
                PanelChrome::Boxed,
                Tone::Status,
                theme,
            ))
            .wrap(Wrap { trim: false }),
        layout[2],
    );
}
