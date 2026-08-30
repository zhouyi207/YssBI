use std::path::{Path, PathBuf};

use super::normalize_project_name;
use thiserror::Error;
use yss_project_layout::PROJECT_METADATA_FILE;
use yss_project_progress::ProjectTaskCancellation;
use yss_project_registry_contract::ProjectRecord;

#[derive(Debug, Error)]
pub enum ProjectDiscoveryError {
    #[error("project discovery was cancelled")]
    Cancelled,
    #[error("project discovery root must be a directory")]
    InvalidRoot,
    #[error("project discovery I/O failed")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProjectsResult {
    pub discovered: usize,
    pub newly_registered: usize,
    pub projects: Vec<ProjectRecord>,
}

const SKIP_DIR_NAMES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "$recycle.bin",
    "system volume information",
];

pub fn discover_project_metadata_files(
    root: &Path,
    cancellation: &ProjectTaskCancellation,
) -> Result<Vec<PathBuf>, ProjectDiscoveryError> {
    if cancellation.is_cancelled() {
        return Err(ProjectDiscoveryError::Cancelled);
    }
    if !root.is_dir() {
        return Err(ProjectDiscoveryError::InvalidRoot);
    }
    let mut found = Vec::new();
    walk_for_metadata(root, &mut found, cancellation)?;
    if cancellation.is_cancelled() {
        return Err(ProjectDiscoveryError::Cancelled);
    }
    found.sort();
    found.dedup();
    Ok(found)
}

pub fn project_name_from_metadata_path(metadata_path: &Path) -> String {
    metadata_path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .map(normalize_project_name)
        .unwrap_or_else(|| "未命名项目".into())
}

fn walk_for_metadata(
    dir: &Path,
    found: &mut Vec<PathBuf>,
    cancellation: &ProjectTaskCancellation,
) -> Result<(), ProjectDiscoveryError> {
    if cancellation.is_cancelled() {
        return Err(ProjectDiscoveryError::Cancelled);
    }

    let metadata_path = dir.join(PROJECT_METADATA_FILE);
    if metadata_path.is_file() {
        found.push(metadata_path);
    }

    for entry in std::fs::read_dir(dir)? {
        if cancellation.is_cancelled() {
            return Err(ProjectDiscoveryError::Cancelled);
        }
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if should_skip_dir(&path) {
            continue;
        }
        walk_for_metadata(&path, found, cancellation)?;
    }
    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            let lower = name.to_ascii_lowercase();
            SKIP_DIR_NAMES.iter().any(|skip| lower == *skip)
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use yss_project_progress::ProjectTaskCancellationRegistry;

    #[test]
    fn discover_nested_metadata_files() {
        let root = std::env::temp_dir().join(format!("yssbi-scan-test-{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("alpha")).unwrap();
        fs::create_dir_all(root.join("nested/beta")).unwrap();
        fs::write(root.join("alpha/metadata.yssbi"), "{}").unwrap();
        fs::write(root.join("nested/beta/metadata.yssbi"), "{}").unwrap();

        let registry = ProjectTaskCancellationRegistry::new();
        let cancellation = registry.begin();
        let found = discover_project_metadata_files(&root, &cancellation).unwrap();
        assert_eq!(found.len(), 2);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn discover_stops_when_cancelled() {
        let root = std::env::temp_dir().join(format!("yssbi-scan-cancel-{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("alpha")).unwrap();
        fs::write(root.join("alpha/metadata.yssbi"), "{}").unwrap();

        let registry = ProjectTaskCancellationRegistry::new();
        let cancellation = registry.begin();
        registry.cancel_active();
        let error = discover_project_metadata_files(&root, &cancellation).unwrap_err();
        assert!(matches!(error, ProjectDiscoveryError::Cancelled));

        let _ = fs::remove_dir_all(&root);
    }
}
