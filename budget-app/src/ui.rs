use std::path::Path;

use budget_core::{AccountKind, AppConfig, CalculatedMonth, MonthDocument};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap};

use crate::state::{
    DeleteDialog, EditorState, FailureState, FieldId, GuidedCreationState, InteractionState,
    NavigationDialog, NavigationState, PersistenceState, Route, SectionId, SyncState,
};

pub fn render(frame: &mut Frame<'_>, route: &Route, repo_root: &Path, config: Option<&AppConfig>) {
    match route {
        Route::Navigation(state) => render_navigation(frame, state, repo_root),
        Route::GuidedCreation(state) => {
            if let Some(config) = config {
                render_guided_creation(frame, state, config);
            }
        }
        Route::MonthEditing(state) => {
            if let Some(config) = config {
                render_editor(frame, state, config);
            }
        }
        Route::BlockingFailure(state) => render_failure(frame, state),
        Route::Shutdown => {}
    }
}

fn render_navigation(frame: &mut Frame<'_>, state: &NavigationState, repo_root: &Path) {
    let compact = frame.area().width < 110 || frame.area().height < 28;
    let mut header_lines = vec![Line::from(format!(
        "Repo: {}",
        abbreviate_path(repo_root, frame.area().width.saturating_sub(8) as usize)
    ))];
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
    ));
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_lines.len() as u16 + 2),
            Constraint::Min(8),
        ])
        .split(frame.area());

    let header = Paragraph::new(header_lines)
        .block(Block::default().borders(Borders::ALL).title("Navigation"));
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
            Cell::from(entry.document.month.display_label()),
            Cell::from(status.to_owned()),
            amount_cell(entry.calculated.validation.overall_difference.format()),
            Cell::from(format_updated_timestamp(
                entry.document.meta.updated_at.as_deref(),
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
            Cell::from("Month"),
            Cell::from("State"),
            Cell::from("Diff"),
            Cell::from("Updated"),
        ])
        .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().borders(Borders::ALL))
    .row_highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White));
    let mut table_state = TableState::default();
    if !state.months.is_empty() {
        table_state.select(Some(state.selected));
    }
    frame.render_stateful_widget(months_table, body[0], &mut table_state);

    let summary_text = if let Some(entry) = state.selected_month() {
        compact_summary_text(&entry.calculated, body[1].width)
    } else {
        Text::from("No months yet.\nPress `n` to create the first month.")
    };
    let summary = Paragraph::new(summary_text)
        .block(Block::default().borders(Borders::ALL).title("Summary"))
        .wrap(Wrap { trim: false });
    frame.render_widget(summary, body[1]);

    if let Some(dialog) = &state.dialog {
        render_navigation_dialog(frame, dialog);
    }
}

fn render_navigation_dialog(frame: &mut Frame<'_>, dialog: &NavigationDialog) {
    let area = centered_rect(68, 36, frame.area());
    frame.render_widget(Clear, area);
    match dialog {
        NavigationDialog::Create(dialog) => {
            render_dialog(
                frame,
                area,
                "New Month",
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
            );
        }
        NavigationDialog::Rename(dialog) => {
            render_dialog(
                frame,
                area,
                "Rename Month",
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
            );
        }
        NavigationDialog::Delete(dialog) => {
            render_delete_dialog(frame, area, dialog);
        }
    }
}

fn render_delete_dialog(frame: &mut Frame<'_>, area: Rect, dialog: &DeleteDialog) {
    render_dialog(
        frame,
        area,
        "Delete Month",
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
    );
}

