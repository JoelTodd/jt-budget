use std::path::Path;
use std::sync::OnceLock;

use budget_core::{AccountKind, AppConfig, CalculatedMonth, MonthDocument};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap};
use serde::Deserialize;

use crate::state::{
    DeleteDialog, EditorState, FailureState, FieldId, GuidedCreationState, InteractionState,
    NavigationDialog, NavigationState, PersistenceState, Route, SectionId, SyncState,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tone {
    Navigation,
    Guided,
    Summary,
    Status,
    Liability,
    Accounts,
    Timing,
    Earmarks,
    Pots,
    Danger,
    Success,
    Warning,
}

type RgbTriplet = (u8, u8, u8);

#[derive(Clone, Debug, Deserialize)]
struct ThemeConfig {
    base24: Base24PaletteConfig,
}

#[derive(Clone, Debug, Deserialize)]
struct Base24PaletteConfig {
    base00: String,
    base01: String,
    base02: String,
    base03: String,
    base04: String,
    base05: String,
    base06: String,
    base07: String,
    base08: String,
    base09: String,
    #[serde(rename = "base0A")]
    base0_a: String,
    #[serde(rename = "base0B")]
    base0_b: String,
    #[serde(rename = "base0C")]
    base0_c: String,
    #[serde(rename = "base0D")]
    base0_d: String,
    #[serde(rename = "base0E")]
    base0_e: String,
    #[serde(rename = "base0F")]
    base0_f: String,
}

#[derive(Clone, Copy, Debug)]
struct UiTheme {
    base00: Color,
    base03: Color,
    base04: Color,
    base05: Color,
    base06: Color,
    base08: Color,
    base09: Color,
    base0a: Color,
    base0b: Color,
    base0c: Color,
    base0d: Color,
    base0e: Color,
}

fn rgb(color: RgbTriplet) -> Color {
    Color::Rgb(color.0, color.1, color.2)
}

impl UiTheme {
    fn project_default() -> &'static Self {
        static THEME: OnceLock<UiTheme> = OnceLock::new();
        THEME.get_or_init(|| {
            let config: ThemeConfig =
                toml::from_str(include_str!("../theme.toml")).expect("project theme.toml is valid");
            config
                .base24
                .validate()
                .expect("project theme.toml passes base24 validation");
            Self::from_palette(&config.base24)
        })
    }

    fn from_palette(palette: &Base24PaletteConfig) -> Self {
        Self {
            base00: rgb(parse_hex_triplet(&palette.base00)),
            base03: rgb(parse_hex_triplet(&palette.base03)),
            base04: rgb(parse_hex_triplet(&palette.base04)),
            base05: rgb(parse_hex_triplet(&palette.base05)),
            base06: rgb(parse_hex_triplet(&palette.base06)),
            base08: rgb(parse_hex_triplet(&palette.base08)),
            base09: rgb(parse_hex_triplet(&palette.base09)),
            base0a: rgb(parse_hex_triplet(&palette.base0_a)),
            base0b: rgb(parse_hex_triplet(&palette.base0_b)),
            base0c: rgb(parse_hex_triplet(&palette.base0_c)),
            base0d: rgb(parse_hex_triplet(&palette.base0_d)),
            base0e: rgb(parse_hex_triplet(&palette.base0_e)),
        }
    }

    fn tone_color(&self, tone: Tone) -> Color {
        match tone {
            Tone::Navigation => self.base0d,
            Tone::Guided => self.base0a,
            Tone::Summary => self.base0c,
            Tone::Status => self.base0a,
            Tone::Liability => self.base0e,
            Tone::Accounts => self.base0d,
            Tone::Timing => self.base09,
            Tone::Earmarks => self.base0e,
            Tone::Pots => self.base0b,
            Tone::Danger => self.base08,
            Tone::Success => self.base0b,
            Tone::Warning => self.base09,
        }
    }

    fn panel_surface_color(&self, _tone: Tone) -> Color {
        self.base00
    }

    fn app_style(&self) -> Style {
        Style::default().bg(self.base00).fg(self.base05)
    }

    fn toned_panel_style(&self, tone: Tone) -> Style {
        Style::default()
            .bg(self.panel_surface_color(tone))
            .fg(self.base05)
    }

    fn muted_style(&self) -> Style {
        Style::default().fg(self.base04)
    }

    fn subtle_style(&self) -> Style {
        Style::default().fg(self.base03)
    }

    fn bright_style(&self) -> Style {
        Style::default().fg(self.base06)
    }

    fn tone_style(&self, tone: Tone) -> Style {
        Style::default().fg(self.tone_color(tone))
    }

    fn emphasized_tone_style(&self, tone: Tone) -> Style {
        self.tone_style(tone).add_modifier(Modifier::BOLD)
    }

    fn panel_border_style(&self, tone: Tone) -> Style {
        self.tone_style(tone)
    }

    fn selected_style(&self) -> Style {
        Style::default()
            .bg(self.base00)
            .fg(self.base06)
            .add_modifier(Modifier::BOLD)
    }

    fn editing_style(&self) -> Style {
        Style::default()
            .bg(self.base00)
            .fg(self.tone_color(Tone::Warning))
            .add_modifier(Modifier::BOLD)
    }
}

impl Base24PaletteConfig {
    fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("base24.base00", self.base00.as_str()),
            ("base24.base01", self.base01.as_str()),
            ("base24.base02", self.base02.as_str()),
            ("base24.base03", self.base03.as_str()),
            ("base24.base04", self.base04.as_str()),
            ("base24.base05", self.base05.as_str()),
            ("base24.base06", self.base06.as_str()),
            ("base24.base07", self.base07.as_str()),
            ("base24.base08", self.base08.as_str()),
            ("base24.base09", self.base09.as_str()),
            ("base24.base0A", self.base0_a.as_str()),
            ("base24.base0B", self.base0_b.as_str()),
            ("base24.base0C", self.base0_c.as_str()),
            ("base24.base0D", self.base0_d.as_str()),
            ("base24.base0E", self.base0_e.as_str()),
            ("base24.base0F", self.base0_f.as_str()),
        ] {
            validate_hex_color(field, value)?;
        }

        Ok(())
    }
}

fn validate_hex_color(field: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix('#') else {
        return Err(format!("{field} must use #RRGGBB"));
    };
    if hex.len() != 6 || !hex.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(format!("{field} must use #RRGGBB"));
    }

    Ok(())
}

fn parse_hex_triplet(color: &str) -> RgbTriplet {
    let hex = color.strip_prefix('#').expect("base24 palette validated");
    (
        u8::from_str_radix(&hex[0..2], 16).expect("base24 palette validated"),
        u8::from_str_radix(&hex[2..4], 16).expect("base24 palette validated"),
        u8::from_str_radix(&hex[4..6], 16).expect("base24 palette validated"),
    )
}

