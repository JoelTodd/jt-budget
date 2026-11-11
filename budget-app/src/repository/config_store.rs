use std::fs;

use anyhow::{Context, Result, ensure};
use budget_core::AppConfig;

use super::Repository;
use super::fs_store::write_atomic;
use super::month_store::MonthStore;

impl Repository {
    /// Saves monthly contribution and earmark defaults, commits, and syncs them.
    ///
    /// Existing month files retain their editable amounts. Only monetary
    /// defaults may change through this boundary; structure and validation
    /// rules remain those of the loaded configuration.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid defaults, concurrent config edits, write or
    /// commit failures, or sync failures. Retrying is safe after a local commit.
    pub fn save_monthly_defaults(&mut self, config: &AppConfig) -> Result<()> {
        config.validate().context("validating defaults")?;
        let mut structure = config.clone();
        for (pot, existing) in structure
            .savings_pots
            .iter_mut()
            .zip(&self.config.savings_pots)
        {
            ensure!(
                pot.default_monthly_change_minor >= 0,
                "contribution defaults must be non-negative"
            );
            pot.default_monthly_change_minor = existing.default_monthly_change_minor;
        }
        for (item, existing) in structure
            .next_month_earmarks
            .iter_mut()
            .zip(&self.config.next_month_earmarks)
        {
            ensure!(
                item.default_amount_minor >= 0,
                "earmark defaults must be non-negative"
            );
            item.default_amount_minor = existing.default_amount_minor;
        }
        ensure!(
            structure == self.config,
            "only monthly monetary defaults can be changed"
        );

        let path = self.root.join("config.toml");
        let text =
            fs::read_to_string(&path).context("reading config.toml before saving defaults")?;
        let disk_config: AppConfig = toml::from_str(&text).context("parsing config.toml")?;
        // The desired config may already be on disk after a failed save. Any
        // other external edit must be refreshed rather than overwritten.
        ensure!(
            disk_config == self.config || disk_config == *config,
            "config.toml changed outside the app; restart to load it before editing defaults"
        );
        if disk_config != *config {
            // Preserve config sections and fields this version does not own.
            let mut value: toml::Value = toml::from_str(&text)?;
            for (section, key, defaults) in [
                (
                    "savings_pots",
                    "default_monthly_change_minor",
                    config
                        .savings_pots
                        .iter()
                        .map(|pot| pot.default_monthly_change_minor)
                        .collect::<Vec<_>>(),
                ),
                (
                    "next_month_earmarks",
                    "default_amount_minor",
                    config
                        .next_month_earmarks
                        .iter()
                        .map(|item| item.default_amount_minor)
                        .collect::<Vec<_>>(),
                ),
            ] {
                if let Some(rows) = value.get_mut(section).and_then(toml::Value::as_array_mut) {
                    for (row, amount) in rows.iter_mut().zip(defaults) {
                        row[key] = toml::Value::Integer(amount);
                    }
                }
            }
            write_atomic(&path, &toml::to_string_pretty(&value)?).context("saving config.toml")?;
        }
        self.git.commit_monthly_defaults()?;
        self.reload_config()?;
        self.push_committed_changes()?;
        Ok(())
    }

    pub(super) fn reload_config(&mut self) -> Result<()> {
        let text = fs::read_to_string(self.root.join("config.toml"))
            .context("reading repository config.toml")?;
        let config: AppConfig = toml::from_str(&text).context("parsing config.toml")?;
        config.validate().context("validating config.toml")?;
        self.months = MonthStore::new(self.root.join("months"), config.clone());
        self.config = config;
        Ok(())
    }
}
