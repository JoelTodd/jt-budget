//! File-backed repository access with explicit git-based synchronisation.
//!
//! The runtime depends on this layer to enforce strict sync semantics: normal
//! operations may save locally first, but failures to pull or push are surfaced
//! as blocking states rather than silently ignored.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail, ensure};
use budget_core::{AppConfig, CalculatedMonth, MonthDocument, MonthId, calculate_month};
use fs4::fs_std::FileExt;
use tempfile::NamedTempFile;
use tracing::{info, warn};

const DEFAULT_BRANCH: &str = "main";

#[derive(Debug)]
pub struct Repository {
    root: PathBuf,
    months_dir: PathBuf,
    _meta_dir: PathBuf,
    config: AppConfig,
    sync_enabled: bool,
    #[allow(dead_code)]
    lock_handle: File,
}

#[derive(Clone, Debug)]
pub struct LoadedMonth {
    pub path: PathBuf,
    pub document: MonthDocument,
    pub calculated: CalculatedMonth,
}

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
        ensure_directory_missing_or_empty(root)?;

        fs::create_dir_all(root.join("months"))
            .with_context(|| format!("creating months directory in `{}`", root.display()))?;
        fs::create_dir_all(root.join("meta"))
            .with_context(|| format!("creating meta directory in `{}`", root.display()))?;

        let config = AppConfig::default_mvp();
        fs::write(root.join("config.toml"), toml::to_string_pretty(&config)?)
            .with_context(|| format!("writing config.toml in `{}`", root.display()))?;
        fs::write(root.join("months/.gitkeep"), "")
            .with_context(|| format!("writing months/.gitkeep in `{}`", root.display()))?;
        fs::write(root.join(".gitignore"), "meta/app.lock\nmeta/app.log\n")
            .with_context(|| format!("writing .gitignore in `{}`", root.display()))?;

        run_git(root, ["init", "-b", DEFAULT_BRANCH])?;
        run_git(root, ["config", "user.name", "jt-budget"])?;
        run_git(root, ["config", "user.email", "jt-budget@example.invalid"])?;
        run_git(root, ["add", "."])?;
        run_git(root, ["commit", "-m", "Initialise budget repository"])?;
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

        let output = Command::new("git")
            .arg("clone")
            .arg("--origin")
            .arg("origin")
            .arg(remote)
            .arg(target)
            .output()
            .with_context(|| {
                format!(
                    "running `git clone --origin origin {remote} {}`",
                    target.display()
                )
            })?;
        if output.status.success() {
            return Ok(());
        }

        bail!(
            "git clone failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
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
        if !config_path.is_file() || !months_dir.is_dir() || !is_git_work_tree(root)? {
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
        has_remote_named(root, "origin")
    }

    /// Returns the configured `origin` remote URL when present.
    ///
    /// # Errors
    ///
    /// Returns an error when Git cannot inspect the repository config.
    pub fn origin_remote_url(root: &Path) -> Result<Option<String>> {
        if has_remote_named(root, "origin")? {
            return Ok(Some(remote_url(root, "origin")?));
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
        run_git(root, ["rev-parse", "--is-inside-work-tree"])
            .context("verifying repository before remote setup")?;

        if has_remote_named(root, "origin")? {
            let existing_remote = remote_url(root, "origin")?;
            ensure!(
                existing_remote == remote,
                "repository already has origin set to `{existing_remote}`"
            );
        } else {
            run_git(root, ["remote", "add", "origin", remote]).context("adding origin remote")?;
        }

        run_git(root, ["push", "-u", "origin", DEFAULT_BRANCH])
            .context("publishing repository state")?;
        verify_branch_upstream(root).context("verifying branch upstream after remote setup")?;
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

        run_git(&root, ["rev-parse", "--is-inside-work-tree"])?;
        let config_text = fs::read_to_string(root.join("config.toml"))
            .context("reading repository config.toml")?;
        let config: AppConfig = toml::from_str(&config_text).context("parsing config.toml")?;
        config.validate().context("validating config.toml")?;
        ensure!(months_dir.is_dir(), "`{}` is missing", months_dir.display());
        let sync_enabled = has_remote_named(&root, "origin")?;

        let repository = Self {
            root,
            months_dir,
            _meta_dir: meta_dir,
            config,
            sync_enabled,
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
        let mut months = Vec::new();
        for entry in fs::read_dir(&self.months_dir)
            .with_context(|| format!("reading `{}`", self.months_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension() != Some(OsStr::new("toml")) {
                continue;
            }
            months.push(self.load_month(&path)?);
        }
        months.sort_by(|left, right| right.document.month.cmp(&left.document.month));
        Ok(months)
    }

    /// Loads a single month by its stable month identifier.
    pub fn load_month_by_id(&self, month: MonthId) -> Result<LoadedMonth> {
        self.load_month(&self.months_dir.join(month.file_name()))
    }

    /// Builds a new draft month document without writing it yet.
    ///
    /// # Errors
    ///
    /// Returns an error if the target month already exists or an earlier month
    /// cannot be loaded while determining carry-forward balances.
    pub fn create_month_draft(&self, month: MonthId) -> Result<MonthDocument> {
        let path = self.months_dir.join(month.file_name());
        ensure!(!path.exists(), "month `{month}` already exists");
        let previous = self
            .list_months()?
            .into_iter()
            .filter(|entry| entry.document.month < month)
            .max_by_key(|entry| entry.document.month);
        Ok(MonthDocument::new_draft(
            month,
            &self.config,
            previous.as_ref().map(|entry| &entry.calculated),
        ))
    }

    /// Persists a month, commits the change, and pushes it when sync is enabled.
    pub fn save_month(&self, document: &mut MonthDocument) -> Result<()> {
        let path = self.months_dir.join(document.month.file_name());
        document.stamp_updated_now();
        write_atomic(&path, &document.to_pretty_toml(&self.config)?)?;
        self.commit_and_push_month(document.month)
    }

    /// Renames a month file, commits the local rename, and then attempts to push.
    pub fn rename_month(&self, source: MonthId, target: MonthId) -> Result<SyncOutcome> {
        ensure!(
            source != target,
            "month `{source}` is already named `{target}`"
        );
        let source_path = self.months_dir.join(source.file_name());
        let target_path = self.months_dir.join(target.file_name());
        ensure!(source_path.exists(), "month `{source}` does not exist");
        ensure!(!target_path.exists(), "month `{target}` already exists");

        let mut document = self.load_month(&source_path)?.document;
        document.month = target;
        document.stamp_updated_now();
        write_atomic(&target_path, &document.to_pretty_toml(&self.config)?)?;
        fs::remove_file(&source_path)
            .with_context(|| format!("removing old month file `{}`", source_path.display()))?;

        self.commit_paths(
            &[
                PathBuf::from("months").join(source.file_name()),
                PathBuf::from("months").join(target.file_name()),
            ],
            &format!("Rename budget month {source} to {target}"),
        )?;
        self.push_after_local_commit()
    }

    /// Deletes a month file, commits the local deletion, and then attempts to push.
    pub fn delete_month(&self, month: MonthId) -> Result<SyncOutcome> {
        let path = self.months_dir.join(month.file_name());
        ensure!(path.exists(), "month `{month}` does not exist");
        fs::remove_file(&path)
            .with_context(|| format!("removing month file `{}`", path.display()))?;
        self.commit_paths(
            &[PathBuf::from("months").join(month.file_name())],
            &format!("Delete budget month {month}"),
        )?;
        self.push_after_local_commit()
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
        verify_branch_upstream(&self.root).context("checking branch upstream")?;
        run_git(&self.root, ["pull", "--ff-only"]).context("pulling latest budget data")?;
        Ok(())
    }

    fn load_month(&self, path: &Path) -> Result<LoadedMonth> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading month file `{}`", path.display()))?;
        let document: MonthDocument =
            toml::from_str(&text).with_context(|| format!("parsing `{}`", path.display()))?;
        let calculated = calculate_month(&self.config, &document)
            .with_context(|| format!("recomputing derived values for `{}`", path.display()))?;
        Ok(LoadedMonth {
            path: path.to_path_buf(),
            document,
            calculated,
        })
    }

    fn commit_and_push_month(&self, month: MonthId) -> Result<()> {
        let relative = PathBuf::from("months").join(month.file_name());
        self.commit_paths(&[relative], &format!("Update budget month {month}"))?;
        self.push_committed_changes()
    }

    fn commit_paths(&self, relative_paths: &[PathBuf], message: &str) -> Result<()> {
        let relative_text = relative_paths
            .iter()
            .map(|path| {
                path.to_str()
                    .ok_or_else(|| anyhow!("path `{}` is not valid utf-8", path.display()))
            })
            .collect::<Result<Vec<_>>>()?;

        let mut args = vec!["add", "-A"];
        args.extend(relative_text.iter().copied());
        run_git_slice(&self.root, &args).context("staging repository changes")?;

        let status = Command::new("git")
            .args(["diff", "--cached", "--quiet", "--exit-code"])
            .current_dir(&self.root)
            .status()
            .context("checking staged diff")?;
        if status.success() {
            warn!(
                "no staged diff for {:?}; pushing existing commits if needed",
                relative_paths
            );
        } else {
            run_git(&self.root, ["commit", "-m", message])
                .context("committing repository changes")?;
        }
        Ok(())
    }

    fn push_after_local_commit(&self) -> Result<SyncOutcome> {
        match self.push_committed_changes() {
            Ok(()) => Ok(SyncOutcome::Synced),
            Err(error) => Ok(SyncOutcome::PushFailed(error.to_string())),
        }
    }

    fn push_committed_changes(&self) -> Result<()> {
        if self.sync_enabled {
            run_git(&self.root, ["push"]).context("pushing repository changes")?;
        } else {
            info!("no origin remote configured; leaving repository changes committed locally");
        }
        Ok(())
    }
}

fn run_git<const N: usize>(root: &Path, args: [&str; N]) -> Result<String> {
    run_git_slice(root, &args)
}

fn run_git_slice(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .with_context(|| format!("running `git {}` in `{}`", args.join(" "), root.display()))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned());
    }
    bail!(
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    );
}