fn render_dialog(frame: &mut Frame<'_>, area: Rect, title: &str, lines: &[String]) {
    let text = Text::from(lines.iter().cloned().map(Line::from).collect::<Vec<_>>());
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_guided_creation(frame: &mut Frame<'_>, state: &GuidedCreationState, config: &AppConfig) {
    let compact = frame.area().width < 110 || frame.area().height < 28;
    let mut header_lines = vec![Line::from(format!(
        "{}  |  Step {}/{}",
        state.document.month.display_label(),
        state.step_index + 1,
        state.steps.len()
    ))];
    header_lines.extend(hint_lines(
        frame.area().width,
        &[
            "Type amount",
            "Backspace delete",
            "Enter save step",
            "Esc months",
            "q quit",
        ],
    ));
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_lines.len() as u16 + 2),
            Constraint::Min(8),
            Constraint::Length(if compact { 4 } else { 5 }),
        ])
        .split(frame.area());

    let header = Paragraph::new(header_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Guided Creation"),
    );
    frame.render_widget(header, layout[0]);

    let body = if compact {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(10), Constraint::Min(8)])
            .split(layout[1])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(layout[1])
    };

    let current_step = &state.steps[state.step_index];
    let mut step_lines = vec![
        Line::from(current_step.label(config)),
        Line::from(""),
        Line::from("Type digits or decimals, then press Enter to autosave."),
        Line::from(""),
        Line::from(format!(
            "Input: {}{}",
            state.input.display_text(),
            if state.input.is_edited() { "_" } else { "" }
        )),
    ];
    if let Some(message) = state
        .message
        .as_deref()
        .filter(|message| !is_guided_status_message(message))
    {
        step_lines.push(Line::from(""));
        step_lines.push(Line::from(message.to_owned()));
    }
    step_lines.push(Line::from(""));
    step_lines.push(Line::from("Next steps:"));
    for step in state
        .steps
        .iter()
        .skip(state.step_index + 1)
        .take(if compact { 3 } else { 5 })
    {
        step_lines.push(Line::from(format!("• {}", step.label(config))));
    }
    frame.render_widget(
        Paragraph::new(step_lines)
            .block(Block::default().borders(Borders::ALL).title("Current Step"))
            .wrap(Wrap { trim: false }),
        body[0],
    );

    let preview = Paragraph::new(compact_summary_text(&state.calculated, body[1].width))
        .block(Block::default().borders(Borders::ALL).title("Live Preview"))
        .wrap(Wrap { trim: false });
    frame.render_widget(preview, body[1]);

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(status_line(state.persistence, state.sync)),
            Line::from(format!(
                "Validation: {}  |  Difference: {}",
                if state.calculated.validation.is_valid {
                    "within tolerance"
                } else {
                    "outside tolerance"
                },
                state.calculated.validation.overall_difference.format()
            )),
            Line::from("The draft is saved as you confirm each guided step."),
        ])
        .block(Block::default().borders(Borders::ALL).title("Status"))
        .wrap(Wrap { trim: false }),
        layout[2],
    );
}

fn render_editor(frame: &mut Frame<'_>, state: &EditorState, config: &AppConfig) {
    let profile = EditorLayoutProfile::for_area(frame.area());
    let mut header_lines = vec![Line::from(state.document.month.display_label())];
    header_lines.extend(hint_lines(
        frame.area().width,
        if profile == EditorLayoutProfile::Compact {
            &["Enter edit", "Tab next", "Esc months", "q quit"]
        } else {
            &["Enter edit", "Tab/Shift-Tab move", "Esc months", "q quit"]
        },
    ));
    let footer_height = 4;
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_lines.len() as u16 + 2),
            Constraint::Min(10),
            Constraint::Length(footer_height),
        ])
        .split(frame.area());

    let header = Paragraph::new(header_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Monthly Sheet"),
    );
    frame.render_widget(header, layout[0]);

    match profile {
        EditorLayoutProfile::Wide => render_editor_wide(frame, layout[1], state, config),
        EditorLayoutProfile::Standard => render_editor_standard(frame, layout[1], state, config),
        EditorLayoutProfile::Compact => render_editor_compact(frame, layout[1], state, config),
    }

    render_editor_footer(
        frame,
        layout[2],
        state,
        config,
        profile == EditorLayoutProfile::Compact,
    );
}

