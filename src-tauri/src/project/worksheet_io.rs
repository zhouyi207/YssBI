use std::collections::{HashMap, HashSet};
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

#[cfg(test)]
pub fn ensure_worksheets_dir(root: &Path) -> Result<(), ProjectError> {
    std::fs::create_dir_all(root.join(WORKSHEETS_DIR))?;
    Ok(())
}

pub fn read_worksheet_index_entries(
    root: &Path,
) -> Result<Vec<ProjectWorksheetIndexEntry>, ProjectError> {
    let mut entries = scan_worksheet_documents(root)?
        .into_iter()
        .map(|(_, document)| ProjectWorksheetIndexEntry {
            id: document.id,
            name: document.name,
            database_id: document.database_id,
            chart_type: document.chart_type,
        })
        .collect::<Vec<_>>();
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
    scan_worksheet_documents(root)?
        .into_iter()
        .find_map(|(_, document)| (document.id == worksheet_id).then_some(document))
        .ok_or_else(|| {
            ProjectError::InvalidProjectFormat(format!("worksheet '{worksheet_id}' not found"))
        })
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
    let mut names = HashSet::new();
    for (_, document) in scan_worksheet_documents(root)? {
        if excluded_id.is_some_and(|id| id == document.id) {
            continue;
        }
        names.insert(document.name);
    }
    let mut out = names.into_iter().collect::<Vec<_>>();
    out.sort();
    Ok(out)
}

pub(crate) fn load_worksheets_from_root(
    root: &Path,
) -> Result<HashMap<String, WorksheetDocument>, ProjectError> {
    Ok(scan_worksheet_documents(root)?
        .into_iter()
        .map(|(_, document)| (document.id.clone(), document))
        .collect())
}

pub fn worksheet_absolute_path(
    root: &Path,
    worksheet_id: &str,
) -> Result<Option<PathBuf>, ProjectError> {
    Ok(scan_worksheet_documents(root)?
        .into_iter()
        .find_map(|(path, document)| (document.id == worksheet_id).then_some(path)))
}

fn scan_worksheet_documents(
    root: &Path,
) -> Result<Vec<(PathBuf, WorksheetDocument)>, ProjectError> {
    let worksheets_dir = root.join(WORKSHEETS_DIR);
    let metadata = match std::fs::symlink_metadata(&worksheets_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(ProjectError::Io(error)),
    };
    reject_redirect(&worksheets_dir, &metadata)?;
    if !metadata.is_dir() {
        return Err(invalid_layout(
            &worksheets_dir,
            "worksheet root is not a directory",
        ));
    }

    let mut documents = Vec::new();
    walk_worksheet_directory(&worksheets_dir, &worksheets_dir, &mut documents)?;
    documents.sort_by(|(left, _), (right, _)| left.cmp(right));
    Ok(documents)
}

fn walk_worksheet_directory(
    worksheets_dir: &Path,
    directory: &Path,
    documents: &mut Vec<(PathBuf, WorksheetDocument)>,
) -> Result<(), ProjectError> {
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        reject_redirect(&path, &metadata)?;
        if metadata.is_dir() {
            walk_worksheet_directory(worksheets_dir, &path, documents)?;
            continue;
        }
        if !metadata.is_file() || !has_worksheet_extension(&path) {
            continue;
        }

        let relative = path
            .strip_prefix(worksheets_dir)
            .map_err(|_| invalid_layout(&path, "worksheet path escaped its root"))?;
        if relative
            .parent()
            .is_some_and(|parent| !parent.as_os_str().is_empty())
        {
            return Err(invalid_layout(
                &path,
                "nested worksheet files are not supported",
            ));
        }
        let document = read_worksheet_document_path(&path)?;
        let expected = PathBuf::from(format!("{}.{}", document.id, WORKSHEET_EXTENSION));
        if relative != expected {
            return Err(invalid_layout(
                &path,
                "worksheet filename must be its stable document ID",
            ));
        }
        documents.push((path, document));
    }
    Ok(())
}

fn reject_redirect(path: &Path, metadata: &std::fs::Metadata) -> Result<(), ProjectError> {
    let mut is_redirect = metadata.file_type().is_symlink();
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        is_redirect |= metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    if is_redirect {
        return Err(invalid_layout(
            path,
            "worksheet redirects/reparse points are forbidden",
        ));
    }
    Ok(())
}

fn invalid_layout(path: &Path, reason: &str) -> ProjectError {
    ProjectError::InvalidProjectFormat(format!("{reason}: '{}'", path.display()))
}

fn has_worksheet_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(WORKSHEET_EXTENSION))
}

fn read_worksheet_document_path(path: &Path) -> Result<WorksheetDocument, ProjectError> {
    let content = std::fs::read_to_string(path).map_err(ProjectError::Io)?;
    serde_json::from_str(&content).map_err(ProjectError::Deserialize)
}

