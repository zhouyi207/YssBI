use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::PoisonError;

use crate::{DatasetStore, DatasetStoreError, catalog};

impl DatasetStore {
    /// Reclaim only generations absent from the catalog, queries, preparations, and handoffs.
    /// The durable queue makes an interrupted or failed physical removal retryable.
    pub fn collect_garbage(&self) -> Result<usize, DatasetStoreError> {
        let mut leases = self.leases.lock().unwrap_or_else(PoisonError::into_inner);
        let mut directories = leases.prepared_directories();
        for path in self.catalog(catalog::committed_files(&self.pool))? {
            let parent = path.parent().ok_or(DatasetStoreError::CorruptCatalog)?;
            directories.insert(self.root.join(parent));
        }
        let orphans = orphan_files(&self.root, &directories)?;
        let paths = self.catalog(catalog::collect_garbage(
            &self.pool,
            &leases.protected(),
            &orphans,
        ))?;
        drop(leases);
        let mut removed = 0;
        for relative in paths {
            let path = self.root.join(&relative);
            crate::paths::validate(&self.root, &path)?;
            match std::fs::remove_file(&path) {
                Ok(()) => removed += 1,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            if let Some(parent) = path.parent() {
                let _ = std::fs::remove_dir(parent);
            }
            self.catalog(catalog::finish_garbage(&self.pool, &relative))?;
        }
        Ok(removed)
    }
}

fn orphan_files(
    root: &Path,
    retained: &BTreeSet<PathBuf>,
) -> Result<Vec<PathBuf>, DatasetStoreError> {
    let datasets = root.join("datasets");
    crate::paths::validate(root, &datasets)?;
    if !datasets.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for dataset in std::fs::read_dir(&datasets)? {
        let dataset = dataset?;
        if !generation_directory(&dataset, root)? {
            continue;
        }
        for generation in std::fs::read_dir(dataset.path())? {
            let generation = generation?;
            if !generation_directory(&generation, root)? || retained.contains(&generation.path()) {
                continue;
            }
            for file in std::fs::read_dir(generation.path())? {
                let file = file?;
                let name = file.file_name();
                let name = name.to_string_lossy();
                if file.file_type()?.is_file()
                    && name
                        .strip_prefix("part-")
                        .and_then(|name| name.strip_suffix(".parquet"))
                        .is_some_and(|ordinal| {
                            ordinal.len() == 6 && ordinal.bytes().all(|byte| byte.is_ascii_digit())
                        })
                {
                    files.push(
                        file.path()
                            .strip_prefix(root)
                            .map_err(|_| DatasetStoreError::InvalidIdentity)?
                            .to_owned(),
                    );
                }
            }
            // An interrupted preparation can leave an empty reserved directory.
            let _ = std::fs::remove_dir(generation.path());
        }
    }
    Ok(files)
}

fn generation_directory(entry: &std::fs::DirEntry, root: &Path) -> Result<bool, DatasetStoreError> {
    if !entry.file_type()?.is_dir()
        || entry
            .file_name()
            .to_str()
            .is_none_or(|name| uuid::Uuid::parse_str(name).is_err())
    {
        return Ok(false);
    }
    let resolved = std::fs::canonicalize(entry.path())?;
    Ok(resolved.starts_with(root) && resolved == entry.path())
}
