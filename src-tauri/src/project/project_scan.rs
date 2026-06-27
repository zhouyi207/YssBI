use std::path::{Path, PathBuf};

use super::{PROJECT_METADATA_FILE, normalize_project_name};
use crate::project::{is_picker_task_cancelled, picker_task_cancelled_error};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProjectsResult {
    pub discovered: usize,
    pub newly_registered: usize,
    pub projects: Vec<super::ProjectRecord>,
}

#[derive(Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProjectScanProgressEvent {
    Scanning,
    Discovered { count: usize },
    Registering { current: usize, total: usize },
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
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Vec<PathBuf>, String> {
    if is_picker_task_cancelled(cancel) {
        return Err(picker_task_cancelled_error());
    }
    if !root.is_dir() {
        return Err("扫描路径必须是文件夹".into());
    }
    let mut found = Vec::new();
    walk_for_metadata(root, &mut found, cancel).map_err(|e| format!("扫描文件夹失败: {e}"))?;
    if is_picker_task_cancelled(cancel) {
        return Err(picker_task_cancelled_error());
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
    cancel: &std::sync::atomic::AtomicBool,
) -> std::io::Result<()> {
    if is_picker_task_cancelled(cancel) {
        return Ok(());
    }

    let metadata_path = dir.join(PROJECT_METADATA_FILE);
    if metadata_path.is_file() {
        found.push(metadata_path);
    }

    for entry in std::fs::read_dir(dir)? {
        if is_picker_task_cancelled(cancel) {
            return Ok(());
        }
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if should_skip_dir(&path) {
            continue;
        }
        walk_for_metadata(&path, found, cancel)?;
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

    #[test]
    fn discover_nested_metadata_files() {
        let root = std::env::temp_dir().join(format!("yssbi-scan-test-{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("alpha")).unwrap();
        fs::create_dir_all(root.join("nested/beta")).unwrap();
        fs::write(root.join("alpha/metadata.yssbi"), "{}").unwrap();
        fs::write(root.join("nested/beta/metadata.yssbi"), "{}").unwrap();

        let cancel = std::sync::atomic::AtomicBool::new(false);
        let found = discover_project_metadata_files(&root, &cancel).unwrap();
        assert_eq!(found.len(), 2);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn discover_stops_when_cancelled() {
        let root = std::env::temp_dir().join(format!("yssbi-scan-cancel-{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("alpha")).unwrap();
        fs::write(root.join("alpha/metadata.yssbi"), "{}").unwrap();

        let cancel = std::sync::atomic::AtomicBool::new(true);
        let err = discover_project_metadata_files(&root, &cancel).unwrap_err();
        assert_eq!(err, picker_task_cancelled_error());

        let _ = fs::remove_dir_all(&root);
    }
}
