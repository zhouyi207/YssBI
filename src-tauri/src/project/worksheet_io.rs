use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ProjectError, SCHEMA_VERSION};

pub const WORKSHEETS_DIR: &str = "worksheets";
pub const WORKSHEET_EXTENSION: &str = "yssbi-worksheet";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorksheetEncodings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorksheetDocument {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub database_id: String,
    pub chart_type: String,
    pub encodings: WorksheetEncodings,
    #[serde(default)]
    pub folder_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorksheetIndexEntry {
    pub id: String,
    pub name: String,
    pub database_id: String,
    pub chart_type: String,
    pub folder_path: String,
}

impl WorksheetDocument {
    pub fn new(name: impl Into<String>, database_id: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            database_id: database_id.into(),
            chart_type: "histogram".to_string(),
            encodings: WorksheetEncodings { x: None, y: None },
            folder_path: String::new(),
        }
    }
}

pub fn ensure_worksheets_dir(root: &Path) -> Result<(), ProjectError> {
    std::fs::create_dir_all(root.join(WORKSHEETS_DIR))?;
    Ok(())
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
            folder_path: document.folder_path,
        });
    }
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(entries)
}

pub fn load_worksheet_from_file(
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

pub fn save_worksheet_to_file(
    root: &Path,
    document: &WorksheetDocument,
) -> Result<(), ProjectError> {
    ensure_worksheets_dir(root)?;
    let relative_path = worksheet_relative_path_for_save(root, &document.name, &document.id)?;
    write_json(root.join(&relative_path).as_path(), document)
}

pub fn delete_worksheet_from_file(root: &Path, worksheet_id: &str) -> Result<(), ProjectError> {
    if let Some(path) = worksheet_absolute_path(root, worksheet_id)? {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn existing_worksheet_names(
    root: &Path,
    excluded_id: Option<&str>,
) -> Result<Vec<String>, ProjectError> {
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

fn list_worksheet_files(root: &Path) -> Result<Vec<PathBuf>, ProjectError> {
    let dir = root.join(WORKSHEETS_DIR);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    collect_worksheet_files(dir.as_path(), &mut paths)?;
    Ok(paths)
}

fn collect_worksheet_files(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), ProjectError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_worksheet_files(path.as_path(), paths)?;
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case(WORKSHEET_EXTENSION))
            .unwrap_or(false)
        {
            paths.push(path);
        }
    }
    Ok(())
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

fn worksheet_relative_path_for_save(
    root: &Path,
    worksheet_name: &str,
    worksheet_id: &str,
) -> Result<String, ProjectError> {
    let existing_path = worksheet_absolute_path(root, worksheet_id)?;
    let target_dir = existing_path
        .as_ref()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| root.join(WORKSHEETS_DIR));
    let file_name = unique_worksheet_file_name(
        target_dir.as_path(),
        worksheet_name,
        existing_path.as_deref(),
    );
    let next_path = target_dir.join(&file_name);
    if let Some(existing_path) = existing_path {
        if existing_path != next_path && existing_path.exists() {
            std::fs::remove_file(existing_path)?;
        }
    }
    next_path
        .strip_prefix(root)
        .map(path_to_slash_string)
        .map_err(|e| ProjectError::InvalidProjectFormat(e.to_string()))
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

fn path_to_slash_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ProjectError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value).map_err(ProjectError::Serialize)?;
    std::fs::write(path, json)?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ProjectError> {
    let content = std::fs::read_to_string(path).map_err(ProjectError::Io)?;
    serde_json::from_str(&content).map_err(ProjectError::Deserialize)
}
