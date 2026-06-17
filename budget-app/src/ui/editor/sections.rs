use super::section_table::{SectionTableSpec, render_section_table};
use super::*;

pub(super) fn render_accounts(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &EditorState,
    compact: bool,
    show_title: bool,
    chrome: PanelChrome,
    theme: &UiTheme,
) {
    let rows = state.calculated.account_rows.iter().map(|row| {
        let focus = field_focus_state_matching(
            state,
            |field| matches!(field, FieldId::Account(id) if id == &row.id),
        );
        Row::new(vec![
            labelled_row_cell(&row.label, focus),
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
                value_for_current(state, focus, || row.raw_balance.format()),
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
    render_section_table(
        frame,
        area,
        rows,
        SectionTableSpec {
            widths: if compact {
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
            header: Row::new(vec![
                Cell::from(if show_title { "" } else { "Account" }),
                Cell::from(""),
                Cell::from("Entered"),
                Cell::from("Net"),
            ]),
            title: show_title.then_some("Accounts"),
            subtotal: format!(
                "Subtotal {}",
                state.calculated.totals.accounts_subtotal.format()
            ),
            focus: section_focus_state(state, SectionId::Accounts),
            tone: Tone::Accounts,
            chrome,
        },
        theme,
    );
}

pub(super) fn render_timing(
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
    let correction_focus = field_focus_state_matching(state, |field| field == &correction);
    let investment_focus = field_focus_state_matching(state, |field| field == &investment);
    let rows = vec![
        Row::new(vec![
            labelled_row_cell("General spending over/under", correction_focus),
            styled_value_cell_with_tone(
                value_for_current(state, correction_focus, || {
                    state
                        .calculated
                        .timing
                        .previous_month_spending_correction_raw
                        .format()
                }),
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
            labelled_row_cell("Investment not yet sent", investment_focus),
            styled_value_cell_with_tone(
                value_for_current(state, investment_focus, || {
                    state.calculated.timing.investment_not_yet_sent_raw.format()
                }),
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
    render_section_table(
        frame,
        area,
        rows,
        SectionTableSpec {
            widths: [
                Constraint::Min(if compact { 20 } else { 28 }),
                Constraint::Length(14),
                Constraint::Length(14),
            ],
            header: Row::new(vec![
                Cell::from(if show_title { "" } else { "Adjustment" }),
                Cell::from("Entered"),
                Cell::from("Effect"),
            ]),
            title: show_title.then_some("Timing Adjustments"),
            subtotal: format!(
                "Subtotal {}",
                state.calculated.totals.timing_adjustments_subtotal.format()
            ),
            focus: section_focus_state(state, SectionId::TimingAdjustments),
            tone: Tone::Timing,
            chrome,
        },
        theme,
    );
}

pub(super) fn render_earmarks(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &EditorState,
    compact: bool,
    show_title: bool,
    chrome: PanelChrome,
    theme: &UiTheme,
) {
    let rows = state.calculated.earmark_rows.iter().map(|row| {
        let focus = field_focus_state_matching(
            state,
            |field| matches!(field, FieldId::Earmark(id) if id == &row.id),
        );
        Row::new(vec![
            labelled_row_cell(&row.label, focus),
            styled_value_cell_with_tone(
                value_for_current(state, focus, || row.amount.format()),
                focus,
                Tone::Earmarks,
                theme,
            ),
        ])
        .style(focused_row_style(focus, theme))
    });
    render_section_table(
        frame,
        area,
        rows,
        SectionTableSpec {
            widths: [
                Constraint::Min(if compact { 18 } else { 24 }),
                Constraint::Length(14),
            ],
            header: Row::new(vec![
                Cell::from(if show_title { "" } else { "Earmark" }),
                Cell::from("Amount"),
            ]),
            title: show_title.then_some("Next Month Earmarks"),
            subtotal: format!(
                "Subtotal {}",
                state
                    .calculated
                    .totals
                    .next_month_earmarks_subtotal
                    .format()
            ),
            focus: section_focus_state(state, SectionId::NextMonthEarmarks),
            tone: Tone::Earmarks,
            chrome,
        },
        theme,
    );
}

pub(super) fn render_pots(
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
            let carried_focus = field_focus_state_matching(
                state,
                |field| matches!(field, FieldId::PotCarried(id) if id == &row.id),
            );
            let change_focus = field_focus_state_matching(
                state,
                |field| matches!(field, FieldId::PotChange(id) if id == &row.id),
            );
            let row_focus = combined_focus_state(carried_focus, change_focus);
            Row::new(vec![
                labelled_row_cell(&row.label, row_focus),
                styled_value_cell_with_tone(
                    value_for_current(state, carried_focus, || row.carried_over.format()),
                    carried_focus,
                    Tone::Pots,
                    theme,
                ),
                styled_value_cell_with_tone(
                    value_for_current(state, change_focus, || row.monthly_change.format()),
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
    rows.push(pots_total_row(state, theme));

    render_section_table(
        frame,
        area,
        rows,
        SectionTableSpec {
            widths: if compact {
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
            header: Row::new(vec![
                Cell::from(if show_title { "" } else { "Pot" }),
                Cell::from("Carried"),
                Cell::from("Change"),
                Cell::from("Final"),
            ]),
            title: show_title.then_some("Savings Pots"),
            subtotal: format!(
                "Subtotal {}",
                state.calculated.totals.pots_final_total.format()
            ),
            focus: section_focus_state(state, SectionId::SavingsPots),
            tone: Tone::Pots,
            chrome,
        },
        theme,
    );
}

fn pots_total_row(state: &EditorState, theme: &UiTheme) -> Row<'static> {
    // Keep the totals row in the same table so compact layouts do not need a
    // separate summary widget for carried, change, and final balances.
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
    .style(theme.bright_style().add_modifier(Modifier::BOLD))
}
