//! Runtime application loop for the TUI state machine.
//!
//! Input handling lives in the route-specific submodules, while this module
//! owns terminal setup, dispatch, and the shared repository/config handles.

mod document;
mod editor;
mod failure;
mod guided;
mod navigation;
mod persistence;

#[cfg(test)]
mod tests;

use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::repository::Repository;
use crate::state::{RepositoryGateState, Route};
use crate::ui;

type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

/// Runs the interactive TUI against an initialised budget repository.
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
        Self {
            repo_root,
            repository: None,
            route: Route::RepositoryGate(RepositoryGateState {
                message: "Checking repository and syncing latest changes".to_owned(),
            }),
        }
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

            if self.process_repository_gate() {
                continue;
            }

            if self.process_pending_sync()? {
                continue;
            }

            // Poll with a short timeout so the UI stays responsive without
            // burning CPU between keypresses.
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

        // Route states can hold full documents and rendered calculation
        // context, so move the active route into the specific handler instead
        // of cloning it for every keypress.
        match std::mem::replace(&mut self.route, Route::Shutdown) {
            Route::RepositoryGate(state) => {
                self.route = Route::RepositoryGate(state);
                Ok(())
            }
            Route::Navigation(state) => self.handle_navigation_key(state, key),
            Route::GuidedCreation(state) => self.handle_guided_key(state, key),
            Route::MonthEditing(state) => self.handle_editor_key(state, key),
            Route::BlockingFailure(state) => self.handle_failure_key(state, key),
            Route::Shutdown => Ok(()),
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

    fn process_repository_gate(&mut self) -> bool {
        if !matches!(self.route, Route::RepositoryGate(_)) {
            return false;
        }
        self.enter_repository_gate();
        true
    }

    fn show_repository_gate(&mut self) {
        self.repository = None;
        self.route = Route::RepositoryGate(RepositoryGateState {
            message: "Checking repository and syncing latest changes".to_owned(),
        });
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
    // Always leave the terminal in a normal shell-friendly state before
    // returning any runtime error to the caller.
    disable_raw_mode().context("disabling raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).context("leaving alternate screen")?;
    terminal.show_cursor().context("restoring cursor")?;
    Ok(())
}
