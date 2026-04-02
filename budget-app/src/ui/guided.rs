use budget_core::AppConfig;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::{Frame, Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};

use super::layout::{GuidedLayoutProfile, PanelChrome};
use super::theme::{Tone, UiTheme, metric_spans, status_value_style, validation_tone};
use super::widgets::{
    compact_summary_text, hint_lines, is_guided_status_message, panel_block, status_line,
};
use crate::state::{FieldId, GuidedCreationState};

pub(super) fn render_guided_creation(
    frame: &mut Frame<'_>,
    state: &GuidedCreationState,
    config: &AppConfig,
    theme: &UiTheme,
) {
    let profile = GuidedLayoutProfile::for_area(frame.area());
    let mut header_lines = vec![Line::from(vec![
        Span::styled(state.document.month.display_label(), theme.bright_style()),
        Span::styled(
            format!("  |  Step {}/{}", state.step_index + 1, state.steps.len()),
            theme.muted_style(),
        ),
    ])];
    header_lines.extend(hint_lines(
        frame.area().width,
        match profile {
            GuidedLayoutProfile::Compact => &["Type amount", "Enter save", "Esc months", "q quit"],
            GuidedLayoutProfile::Standard | GuidedLayoutProfile::Wide => &[
                "Type amount",
                "Backspace delete",
                "Enter save step",
                "Esc months",
                "q quit",
            ],
        },
        theme,
    ));
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_lines.len() as u16 + 2),
            Constraint::Min(8),
            Constraint::Length(match profile {
                GuidedLayoutProfile::Compact => 3,
                GuidedLayoutProfile::Standard => 5,
                GuidedLayoutProfile::Wide => 4,
            }),
        ])
        .split(frame.area());

    let header = Paragraph::new(header_lines)
        .style(theme.toned_panel_style(Tone::Guided))
        .block(panel_block(
            "Guided Creation",
            PanelChrome::Boxed,
            Tone::Guided,
            theme,
        ));
    frame.render_widget(header, layout[0]);

    let current_step = &state.steps[state.step_index];
    let step_widget = Paragraph::new(guided_step_text(
        state,
        config,
        current_step,
        profile,
        theme,
    ))
    .style(theme.toned_panel_style(Tone::Guided))
    .block(panel_block(
        "Current Step",
        guided_panel_chrome(profile),
        Tone::Guided,
        theme,
    ))
    .wrap(Wrap { trim: false });

    match profile {
        GuidedLayoutProfile::Compact => {
            let body = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(11), Constraint::Min(5)])
                .split(layout[1]);
            frame.render_widget(step_widget, body[0]);
            frame.render_widget(
                Paragraph::new(guided_preview_text(state, body[1].width, profile, theme))
                    .style(theme.toned_panel_style(Tone::Summary))
                    .block(panel_block(
                        "Preview",
                        PanelChrome::Boxed,
                        Tone::Summary,
                        theme,
                    ))
                    .wrap(Wrap { trim: false }),
                body[1],
            );
        }
        GuidedLayoutProfile::Standard => {
            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
                .split(layout[1]);
            frame.render_widget(step_widget, body[0]);
            frame.render_widget(
                Paragraph::new(guided_preview_text(state, body[1].width, profile, theme))
                    .style(theme.toned_panel_style(Tone::Summary))
                    .block(panel_block(
                        "Live Preview",
                        PanelChrome::Boxed,
                        Tone::Summary,
                        theme,
                    ))
                    .wrap(Wrap { trim: false }),
                body[1],
            );
        }
        GuidedLayoutProfile::Wide => {
            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(37),
                    Constraint::Length(2),
                    Constraint::Percentage(63),
                ])
                .split(layout[1]);
            frame.render_widget(step_widget, body[0]);
            frame.render_widget(
                Paragraph::new(guided_preview_text(state, body[2].width, profile, theme))
                    .style(theme.toned_panel_style(Tone::Summary))
                    .block(panel_block(
                        "Live Preview",
                        PanelChrome::TopRule,
                        Tone::Summary,
                        theme,
                    ))
                    .wrap(Wrap { trim: false }),
                body[2],
            );
        }
    }

    frame.render_widget(
        Paragraph::new(guided_status_lines(state, profile, theme))
            .style(theme.toned_panel_style(Tone::Status))
            .block(panel_block(
                "Status",
                guided_panel_chrome(profile),
                Tone::Status,
                theme,
            ))
            .wrap(Wrap { trim: false }),
        layout[2],
    );
}

