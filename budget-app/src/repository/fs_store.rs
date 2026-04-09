use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail, ensure};
use tempfile::NamedTempFile;

pub(super) fn ensure_directory_missing_or_empty(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    ensure!(
        path.is_dir(),
        "repository path `{}` must be a directory",
        path.display()
    );

    if std::fs::read_dir(path)
        .with_context(|| format!("reading `{}`", path.display()))?
        .next()
        .transpose()?
        .is_none()
    {
        return Ok(());
    }

    bail!(
        "repository path `{}` must be empty or not exist",
        path.display()
    );
}

pub(super) fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("path `{}` has no parent", path.display()))?;
    let mut temp = NamedTempFile::new_in(parent)
        .with_context(|| format!("creating temp file for `{}`", path.display()))?;
    temp.write_all(contents.as_bytes())
        .with_context(|| format!("writing temp file for `{}`", path.display()))?;
    temp.flush()
        .with_context(|| format!("flushing temp file for `{}`", path.display()))?;
    // Sync both the file and its directory so the rename is durable before we
    // tell higher layers that autosave succeeded.
    temp.as_file()
        .sync_all()
        .with_context(|| format!("syncing temp file for `{}`", path.display()))?;
    temp.persist(path)
        .map_err(|error| anyhow!(error.error))
        .with_context(|| format!("persisting `{}`", path.display()))?;
    File::open(parent)
        .with_context(|| format!("opening directory `{}` for sync", parent.display()))?
        .sync_all()
        .with_context(|| format!("syncing directory `{}`", parent.display()))?;
    Ok(())
}
