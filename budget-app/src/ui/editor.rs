mod section_table;
mod sections;

use budget_core::AccountKind;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Frame, Line, Modifier, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Wrap};

use self::sections::{render_accounts, render_earmarks, render_pots, render_timing};
use super::layout::{EditorLayoutProfile, PanelChrome, section_height};
use super::theme::{Tone, UiTheme, section_tone, status_value_style, validation_tone};
use super::widgets::{
    amount_cell_with_style, combined_focus_state, field_focus_state_matching, focused_row_style,
    hint_lines, labelled_row_cell, panel_block, section_focus_state, selected_field, status_line,
    styled_value_cell_with_tone, value_for_current,
};
use crate::state::{EditorState, FieldId, SectionId};

pub(super) fn render_editor(frame: &mut Frame<'_>, state: &EditorState, theme: &UiTheme) {
    let profile = EditorLayoutProfile::for_area(frame.area());
    let mut header_lines = vec![Line::from(Span::styled(
        state.document.month.display_label(),
        theme.bright_style(),
    ))];
    header_lines.extend(hint_lines(
        frame.area().width,
        if profile == EditorLayoutProfile::Compact {
            &["Enter edit", "Tab next", "Esc months", "q quit"]
        } else {
            &["Enter edit", "Tab/Shift-Tab move", "Esc months", "q quit"]
        },
        theme,
    ));
    let footer_height = if profile == EditorLayoutProfile::Wide {
        3
    } else {
        4
    };
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_lines.len() as u16 + 2),
            Constraint::Min(10),
            Constraint::Length(footer_height),
        ])
        .split(frame.area());

    let header = Paragraph::new(header_lines)
        .block(panel_block(
            "Monthly Sheet",
            PanelChrome::Boxed,
            Tone::Navigation,
            theme,
        ))
        .style(theme.toned_panel_style(Tone::Navigation));
    frame.render_widget(header, layout[0]);

    match profile {
        EditorLayoutProfile::Wide => render_editor_wide(frame, layout[1], state, theme),
        EditorLayoutProfile::Standard => render_editor_standard(frame, layout[1], state, theme),
        EditorLayoutProfile::Compact => render_editor_compact(frame, layout[1], state, theme),
    }

    render_editor_footer(frame, layout[2], state, profile, theme);
}

fn render_editor_wide(frame: &mut Frame<'_>, area: Rect, state: &EditorState, theme: &UiTheme) {
    // Wide terminals reserve the right column for pots because that section is
    // the tallest and benefits most from uninterrupted vertical space.
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(54),
            Constraint::Length(2),
            Constraint::Percentage(46),
        ])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(section_height(state.calculated.account_rows.len(), false)),
            Constraint::Length(1),
            Constraint::Length(section_height(2, false)),
            Constraint::Length(1),
            Constraint::Min(section_height(state.calculated.earmark_rows.len(), false)),
        ])
        .split(columns[0]);

    render_accounts(
        frame,
        left[0],
        state,
        false,
        true,
        PanelChrome::TopRule,
        theme,
    );
    render_timing(
        frame,
        left[2],
        state,
        false,
        true,
        PanelChrome::TopRule,
        theme,
    );
    render_earmarks(
        frame,
        left[4],
        state,
        false,
        true,
        PanelChrome::TopRule,
        theme,
    );
    render_pots(
        frame,
        columns[2],
        state,
        false,
        true,
        PanelChrome::TopRule,
        theme,
    );
}

fn render_editor_standard(frame: &mut Frame<'_>, area: Rect, state: &EditorState, theme: &UiTheme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(section_height(state.calculated.account_rows.len(), false)),
            Constraint::Length(section_height(2, true)),
            Constraint::Length(section_height(state.calculated.earmark_rows.len(), true)),
            Constraint::Min(section_height(state.calculated.pot_rows.len() + 2, false)),
        ])
        .split(area);
    render_accounts(
        frame,
        rows[0],
        state,
        false,
        true,
        PanelChrome::Boxed,
        theme,
    );
    render_timing(frame, rows[1], state, true, true, PanelChrome::Boxed, theme);
    render_earmarks(frame, rows[2], state, true, true, PanelChrome::Boxed, theme);
    render_pots(
        frame,
        rows[3],
        state,
        false,
        true,
        PanelChrome::Boxed,
        theme,
    );
}

fn render_editor_compact(frame: &mut Frame<'_>, area: Rect, state: &EditorState, theme: &UiTheme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(7)])
        .split(area);
    let selected_section = selected_field(state)
        .map(FieldId::section)
        .unwrap_or(SectionId::Accounts);
    render_section_tabs(frame, rows[0], selected_section, theme);
    match selected_section {
        SectionId::Accounts => render_accounts(
            frame,
            rows[1],
            state,
            true,
            false,
            PanelChrome::Boxed,
            theme,
        ),
        SectionId::TimingAdjustments => render_timing(
            frame,
            rows[1],
            state,
            true,
            false,
            PanelChrome::Boxed,
            theme,
        ),
        SectionId::NextMonthEarmarks => render_earmarks(
            frame,
            rows[1],
            state,
            true,
            false,
            PanelChrome::Boxed,
            theme,
        ),
        SectionId::SavingsPots => render_pots(
            frame,
            rows[1],
            state,
            true,
            false,
            PanelChrome::Boxed,
            theme,
        ),
    }
}

fn render_section_tabs(frame: &mut Frame<'_>, area: Rect, selected: SectionId, theme: &UiTheme) {
    let mut spans = Vec::new();
    for section in SectionId::ALL {
        if !spans.is_empty() {
            spans.push(Span::styled(" | ", theme.subtle_style()));
        }
        spans.push(Span::styled(
            section.compact_title(),
            if section == selected {
                theme.selected_style()
            } else {
                theme.emphasized_tone_style(section_tone(section))
            },
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .style(theme.toned_panel_style(Tone::Navigation))
            .block(panel_block(
                "Sections",
                PanelChrome::Boxed,
                Tone::Navigation,
                theme,
            )),
        area,
    );
}

fn render_editor_footer(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &EditorState,
    profile: EditorLayoutProfile,
    theme: &UiTheme,
) {
    let lines = vec![
        Line::from(vec![
            Span::styled("Budget: ", theme.muted_style()),
            Span::styled(
                if state.calculated.validation.is_valid {
                    "within tolerance"
                } else {
                    "outside tolerance"
                },
                status_value_style(
                    theme,
                    if state.calculated.validation.is_valid {
                        "within tolerance"
                    } else {
                        "outside tolerance"
                    },
                ),
            ),
            Span::styled("  |  Difference: ", theme.subtle_style()),
            Span::styled(
                state.calculated.validation.overall_difference.format(),
                theme.emphasized_tone_style(validation_tone(state.calculated.validation.is_valid)),
            ),
        ]),
        status_line(state.persistence, state.sync, theme),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(if state.calculated.validation.is_valid {
                theme.toned_panel_style(Tone::Success)
            } else {
                theme.toned_panel_style(Tone::Danger)
            })
            .block(panel_block(
                "Validation",
                if profile == EditorLayoutProfile::Wide {
                    PanelChrome::TopRule
                } else {
                    PanelChrome::Boxed
                },
                if state.calculated.validation.is_valid {
                    Tone::Success
                } else {
                    Tone::Danger
                },
                theme,
            ))
            .wrap(Wrap { trim: false }),
        area,
    );
}
