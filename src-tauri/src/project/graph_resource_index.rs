use std::collections::HashMap;
use std::path::Path;

use super::graph_resource_path::{GraphResourcePath, normalize_graph_resource_path};
use super::project_error::ProjectError;
use super::{
    EVENT_EXTENSION, EVENTS_DIR, FUNCTION_EXTENSION, FUNCTIONS_DIR, GraphDocumentKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedGraphEntry {
    pub path: GraphResourcePath,
    pub kind: GraphDocumentKind,
}

#[derive(Debug, Clone)]
pub struct GraphResourceIndex {
    entries: Vec<ScannedGraphEntry>,
    by_path: HashMap<String, ScannedGraphEntry>,
}

impl GraphResourceIndex {
    pub fn entries(&self) -> &[ScannedGraphEntry] {
        &self.entries
    }

    pub fn get_by_path(&self, path: &str) -> Option<&ScannedGraphEntry> {
        self.by_path.get(&normalize_resource_path(path))
    }
}

/// 扫描 `events/` 与 `functions/` 目录，路径为唯一身份。
pub fn scan_graph_resource_index(root: &Path) -> Result<GraphResourceIndex, ProjectError> {
    let files = collect_graph_resource_files(root)?;
    let mut entries = Vec::with_capacity(files.len());
    for (path, kind) in files {
        let path = GraphResourcePath::new(path)?;
        entries.push(ScannedGraphEntry { path, kind });
    }
    entries.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str()));

    let by_path = entries
        .iter()
        .cloned()
        .map(|entry| (entry.path.as_str().to_string(), entry))
        .collect();

    Ok(GraphResourceIndex { entries, by_path })
}

pub fn normalize_resource_path(path: &str) -> String {
    normalize_graph_resource_path(path)
}

fn collect_graph_resource_files(
    root: &Path,
) -> Result<Vec<(String, GraphDocumentKind)>, ProjectError> {
    let mut files = Vec::new();
    collect_kind_files(
        root,
        EVENTS_DIR,
        EVENT_EXTENSION,
        GraphDocumentKind::Event,
        &mut files,
    )?;
    collect_kind_files(
        root,
        FUNCTIONS_DIR,
        FUNCTION_EXTENSION,
        GraphDocumentKind::Function,
        &mut files,
    )?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

fn collect_kind_files(
    root: &Path,
    dir: &str,
    extension: &str,
    kind: GraphDocumentKind,
    files: &mut Vec<(String, GraphDocumentKind)>,
) -> Result<(), ProjectError> {
    let graph_dir = root.join(dir);
    if !graph_dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&graph_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && is_graph_file(path.as_path(), extension) {
            files.push((relative_slash_path(root, path.as_path())?, kind));
        }
    }
    Ok(())
}

fn is_graph_file(path: &Path, extension: &str) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case(extension))
            .unwrap_or(false)
}

fn relative_slash_path(root: &Path, path: &Path) -> Result<String, ProjectError> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))
}