fn verify_branch_upstream(root: &Path) -> Result<String> {
    run_git(
        root,
        [
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    )
}

fn is_git_work_tree(root: &Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(root)
        .output()
        .with_context(|| {
            format!(
                "running `git rev-parse --is-inside-work-tree` in `{}`",
                root.display()
            )
        })?;
    if output.status.success() {
        return Ok(true);
    }
    if output.status.code().is_some() {
        return Ok(false);
    }
    bail!(
        "git rev-parse --is-inside-work-tree failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
}

fn has_remote_named(root: &Path, remote: &str) -> Result<bool> {
    let remote_key = format!("remote.{remote}.url");
    let output = Command::new("git")
        .args(["config", "--get", &remote_key])
        .current_dir(root)
        .output()
        .with_context(|| {
            format!(
                "running `git config --get {remote_key}` in `{}`",
                root.display()
            )
        })?;
    if output.status.success() {
        return Ok(true);
    }
    if output.status.code() == Some(1) {
        return Ok(false);
    }
    bail!(
        "git config --get {} failed: {}",
        remote_key,
        String::from_utf8_lossy(&output.stderr).trim()
    );
}

fn remote_url(root: &Path, remote: &str) -> Result<String> {
    let remote_key = format!("remote.{remote}.url");
    run_git(root, ["config", "--get", &remote_key])
}

fn ensure_directory_missing_or_empty(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    ensure!(
        path.is_dir(),
        "repository path `{}` must be a directory",
        path.display()
    );

    if fs::read_dir(path)
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

fn write_atomic(path: &Path, contents: &str) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use budget_core::{MonthId, calculate_month};
    use tempfile::tempdir;

    use super::{Repository, SyncOutcome};

    #[test]
    fn init_repo_and_open_repo_gate() {
        let temp = tempdir().unwrap();
        let remote = temp.path().join("remote.git");
        git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);

        let repo = temp.path().join("budget");
        Repository::init(&repo, Some(remote.to_str().unwrap())).unwrap();
        assert_eq!(
            git_capture(
                &repo,
                &[
                    "rev-parse",
                    "--abbrev-ref",
                    "--symbolic-full-name",
                    "@{upstream}"
                ],
            ),
            "origin/main"
        );
        assert_eq!(
            git_capture(&repo, &["config", "--get", "branch.main.remote"]),
            "origin"
        );
        assert_eq!(
            git_capture(&repo, &["config", "--get", "branch.main.merge"]),
            "refs/heads/main"
        );

        let opened = Repository::open(&repo).unwrap();
        assert!(opened.sync_enabled());
        assert!(opened.root().join("config.toml").exists());
        assert!(opened.list_months().unwrap().is_empty());
    }

    #[test]
    fn init_repo_without_remote_opens_repo_gate() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("budget");
        Repository::init(&repo, None).unwrap();

        let config_text = fs::read_to_string(repo.join("config.toml")).unwrap();
        assert!(!config_text.contains("[ui.base24]"));

        let opened = Repository::open(&repo).unwrap();
        assert!(!opened.sync_enabled());
        assert!(opened.root().join("config.toml").exists());
        assert!(opened.list_months().unwrap().is_empty());
    }

    #[test]
    fn clone_from_remote_opens_repo_gate() {
        let temp = tempdir().unwrap();
        let remote = temp.path().join("remote.git");
        git(
            temp.path(),
            &[
                "init",
                "--bare",
                "--initial-branch=main",
                remote.to_str().unwrap(),
            ],
        );

        let source = temp.path().join("source");
        Repository::init(&source, Some(remote.to_str().unwrap())).unwrap();

        let clone = temp.path().join("clone");
        Repository::clone_from_remote(remote.to_str().unwrap(), &clone).unwrap();

        let opened = Repository::open(&clone).unwrap();
        assert!(opened.sync_enabled());
        assert!(opened.root().join("config.toml").exists());
    }

    #[test]
    fn clone_from_remote_rejects_non_empty_target() {
        let temp = tempdir().unwrap();
        let target = temp.path().join("clone");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("note.txt"), "occupied").unwrap();

        let error =
            Repository::clone_from_remote("https://github.com/example/example.git", &target)
                .unwrap_err();
        assert!(
            error.to_string().contains("repository path")
                && error.to_string().contains("must be empty or not exist")
        );
    }

    #[test]
    fn save_month_writes_derived_cache_and_pushes() {
        let temp = tempdir().unwrap();
        let remote = temp.path().join("remote.git");
        git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);
        let repo_path = temp.path().join("budget");
        Repository::init(&repo_path, Some(remote.to_str().unwrap())).unwrap();

        let repository = Repository::open(&repo_path).unwrap();
        let mut month = repository
            .create_month_draft(MonthId::parse("2026-03").unwrap())
            .unwrap();
        month.accounts.insert("current".to_owned(), 100_000);
        month.accounts.insert("cash_isa".to_owned(), 20_000);
        month.accounts.insert("amex_credit".to_owned(), 5_000);
        month.accounts.insert("nationwide_credit".to_owned(), 0);
        month
            .next_month_earmarks
            .insert("subscriptions".to_owned(), 10_000);
        month
            .next_month_earmarks
            .insert("general_spending".to_owned(), 37_500);
        repository.save_month(&mut month).unwrap();

        let opened = repository
            .load_month_by_id(MonthId::parse("2026-03").unwrap())
            .unwrap();
        let recalculated = calculate_month(repository.config(), &opened.document).unwrap();
        assert_eq!(
            opened.calculated.validation.overall_difference,
            recalculated.validation.overall_difference
        );

        let log = git_capture(&repo_path, &["log", "--oneline", "--max-count", "1"]);
        assert!(log.contains("Update budget month 2026-03"));
        let remote_log = git_capture_bare(
            &remote,
            &["log", "--oneline", "--max-count", "1", "refs/heads/main"],
        );
        assert!(remote_log.contains("Update budget month 2026-03"));

        let file_text = fs::read_to_string(repo_path.join("months/2026-03.toml")).unwrap();
        assert!(file_text.contains("[derived]"));
    }

    #[test]
    fn save_month_without_remote_commits_locally() {
        let temp = tempdir().unwrap();
        let repo_path = temp.path().join("budget");
        Repository::init(&repo_path, None).unwrap();

        let repository = Repository::open(&repo_path).unwrap();
        let mut month = repository
            .create_month_draft(MonthId::parse("2026-03").unwrap())
            .unwrap();
        month.accounts.insert("current".to_owned(), 100_000);
        month.accounts.insert("cash_isa".to_owned(), 20_000);
        month.accounts.insert("amex_credit".to_owned(), 5_000);
        month.accounts.insert("nationwide_credit".to_owned(), 0);
        month
            .next_month_earmarks
            .insert("subscriptions".to_owned(), 10_000);
        month
            .next_month_earmarks
            .insert("general_spending".to_owned(), 37_500);
        repository.save_month(&mut month).unwrap();

        let log = git_capture(&repo_path, &["log", "--oneline", "--max-count", "1"]);
        assert!(log.contains("Update budget month 2026-03"));
        assert!(repo_path.join("months/2026-03.toml").exists());
    }

    #[test]
    fn rename_month_updates_persisted_identifier() {
        let temp = tempdir().unwrap();
        let repo_path = temp.path().join("budget");
        Repository::init(&repo_path, None).unwrap();

        let repository = Repository::open(&repo_path).unwrap();
        let mut month = repository
            .create_month_draft(MonthId::parse("2026-03").unwrap())
            .unwrap();
        repository.save_month(&mut month).unwrap();
        let outcome = repository
            .rename_month(
                MonthId::parse("2026-03").unwrap(),
                MonthId::parse("2026-04").unwrap(),
            )
            .unwrap();
        assert_eq!(outcome, SyncOutcome::Synced);

        assert!(!repo_path.join("months/2026-03.toml").exists());
        let renamed_path = repo_path.join("months/2026-04.toml");
        assert!(renamed_path.exists());
        let renamed_text = fs::read_to_string(&renamed_path).unwrap();
        assert!(renamed_text.contains("month = \"2026-04\""));
        let log = git_capture(&repo_path, &["log", "--oneline", "--max-count", "1"]);
        assert!(log.contains("Rename budget month 2026-03 to 2026-04"));
    }

    #[test]
    fn delete_month_removes_file_and_commits() {
        let temp = tempdir().unwrap();
        let repo_path = temp.path().join("budget");
        Repository::init(&repo_path, None).unwrap();

        let repository = Repository::open(&repo_path).unwrap();
        let mut month = repository
            .create_month_draft(MonthId::parse("2026-03").unwrap())
            .unwrap();
        repository.save_month(&mut month).unwrap();
        let outcome = repository
            .delete_month(MonthId::parse("2026-03").unwrap())
            .unwrap();
        assert_eq!(outcome, SyncOutcome::Synced);

        assert!(!repo_path.join("months/2026-03.toml").exists());
        assert!(repository.list_months().unwrap().is_empty());
        let log = git_capture(&repo_path, &["log", "--oneline", "--max-count", "1"]);
        assert!(log.contains("Delete budget month 2026-03"));
    }

    #[test]
    fn rename_month_keeps_local_commit_when_push_fails() {
        let temp = tempdir().unwrap();
        let remote = temp.path().join("remote.git");
        git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);

        let repo_path = temp.path().join("budget");
        Repository::init(&repo_path, Some(remote.to_str().unwrap())).unwrap();

        let repository = Repository::open(&repo_path).unwrap();
        let mut month = repository
            .create_month_draft(MonthId::parse("2026-03").unwrap())
            .unwrap();
        repository.save_month(&mut month).unwrap();

        git(
            &repo_path,
            &[
                "config",
                "remote.origin.url",
                "/definitely/missing/repo.git",
            ],
        );

        let outcome = repository
            .rename_month(
                MonthId::parse("2026-03").unwrap(),
                MonthId::parse("2026-04").unwrap(),
            )
            .unwrap();

        match outcome {
            SyncOutcome::PushFailed(message) => {
                assert!(message.contains("pushing repository changes"));
            }
            other => panic!("expected push failure, got {other:?}"),
        }

        assert!(!repo_path.join("months/2026-03.toml").exists());
        assert!(repo_path.join("months/2026-04.toml").exists());
        let log = git_capture(&repo_path, &["log", "--oneline", "--max-count", "1"]);
        assert!(log.contains("Rename budget month 2026-03 to 2026-04"));
    }

    #[test]
    fn delete_month_keeps_local_commit_when_push_fails() {
        let temp = tempdir().unwrap();
        let remote = temp.path().join("remote.git");
        git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);

        let repo_path = temp.path().join("budget");
        Repository::init(&repo_path, Some(remote.to_str().unwrap())).unwrap();

        let repository = Repository::open(&repo_path).unwrap();
        let mut month = repository
            .create_month_draft(MonthId::parse("2026-03").unwrap())
            .unwrap();
        repository.save_month(&mut month).unwrap();

        git(
            &repo_path,
            &[
                "config",
                "remote.origin.url",
                "/definitely/missing/repo.git",
            ],
        );

        let outcome = repository
            .delete_month(MonthId::parse("2026-03").unwrap())
            .unwrap();

        match outcome {
            SyncOutcome::PushFailed(message) => {
                assert!(message.contains("pushing repository changes"));
            }
            other => panic!("expected push failure, got {other:?}"),
        }

        assert!(!repo_path.join("months/2026-03.toml").exists());
        let log = git_capture(&repo_path, &["log", "--oneline", "--max-count", "1"]);
        assert!(log.contains("Delete budget month 2026-03"));
    }

    #[test]
    fn connect_remote_publishes_existing_local_repo() {
        let temp = tempdir().unwrap();
        let remote = temp.path().join("remote.git");
        git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);

        let repo_path = temp.path().join("budget");
        Repository::init(&repo_path, None).unwrap();

        Repository::connect_remote(&repo_path, remote.to_str().unwrap()).unwrap();

        assert!(Repository::has_origin_remote(&repo_path).unwrap());
        assert_eq!(
            git_capture(
                &repo_path,
                &[
                    "rev-parse",
                    "--abbrev-ref",
                    "--symbolic-full-name",
                    "@{upstream}"
                ],
            ),
            "origin/main"
        );
    }

    #[test]
    fn looks_like_budget_repo_accepts_initialised_repository() {
        let temp = tempdir().unwrap();
        let repo_path = temp.path().join("budget");
        Repository::init(&repo_path, None).unwrap();

        assert!(Repository::looks_like_budget_repo(&repo_path).unwrap());
    }

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(root)
            .status()
            .unwrap();
        assert!(status.success(), "git {:?} failed", args);
    }

    fn git_capture(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {:?} failed", args);
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn git_capture_bare(git_dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("--git-dir")
            .arg(git_dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git --git-dir {:?} {:?} failed",
            git_dir,
            args
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }
}
