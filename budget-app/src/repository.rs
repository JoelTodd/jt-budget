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

impl Repository {
    pub fn init(root: &Path, remote: Option<&str>) -> Result<()> {
        ensure!(
            !root.exists() || fs::read_dir(root)?.next().transpose()?.is_none(),
            "repository path `{}` must be empty or not exist",
            root.display()
        );

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
        if let Some(remote) = remote {
            run_git(root, ["remote", "add", "origin", remote])?;
        }
        run_git(root, ["add", "."])?;
        run_git(root, ["commit", "-m", "Initialize budget repository"])?;
        if remote.is_some() {
            run_git(root, ["push", "origin", DEFAULT_BRANCH])
                .context("publishing initial repository state")?;
            configure_branch_upstream(root, "origin", DEFAULT_BRANCH)
                .context("configuring branch upstream after init")?;
            verify_branch_upstream(root).context("verifying branch upstream after init")?;
        }

        Ok(())
    }

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

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn sync_enabled(&self) -> bool {
        self.sync_enabled
    }

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

    pub fn load_month_by_id(&self, month: MonthId) -> Result<LoadedMonth> {
        self.load_month(&self.months_dir.join(month.file_name()))
    }

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

    pub fn save_month(&self, document: &mut MonthDocument) -> Result<()> {
        let path = self.months_dir.join(document.month.file_name());
        document.stamp_updated_now();
        write_atomic(&path, &document.to_pretty_toml(&self.config)?)?;
        self.commit_and_push_month(document.month)
    }

    pub fn rename_month(&self, source: MonthId, target: MonthId) -> Result<()> {
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

        self.commit_paths_and_push(
            &[
                PathBuf::from("months").join(source.file_name()),
                PathBuf::from("months").join(target.file_name()),
            ],
            &format!("Rename budget month {source} to {target}"),
        )
    }

    pub fn delete_month(&self, month: MonthId) -> Result<()> {
        let path = self.months_dir.join(month.file_name());
        ensure!(path.exists(), "month `{month}` does not exist");
        fs::remove_file(&path)
            .with_context(|| format!("removing month file `{}`", path.display()))?;
        self.commit_paths_and_push(
            &[PathBuf::from("months").join(month.file_name())],
            &format!("Delete budget month {month}"),
        )
    }

    pub fn retry_push_for_month(&self, month: MonthId) -> Result<()> {
        self.commit_and_push_month(month)
    }

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
        self.commit_paths_and_push(&[relative], &format!("Update budget month {month}"))
    }

    fn commit_paths_and_push(&self, relative_paths: &[PathBuf], message: &str) -> Result<()> {
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

fn configure_branch_upstream(root: &Path, remote: &str, branch: &str) -> Result<()> {
    let branch_remote_key = format!("branch.{branch}.remote");
    let branch_merge_key = format!("branch.{branch}.merge");
    let merge_ref = format!("refs/heads/{branch}");
    run_git(root, ["config", &branch_remote_key, remote])?;
    run_git(root, ["config", &branch_merge_key, &merge_ref])?;
    Ok(())
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
    temp.persist(path)
        .map_err(|error| anyhow!(error.error))
        .with_context(|| format!("persisting `{}`", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use budget_core::{MonthId, calculate_month};
    use tempfile::tempdir;

    use super::Repository;

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

        let opened = Repository::open(&repo).unwrap();
        assert!(!opened.sync_enabled());
        assert!(opened.root().join("config.toml").exists());
        assert!(opened.list_months().unwrap().is_empty());
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
        repository
            .rename_month(
                MonthId::parse("2026-03").unwrap(),
                MonthId::parse("2026-04").unwrap(),
            )
            .unwrap();

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
        repository
            .delete_month(MonthId::parse("2026-03").unwrap())
            .unwrap();

        assert!(!repo_path.join("months/2026-03.toml").exists());
        assert!(repository.list_months().unwrap().is_empty());
        let log = git_capture(&repo_path, &["log", "--oneline", "--max-count", "1"]);
        assert!(log.contains("Delete budget month 2026-03"));
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