fn guided_step_text(
    state: &GuidedCreationState,
    config: &AppConfig,
    current_step: &FieldId,
    profile: GuidedLayoutProfile,
    theme: &UiTheme,
) -> Text<'static> {
    match profile {
        GuidedLayoutProfile::Compact => {
            // Compact mode keeps the current step, current value, and next step
            // together so the flow still makes sense at 80x24.
            let mut lines = vec![
                Line::from(Span::styled(
                    current_step.label(config),
                    theme.emphasized_tone_style(Tone::Guided),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Amount",
                    theme.emphasized_tone_style(Tone::Guided),
                )),
                Line::from(Span::styled(
                    format!(
                        "{}{}",
                        state.input.display_text(),
                        if state.input.is_edited() { "_" } else { "" }
                    ),
                    if state.input.is_edited() {
                        theme.editing_style()
                    } else {
                        theme.selected_style()
                    },
                )),
            ];
            if let Some(message) = state
                .message
                .as_deref()
                .filter(|message| !is_guided_status_message(message))
            {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    message.to_owned(),
                    theme.bright_style(),
                )));
            } else {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Enter saves this step.",
                    theme.muted_style(),
                )));
            }
            lines.push(Line::from(""));
            let next_step = guided_next_step_line(state, config);
            let (prefix, remainder) = next_step
                .split_once(": ")
                .map(|(prefix, remainder)| (format!("{prefix}: "), remainder.to_owned()))
                .unwrap_or_else(|| ("".to_owned(), next_step));
            lines.push(Line::from(vec![
                Span::styled(prefix, theme.muted_style()),
                Span::styled(remainder, theme.emphasized_tone_style(Tone::Summary)),
            ]));
            Text::from(lines)
        }
        GuidedLayoutProfile::Standard | GuidedLayoutProfile::Wide => {
            let mut lines = vec![
                Line::from(Span::styled(
                    current_step.label(config),
                    theme.emphasized_tone_style(Tone::Guided),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Type digits or decimals, then press Enter to autosave.",
                    theme.muted_style(),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Input: ", theme.muted_style()),
                    Span::styled(
                        format!(
                            "{}{}",
                            state.input.display_text(),
                            if state.input.is_edited() { "_" } else { "" }
                        ),
                        if state.input.is_edited() {
                            theme.editing_style()
                        } else {
                            theme.selected_style()
                        },
                    ),
                ]),
            ];
            if let Some(message) = state
                .message
                .as_deref()
                .filter(|message| !is_guided_status_message(message))
            {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    message.to_owned(),
                    theme.bright_style(),
                )));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("Next steps:", theme.muted_style())));
            for step in state.steps.iter().skip(state.step_index + 1).take(
                if profile == GuidedLayoutProfile::Wide {
                    6
                } else {
                    5
                },
            ) {
                lines.push(Line::from(vec![
                    Span::styled("• ", theme.subtle_style()),
                    Span::styled(step.label(config), theme.tone_style(Tone::Summary)),
                ]));
            }
            Text::from(lines)
        }
    }
}

fn guided_next_step_line(state: &GuidedCreationState, config: &AppConfig) -> String {
    state
        .steps
        .get(state.step_index + 1)
        .map(|step| format!("Next: {}", step.label(config)))
        .unwrap_or_else(|| "Final step.".to_owned())
}

