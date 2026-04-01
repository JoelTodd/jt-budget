use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use budget_core::{CalculatedMonth, MonthDocument, MonthId, calculate_month};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tracing::info;

use crate::repository::{LoadedMonth, Repository};
use crate::state::{
    CreateDialog, DeleteDialog, EditorState, FailureState, FieldId, GuidedCreationState,
    InteractionState, MoneyInput, MonthEntry, NavigationDialog, NavigationState, PersistenceState,
    RenameDialog, RetryTarget, Route, SyncState,
};
use crate::ui;

type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

pub fn run(repo_root: PathBuf) -> Result<()> {
    let mut app = App::bootstrap(repo_root);
    let mut terminal = setup_terminal()?;
    let result = app.run_loop(&mut terminal);
    restore_terminal(&mut terminal)?;
    result
}

struct App {
    repo_root: PathBuf,
    repository: Option<Repository>,
    route: Route,
}

impl App {
    fn bootstrap(repo_root: PathBuf) -> Self {
        let mut app = Self {
            repo_root,
            repository: None,
            route: Route::Shutdown,
        };
        app.enter_repository_gate();
        app
    }

    fn run_loop(&mut self, terminal: &mut AppTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| {
                ui::render(
                    frame,
                    &self.route,
                    &self.repo_root,
                    self.repository.as_ref().map(Repository::config),
                )
            })?;

            if matches!(self.route, Route::Shutdown) {
                return Ok(());
            }

