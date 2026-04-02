use std::path::Path;

use budget_core::{CalculatedMonth, MonthDocument};
use ratatui::prelude::{Alignment, Line, Span, Style, Text};
use ratatui::widgets::{Block, Borders, Cell};

use super::layout::PanelChrome;
use super::theme::{
    Tone, UiTheme, key_hint_spans, metric_spans, operational_status_style, validation_tone,
};
use crate::state::{
    EditorState, FieldId, InteractionState, PersistenceState, SectionId, SyncState,
};

/// Visual focus state for editor rows and cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EditorFocusState {
    Unfocused,
    Selected,
    Editing,
}

pub(super) fn selected_field(state: &EditorState) -> Option<&FieldId> {
    state.fields.get(state.focus_index)
}

pub(super) fn field_focus_state(state: &EditorState, field: &FieldId) -> EditorFocusState {
    if selected_field(state).is_none_or(|selected| selected != field) {
        return EditorFocusState::Unfocused;
    }
    match state.interaction {
        InteractionState::SheetIdle => EditorFocusState::Selected,
        InteractionState::FieldEditing => EditorFocusState::Editing,
    }
}

pub(super) fn section_focus_state(state: &EditorState, section: SectionId) -> EditorFocusState {
    selected_field(state)
        .filter(|field| field.section() == section)
        .map(|field| field_focus_state(state, field))
        .unwrap_or(EditorFocusState::Unfocused)
}

pub(super) fn combined_focus_state(
    left: EditorFocusState,
    right: EditorFocusState,
) -> EditorFocusState {
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

pub(super) fn value_for_field(
    state: &EditorState,
    field: &FieldId,
    document: &MonthDocument,
) -> String {
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

pub(super) fn styled_value_cell_with_tone(
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

pub(super) fn labeled_row_cell(label: &str, focus: EditorFocusState) -> Cell<'static> {
    Cell::from(format!("{}{}", focus_marker(focus), label))
}

fn focus_marker(focus: EditorFocusState) -> &'static str {
    match focus {
        EditorFocusState::Unfocused => "  ",
        EditorFocusState::Selected => "› ",
        EditorFocusState::Editing => "✎ ",
    }
}

pub(super) fn focused_row_style(focus: EditorFocusState, theme: &UiTheme) -> Style {
    match focus {
        EditorFocusState::Unfocused => Style::default(),
        EditorFocusState::Selected => theme.selected_style(),
        EditorFocusState::Editing => theme.selected_style(),
    }
}

fn amount_cell(value: String) -> Cell<'static> {
    Cell::from(Line::from(value).alignment(Alignment::Right))
}

pub(super) fn amount_cell_with_style(value: String, style: Style) -> Cell<'static> {
    amount_cell(value).style(style)
}

pub(super) fn status_line(
    persistence: PersistenceState,
    sync: SyncState,
    theme: &UiTheme,
) -> Line<'static> {
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

pub(super) fn hint_lines(width: u16, hints: &[&str], theme: &UiTheme) -> Vec<Line<'static>> {
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

/// Shortens a path from the left so the tail remains visible in narrow headers.
pub(super) fn abbreviate_path(path: &Path, max_width: usize) -> String {
    let text = path.display().to_string();
    if text.len() <= max_width || max_width <= 3 {
        return text;
    }
    format!("...{}", &text[text.len() - (max_width - 3)..])
}

pub(super) fn panel_block(
    title: &str,
    chrome: PanelChrome,
    tone: Tone,
    theme: &UiTheme,
) -> Block<'static> {
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

pub(super) fn section_block(
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

pub(super) fn compact_summary_text(
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

    // Wrap on whole metrics so compact summaries stay scan-friendly instead of
    // splitting labels and values across lines.
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

/// Formats stored RFC3339 timestamps for narrow navigation columns.
pub(super) fn format_updated_timestamp(updated_at: Option<&str>) -> String {
    let Some(updated_at) = updated_at else {
        return "-".to_owned();
    };
    if updated_at.len() >= 16 && updated_at.as_bytes().get(10) == Some(&b'T') {
        return format!("{} | {}", &updated_at[..10], &updated_at[11..16]);
    }
    updated_at.to_owned()
}

/// Distinguishes transient guided autosave messages from validation feedback.
pub(super) fn is_guided_status_message(message: &str) -> bool {
    matches!(
        message,
        message if message.starts_with("Draft created")
            || message.starts_with("Draft autosaved")
            || message.starts_with("Autosaving draft")
    )
}
