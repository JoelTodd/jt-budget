use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use tracing::warn;

const DEFAULT_BRANCH: &str = "main";

#[derive(Clone, Debug)]
pub(super) struct GitRepo {
    root: PathBuf,
}

impl GitRepo {
    pub(super) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub(super) fn initialise_budget_repo(&self) -> Result<()> {
        self.run(["init", "-b", DEFAULT_BRANCH])?;
        self.run(["config", "user.name", "jt-budget"])?;
        self.run(["config", "user.email", "jt-budget@example.invalid"])?;
        self.run(["add", "."])?;
        self.run(["commit", "-m", "Initialise budget repository"])?;
        Ok(())
    }

    pub(super) fn ensure_repository(&self) -> Result<()> {
        self.run(["rev-parse", "--is-inside-work-tree"])
            .context("verifying repository before remote setup")?;
        Ok(())
    }

    pub(super) fn has_remote_named(&self, remote: &str) -> Result<bool> {
        let remote_key = format!("remote.{remote}.url");
        let output = Command::new("git")
            .args(["config", "--get", &remote_key])
            .current_dir(&self.root)
            .output()
            .with_context(|| {
                format!(
                    "running `git config --get {remote_key}` in `{}`",
                    self.root.display()
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

    pub(super) fn remote_url(&self, remote: &str) -> Result<String> {
        let remote_key = format!("remote.{remote}.url");
        self.run(["config", "--get", &remote_key])
    }

    pub(super) fn add_remote(&self, remote_name: &str, remote: &str) -> Result<()> {
        self.run(["remote", "add", remote_name, remote]).map(|_| ())
    }

    pub(super) fn push_default_branch_with_upstream(&self, remote: &str) -> Result<()> {
        self.run(["push", "-u", remote, DEFAULT_BRANCH]).map(|_| ())
    }

    pub(super) fn verify_branch_upstream(&self) -> Result<String> {
        self.run([
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ])
    }

    pub(super) fn pull_ff_only(&self) -> Result<()> {
        self.run(["pull", "--ff-only"]).map(|_| ())
    }

    pub(super) fn stage_and_commit_paths(
        &self,
        relative_paths: &[PathBuf],
        message: &str,
    ) -> Result<()> {
        let relative_text = relative_paths
            .iter()
            .map(|path| {
                path.to_str()
                    .ok_or_else(|| anyhow!("path `{}` is not valid utf-8", path.display()))
            })
            .collect::<Result<Vec<_>>>()?;

        let mut args = vec!["add", "-A"];
        args.extend(relative_text.iter().copied());
        self.run_slice(&args)?;

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
            self.run(["commit", "-m", message])
                .context("committing repository changes")?;
        }
        Ok(())
    }

    pub(super) fn push(&self) -> Result<()> {
        self.run(["push"]).map(|_| ())
    }

    pub(super) fn is_work_tree(root: &Path) -> Result<bool> {
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

    fn run<const N: usize>(&self, args: [&str; N]) -> Result<String> {
        self.run_slice(&args)
    }

    fn run_slice(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .with_context(|| {
                format!(
                    "running `git {}` in `{}`",
                    args.join(" "),
                    self.root.display()
                )
            })?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned());
        }
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
}

pub(super) fn clone_from_remote(remote: &str, target: &Path) -> Result<()> {
    let output = Command::new("git")
        .arg("clone")
        .arg("--origin")
        .arg("origin")
        .arg(remote)
        .arg(target)
        .stdout(Stdio::null())
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