            if event::poll(Duration::from_millis(250)).context("polling for input")? {
                if let Event::Key(key) = event::read().context("reading terminal input")? {
                    self.handle_key(key)?;
                }
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }
        if matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.route = Route::Shutdown;
            return Ok(());
        }

        match self.route.clone() {
            Route::Navigation(state) => self.handle_navigation_key(state, key),
            Route::GuidedCreation(state) => self.handle_guided_key(state, key),
            Route::MonthEditing(state) => self.handle_editor_key(state, key),
            Route::BlockingFailure(state) => self.handle_failure_key(state, key),
            Route::Shutdown => Ok(()),
        }
    }

    fn handle_navigation_key(&mut self, mut state: NavigationState, key: KeyEvent) -> Result<()> {
        if let Some(dialog) = state.dialog.clone() {
            return self.handle_navigation_dialog(state, dialog, key);
        }

        match key.code {
            KeyCode::Char('q') => self.route = Route::Shutdown,
            KeyCode::Char('n') => {
                state.dialog = Some(NavigationDialog::Create(CreateDialog {
                    input: String::new(),
                    error: None,
                }));
                self.route = Route::Navigation(state);
            }
            KeyCode::Char('m') => {
                if let Some(entry) = state.selected_month() {
                    state.dialog = Some(NavigationDialog::Rename(RenameDialog {
                        source: entry.document.month,
                        input: entry.document.month.key(),
                        error: None,
                    }));
                }
                self.route = Route::Navigation(state);
            }
            KeyCode::Char('d') => {
                if let Some(entry) = state.selected_month() {
                    state.dialog = Some(NavigationDialog::Delete(DeleteDialog {
                        month: entry.document.month,
                        confirmation: String::new(),
                        error: None,
                    }));
                }
                self.route = Route::Navigation(state);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !state.months.is_empty() {
                    state.selected = (state.selected + 1).min(state.months.len().saturating_sub(1));
                }
                self.route = Route::Navigation(state);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !state.months.is_empty() {
                    state.selected = state.selected.saturating_sub(1);
                }
                self.route = Route::Navigation(state);
            }
            KeyCode::Enter => {
                if let Some(entry) = state.selected_month() {
                    self.open_month(entry.document.month)?;
                } else {
                    self.route = Route::Navigation(state);
                }
            }
            KeyCode::Char('r') => self.enter_repository_gate(),
            _ => self.route = Route::Navigation(state),
        }
        Ok(())
    }

    fn handle_navigation_dialog(
        &mut self,
        mut state: NavigationState,
        dialog: NavigationDialog,
        key: KeyEvent,
    ) -> Result<()> {
        match dialog {
            NavigationDialog::Create(mut dialog) => match key.code {
                KeyCode::Esc => state.dialog = None,
                KeyCode::Enter => match MonthId::parse(dialog.input.trim()) {
                    Ok(month) => return self.start_guided_creation(month),
                    Err(error) => {
                        dialog.error = Some(error.to_string());
                        state.dialog = Some(NavigationDialog::Create(dialog));
                    }
                },
                KeyCode::Backspace => {
                    dialog.input.pop();
                    dialog.error = None;
                    state.dialog = Some(NavigationDialog::Create(dialog));
                }
                KeyCode::Char(character) if is_month_id_character(character) => {
                    dialog.input.push(character);
                    dialog.error = None;
                    state.dialog = Some(NavigationDialog::Create(dialog));
                }
                _ => state.dialog = Some(NavigationDialog::Create(dialog)),
            },
            NavigationDialog::Rename(mut dialog) => match key.code {
                KeyCode::Esc => state.dialog = None,
                KeyCode::Enter => match validate_rename_target(dialog.source, &dialog.input) {
                    Ok(target) => return self.rename_month(dialog.source, target),
                    Err(error) => {
                        dialog.error = Some(error);
                        state.dialog = Some(NavigationDialog::Rename(dialog));
                    }
                },
                KeyCode::Backspace => {
                    dialog.input.pop();
                    dialog.error = None;
                    state.dialog = Some(NavigationDialog::Rename(dialog));
                }
                KeyCode::Char(character) if is_month_id_character(character) => {
                    dialog.input.push(character);
                    dialog.error = None;
                    state.dialog = Some(NavigationDialog::Rename(dialog));
                }
                _ => state.dialog = Some(NavigationDialog::Rename(dialog)),
            },
            NavigationDialog::Delete(mut dialog) => match key.code {
                KeyCode::Esc => state.dialog = None,
                KeyCode::Enter => {
                    if dialog.confirmation.trim() == dialog.month.key() {
                        return self.delete_month(dialog.month);
                    }
                    dialog.error = Some(format!("Type {} to confirm deletion", dialog.month));
                    state.dialog = Some(NavigationDialog::Delete(dialog));
                }
                KeyCode::Backspace => {
                    dialog.confirmation.pop();
                    dialog.error = None;
                    state.dialog = Some(NavigationDialog::Delete(dialog));
                }
                KeyCode::Char(character) if is_month_id_character(character) => {
                    dialog.confirmation.push(character);
                    dialog.error = None;
                    state.dialog = Some(NavigationDialog::Delete(dialog));
                }
                _ => state.dialog = Some(NavigationDialog::Delete(dialog)),
            },
        }

        self.route = Route::Navigation(state);
        Ok(())
    }

    fn handle_guided_key(&mut self, mut state: GuidedCreationState, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => {
                self.route = Route::Shutdown;
                Ok(())
            }
            KeyCode::Esc => {
                self.reload_navigation(Some(state.document.month));
                Ok(())
            }
            KeyCode::Backspace => {
                state.input.backspace();
                self.route = Route::GuidedCreation(state);
                Ok(())
            }
            KeyCode::Enter => self.commit_guided_step(state),
            KeyCode::Char(character) if is_money_input_character(character) => {
                state.input.push(character);
                self.route = Route::GuidedCreation(state);
                Ok(())
            }
            _ => {
                self.route = Route::GuidedCreation(state);
                Ok(())
            }
        }
    }

    fn handle_editor_key(&mut self, mut state: EditorState, key: KeyEvent) -> Result<()> {
        match state.interaction {
            InteractionState::SheetIdle => {
                match key.code {
                    KeyCode::Char('q') => self.route = Route::Shutdown,
                    KeyCode::Esc => self.reload_navigation(Some(state.document.month)),
                    KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                        state.focus_index =
                            (state.focus_index + 1).min(state.fields.len().saturating_sub(1));
                        self.route = Route::MonthEditing(state);
                    }
                    KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                        state.focus_index = state.focus_index.saturating_sub(1);
                        self.route = Route::MonthEditing(state);
                    }
                    KeyCode::Enter | KeyCode::Char('e') => {
                        let field = state.fields[state.focus_index].clone();
                        state.edit_buffer = Some(MoneyInput::from_field(&field, &state.document));
                        state.interaction = InteractionState::FieldEditing;
                        self.route = Route::MonthEditing(state);
                    }
                    _ => self.route = Route::MonthEditing(state),
                }
                Ok(())
            }
            InteractionState::FieldEditing => match key.code {
                KeyCode::Esc => {
                    state.edit_buffer = None;
                    state.interaction = InteractionState::SheetIdle;
                    self.route = Route::MonthEditing(state);
                    Ok(())
                }
                KeyCode::Backspace => {
                    if let Some(buffer) = &mut state.edit_buffer {
                        buffer.backspace();
                    }
                    self.route = Route::MonthEditing(state);
                    Ok(())
                }
                KeyCode::Enter => self.commit_editor_field(state),
                KeyCode::Char(character) if is_money_input_character(character) => {
                    if let Some(buffer) = &mut state.edit_buffer {
                        buffer.push(character);
                    }
                    self.route = Route::MonthEditing(state);
                    Ok(())
                }
                _ => {
                    self.route = Route::MonthEditing(state);
                    Ok(())
                }
            },
        }
    }

    fn handle_failure_key(&mut self, state: FailureState, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.route = Route::Shutdown;
                Ok(())
            }
            KeyCode::Char('r') => self.retry_failure(state.retry),
            _ => {
                self.route = Route::BlockingFailure(state);
                Ok(())
            }
        }
    }

    fn retry_failure(&mut self, retry: RetryTarget) -> Result<()> {
        match retry {
            RetryTarget::RepositoryGate => {
                self.enter_repository_gate();
                Ok(())
            }
            RetryTarget::CreateDraft(state) => self.save_initial_guided_state(state),
            RetryTarget::OpenMonth(month) => self.open_month(month),
            RetryTarget::GuidedSave(state) => self.persist_guided_state(state),
            RetryTarget::EditorSave(state) => self.persist_editor_state(state),
            RetryTarget::RenameMonth { source, target } => self.rename_month(source, target),
            RetryTarget::DeleteMonth(month) => self.delete_month(month),
        }
    }

    fn enter_repository_gate(&mut self) {
        match Repository::open(&self.repo_root)
            .and_then(|repo| self.navigation_state_from_repository(repo, None))
        {
            Ok((repo, navigation)) => {
                info!("repository ready");
                self.repository = Some(repo);
                self.route = Route::Navigation(navigation);
            }
            Err(error) => {
                self.route = Route::BlockingFailure(FailureState {
                    title: "Repository blocked".to_owned(),
                    message: error.to_string(),
                    retry: RetryTarget::RepositoryGate,
                });
            }
        }
    }

    fn navigation_state_from_repository(
        &self,
        repo: Repository,
        selected: Option<MonthId>,
    ) -> Result<(Repository, NavigationState)> {
        let months = repo
            .list_months()?
            .into_iter()
            .map(|loaded| MonthEntry {
                document: loaded.document,
                calculated: loaded.calculated,
            })
            .collect::<Vec<_>>();
        let mut navigation = NavigationState::new(months);
        if let Some(selected_month) = selected {
            if let Some(index) = navigation
                .months
                .iter()
                .position(|entry| entry.document.month == selected_month)
            {
                navigation.selected = index;
            }
        }
        Ok((repo, navigation))
    }

    fn reload_navigation(&mut self, selected: Option<MonthId>) {
        let Some(repo) = self.repository.take() else {
            self.enter_repository_gate();
            return;
        };

        match self.navigation_state_from_repository(repo, selected) {
            Ok((repo, navigation)) => {
                self.repository = Some(repo);
                self.route = Route::Navigation(navigation);
            }
            Err(error) => {
                self.route = Route::BlockingFailure(FailureState {
                    title: "Repository blocked".to_owned(),
                    message: error.to_string(),
                    retry: RetryTarget::RepositoryGate,
                });
            }
        }
    }

    fn open_month(&mut self, month: MonthId) -> Result<()> {
        let repository = self.repository()?;
        match repository.load_month_by_id(month) {
            Ok(loaded) => {
                self.route = Route::MonthEditing(self.editor_state_from_loaded(loaded));
            }
            Err(error) => {
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not open {month}"),
                    message: error.to_string(),
                    retry: RetryTarget::OpenMonth(month),
                });
            }
        }
        Ok(())
    }

    fn rename_month(&mut self, source: MonthId, target: MonthId) -> Result<()> {
        let repository = self.repository()?;
        match repository.rename_month(source, target) {
            Ok(()) => {
                self.reload_navigation(Some(target));
                Ok(())
            }
            Err(error) => {
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not rename {source}"),
                    message: error.to_string(),
                    retry: RetryTarget::RenameMonth { source, target },
                });
                Ok(())
            }
        }
    }

    fn delete_month(&mut self, month: MonthId) -> Result<()> {
        let repository = self.repository()?;
        match repository.delete_month(month) {
            Ok(()) => {
                self.reload_navigation(None);
                Ok(())
            }
            Err(error) => {
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not delete {month}"),
                    message: error.to_string(),
                    retry: RetryTarget::DeleteMonth(month),
                });
                Ok(())
            }
        }
    }

    fn start_guided_creation(&mut self, month: MonthId) -> Result<()> {
        let repository = self.repository()?;
        let document = repository.create_month_draft(month)?;
        let guided = self.guided_state_from_document(document, Some("Draft created".to_owned()))?;
        self.save_initial_guided_state(guided)
    }

    fn commit_guided_step(&mut self, mut state: GuidedCreationState) -> Result<()> {
        let field = state.steps[state.step_index].clone();
        match update_document_field(&mut state.document, &field, &state.input) {
            Ok(()) => match calculate_month(self.config()?, &state.document) {
                Ok(calculated) => {
                    state.calculated = calculated;
                    state.message = Some("Autosaving draft".to_owned());
                    state.persistence = PersistenceState::Dirty;
                    state.sync = SyncState::SyncPending;
                    self.persist_guided_state(state)
                }
                Err(error) => {
                    state.message = Some(error.to_string());
                    self.route = Route::GuidedCreation(state);
                    Ok(())
                }
            },
            Err(error) => {
                state.message = Some(error.to_string());
                self.route = Route::GuidedCreation(state);
                Ok(())
            }
        }
    }

    fn persist_guided_state(&mut self, mut state: GuidedCreationState) -> Result<()> {
        state.persistence = PersistenceState::Autosaving;
        state.sync = SyncState::Syncing;
        let repository = self.repository()?;
        match repository.save_month(&mut state.document) {
            Ok(()) => {
                state.calculated = calculate_month(self.config()?, &state.document)?;
                state.persistence = PersistenceState::Clean;
                state.sync = SyncState::Synced;
                state.message = Some(self.autosave_message("Draft autosaved")?);
                if state.step_index + 1 >= state.steps.len() {
                    let editor =
                        self.editor_state_from_document(state.document, state.message.clone())?;
                    self.route = Route::MonthEditing(editor);
                } else {
                    state.step_index += 1;
                    state.input =
                        MoneyInput::from_field(&state.steps[state.step_index], &state.document);
                    self.route = Route::GuidedCreation(state);
                }
                Ok(())
            }
            Err(error) => {
                state.persistence = PersistenceState::SaveFailed;
                state.sync = SyncState::SyncFailed;
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not save {}", state.document.month),
                    message: error.to_string(),
                    retry: RetryTarget::GuidedSave(state),
                });
                Ok(())
            }
        }
    }

    fn save_initial_guided_state(&mut self, mut state: GuidedCreationState) -> Result<()> {
        state.persistence = PersistenceState::Autosaving;
        state.sync = SyncState::Syncing;
        let repository = self.repository()?;
        match repository.save_month(&mut state.document) {
            Ok(()) => {
                state.calculated = calculate_month(self.config()?, &state.document)?;
                state.persistence = PersistenceState::Clean;
                state.sync = SyncState::Synced;
                state.message = Some(self.autosave_message("Draft created")?);
                self.route = Route::GuidedCreation(state);
                Ok(())
            }
            Err(error) => {
                state.persistence = PersistenceState::SaveFailed;
                state.sync = SyncState::SyncFailed;
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not create {}", state.document.month),
                    message: error.to_string(),
                    retry: RetryTarget::CreateDraft(state),
                });
                Ok(())
            }
        }
    }

    fn commit_editor_field(&mut self, mut state: EditorState) -> Result<()> {
        let field = state.fields[state.focus_index].clone();
        let Some(buffer) = state.edit_buffer.clone() else {
            self.route = Route::MonthEditing(state);
            return Ok(());
        };

        match update_document_field(&mut state.document, &field, &buffer) {
            Ok(()) => match calculate_month(self.config()?, &state.document) {
                Ok(calculated) => {
                    state.calculated = calculated;
                    state.message = Some("Autosaving changes".to_owned());
                    state.persistence = PersistenceState::Dirty;
                    state.sync = SyncState::SyncPending;
                    state.edit_buffer = None;
                    state.interaction = InteractionState::SheetIdle;
                    self.persist_editor_state(state)
                }
                Err(error) => {
                    state.message = Some(error.to_string());
                    self.route = Route::MonthEditing(state);
                    Ok(())
                }
            },
            Err(error) => {
                state.message = Some(error.to_string());
                self.route = Route::MonthEditing(state);
                Ok(())
            }
        }
    }

    fn persist_editor_state(&mut self, mut state: EditorState) -> Result<()> {
        state.persistence = PersistenceState::Autosaving;
        state.sync = SyncState::Syncing;
        let repository = self.repository()?;
        match repository.save_month(&mut state.document) {
            Ok(()) => {
                state.calculated = calculate_month(self.config()?, &state.document)?;
                state.persistence = PersistenceState::Clean;
                state.sync = SyncState::Synced;
                state.message = Some(self.autosave_message("Month autosaved")?);
                self.route = Route::MonthEditing(state);
                Ok(())
            }
            Err(error) => {
                state.persistence = PersistenceState::SaveFailed;
                state.sync = SyncState::SyncFailed;
                self.route = Route::BlockingFailure(FailureState {
                    title: format!("Could not save {}", state.document.month),
                    message: error.to_string(),
                    retry: RetryTarget::EditorSave(state),
                });
                Ok(())
            }
        }
    }

    fn guided_state_from_document(
        &self,
        document: MonthDocument,
        message: Option<String>,
    ) -> Result<GuidedCreationState> {
        let calculated = calculate_month(self.config()?, &document)?;
        let steps = FieldId::guided_steps(self.config()?);
        let input = steps
            .first()
            .ok_or_else(|| anyhow!("guided step list is empty"))?
            .clone();
        let initial_input = MoneyInput::from_field(&input, &document);
        Ok(GuidedCreationState {
            document,
            calculated,
            steps,
            step_index: 0,
            input: initial_input,
            message,
            persistence: PersistenceState::Clean,
            sync: SyncState::Synced,
        })
    }

    fn editor_state_from_loaded(&self, loaded: LoadedMonth) -> EditorState {
        self.editor_state_from_calculated(loaded.document, loaded.calculated, None)
    }

    fn editor_state_from_document(
        &self,
        document: MonthDocument,
        message: Option<String>,
    ) -> Result<EditorState> {
        let calculated = calculate_month(self.config()?, &document)?;
        Ok(self.editor_state_from_calculated(document, calculated, message))
    }

    fn editor_state_from_calculated(
        &self,
        document: MonthDocument,
        calculated: CalculatedMonth,
        message: Option<String>,
    ) -> EditorState {
        EditorState {
            document,
            calculated,
            fields: FieldId::editor_fields(self.config().expect("config available")),
            focus_index: 0,
            edit_buffer: None,
            message,
            interaction: InteractionState::SheetIdle,
            persistence: PersistenceState::Clean,
            sync: SyncState::Synced,
        }
    }

    fn repository(&self) -> Result<&Repository> {
        self.repository
            .as_ref()
            .ok_or_else(|| anyhow!("repository not available"))
    }

    fn config(&self) -> Result<&budget_core::AppConfig> {
        Ok(self.repository()?.config())
    }

    fn autosave_message(&self, local_message: &str) -> Result<String> {
        Ok(if self.repository()?.sync_enabled() {
            format!("{local_message} and synced")
        } else {
            local_message.to_owned()
        })
    }
}