fn section_tone(section: SectionId) -> Tone {
    match section {
        SectionId::Accounts => Tone::Accounts,
        SectionId::TimingAdjustments => Tone::Timing,
        SectionId::NextMonthEarmarks => Tone::Earmarks,
        SectionId::SavingsPots => Tone::Pots,
    }
}

fn validation_tone(is_valid: bool) -> Tone {
    if is_valid {
        Tone::Success
    } else {
        Tone::Danger
    }
}

fn metric_spans(theme: &UiTheme, label: &str, value: String, tone: Tone) -> Vec<Span<'static>> {
    vec![
        Span::styled(format!("{label} "), theme.muted_style()),
        Span::styled(value, theme.emphasized_tone_style(tone)),
    ]
}

fn status_value_style(theme: &UiTheme, label: &str) -> Style {
    match label {
        "clean" | "synced" | "valid" | "within tolerance" => {
            theme.emphasized_tone_style(Tone::Success)
        }
        "dirty" | "pending" => theme.emphasized_tone_style(Tone::Warning),
        "autosaving" | "syncing" => theme.emphasized_tone_style(Tone::Status),
        "failed" | "invalid" | "outside tolerance" => theme.emphasized_tone_style(Tone::Danger),
        _ => theme.bright_style(),
    }
}

fn operational_status_style(theme: &UiTheme, label: &str) -> Style {
    match label {
        "clean" | "synced" => theme.tone_style(Tone::Status),
        "dirty" | "pending" => theme.tone_style(Tone::Warning),
        "autosaving" | "syncing" => theme.emphasized_tone_style(Tone::Status),
        "failed" => theme.emphasized_tone_style(Tone::Danger),
        _ => theme.muted_style(),
    }
}

fn month_state_style(theme: &UiTheme, is_valid: bool) -> Style {
    if is_valid {
        theme.tone_style(Tone::Success)
    } else {
        theme.tone_style(Tone::Warning)
    }
}

fn key_hint_spans(theme: &UiTheme, hint: &str) -> Vec<Span<'static>> {
    if let Some((key, action)) = hint.split_once(' ') {
        vec![
            Span::styled(key.to_owned(), theme.emphasized_tone_style(Tone::Guided)),
            Span::raw(" "),
            Span::styled(action.to_owned(), theme.muted_style()),
        ]
    } else {
        vec![Span::styled(
            hint.to_owned(),
            theme.emphasized_tone_style(Tone::Guided),
        )]
    }
}

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
    frame.render_widget(Block::default().style(theme.app_style()), frame.area());
    match route {
        Route::Navigation(state) => render_navigation(frame, state, repo_root, theme),
        Route::GuidedCreation(state) => {
            if let Some(config) = config {
                render_guided_creation(frame, state, config, theme);
            }
        }
        Route::MonthEditing(state) => {
            if config.is_some() {
                render_editor(frame, state, theme);
            }
        }
        Route::BlockingFailure(state) => render_failure(frame, state, theme),
        Route::Shutdown => {}
    }
}

fn render_navigation(
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
            "finalized"
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

    if let Some(dialog) = &state.dialog {
        render_navigation_dialog(frame, dialog, theme);
    }
}

fn render_navigation_dialog(frame: &mut Frame<'_>, dialog: &NavigationDialog, theme: &UiTheme) {
    let area = centered_rect(68, 36, frame.area());
    frame.render_widget(Clear, area);
    match dialog {
        NavigationDialog::Create(dialog) => {
            render_dialog(
                frame,
                area,
                "New Month",
                Tone::Navigation,
                &[
                    "Create month".to_owned(),
                    "".to_owned(),
                    "Enter YYYY-MM".to_owned(),
                    dialog.input.clone(),
                    "".to_owned(),
                    dialog
                        .error
                        .clone()
                        .unwrap_or_else(|| "Enter confirms. Esc cancels.".to_owned()),
                ],
                theme,
            );
        }
        NavigationDialog::Rename(dialog) => {
            render_dialog(
                frame,
                area,
                "Rename Month",
                Tone::Navigation,
                &[
                    format!("Rename {}", dialog.source.display_label()),
                    "".to_owned(),
                    "Enter the new month id (YYYY-MM)".to_owned(),
                    dialog.input.clone(),
                    "".to_owned(),
                    dialog
                        .error
                        .clone()
                        .unwrap_or_else(|| "Enter confirms. Esc cancels.".to_owned()),
                ],
                theme,
            );
        }
        NavigationDialog::Delete(dialog) => {
            render_delete_dialog(frame, area, dialog, theme);
        }
    }
}

fn render_delete_dialog(frame: &mut Frame<'_>, area: Rect, dialog: &DeleteDialog, theme: &UiTheme) {
    render_dialog(
        frame,
        area,
        "Delete Month",
        Tone::Danger,
        &[
            format!("Delete {}?", dialog.month.display_label()),
            "".to_owned(),
            format!("Type {} to confirm deletion.", dialog.month),
            dialog.confirmation.clone(),
            "".to_owned(),
            dialog
                .error
                .clone()
                .unwrap_or_else(|| "Enter confirms. Esc cancels.".to_owned()),
        ],
        theme,
    );
}

