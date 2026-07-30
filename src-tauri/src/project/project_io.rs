use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[cfg(test)]
use super::ensure_worksheets_dir;
use super::{
    GraphResourceDocument, GraphResourceIndex, GraphResourcePath, PROJECT_METADATA_FILE,
    ProjectData, ProjectError, ProjectWorksheetIndexEntry, load_worksheets_from_root,
    read_worksheet_index_entries, scan_graph_resource_index,
};
use crate::database::{DatabaseDecl, DatabaseEngine};

use crate::node_system::document::{GraphDocument as NodeGraphDocument, NodeId};
use crate::variable::{VariableId, VariableInstance, VariableScope};

pub const SCHEMA_VERSION: u32 = 3;
pub const EVENTS_DIR: &str = "events";
pub const FUNCTIONS_DIR: &str = "functions";
pub const DATABASE_DIR: &str = "database";
pub const PROJECT_DUCKDB_FILE: &str = "project.duckdb";
pub const GLOBAL_VARIABLES_FILE: &str = "variables.yssbi-vars";
pub const EVENT_EXTENSION: &str = "yssbi-event";
pub const FUNCTION_EXTENSION: &str = "yssbi-function";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectManifest {
    pub schema_version: u32,
    pub project_name: String,
    pub app_version: String,
    pub export_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalVariablesDocument {
    pub schema_version: u32,
    pub variables: HashMap<VariableId, VariableInstance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDocument {
    pub schema_version: u32,
    pub kind: GraphDocumentKind,
    pub name: String,
    #[serde(default)]
    pub revision: crate::node_system::document::ResourceRevision,
    pub document: NodeGraphDocument,
    pub function: Option<crate::node_system::document::FunctionDocument>,
    pub local_variables: HashMap<VariableId, VariableInstance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphDocumentKind {
    Event,
    Function,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGraphIndexEntry {
    pub path: String,
    pub name: String,
    #[serde(rename = "type")]
    pub graph_type: GraphDocumentKind,
    pub revision: crate::node_system::document::ResourceRevision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_revision: Option<crate::node_system::document::ResourceRevision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_signature: Option<crate::node_system::document::FunctionSignature>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVariableIndexEntry {
    pub id: String,
    pub revision: crate::node_system::document::ResourceRevision,
    pub name: String,
    pub data_type: crate::graph::value::DataType,
    pub data_value: crate::graph::value::DataValue,
    pub description: String,
    pub scope: VariableScope,
    pub tags: Vec<String>,
    pub owner_graph_path: Option<String>,
    pub owner_graph_name: Option<String>,
    #[serde(rename = "ownerGraphKind", skip_serializing_if = "Option::is_none")]
    pub owner_graph_kind: Option<GraphDocumentKind>,
}

impl From<VariableInstance> for ProjectVariableIndexEntry {
    fn from(value: VariableInstance) -> Self {
        Self {
            id: value.id.to_string(),
            revision: crate::node_system::document::ResourceRevision::INITIAL,
            name: value.name,
            data_type: value.data_type,
            data_value: value.data_value,
            description: value.description,
            scope: value.scope,
            tags: value.tags,
            owner_graph_path: None,
            owner_graph_name: None,
            owner_graph_kind: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIndex {
    pub project_instance_id: String,
    #[serde(default)]
    pub publication_revision: u64,
    #[serde(default)]
    pub history: crate::node_system::document::HistoryStatusDto,
    pub project_name: String,
    pub app_version: String,
    pub export_time: String,
    pub graphs: Vec<ProjectGraphIndexEntry>,
    #[serde(default)]
    pub worksheets: Vec<ProjectWorksheetIndexEntry>,
    #[serde(default)]
    pub variables: Vec<ProjectVariableIndexEntry>,
}

pub fn serialize_project_manifest(data: &ProjectData) -> Result<Vec<u8>, ProjectError> {
    serde_json::to_vec_pretty(&ProjectManifest {
        schema_version: SCHEMA_VERSION,
        project_name: data.metadata.project_name.clone(),
        app_version: data.metadata.app_version.clone(),
        export_time: data.metadata.export_time.clone(),
    })
    .map_err(ProjectError::Serialize)
}

pub fn serialize_global_variables(data: &ProjectData) -> Result<Vec<u8>, ProjectError> {
    let variables = data
        .variables
        .iter()
        .filter(|(_, variable)| matches!(variable.scope, VariableScope::Global))
        .map(|(id, variable)| (*id, variable.clone()))
        .collect();
    serialize_global_variable_map(variables)
}

pub(crate) fn serialize_global_variable_map(
    variables: std::collections::HashMap<
        crate::variable::VariableId,
        crate::variable::VariableInstance,
    >,
) -> Result<Vec<u8>, ProjectError> {
    serde_json::to_vec_pretty(&GlobalVariablesDocument {
        schema_version: SCHEMA_VERSION,
        variables,
    })
    .map_err(ProjectError::Serialize)
}

pub fn serialize_graph_document(
    data: &ProjectData,
    graph_path: &GraphResourcePath,
) -> Result<(PathBuf, Vec<u8>), ProjectError> {
    let graph = data.graphs.get(graph_path).ok_or_else(|| {
        ProjectError::InvalidProjectFormat(format!("graph '{}' not loaded", graph_path))
    })?;
    let local_variables = local_variables_for_graph(&data.variables, graph_path, graph.kind);
    serialize_graph_resource_document(graph, local_variables)
        .map(|contents| (PathBuf::from(graph_path.as_str()), contents))
}

#[cfg(test)]
pub(crate) fn initialize_project_directory(
    project_data: &ProjectData,
    root: &Path,
) -> Result<(), ProjectError> {
    save_project_to_directory(project_data, root)
}

pub(crate) fn serialize_graph_resource_document(
    graph: &GraphResourceDocument,
    local_variables: HashMap<VariableId, VariableInstance>,
) -> Result<Vec<u8>, ProjectError> {
    serde_json::to_vec_pretty(&GraphDocument {
        schema_version: SCHEMA_VERSION,
        kind: graph.kind,
        name: graph.name.clone(),
        revision: graph.document.revision,
        document: graph.document.clone(),
        function: graph.function.clone(),
        local_variables,
    })
    .map_err(ProjectError::Serialize)
}

#[cfg(test)]
fn write_loaded_graph_document(
    project_data: &ProjectData,
    root: &Path,
    graph_path: &GraphResourcePath,
) -> Result<String, ProjectError> {
    let graph = project_data.graphs.get(graph_path).ok_or_else(|| {
        ProjectError::InvalidProjectFormat(format!("graph '{}' not loaded", graph_path))
    })?;
    let local_variables =
        local_variables_for_graph(&project_data.variables, graph_path, graph.kind);
    let (dir, extension) = match graph.kind {
        GraphDocumentKind::Event => (EVENTS_DIR, EVENT_EXTENSION),
        GraphDocumentKind::Function => (FUNCTIONS_DIR, FUNCTION_EXTENSION),
    };
    let relative_path =
        graph_relative_path_for_save(root, dir, extension, &graph.name, graph_path)?;
    write_json(
        root.join(&relative_path).as_path(),
        &GraphDocument {
            schema_version: SCHEMA_VERSION,
            kind: graph.kind,
            name: graph.name.clone(),
            revision: graph.document.revision,
            document: graph.document.clone(),
            function: graph.function.clone(),
            local_variables,
        },
    )?;
    Ok(relative_path)
}

#[cfg(test)]
fn save_project_to_directory(project_data: &ProjectData, root: &Path) -> Result<(), ProjectError> {
    std::fs::create_dir_all(root)?;
    std::fs::create_dir_all(root.join(EVENTS_DIR))?;
    std::fs::create_dir_all(root.join(FUNCTIONS_DIR))?;
    ensure_worksheets_dir(root)?;
    std::fs::create_dir_all(root.join(DATABASE_DIR))?;

    let global_variables = project_data
        .variables
        .iter()
        .filter(|(_, variable)| matches!(variable.scope, VariableScope::Global))
        .map(|(id, variable)| (*id, variable.clone()))
        .collect();
    write_json(
        root.join(GLOBAL_VARIABLES_FILE).as_path(),
        &GlobalVariablesDocument {
            schema_version: SCHEMA_VERSION,
            variables: global_variables,
        },
    )?;

    // Reconcile on-disk graph files first so nested layouts are flattened before save.
    flatten_graph_layout(root)?;

    for graph_path in project_data.graphs.keys() {
        write_loaded_graph_document(project_data, root, graph_path)?;
    }
    for worksheet in project_data.worksheets.values() {
        let (relative_path, contents) = super::serialize_worksheet(worksheet)?;
        let target = root.join(relative_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(target, contents)?;
    }

    let mut manifest = read_project_manifest_from_root(root)?;
    manifest.project_name = project_data.metadata.project_name.clone();
    manifest.app_version = project_data.metadata.app_version.clone();
    manifest.export_time = project_data.metadata.export_time.clone();
    write_json(root.join(PROJECT_METADATA_FILE).as_path(), &manifest)?;
    Ok(())
}

/// 从文件加载项目
pub fn load_project_from_file(path: &str) -> Result<ProjectData, ProjectError> {
    let root = project_root_from_path(path);
    let manifest = read_project_manifest_from_root(root.as_path())?;
    let mut project_data = ProjectData::new();
    project_data.metadata.project_name = manifest.project_name;
    project_data.metadata.app_version = manifest.app_version;
    project_data.metadata.export_time = manifest.export_time;
    project_data.databases = discover_databases_from_root(root.as_path())?;
    project_data.worksheets = load_worksheets_from_root(root.as_path())?;

    let variables_path = root.join(GLOBAL_VARIABLES_FILE);
    if variables_path.exists() {
        let document: GlobalVariablesDocument = read_json(variables_path.as_path())?;
        project_data.variables.extend(document.variables);
    }

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
        GraphDocumentKind::Event,
        &graph_resources,
    )?);
    graphs.extend(read_graph_index_entries(
        root,
        FUNCTIONS_DIR,
        FUNCTION_EXTENSION,
        GraphDocumentKind::Function,
        &graph_resources,
    )?);
    let worksheets = read_worksheet_index_entries(root)?;
    let variables = read_variable_index_entries(root)?;

    Ok(ProjectIndex {
        project_instance_id: String::new(),
        publication_revision: 0,
        history: Default::default(),
        project_name: manifest.project_name,
        app_version: manifest.app_version,
        export_time: manifest.export_time,
        graphs,
        worksheets,
        variables,
    })
}

/// 轻量 Call 扫描结果：仅 node id + 目标函数 id（不构建 `GraphInstance`）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphCallSiteStub {
    pub node_id: NodeId,
    pub target_function_path: Option<String>,
}

/// 从图文件读取 Call Function 节点 stub（跳过 pins / connections / localVariables 物化）。
pub fn read_graph_call_sites_from_file(
    path: &Path,
) -> Result<Vec<GraphCallSiteStub>, ProjectError> {
    let scan: GraphCallSiteScanDocument = read_json(path)?;
    Ok(scan
        .document
        .nodes
        .into_values()
        .filter(|node| node.node_type.as_str() == "yssbi.project.function.call")
        .map(|node| GraphCallSiteStub {
            node_id: node.id,
            target_function_path: node
                .parameters
                .values()
                .find_map(|value| value.as_str())
                .and_then(|path| GraphResourcePath::new(path).ok())
                .map(|path| path.as_str().to_string()),
        })
        .collect())
}

/// 从项目磁盘读取某张图的 Call stub（图未加载时使用）。
pub fn read_graph_call_sites_from_project(
    project_path: &str,
    graph_path: &GraphResourcePath,
) -> Result<Vec<GraphCallSiteStub>, ProjectError> {
    let root = project_root_from_path(project_path);
    let graph_resources = load_graph_resource_index(root.as_path())?;
    let Some(resource) = graph_resources.get_by_path(graph_path.as_str()) else {
        return Ok(Vec::new());
    };
    read_graph_call_sites_from_file(root.join(resource.path.as_str()).as_path())
}

pub(crate) fn load_project_graph_document_from_file(
    path: &str,
    graph_path: &GraphResourcePath,
) -> Result<GraphDocument, ProjectError> {
    let root = project_root_from_path(path);
    let graph_resources = load_graph_resource_index(root.as_path())?;
    if let Some(resource) = graph_resources.get_by_path(graph_path.as_str()) {
        let document =
            read_graph_document(root.join(resource.path.as_str()).as_path(), resource.kind)?;
        return Ok(bind_graph_document_scope_by_path(
            document,
            resource.kind,
            resource.path.as_str(),
        ));
    }

    Err(ProjectError::InvalidProjectFormat(format!(
        "graph '{}' not found in project graph files",
        graph_path
    )))
}

pub fn load_project_graph_from_file(
    path: &str,
    graph_path: &GraphResourcePath,
) -> Result<super::GraphResourceDocument, ProjectError> {
    let document = load_project_graph_document_from_file(path, graph_path)?;
    let mut graph = document.document;
    graph.revision = document.revision;
    Ok(super::GraphResourceDocument {
        name: document.name,
        kind: document.kind,
        document: graph,
        function: document.function,
    })
}

fn read_project_manifest_from_root(root: &Path) -> Result<ProjectManifest, ProjectError> {
    let manifest_path = root.join(PROJECT_METADATA_FILE);
    if !manifest_path.exists() {
        let default_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_else(|| "Untitled".to_string());
        return Ok(ProjectManifest {
            schema_version: SCHEMA_VERSION,
            project_name: default_name,
            app_version: String::new(),
            export_time: String::new(),
        });
    }
    read_json(manifest_path.as_path())
}

fn load_graph_resource_index(root: &Path) -> Result<GraphResourceIndex, ProjectError> {
    scan_graph_resource_index(root)
}

pub fn project_root_from_path(path: &str) -> PathBuf {
    let path = PathBuf::from(path.trim());
    let is_metadata_file = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.eq_ignore_ascii_case(PROJECT_METADATA_FILE))
        .unwrap_or(false);
    if path.is_file() || is_metadata_file {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or(path)
    } else {
        path
    }
}

fn local_variables_for_graph(
    variables: &HashMap<VariableId, VariableInstance>,
    graph_path: &GraphResourcePath,
    graph_kind: GraphDocumentKind,
) -> HashMap<VariableId, VariableInstance> {
    let graph_path = graph_path.as_str();
    variables
        .iter()
        .filter(|(_, variable)| match (&variable.scope, graph_kind) {
            (VariableScope::Event { event_path }, GraphDocumentKind::Event) => {
                event_path == graph_path
            }
            (VariableScope::Function { function_path }, GraphDocumentKind::Function) => {
                function_path == graph_path
            }
            _ => false,
        })
        .map(|(id, variable)| (*id, variable.clone()))
        .collect()
}

fn read_graph_document(
    path: &Path,
    expected_kind: GraphDocumentKind,
) -> Result<GraphDocument, ProjectError> {
    let content = std::fs::read_to_string(path)?;
    let mut document: GraphDocument =
        serde_json::from_str(&content).map_err(ProjectError::Deserialize)?;
    if document.schema_version != SCHEMA_VERSION {
        return Err(ProjectError::InvalidProjectFormat(format!(
            "graph file '{}' uses unsupported schema version {}; expected {}",
            path.display(),
            document.schema_version,
            SCHEMA_VERSION
        )));
    }
    if document.kind != expected_kind {
        return Err(ProjectError::InvalidProjectFormat(format!(
            "graph file '{}' kind does not match manifest",
            path.display()
        )));
    }
    validate_function_shape(path, document.kind, document.function.as_ref())?;
    if let Some(name) = graph_name_from_file_path(path) {
        document.name = name;
    }
    Ok(document)
}

fn read_graph_document_for_resource(
    root: &Path,
    path: &Path,
    expected_kind: GraphDocumentKind,
) -> Result<GraphDocument, ProjectError> {
    let document = read_graph_document(path, expected_kind)?;
    let graph_resources = load_graph_resource_index(root)?;
    let relative_path = path_to_slash_string(
        path.strip_prefix(root)
            .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))?,
    );
    let resource = graph_resources.get_by_path(&relative_path).ok_or_else(|| {
        ProjectError::InvalidProjectFormat(format!(
            "graph resource '{}' not indexed",
            relative_path
        ))
    })?;
    Ok(bind_graph_document_scope_by_path(
        document,
        expected_kind,
        resource.path.as_str(),
    ))
}

fn bind_graph_document_scope_by_path(
    mut document: GraphDocument,
    kind: GraphDocumentKind,
    resource_path: &str,
) -> GraphDocument {
    let scope = scoped_variable_scope(kind, resource_path);
    for variable in document.local_variables.values_mut() {
        variable.scope = scope.clone();
    }
    document
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphCallSiteScanDocument {
    document: NodeGraphDocument,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphFileHeader {
    schema_version: u32,
    kind: GraphDocumentKind,
    name: String,
    #[serde(default)]
    revision: crate::node_system::document::ResourceRevision,
    function: Option<crate::node_system::document::FunctionDocument>,
}

fn read_graph_file_header(path: &Path) -> Result<GraphFileHeader, ProjectError> {
    let header: GraphFileHeader = read_json(path)?;
    validate_function_shape(path, header.kind, header.function.as_ref())?;
    Ok(header)
}

fn validate_function_shape(
    path: &Path,
    kind: GraphDocumentKind,
    function: Option<&crate::node_system::document::FunctionDocument>,
) -> Result<(), ProjectError> {
    match (kind, function) {
        (GraphDocumentKind::Function, None) => Err(ProjectError::InvalidProjectFormat(format!(
            "function graph file '{}' is missing its function document",
            path.display()
        ))),
        (GraphDocumentKind::Event, Some(_)) => Err(ProjectError::InvalidProjectFormat(format!(
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
    expected_kind: GraphDocumentKind,
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
        if header.schema_version != SCHEMA_VERSION {
            return Err(ProjectError::InvalidProjectFormat(format!(
                "graph file '{}' uses unsupported schema version {}; expected {}",
                path.display(),
                header.schema_version,
                SCHEMA_VERSION
            )));
        }
        if header.kind != expected_kind {
            return Err(ProjectError::InvalidProjectFormat(format!(
                "graph file '{}' kind does not match its resource directory",
                path.display()
            )));
        }
        let name = graph_name_from_file_path(path.as_path()).unwrap_or(header.name);
        let (function_revision, function_signature) = header
            .function
            .map(|function| (Some(function.revision), Some(function.signature)))
            .unwrap_or((None, None));
        entries.push(ProjectGraphIndexEntry {
            path: resource.path.as_str().to_string(),
            name,
            graph_type: expected_kind,
            revision: header.revision,
            function_revision,
            function_signature,
        });
    }
    Ok(entries)
}

fn scoped_variable_scope(kind: GraphDocumentKind, graph_path: &str) -> VariableScope {
    match kind {
        GraphDocumentKind::Event => VariableScope::Event {
            event_path: graph_path.to_string(),
        },
        GraphDocumentKind::Function => VariableScope::Function {
            function_path: graph_path.to_string(),
        },
    }
}

pub(crate) fn read_global_variable_index_entries(
    root: &Path,
) -> Result<Vec<ProjectVariableIndexEntry>, ProjectError> {
    let variables_path = root.join(GLOBAL_VARIABLES_FILE);
    if !variables_path.exists() {
        return Ok(Vec::new());
    }
    let document: GlobalVariablesDocument = read_json(variables_path.as_path())?;
    Ok(document
        .variables
        .into_values()
        .map(ProjectVariableIndexEntry::from)
        .collect())
}

fn read_graph_local_variable_index_entries(
    root: &Path,
    dir: &str,
    extension: &str,
    expected_kind: GraphDocumentKind,
) -> Result<Vec<ProjectVariableIndexEntry>, ProjectError> {
    let mut entries = Vec::new();
    for path in list_graph_files(root, dir, extension)? {
        let document = read_graph_document_for_resource(root, path.as_path(), expected_kind)?;
        let graph_name = graph_name_from_file_path(path.as_path()).unwrap_or(document.name);
        let owner_graph_path = path_to_slash_string(
            path.strip_prefix(root)
                .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))?,
        );
        for variable in document.local_variables.into_values() {
            entries.push(ProjectVariableIndexEntry {
                id: variable.id.to_string(),
                revision: crate::node_system::document::ResourceRevision::INITIAL,
                name: variable.name,
                data_type: variable.data_type,
                data_value: variable.data_value,
                description: variable.description,
                scope: variable.scope,
                tags: variable.tags,
                owner_graph_path: Some(owner_graph_path.clone()),
                owner_graph_name: Some(graph_name.clone()),
                owner_graph_kind: Some(expected_kind),
            });
        }
    }
    Ok(entries)
}

fn read_variable_index_entries(
    root: &Path,
) -> Result<Vec<ProjectVariableIndexEntry>, ProjectError> {
    let mut entries = read_global_variable_index_entries(root)?;
    entries.extend(read_graph_local_variable_index_entries(
        root,
        EVENTS_DIR,
        EVENT_EXTENSION,
        GraphDocumentKind::Event,
    )?);
    entries.extend(read_graph_local_variable_index_entries(
        root,
        FUNCTIONS_DIR,
        FUNCTION_EXTENSION,
        GraphDocumentKind::Function,
    )?);
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

#[cfg(test)]
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

#[cfg(test)]
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
    if let Some(resource) = graph_resources.get_by_path(graph_path.as_str()) {
        if resource.path.as_str().starts_with(&format!("{dir}/")) {
            return Ok(Some(root.join(resource.path.as_str())));
        }
    }
    Ok(None)
}

pub(crate) fn find_graph_document_path(
    root: &Path,
    graph_path: &GraphResourcePath,
) -> Result<Option<(PathBuf, GraphDocumentKind, GraphDocument)>, ProjectError> {
    if let Some(resource) = load_graph_resource_index(root)?.get_by_path(graph_path.as_str()) {
        let path = root.join(resource.path.as_str());
        let document = bind_graph_document_scope_by_path(
            read_graph_document(path.as_path(), resource.kind)?,
            resource.kind,
            resource.path.as_str(),
        );
        return Ok(Some((path, resource.kind, document)));
    }
    Ok(None)
}

#[cfg(test)]
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

#[cfg(test)]
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

/// Test-fixture helper for exercising legacy nested graph layouts.
#[cfg(test)]
pub fn flatten_graph_layout(root: &Path) -> Result<bool, ProjectError> {
    let mut changed = false;
    changed |= flatten_kind_graph_layout(root, EVENTS_DIR, EVENT_EXTENSION)?;
    changed |= flatten_kind_graph_layout(root, FUNCTIONS_DIR, FUNCTION_EXTENSION)?;
    if changed {
        remove_empty_graph_subdirs(&root.join(EVENTS_DIR))?;
        remove_empty_graph_subdirs(&root.join(FUNCTIONS_DIR))?;
    }
    Ok(changed)
}

#[cfg(test)]
fn flatten_kind_graph_layout(
    root: &Path,
    dir: &str,
    extension: &str,
) -> Result<bool, ProjectError> {
    let graph_dir = root.join(dir);
    if !graph_dir.is_dir() {
        return Ok(false);
    }

    let mut nested_paths = Vec::new();
    collect_nested_graph_files(&graph_dir, extension, &mut nested_paths)?;
    if nested_paths.is_empty() {
        return Ok(false);
    }

    let mut changed = false;
    for nested_path in nested_paths {
        let graph_name = read_graph_file_header(nested_path.as_path())
            .ok()
            .map(|header| header.name)
            .or_else(|| graph_name_from_file_path(nested_path.as_path()))
            .unwrap_or_else(|| "Untitled".to_string());
        let file_name = unique_graph_file_name(graph_dir.as_path(), &graph_name, extension, None);
        let target_path = graph_dir.join(&file_name);
        if nested_path == target_path {
            continue;
        }

        std::fs::rename(&nested_path, &target_path)?;
        changed = true;
    }

    Ok(changed)
}

#[cfg(test)]
fn collect_nested_graph_files(
    graph_dir: &Path,
    extension: &str,
    nested_paths: &mut Vec<PathBuf>,
) -> Result<(), ProjectError> {
    for entry in std::fs::read_dir(graph_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_all_graph_files(path.as_path(), extension, nested_paths)?;
        }
    }
    Ok(())
}

#[cfg(test)]
fn collect_all_graph_files(
    dir: &Path,
    extension: &str,
    paths: &mut Vec<PathBuf>,
) -> Result<(), ProjectError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_all_graph_files(path.as_path(), extension, paths)?;
        } else if path.is_file()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case(extension))
                .unwrap_or(false)
        {
            paths.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
fn remove_empty_graph_subdirs(dir: &Path) -> Result<(), ProjectError> {
    if !dir.is_dir() {
        return Ok(());
    }
    let entries: Vec<_> = std::fs::read_dir(dir)?.collect();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            remove_empty_graph_subdirs(&path)?;
            if std::fs::read_dir(&path)?.next().is_none() {
                std::fs::remove_dir(path)?;
            }
        }
    }
    Ok(())
}

fn path_to_slash_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ProjectError> {
    let content = std::fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(ProjectError::Deserialize)
}

#[cfg(test)]
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
    let tables = crate::database::list_data_tables(&duckdb_path).map_err(|e| {
        ProjectError::InvalidProjectFormat(format!("Failed to list DuckDB tables: {e}"))
    })?;

    let relative_path = relative_project_duckdb_path();
    for table in tables {
        let display_name = crate::database::read_display_name(&duckdb_path, &table)
            .unwrap_or_else(|| table.clone());
        let decl = DatabaseDecl {
            id: table.clone(),
            engine: DatabaseEngine::DuckDb {
                path: relative_path.clone(),
                table: table.clone(),
            },
            schema_version: SCHEMA_VERSION,
            required: false,
            name: Some(display_name),
        };
        map.insert(table, decl);
    }

    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::document::{
        DocumentConnection, DocumentNode, GraphDocument as NodeGraphDocument, NodeId, NodePosition,
        ParameterValues, PortAddress,
    };
    use crate::node_system::protocol::{NodeTypeId, PortKey};
    use crate::project::GraphResourceDocument;

    fn temp_project_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("yssbi-production-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn normalized_graph() -> GraphResourceDocument {
        let source = NodeId::new();
        let target = NodeId::new();
        let node = |id| DocumentNode {
            id,
            node_type: NodeTypeId::new("yssbi.constant.int64").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        };
        let mut document = NodeGraphDocument::default();
        document.nodes.insert(source, node(source));
        document.nodes.insert(target, node(target));
        let connection = crate::node_system::document::ConnectionId::new();
        document.connections.insert(
            connection,
            DocumentConnection {
                id: connection,
                output: PortAddress::declared(source, PortKey::new("value").unwrap()),
                input: PortAddress::declared(target, PortKey::new("value").unwrap()),
                order: None,
            },
        );
        GraphResourceDocument {
            name: "RoundTrip".into(),
            kind: GraphDocumentKind::Event,
            document,
            function: None,
        }
    }

    #[test]
    fn production_graph_io_round_trips_normalized_document_without_fixed_ports() {
        let root = temp_project_dir();
        let graph_path = GraphResourcePath::new("events/RoundTrip.yssbi-event").unwrap();
        let graph = normalized_graph();
        let mut project = ProjectData::new();
        project.graphs.insert(graph_path.clone(), graph.clone());

        initialize_project_directory(&project, root.as_path()).unwrap();
        let graph_file = root.join(graph_path.as_str());
        let value: serde_json::Value = read_json(&graph_file).unwrap();
        let serialized = serde_json::to_string(&value).unwrap();
        assert_eq!(value["schemaVersion"], serde_json::json!(SCHEMA_VERSION));
        assert!(value.get("document").is_some());
        assert!(value.get("graph").is_none());
        assert!(!serialized.contains("pins"));
        assert!(!serialized.contains("pinIds"));

        let loaded =
            load_project_graph_from_file(root.to_string_lossy().as_ref(), &graph_path).unwrap();
        assert_eq!(loaded, graph);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn production_project_index_rejects_legacy_graph_schema() {
        let root = temp_project_dir();
        let graph_path = GraphResourcePath::new("events/RoundTrip.yssbi-event").unwrap();
        let mut project = ProjectData::new();
        project
            .graphs
            .insert(graph_path.clone(), normalized_graph());
        initialize_project_directory(&project, root.as_path()).unwrap();
        let graph_file = root.join(graph_path.as_str());
        let mut value: serde_json::Value = read_json(&graph_file).unwrap();
        value["schemaVersion"] = serde_json::json!(1);
        write_json(&graph_file, &value).unwrap();

        let error = read_project_index(root.to_string_lossy().as_ref()).unwrap_err();
        assert!(error.to_string().contains("unsupported schema version 1"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn production_graph_io_rejects_function_shape_mismatches() {
        let root = temp_project_dir();
        let function_path = GraphResourcePath::new("functions/Strict.yssbi-function").unwrap();
        let event_path = GraphResourcePath::new("events/Strict.yssbi-event").unwrap();
        let mut project = ProjectData::new();
        project.graphs.insert(
            function_path.clone(),
            GraphResourceDocument::new("Strict", GraphDocumentKind::Function),
        );
        let mut event = normalized_graph();
        event.name = "Strict".into();
        project.graphs.insert(event_path.clone(), event);
        initialize_project_directory(&project, root.as_path()).unwrap();

        let function_file = root.join(function_path.as_str());
        let mut function_value: serde_json::Value = read_json(&function_file).unwrap();
        function_value.as_object_mut().unwrap().remove("function");
        write_json(&function_file, &function_value).unwrap();
        let missing = load_project_graph_from_file(root.to_string_lossy().as_ref(), &function_path)
            .unwrap_err();
        assert!(missing.to_string().contains("function"));

        let event_file = root.join(event_path.as_str());
        let mut event_value: serde_json::Value = read_json(&event_file).unwrap();
        event_value["function"] = serde_json::to_value(
            crate::node_system::document::FunctionDocument::new(Default::default()),
        )
        .unwrap();
        write_json(&event_file, &event_value).unwrap();
        let unexpected =
            load_project_graph_from_file(root.to_string_lossy().as_ref(), &event_path).unwrap_err();
        assert!(unexpected.to_string().contains("event"));
        assert!(unexpected.to_string().contains("function"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn production_graph_io_rejects_legacy_schema() {
        let root = temp_project_dir();
        let graph_path = GraphResourcePath::new("events/RoundTrip.yssbi-event").unwrap();
        let mut project = ProjectData::new();
        project
            .graphs
            .insert(graph_path.clone(), normalized_graph());
        initialize_project_directory(&project, root.as_path()).unwrap();
        let graph_file = root.join(graph_path.as_str());
        let mut value: serde_json::Value = read_json(&graph_file).unwrap();
        value["schemaVersion"] = serde_json::json!(1);
        write_json(&graph_file, &value).unwrap();

        let error =
            load_project_graph_from_file(root.to_string_lossy().as_ref(), &graph_path).unwrap_err();
        assert!(error.to_string().contains("unsupported schema version 1"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
