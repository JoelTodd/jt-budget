use std::cmp::Reverse;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, anyhow, ensure};
use budget_core::{AppConfig, CalculatedMonth, MonthDocument, MonthId, calculate_month};

use super::fs_store::write_atomic;
use super::{LoadedMonth, MonthFileFingerprint};

const PARALLEL_LIST_THRESHOLD: usize = 256;
const MAX_LIST_WORKERS: usize = 8;

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
        let paths = self.month_paths()?;
        let mut months = if should_parallelise_list(paths.len()) {
            self.load_months_parallel(&paths)?
        } else {
            paths
                .iter()
                .map(|path| self.load_month(path))
                .collect::<Result<Vec<_>>>()?
        };
        months.sort_by_key(|month| Reverse(month.document.month));
        Ok(months)
    }

    pub(super) fn list_months_with_fingerprint(
        &self,
    ) -> Result<(Vec<LoadedMonth>, MonthFileFingerprint)> {
        let paths = self.month_paths()?;
        let loaded = if should_parallelise_list(paths.len()) {
            self.load_months_with_fingerprint_parallel(&paths)?
        } else {
            paths
                .iter()
                .map(|path| self.load_month_with_fingerprint(path))
                .collect::<Result<Vec<_>>>()?
        };
        let mut months = Vec::with_capacity(loaded.len());
        let mut fingerprint = Vec::with_capacity(loaded.len());
        for (loaded, entry_fingerprint) in loaded {
            months.push(loaded);
            fingerprint.push(entry_fingerprint);
        }
        months.sort_by_key(|month| Reverse(month.document.month));
        fingerprint.sort_by_key(|(month, _)| Reverse(*month));
        Ok((months, fingerprint))
    }

    pub(super) fn month_file_fingerprint(&self) -> Result<MonthFileFingerprint> {
        let mut fingerprint = Vec::new();
        for entry in fs::read_dir(&self.months_dir)
            .with_context(|| format!("reading `{}`", self.months_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension() != Some(OsStr::new("toml")) {
                continue;
            }
            let Some(month) = month_id_from_path(&path) else {
                continue;
            };
            fingerprint.push((month, month_file_fingerprint(&path)?));
        }
        fingerprint.sort_by_key(|(month, _)| Reverse(*month));
        Ok(fingerprint)
    }

    pub(super) fn month_file_fingerprint_for_month(
        &self,
        month: MonthId,
    ) -> Result<(MonthId, u64)> {
        Ok((month, month_file_fingerprint(&self.path_for_month(month))?))
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

    pub(super) fn create_month_draft_with_cached_previous(
        &self,
        month: MonthId,
        cached_previous: Option<(MonthId, &CalculatedMonth, u64)>,
    ) -> Result<MonthDocument> {
        let path = self.path_for_month(month);
        ensure!(!path.exists(), "month `{month}` already exists");

        let latest = self.latest_month_id_before(month)?;
        let previous = match (latest, cached_previous) {
            (Some(latest), Some((cached_month, cached_calculated, cached_fingerprint)))
                if latest == cached_month
                    && month_file_fingerprint(&self.path_for_month(latest))?
                        == cached_fingerprint =>
            {
                Some(cached_calculated)
            }
            (Some(latest), _) => {
                let loaded = self.load_month(&self.path_for_month(latest))?;
                return Ok(MonthDocument::new_draft(
                    month,
                    &self.config,
                    Some(&loaded.calculated),
                ));
            }
            (None, _) => None,
        };

        Ok(MonthDocument::new_draft(month, &self.config, previous))
    }

    pub(super) fn write_month(&self, document: &mut MonthDocument) -> Result<bool> {
        let path = self.path_for_month(document.month);
        let existed_before_write = path.exists();
        document.stamp_updated_now();
        write_atomic(&path, &document.to_pretty_toml(&self.config)?)?;
        Ok(existed_before_write)
    }

    pub(super) fn rename_month(&self, source: MonthId, target: MonthId) -> Result<LoadedMonth> {
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
        let calculated = calculate_month(&self.config, &document).with_context(|| {
            format!("recomputing derived values for `{}`", target_path.display())
        })?;
        write_atomic(&target_path, &document.to_pretty_toml(&self.config)?)?;
        fs::remove_file(&source_path)
            .with_context(|| format!("removing old month file `{}`", source_path.display()))?;
        Ok(LoadedMonth {
            path: target_path,
            document,
            calculated,
        })
    }

    pub(super) fn delete_month(&self, month: MonthId) -> Result<()> {
        let path = self.path_for_month(month);
        ensure!(path.exists(), "month `{month}` does not exist");
        fs::remove_file(&path)
            .with_context(|| format!("removing month file `{}`", path.display()))?;
        Ok(())
    }

    fn latest_month_before(&self, month: MonthId) -> Result<Option<LoadedMonth>> {
        self.latest_month_id_before(month)?
            .map(|month| self.load_month(&self.path_for_month(month)))
            .transpose()
    }

    fn latest_month_id_before(&self, month: MonthId) -> Result<Option<MonthId>> {
        let mut candidate: Option<MonthId> = None;
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
                .is_none_or(|best_month| entry_month > *best_month)
            {
                candidate = Some(entry_month);
            }
        }

        Ok(candidate)
    }

    fn load_month(&self, path: &Path) -> Result<LoadedMonth> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading month file `{}`", path.display()))?;
        self.load_month_from_text(path, &text)
    }

    fn load_month_with_fingerprint(&self, path: &Path) -> Result<(LoadedMonth, (MonthId, u64))> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading month file `{}`", path.display()))?;
        let loaded = self.load_month_from_text(path, &text)?;
        let fingerprint = (loaded.document.month, month_file_fingerprint(path)?);
        Ok((loaded, fingerprint))
    }

    fn load_month_from_text(&self, path: &Path, text: &str) -> Result<LoadedMonth> {
        let document: MonthDocument =
            toml::from_str(text).with_context(|| format!("parsing `{}`", path.display()))?;
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

    fn month_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for entry in fs::read_dir(&self.months_dir)
            .with_context(|| format!("reading `{}`", self.months_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension() == Some(OsStr::new("toml")) {
                paths.push(path);
            }
        }
        Ok(paths)
    }

    fn load_months_parallel(&self, paths: &[PathBuf]) -> Result<Vec<LoadedMonth>> {
        let chunk_size = parallel_chunk_size(paths.len());
        thread::scope(|scope| {
            let handles = paths
                .chunks(chunk_size)
                .map(|chunk| {
                    scope.spawn(move || {
                        chunk
                            .iter()
                            .map(|path| self.load_month(path))
                            .collect::<Result<Vec<_>>>()
                    })
                })
                .collect::<Vec<_>>();

            let mut months = Vec::with_capacity(paths.len());
            for handle in handles {
                months.extend(
                    handle
                        .join()
                        .map_err(|_| anyhow!("month loader worker panicked"))??,
                );
            }
            Ok(months)
        })
    }

    fn load_months_with_fingerprint_parallel(
        &self,
        paths: &[PathBuf],
    ) -> Result<Vec<(LoadedMonth, (MonthId, u64))>> {
        let chunk_size = parallel_chunk_size(paths.len());
        thread::scope(|scope| {
            let handles = paths
                .chunks(chunk_size)
                .map(|chunk| {
                    scope.spawn(move || {
                        chunk
                            .iter()
                            .map(|path| self.load_month_with_fingerprint(path))
                            .collect::<Result<Vec<_>>>()
                    })
                })
                .collect::<Vec<_>>();

            let mut loaded = Vec::with_capacity(paths.len());
            for handle in handles {
                loaded.extend(
                    handle
                        .join()
                        .map_err(|_| anyhow!("month loader worker panicked"))??,
                );
            }
            Ok(loaded)
        })
    }
}