fn render_dialog(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    tone: Tone,
    lines: &[String],
    theme: &UiTheme,
) {
    let text = Text::from(lines.iter().cloned().map(Line::from).collect::<Vec<_>>());
    frame.render_widget(
        Paragraph::new(text)
            .style(theme.toned_panel_style(tone))
            .block(panel_block(title, PanelChrome::Boxed, tone, theme))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_guided_creation(
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
                let spans = vec![
                    Span::styled("Validation: ", theme.muted_style()),
                    Span::styled(validation, status_value_style(theme, validation)),
                    Span::styled("  |  Difference: ", theme.subtle_style()),
                    Span::styled(
                        state.calculated.validation.overall_difference.format(),
                        theme.emphasized_tone_style(validation_tone(
                            state.calculated.validation.is_valid,
                        )),
                    ),
                ];
                spans
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
                let spans = vec![
                    Span::styled("Validation: ", theme.muted_style()),
                    Span::styled(validation, status_value_style(theme, validation)),
                    Span::styled("  |  Difference: ", theme.subtle_style()),
                    Span::styled(
                        state.calculated.validation.overall_difference.format(),
                        theme.emphasized_tone_style(validation_tone(
                            state.calculated.validation.is_valid,
                        )),
                    ),
                ];
                spans
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

fn render_editor(frame: &mut Frame<'_>, state: &EditorState, theme: &UiTheme) {
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

    let header = Paragraph::new(header_lines).block(panel_block(
        "Monthly Sheet",
        PanelChrome::Boxed,
        Tone::Navigation,
        theme,
    ));
    let header = header.style(theme.toned_panel_style(Tone::Navigation));
    frame.render_widget(header, layout[0]);

    match profile {
        EditorLayoutProfile::Wide => render_editor_wide(frame, layout[1], state, theme),
        EditorLayoutProfile::Standard => render_editor_standard(frame, layout[1], state, theme),
        EditorLayoutProfile::Compact => render_editor_compact(frame, layout[1], state, theme),
    }

    render_editor_footer(frame, layout[2], state, profile, theme);
}

fn render_editor_wide(frame: &mut Frame<'_>, area: Rect, state: &EditorState, theme: &UiTheme) {
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

fn render_failure(frame: &mut Frame<'_>, state: &FailureState, theme: &UiTheme) {
    let area = centered_rect(70, 40, frame.area());
    frame.render_widget(Clear, area);
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

fn render_accounts(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &EditorState,
    compact: bool,
    show_title: bool,
    chrome: PanelChrome,
    theme: &UiTheme,
) {
    let rows = state.calculated.account_rows.iter().map(|row| {
        let field = FieldId::Account(row.id.clone());
        let focus = field_focus_state(state, &field);
        Row::new(vec![
            labeled_row_cell(&row.label, focus),
            Cell::from(Span::styled(
                match row.kind {
                    AccountKind::Asset => "+",
                    AccountKind::Liability => "-",
                },
                theme.emphasized_tone_style(match row.kind {
                    AccountKind::Asset => Tone::Success,
                    AccountKind::Liability => Tone::Liability,
                }),
            )),
            styled_value_cell_with_tone(
                value_for_field(state, &field, &state.document),
                focus,
                Tone::Accounts,
                theme,
            ),
            amount_cell_with_style(
                row.normalised_balance.format(),
                theme.emphasized_tone_style(match row.kind {
                    AccountKind::Asset => Tone::Accounts,
                    AccountKind::Liability => Tone::Liability,
                }),
            ),
        ])
        .style(focused_row_style(focus, theme))
    });
    let table = Table::new(
        rows,
        if compact {
            [
                Constraint::Min(18),
                Constraint::Length(2),
                Constraint::Length(12),
                Constraint::Length(12),
            ]
        } else {
            [
                Constraint::Min(18),
                Constraint::Length(4),
                Constraint::Length(14),
                Constraint::Length(14),
            ]
        },
    )
    .header(
        Row::new(vec![
            Cell::from(if show_title { "" } else { "Account" }),
            Cell::from(""),
            Cell::from("Entered"),
            Cell::from("Net"),
        ])
        .style(theme.emphasized_tone_style(Tone::Accounts)),
    )
    .style(theme.toned_panel_style(Tone::Accounts))
    .block(section_block(
        show_title.then_some("Accounts"),
        format!(
            "Subtotal {}",
            state.calculated.totals.accounts_subtotal.format()
        ),
        section_focus_state(state, SectionId::Accounts),
        Tone::Accounts,
        chrome,
        theme,
    ));
    frame.render_widget(table, area);
}

fn render_timing(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &EditorState,
    compact: bool,
    show_title: bool,
    chrome: PanelChrome,
    theme: &UiTheme,
) {
    let correction = FieldId::PreviousMonthSpendingCorrection;
    let investment = FieldId::InvestmentNotYetSent;
    let correction_focus = field_focus_state(state, &correction);
    let investment_focus = field_focus_state(state, &investment);
    let rows = vec![
        Row::new(vec![
            labeled_row_cell("General spending over/under", correction_focus),
            styled_value_cell_with_tone(
                value_for_field(state, &correction, &state.document),
                correction_focus,
                Tone::Timing,
                theme,
            ),
            amount_cell_with_style(
                state
                    .calculated
                    .timing
                    .previous_month_spending_correction_effect
                    .format(),
                theme.emphasized_tone_style(Tone::Timing),
            ),
        ])
        .style(focused_row_style(correction_focus, theme)),
        Row::new(vec![
            labeled_row_cell("Investment not yet sent", investment_focus),
            styled_value_cell_with_tone(
                value_for_field(state, &investment, &state.document),
                investment_focus,
                Tone::Timing,
                theme,
            ),
            amount_cell_with_style(
                state.calculated.timing.investment_effect.format(),
                theme.emphasized_tone_style(Tone::Timing),
            ),
        ])
        .style(focused_row_style(investment_focus, theme)),
    ];
    let table = Table::new(
        rows,
        [
            Constraint::Min(if compact { 20 } else { 28 }),
            Constraint::Length(14),
            Constraint::Length(14),
        ],
    )
    .header(
        Row::new(vec![
            Cell::from(if show_title { "" } else { "Adjustment" }),
            Cell::from("Entered"),
            Cell::from("Effect"),
        ])
        .style(theme.emphasized_tone_style(Tone::Timing)),
    )
    .style(theme.toned_panel_style(Tone::Timing))
    .block(section_block(
        show_title.then_some("Timing Adjustments"),
        format!(
            "Subtotal {}",
            state.calculated.totals.timing_adjustments_subtotal.format()
        ),
        section_focus_state(state, SectionId::TimingAdjustments),
        Tone::Timing,
        chrome,
        theme,
    ));
    frame.render_widget(table, area);
}

fn render_earmarks(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &EditorState,
    compact: bool,
    show_title: bool,
    chrome: PanelChrome,
    theme: &UiTheme,
) {
    let rows = state.calculated.earmark_rows.iter().map(|row| {
        let field = FieldId::Earmark(row.id.clone());
        let focus = field_focus_state(state, &field);
        Row::new(vec![
            labeled_row_cell(&row.label, focus),
            styled_value_cell_with_tone(
                value_for_field(state, &field, &state.document),
                focus,
                Tone::Earmarks,
                theme,
            ),
        ])
        .style(focused_row_style(focus, theme))
    });
    let table = Table::new(
        rows,
        [
            Constraint::Min(if compact { 18 } else { 24 }),
            Constraint::Length(14),
        ],
    )
    .header(
        Row::new(vec![
            Cell::from(if show_title { "" } else { "Earmark" }),
            Cell::from("Amount"),
        ])
        .style(theme.emphasized_tone_style(Tone::Earmarks)),
    )
    .style(theme.toned_panel_style(Tone::Earmarks))
    .block(section_block(
        show_title.then_some("Next Month Earmarks"),
        format!(
            "Subtotal {}",
            state
                .calculated
                .totals
                .next_month_earmarks_subtotal
                .format()
        ),
        section_focus_state(state, SectionId::NextMonthEarmarks),
        Tone::Earmarks,
        chrome,
        theme,
    ));
    frame.render_widget(table, area);
}

fn render_pots(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &EditorState,
    compact: bool,
    show_title: bool,
    chrome: PanelChrome,
    theme: &UiTheme,
) {
    let mut rows = state
        .calculated
        .pot_rows
        .iter()
        .map(|row| {
            let carried = FieldId::PotCarried(row.id.clone());
            let change = FieldId::PotChange(row.id.clone());
            let carried_focus = field_focus_state(state, &carried);
            let change_focus = field_focus_state(state, &change);
            let row_focus = combined_focus_state(carried_focus, change_focus);
            Row::new(vec![
                labeled_row_cell(&row.label, row_focus),
                styled_value_cell_with_tone(
                    value_for_field(state, &carried, &state.document),
                    carried_focus,
                    Tone::Pots,
                    theme,
                ),
                styled_value_cell_with_tone(
                    value_for_field(state, &change, &state.document),
                    change_focus,
                    Tone::Pots,
                    theme,
                ),
                amount_cell_with_style(
                    row.final_balance.format(),
                    theme.emphasized_tone_style(Tone::Pots),
                ),
            ])
            .style(focused_row_style(row_focus, theme))
        })
        .collect::<Vec<_>>();
    rows.push(
        Row::new(vec![
            Cell::from(Span::styled(
                "Total",
                theme.emphasized_tone_style(Tone::Pots),
            )),
            amount_cell_with_style(
                state.calculated.totals.pots_carried_total.format(),
                theme.emphasized_tone_style(Tone::Pots),
            ),
            amount_cell_with_style(
                state.calculated.totals.pots_monthly_change_total.format(),
                theme.emphasized_tone_style(Tone::Pots),
            ),
            amount_cell_with_style(
                state.calculated.totals.pots_final_total.format(),
                theme.emphasized_tone_style(Tone::Pots),
            ),
        ])
        .style(theme.bright_style().add_modifier(Modifier::BOLD)),
    );

    let table = Table::new(
        rows,
        if compact {
            [
                Constraint::Min(18),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(12),
            ]
        } else {
            [
                Constraint::Min(18),
                Constraint::Length(14),
                Constraint::Length(14),
                Constraint::Length(14),
            ]
        },
    )
    .header(
        Row::new(vec![
            Cell::from(if show_title { "" } else { "Pot" }),
            Cell::from("Carried"),
            Cell::from("Change"),
            Cell::from("Final"),
        ])
        .style(theme.emphasized_tone_style(Tone::Pots)),
    )
    .style(theme.toned_panel_style(Tone::Pots))
    .block(section_block(
        show_title.then_some("Savings Pots"),
        format!(
            "Subtotal {}",
            state.calculated.totals.pots_final_total.format()
        ),
        section_focus_state(state, SectionId::SavingsPots),
        Tone::Pots,
        chrome,
        theme,
    ));
    frame.render_widget(table, area);
}

fn selected_field(state: &EditorState) -> Option<&FieldId> {
    state.fields.get(state.focus_index)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditorFocusState {
    Unfocused,
    Selected,
    Editing,
}

fn field_focus_state(state: &EditorState, field: &FieldId) -> EditorFocusState {
    if selected_field(state).is_none_or(|selected| selected != field) {
        return EditorFocusState::Unfocused;
    }
    match state.interaction {
        InteractionState::SheetIdle => EditorFocusState::Selected,
        InteractionState::FieldEditing => EditorFocusState::Editing,
    }
}

fn section_focus_state(state: &EditorState, section: SectionId) -> EditorFocusState {
    selected_field(state)
        .filter(|field| field.section() == section)
        .map(|field| field_focus_state(state, field))
        .unwrap_or(EditorFocusState::Unfocused)
}

fn combined_focus_state(left: EditorFocusState, right: EditorFocusState) -> EditorFocusState {
    match (left, right) {
        (EditorFocusState::Editing, _) | (_, EditorFocusState::Editing) => {
            EditorFocusState::Editing
        }
        (EditorFocusState::Selected, _) | (_, EditorFocusState::Selected) => {
            EditorFocusState::Selected
        }
        _ => EditorFocusState::Unfocused,
    }
}

fn value_for_field(state: &EditorState, field: &FieldId, document: &MonthDocument) -> String {
    if state.interaction == InteractionState::FieldEditing
        && selected_field(state).is_some_and(|selected| selected == field)
    {
        return state
            .edit_buffer
            .as_ref()
            .map(|value| {
                let mut display = value.display_text();
                display.push('_');
                display
            })
            .unwrap_or_else(|| field.current_value_text(document));
    }
    field.current_value_text(document)
}

fn styled_value_cell_with_tone(
    value: String,
    focus: EditorFocusState,
    tone: Tone,
    theme: &UiTheme,
) -> Cell<'static> {
    let cell = amount_cell(value);
    match focus {
        EditorFocusState::Unfocused => cell.style(theme.tone_style(tone)),
        EditorFocusState::Selected => cell.style(theme.selected_style()),
        EditorFocusState::Editing => cell.style(theme.editing_style()),
    }
}

fn labeled_row_cell(label: &str, focus: EditorFocusState) -> Cell<'static> {
    Cell::from(format!("{}{}", focus_marker(focus), label))
}

fn focus_marker(focus: EditorFocusState) -> &'static str {
    match focus {
        EditorFocusState::Unfocused => "  ",
        EditorFocusState::Selected => "› ",
        EditorFocusState::Editing => "✎ ",
    }
}

fn focused_row_style(focus: EditorFocusState, theme: &UiTheme) -> Style {
    match focus {
        EditorFocusState::Unfocused => Style::default(),
        EditorFocusState::Selected => theme.selected_style(),
        EditorFocusState::Editing => theme.selected_style(),
    }
}

fn amount_cell(value: String) -> Cell<'static> {
    Cell::from(Line::from(value).alignment(Alignment::Right))
}

fn amount_cell_with_style(value: String, style: Style) -> Cell<'static> {
    amount_cell(value).style(style)
}

fn section_height(row_count: usize, compact_title: bool) -> u16 {
    let base = row_count as u16 + if compact_title { 3 } else { 4 };
    base.max(6)
}

fn status_line(persistence: PersistenceState, sync: SyncState, theme: &UiTheme) -> Line<'static> {
    let persistence_label = match persistence {
        PersistenceState::Clean => "clean",
        PersistenceState::Dirty => "dirty",
        PersistenceState::Autosaving => "autosaving",
        PersistenceState::SaveFailed => "failed",
    };
    let sync_label = match sync {
        SyncState::SyncPending => "pending",
        SyncState::Syncing => "syncing",
        SyncState::Synced => "synced",
        SyncState::SyncFailed => "failed",
    };
    Line::from(vec![
        Span::styled("Persistence: ", theme.muted_style()),
        Span::styled(
            persistence_label,
            operational_status_style(theme, persistence_label),
        ),
        Span::styled("  |  Sync: ", theme.subtle_style()),
        Span::styled(sync_label, operational_status_style(theme, sync_label)),
    ])
}

