use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    GraphResourceIndex, GraphResourcePath, ProjectChartIndexEntry, ProjectError,
    load_charts_from_root, read_chart_index_entries, scan_graph_resource_index,
};
use crate::manifest::{CURRENT_PROJECT_SCHEMA_VERSION, ProjectManifest};
use yss_database_contract::{DatabaseDecl, DatabaseEngine, DatabaseId};
use yss_duckdb::{list_data_tables, read_display_name};
use yss_function_editor_projection::FunctionEditorProjection;
use yss_graph_document::{GraphDocument as NodeGraphDocument, GraphResourceKind};
use yss_project_filesystem::project_root_from_path;
use yss_project_identity::ProjectResourcePath;
#[cfg(any(test, feature = "test-support"))]
use yss_project_layout::PROJECT_CONTENT_DIRECTORIES;
use yss_project_layout::{
    DATABASE_DIR, EVENT_EXTENSION, EVENTS_DIR, FUNCTION_EXTENSION, FUNCTIONS_DIR,
    PROJECT_DUCKDB_FILE, PROJECT_METADATA_FILE,
};
use yss_project_model::{GraphResourceDocument, ProjectData};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphResourceFile {
    pub kind: GraphResourceKind,
    pub name: String,
    pub document: NodeGraphDocument,
    pub function: Option<yss_project_history::FunctionDocument>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGraphIndexEntry {
    pub path: String,
    pub name: String,
    #[serde(rename = "type")]
    pub graph_type: GraphResourceKind,
    pub revision: yss_project_identity::ResourceRevision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_revision: Option<yss_project_identity::ResourceRevision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_signature: Option<yss_project_history::FunctionSignature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_editor_projection: Option<FunctionEditorProjection>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDatabaseIndexEntry {
    pub id: String,
    pub resource_path: ProjectResourcePath,
    pub revision: yss_project_identity::ResourceRevision,
    pub engine: yss_database_contract::DatabaseEngine,
    pub schema_version: u32,
    pub required: bool,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIndex {
    pub project_instance_id: String,
    #[serde(default)]
    pub publication_revision: u64,
    #[serde(skip)]
    pub(crate) authority_generation: u64,
    #[serde(default)]
    pub project_name: String,
    pub export_time: String,
    pub graphs: Vec<ProjectGraphIndexEntry>,
    #[serde(default)]
    pub charts: Vec<ProjectChartIndexEntry>,
    #[serde(default)]
    pub databases: Vec<ProjectDatabaseIndexEntry>,
}

impl ProjectIndex {
    pub const fn authority_generation(&self) -> u64 {
        self.authority_generation
    }
}

pub fn serialize_project_manifest(data: &ProjectData) -> Result<Vec<u8>, ProjectError> {
    serde_json::to_vec_pretty(&project_manifest_from_data(data)?).map_err(ProjectError::Serialize)
}

fn project_manifest_from_data(data: &ProjectData) -> Result<ProjectManifest, ProjectError> {
    Ok(ProjectManifest::new(
        data.metadata.project_name.clone(),
        data.metadata.export_time.clone(),
    ))
}

pub fn serialize_graph_document(
    data: &ProjectData,
    graph_path: &GraphResourcePath,
) -> Result<(PathBuf, Vec<u8>), ProjectError> {
    let document = snapshot_graph_document(data, graph_path)?;
    serde_json::to_vec_pretty(&document)
        .map(|contents| (PathBuf::from(graph_path.as_str()), contents))
        .map_err(ProjectError::Serialize)
}

pub(crate) fn snapshot_graph_document(
    data: &ProjectData,
    graph_path: &GraphResourcePath,
) -> Result<GraphResourceFile, ProjectError> {
    let graph = data.graphs.get(graph_path).ok_or_else(|| {
        ProjectError::InvalidProjectFormat(format!("graph '{}' not loaded", graph_path))
    })?;
    Ok(graph_document_from_resource(graph))
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn initialize_project_directory(
    project_data: &ProjectData,
    root: &Path,
) -> Result<(), ProjectError> {
    save_project_to_directory(project_data, root)
}

pub(crate) fn serialize_graph_resource_document(
    graph: &GraphResourceDocument,
) -> Result<Vec<u8>, ProjectError> {
    serde_json::to_vec_pretty(&graph_document_from_resource(graph)).map_err(ProjectError::Serialize)
}

fn graph_document_from_resource(graph: &GraphResourceDocument) -> GraphResourceFile {
    GraphResourceFile {
        kind: graph.kind,
        name: graph.name.clone(),
        document: graph.document.clone(),
        function: graph.function.clone(),
    }
}

#[cfg(any(test, feature = "test-support"))]
fn write_loaded_graph_document(
    project_data: &ProjectData,
    root: &Path,
    graph_path: &GraphResourcePath,
) -> Result<String, ProjectError> {
    let graph = project_data.graphs.get(graph_path).ok_or_else(|| {
        ProjectError::InvalidProjectFormat(format!("graph '{}' not loaded", graph_path))
    })?;
    let (dir, extension) = match graph.kind {
        GraphResourceKind::Event => (EVENTS_DIR, EVENT_EXTENSION),
        GraphResourceKind::Function => (FUNCTIONS_DIR, FUNCTION_EXTENSION),
    };
    let relative_path =
        graph_relative_path_for_save(root, dir, extension, &graph.name, graph_path)?;
    write_json(
        root.join(&relative_path).as_path(),
        &GraphResourceFile {
            kind: graph.kind,
            name: graph.name.clone(),
            document: graph.document.clone(),
            function: graph.function.clone(),
        },
    )?;
    Ok(relative_path)
}

#[cfg(any(test, feature = "test-support"))]
fn save_project_to_directory(project_data: &ProjectData, root: &Path) -> Result<(), ProjectError> {
    std::fs::create_dir_all(root)?;
    for directory in PROJECT_CONTENT_DIRECTORIES {
        std::fs::create_dir_all(root.join(directory))?;
    }

    scan_graph_resource_index(root)?;

    for graph_path in project_data.graphs.keys() {
        write_loaded_graph_document(project_data, root, graph_path)?;
    }
    for (chart_path, chart) in &project_data.charts {
        let (relative_path, contents) = super::serialize_chart(chart_path, chart)?;
        let target = root.join(relative_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(target, contents)?;
    }

    let manifest = project_manifest_from_data(project_data)?;
    write_json(root.join(PROJECT_METADATA_FILE).as_path(), &manifest)?;
    Ok(())
}

/// 从文件加载项目
pub fn load_project_from_file(path: &str) -> Result<ProjectData, ProjectError> {
    let root = project_root_from_path(path);
    let manifest = read_project_manifest_from_root(root.as_path())?;
    let (project_name, export_time) = manifest.into_parts();
    let mut project_data = ProjectData::new();
    project_data.metadata.project_name = project_name;
    project_data.metadata.export_time = export_time;
    project_data.databases = discover_databases_from_root(root.as_path())?;
    project_data.charts = load_charts_from_root(root.as_path())?;

    Ok(project_data)
}

pub fn read_project_index(path: &str) -> Result<ProjectIndex, ProjectError> {
    let root = project_root_from_path(path);
    read_project_index_from_root(root.as_path())
}

pub(crate) fn read_project_index_from_root(root: &Path) -> Result<ProjectIndex, ProjectError> {
    let manifest = read_project_manifest_from_root(root)?;
    let graph_resources = load_graph_resource_index(root)?;
    let mut graphs = Vec::new();
    graphs.extend(read_graph_index_entries(
        root,
        EVENTS_DIR,
        EVENT_EXTENSION,
        GraphResourceKind::Event,
        &graph_resources,
    )?);
    graphs.extend(read_graph_index_entries(
        root,
        FUNCTIONS_DIR,
        FUNCTION_EXTENSION,
        GraphResourceKind::Function,
        &graph_resources,
    )?);
    let charts = read_chart_index_entries(root)?;

    let (project_name, export_time) = manifest.into_parts();
    Ok(ProjectIndex {
        project_instance_id: String::new(),
        publication_revision: 0,
        authority_generation: 0,
        project_name,
        export_time,
        graphs,
        charts,
        databases: Vec::new(),
    })
}

pub(crate) fn load_project_graph_document_from_file(
    path: &str,
    graph_path: &GraphResourcePath,
) -> Result<GraphResourceFile, ProjectError> {
    let root = project_root_from_path(path);
    let graph_resources = load_graph_resource_index(root.as_path())?;
    if let Some(resource) = graph_resources.get_by_path(graph_path.as_str()) {
        let document =
            read_graph_document(root.join(resource.path.as_str()).as_path(), resource.kind)?;
        return Ok(document);
    }

    Err(ProjectError::InvalidProjectFormat(format!(
        "graph '{}' not found in project graph files",
        graph_path
    )))
}

pub fn load_project_graph_from_file(
    path: &str,
    graph_path: &GraphResourcePath,
) -> Result<yss_project_model::GraphResourceDocument, ProjectError> {
    let document = load_project_graph_document_from_file(path, graph_path)?;
    Ok(yss_project_model::GraphResourceDocument {
        name: document.name,
        kind: document.kind,
        document: document.document,
        function: document.function,
    })
}

fn read_project_manifest_from_root(root: &Path) -> Result<ProjectManifest, ProjectError> {
    read_json(root.join(PROJECT_METADATA_FILE).as_path())
}

fn load_graph_resource_index(root: &Path) -> Result<GraphResourceIndex, ProjectError> {
    scan_graph_resource_index(root)
}

pub(crate) fn parse_graph_resource_document(
    contents: &[u8],
    path: &Path,
    expected_kind: GraphResourceKind,
) -> Result<GraphResourceFile, ProjectError> {
    let document: GraphResourceFile =
        serde_json::from_slice(contents).map_err(ProjectError::Deserialize)?;
    if document.kind != expected_kind {
        return Err(ProjectError::InvalidProjectFormat(format!(
            "graph file '{}' kind does not match manifest",
            path.display()
        )));
    }
    validate_function_shape(path, document.kind, document.function.as_ref())?;
    yss_graph_document::validate_constant_definitions(&document.document.constants)
        .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))?;
    Ok(document)
}

fn read_graph_document(
    path: &Path,
    expected_kind: GraphResourceKind,
) -> Result<GraphResourceFile, ProjectError> {
    let contents = std::fs::read(path)?;
    let mut document = parse_graph_resource_document(&contents, path, expected_kind)?;
    if let Some(name) = graph_name_from_file_path(path) {
        document.name = name;
    }
    Ok(document)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphFileHeader {
    kind: GraphResourceKind,
    name: String,
    function: Option<yss_project_history::FunctionDocument>,
}

fn read_graph_file_header(path: &Path) -> Result<GraphFileHeader, ProjectError> {
    let header: GraphFileHeader = read_json(path)?;
    validate_function_shape(path, header.kind, header.function.as_ref())?;
    Ok(header)
}

fn validate_function_shape(
    path: &Path,
    kind: GraphResourceKind,
    function: Option<&yss_project_history::FunctionDocument>,
) -> Result<(), ProjectError> {
    match (kind, function) {
        (GraphResourceKind::Function, None) => Err(ProjectError::InvalidProjectFormat(format!(
            "function graph file '{}' is missing its function document",
            path.display()
        ))),
        (GraphResourceKind::Event, Some(_)) => Err(ProjectError::InvalidProjectFormat(format!(
            "event graph file '{}' must not contain a function document",
            path.display()
        ))),
        _ => Ok(()),
    }
}

fn graph_name_from_file_path(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.trim().to_string())
        .filter(|stem| !stem.is_empty())
}

fn read_graph_index_entries(
    root: &Path,
    dir: &str,
    extension: &str,
    expected_kind: GraphResourceKind,
    graph_resources: &GraphResourceIndex,
) -> Result<Vec<ProjectGraphIndexEntry>, ProjectError> {
    let mut entries = Vec::new();
    for path in list_graph_files(root, dir, extension)? {
        let header = read_graph_file_header(path.as_path())?;
        let relative_path = path_to_slash_string(
            path.strip_prefix(root)
                .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))?,
        );
        let Some(resource) = graph_resources.get_by_path(&relative_path) else {
            continue;
        };
        if header.kind != expected_kind {
            return Err(ProjectError::InvalidProjectFormat(format!(
                "graph file '{}' kind does not match its resource directory",
                path.display()
            )));
        }
        let name = graph_name_from_file_path(path.as_path()).unwrap_or(header.name);
        let function_editor_projection = header
            .function
            .as_ref()
            .map(FunctionEditorProjection::try_from)
            .transpose()
            .map_err(|error| {
                ProjectError::InvalidProjectFormat(format!(
                    "function graph file '{}' has an invalid editor projection: {error}",
                    path.display()
                ))
            })?;
        let (function_revision, function_signature) = header
            .function
            .map(|function| (Some(function.revision), Some(function.signature)))
            .unwrap_or((None, None));
        entries.push(ProjectGraphIndexEntry {
            path: resource.path.as_str().to_string(),
            name,
            graph_type: expected_kind,
            revision: yss_project_identity::ResourceRevision::INITIAL,
            function_revision,
            function_signature,
            function_editor_projection,
        });
    }
    Ok(entries)
}

fn list_graph_files(root: &Path, dir: &str, extension: &str) -> Result<Vec<PathBuf>, ProjectError> {
    let graph_dir = root.join(dir);
    if !graph_dir.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in std::fs::read_dir(&graph_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case(extension))
                .unwrap_or(false)
        {
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

#[cfg(any(test, feature = "test-support"))]
fn graph_relative_path_for_save(
    root: &Path,
    dir: &str,
    extension: &str,
    graph_name: &str,
    graph_path: &GraphResourcePath,
) -> Result<String, ProjectError> {
    let target_dir = root.join(dir);
    std::fs::create_dir_all(&target_dir)?;
    let existing_path = find_graph_file_path(root, dir, extension, graph_path)?;
    let file_name = unique_graph_file_name(
        target_dir.as_path(),
        graph_name,
        extension,
        existing_path.as_deref(),
    );
    let next_path = target_dir.join(&file_name);
    if let Some(existing_path) = existing_path
        && existing_path != next_path
        && existing_path.exists()
    {
        std::fs::remove_file(existing_path)?;
    }
    next_path
        .strip_prefix(root)
        .map(path_to_slash_string)
        .map_err(|e| ProjectError::InvalidProjectFormat(e.to_string()))
}

#[cfg(any(test, feature = "test-support"))]
fn find_graph_file_path(
    root: &Path,
    dir: &str,
    _extension: &str,
    graph_path: &GraphResourcePath,
) -> Result<Option<PathBuf>, ProjectError> {
    let graph_resources = match load_graph_resource_index(root) {
        Ok(index) => index,
        Err(ProjectError::FileNotFound(_)) => return Ok(None),
        Err(error) => return Err(error),
    };
    if let Some(resource) = graph_resources.get_by_path(graph_path.as_str())
        && resource.path.as_str().starts_with(&format!("{dir}/"))
    {
        return Ok(Some(root.join(resource.path.as_str())));
    }
    Ok(None)
}

pub(crate) fn find_graph_document_path(
    root: &Path,
    graph_path: &GraphResourcePath,
) -> Result<Option<(PathBuf, GraphResourceKind, GraphResourceFile)>, ProjectError> {
    if let Some(resource) = load_graph_resource_index(root)?.get_by_path(graph_path.as_str()) {
        let path = root.join(resource.path.as_str());
        let document = read_graph_document(path.as_path(), resource.kind)?;
        return Ok(Some((path, resource.kind, document)));
    }
    Ok(None)
}

#[cfg(any(test, feature = "test-support"))]
fn unique_graph_file_name(
    dir: &Path,
    graph_name: &str,
    extension: &str,
    existing_path: Option<&Path>,
) -> String {
    let stem = sanitize_file_stem(graph_name);
    for index in 0.. {
        let candidate = if index == 0 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem} {index}.{extension}")
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
    unreachable!("unique file name loop should always return")
}

#[cfg(any(test, feature = "test-support"))]
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
        "Untitled".to_string()
    } else {
        sanitized.to_string()
    }
}

fn path_to_slash_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ProjectError> {
    let content = std::fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(ProjectError::Deserialize)
}

#[cfg(any(test, feature = "test-support"))]
fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ProjectError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(value).map_err(ProjectError::Serialize)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn relative_project_duckdb_path() -> String {
    format!("{}/{}", DATABASE_DIR, PROJECT_DUCKDB_FILE)
}

pub fn project_duckdb_abs(root: &Path) -> PathBuf {
    root.join(relative_project_duckdb_path())
}

/// 打开项目时枚举 `database/project.duckdb` 内的用户表，重建运行时 `DatabaseDecl` 索引。
pub fn discover_databases_from_root(
    root: &Path,
) -> Result<HashMap<String, DatabaseDecl>, ProjectError> {
    let mut map = HashMap::new();
    let duckdb_path = project_duckdb_abs(root);
    let tables = list_data_tables(&duckdb_path).map_err(|e| {
        ProjectError::InvalidProjectFormat(format!("Failed to list DuckDB tables: {e}"))
    })?;

    let relative_path = relative_project_duckdb_path();
    for table in tables {
        let display_name = read_display_name(&duckdb_path, &table).unwrap_or_else(|| table.clone());
        let decl = DatabaseDecl {
            id: DatabaseId::from_existing(table.clone().into()),
            engine: DatabaseEngine::DuckDb {
                path: relative_path.clone(),
                table: table.clone(),
            },
            schema_version: CURRENT_PROJECT_SCHEMA_VERSION,
            required: false,
            name: display_name.into(),
        };
        map.insert(table, decl);
    }

    Ok(map)
}

#[cfg(test)]
mod project_manifest_adapter_tests {
    use super::{ProjectManifest, serialize_project_manifest};
    use yss_project_model::ProjectData;

    #[test]
    fn project_manifest_serialization_uses_the_canonical_validated_contract() {
        let mut data = ProjectData::new();
        data.metadata.project_name = "Canonical Manifest".into();
        data.metadata.export_time = "2026-08-30T00:00:00Z".into();

        let contents = serialize_project_manifest(&data).unwrap();
        let manifest: ProjectManifest = serde_json::from_slice(&contents).unwrap();

        assert_eq!(
            manifest.into_parts(),
            ("Canonical Manifest".into(), "2026-08-30T00:00:00Z".into())
        );
    }
}
