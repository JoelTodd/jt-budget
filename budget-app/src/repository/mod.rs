//! File-backed repository access with explicit git-based synchronisation.
//!
//! The runtime depends on this layer to enforce strict sync semantics: normal
//! operations may save locally first, but failures to pull or push are surfaced
//! as blocking states rather than silently ignored.

mod fs_store;
mod git_repo;
mod month_store;

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use budget_core::{AppConfig, CalculatedMonth, MonthDocument, MonthId};
use fs4::fs_std::FileExt;
use tracing::info;

use self::fs_store::ensure_directory_missing_or_empty;
use self::git_repo::{GitRepo, TrackedCommitOutcome, clone_from_remote};
use self::month_store::MonthStore;

#[derive(Debug)]
pub struct Repository {
    root: PathBuf,
    config: AppConfig,
    sync_enabled: bool,
    git: GitRepo,
    months: MonthStore,
    _meta_dir: PathBuf,
    #[allow(dead_code)]
    lock_handle: File,
}

#[derive(Clone, Debug)]
pub struct LoadedMonth {
    pub path: PathBuf,
    pub document: MonthDocument,
    pub calculated: CalculatedMonth,
}

pub(crate) type MonthFileFingerprint = Vec<(MonthId, u64)>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncOutcome {
    Synced,
    PushFailed(String),
}

impl Repository {
    /// Creates a new on-disk budget repository and bootstraps its git history.
    ///
    /// # Errors
    ///
    /// Returns an error if the target directory is not empty, required files
    /// cannot be written, or git initialisation fails.
    pub fn init(root: &Path, remote: Option<&str>) -> Result<()> {
        Self::init_with_config(root, remote, &AppConfig::default_mvp())
    }

    /// Creates a new budget repository with an explicit initial config.
    ///
    /// # Errors
    ///
    /// Returns an error if the config is invalid, the target directory is not
    /// empty, required files cannot be written, or git initialisation fails.
    pub(crate) fn init_with_config(
        root: &Path,
        remote: Option<&str>,
        config: &AppConfig,
    ) -> Result<()> {
        config.validate().context("validating initial config")?;
        ensure_directory_missing_or_empty(root)?;

        fs::create_dir_all(root.join("months"))
            .with_context(|| format!("creating months directory in `{}`", root.display()))?;
        fs::create_dir_all(root.join("meta"))
            .with_context(|| format!("creating meta directory in `{}`", root.display()))?;

        fs::write(root.join("config.toml"), toml::to_string_pretty(&config)?)
            .with_context(|| format!("writing config.toml in `{}`", root.display()))?;
        fs::write(root.join("months/.gitkeep"), "")
            .with_context(|| format!("writing months/.gitkeep in `{}`", root.display()))?;
        fs::write(root.join(".gitignore"), "meta/app.lock\nmeta/app.log\n")
            .with_context(|| format!("writing .gitignore in `{}`", root.display()))?;

        let git = GitRepo::new(root.to_path_buf());
        git.initialise_budget_repo()?;
        if let Some(remote) = remote {
            Self::connect_remote(root, remote).context("publishing initial repository state")?;
        }

        Ok(())
    }

    /// Clones an existing remote repository into a missing or empty directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the target directory is unsuitable or the clone
    /// operation fails.
    pub fn clone_from_remote(remote: &str, target: &Path) -> Result<()> {
        ensure!(!remote.trim().is_empty(), "remote cannot be empty");
        ensure_directory_missing_or_empty(target)?;
        clone_from_remote(remote, target)
    }

    /// Reports whether a path already looks like a jt-budget repository root.
    ///
    /// # Errors
    ///
    /// Returns an error if the path contains a malformed config or Git command
    /// execution fails while probing the repository layout.
    pub fn looks_like_budget_repo(root: &Path) -> Result<bool> {
        if !root.is_dir() {
            return Ok(false);
        }

        let config_path = root.join("config.toml");
        let months_dir = root.join("months");
        if !config_path.is_file() || !months_dir.is_dir() || !GitRepo::is_work_tree(root)? {
            return Ok(false);
        }

        let config_text = fs::read_to_string(&config_path)
            .with_context(|| format!("reading `{}`", config_path.display()))?;
        let config: AppConfig = toml::from_str(&config_text)
            .with_context(|| format!("parsing `{}`", config_path.display()))?;
        config
            .validate()
            .with_context(|| format!("validating `{}`", config_path.display()))?;
        Ok(true)
    }

    /// Reports whether the repository has an `origin` remote configured.
    ///
    /// # Errors
    ///
    /// Returns an error when Git cannot inspect the repository config.
    pub fn has_origin_remote(root: &Path) -> Result<bool> {
        GitRepo::new(root.to_path_buf()).has_remote_named("origin")
    }