fn hint_lines(width: u16, hints: &[&str], theme: &UiTheme) -> Vec<Line<'static>> {
    let available = width.saturating_sub(4) as usize;
    let separator = " | ";
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_spans = Vec::new();

    for hint in hints {
        let candidate_len = if current.is_empty() {
            hint.len()
        } else {
            current.len() + separator.len() + hint.len()
        };
        if !current.is_empty() && candidate_len > available {
            lines.push(Line::from(std::mem::take(&mut current_spans)));
            current.clear();
        }
        if !current.is_empty() {
            current.push_str(separator);
            current_spans.push(Span::styled(separator, theme.subtle_style()));
        }
        current.push_str(hint);
        current_spans.extend(key_hint_spans(theme, hint));
    }

    if !current.is_empty() {
        lines.push(Line::from(current_spans));
    }

    lines
}

fn abbreviate_path(path: &Path, max_width: usize) -> String {
    let text = path.display().to_string();
    if text.len() <= max_width || max_width <= 3 {
        return text;
    }
    format!("...{}", &text[text.len() - (max_width - 3)..])
}

fn panel_block(title: &str, chrome: PanelChrome, tone: Tone, theme: &UiTheme) -> Block<'static> {
    Block::default()
        .borders(panel_borders(chrome))
        .style(theme.toned_panel_style(tone))
        .border_style(theme.panel_border_style(tone))
        .title(
            Line::from(Span::styled(
                title.to_owned(),
                theme.emphasized_tone_style(tone),
            ))
            .alignment(Alignment::Left),
        )
}

