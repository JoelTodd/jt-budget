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
    let mut document = MonthDocument::new_draft(MonthId::parse("2026-04").unwrap(), config, None);
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

pub(super) fn navigation_route(config: &AppConfig) -> Route {
    let mut document = MonthDocument::new_draft(MonthId::parse("2026-08").unwrap(), config, None);
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