fn guided_preview_text(
    state: &GuidedCreationState,
    width: u16,
    profile: GuidedLayoutProfile,
    theme: &UiTheme,
) -> Text<'static> {
    match profile {
        GuidedLayoutProfile::Compact => Text::from(vec![
            // Compact preview surfaces only the metrics most useful while the
            // user is still stepping through the draft.
            Line::from({
                let mut spans = metric_spans(
                    theme,
                    "Accounts",
                    state.calculated.totals.accounts_subtotal.format(),
                    Tone::Accounts,
                );
                spans.push(Span::styled("  |  ", theme.subtle_style()));
                spans.extend(metric_spans(
                    theme,
                    "Pots",
                    state.calculated.totals.pots_final_total.format(),
                    Tone::Pots,
                ));
                spans
            }),
            Line::from({
                let mut spans = metric_spans(
                    theme,
                    "Earmarks",
                    state
                        .calculated
                        .totals
                        .next_month_earmarks_subtotal
                        .format(),
                    Tone::Earmarks,
                );
                spans.push(Span::styled("  |  ", theme.subtle_style()));
                spans.extend(metric_spans(
                    theme,
                    "Diff",
                    state.calculated.validation.overall_difference.format(),
                    validation_tone(state.calculated.validation.is_valid),
                ));
                spans
            }),
            Line::from(vec![
                Span::styled("Status ", theme.muted_style()),
                Span::styled(
                    if state.calculated.validation.is_valid {
                        "valid"
                    } else {
                        "invalid"
                    },
                    status_value_style(
                        theme,
                        if state.calculated.validation.is_valid {
                            "valid"
                        } else {
                            "invalid"
                        },
                    ),
                ),
            ]),
        ]),
        GuidedLayoutProfile::Standard | GuidedLayoutProfile::Wide => {
            compact_summary_text(&state.calculated, width, theme)
        }
    }
}

fn guided_status_lines(
    state: &GuidedCreationState,
    profile: GuidedLayoutProfile,
    theme: &UiTheme,
) -> Vec<Line<'static>> {
    match profile {
        GuidedLayoutProfile::Compact => vec![status_line(state.persistence, state.sync, theme)],
        GuidedLayoutProfile::Standard => vec![
            Line::from({
                let validation = if state.calculated.validation.is_valid {
                    "within tolerance"
                } else {
                    "outside tolerance"
                };
                vec![
                    Span::styled("Validation: ", theme.muted_style()),
                    Span::styled(validation, status_value_style(theme, validation)),
                    Span::styled("  |  Difference: ", theme.subtle_style()),
                    Span::styled(
                        state.calculated.validation.overall_difference.format(),
                        theme.emphasized_tone_style(validation_tone(
                            state.calculated.validation.is_valid,
                        )),
                    ),
                ]
            }),
            status_line(state.persistence, state.sync, theme),
            Line::from(Span::styled(
                "The draft is saved as you confirm each guided step.",
                theme.muted_style(),
            )),
        ],
        GuidedLayoutProfile::Wide => vec![
            Line::from({
                let validation = if state.calculated.validation.is_valid {
                    "within tolerance"
                } else {
                    "outside tolerance"
                };
                vec![
                    Span::styled("Validation: ", theme.muted_style()),
                    Span::styled(validation, status_value_style(theme, validation)),
                    Span::styled("  |  Difference: ", theme.subtle_style()),
                    Span::styled(
                        state.calculated.validation.overall_difference.format(),
                        theme.emphasized_tone_style(validation_tone(
                            state.calculated.validation.is_valid,
                        )),
                    ),
                ]
            }),
            status_line(state.persistence, state.sync, theme),
        ],
    }
}

fn guided_panel_chrome(profile: GuidedLayoutProfile) -> PanelChrome {
    match profile {
        GuidedLayoutProfile::Compact | GuidedLayoutProfile::Standard => PanelChrome::Boxed,
        GuidedLayoutProfile::Wide => PanelChrome::TopRule,
    }
}