fn panel_borders(chrome: PanelChrome) -> Borders {
    match chrome {
        PanelChrome::Boxed => Borders::ALL,
        PanelChrome::TopRule => Borders::TOP,
    }
}

fn section_block(
    title: Option<&str>,
    subtotal: String,
    focus: EditorFocusState,
    tone: Tone,
    chrome: PanelChrome,
    theme: &UiTheme,
) -> Block<'static> {
    let mut block = Block::default()
        .borders(panel_borders(chrome))
        .style(theme.toned_panel_style(tone))
        .border_style(section_emphasis_style(focus, tone, theme));
    if let Some(title) = title {
        block = block.title(
            Line::from(title.to_owned())
                .style(section_emphasis_style(focus, tone, theme))
                .alignment(Alignment::Left),
        );
    }
    block.title(
        Line::from(Span::styled(
            subtotal,
            theme.emphasized_tone_style(match focus {
                EditorFocusState::Editing => Tone::Warning,
                EditorFocusState::Selected => tone,
                EditorFocusState::Unfocused => tone,
            }),
        ))
        .alignment(Alignment::Right),
    )
}

fn section_emphasis_style(focus: EditorFocusState, tone: Tone, theme: &UiTheme) -> Style {
    match focus {
        EditorFocusState::Unfocused => theme.tone_style(tone),
        EditorFocusState::Selected => theme.emphasized_tone_style(tone),
        EditorFocusState::Editing => theme.emphasized_tone_style(Tone::Warning),
    }
}

fn compact_summary_text(
    calculated: &CalculatedMonth,
    width: u16,
    theme: &UiTheme,
) -> Text<'static> {
    let metrics = vec![
        (
            "Accounts",
            calculated.totals.accounts_subtotal.format(),
            Tone::Accounts,
        ),
        (
            "Timing Adjustments",
            calculated.totals.timing_adjustments_subtotal.format(),
            Tone::Timing,
        ),
        (
            "Next Month Earmarks",
            calculated.totals.next_month_earmarks_subtotal.format(),
            Tone::Earmarks,
        ),
        (
            "Savings Pots",
            calculated.totals.pots_final_total.format(),
            Tone::Pots,
        ),
        (
            "Total allocated",
            calculated.totals.total_allocated.format(),
            Tone::Summary,
        ),
        (
            "Overall difference",
            calculated.validation.overall_difference.format(),
            validation_tone(calculated.validation.is_valid),
        ),
        (
            "Status",
            if calculated.validation.is_valid {
                "valid".to_owned()
            } else {
                "invalid".to_owned()
            },
            validation_tone(calculated.validation.is_valid),
        ),
    ];
    Text::from(wrapped_metric_lines(width, &metrics, theme))
}

fn wrapped_metric_lines(
    width: u16,
    metrics: &[(&str, String, Tone)],
    theme: &UiTheme,
) -> Vec<Line<'static>> {
    let available = width.saturating_sub(4) as usize;
    let separator = " | ";
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_spans = Vec::new();

    for (label, value, tone) in metrics {
        let metric_text = format!("{label} {value}");
        let candidate_len = if current.is_empty() {
            metric_text.len()
        } else {
            current.len() + separator.len() + metric_text.len()
        };
        if !current.is_empty() && candidate_len > available {
            lines.push(Line::from(std::mem::take(&mut current_spans)));
            current.clear();
        }
        if !current.is_empty() {
            current.push_str(separator);
            current_spans.push(Span::styled(separator, theme.subtle_style()));
        }
        current.push_str(&metric_text);
        current_spans.extend(metric_spans(theme, label, value.clone(), *tone));
    }

    if !current.is_empty() {
        lines.push(Line::from(current_spans));
    }

    lines
}

fn format_updated_timestamp(updated_at: Option<&str>) -> String {
    let Some(updated_at) = updated_at else {
        return "-".to_owned();
    };
    if updated_at.len() >= 16 && updated_at.as_bytes().get(10) == Some(&b'T') {
        return format!("{} | {}", &updated_at[..10], &updated_at[11..16]);
    }
    updated_at.to_owned()
}