    /// Returns the configured `origin` remote URL when present.
    ///
    /// # Errors
    ///
    /// Returns an error when Git cannot inspect the repository config.
    pub fn origin_remote_url(root: &Path) -> Result<Option<String>> {
        let git = GitRepo::new(root.to_path_buf());
        if git.has_remote_named("origin")? {
            return Ok(Some(git.remote_url("origin")?));
        }
        Ok(None)
    }

    /// Configures or verifies the `origin` remote and ensures `main` tracks it.
    ///
    /// # Errors
    ///
    /// Returns an error if `origin` already points elsewhere or the remote
    /// cannot be reached and published cleanly.
    pub fn connect_remote(root: &Path, remote: &str) -> Result<()> {
        ensure!(!remote.trim().is_empty(), "remote cannot be empty");
        let git = GitRepo::new(root.to_path_buf());
        git.ensure_repository()?;

        if git.has_remote_named("origin")? {
            let existing_remote = git.remote_url("origin")?;
            ensure!(
                existing_remote == remote,
                "repository already has origin set to `{existing_remote}`"
            );
        } else {
            git.add_remote("origin", remote)
                .context("adding origin remote")?;
        }

        git.push_default_branch_with_upstream("origin")
            .context("publishing repository state")?;
        git.verify_branch_upstream()
            .context("verifying branch upstream after remote setup")?;
        Ok(())
    }

    /// Opens an existing repository, acquires the single-app lock, and performs
    /// the initial sync gate.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository is malformed, another app instance is
    /// already holding the lock, or the initial pull check fails.
    pub fn open(root: &Path) -> Result<Self> {
        let root = root
            .canonicalize()
            .or_else(|_| Ok::<_, std::io::Error>(root.to_path_buf()))
            .context("resolving repository path")?;
        let months_dir = root.join("months");
        let meta_dir = root.join("meta");
        fs::create_dir_all(&meta_dir)
            .with_context(|| format!("creating `{}`", meta_dir.display()))?;

        let lock_path = meta_dir.join("app.lock");
        let lock_handle = File::options()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .with_context(|| format!("opening lock file `{}`", lock_path.display()))?;
        // The TUI is intentionally single-writer so autosave and git operations
        // never race from multiple local sessions.
        lock_handle
            .lock_exclusive()
            .with_context(|| format!("acquiring lock on `{}`", lock_path.display()))?;

        let config_text = fs::read_to_string(root.join("config.toml"))
            .context("reading repository config.toml")?;
        let config: AppConfig = toml::from_str(&config_text).context("parsing config.toml")?;
        config.validate().context("validating config.toml")?;
        ensure!(months_dir.is_dir(), "`{}` is missing", months_dir.display());
        let git = GitRepo::new(root.clone());
        let sync_enabled = git.has_remote_named_for_startup("origin")?;
        let months = MonthStore::new(months_dir, config.clone());

        let repository = Self {
            root,
            config,
            sync_enabled,
            git,
            months,
            _meta_dir: meta_dir,
            lock_handle,
        };
        repository.pull_latest()?;
        Ok(repository)
    }

    /// Returns the validated repository configuration.
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Returns the repository root on disk.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reports whether a configured `origin` enables pull/push synchronisation.
    pub fn sync_enabled(&self) -> bool {
        self.sync_enabled
    }

    /// Loads and recalculates all month documents in reverse chronological order.
    pub fn list_months(&self) -> Result<Vec<LoadedMonth>> {
        self.months.list_months()
    }

    pub(crate) fn list_months_with_fingerprint(
        &self,
    ) -> Result<(Vec<LoadedMonth>, MonthFileFingerprint)> {
        self.months.list_months_with_fingerprint()
    }

    pub(crate) fn month_file_fingerprint(&self) -> Result<MonthFileFingerprint> {
        self.months.month_file_fingerprint()
    }

    pub(crate) fn month_file_fingerprint_for_month(
        &self,
        month: MonthId,
    ) -> Result<(MonthId, u64)> {
        self.months.month_file_fingerprint_for_month(month)
    }

    /// Loads a single month by its stable month identifier.
    pub fn load_month_by_id(&self, month: MonthId) -> Result<LoadedMonth> {
        self.months.load_month_by_id(month)
    }

    /// Builds a new draft month document without writing it yet.
    ///
    /// # Errors
    ///
    /// Returns an error if the target month already exists or an earlier month
    /// cannot be loaded while determining carry-forward balances.
    pub fn create_month_draft(&self, month: MonthId) -> Result<MonthDocument> {
        self.months.create_month_draft(month)
    }

