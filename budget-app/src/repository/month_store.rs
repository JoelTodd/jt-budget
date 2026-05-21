use std::cmp::Reverse;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use budget_core::{AppConfig, MonthDocument, MonthId, calculate_month};

use super::LoadedMonth;
use super::fs_store::write_atomic;

#[derive(Clone, Debug)]
pub(super) struct MonthStore {
    months_dir: PathBuf,
    config: AppConfig,
}

impl MonthStore {
    pub(super) fn new(months_dir: PathBuf, config: AppConfig) -> Self {
        Self { months_dir, config }
    }

    pub(super) fn list_months(&self) -> Result<Vec<LoadedMonth>> {
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
        months.sort_by_key(|month| Reverse(month.document.month));
        Ok(months)
    }

    pub(super) fn load_month_by_id(&self, month: MonthId) -> Result<LoadedMonth> {
        self.load_month(&self.path_for_month(month))
    }

    pub(super) fn create_month_draft(&self, month: MonthId) -> Result<MonthDocument> {
        let path = self.path_for_month(month);
        ensure!(!path.exists(), "month `{month}` already exists");
        let previous = self.latest_month_before(month)?;
        Ok(MonthDocument::new_draft(
            month,
            &self.config,
            previous.as_ref().map(|entry| &entry.calculated),
        ))
    }

    pub(super) fn write_month(&self, document: &mut MonthDocument) -> Result<()> {
        let path = self.path_for_month(document.month);
        document.stamp_updated_now();
        write_atomic(&path, &document.to_pretty_toml(&self.config)?)?;
        Ok(())
    }

    pub(super) fn rename_month(&self, source: MonthId, target: MonthId) -> Result<()> {
        ensure!(
            source != target,
            "month `{source}` is already named `{target}`"
        );
        let source_path = self.path_for_month(source);
        let target_path = self.path_for_month(target);
        ensure!(source_path.exists(), "month `{source}` does not exist");
        ensure!(!target_path.exists(), "month `{target}` already exists");

        let mut document = self.load_month(&source_path)?.document;
        document.month = target;
        document.stamp_updated_now();
        write_atomic(&target_path, &document.to_pretty_toml(&self.config)?)?;
        fs::remove_file(&source_path)
            .with_context(|| format!("removing old month file `{}`", source_path.display()))?;
        Ok(())
    }

    pub(super) fn delete_month(&self, month: MonthId) -> Result<()> {
        let path = self.path_for_month(month);
        ensure!(path.exists(), "month `{month}` does not exist");
        fs::remove_file(&path)
            .with_context(|| format!("removing month file `{}`", path.display()))?;
        Ok(())
    }

    fn latest_month_before(&self, month: MonthId) -> Result<Option<LoadedMonth>> {
        let mut candidate: Option<(MonthId, PathBuf)> = None;
        for entry in fs::read_dir(&self.months_dir)
            .with_context(|| format!("reading `{}`", self.months_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            let Some(entry_month) = month_id_from_path(&path) else {
                continue;
            };
            if entry_month >= month {
                continue;
            }
            if candidate
                .as_ref()
                .is_none_or(|(best_month, _)| entry_month > *best_month)
            {
                candidate = Some((entry_month, path));
            }
        }

        candidate
            .map(|(_, path)| self.load_month(&path))
            .transpose()
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

    fn path_for_month(&self, month: MonthId) -> PathBuf {
        self.months_dir.join(month.file_name())
    }
}

fn month_id_from_path(path: &Path) -> Option<MonthId> {
    if path.extension() != Some(OsStr::new("toml")) {
        return None;
    }

    path.file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| MonthId::parse(stem).ok())
}