fn is_guided_status_message(message: &str) -> bool {
    matches!(
        message,
        message if message.starts_with("Draft created")
            || message.starts_with("Draft autosaved")
            || message.starts_with("Autosaving draft")
    )
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PanelChrome {
    Boxed,
    TopRule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GuidedLayoutProfile {
    Compact,
    Standard,
    Wide,
}

impl GuidedLayoutProfile {
    fn for_area(area: Rect) -> Self {
        if area.width >= 160 && area.height >= 32 {
            Self::Wide
        } else if area.width >= 100 && area.height >= 32 {
            Self::Standard
        } else {
            Self::Compact
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditorLayoutProfile {
    Compact,
    Standard,
    Wide,
}

impl EditorLayoutProfile {
    fn for_area(area: Rect) -> Self {
        if area.width >= 160 && area.height >= 32 {
            Self::Wide
        } else if area.width >= 100 && area.height >= 32 {
            Self::Standard
        } else {
            Self::Compact
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use budget_core::{AppConfig, MonthDocument, MonthId, SavingsPotState, calculate_month};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::prelude::{Color, Modifier};

    use super::{Base24PaletteConfig, UiTheme, render_with_theme};
    use crate::state::{
        EditorState, FieldId, GuidedCreationState, InteractionState, MoneyInput, MonthEntry,
        NavigationState, PersistenceState, Route, SyncState,
    };

    #[test]
    fn navigation_snapshot() {
        let config = AppConfig::default_mvp();
        let route = navigation_route(&config);

        insta::assert_snapshot!(buffer_to_string(draw_route(&route, Some(&config), 120, 40)));
    }

    #[test]
    fn navigation_snapshot_80x24() {
        let config = AppConfig::default_mvp();
        let route = navigation_route(&config);
        insta::assert_snapshot!(buffer_to_string(draw_route(&route, Some(&config), 80, 24)));
    }

    #[test]
    fn navigation_snapshot_105x48() {
        let config = AppConfig::default_mvp();
        let route = navigation_route(&config);
        insta::assert_snapshot!(buffer_to_string(draw_route(&route, Some(&config), 105, 48)));
    }

    #[test]
    fn navigation_snapshot_210x48() {
        let config = AppConfig::default_mvp();
        let route = navigation_route(&config);
        insta::assert_snapshot!(buffer_to_string(draw_route(&route, Some(&config), 210, 48)));
    }

    #[test]
    fn navigation_render_omits_redundant_month_action_helper_copy() {
        let config = AppConfig::default_mvp();
        let route = Route::Navigation(NavigationState::new(Vec::new()));
        let rendered = buffer_to_string(draw_route(&route, Some(&config), 105, 48));
        assert!(!rendered.contains("Month actions are explicit"));
    }

    #[test]
    fn navigation_render_uses_compact_summary_and_short_updated_text() {
        let config = AppConfig::default_mvp();
        let route = navigation_route(&config);
        let rendered = buffer_to_string(draw_route(&route, Some(&config), 105, 48));

        assert!(rendered.contains("2026-03-31 | 14:30"));
        assert!(!rendered.contains("14:30:06"));
        assert!(!rendered.contains("Current:"));
        assert!(rendered.contains("Accounts"));
        assert!(rendered.contains("Timing Adjustments"));
        assert!(rendered.contains("Next Month Earmarks"));
        assert!(rendered.contains("Savings Pots"));
        assert!(rendered.contains("Total allocated"));
        assert!(rendered.contains("Overall difference"));
        assert!(rendered.contains("Status invalid"));
        assert!(!rendered.contains("Months"));
        assert!(!rendered.contains("│Month"));
    }

    #[test]
    fn navigation_render_uses_semantic_status_and_difference_colours() {
        let config = AppConfig::default_mvp();
        let mut state = match navigation_route(&config) {
            Route::Navigation(state) => state,
            _ => unreachable!(),
        };
        state.months.push(state.months[0].clone());
        state.selected = 1;
        let route = Route::Navigation(state);
        let buffer = draw_route(&route, Some(&config), 105, 48);

        let (status_x, status_y) = find_text(&buffer, "draft").unwrap();
        let status = &buffer[(status_x, status_y)];
        assert_eq!(status.fg, Color::Rgb(0xff, 0xc6, 0x6d));

        let (diff_x, diff_y) = find_text(&buffer, "£801.00").unwrap();
        let diff = &buffer[(diff_x, diff_y)];
        assert_eq!(diff.fg, Color::Rgb(0xd2, 0x51, 0x51));
    }

    #[test]
    fn editor_snapshot() {
        let config = AppConfig::default_mvp();
        let route = editor_route(&config);
        insta::assert_snapshot!(buffer_to_string(draw_route(&route, Some(&config), 120, 40)));
    }

    #[test]
    fn editor_snapshot_80x24() {
        let config = AppConfig::default_mvp();
        let route = editor_route(&config);
        insta::assert_snapshot!(buffer_to_string(draw_route(&route, Some(&config), 80, 24)));
    }

    #[test]
    fn editor_snapshot_105x48() {
        let config = AppConfig::default_mvp();
        let route = editor_route(&config);
        insta::assert_snapshot!(buffer_to_string(draw_route(&route, Some(&config), 105, 48)));
    }

    #[test]
    fn editor_snapshot_210x48() {
        let config = AppConfig::default_mvp();
        let route = editor_route(&config);
        insta::assert_snapshot!(buffer_to_string(draw_route(&route, Some(&config), 210, 48)));
    }

    #[test]
    fn editor_render_omits_redundant_validation_and_focus_copy() {
        let config = AppConfig::default_mvp();
        let route = editor_route(&config);
        let rendered = buffer_to_string(draw_route(&route, Some(&config), 105, 48));

        assert!(!rendered.contains("validation:"));
        assert!(!rendered.contains("Focused field"));
        assert!(!rendered.contains("Previous month spending correction"));
        assert!(!rendered.contains("Previous month correction"));
        assert!(rendered.contains("General spending over/under"));
    }

    #[test]
    fn editor_render_shows_idle_focus_marker_and_active_section_emphasis() {
        let config = AppConfig::default_mvp();
        let buffer = draw_route(&editor_route(&config), Some(&config), 105, 48);
        let rendered = buffer_to_string(buffer.clone());

        assert!(rendered.contains("› Fun expensive stuff"));

        let (marker_x, marker_y) = find_text(&buffer, "› Fun expensive stuff").unwrap();
        let marker = &buffer[(marker_x, marker_y)];
        assert_eq!(marker.fg, Color::Rgb(0xee, 0xee, 0xec));
        assert_eq!(marker.bg, Color::Rgb(0x26, 0x26, 0x26));
        assert!(!marker.modifier.contains(Modifier::UNDERLINED));

        let (title_x, title_y) = find_text(&buffer, "Savings Pots").unwrap();
        let title = &buffer[(title_x, title_y)];
        assert_eq!(title.fg, Color::Rgb(0xa5, 0xc2, 0x61));
        assert!(title.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn editor_render_distinguishes_editing_from_idle_focus() {
        let config = AppConfig::default_mvp();
        let buffer = draw_route(
            &editor_route_with_state(&config, 8, InteractionState::FieldEditing),
            Some(&config),
            105,
            48,
        );
        let rendered = buffer_to_string(buffer.clone());

        assert!(rendered.contains("✎ Fun expensive stuff"));
        assert!(rendered.contains("£820.00_"));

        let (value_x, value_y) = find_text(&buffer, "£820.00_").unwrap();
        let value = &buffer[(value_x, value_y)];
        assert_eq!(value.fg, Color::Rgb(0xff, 0xc6, 0x6d));
        assert_eq!(value.bg, Color::Rgb(0x26, 0x26, 0x26));
        assert!(value.modifier.contains(Modifier::BOLD));
        assert!(!value.modifier.contains(Modifier::UNDERLINED));

        let (title_x, title_y) = find_text(&buffer, "Savings Pots").unwrap();
        let title = &buffer[(title_x, title_y)];
        assert_eq!(title.fg, Color::Rgb(0xff, 0xc6, 0x6d));
        assert!(title.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn navigation_selected_row_does_not_use_underlined_modifier() {
        let config = AppConfig::default_mvp();
        let buffer = draw_route(&navigation_route(&config), Some(&config), 105, 48);

        let (month_x, month_y) = find_text(&buffer, "August 2026").unwrap();
        let month = &buffer[(month_x, month_y)];
        assert!(month.modifier.contains(Modifier::BOLD));
        assert!(!month.modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn editor_render_uses_dark_neutral_section_surfaces_in_wide_layout() {
        let config = AppConfig::default_mvp();
        let buffer = draw_route(&editor_route(&config), Some(&config), 210, 48);

        let (accounts_x, accounts_y) = find_text(&buffer, "Entered").unwrap();
        let accounts = &buffer[(accounts_x, accounts_y)];
        assert_eq!(accounts.bg, Color::Rgb(0x26, 0x26, 0x26));

        let (timing_x, timing_y) = find_text(&buffer, "Effect").unwrap();
        let timing = &buffer[(timing_x, timing_y)];
        assert_eq!(timing.bg, Color::Rgb(0x26, 0x26, 0x26));

        let (pots_x, pots_y) = find_text(&buffer, "Final").unwrap();
        let pots = &buffer[(pots_x, pots_y)];
        assert_eq!(pots.bg, Color::Rgb(0x26, 0x26, 0x26));
    }

    #[test]
    fn editor_render_can_use_a_project_theme_override() {
        let config = AppConfig::default_mvp();
        let theme = UiTheme::from_palette(&Base24PaletteConfig {
            base00: "#101112".to_owned(),
            base01: "#343434".to_owned(),
            base02: "#535353".to_owned(),
            base03: "#797979".to_owned(),
            base04: "#a0a09f".to_owned(),
            base05: "#c7c7c5".to_owned(),
            base06: "#eeeeec".to_owned(),
            base07: "#ffffff".to_owned(),
            base08: "#d25151".to_owned(),
            base09: "#ffc66d".to_owned(),
            base0_a: "#8ab7d9".to_owned(),
            base0_b: "#11ee44".to_owned(),
            base0_c: "#bed6ff".to_owned(),
            base0_d: "#6c99bb".to_owned(),
            base0_e: "#d197d9".to_owned(),
            base0_f: "#692828".to_owned(),
        });

        let buffer = draw_route_with_theme(&editor_route(&config), Some(&config), 105, 48, &theme);

        let (title_x, title_y) = find_text(&buffer, "Savings Pots").unwrap();
        let title = &buffer[(title_x, title_y)];
        assert_eq!(title.fg, Color::Rgb(0x11, 0xee, 0x44));

        let (input_x, input_y) = find_text(&buffer, "£820.00").unwrap();
        let input = &buffer[(input_x, input_y)];
        assert_eq!(input.bg, Color::Rgb(0x10, 0x11, 0x12));
    }

    #[test]
    fn bundled_project_theme_is_valid() {
        let theme = UiTheme::project_default();
        assert_eq!(theme.base00, Color::Rgb(0x26, 0x26, 0x26));
        assert_eq!(theme.base0b, Color::Rgb(0xa5, 0xc2, 0x61));
    }

    #[test]
    fn editor_render_separates_liability_cues_from_validation_failure() {
        let config = AppConfig::default_mvp();
        let buffer = draw_route(&editor_route(&config), Some(&config), 105, 48);

        let (minus_x, minus_y) = find_text(&buffer, "-£204.00").unwrap();
        let liability = &buffer[(minus_x, minus_y)];
        assert_eq!(liability.fg, Color::Rgb(0xd1, 0x97, 0xd9));

        let (validation_x, validation_y) = find_text(&buffer, "outside tolerance").unwrap();
        let validation = &buffer[(validation_x, validation_y)];
        assert_eq!(validation.fg, Color::Rgb(0xd2, 0x51, 0x51));

        let (sync_x, sync_y) = find_text(&buffer, "synced").unwrap();
        let sync = &buffer[(sync_x, sync_y)];
        assert_eq!(sync.fg, Color::Rgb(0x8a, 0xb7, 0xd9));
    }

    #[test]
    fn editor_large_layout_keeps_section_titles_and_omits_titular_column_headers() {
        let config = AppConfig::default_mvp();
        let route = editor_route(&config);

        for (width, height) in [(105, 48), (210, 48)] {
            let rendered = buffer_to_string(draw_route(&route, Some(&config), width, height));
            assert!(rendered.contains("Accounts"));
            assert!(rendered.contains("Timing Adjustments"));
            assert!(rendered.contains("Next Month Earmarks"));
            assert!(rendered.contains("Savings Pots"));
            assert!(!rendered.contains("│Account"));
            assert!(!rendered.contains("│Adjustment"));
            assert!(!rendered.contains("│Earmark"));
            assert!(!rendered.contains("│Pot"));
        }
    }

    #[test]
    fn guided_snapshot() {
        let config = AppConfig::default_mvp();
        let route = guided_route(&config);

        insta::assert_snapshot!(buffer_to_string(draw_route(&route, Some(&config), 120, 40)));
    }

    #[test]
    fn guided_snapshot_80x24() {
        let config = AppConfig::default_mvp();
        let route = guided_route(&config);
        insta::assert_snapshot!(buffer_to_string(draw_route(&route, Some(&config), 80, 24)));
    }

    #[test]
    fn guided_snapshot_105x48() {
        let config = AppConfig::default_mvp();
        let route = guided_route(&config);
        insta::assert_snapshot!(buffer_to_string(draw_route(&route, Some(&config), 105, 48)));
    }

    #[test]
    fn guided_snapshot_210x48() {
        let config = AppConfig::default_mvp();
        let route = guided_route(&config);
        insta::assert_snapshot!(buffer_to_string(draw_route(&route, Some(&config), 210, 48)));
    }

    #[test]
    fn guided_render_omits_redundant_draft_status_copy_and_uses_compact_preview() {
        let config = AppConfig::default_mvp();
        let route = guided_route(&config);
        let rendered = buffer_to_string(draw_route(&route, Some(&config), 80, 24));

        assert!(!rendered.contains("Draft created and synced"));
        assert!(!rendered.contains("Draft autosaved and synced"));
        assert!(!rendered.contains("Subscriptions:"));
        assert!(!rendered.contains("Cash ISA:"));
        assert!(!rendered.contains("Type digits or decimals, then press Enter to autosave."));
        assert!(!rendered.contains("Validation: outside tolerance"));
        assert!(rendered.contains("Amount"));
        assert!(rendered.contains("Next: General spending"));
        assert!(rendered.contains("Earmarks £505.00  |  Diff -£745.00"));
    }

    #[test]
    fn guided_render_gives_current_input_more_focus_than_preview_context() {
        let config = AppConfig::default_mvp();
        let route = guided_route(&config);
        let input = match &route {
            Route::GuidedCreation(state) => state.input.display_text(),
            _ => unreachable!(),
        };
        let buffer = draw_route(&route, Some(&config), 105, 48);

        let (input_x, input_y) = find_text(&buffer, &input).unwrap();
        let input_cell = &buffer[(input_x, input_y)];
        assert_eq!(input_cell.fg, Color::Rgb(0xee, 0xee, 0xec));
        assert_eq!(input_cell.bg, Color::Rgb(0x26, 0x26, 0x26));

        let (preview_x, preview_y) = find_text(&buffer, "Accounts").unwrap();
        let preview = &buffer[(preview_x, preview_y)];
        assert_eq!(preview.bg, Color::Rgb(0x26, 0x26, 0x26));
    }

    #[test]
    fn guided_status_prioritises_validation_above_operational_metadata() {
        let config = AppConfig::default_mvp();
        let buffer = draw_route(&guided_route(&config), Some(&config), 105, 48);

        let (_, validation_y) = find_text(&buffer, "outside tolerance").unwrap();
        let (_, persistence_y) = find_text(&buffer, "Persistence").unwrap();
        assert!(validation_y < persistence_y);
    }

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

    fn editor_route(config: &AppConfig) -> Route {
        editor_route_with_state(config, 8, InteractionState::SheetIdle)
    }

    fn editor_route_with_state(
        config: &AppConfig,
        focus_index: usize,
        interaction: InteractionState,
    ) -> Route {
        let mut document =
            MonthDocument::new_draft(MonthId::parse("2026-04").unwrap(), config, None);
        document.accounts.insert("current".to_owned(), 250_000);
        document.accounts.insert("cash_isa".to_owned(), 35_500);
        document.accounts.insert("amex_credit".to_owned(), 20_400);
        document
            .accounts
            .insert("nationwide_credit".to_owned(), 6_000);
        document
            .next_month_earmarks
            .insert("subscriptions".to_owned(), 12_500);
        document
            .next_month_earmarks
            .insert("general_spending".to_owned(), 39_900);
        document.savings_pots.insert(
            "fun_expensive_stuff".to_owned(),
            SavingsPotState {
                carried_over: 82_000,
                monthly_change: -2_500,
            },
        );
        document.savings_pots.insert(
            "long_term_savings".to_owned(),
            SavingsPotState {
                carried_over: 15_500,
                monthly_change: 6_000,
            },
        );
        document.savings_pots.insert(
            "label".to_owned(),
            SavingsPotState {
                carried_over: 2_500,
                monthly_change: 25_000,
            },
        );
        let calculated = calculate_month(config, &document).unwrap();
        let fields = FieldId::editor_fields(config);
        let edit_buffer = (interaction == InteractionState::FieldEditing)
            .then(|| MoneyInput::from_field(&fields[focus_index], &document));
        Route::MonthEditing(EditorState {
            document,
            calculated,
            fields,
            focus_index,
            edit_buffer,
            message: Some("Month autosaved and synced".to_owned()),
            interaction,
            persistence: PersistenceState::Clean,
            sync: SyncState::Synced,
        })
    }

    fn navigation_route(config: &AppConfig) -> Route {
        let mut document =
            MonthDocument::new_draft(MonthId::parse("2026-08").unwrap(), config, None);
        document.accounts.insert("current".to_owned(), 250_000);
        document.accounts.insert("cash_isa".to_owned(), 35_500);
        document.accounts.insert("amex_credit".to_owned(), 20_400);
        document
            .accounts
            .insert("nationwide_credit".to_owned(), 6_000);
        document
            .next_month_earmarks
            .insert("subscriptions".to_owned(), 2_500);
        document
            .next_month_earmarks
            .insert("general_spending".to_owned(), 25_000);
        document.savings_pots.insert(
            "fun_expensive_stuff".to_owned(),
            SavingsPotState {
                carried_over: 13_000,
                monthly_change: -2_500,
            },
        );
        document.savings_pots.insert(
            "long_term_savings".to_owned(),
            SavingsPotState {
                carried_over: 37_500,
                monthly_change: 15_500,
            },
        );
        document.savings_pots.insert(
            "label".to_owned(),
            SavingsPotState {
                carried_over: 82_000,
                monthly_change: 6_000,
            },
        );
        document.meta.updated_at = Some("2026-03-31T14:30:06.464168849Z".to_owned());
        let calculated = calculate_month(config, &document).unwrap();
        Route::Navigation(NavigationState {
            months: vec![MonthEntry {
                document,
                calculated,
            }],
            selected: 0,
            dialog: None,
        })
    }

    fn guided_route(config: &AppConfig) -> Route {
        let document = MonthDocument::new_draft(MonthId::parse("2026-05").unwrap(), config, None);
        let calculated = calculate_month(config, &document).unwrap();
        let steps = FieldId::guided_steps(config);
        let step_index = steps
            .iter()
            .position(|field| matches!(field, FieldId::Earmark(id) if id == "subscriptions"))
            .unwrap();
        let current_step = steps[step_index].clone();
        Route::GuidedCreation(GuidedCreationState {
            document: document.clone(),
            calculated,
            steps,
            step_index,
            input: MoneyInput::from_field(&current_step, &document),
            message: Some("Draft created and synced".to_owned()),
            persistence: PersistenceState::Clean,
            sync: SyncState::Synced,
        })
    }

    fn draw_route(route: &Route, config: Option<&AppConfig>, width: u16, height: u16) -> Buffer {
        draw_route_with_theme(route, config, width, height, UiTheme::project_default())
    }

    fn draw_route_with_theme(
        route: &Route,
        config: Option<&AppConfig>,
        width: u16,
        height: u16,
        theme: &UiTheme,
    ) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_with_theme(frame, route, Path::new("/tmp/budget"), config, theme))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn buffer_to_string(buffer: Buffer) -> String {
        let mut output = String::new();
        for y in 0..buffer.area.height {
            let mut line = String::new();
            for x in 0..buffer.area.width {
                line.push_str(buffer[(x, y)].symbol());
            }
            output.push_str(line.trim_end());
            output.push('\n');
        }
        output
    }

    fn find_text(buffer: &Buffer, text: &str) -> Option<(u16, u16)> {
        let symbols = text
            .chars()
            .map(|character| character.to_string())
            .collect::<Vec<_>>();
        let width = symbols.len() as u16;
        if width == 0 || width > buffer.area.width {
            return None;
        }

        for y in 0..buffer.area.height {
            for x in 0..=buffer.area.width - width {
                let matches = symbols
                    .iter()
                    .enumerate()
                    .all(|(offset, symbol)| buffer[(x + offset as u16, y)].symbol() == symbol);
                if matches {
                    return Some((x, y));
                }
            }
        }

        None
    }
}