    pub(crate) fn create_month_draft_with_cached_previous(
        &self,
        month: MonthId,
        cached_previous: Option<(MonthId, &CalculatedMonth, u64)>,
    ) -> Result<MonthDocument> {
        self.months
            .create_month_draft_with_cached_previous(month, cached_previous)
    }

    /// Persists a month, commits the change, and pushes it when sync is enabled.
    pub fn save_month(&self, document: &mut MonthDocument) -> Result<()> {
        self.save_month_local_commit(document)?;
        self.push_committed_changes()?;
        Ok(())
    }

    /// Persists a month and commits it locally without pushing.
    pub(crate) fn save_month_local_commit(&self, document: &mut MonthDocument) -> Result<()> {
        let existed_before_write = self.months.write_month(document)?;
        let relative_path = self.relative_month_path(document.month);
        let message = format!("Update budget month {}", document.month);
        if existed_before_write {
            match self
                .git
                .commit_tracked_path(&relative_path, &message)
                .context("committing tracked month file")?
            {
                TrackedCommitOutcome::Committed => {
                    return Ok(());
                }
                TrackedCommitOutcome::PathNotTracked => {}
            }
        }

        self.git
            .stage_and_commit_paths(&[relative_path], &message)
            .context("staging repository changes")?;
        Ok(())
    }

    /// Persists a month file without committing it.
    pub(crate) fn save_month_file_only(&self, document: &mut MonthDocument) -> Result<()> {
        self.months.write_month(document)?;
        Ok(())
    }

    /// Commits a previously persisted month file and pushes when sync is enabled.
    pub(crate) fn commit_month_file(&self, month: MonthId, message: &str) -> Result<()> {
        self.finalise_local_mutation(&[self.relative_month_path(month)], message)?;
        Ok(())
    }

    /// Renames a month file, commits the local rename, and then attempts to push.
    pub fn rename_month(
        &self,
        source: MonthId,
        target: MonthId,
    ) -> Result<(SyncOutcome, LoadedMonth)> {
        let loaded = self.rename_month_local_commit(source, target)?;
        let outcome = self.push_after_local_commit()?;
        Ok((outcome, loaded))
    }

    /// Renames a month file and commits the local rename without pushing.
    pub(crate) fn rename_month_local_commit(
        &self,
        source: MonthId,
        target: MonthId,
    ) -> Result<LoadedMonth> {
        let loaded = self.months.rename_month(source, target)?;
        self.git
            .stage_and_commit_paths(
                &[
                    self.relative_month_path(source),
                    self.relative_month_path(target),
                ],
                &format!("Rename budget month {source} to {target}"),
            )
            .context("staging repository changes")?;
        Ok(loaded)
    }

    /// Deletes a month file, commits the local deletion, and then attempts to push.
    pub fn delete_month(&self, month: MonthId) -> Result<SyncOutcome> {
        self.delete_month_local_commit(month)?;
        self.push_after_local_commit()
    }

    /// Deletes a month file and commits the local deletion without pushing.
    pub(crate) fn delete_month_local_commit(&self, month: MonthId) -> Result<()> {
        self.months.delete_month(month)?;
        self.git
            .stage_and_commit_paths(
                &[self.relative_month_path(month)],
                &format!("Delete budget month {month}"),
            )
            .context("staging repository changes")?;
        Ok(())
    }

    /// Retries a push for changes that were already committed locally.
    pub fn retry_pending_push(&self) -> Result<()> {
        self.push_committed_changes()
    }

    /// Performs the startup repository gate.
    pub fn pull_latest(&self) -> Result<()> {
        if !self.sync_enabled {
            info!(
                "no origin remote configured for {}; skipping repository sync gate",
                self.root.display()
            );
            return Ok(());
        }
        info!("running repository gate for {}", self.root.display());
        self.git
            .pull_ff_only()
            .context("pulling latest budget data")?;
        Ok(())
    }

    fn relative_month_path(&self, month: MonthId) -> PathBuf {
        PathBuf::from("months").join(month.file_name())
    }

    fn finalise_local_mutation(
        &self,
        relative_paths: &[PathBuf],
        message: &str,
    ) -> Result<SyncOutcome> {
        self.git
            .stage_and_commit_paths(relative_paths, message)
            .context("staging repository changes")?;
        self.push_committed_changes()?;
        Ok(SyncOutcome::Synced)
    }

    fn push_after_local_commit(&self) -> Result<SyncOutcome> {
        match self.push_committed_changes() {
            Ok(()) => Ok(SyncOutcome::Synced),
            Err(error) => Ok(SyncOutcome::PushFailed(error.to_string())),
        }
    }

    fn push_committed_changes(&self) -> Result<()> {
        if self.sync_enabled {
            self.git.push().context("pushing repository changes")?;
        } else {
            info!("no origin remote configured; leaving repository changes committed locally");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
