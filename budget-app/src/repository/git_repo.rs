use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, anyhow, bail, ensure};
const DEFAULT_BRANCH: &str = "main";

#[derive(Clone, Debug)]
pub(super) struct GitRepo {
    root: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TrackedCommitOutcome {
    Committed,
    PathNotTracked,
}

impl GitRepo {
    pub(super) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub(super) fn initialise_budget_repo(&self) -> Result<()> {
        self.run(["init", "-b", DEFAULT_BRANCH])?;
        self.run(["config", "user.name", "jt-budget"])?;
        self.run(["config", "user.email", "jt-budget@example.invalid"])?;
        self.run(["add", "--", "."])?;
        self.run(["commit", "-m", "Initialise budget repository"])?;
        Ok(())
    }

    pub(super) fn ensure_repository(&self) -> Result<()> {
        self.run(["rev-parse", "--is-inside-work-tree"])
            .context("verifying repository before remote setup")?;
        Ok(())
    }

    pub(super) fn has_remote_named(&self, remote: &str) -> Result<bool> {
        let output = Command::new("git")
            .args(["remote", "get-url", remote])
            .current_dir(&self.root)
            .output()
            .with_context(|| {
                format!(
                    "running `git remote get-url {remote}` in `{}`",
                    self.root.display()
                )
            })?;
        if output.status.success() {
            return Ok(true);
        }
        if output.status.code() == Some(2) {
            return Ok(false);
        }
        bail!(
            "{}",
            git_failure_message(&["remote", "get-url", remote], &output)
        );
    }

    pub(super) fn has_remote_named_for_startup(&self, remote: &str) -> Result<bool> {
        if self.has_remote_named_in_plain_config(remote)? == Some(true) {
            return Ok(true);
        }

        self.has_remote_named(remote)
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
        self.run(["pull", "--ff-only", "--quiet"]).map(|_| ())
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

        let mut args = vec!["add", "-A", "--"];
        args.extend(relative_text.iter().copied());
        self.run_slice(&args)?;

        self.run(["commit", "-m", message])
            .context("committing repository changes")?;
        Ok(())
    }

    pub(super) fn commit_tracked_path(
        &self,
        relative_path: &Path,
        message: &str,
    ) -> Result<TrackedCommitOutcome> {
        let relative_text = relative_path
            .to_str()
            .ok_or_else(|| anyhow!("path `{}` is not valid utf-8", relative_path.display()))?;
        let args = [
            "commit",
            "--quiet",
            "--only",
            "-m",
            message,
            "--",
            relative_text,
        ];
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
            return Ok(TrackedCommitOutcome::Committed);
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("did not match any file(s) known to git") {
            return Ok(TrackedCommitOutcome::PathNotTracked);
        }

        bail!("{}", git_failure_message(&args, &output));
    }

    pub(super) fn push(&self) -> Result<()> {
        self.run(["push"]).map(|_| ())
    }

    pub(super) fn commit_monthly_defaults(&self) -> Result<()> {
        // A push retry must not attempt an empty commit or include unrelated
        // staged files in the user's budget repository.
        if !self
            .run(["diff", "--name-only", "HEAD", "--", "config.toml"])?
            .is_empty()
        {
            ensure!(
                self.commit_tracked_path(
                    Path::new("config.toml"),
                    "Update monthly budget defaults"
                )? == TrackedCommitOutcome::Committed,
                "config.toml must be tracked before defaults can be saved"
            );
        }
        Ok(())
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
            "{}",
            git_failure_message(&["rev-parse", "--is-inside-work-tree"], &output)
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
        bail!("{}", git_failure_message(args, &output));
    }

    fn has_remote_named_in_plain_config(&self, remote: &str) -> Result<Option<bool>> {
        let config_path = self.root.join(".git/config");
        if !config_path.is_file() {
            return Ok(None);
        }

        let config = std::fs::read_to_string(&config_path)
            .with_context(|| format!("reading `{}`", config_path.display()))?;
        Ok(Some(git_config_has_remote_url(&config, remote)))
    }
}

pub(super) fn clone_from_remote(remote: &str, target: &Path) -> Result<()> {
    let output = Command::new("git")
        .arg("clone")
        .arg("--origin")
        .arg("origin")
        .arg("--")
        .arg(remote)
        .arg(target)
        .output()
        .with_context(|| {
            format!(
                "running `git clone --origin origin -- {remote} {}`",
                target.display()
            )
        })?;
    if output.status.success() {
        return Ok(());
    }

    bail!(
        "{}",
        git_failure_message(&["clone", "--origin", "origin", "--", remote], &output)
    );
}

fn git_failure_message(args: &[&str], output: &Output) -> String {
    format!(
        "git {} failed\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

fn git_config_has_remote_url(config: &str, remote: &str) -> bool {
    let remote_section = format!(r#"[remote "{remote}"]"#);
    let mut in_remote_section = false;

    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_remote_section = line == remote_section;
            continue;
        }

        if !in_remote_section {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("url") && !value.trim().is_empty() {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::git_config_has_remote_url;

    #[test]
    fn git_config_remote_detection_requires_url_in_matching_section() {
        let config = r#"
[core]
	repositoryformatversion = 0
[remote "upstream"]
	url = ../upstream.git
[remote "origin"]
	fetch = +refs/heads/*:refs/remotes/origin/*
	url = ../origin.git
"#;

        assert!(git_config_has_remote_url(config, "origin"));
        assert!(git_config_has_remote_url(config, "upstream"));
        assert!(!git_config_has_remote_url(config, "missing"));
    }

    #[test]
    fn git_config_remote_detection_ignores_empty_or_unrelated_url() {
        let config = r#"
[remote "upstream"]
	url = ../upstream.git
[remote "origin"]
	url =
[branch "main"]
	remote = origin
"#;

        assert!(!git_config_has_remote_url(config, "origin"));
    }
}
