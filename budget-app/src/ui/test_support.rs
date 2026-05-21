use std::path::Path;

use budget_core::{AppConfig, MonthDocument, MonthId, SavingsPotState, calculate_month};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

use super::render_with_theme;
use super::theme::UiTheme;
use crate::state::{
    EditorState, FieldId, GuidedCreationState, InteractionState, MoneyInput, MonthEntry,
    NavigationState, PersistenceState, Route, SyncState,
};

pub(super) fn editor_route(config: &AppConfig) -> Route {
    editor_route_with_state(config, 8, InteractionState::SheetIdle)
}

pub(super) fn editor_route_with_state(
    config: &AppConfig,
    focus_index: usize,
    interaction: InteractionState,
) -> Route {
    let document = sample_month_document(config, "2026-04", |document| {
        document
            .next_month_earmarks
            .insert("subscriptions".to_owned(), 12_000);
        document
            .next_month_earmarks
            .insert("general_spending".to_owned(), 34_000);
        document.savings_pots.insert(
            "travel_fund".to_owned(),
            SavingsPotState {
                carried_over: 80_000,
                monthly_change: -5_000,
            },
        );
        document.savings_pots.insert(
            "home_upkeep".to_owned(),
            SavingsPotState {
                carried_over: 20_000,
                monthly_change: 5_000,
            },
        );
        document.savings_pots.insert(
            "emergency_buffer".to_owned(),
            SavingsPotState {
                carried_over: 10_000,
                monthly_change: 3_000,
            },
        );
    });
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

pub(super) fn navigation_route(config: &AppConfig) -> Route {
    let mut document = sample_month_document(config, "2026-08", |document| {
        document
            .next_month_earmarks
            .insert("subscriptions".to_owned(), 3_000);
        document
            .next_month_earmarks
            .insert("general_spending".to_owned(), 18_000);
        document.savings_pots.insert(
            "travel_fund".to_owned(),
            SavingsPotState {
                carried_over: 15_000,
                monthly_change: -5_000,
            },
        );
        document.savings_pots.insert(
            "home_upkeep".to_owned(),
            SavingsPotState {
                carried_over: 30_000,
                monthly_change: 10_000,
            },
        );
        document.savings_pots.insert(
            "emergency_buffer".to_owned(),
            SavingsPotState {
                carried_over: 50_000,
                monthly_change: 5_000,
            },
        );
    });
    document.meta.updated_at = Some("2026-03-31T14:30:06.464168849Z".to_owned());
    let calculated = calculate_month(config, &document).unwrap();
    Route::Navigation(NavigationState {
        months: vec![MonthEntry {
            document,
            calculated,
        }],
        selected: 0,
        dialogue: None,
    })
}

pub(super) fn guided_route(config: &AppConfig) -> Route {
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

fn sample_month_document(
    config: &AppConfig,
    month: &str,
    customise: impl FnOnce(&mut MonthDocument),
) -> MonthDocument {
    let mut document = MonthDocument::new_draft(MonthId::parse(month).unwrap(), config, None);
    document
        .accounts
        .insert("current_account".to_owned(), 250_000);
    document
        .accounts
        .insert("savings_account".to_owned(), 40_000);
    document.accounts.insert("credit_card_a".to_owned(), 20_000);
    document.accounts.insert("credit_card_b".to_owned(), 10_000);
    customise(&mut document);
    document
}

pub(super) fn draw_route(
    route: &Route,
    config: Option<&AppConfig>,
    width: u16,
    height: u16,
) -> Buffer {
    draw_route_with_theme(route, config, width, height, UiTheme::project_default())
}

pub(super) fn draw_route_with_theme(
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

pub(super) fn buffer_to_string(buffer: Buffer) -> String {
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

pub(super) fn find_text(buffer: &Buffer, text: &str) -> Option<(u16, u16)> {
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