fn render_editor_wide(frame: &mut Frame<'_>, area: Rect, state: &EditorState, config: &AppConfig) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(54), Constraint::Percentage(46)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(section_height(state.calculated.account_rows.len(), false)),
            Constraint::Length(section_height(2, false)),
            Constraint::Min(section_height(state.calculated.earmark_rows.len(), false)),
        ])
        .split(columns[0]);

    render_accounts(frame, left[0], state, config, false, false);
    render_timing(frame, left[1], state, false, false);
    render_earmarks(frame, left[2], state, false, false);
    render_pots(frame, columns[1], state, false, false);
}

fn render_editor_standard(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &EditorState,
    config: &AppConfig,
) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(section_height(state.calculated.account_rows.len(), false)),
            Constraint::Length(section_height(2, true)),
            Constraint::Length(section_height(state.calculated.earmark_rows.len(), true)),
            Constraint::Min(section_height(state.calculated.pot_rows.len() + 2, false)),
        ])
        .split(area);
    render_accounts(frame, rows[0], state, config, false, false);
    render_timing(frame, rows[1], state, true, false);
    render_earmarks(frame, rows[2], state, true, false);
    render_pots(frame, rows[3], state, false, false);
}

fn render_editor_compact(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &EditorState,
    config: &AppConfig,
) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(7)])
        .split(area);
    let selected_section = selected_field(state)
        .map(FieldId::section)
        .unwrap_or(SectionId::Accounts);
    render_section_tabs(frame, rows[0], selected_section);
    match selected_section {
        SectionId::Accounts => render_accounts(frame, rows[1], state, config, true, false),
        SectionId::TimingAdjustments => render_timing(frame, rows[1], state, true, false),
        SectionId::NextMonthEarmarks => render_earmarks(frame, rows[1], state, true, false),
        SectionId::SavingsPots => render_pots(frame, rows[1], state, true, false),
    }
}

