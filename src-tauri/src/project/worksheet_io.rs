use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ProjectError, SCHEMA_VERSION};

pub const WORKSHEETS_DIR: &str = "worksheets";
pub const WORKSHEET_EXTENSION: &str = "yssbi-worksheet";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorksheetEncodings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorksheetDocument {
    pub schema_version: u32,
    #[serde(default)]
    pub revision: crate::node_system::document::ResourceRevision,
    pub id: String,
    pub name: String,
    pub database_id: String,
    pub chart_type: String,
    pub encodings: WorksheetEncodings,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorksheetIndexEntry {
    pub id: String,
    pub name: String,
    pub database_id: String,
    pub chart_type: String,
}

impl WorksheetDocument {
    pub fn new(name: impl Into<String>, database_id: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            revision: crate::node_system::document::ResourceRevision::INITIAL,
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            database_id: database_id.into(),
            chart_type: "histogram".to_string(),
            encodings: WorksheetEncodings { x: None, y: None },
        }
    }
}

pub fn ensure_worksheets_dir(root: &Path) -> Result<(), ProjectError> {
    std::fs::create_dir_all(root.join(WORKSHEETS_DIR))?;
    Ok(())
}

/// Hoists nested worksheet files under `worksheets/` to the directory root.
pub fn flatten_worksheet_layout(root: &Path) -> Result<bool, ProjectError> {
    let dir = root.join(WORKSHEETS_DIR);
    if !dir.is_dir() {
        return Ok(false);
    }

    let mut nested_paths = Vec::new();
    collect_nested_worksheet_files(&dir, &mut nested_paths)?;
    if nested_paths.is_empty() {
        return Ok(false);
    }

    let mut changed = false;
    for nested_path in nested_paths {
        let document = read_worksheet_document_path(nested_path.as_path())?;
        let file_name = unique_worksheet_file_name(dir.as_path(), &document.name, None);
        let target_path = dir.join(&file_name);
        if nested_path == target_path {
            continue;
        }
        std::fs::rename(&nested_path, &target_path)?;
        changed = true;
    }

    if changed {
        remove_empty_worksheet_subdirs(&dir)?;
    }

    Ok(changed)
}

pub fn read_worksheet_index_entries(
    root: &Path,
) -> Result<Vec<ProjectWorksheetIndexEntry>, ProjectError> {
    let dir = root.join(WORKSHEETS_DIR);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for path in list_worksheet_files(root)? {
        let document = read_worksheet_document_path(path.as_path())?;
        entries.push(ProjectWorksheetIndexEntry {
            id: document.id,
            name: document.name,
            database_id: document.database_id,
            chart_type: document.chart_type,
        });
    }
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(entries)
}

pub fn load_worksheet_from_file(
    root: &Path,
    worksheet_id: &str,
) -> Result<WorksheetDocument, ProjectError> {
    load_worksheet_from_root_readonly(root, worksheet_id)
}

pub(crate) fn load_worksheet_from_root_readonly(
    root: &Path,
    worksheet_id: &str,
) -> Result<WorksheetDocument, ProjectError> {
    for path in list_worksheet_files(root)? {
        let document = read_worksheet_document_path(path.as_path())?;
        if document.id == worksheet_id {
            return Ok(document);
        }
    }
    Err(ProjectError::InvalidProjectFormat(format!(
        "worksheet '{}' not found",
        worksheet_id
    )))
}

pub fn serialize_worksheet(
    document: &WorksheetDocument,
) -> Result<(PathBuf, Vec<u8>), ProjectError> {
    let contents = serde_json::to_vec_pretty(document).map_err(ProjectError::Serialize)?;
    Ok((worksheet_relative_path(document), contents))
}

pub fn existing_worksheet_names(
    root: &Path,
    excluded_id: Option<&str>,
) -> Result<Vec<String>, ProjectError> {
    flatten_worksheet_layout(root)?;
    let mut names = HashSet::new();
    for entry in read_worksheet_index_entries(root)? {
        if excluded_id.map(|id| id == entry.id).unwrap_or(false) {
            continue;
        }
        names.insert(entry.name);
    }
    let mut out: Vec<String> = names.into_iter().collect();
    out.sort();
    Ok(out)
}

pub(crate) fn load_worksheets_from_root(
    root: &Path,
) -> Result<std::collections::HashMap<String, WorksheetDocument>, ProjectError> {
    let worksheets_dir = root.join(WORKSHEETS_DIR);
    if !worksheets_dir.exists() {
        return Ok(std::collections::HashMap::new());
    }
    let mut paths = Vec::new();
    collect_all_worksheet_files(&worksheets_dir, &mut paths)?;
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            read_worksheet_document_path(&path).map(|document| (document.id.clone(), document))
        })
        .collect()
}

