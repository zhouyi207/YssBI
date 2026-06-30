use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::graph::GraphId;
use crate::project::{
    EVENT_EXTENSION, EVENTS_DIR, FUNCTION_EXTENSION, FUNCTIONS_DIR, GraphDocumentKind,
    ProjectError, ProjectManifest,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphResourceManifestEntry {
    pub id: GraphId,
    pub path: String,
    pub kind: GraphDocumentKind,
}

#[derive(Debug, Clone)]
pub struct GraphResourceIndex {
    entries: Vec<GraphResourceManifestEntry>,
    by_id: HashMap<GraphId, GraphResourceManifestEntry>,
    by_path: HashMap<String, GraphResourceManifestEntry>,
}

impl GraphResourceIndex {
    pub fn entries(&self) -> &[GraphResourceManifestEntry] {
        &self.entries
    }

    pub fn get_by_id(&self, id: &GraphId) -> Option<&GraphResourceManifestEntry> {
        self.by_id.get(id)
    }

    pub fn get_by_path(&self, path: &str) -> Option<&GraphResourceManifestEntry> {
        self.by_path.get(&normalize_resource_path(path))
    }
}

pub fn reconcile_graph_resources(
    root: &Path,
    manifest: &mut ProjectManifest,
) -> Result<(GraphResourceIndex, bool), ProjectError> {
    let files = collect_graph_resource_files(root)?;
    let live_paths: HashSet<String> = files.iter().map(|(path, _)| path.clone()).collect();
    let mut existing: HashMap<String, GraphResourceManifestEntry> = manifest
        .graphs
        .iter()
        .filter(|entry| live_paths.contains(&normalize_resource_path(&entry.path)))
        .map(|entry| (normalize_resource_path(&entry.path), entry.clone()))
        .collect();

    let mut entries = Vec::with_capacity(files.len());
    for (path, kind) in files {
        let path = normalize_resource_path(&path);
        let entry = existing
            .remove(&path)
            .filter(|entry| entry.kind == kind)
            .unwrap_or(GraphResourceManifestEntry {
                id: GraphId::new(),
                path: path.clone(),
                kind,
            });
        entries.push(GraphResourceManifestEntry {
            path,
            kind,
            ..entry
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    let changed = manifest.graphs != entries;
    if changed {
        manifest.graphs = entries.clone();
    }

    let by_id = entries
        .iter()
        .cloned()
        .map(|entry| (entry.id, entry))
        .collect();
    let by_path = entries
        .iter()
        .cloned()
        .map(|entry| (entry.path.clone(), entry))
        .collect();

    Ok((
        GraphResourceIndex {
            entries,
            by_id,
            by_path,
        },
        changed,
    ))
}

pub fn normalize_resource_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
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