fn render_section_tabs(frame: &mut Frame<'_>, area: Rect, selected: SectionId) {
    let mut spans = Vec::new();
    for section in SectionId::ALL {
        if !spans.is_empty() {
            spans.push(Span::raw(" | "));
        }
        spans.push(Span::styled(
            section.compact_title(),
            if section == selected {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::ALL).title("Sections")),
        area,
    );
}

fn render_editor_footer(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &EditorState,
    _config: &AppConfig,
    _compact: bool,
) {
    let lines = vec![
        Line::from(format!(
            "Overall difference: {}",
            state.calculated.validation.overall_difference.format()
        )),
        Line::from(format!(
            "Status: {}  |  {}",
            if state.calculated.validation.is_valid {
                "valid"
            } else {
                "invalid"
            },
            status_line(state.persistence, state.sync)
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Validation"))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_failure(frame: &mut Frame<'_>, state: &FailureState) {
    let area = centered_rect(70, 40, frame.area());
    frame.render_widget(Clear, area);
    let lines = vec![
        Line::from(state.title.clone()),
        Line::from(""),
        Line::from(state.message.clone()),
        Line::from(""),
        Line::from("r retry"),
        Line::from("q quit"),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Blocking Failure"),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_accounts(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &EditorState,
    _config: &AppConfig,
    compact: bool,
    show_title: bool,
) {
    let rows = state.calculated.account_rows.iter().map(|row| {
        let field = FieldId::Account(row.id.clone());
        Row::new(vec![
            Cell::from(row.label.clone()),
            Cell::from(match row.kind {
                AccountKind::Asset => "+",
                AccountKind::Liability => "-",
            }),
            styled_value_cell(
                value_for_field(state, &field, &state.document),
                is_selected(state, &field),
            ),
            amount_cell(row.normalised_balance.format()),
        ])
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
            Cell::from("Account"),
            Cell::from(""),
            Cell::from("Entered"),
            Cell::from("Net"),
        ])
        .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(section_block(
        show_title.then_some("Accounts"),
        format!(
            "Subtotal {}",
            state.calculated.totals.accounts_subtotal.format()
        ),
    ));
    frame.render_widget(table, area);
}

fn render_timing(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &EditorState,
    compact: bool,
    show_title: bool,
) {
    let correction = FieldId::PreviousMonthSpendingCorrection;
    let investment = FieldId::InvestmentNotYetSent;
    let rows = vec![
        Row::new(vec![
            Cell::from("General spending over/under"),
            styled_value_cell(
                value_for_field(state, &correction, &state.document),
                is_selected(state, &correction),
            ),
            amount_cell(
                state
                    .calculated
                    .timing
                    .previous_month_spending_correction_effect
                    .format(),
            ),
        ]),
        Row::new(vec![
            Cell::from("Investment not yet sent"),
            styled_value_cell(
                value_for_field(state, &investment, &state.document),
                is_selected(state, &investment),
            ),
            amount_cell(state.calculated.timing.investment_effect.format()),
        ]),
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
            Cell::from("Adjustment"),
            Cell::from("Entered"),
            Cell::from("Effect"),
        ])
        .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(section_block(
        show_title.then_some("Timing Adjustments"),
        format!(
            "Subtotal {}",
            state.calculated.totals.timing_adjustments_subtotal.format()
        ),
    ));
    frame.render_widget(table, area);
}

fn render_earmarks(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &EditorState,
    compact: bool,
    show_title: bool,
) {
    let rows = state.calculated.earmark_rows.iter().map(|row| {
        let field = FieldId::Earmark(row.id.clone());
        Row::new(vec![
            Cell::from(row.label.clone()),
            styled_value_cell(
                value_for_field(state, &field, &state.document),
                is_selected(state, &field),
            ),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Min(if compact { 18 } else { 24 }),
            Constraint::Length(14),
        ],
    )
    .header(
        Row::new(vec![Cell::from("Earmark"), Cell::from("Amount")])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
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
    ));
    frame.render_widget(table, area);
}

fn render_pots(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &EditorState,
    compact: bool,
    show_title: bool,
) {
    let mut rows = state
        .calculated
        .pot_rows
        .iter()
        .map(|row| {
            let carried = FieldId::PotCarried(row.id.clone());
            let change = FieldId::PotChange(row.id.clone());
            Row::new(vec![
                Cell::from(row.label.clone()),
                styled_value_cell(
                    value_for_field(state, &carried, &state.document),
                    is_selected(state, &carried),
                ),
                styled_value_cell(
                    value_for_field(state, &change, &state.document),
                    is_selected(state, &change),
                ),
                amount_cell(row.final_balance.format()),
            ])
        })
        .collect::<Vec<_>>();
    rows.push(
        Row::new(vec![
            Cell::from("Total"),
            amount_cell(state.calculated.totals.pots_carried_total.format()),
            amount_cell(state.calculated.totals.pots_monthly_change_total.format()),
            amount_cell(state.calculated.totals.pots_final_total.format()),
        ])
        .style(Style::default().add_modifier(Modifier::BOLD)),
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
            Cell::from("Pot"),
            Cell::from("Carried"),
            Cell::from("Change"),
            Cell::from("Final"),
        ])
        .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(section_block(
        show_title.then_some("Savings Pots"),
        format!(
            "Subtotal {}",
            state.calculated.totals.pots_final_total.format()
        ),
    ));
    frame.render_widget(table, area);
}

fn selected_field(state: &EditorState) -> Option<&FieldId> {
    state.fields.get(state.focus_index)
}

fn is_selected(state: &EditorState, field: &FieldId) -> bool {
    selected_field(state).is_some_and(|selected| selected == field)
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

fn styled_value_cell(value: String, selected: bool) -> Cell<'static> {
    let cell = amount_cell(value);
    if selected {
        cell.style(Style::default().bg(Color::DarkGray).fg(Color::White))
    } else {
        cell
    }
}

fn amount_cell(value: String) -> Cell<'static> {
    Cell::from(Line::from(value).alignment(Alignment::Right))
}

fn section_height(row_count: usize, compact_title: bool) -> u16 {
    let base = row_count as u16 + if compact_title { 3 } else { 4 };
    base.max(6)
}

fn status_line(persistence: PersistenceState, sync: SyncState) -> String {
    format!(
        "Persistence: {}  |  Sync: {}",
        match persistence {
            PersistenceState::Clean => "clean",
            PersistenceState::Dirty => "dirty",
            PersistenceState::Autosaving => "autosaving",
            PersistenceState::SaveFailed => "failed",
        },
        match sync {
            SyncState::SyncPending => "pending",
            SyncState::Syncing => "syncing",
            SyncState::Synced => "synced",
            SyncState::SyncFailed => "failed",
        }
    )
}

fn hint_lines(width: u16, hints: &[&str]) -> Vec<Line<'static>> {
    let available = width.saturating_sub(4) as usize;
    let separator = " | ";
    let mut lines = Vec::new();
    let mut current = String::new();

    for hint in hints {
        let candidate_len = if current.is_empty() {
            hint.len()
        } else {
            current.len() + separator.len() + hint.len()
        };
        if !current.is_empty() && candidate_len > available {
            lines.push(Line::from(std::mem::take(&mut current)));
        }
        if !current.is_empty() {
            current.push_str(separator);
        }
        current.push_str(hint);
    }

    if !current.is_empty() {
        lines.push(Line::from(current));
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

fn section_block(title: Option<&str>, subtotal: String) -> Block<'static> {
    let mut block = Block::default().borders(Borders::ALL);
    if let Some(title) = title {
        block = block.title(Line::from(title.to_owned()));
    }
    block.title(Line::from(subtotal).alignment(Alignment::Right))
}

fn compact_summary_text(calculated: &CalculatedMonth, width: u16) -> Text<'static> {
    let metrics = vec![
        format!("Accounts {}", calculated.totals.accounts_subtotal.format()),
        format!(
            "Timing Adjustments {}",
            calculated.totals.timing_adjustments_subtotal.format()
        ),
        format!(
            "Next Month Earmarks {}",
            calculated.totals.next_month_earmarks_subtotal.format()
        ),
        format!(
            "Savings Pots {}",
            calculated.totals.pots_final_total.format()
        ),
        format!(
            "Total allocated {}",
            calculated.totals.total_allocated.format()
        ),
        format!(
            "Overall difference {}",
            calculated.validation.overall_difference.format()
        ),
        format!(
            "Status {}",
            if calculated.validation.is_valid {
                "valid"
            } else {
                "invalid"
            }
        ),
    ];
    Text::from(wrapped_metric_lines(width, &metrics))
}

fn wrapped_metric_lines(width: u16, metrics: &[String]) -> Vec<Line<'static>> {
    let available = width.saturating_sub(4) as usize;
    let separator = " | ";
    let mut lines = Vec::new();
    let mut current = String::new();

    for metric in metrics {
        let candidate_len = if current.is_empty() {
            metric.len()
        } else {
            current.len() + separator.len() + metric.len()
        };
        if !current.is_empty() && candidate_len > available {
            lines.push(Line::from(std::mem::take(&mut current)));
        }
        if !current.is_empty() {
            current.push_str(separator);
        }
        current.push_str(metric);
    }

    if !current.is_empty() {
        lines.push(Line::from(current));
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

    use super::render;
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
    fn editor_large_layout_omits_redundant_section_titles() {
        let config = AppConfig::default_mvp();
        let route = editor_route(&config);

        for (width, height) in [(105, 48), (210, 48)] {
            let rendered = buffer_to_string(draw_route(&route, Some(&config), width, height));
            assert!(!rendered.contains("Accounts"));
            assert!(!rendered.contains("Timing Adjustments"));
            assert!(!rendered.contains("Next Month Earmarks"));
            assert!(!rendered.contains("Savings Pots"));
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
        assert!(rendered.contains("Next Month Earmarks £505.00"));
        assert!(rendered.contains("Overall difference -£745.00"));
    }

    fn editor_route(config: &AppConfig) -> Route {
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
        Route::MonthEditing(EditorState {
            document,
            calculated,
            fields: FieldId::editor_fields(config),
            focus_index: 8,
            edit_buffer: None,
            message: Some("Month autosaved and synced".to_owned()),
            interaction: InteractionState::SheetIdle,
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
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, route, Path::new("/tmp/budget"), config))
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
}
