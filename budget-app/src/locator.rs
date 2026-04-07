use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, ensure};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

const LOCATOR_SCHEMA_VERSION: u32 = 1;

/// Stores and loads the default budget repository path outside the repo itself.
#[derive(Clone, Debug)]
pub struct RepoLocator {
    config_path: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
struct LocatorFile {
    version: u32,
    repo: PathBuf,
}

impl RepoLocator {
    /// Builds the standard locator path for the current user account.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform config directory cannot be resolved.
    pub fn for_current_user() -> Result<Self> {
        let config_dir =
            dirs::config_dir().ok_or_else(|| anyhow!("could not determine config directory"))?;
        Ok(Self::new(config_dir.join("jt-budget").join("config.toml")))
    }

    /// Builds a locator that persists to the provided config path.
    pub fn new(config_path: PathBuf) -> Self {
        Self { config_path }
    }

    /// Loads the saved default repository path if one has been configured.
    ///
    /// # Errors
    ///
    /// Returns an error when the locator file cannot be read or parsed.
    pub fn load(&self) -> Result<Option<PathBuf>> {
        if !self.config_path.exists() {
            return Ok(None);
        }

        let text = fs::read_to_string(&self.config_path)
            .with_context(|| format!("reading `{}`", self.config_path.display()))?;
        let locator: LocatorFile = toml::from_str(&text)
            .with_context(|| format!("parsing `{}`", self.config_path.display()))?;
        ensure!(
            locator.version == LOCATOR_SCHEMA_VERSION,
            "locator `{}` uses unsupported schema version {}",
            self.config_path.display(),
            locator.version
        );
        ensure!(
            !locator.repo.as_os_str().is_empty(),
            "locator `{}` is missing a repository path",
            self.config_path.display()
        );
        Ok(Some(locator.repo))
    }

    /// Saves the canonical repository path as the default launch target.
    ///
    /// # Errors
    ///
    /// Returns an error when the locator directory cannot be created or the
    /// locator file cannot be written atomically.
    pub fn save(&self, repo: &Path) -> Result<()> {
        let repo = repo
            .canonicalize()
            .with_context(|| format!("resolving repository path `{}`", repo.display()))?;
        let locator = LocatorFile {
            version: LOCATOR_SCHEMA_VERSION,
            repo,
        };
        let text = toml::to_string_pretty(&locator).context("serialising repo locator")?;
        let parent = self.config_path.parent().ok_or_else(|| {
            anyhow!(
                "locator path `{}` has no parent",
                self.config_path.display()
            )
        })?;
        fs::create_dir_all(parent).with_context(|| format!("creating `{}`", parent.display()))?;
        let mut temp = NamedTempFile::new_in(parent)
            .with_context(|| format!("creating temp locator in `{}`", parent.display()))?;
        temp.write_all(text.as_bytes()).with_context(|| {
            format!("writing temp locator for `{}`", self.config_path.display())
        })?;
        temp.flush().with_context(|| {
            format!("flushing temp locator for `{}`", self.config_path.display())
        })?;
        temp.as_file().sync_all().with_context(|| {
            format!("syncing temp locator for `{}`", self.config_path.display())
        })?;
        temp.persist(&self.config_path)
            .map_err(|error| anyhow!(error.error))
            .with_context(|| format!("persisting `{}`", self.config_path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::RepoLocator;

    #[test]
    fn saves_and_loads_locator_path() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("budget");
        std::fs::create_dir_all(&repo).unwrap();
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));

        locator.save(&repo).unwrap();

        assert_eq!(locator.load().unwrap(), Some(repo.canonicalize().unwrap()));
    }

    #[test]
    fn missing_locator_returns_none() {
        let temp = tempdir().unwrap();
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));
        assert_eq!(locator.load().unwrap(), None);
    }
}