fn setup_terminal() -> Result<AppTerminal> {
    enable_raw_mode().context("enabling raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("entering alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend).context("creating terminal backend")?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut AppTerminal) -> Result<()> {
    disable_raw_mode().context("disabling raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).context("leaving alternate screen")?;
    terminal.show_cursor().context("restoring cursor")?;
    Ok(())
}

fn update_document_field(
    document: &mut MonthDocument,
    field: &FieldId,
    input: &MoneyInput,
) -> Result<()> {
    let value = input.commit_value()?;
    if !field.allows_negative() && value.minor() < 0 {
        anyhow::bail!("{} cannot be negative", field.labelless_name());
    }

    match field {
        FieldId::Account(id) => {
            document.accounts.insert(id.clone(), value.minor());
        }
        FieldId::PreviousMonthSpendingCorrection => {
            document
                .timing_adjustments
                .previous_month_spending_correction_raw = value.minor();
        }
        FieldId::InvestmentNotYetSent => {
            document.timing_adjustments.investment_not_yet_sent_raw = value.minor();
        }
        FieldId::Earmark(id) => {
            document
                .next_month_earmarks
                .insert(id.clone(), value.minor());
        }
        FieldId::PotCarried(id) => {
            let entry = document.savings_pots.entry(id.clone()).or_default();
            entry.carried_over = value.minor();
        }
        FieldId::PotChange(id) => {
            let entry = document.savings_pots.entry(id.clone()).or_default();
            entry.monthly_change = value.minor();
        }
    }

    Ok(())
}

trait FieldNameExt {
    fn labelless_name(&self) -> &'static str;
}

impl FieldNameExt for FieldId {
    fn labelless_name(&self) -> &'static str {
        match self {
            FieldId::Account(_) => "account balance",
            FieldId::PreviousMonthSpendingCorrection => "general spending over/under",
            FieldId::InvestmentNotYetSent => "investment not yet sent",
            FieldId::Earmark(_) => "next-month earmark",
            FieldId::PotCarried(_) => "pot carried-over balance",
            FieldId::PotChange(_) => "pot monthly change",
        }
    }
}

