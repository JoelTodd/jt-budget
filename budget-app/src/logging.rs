use std::fs::{self, File, OpenOptions};
use std::path::Path;
use std::sync::OnceLock;

use tracing_subscriber::fmt::writer::BoxMakeWriter;

static LOG_FILE: OnceLock<File> = OnceLock::new();

pub fn init_logging(log_path: &Path) {
    if LOG_FILE.get().is_some() {
        return;
    }

    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let file = match OpenOptions::new().create(true).append(true).open(log_path) {
        Ok(file) => file,
        Err(_) => return,
    };

    let writer = match file.try_clone() {
        Ok(cloned) => cloned,
        Err(_) => return,
    };

    let _ = LOG_FILE.set(file);
    let _ = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(BoxMakeWriter::new(writer))
        .with_target(false)
        .compact()
        .try_init();
}