fn list_worksheet_files(root: &Path) -> Result<Vec<PathBuf>, ProjectError> {
    let dir = root.join(WORKSHEETS_DIR);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && is_worksheet_file(path.as_path()) {
            paths.push(path);
        }
    }
    paths.sort_by_key(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });
    Ok(paths)
}

fn collect_nested_worksheet_files(
    worksheets_dir: &Path,
    nested_paths: &mut Vec<PathBuf>,
) -> Result<(), ProjectError> {
    for entry in std::fs::read_dir(worksheets_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_all_worksheet_files(path.as_path(), nested_paths)?;
        }
    }
    Ok(())
}

fn collect_all_worksheet_files(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), ProjectError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_all_worksheet_files(path.as_path(), paths)?;
        } else if is_worksheet_file(path.as_path()) {
            paths.push(path);
        }
    }
    Ok(())
}

fn remove_empty_worksheet_subdirs(dir: &Path) -> Result<(), ProjectError> {
    if !dir.is_dir() {
        return Ok(());
    }
    let entries: Vec<_> = std::fs::read_dir(dir)?.collect();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            remove_empty_worksheet_subdirs(&path)?;
            if std::fs::read_dir(&path)?.next().is_none() {
                std::fs::remove_dir(path)?;
            }
        }
    }
    Ok(())
}

fn is_worksheet_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case(WORKSHEET_EXTENSION))
            .unwrap_or(false)
}

fn read_worksheet_document_path(path: &Path) -> Result<WorksheetDocument, ProjectError> {
    read_json(path)
}

pub fn worksheet_absolute_path(
    root: &Path,
    worksheet_id: &str,
) -> Result<Option<PathBuf>, ProjectError> {
    for path in list_worksheet_files(root)? {
        let document = read_worksheet_document_path(path.as_path())?;
        if document.id == worksheet_id {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn unique_worksheet_file_name(
    dir: &Path,
    worksheet_name: &str,
    existing_path: Option<&Path>,
) -> String {
    let stem = sanitize_file_stem(worksheet_name);
    for index in 0.. {
        let candidate = if index == 0 {
            format!("{stem}.{WORKSHEET_EXTENSION}")
        } else {
            format!("{stem} {index}.{WORKSHEET_EXTENSION}")
        };
        let candidate_path = dir.join(&candidate);
        if existing_path
            .map(|path| path == candidate_path.as_path())
            .unwrap_or(false)
            || !candidate_path.exists()
        {
            return candidate;
        }
    }
    unreachable!("unique worksheet file name loop should always return")
}

pub(crate) fn worksheet_relative_path(document: &WorksheetDocument) -> PathBuf {
    PathBuf::from(WORKSHEETS_DIR).join(format!("{}.{}", document.id, WORKSHEET_EXTENSION))
}

fn sanitize_file_stem(name: &str) -> String {
    let sanitized: String = name
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .collect();
    let sanitized = sanitized.trim_matches([' ', '.']).trim();
    if sanitized.is_empty() {
        "Untitled Worksheet".to_string()
    } else {
        sanitized.to_string()
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ProjectError> {
    let content = std::fs::read_to_string(path).map_err(ProjectError::Io)?;
    serde_json::from_str(&content).map_err(ProjectError::Deserialize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("yssbi-worksheet-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn flatten_worksheet_layout_hoists_nested_files() {
        let root = temp_project_dir();
        let document = WorksheetDocument::new("Nested Sheet", "db-1");
        crate::project::fixtures::write_worksheet(root.as_path(), &document).unwrap();

        let nested_dir = root.join(WORKSHEETS_DIR).join("Sub");
        std::fs::create_dir_all(&nested_dir).unwrap();
        let flat_file = root
            .join(WORKSHEETS_DIR)
            .join(format!("Nested Sheet.{WORKSHEET_EXTENSION}"));
        let nested_file = nested_dir.join(format!("Nested Sheet.{WORKSHEET_EXTENSION}"));
        std::fs::rename(&flat_file, &nested_file).unwrap();

        flatten_worksheet_layout(root.as_path()).unwrap();

        assert!(
            root.join(WORKSHEETS_DIR)
                .join(format!("Nested Sheet.{WORKSHEET_EXTENSION}"))
                .is_file()
        );
        assert!(!nested_file.exists());
        assert!(!nested_dir.exists());

        let entries = read_worksheet_index_entries(root.as_path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Nested Sheet");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn save_worksheet_writes_to_worksheets_root() {
        let root = temp_project_dir();
        let document = WorksheetDocument::new("Root Sheet", "db-1");
        crate::project::fixtures::write_worksheet(root.as_path(), &document).unwrap();

        let path = root
            .join(WORKSHEETS_DIR)
            .join(format!("Root Sheet.{WORKSHEET_EXTENSION}"));
        assert!(path.is_file());
        assert_eq!(
            path.parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str()),
            Some(WORKSHEETS_DIR)
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
