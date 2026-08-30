use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    ProjectError, SCHEMA_VERSION, WorksheetResourcePath,
    project_io::deserialize_current_schema_version,
};

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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorksheetDocument {
    #[serde(deserialize_with = "deserialize_current_schema_version")]
    pub schema_version: u32,
    pub revision: yss_project_identity::ResourceRevision,
    pub database_id: String,
    pub chart_type: String,
    pub encodings: WorksheetEncodings,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorksheetIndexEntry {
    pub worksheet_path: WorksheetResourcePath,
    pub name: String,
    pub database_id: String,
    pub chart_type: String,
    pub revision: yss_project_identity::ResourceRevision,
}

impl WorksheetDocument {
    pub fn new(database_id: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            revision: yss_project_identity::ResourceRevision::INITIAL,
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
        .map(|(worksheet_path, document)| ProjectWorksheetIndexEntry {
            name: worksheet_path.display_name().as_str().to_string(),
            worksheet_path,
            database_id: document.database_id,
            chart_type: document.chart_type,
            revision: document.revision,
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        a.worksheet_path
            .display_name()
            .portable_key()
            .cmp(&b.worksheet_path.display_name().portable_key())
    });
    Ok(entries)
}

pub fn load_worksheet_from_file(
    root: &Path,
    worksheet_path: &WorksheetResourcePath,
) -> Result<WorksheetDocument, ProjectError> {
    load_worksheet_from_root_readonly(root, worksheet_path)
}

pub(crate) fn load_worksheet_from_root_readonly(
    root: &Path,
    worksheet_path: &WorksheetResourcePath,
) -> Result<WorksheetDocument, ProjectError> {
    scan_worksheet_documents(root)?
        .into_iter()
        .find_map(|(path, document)| (path == *worksheet_path).then_some(document))
        .ok_or_else(|| {
            ProjectError::InvalidProjectFormat(format!(
                "worksheet '{}' not found",
                worksheet_path.as_str()
            ))
        })
}

pub fn serialize_worksheet(
    path: &WorksheetResourcePath,
    document: &WorksheetDocument,
) -> Result<(PathBuf, Vec<u8>), ProjectError> {
    let contents = serde_json::to_vec_pretty(document).map_err(ProjectError::Serialize)?;
    Ok((path.relative_path().to_path_buf(), contents))
}

pub(crate) fn load_worksheets_from_root(
    root: &Path,
) -> Result<HashMap<WorksheetResourcePath, WorksheetDocument>, ProjectError> {
    Ok(scan_worksheet_documents(root)?.into_iter().collect())
}

fn scan_worksheet_documents(
    root: &Path,
) -> Result<Vec<(WorksheetResourcePath, WorksheetDocument)>, ProjectError> {
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
    let mut portable_paths = HashSet::new();
    for (path, _) in &documents {
        if !portable_paths.insert(path.display_name().portable_key()) {
            return Err(ProjectError::InvalidProjectFormat(format!(
                "portable worksheet path collision at '{}'",
                path.as_str()
            )));
        }
    }
    Ok(documents)
}

fn walk_worksheet_directory(
    worksheets_dir: &Path,
    directory: &Path,
    documents: &mut Vec<(WorksheetResourcePath, WorksheetDocument)>,
) -> Result<(), ProjectError> {
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        reject_redirect(&path, &metadata)?;
        if metadata.is_dir() {
            walk_worksheet_directory(worksheets_dir, &path, documents)?;
            continue;
        }
        if !metadata.is_file() {
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
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid_layout(&path, "worksheet path is not valid Unicode"))?;
        let relative_path = format!("{WORKSHEETS_DIR}/{file_name}");
        let relative_path = relative_path.as_str();
        let worksheet_path = WorksheetResourcePath::parse(relative_path).map_err(|error| {
            invalid_layout(&path, &format!("invalid worksheet resource path: {error}"))
        })?;
        if uuid::Uuid::parse_str(worksheet_path.display_name().as_str()).is_ok() {
            return Err(invalid_layout(
                &path,
                "invalid worksheet resource path: UUID filenames are forbidden",
            ));
        }
        if worksheet_path.relative_path() != relative_path {
            return Err(invalid_layout(
                &path,
                "invalid worksheet resource path: path is not canonical",
            ));
        }
        let document = read_worksheet_document_path(&path)?;
        documents.push((worksheet_path, document));
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

fn read_worksheet_document_path(path: &Path) -> Result<WorksheetDocument, ProjectError> {
    let content = std::fs::read_to_string(path).map_err(ProjectError::Io)?;
    serde_json::from_str(&content).map_err(ProjectError::Deserialize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{ResourceName, WorksheetResourcePath};

    fn temp_project_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("yssbi-worksheet-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_document_at(path: &Path, document: &WorksheetDocument) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec_pretty(document).unwrap()).unwrap();
    }

    fn worksheet_path(name: &str) -> WorksheetResourcePath {
        WorksheetResourcePath::from_name(&ResourceName::parse(name).unwrap())
    }

    fn document(revision: u64) -> WorksheetDocument {
        serde_json::from_value(serde_json::json!({
            "schemaVersion": SCHEMA_VERSION,
            "revision": revision,
            "databaseId": "db-1",
            "chartType": "histogram",
            "encodings": { "x": null, "y": null }
        }))
        .unwrap()
    }

    fn assert_all_read_entries_reject(root: &Path, worksheet_path: &WorksheetResourcePath) {
        assert!(load_worksheets_from_root(root).is_err());
        assert!(read_worksheet_index_entries(root).is_err());
        assert!(load_worksheet_from_file(root, worksheet_path).is_err());
    }

    #[test]
    fn canonical_name_path_is_shared_by_activation_index_and_direct_load() {
        let root = temp_project_dir();
        let path = worksheet_path("Root Sheet");
        let document = document(3);
        crate::project::fixtures::write_worksheet(root.as_path(), &path, &document).unwrap();
        let canonical = root.join(path.relative_path());

        assert!(canonical.is_file());
        assert_eq!(
            load_worksheets_from_root(root.as_path())
                .unwrap()
                .get(&path),
            Some(&document)
        );
        let entries = read_worksheet_index_entries(root.as_path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].worksheet_path, path);
        assert_eq!(entries[0].name, "Root Sheet");
        assert_eq!(entries[0].revision.get(), 3);
        assert_eq!(
            load_worksheet_from_file(root.as_path(), &path).unwrap(),
            document
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn worksheet_document_rejects_embedded_identity_fields() {
        for field in ["id", "name"] {
            let mut value = serde_json::to_value(document(0)).unwrap();
            value.as_object_mut().unwrap().insert(
                field.into(),
                serde_json::Value::String("embedded-identity".into()),
            );

            assert!(serde_json::from_value::<WorksheetDocument>(value).is_err());
        }
    }

    #[test]
    fn worksheet_document_requires_revision() {
        let mut value = serde_json::to_value(document(4)).unwrap();
        value.as_object_mut().unwrap().remove("revision");

        assert!(serde_json::from_value::<WorksheetDocument>(value).is_err());
    }

    #[test]
    fn worksheet_load_rejects_unsupported_schema_version() {
        let root = temp_project_dir();
        let path = worksheet_path("Unsupported");
        let mut value = serde_json::to_value(document(0)).unwrap();
        value["schemaVersion"] = serde_json::json!(SCHEMA_VERSION + 1);
        let file = root.join(path.relative_path());
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let error = load_worksheet_from_file(&root, &path).unwrap_err();

        assert!(matches!(
            error,
            ProjectError::Deserialize(source)
                if source.to_string().contains("unsupported schema version 4")
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn worksheet_activation_rejects_uuid_filename() {
        let root = temp_project_dir();
        let uuid = uuid::Uuid::new_v4().to_string();
        let file = root
            .join(WORKSHEETS_DIR)
            .join(format!("{uuid}.{WORKSHEET_EXTENSION}"));
        write_document_at(&file, &document(0));

        let error = load_worksheets_from_root(&root).unwrap_err().to_string();
        assert!(error.contains("invalid worksheet resource path"), "{error}");
        assert!(file.is_file(), "activation must not migrate invalid files");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn worksheet_activation_rejects_noncanonical_and_casefold_duplicate_paths() {
        let noncanonical_root = temp_project_dir();
        let noncanonical = noncanonical_root
            .join(WORKSHEETS_DIR)
            .join(format!("Report.{}", WORKSHEET_EXTENSION.to_uppercase()));
        write_document_at(&noncanonical, &document(0));
        let error = load_worksheets_from_root(&noncanonical_root)
            .unwrap_err()
            .to_string();
        assert!(error.contains("invalid worksheet resource path"), "{error}");

        let wrong_extension_root = temp_project_dir();
        let wrong_extension = wrong_extension_root
            .join(WORKSHEETS_DIR)
            .join("Report.json");
        write_document_at(&wrong_extension, &document(0));
        let error = load_worksheets_from_root(&wrong_extension_root)
            .unwrap_err()
            .to_string();
        assert!(error.contains("invalid worksheet resource path"), "{error}");

        let duplicate_root = temp_project_dir();
        for name in ["Straße", "STRASSE"] {
            write_document_at(
                &duplicate_root
                    .join(WORKSHEETS_DIR)
                    .join(format!("{name}.{WORKSHEET_EXTENSION}")),
                &document(0),
            );
        }
        let error = load_worksheets_from_root(&duplicate_root)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("portable worksheet path collision"),
            "{error}"
        );

        let _ = std::fs::remove_dir_all(noncanonical_root);
        let _ = std::fs::remove_dir_all(wrong_extension_root);
        let _ = std::fs::remove_dir_all(duplicate_root);
    }

    #[test]
    fn worksheet_index_derives_name_from_path_and_includes_revision() {
        let root = temp_project_dir();
        let path = worksheet_path("销售分析 2");
        crate::project::fixtures::write_worksheet(&root, &path, &document(9)).unwrap();

        let entries = read_worksheet_index_entries(&root).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].worksheet_path, path);
        assert_eq!(entries[0].name, "销售分析 2");
        assert_eq!(entries[0].database_id, "db-1");
        assert_eq!(entries[0].chart_type, "histogram");
        assert_eq!(entries[0].revision.get(), 9);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nested_canonical_worksheet_is_rejected_without_flattening() {
        let root = temp_project_dir();
        let path = worksheet_path("Nested");
        let nested = root
            .join(WORKSHEETS_DIR)
            .join("nested")
            .join(format!("Nested.{WORKSHEET_EXTENSION}"));
        write_document_at(&nested, &document(0));

        assert_all_read_entries_reject(root.as_path(), &path);
        assert!(nested.is_file(), "read paths must not flatten nested files");
        assert!(!root.join(path.relative_path()).exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn external_file_symlink_is_rejected_by_every_read_entry_before_reading() {
        use std::os::unix::fs::symlink;

        let root = temp_project_dir();
        let external_root = temp_project_dir();
        let path = worksheet_path("External");
        let document = document(0);
        let external = external_root.join(format!("External.{WORKSHEET_EXTENSION}"));
        write_document_at(&external, &document);
        let link = root.join(path.relative_path());
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(&external, &link).unwrap();

        assert_all_read_entries_reject(root.as_path(), &path);
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
        let path = worksheet_path("External");
        let document = document(0);
        crate::project::fixtures::write_worksheet(external_root.as_path(), &path, &document)
            .unwrap();
        let worksheets = root.join(WORKSHEETS_DIR);
        std::fs::create_dir_all(&worksheets).unwrap();
        if !create_test_junction(
            &worksheets.join("external"),
            &external_root.join(WORKSHEETS_DIR),
        ) {
            eprintln!("skipping junction assertion: Windows junction creation is unavailable");
            return;
        }

        assert_all_read_entries_reject(root.as_path(), &path);

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

        assert_all_read_entries_reject(root.as_path(), &worksheet_path("Missing"));

        let _ = std::fs::remove_dir_all(root);
    }
}
