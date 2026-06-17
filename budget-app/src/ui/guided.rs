use budget_core::AppConfig;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
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
    let header_lines = guided_header_lines(state, frame.area().width, profile, theme);
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

    render_guided_header(frame, layout[0], header_lines, theme);
    render_guided_body(frame, layout[1], state, config, profile, theme);
    render_guided_status(frame, layout[2], state, profile, theme);
}

fn guided_header_lines(
    state: &GuidedCreationState,
    width: u16,
    profile: GuidedLayoutProfile,
    theme: &UiTheme,
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled(state.document.month.display_label(), theme.bright_style()),
        Span::styled(
            format!("  |  Step {}/{}", state.step_index + 1, state.steps.len()),
            theme.muted_style(),
        ),
    ])];
    lines.extend(hint_lines(
        width,
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
    lines
}

fn render_guided_header(
    frame: &mut Frame<'_>,
    area: Rect,
    header_lines: Vec<Line<'static>>,
    theme: &UiTheme,
) {
    let header = Paragraph::new(header_lines)
        .style(theme.toned_panel_style(Tone::Guided))
        .block(panel_block(
            "Guided Creation",
            PanelChrome::Boxed,
            Tone::Guided,
            theme,
        ));
    frame.render_widget(header, area);
}

fn render_guided_body(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &GuidedCreationState,
    config: &AppConfig,
    profile: GuidedLayoutProfile,
    theme: &UiTheme,
) {
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
                .split(area);
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
                .split(area);
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
                .split(area);
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
}

fn render_guided_status(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &GuidedCreationState,
    profile: GuidedLayoutProfile,
    theme: &UiTheme,
) {
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
        area,
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
            compact_guided_step_text(state, config, current_step, theme)
        }
        GuidedLayoutProfile::Standard | GuidedLayoutProfile::Wide => {
            full_guided_step_text(state, config, current_step, profile, theme)
        }
    }
}

fn compact_guided_step_text(
    state: &GuidedCreationState,
    config: &AppConfig,
    current_step: &FieldId,
    theme: &UiTheme,
) -> Text<'static> {
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
        Line::from(input_display_span(state, theme)),
    ];
    lines.extend(guided_inline_message_lines(state, theme));
    lines.push(Line::from(""));
    lines.push(compact_next_step_line(state, config, theme));
    Text::from(lines)
}

fn full_guided_step_text(
    state: &GuidedCreationState,
    config: &AppConfig,
    current_step: &FieldId,
    profile: GuidedLayoutProfile,
    theme: &UiTheme,
) -> Text<'static> {
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
            input_display_span(state, theme),
        ]),
    ];
    if let Some(message) = non_status_message(state) {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(message, theme.bright_style())));
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

fn input_display_span(state: &GuidedCreationState, theme: &UiTheme) -> Span<'static> {
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
    )
}

fn guided_inline_message_lines(state: &GuidedCreationState, theme: &UiTheme) -> Vec<Line<'static>> {
    let message = non_status_message(state).unwrap_or_else(|| "Enter saves this step.".to_owned());
    let style = if non_status_message(state).is_some() {
        theme.bright_style()
    } else {
        theme.muted_style()
    };
    vec![Line::from(""), Line::from(Span::styled(message, style))]
}

fn compact_next_step_line(
    state: &GuidedCreationState,
    config: &AppConfig,
    theme: &UiTheme,
) -> Line<'static> {
    let next_step = guided_next_step_line(state, config);
    let (prefix, remainder) = next_step
        .split_once(": ")
        .map(|(prefix, remainder)| (format!("{prefix}: "), remainder.to_owned()))
        .unwrap_or_else(|| ("".to_owned(), next_step));
    Line::from(vec![
        Span::styled(prefix, theme.muted_style()),
        Span::styled(remainder, theme.emphasized_tone_style(Tone::Summary)),
    ])
}

fn non_status_message(state: &GuidedCreationState) -> Option<String> {
    state
        .message
        .as_deref()
        .filter(|message| !is_guided_status_message(message))
        .map(ToOwned::to_owned)
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
