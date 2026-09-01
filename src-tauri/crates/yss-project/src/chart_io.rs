use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use yss_chart_document::{ChartDocument, ChartResourcePath};
#[cfg(test)]
use yss_project_layout::CHART_EXTENSION;
use yss_project_layout::CHARTS_DIR;

use super::ProjectError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectChartIndexEntry {
    pub chart_path: ChartResourcePath,
    pub name: String,
    pub database_id: String,
    pub chart_type: String,
    pub revision: yss_project_identity::ResourceRevision,
}

pub fn read_chart_index_entries(root: &Path) -> Result<Vec<ProjectChartIndexEntry>, ProjectError> {
    let mut entries = scan_chart_documents(root)?
        .into_iter()
        .map(|(chart_path, document)| ProjectChartIndexEntry {
            name: chart_path.display_name().as_str().to_string(),
            chart_path,
            database_id: document.database_id,
            chart_type: document.chart_type,
            revision: document.revision,
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        a.chart_path
            .display_name()
            .portable_key()
            .cmp(&b.chart_path.display_name().portable_key())
    });
    Ok(entries)
}

pub fn load_chart_from_file(
    root: &Path,
    chart_path: &ChartResourcePath,
) -> Result<ChartDocument, ProjectError> {
    load_chart_from_root_readonly(root, chart_path)
}

pub(crate) fn load_chart_from_root_readonly(
    root: &Path,
    chart_path: &ChartResourcePath,
) -> Result<ChartDocument, ProjectError> {
    scan_chart_documents(root)?
        .into_iter()
        .find_map(|(path, document)| (path == *chart_path).then_some(document))
        .ok_or_else(|| {
            ProjectError::InvalidProjectFormat(format!("chart '{}' not found", chart_path.as_str()))
        })
}

pub fn serialize_chart(
    path: &ChartResourcePath,
    document: &ChartDocument,
) -> Result<(PathBuf, Vec<u8>), ProjectError> {
    let contents = serde_json::to_vec_pretty(document).map_err(ProjectError::Serialize)?;
    Ok((path.relative_path().to_path_buf(), contents))
}

pub(crate) fn load_charts_from_root(
    root: &Path,
) -> Result<HashMap<ChartResourcePath, ChartDocument>, ProjectError> {
    Ok(scan_chart_documents(root)?.into_iter().collect())
}

fn scan_chart_documents(
    root: &Path,
) -> Result<Vec<(ChartResourcePath, ChartDocument)>, ProjectError> {
    let charts_dir = root.join(CHARTS_DIR);
    let metadata = match std::fs::symlink_metadata(&charts_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(ProjectError::Io(error)),
    };
    reject_redirect(&charts_dir, &metadata)?;
    if !metadata.is_dir() {
        return Err(invalid_layout(&charts_dir, "chart root is not a directory"));
    }

    let mut documents = Vec::new();
    walk_chart_directory(&charts_dir, &charts_dir, &mut documents)?;
    documents.sort_by(|(left, _), (right, _)| left.cmp(right));
    let mut portable_paths = HashSet::new();
    for (path, _) in &documents {
        if !portable_paths.insert(path.display_name().portable_key()) {
            return Err(ProjectError::InvalidProjectFormat(format!(
                "portable chart path collision at '{}'",
                path.as_str()
            )));
        }
    }
    Ok(documents)
}

fn walk_chart_directory(
    charts_dir: &Path,
    directory: &Path,
    documents: &mut Vec<(ChartResourcePath, ChartDocument)>,
) -> Result<(), ProjectError> {
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        reject_redirect(&path, &metadata)?;
        if metadata.is_dir() {
            walk_chart_directory(charts_dir, &path, documents)?;
            continue;
        }
        if !metadata.is_file() {
            continue;
        }

        let relative = path
            .strip_prefix(charts_dir)
            .map_err(|_| invalid_layout(&path, "chart path escaped its root"))?;
        if relative
            .parent()
            .is_some_and(|parent| !parent.as_os_str().is_empty())
        {
            return Err(invalid_layout(
                &path,
                "nested chart files are not supported",
            ));
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid_layout(&path, "chart path is not valid Unicode"))?;
        let relative_path = format!("{CHARTS_DIR}/{file_name}");
        let relative_path = relative_path.as_str();
        let chart_path = ChartResourcePath::parse(relative_path).map_err(|error| {
            invalid_layout(&path, &format!("invalid chart resource path: {error}"))
        })?;
        if uuid::Uuid::parse_str(chart_path.display_name().as_str()).is_ok() {
            return Err(invalid_layout(
                &path,
                "invalid chart resource path: UUID filenames are forbidden",
            ));
        }
        if chart_path.relative_path() != relative_path {
            return Err(invalid_layout(
                &path,
                "invalid chart resource path: path is not canonical",
            ));
        }
        let document = read_chart_document_path(&path)?;
        documents.push((chart_path, document));
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
            "chart redirects/reparse points are forbidden",
        ));
    }
    Ok(())
}

fn invalid_layout(path: &Path, reason: &str) -> ProjectError {
    ProjectError::InvalidProjectFormat(format!("{reason}: '{}'", path.display()))
}