fn should_parallelise_list(month_count: usize) -> bool {
    month_count >= PARALLEL_LIST_THRESHOLD && parallel_worker_count(month_count) > 1
}

fn parallel_worker_count(item_count: usize) -> usize {
    thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(MAX_LIST_WORKERS)
        .min(item_count.max(1))
}

fn parallel_chunk_size(item_count: usize) -> usize {
    item_count.div_ceil(parallel_worker_count(item_count))
}

fn month_file_fingerprint(path: &Path) -> Result<u64> {
    let metadata =
        fs::metadata(path).with_context(|| format!("reading metadata for `{}`", path.display()))?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    Ok(metadata.len() ^ modified)
}

fn month_id_from_path(path: &Path) -> Option<MonthId> {
    if path.extension() != Some(OsStr::new("toml")) {
        return None;
    }

    path.file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| MonthId::parse(stem).ok())
}

#[cfg(test)]
mod tests {
    use super::{PARALLEL_LIST_THRESHOLD, parallel_worker_count, should_parallelise_list};

    #[test]
    fn ordinary_histories_stay_on_sequential_loader() {
        assert!(!should_parallelise_list(120));
        assert!(!should_parallelise_list(PARALLEL_LIST_THRESHOLD - 1));
    }

    #[test]
    fn large_histories_use_parallel_loader_when_workers_are_available() {
        assert_eq!(
            should_parallelise_list(PARALLEL_LIST_THRESHOLD),
            parallel_worker_count(PARALLEL_LIST_THRESHOLD) > 1
        );
    }
}
