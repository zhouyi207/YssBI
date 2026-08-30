use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    GraphResourceIndex, GraphResourcePath, ProjectError, ProjectWorksheetIndexEntry,
    load_worksheets_from_root, read_worksheet_index_entries, scan_graph_resource_index,
};
use yss_database_contract::{DatabaseDecl, DatabaseEngine, DatabaseId};
use yss_function_editor_projection::FunctionEditorProjection;
use yss_graph_document::{GraphDocument as NodeGraphDocument, GraphResourceKind};
use yss_project_filesystem::project_root_from_path;
use yss_project_identity::ProjectResourcePath;
#[cfg(test)]
use yss_project_layout::PROJECT_CONTENT_DIRECTORIES;
use yss_project_layout::{
    DATABASE_DIR, EVENT_EXTENSION, EVENTS_DIR, FUNCTION_EXTENSION, FUNCTIONS_DIR,
    GLOBAL_VARIABLES_FILE, PROJECT_DUCKDB_FILE, PROJECT_METADATA_FILE,
};
use yss_project_manifest::{
    CURRENT_PROJECT_SCHEMA_VERSION, ProjectManifest, deserialize_current_project_schema_version,
};
use yss_project_model::{GraphResourceDocument, ProjectData};
use yss_variable_contract::{VariableId, VariableInstance, VariableScope};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalVariablesDocument {
    #[serde(deserialize_with = "deserialize_current_project_schema_version")]
    pub schema_version: u32,
    pub variables: HashMap<VariableId, VariableInstance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDocument {
    pub schema_version: u32,
    pub kind: GraphResourceKind,
    pub name: String,
    #[serde(default)]
    pub revision: yss_project_identity::ResourceRevision,
    pub document: NodeGraphDocument,
    pub function: Option<yss_project_history::FunctionDocument>,
    pub local_variables: HashMap<VariableId, VariableInstance>,
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
pub struct ProjectVariableIndexEntry {
    pub id: String,
    pub resource_path: ProjectResourcePath,
    pub revision: yss_project_identity::ResourceRevision,
    pub name: String,
    pub data_type: yss_data_contract::DataType,
    pub data_value: yss_data_contract::DataValue,
    pub description: String,
    pub scope: VariableScope,
    pub tags: Vec<String>,
    pub owner_graph_path: Option<String>,
    pub owner_graph_name: Option<String>,
    #[serde(rename = "ownerGraphKind", skip_serializing_if = "Option::is_none")]
    pub owner_graph_kind: Option<GraphResourceKind>,
}

impl From<VariableInstance> for ProjectVariableIndexEntry {
    fn from(value: VariableInstance) -> Self {
        Self {
            id: value.id.to_string(),
            resource_path: ProjectResourcePath::new(format!("variables/{}", value.id)),
            revision: yss_project_identity::ResourceRevision::INITIAL,
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
    pub history: yss_project_history::HistoryStatusDto,
    pub project_name: String,
    pub export_time: String,
    pub graphs: Vec<ProjectGraphIndexEntry>,
    #[serde(default)]
    pub worksheets: Vec<ProjectWorksheetIndexEntry>,
    #[serde(default)]
    pub variables: Vec<ProjectVariableIndexEntry>,
    #[serde(default)]
    pub databases: Vec<ProjectDatabaseIndexEntry>,
}

pub fn serialize_project_manifest(data: &ProjectData) -> Result<Vec<u8>, ProjectError> {
    serde_json::to_vec_pretty(&project_manifest_from_data(data)?).map_err(ProjectError::Serialize)
}

fn project_manifest_from_data(data: &ProjectData) -> Result<ProjectManifest, ProjectError> {
    ProjectManifest::try_new(
        data.metadata.project_name.clone(),
        data.metadata.export_time.clone(),
        data.computation_settings.clone(),
    )
    .map_err(|error| {
        ProjectError::InvalidProjectFormat(format!(
            "project computation settings are invalid: {error}"
        ))
    })
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
        yss_variable_contract::VariableId,
        yss_variable_contract::VariableInstance,
    >,
) -> Result<Vec<u8>, ProjectError> {
    serde_json::to_vec_pretty(&GlobalVariablesDocument {
        schema_version: CURRENT_PROJECT_SCHEMA_VERSION,
        variables,
    })
    .map_err(ProjectError::Serialize)
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
) -> Result<GraphDocument, ProjectError> {
    let graph = data.graphs.get(graph_path).ok_or_else(|| {
        ProjectError::InvalidProjectFormat(format!("graph '{}' not loaded", graph_path))
    })?;
    let local_variables = local_variables_for_graph(&data.variables, graph_path, graph.kind);
    Ok(graph_document_from_resource(graph, local_variables))
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
    serde_json::to_vec_pretty(&graph_document_from_resource(graph, local_variables))
        .map_err(ProjectError::Serialize)
}

fn graph_document_from_resource(
    graph: &GraphResourceDocument,
    local_variables: HashMap<VariableId, VariableInstance>,
) -> GraphDocument {
    GraphDocument {
        schema_version: CURRENT_PROJECT_SCHEMA_VERSION,
        kind: graph.kind,
        name: graph.name.clone(),
        revision: yss_project_identity::ResourceRevision::from_graph_revision(
            graph.document.revision,
        ),
        document: graph.document.clone(),
        function: graph.function.clone(),
        local_variables,
    }
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
        GraphResourceKind::Event => (EVENTS_DIR, EVENT_EXTENSION),
        GraphResourceKind::Function => (FUNCTIONS_DIR, FUNCTION_EXTENSION),
    };
    let relative_path =
        graph_relative_path_for_save(root, dir, extension, &graph.name, graph_path)?;
    write_json(
        root.join(&relative_path).as_path(),
        &GraphDocument {
            schema_version: CURRENT_PROJECT_SCHEMA_VERSION,
            kind: graph.kind,
            name: graph.name.clone(),
            revision: yss_project_identity::ResourceRevision::from_graph_revision(
                graph.document.revision,
            ),
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
    for directory in PROJECT_CONTENT_DIRECTORIES {
        std::fs::create_dir_all(root.join(directory))?;
    }

    scan_graph_resource_index(root)?;

    let global_variables = project_data
        .variables
        .iter()
        .filter(|(_, variable)| matches!(variable.scope, VariableScope::Global))
        .map(|(id, variable)| (*id, variable.clone()))
        .collect();
    write_json(
        root.join(GLOBAL_VARIABLES_FILE).as_path(),
        &GlobalVariablesDocument {
            schema_version: CURRENT_PROJECT_SCHEMA_VERSION,
            variables: global_variables,
        },
    )?;

    for graph_path in project_data.graphs.keys() {
        write_loaded_graph_document(project_data, root, graph_path)?;
    }
    for (worksheet_path, worksheet) in &project_data.worksheets {
        let (relative_path, contents) = super::serialize_worksheet(worksheet_path, worksheet)?;
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
    let (project_name, export_time, computation_settings) = manifest.into_parts();
    let mut project_data = ProjectData::new();
    project_data.metadata.project_name = project_name;
    project_data.metadata.export_time = export_time;
    project_data.computation_settings = computation_settings;
    project_data.databases = discover_databases_from_root(root.as_path())?;
    project_data.worksheets = load_worksheets_from_root(root.as_path())?;

    let variables_path = root.join(GLOBAL_VARIABLES_FILE);
    if variables_path.exists() {
        let contents = std::fs::read(&variables_path)?;
        let document = parse_global_variables_document(&contents)?;
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
    let worksheets = read_worksheet_index_entries(root)?;
    let variables = read_variable_index_entries(root)?;

    let (project_name, export_time, _) = manifest.into_parts();
    Ok(ProjectIndex {
        project_instance_id: String::new(),
        publication_revision: 0,
        authority_generation: 0,
        history: Default::default(),
        project_name,
        export_time,
        graphs,
        worksheets,
        variables,
        databases: Vec::new(),
    })
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
) -> Result<yss_project_model::GraphResourceDocument, ProjectError> {
    let document = load_project_graph_document_from_file(path, graph_path)?;
    let mut graph = document.document;
    graph.revision = document.revision.to_graph_revision();
    Ok(yss_project_model::GraphResourceDocument {
        name: document.name,
        kind: document.kind,
        document: graph,
        function: document.function,
    })
}

fn read_project_manifest_from_root(root: &Path) -> Result<ProjectManifest, ProjectError> {
    read_json(root.join(PROJECT_METADATA_FILE).as_path())
}

fn load_graph_resource_index(root: &Path) -> Result<GraphResourceIndex, ProjectError> {
    scan_graph_resource_index(root)
}

fn local_variables_for_graph(
    variables: &HashMap<VariableId, VariableInstance>,
    graph_path: &GraphResourcePath,
    graph_kind: GraphResourceKind,
) -> HashMap<VariableId, VariableInstance> {
    let graph_path = graph_path.as_str();
    variables
        .iter()
        .filter(|(_, variable)| match (&variable.scope, graph_kind) {
            (VariableScope::Event { event_path }, GraphResourceKind::Event) => {
                event_path == graph_path
            }
            (VariableScope::Function { function_path }, GraphResourceKind::Function) => {
                function_path == graph_path
            }
            _ => false,
        })
        .map(|(id, variable)| (*id, variable.clone()))
        .collect()
}

pub(crate) fn parse_global_variables_document(
    contents: &[u8],
) -> Result<GlobalVariablesDocument, ProjectError> {
    serde_json::from_slice(contents).map_err(ProjectError::Deserialize)
}

pub(crate) fn parse_graph_resource_document(
    contents: &[u8],
    path: &Path,
    expected_kind: GraphResourceKind,
) -> Result<GraphDocument, ProjectError> {
    let document: GraphDocument =
        serde_json::from_slice(contents).map_err(ProjectError::Deserialize)?;
    if document.schema_version != CURRENT_PROJECT_SCHEMA_VERSION {
        return Err(ProjectError::InvalidProjectFormat(format!(
            "graph file '{}' uses unsupported schema version {}; expected {}",
            path.display(),
            document.schema_version,
            CURRENT_PROJECT_SCHEMA_VERSION
        )));
    }
    if document.kind != expected_kind {
        return Err(ProjectError::InvalidProjectFormat(format!(
            "graph file '{}' kind does not match manifest",
            path.display()
        )));
    }
    validate_function_shape(path, document.kind, document.function.as_ref())?;
    Ok(document)
}

fn read_graph_document(
    path: &Path,
    expected_kind: GraphResourceKind,
) -> Result<GraphDocument, ProjectError> {
    let contents = std::fs::read(path)?;
    let mut document = parse_graph_resource_document(&contents, path, expected_kind)?;
    if let Some(name) = graph_name_from_file_path(path) {
        document.name = name;
    }
    Ok(document)
}

fn read_graph_document_for_resource(
    root: &Path,
    path: &Path,
    expected_kind: GraphResourceKind,
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
    kind: GraphResourceKind,
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
struct GraphFileHeader {
    schema_version: u32,
    kind: GraphResourceKind,
    name: String,
    #[serde(default)]
    revision: yss_project_identity::ResourceRevision,
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
        if header.schema_version != CURRENT_PROJECT_SCHEMA_VERSION {
            return Err(ProjectError::InvalidProjectFormat(format!(
                "graph file '{}' uses unsupported schema version {}; expected {}",
                path.display(),
                header.schema_version,
                CURRENT_PROJECT_SCHEMA_VERSION
            )));
        }
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
            revision: header.revision,
            function_revision,
            function_signature,
            function_editor_projection,
        });
    }
    Ok(entries)
}

fn scoped_variable_scope(kind: GraphResourceKind, graph_path: &str) -> VariableScope {
    match kind {
        GraphResourceKind::Event => VariableScope::Event {
            event_path: graph_path.to_string(),
        },
        GraphResourceKind::Function => VariableScope::Function {
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
    expected_kind: GraphResourceKind,
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
                resource_path: ProjectResourcePath::new(format!("variables/{}", variable.id)),
                revision: yss_project_identity::ResourceRevision::INITIAL,
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
        GraphResourceKind::Event,
    )?);
    entries.extend(read_graph_local_variable_index_entries(
        root,
        FUNCTIONS_DIR,
        FUNCTION_EXTENSION,
        GraphResourceKind::Function,
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
) -> Result<Option<(PathBuf, GraphResourceKind, GraphDocument)>, ProjectError> {
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
    use super::{ProjectError, ProjectManifest, serialize_project_manifest};
    use serde_json::json;
    use yss_computation_settings::ProjectComputationSettings;
    use yss_project_model::ProjectData;

    #[test]
    fn project_manifest_serialization_uses_the_canonical_validated_contract() {
        let mut data = ProjectData::new();
        data.metadata.project_name = "Canonical Manifest".into();
        data.metadata.export_time = "2026-08-30T00:00:00Z".into();

        let contents = serialize_project_manifest(&data).unwrap();
        let manifest: ProjectManifest = serde_json::from_slice(&contents).unwrap();

        assert_eq!(manifest.project_name(), "Canonical Manifest");
        assert_eq!(manifest.export_time(), "2026-08-30T00:00:00Z");
        assert_eq!(manifest.computation_settings(), &data.computation_settings);
    }

    #[test]
    fn project_manifest_serialization_rejects_invalid_internal_settings() {
        let mut data = ProjectData::new();
        data.computation_settings = serde_json::from_value::<ProjectComputationSettings>(json!({
            "numeric": { "tolerance": { "absolute": 0.0, "relative": 0.0 } },
            "missingValues": { "statistics": "listwise" }
        }))
        .unwrap();

        let error = serialize_project_manifest(&data).unwrap_err();
        assert!(matches!(error, ProjectError::InvalidProjectFormat(_)));
    }
}

#[cfg(all(test, any()))]
mod tests {
    use super::*;
    use crate::graph::document::EffectiveInputBinding;
    use crate::project::NumericTolerance;
    use serde_json::json;
    use yss_graph_document::{
        ConnectionId, DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
        FunctionParameterId, GraphDocument as NodeGraphDocument, InputState, NodeId, NodePosition,
        OrderKey, ParameterValues, PortAddress, PortInstanceId,
    };
    use yss_graph_protocol::{NodeTypeId, PortKey};
    use yss_project_model::GraphResourceDocument;

    fn temp_project_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("yssbi-production-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn top_level_keys(value: &serde_json::Value) -> std::collections::BTreeSet<&str> {
        value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect()
    }

    #[test]
    fn project_load_requires_manifest() {
        let root = temp_project_dir();

        let error = read_project_index(root.to_string_lossy().as_ref()).unwrap_err();

        assert!(
            matches!(error, ProjectError::Io(ref source) if source.kind() == std::io::ErrorKind::NotFound)
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_manifest_rejects_unsupported_schema_version() {
        let root = temp_project_dir();
        initialize_project_directory(&ProjectData::new(), root.as_path()).unwrap();
        let manifest_path = root.join(PROJECT_METADATA_FILE);
        let mut value: serde_json::Value = read_json(&manifest_path).unwrap();
        value["schemaVersion"] = json!(CURRENT_PROJECT_SCHEMA_VERSION - 1);
        write_json(&manifest_path, &value).unwrap();

        let error = read_project_index(root.to_string_lossy().as_ref()).unwrap_err();

        assert!(matches!(
            error,
            ProjectError::Deserialize(source)
                if source.to_string().contains("unsupported schema version 2")
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn global_variables_reject_unsupported_schema_version() {
        let mut value: serde_json::Value =
            serde_json::from_slice(&serialize_global_variables(&ProjectData::new()).unwrap())
                .unwrap();
        value["schemaVersion"] = json!(CURRENT_PROJECT_SCHEMA_VERSION + 1);
        let contents = serde_json::to_vec(&value).unwrap();

        let error = parse_global_variables_document(&contents).unwrap_err();

        assert!(matches!(
            error,
            ProjectError::Deserialize(source)
                if source.to_string().contains("unsupported schema version 4")
        ));
    }

    #[test]
    fn project_manifest_omits_application_version() {
        let manifest = serialize_project_manifest(&ProjectData::new()).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&manifest).unwrap();

        assert_eq!(
            top_level_keys(&value),
            std::collections::BTreeSet::from([
                "schemaVersion",
                "projectName",
                "exportTime",
                "computationSettings",
            ])
        );
    }

    #[test]
    fn project_index_omits_application_version() {
        let root = temp_project_dir();
        initialize_project_directory(&ProjectData::new(), root.as_path()).unwrap();
        let index = read_project_index(root.to_string_lossy().as_ref()).unwrap();
        let value = serde_json::to_value(index).unwrap();

        assert_eq!(
            top_level_keys(&value),
            std::collections::BTreeSet::from([
                "projectInstanceId",
                "publicationRevision",
                "history",
                "projectName",
                "exportTime",
                "graphs",
                "worksheets",
                "variables",
                "databases",
            ])
        );
        std::fs::remove_dir_all(root).unwrap();
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
        let connection = yss_graph_document::ConnectionId::new();
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
            kind: GraphResourceKind::Event,
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
        assert_eq!(
            value["schemaVersion"],
            serde_json::json!(CURRENT_PROJECT_SCHEMA_VERSION)
        );
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
    fn production_project_index_rejects_unsupported_graph_schema() {
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
    fn production_graph_io_rejects_structurally_invalid_document() {
        let root = temp_project_dir();
        let graph_path = GraphResourcePath::new("events/Invalid.yssbi-event").unwrap();
        let mut project = ProjectData::new();
        let mut graph = normalized_graph();
        graph.name = "Invalid".into();
        project.graphs.insert(graph_path.clone(), graph);
        initialize_project_directory(&project, root.as_path()).unwrap();

        let graph_file = root.join(graph_path.as_str());
        let mut envelope: GraphDocument = read_json(&graph_file).unwrap();
        let missing_node_id = NodeId::from_uuid(uuid::Uuid::from_u128(0x100));
        let connection_id =
            yss_graph_document::ConnectionId::from_uuid(uuid::Uuid::from_u128(0x101));
        let existing_node_id = *envelope.document.nodes.keys().next().unwrap();
        envelope.document.connections.insert(
            connection_id,
            DocumentConnection {
                id: connection_id,
                output: PortAddress::declared(missing_node_id, PortKey::new("value").unwrap()),
                input: PortAddress::declared(existing_node_id, PortKey::new("value").unwrap()),
                order: None,
            },
        );
        write_json(&graph_file, &envelope).unwrap();

        let error =
            load_project_graph_from_file(root.to_string_lossy().as_ref(), &graph_path).unwrap_err();
        assert!(matches!(
            &error,
            ProjectError::InvalidGraphDocument { path, source }
                if path == &graph_file
                    && source
                        == &yss_graph_document_edit::DocumentError::EndpointNodeNotFound(
                            missing_node_id,
                        )
        ));
        let source = std::error::Error::source(&error)
            .and_then(|source| source.downcast_ref::<yss_graph_document_edit::DocumentError>());
        assert_eq!(
            source,
            Some(&yss_graph_document_edit::DocumentError::EndpointNodeNotFound(missing_node_id,))
        );
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
            GraphResourceDocument::new("Strict", GraphResourceKind::Function),
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
        event_value["function"] = serde_json::to_value(yss_project_history::FunctionDocument::new(
            Default::default(),
        ))
        .unwrap();
        write_json(&event_file, &event_value).unwrap();
        let unexpected =
            load_project_graph_from_file(root.to_string_lossy().as_ref(), &event_path).unwrap_err();
        assert!(unexpected.to_string().contains("event"));
        assert!(unexpected.to_string().contains("function"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn production_graph_io_rejects_unsupported_schema() {
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

    #[test]
    fn production_graph_io_preserves_input_precedence_and_connection_order() {
        let root = temp_project_dir();
        let graph_path = GraphResourcePath::new("events/Precedence.yssbi-event").unwrap();
        let first_source = NodeId::from_uuid(uuid::Uuid::from_u128(0x301));
        let second_source = NodeId::from_uuid(uuid::Uuid::from_u128(0x302));
        let target = NodeId::from_uuid(uuid::Uuid::from_u128(0x303));
        let later_connection = ConnectionId::from_uuid(uuid::Uuid::from_u128(0x304));
        let earlier_connection = ConnectionId::from_uuid(uuid::Uuid::from_u128(0x305));
        let input = PortAddress::declared(target, PortKey::new("value").unwrap());
        let node = |id| DocumentNode {
            id,
            node_type: NodeTypeId::new("yssbi.constant.int64").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        };
        let mut document = NodeGraphDocument::default();
        for id in [first_source, second_source, target] {
            document.nodes.insert(id, node(id));
        }
        document.input_states.insert(
            input.clone(),
            InputState {
                literal_override: Some(serde_json::json!(41)),
            },
        );
        document.connections.insert(
            later_connection,
            DocumentConnection {
                id: later_connection,
                output: PortAddress::declared(first_source, PortKey::new("value").unwrap()),
                input: input.clone(),
                order: Some(OrderKey::new("rank-b")),
            },
        );
        document.connections.insert(
            earlier_connection,
            DocumentConnection {
                id: earlier_connection,
                output: PortAddress::declared(second_source, PortKey::new("value").unwrap()),
                input: input.clone(),
                order: Some(OrderKey::new("rank-a")),
            },
        );
        let graph = GraphResourceDocument {
            name: "Precedence".into(),
            kind: GraphResourceKind::Event,
            document,
            function: None,
        };

        assert_eq!(
            graph.document.effective_input_binding(&input, None),
            EffectiveInputBinding::Connections(vec![earlier_connection, later_connection])
        );
        assert_eq!(
            graph.document.input_states.get(&input),
            Some(&InputState {
                literal_override: Some(serde_json::json!(41)),
            })
        );

        let mut project = ProjectData::new();
        project.graphs.insert(graph_path.clone(), graph);
        initialize_project_directory(&project, root.as_path()).unwrap();
        let mut loaded =
            load_project_graph_from_file(root.to_string_lossy().as_ref(), &graph_path).unwrap();

        assert_eq!(
            loaded.document.effective_input_binding(&input, None),
            EffectiveInputBinding::Connections(vec![earlier_connection, later_connection])
        );
        assert_eq!(
            loaded.document.connections[&earlier_connection].order,
            Some(OrderKey::new("rank-a"))
        );
        assert_eq!(
            loaded.document.connections[&later_connection].order,
            Some(OrderKey::new("rank-b"))
        );
        assert_eq!(
            loaded.document.input_states.get(&input),
            Some(&InputState {
                literal_override: Some(serde_json::json!(41)),
            })
        );

        loaded.document.disconnect(earlier_connection).unwrap();
        loaded.document.disconnect(later_connection).unwrap();
        assert_eq!(
            loaded.document.effective_input_binding(&input, None),
            EffectiveInputBinding::Literal(serde_json::json!(41))
        );

        loaded.document.set_literal(input.clone(), None).unwrap();
        assert_eq!(
            loaded
                .document
                .effective_input_binding(&input, Some(serde_json::json!(7))),
            EffectiveInputBinding::ProtocolDefault(serde_json::json!(7))
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    fn reject_unstable_metadata_keys(value: &serde_json::Value) {
        const FORBIDDEN_KEYS: &[&str] = &[
            "displayName",
            "displayLabel",
            "categoryTitle",
            "localizedLabel",
            "projection",
            "projectionBasis",
            "sourceRevision",
            "registryFingerprint",
            "registrySnapshot",
            "snapshotHandle",
            "resourceVersions",
            "compilerValueRef",
            "planValueSource",
        ];

        match value {
            serde_json::Value::Object(entries) => {
                for (key, child) in entries {
                    assert!(
                        !FORBIDDEN_KEYS.contains(&key.as_str()),
                        "persisted graph contains unstable metadata key '{key}'"
                    );
                    reject_unstable_metadata_keys(child);
                }
            }
            serde_json::Value::Array(entries) => {
                for child in entries {
                    reject_unstable_metadata_keys(child);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn persisted_graph_json_contains_only_stable_document_metadata() {
        let node_id = NodeId::from_uuid(uuid::Uuid::from_u128(0x401));
        let port_instance_id = PortInstanceId::from_uuid(uuid::Uuid::from_u128(0x402));
        let connection_id = ConnectionId::from_uuid(uuid::Uuid::from_u128(0x403));
        let node_type = NodeTypeId::new("yssbi.constant.int64").unwrap();
        let dynamic_address = PortAddress::instance(
            node_id,
            PortKey::new("dynamic_value").unwrap(),
            port_instance_id,
        );
        let locator = DynamicMemberLocator::FunctionParameter {
            function: yss_graph_document::GraphResourcePath::new("functions/stable.yssbi-function")
                .unwrap(),
            parameter: FunctionParameterId::new("stable-parameter"),
        };
        let mut document = NodeGraphDocument::default();
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: node_type.clone(),
                position: NodePosition { x: 1.0, y: 2.0 },
                parameters: ParameterValues::new(),
                user_label: Some("User-authored label".into()),
            },
        );
        document.port_bindings.insert(
            dynamic_address.clone(),
            DynamicPortBinding::Resolved {
                origin: locator.clone(),
                order: OrderKey::new("stable-order"),
                last_known: yss_graph_document::LastKnownPortMetadata::default(),
            },
        );
        document.connections.insert(
            connection_id,
            DocumentConnection {
                id: connection_id,
                output: PortAddress::declared(node_id, PortKey::new("value").unwrap()),
                input: dynamic_address,
                order: Some(OrderKey::new("connection-order")),
            },
        );
        let graph = GraphResourceDocument {
            name: "Stable".into(),
            kind: GraphResourceKind::Event,
            document,
            function: None,
        };
        let variable_id = VariableId::nil();
        let local_variables = HashMap::from([(
            variable_id,
            VariableInstance {
                id: variable_id,
                name: "Stable local".into(),
                data_type: yss_data_contract::DataType::Int64,
                data_value: yss_data_contract::DataValue::Int64(7),
                tabular: None,
                description: String::new(),
                scope: VariableScope::Event {
                    event_path: "events/Stable.yssbi-event".into(),
                },
                tags: Vec::new(),
            },
        )]);

        let first = serialize_graph_resource_document(&graph, local_variables.clone()).unwrap();
        let second = serialize_graph_resource_document(&graph, local_variables).unwrap();
        assert_eq!(first, second);

        let value: serde_json::Value = serde_json::from_slice(&first).unwrap();
        let persisted_node = &value["document"]["nodes"][node_id.to_string()];
        assert_eq!(persisted_node["id"], serde_json::json!(node_id.to_string()));
        assert_eq!(
            persisted_node["node_type"],
            serde_json::json!(node_type.as_str())
        );
        assert_eq!(
            persisted_node["user_label"],
            serde_json::json!("User-authored label")
        );

        let persisted_binding = &value["document"]["port_bindings"][0];
        assert_eq!(
            persisted_binding[0],
            serde_json::json!({
                "node_id": node_id.to_string(),
                "port": {
                    "kind": "instance",
                    "template": "dynamic_value",
                    "instance_id": port_instance_id.to_string(),
                },
            })
        );
        assert_eq!(
            persisted_binding[1],
            serde_json::json!({
                "kind": "resolved",
                "origin": {
                    "kind": "function_parameter",
                    "function": "functions/stable.yssbi-function",
                    "parameter": "stable-parameter",
                },
                "order": "stable-order",
                "last_known": {
                    "label": "",
                },
            })
        );

        let persisted_connection = &value["document"]["connections"][connection_id.to_string()];
        assert_eq!(
            persisted_connection["id"],
            serde_json::json!(connection_id.to_string())
        );
        assert_eq!(
            persisted_connection["input"],
            serde_json::json!({
                "node_id": node_id.to_string(),
                "port": {
                    "kind": "instance",
                    "template": "dynamic_value",
                    "instance_id": port_instance_id.to_string(),
                },
            })
        );
        reject_unstable_metadata_keys(&value);
    }

    #[test]
    fn production_graph_io_loads_unknown_node_types_for_compiler_diagnostics() {
        let root = temp_project_dir();
        let graph_path = GraphResourcePath::new("events/Unknown.yssbi-event").unwrap();
        let node_id = NodeId::from_uuid(uuid::Uuid::from_u128(0x501));
        let mut graph = GraphResourceDocument::new("Unknown", GraphResourceKind::Event);
        graph.document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: NodeTypeId::new("yssbi.test.missing").unwrap(),
                position: NodePosition { x: 3.0, y: 4.0 },
                parameters: ParameterValues::new(),
                user_label: Some("Preserved unknown node".into()),
            },
        );
        let mut project = ProjectData::new();
        project.graphs.insert(graph_path.clone(), graph.clone());
        initialize_project_directory(&project, root.as_path()).unwrap();

        let loaded =
            load_project_graph_from_file(root.to_string_lossy().as_ref(), &graph_path).unwrap();

        assert_eq!(loaded, graph);
        std::fs::remove_dir_all(root).unwrap();
    }
}