fn read_chart_document_path(path: &Path) -> Result<ChartDocument, ProjectError> {
    let content = std::fs::read_to_string(path).map_err(ProjectError::Io)?;
    serde_json::from_str(&content).map_err(ProjectError::Deserialize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_chart_document::{CURRENT_CHART_SCHEMA_VERSION, ChartResourcePath};
    use yss_resource_naming::ResourceName;

    fn temp_project_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("yssbi-chart-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_document_at(path: &Path, document: &ChartDocument) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec_pretty(document).unwrap()).unwrap();
    }

    fn chart_path(name: &str) -> ChartResourcePath {
        ChartResourcePath::from_name(&ResourceName::parse(name).unwrap())
    }

    fn document(revision: u64) -> ChartDocument {
        serde_json::from_value(serde_json::json!({
            "schemaVersion": CURRENT_CHART_SCHEMA_VERSION,
            "revision": revision,
            "databaseId": "db-1",
            "chartType": "histogram",
            "encodings": { "x": null, "y": null }
        }))
        .unwrap()
    }

    fn assert_all_read_entries_reject(root: &Path, chart_path: &ChartResourcePath) {
        assert!(load_charts_from_root(root).is_err());
        assert!(read_chart_index_entries(root).is_err());
        assert!(load_chart_from_file(root, chart_path).is_err());
    }

    #[test]
    fn canonical_name_path_is_shared_by_activation_index_and_direct_load() {
        let root = temp_project_dir();
        let path = chart_path("Root Sheet");
        let document = document(3);
        crate::fixtures::write_chart(root.as_path(), &path, &document).unwrap();
        let canonical = root.join(path.relative_path());

        assert!(canonical.is_file());
        assert_eq!(
            load_charts_from_root(root.as_path()).unwrap().get(&path),
            Some(&document)
        );
        let entries = read_chart_index_entries(root.as_path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].chart_path, path);
        assert_eq!(entries[0].name, "Root Sheet");
        assert_eq!(entries[0].revision.get(), 3);
        assert_eq!(
            load_chart_from_file(root.as_path(), &path).unwrap(),
            document
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn chart_load_rejects_unsupported_schema_version() {
        let root = temp_project_dir();
        let path = chart_path("Unsupported");
        let mut value = serde_json::to_value(document(0)).unwrap();
        value["schemaVersion"] = serde_json::json!(CURRENT_CHART_SCHEMA_VERSION + 1);
        let file = root.join(path.relative_path());
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

        let error = load_chart_from_file(&root, &path).unwrap_err();

        assert!(matches!(
            error,
            ProjectError::Deserialize(source)
                if source.to_string().contains("unsupported schema version 4")
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn chart_activation_rejects_uuid_filename() {
        let root = temp_project_dir();
        let uuid = uuid::Uuid::new_v4().to_string();
        let file = root
            .join(CHARTS_DIR)
            .join(format!("{uuid}.{CHART_EXTENSION}"));
        write_document_at(&file, &document(0));

        let error = load_charts_from_root(&root).unwrap_err().to_string();
        assert!(error.contains("invalid chart resource path"), "{error}");
        assert!(file.is_file(), "activation must not migrate invalid files");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn chart_activation_rejects_noncanonical_and_casefold_duplicate_paths() {
        let noncanonical_root = temp_project_dir();
        let noncanonical = noncanonical_root
            .join(CHARTS_DIR)
            .join(format!("Report.{}", CHART_EXTENSION.to_uppercase()));
        write_document_at(&noncanonical, &document(0));
        let error = load_charts_from_root(&noncanonical_root)
            .unwrap_err()
            .to_string();
        assert!(error.contains("invalid chart resource path"), "{error}");

        let wrong_extension_root = temp_project_dir();
        let wrong_extension = wrong_extension_root.join(CHARTS_DIR).join("Report.json");
        write_document_at(&wrong_extension, &document(0));
        let error = load_charts_from_root(&wrong_extension_root)
            .unwrap_err()
            .to_string();
        assert!(error.contains("invalid chart resource path"), "{error}");

        let duplicate_root = temp_project_dir();
        for name in ["Straße", "STRASSE"] {
            write_document_at(
                &duplicate_root
                    .join(CHARTS_DIR)
                    .join(format!("{name}.{CHART_EXTENSION}")),
                &document(0),
            );
        }
        let error = load_charts_from_root(&duplicate_root)
            .unwrap_err()
            .to_string();
        assert!(error.contains("portable chart path collision"), "{error}");

        let _ = std::fs::remove_dir_all(noncanonical_root);
        let _ = std::fs::remove_dir_all(wrong_extension_root);
        let _ = std::fs::remove_dir_all(duplicate_root);
    }

    #[test]
    fn chart_index_derives_name_from_path_and_includes_revision() {
        let root = temp_project_dir();
        let path = chart_path("销售分析 2");
        crate::fixtures::write_chart(&root, &path, &document(9)).unwrap();

        let entries = read_chart_index_entries(&root).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].chart_path, path);
        assert_eq!(entries[0].name, "销售分析 2");
        assert_eq!(entries[0].database_id, "db-1");
        assert_eq!(entries[0].chart_type, "histogram");
        assert_eq!(entries[0].revision.get(), 9);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nested_canonical_chart_is_rejected_without_flattening() {
        let root = temp_project_dir();
        let path = chart_path("Nested");
        let nested = root
            .join(CHARTS_DIR)
            .join("nested")
            .join(format!("Nested.{CHART_EXTENSION}"));
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
        let path = chart_path("External");
        let document = document(0);
        let external = external_root.join(format!("External.{CHART_EXTENSION}"));
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
        let path = chart_path("External");
        let document = document(0);
        crate::fixtures::write_chart(external_root.as_path(), &path, &document).unwrap();
        let charts = root.join(CHARTS_DIR);
        std::fs::create_dir_all(&charts).unwrap();
        if !create_test_junction(&charts.join("external"), &external_root.join(CHARTS_DIR)) {
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
        let charts = root.join(CHARTS_DIR);
        std::fs::create_dir_all(&charts).unwrap();
        if !create_test_junction(&charts.join("loop"), &charts) {
            eprintln!("skipping junction assertion: Windows junction creation is unavailable");
            return;
        }

        assert_all_read_entries_reject(root.as_path(), &chart_path("Missing"));

        let _ = std::fs::remove_dir_all(root);
    }
}
