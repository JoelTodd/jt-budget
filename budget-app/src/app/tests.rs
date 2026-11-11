use std::fs;
use std::hint::black_box;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use budget_core::{AppConfig, MonthDocument, MonthId, calculate_month};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tempfile::TempDir;

use super::App;
use crate::repository::Repository;
use crate::state::{
    CreateDialogue, DeleteDialogue, EditorState, FailureState, FieldId, InteractionState,
    MonthEntry, NavigationDialogue, NavigationState, PersistenceState, RenameDialogue, RetryTarget,
    Route, SyncState,
};

mod defaults;
mod editor_return_cache;
mod fixtures;
mod git_support;
mod guided_pending;
mod guided_perf;
mod large_history_first_draw;
mod large_history_mutations;
mod navigation_dialogues;
mod navigation_freshness;
mod navigation_pending;
mod perf_support;
mod redraw;
mod remote_delete_failures;
mod remote_large_history;
mod save_paths;

use fixtures::*;
use git_support::*;
use perf_support::*;