pub(crate) fn worksheet_relative_path(document: &WorksheetDocument) -> PathBuf {
    PathBuf::from(WORKSHEETS_DIR).join(format!("{}.{}", document.id, WORKSHEET_EXTENSION))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("yssbi-worksheet-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_document_at(path: &Path, document: &WorksheetDocument) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec_pretty(document).unwrap()).unwrap();
    }

    fn assert_all_read_entries_reject(root: &Path, worksheet_id: &str) {
        assert!(load_worksheets_from_root(root).is_err());
        assert!(read_worksheet_index_entries(root).is_err());
        assert!(load_worksheet_from_file(root, worksheet_id).is_err());
    }

    #[test]
    fn canonical_id_path_is_shared_by_activation_index_and_direct_load() {
        let root = temp_project_dir();
        let document = WorksheetDocument::new("Root Sheet", "db-1");
        crate::project::fixtures::write_worksheet(root.as_path(), &document).unwrap();
        let canonical = root.join(worksheet_relative_path(&document));

        assert!(canonical.is_file());
        let expected_file_name = format!("{}.{}", document.id, WORKSHEET_EXTENSION);
        assert_eq!(
            canonical.file_name().and_then(|name| name.to_str()),
            Some(expected_file_name.as_str())
        );
        assert_eq!(
            load_worksheets_from_root(root.as_path())
                .unwrap()
                .get(&document.id),
            Some(&document)
        );
        let entries = read_worksheet_index_entries(root.as_path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, document.id);
        assert_eq!(
            load_worksheet_from_file(root.as_path(), &document.id).unwrap(),
            document
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_name_based_worksheet_is_rejected_by_every_read_entry() {
        let root = temp_project_dir();
        let document = WorksheetDocument::new("Legacy Name", "db-1");
        let legacy = root
            .join(WORKSHEETS_DIR)
            .join(format!("Legacy Name.{WORKSHEET_EXTENSION}"));
        write_document_at(&legacy, &document);

        assert_all_read_entries_reject(root.as_path(), &document.id);
        assert!(legacy.is_file(), "read paths must not migrate legacy files");
        assert!(!root.join(worksheet_relative_path(&document)).exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nested_canonical_worksheet_is_rejected_without_flattening() {
        let root = temp_project_dir();
        let document = WorksheetDocument::new("Nested", "db-1");
        let nested = root
            .join(WORKSHEETS_DIR)
            .join("nested")
            .join(format!("{}.{}", document.id, WORKSHEET_EXTENSION));
        write_document_at(&nested, &document);

        assert_all_read_entries_reject(root.as_path(), &document.id);
        assert!(nested.is_file(), "read paths must not flatten nested files");
        assert!(!root.join(worksheet_relative_path(&document)).exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn external_file_symlink_is_rejected_by_every_read_entry_before_reading() {
        use std::os::unix::fs::symlink;

        let root = temp_project_dir();
        let external_root = temp_project_dir();
        let document = WorksheetDocument::new("External", "db-1");
        let external = external_root.join(format!("{}.{}", document.id, WORKSHEET_EXTENSION));
        write_document_at(&external, &document);
        let link = root.join(worksheet_relative_path(&document));
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(&external, &link).unwrap();

        assert_all_read_entries_reject(root.as_path(), &document.id);
        assert!(external.is_file());

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(external_root);
    }

    #[cfg(windows)]
    fn create_test_junction(link: &Path, target: &Path) -> bool {
        std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link.to_string_lossy().as_ref(),
                target.to_string_lossy().as_ref(),
            ])
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(windows)]
    #[test]
    fn external_directory_junction_is_rejected_by_every_read_entry_before_reading() {
        let root = temp_project_dir();
        let external_root = temp_project_dir();
        let document = WorksheetDocument::new("External", "db-1");
        crate::project::fixtures::write_worksheet(external_root.as_path(), &document).unwrap();
        let worksheets = root.join(WORKSHEETS_DIR);
        std::fs::create_dir_all(&worksheets).unwrap();
        if !create_test_junction(
            &worksheets.join("external"),
            &external_root.join(WORKSHEETS_DIR),
        ) {
            eprintln!("skipping junction assertion: Windows junction creation is unavailable");
            return;
        }

        assert_all_read_entries_reject(root.as_path(), &document.id);

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(external_root);
    }

    #[cfg(windows)]
    #[test]
    fn directory_junction_loop_is_rejected_by_every_read_entry_without_recursing() {
        let root = temp_project_dir();
        let worksheets = root.join(WORKSHEETS_DIR);
        std::fs::create_dir_all(&worksheets).unwrap();
        if !create_test_junction(&worksheets.join("loop"), &worksheets) {
            eprintln!("skipping junction assertion: Windows junction creation is unavailable");
            return;
        }

        assert_all_read_entries_reject(root.as_path(), "missing");

        let _ = std::fs::remove_dir_all(root);
    }
}