fn is_month_id_character(character: char) -> bool {
    character.is_ascii_digit() || character == '-'
}

fn validate_rename_target(source: MonthId, input: &str) -> std::result::Result<MonthId, String> {
    let target = MonthId::parse(input.trim()).map_err(|error| error.to_string())?;
    if target == source {
        return Err(format!("Month is already named {source}"));
    }
    Ok(target)
}

fn is_money_input_character(character: char) -> bool {
    character.is_ascii_digit() || matches!(character, '.' | '-')
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use budget_core::{AppConfig, MonthDocument, MonthId, calculate_month};
    use crossterm::event::{KeyCode, KeyEvent};
    use tempfile::TempDir;

    use super::App;
    use crate::state::{
        EditorState, FieldId, InteractionState, MonthEntry, NavigationDialog, NavigationState,
        PersistenceState, RenameDialog, Route, SyncState,
    };
    use crate::{repository::Repository, state::FailureState};

    #[test]
    fn editor_navigation_visits_each_field_once_in_visible_order() {
        let config = AppConfig::default_mvp();
        let document = MonthDocument::new_draft(MonthId::parse("2026-03").unwrap(), &config, None);
        let calculated = calculate_month(&config, &document).unwrap();
        let fields = FieldId::editor_fields(&config);
        let expected = fields.clone();

        let mut app = App {
            repo_root: PathBuf::from("/tmp/budget"),
            repository: None,
            route: Route::MonthEditing(EditorState {
                document,
                calculated,
                fields,
                focus_index: 0,
                edit_buffer: None,
                message: None,
                interaction: InteractionState::SheetIdle,
                persistence: PersistenceState::Clean,
                sync: SyncState::Synced,
            }),
        };

        let mut visited = Vec::new();
        loop {
            match &app.route {
                Route::MonthEditing(state) => {
                    visited.push(state.fields[state.focus_index].clone());
                    if state.focus_index + 1 == state.fields.len() {
                        break;
                    }
                }
                _ => panic!("editor route unexpectedly changed"),
            }
            app.handle_key(KeyEvent::from(KeyCode::Tab)).unwrap();
        }

        assert_eq!(visited, expected);
        let unique = visited.iter().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), visited.len());
    }

    #[test]
    fn rename_dialog_keeps_unchanged_month_error_inline() {
        let (_temp, repo_root, repository, navigation, source) = seeded_navigation_app("2026-03");
        let mut app = App {
            repo_root,
            repository: Some(repository),
            route: Route::Navigation(NavigationState {
                dialog: Some(NavigationDialog::Rename(RenameDialog {
                    source,
                    input: source.key(),
                    error: None,
                })),
                ..navigation
            }),
        };

        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

        match app.route {
            Route::Navigation(state) => {
                assert_eq!(state.selected, 0);
                match state.dialog {
                    Some(NavigationDialog::Rename(dialog)) => {
                        assert_eq!(dialog.source, source);
                        assert_eq!(dialog.input, "2026-03");
                        assert_eq!(
                            dialog.error.as_deref(),
                            Some("Month is already named 2026-03")
                        );
                    }
                    other => panic!("expected rename dialog, got {other:?}"),
                }
            }
            other => panic!("expected navigation route, got {other:?}"),
        }
    }

    #[test]
    fn rename_dialog_preserves_input_after_validation_error_and_allows_retry() {
        let (_temp, repo_root, repository, navigation, source) = seeded_navigation_app("2026-03");
        let mut app = App {
            repo_root,
            repository: Some(repository),
            route: Route::Navigation(NavigationState {
                dialog: Some(NavigationDialog::Rename(RenameDialog {
                    source,
                    input: "2026-13".to_owned(),
                    error: None,
                })),
                ..navigation
            }),
        };

        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();
        match &app.route {
            Route::Navigation(state) => match &state.dialog {
                Some(NavigationDialog::Rename(dialog)) => {
                    assert_eq!(dialog.input, "2026-13");
                    assert_eq!(
                        dialog.error.as_deref(),
                        Some("invalid month id `2026-13`, expected YYYY-MM")
                    );
                }
                other => panic!("expected rename dialog, got {other:?}"),
            },
            other => panic!("expected navigation route, got {other:?}"),
        }

        app.handle_key(KeyEvent::from(KeyCode::Backspace)).unwrap();
        app.handle_key(KeyEvent::from(KeyCode::Char('2'))).unwrap();

        match &app.route {
            Route::Navigation(state) => match &state.dialog {
                Some(NavigationDialog::Rename(dialog)) => {
                    assert_eq!(dialog.input, "2026-12");
                    assert_eq!(dialog.error, None);
                }
                other => panic!("expected rename dialog, got {other:?}"),
            },
            other => panic!("expected navigation route, got {other:?}"),
        }

        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

        match app.route {
            Route::Navigation(state) => {
                assert!(state.dialog.is_none());
                assert_eq!(state.selected, 0);
                assert_eq!(
                    state.months[0].document.month,
                    MonthId::parse("2026-12").unwrap()
                );
            }
            other => panic!("expected navigation route, got {other:?}"),
        }
    }

    #[test]
    fn rename_dialog_uses_blocking_failure_for_repository_faults() {
        let (_temp, repo_root, repository, navigation, source) = seeded_navigation_app("2026-03");
        fs::remove_file(repo_root.join("months/2026-03.toml")).unwrap();

        let mut app = App {
            repo_root,
            repository: Some(repository),
            route: Route::Navigation(NavigationState {
                dialog: Some(NavigationDialog::Rename(RenameDialog {
                    source,
                    input: "2026-04".to_owned(),
                    error: None,
                })),
                ..navigation
            }),
        };

        app.handle_key(KeyEvent::from(KeyCode::Enter)).unwrap();

        match app.route {
            Route::BlockingFailure(FailureState { title, message, .. }) => {
                assert_eq!(title, "Could not rename 2026-03");
                assert!(message.contains("month `2026-03` does not exist"));
            }
            other => panic!("expected blocking failure, got {other:?}"),
        }
    }

    fn seeded_navigation_app(
        month_key: &str,
    ) -> (TempDir, PathBuf, Repository, NavigationState, MonthId) {
        let temp = tempfile::tempdir().unwrap();
        let repo_root = temp.path().join("budget");
        Repository::init(&repo_root, None).unwrap();

        let repository = Repository::open(&repo_root).unwrap();
        let month = MonthId::parse(month_key).unwrap();
        let mut document = repository.create_month_draft(month).unwrap();
        repository.save_month(&mut document).unwrap();
        let navigation = NavigationState::new(
            repository
                .list_months()
                .unwrap()
                .into_iter()
                .map(|loaded| MonthEntry {
                    document: loaded.document,
                    calculated: loaded.calculated,
                })
                .collect(),
        );

        (temp, repo_root, repository, navigation, month)
    }
}
