use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::{
    GraphResourceIndex, GraphResourcePath, ProjectChartIndexEntry, ProjectError,
    load_charts_from_root, read_chart_index_entries, scan_graph_resource_index,
};
use crate::filesystem::project_root_from_path;
use crate::manifest::{CURRENT_PROJECT_SCHEMA_VERSION, ProjectManifest};
use yss_database_contract::{DatabaseDecl, DatabaseEngine};
use yss_database_store::DatasetStore;
use yss_function_editor_projection::FunctionEditorProjection;
use yss_graph_document::{GraphDocument as NodeGraphDocument, GraphResourceKind};
use yss_project_identity::ProjectResourcePath;
#[cfg(any(test, feature = "test-support"))]
use yss_project_layout::PROJECT_CONTENT_DIRECTORIES;
use yss_project_layout::{DATABASE_DIR, PROJECT_DATASET_CATALOG_FILE, PROJECT_METADATA_FILE};
use yss_project_model::{GraphResourceDocument, ProjectData};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphResourceFile {
    pub kind: GraphResourceKind,
    pub name: String,
    pub document: Arc<NodeGraphDocument>,
    pub function: Option<yss_project_history::FunctionDocument>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEventGraphIndexEntry {
    pub path: String,
    pub name: String,
    #[serde(rename = "type")]
    file_kind: GraphResourceKind,
    pub revision: yss_project_identity::ResourceRevision,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFunctionGraphIndexEntry {
    pub path: String,
    pub name: String,
    #[serde(rename = "type")]
    file_kind: GraphResourceKind,
    pub revision: yss_project_identity::ResourceRevision,
    pub function_revision: yss_project_identity::ResourceRevision,
    pub function_signature: yss_project_history::FunctionSignature,
    pub function_editor_projection: FunctionEditorProjection,
}

struct ScannedNodeFileHeader {
    path: String,
    name: String,
    function: Option<yss_project_history::FunctionDocument>,
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
    pub minds: Vec<crate::minds::MindIndexEntry>,
    pub docs: Vec<crate::docs::DocIndexEntry>,
    pub project_instance_id: String,
    pub publication_revision: u64,
    #[serde(skip)]
    pub(crate) authority_generation: u64,
    pub project_name: String,
    pub export_time: String,
    pub event_graphs: Vec<ProjectEventGraphIndexEntry>,
    pub function_graphs: Vec<ProjectFunctionGraphIndexEntry>,
    pub charts: Vec<ProjectChartIndexEntry>,
    pub databases: Vec<ProjectDatabaseIndexEntry>,
}

impl ProjectIndex {
    pub const fn authority_generation(&self) -> u64 {
        self.authority_generation
    }
}

pub fn serialize_project_manifest(data: &ProjectData) -> Result<Vec<u8>, ProjectError> {
    serde_json::to_vec_pretty(&project_manifest_from_data(data)).map_err(ProjectError::Serialize)
}

fn project_manifest_from_data(data: &ProjectData) -> ProjectManifest {
    ProjectManifest::new(
        data.metadata.project_name.clone(),
        data.metadata.export_time.clone(),
    )
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
) -> Result<(), ProjectError> {
    let (relative_path, contents) = serialize_graph_document(project_data, graph_path)?;
    let path = root.join(relative_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    Ok(())
}

#[cfg(any(test, feature = "test-support"))]
fn save_project_to_directory(project_data: &ProjectData, root: &Path) -> Result<(), ProjectError> {
    std::fs::create_dir_all(root)?;
    for directory in PROJECT_CONTENT_DIRECTORIES {
        std::fs::create_dir_all(root.join(directory))?;
    }

    if !root.join(relative_dataset_catalog_path()).exists() {
        DatasetStore::create(root)
            .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))?;
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
    for (path, document) in &project_data.minds {
        use yss_project_model::file::FileContent;
        std::fs::write(
            root.join(path.as_str()),
            document
                .content()
                .encode()
                .map_err(ProjectError::InvalidProjectFormat)?,
        )?;
    }
    for (path, document) in &project_data.docs {
        use yss_project_model::file::FileContent;
        std::fs::write(
            root.join(path.as_str()),
            document
                .content()
                .encode()
                .map_err(ProjectError::InvalidProjectFormat)?,
        )?;
    }

    let manifest = project_manifest_from_data(project_data);
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
    project_data.minds = crate::file_resources::load_files(root.as_path())?;
    project_data.docs = crate::file_resources::load_files(root.as_path())?;

    Ok(project_data)
}

pub fn read_project_index(path: &str) -> Result<ProjectIndex, ProjectError> {
    let root = project_root_from_path(path);
    read_project_index_from_root(root.as_path())
}

pub(crate) fn read_project_index_from_root(root: &Path) -> Result<ProjectIndex, ProjectError> {
    let manifest = read_project_manifest_from_root(root)?;
    let graph_resources = load_graph_resource_index(root)?;
    let event_graphs =
        read_node_file_headers(root, GraphResourceKind::EventGraph, &graph_resources)?
            .into_iter()
            .map(|file| ProjectEventGraphIndexEntry {
                path: file.path,
                name: file.name,
                file_kind: GraphResourceKind::EventGraph,
                revision: yss_project_identity::ResourceRevision::INITIAL,
            })
            .collect();
    let function_graphs =
        read_node_file_headers(root, GraphResourceKind::FunctionGraph, &graph_resources)?
            .into_iter()
            .map(|file| {
                let function = file.function.ok_or_else(|| {
                    ProjectError::InvalidProjectFormat("function file has no signature".into())
                })?;
                let projection = FunctionEditorProjection::try_from(&function)
                    .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))?;
                Ok(ProjectFunctionGraphIndexEntry {
                    path: file.path,
                    name: file.name,
                    file_kind: GraphResourceKind::FunctionGraph,
                    revision: yss_project_identity::ResourceRevision::INITIAL,
                    function_revision: function.revision,
                    function_signature: function.signature,
                    function_editor_projection: projection,
                })
            })
            .collect::<Result<Vec<_>, ProjectError>>()?;
    let charts = read_chart_index_entries(root)?;

    let (project_name, export_time) = manifest.into_parts();
    Ok(ProjectIndex {
        minds: crate::file_resources::file_index(root)?,
        docs: crate::file_resources::file_index(root)?,
        project_instance_id: String::new(),
        publication_revision: 0,
        authority_generation: 0,
        project_name,
        export_time,
        event_graphs,
        function_graphs,
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
    load_project_graph_document_from_index(root.as_path(), &graph_resources, graph_path)
}

fn load_project_graph_document_from_index(
    root: &Path,
    graph_resources: &GraphResourceIndex,
    graph_path: &GraphResourcePath,
) -> Result<GraphResourceFile, ProjectError> {
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
    let root = project_root_from_path(path);
    let graph_resources = load_graph_resource_index(root.as_path())?;
    load_project_graph_from_index(root.as_path(), &graph_resources, graph_path)
}

pub(crate) fn load_project_graph_from_index(
    root: &Path,
    graph_resources: &GraphResourceIndex,
    graph_path: &GraphResourcePath,
) -> Result<GraphResourceDocument, ProjectError> {
    let document = load_project_graph_document_from_index(root, graph_resources, graph_path)?;
    Ok(yss_project_model::GraphResourceDocument {
        name: document.name,
        kind: document.kind,
        document: document.document,
        function: document.function,
    })
}

pub(crate) fn read_project_manifest_from_root(
    root: &Path,
) -> Result<ProjectManifest, ProjectError> {
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

pub(crate) fn read_graph_document(
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
    // Validate the persisted field even though display names come from resource paths.
    #[serde(rename = "name")]
    _name: String,
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
        (GraphResourceKind::FunctionGraph, None) => {
            Err(ProjectError::InvalidProjectFormat(format!(
                "function graph file '{}' is missing its function document",
                path.display()
            )))
        }
        (GraphResourceKind::EventGraph, Some(_)) => {
            Err(ProjectError::InvalidProjectFormat(format!(
                "event graph file '{}' must not contain a function document",
                path.display()
            )))
        }
        _ => Ok(()),
    }
}

fn graph_name_from_file_path(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.trim().to_string())
        .filter(|stem| !stem.is_empty())
}

fn read_node_file_headers(
    root: &Path,
    expected_kind: GraphResourceKind,
    graph_resources: &GraphResourceIndex,
) -> Result<Vec<ScannedNodeFileHeader>, ProjectError> {
    let mut resources: Vec<_> = graph_resources
        .entries()
        .iter()
        .filter(|entry| entry.kind == expected_kind)
        .collect();
    resources.sort_by_key(|entry| entry.path.as_str().to_lowercase());
    resources
        .into_iter()
        .map(|resource| {
            let path = root.join(resource.path.as_str());
            let header = read_graph_file_header(&path)?;
            if header.kind != expected_kind {
                return Err(ProjectError::InvalidProjectFormat(format!(
                    "graph file '{}' kind does not match its resource directory",
                    path.display()
                )));
            }
            Ok(ScannedNodeFileHeader {
                path: resource.path.as_str().to_owned(),
                name: resource.path.display_name().to_owned(),
                function: header.function,
            })
        })
        .collect()
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

pub fn relative_dataset_catalog_path() -> String {
    format!("{}/{}", DATABASE_DIR, PROJECT_DATASET_CATALOG_FILE)
}

/// Rebuild declarations solely from the committed catalog after manifest validation.
pub fn discover_databases_from_root(
    root: &Path,
) -> Result<HashMap<String, DatabaseDecl>, ProjectError> {
    let store = DatasetStore::open(root)
        .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))?;
    store
        .catalog_metadata()
        .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))?
        .into_iter()
        .map(|metadata| {
            let id = metadata.id.as_str().to_owned();
            Ok((
                id,
                DatabaseDecl {
                    id: metadata.id,
                    engine: DatabaseEngine::Dataset {},
                    schema_version: CURRENT_PROJECT_SCHEMA_VERSION,
                    required: false,
                    name: metadata.name,
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod project_manifest_adapter_tests {
    use super::{ProjectManifest, serialize_project_manifest};
    use yss_project_model::ProjectData;

    #[test]
    fn fixture_writes_graph_identity_without_renaming_from_display_name() {
        use yss_graph_document::{GraphResourceKind, GraphResourcePath};
        use yss_project_model::GraphResourceDocument;

        let mut data = ProjectData::new();
        for path in ["events/First.yssbi-event", "events/Second.yssbi-event"] {
            data.graphs.insert(
                GraphResourcePath::new(path).unwrap(),
                GraphResourceDocument::new("Shared display name", GraphResourceKind::EventGraph),
            );
        }
        let fixture = crate::fixtures::TempProject::activate("fixture-resource-paths", data);
        let session = fixture.state().capture_project_session().unwrap();
        let index = super::read_project_index_from_root(session.root.as_path()).unwrap();
        assert_eq!(
            index
                .event_graphs
                .iter()
                .map(|event| event.path.as_str())
                .collect::<Vec<_>>(),
            ["events/First.yssbi-event", "events/Second.yssbi-event"]
        );
        assert_eq!(
            index
                .event_graphs
                .iter()
                .map(|event| event.name.as_str())
                .collect::<Vec<_>>(),
            ["First", "Second"]
        );

        let path = session.root.as_path().join("events/First.yssbi-event");
        let mut wire: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let node_id = "00000000-0000-0000-0000-000000000001";
        wire["document"]["nodes"][node_id] = serde_json::json!({
            "id": node_id,
            "node_type": "acme.extension.source",
            "position": {"x": 0, "y": 0},
            "parameters": {"value": 1},
            "user_label": null,
        });
        let parse = |wire: &serde_json::Value| {
            super::parse_graph_resource_document(
                &serde_json::to_vec(wire).unwrap(),
                &path,
                GraphResourceKind::EventGraph,
            )
        };
        assert!(parse(&wire).is_ok());
        for field in ["node_type", "parameters"] {
            let mut invalid = wire.clone();
            invalid["document"]["nodes"][node_id][field] = if field == "node_type" {
                serde_json::json!(" Invalid Node Type ")
            } else {
                serde_json::json!({"bad key": 1})
            };
            assert!(matches!(
                parse(&invalid),
                Err(super::ProjectError::Deserialize(_))
            ));
        }
    }

    #[test]
    fn project_manifest_serialization_uses_the_canonical_validated_contract() {
        let mut data = ProjectData::new();
        data.metadata.project_name = "Canonical Manifest".into();
        data.metadata.export_time = "2026-08-30T00:00:00".into();

        let contents = serialize_project_manifest(&data).unwrap();
        let manifest: ProjectManifest = serde_json::from_slice(&contents).unwrap();

        assert_eq!(
            manifest.into_parts(),
            ("Canonical Manifest".into(), "2026-08-30T00:00:00".into())
        );
    }
}
