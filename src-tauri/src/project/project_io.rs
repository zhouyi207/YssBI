use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    GraphResourceIndex, GraphResourceManifestEntry, PROJECT_METADATA_FILE, ProjectData,
    ProjectError, ProjectWorksheetIndexEntry, ensure_worksheets_dir, flatten_worksheet_layout,
    read_worksheet_index_entries, reconcile_graph_resources,
};
use crate::database::{DatabaseDecl, DatabaseEngine};
use crate::graph::NodeInstanceParams;
use crate::graph::{GraphId, GraphInstance, GraphKind};
use crate::variable::{VariableId, VariableInstance, VariableScope};

pub const SCHEMA_VERSION: u32 = 1;
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
    #[serde(default)]
    pub graphs: Vec<GraphResourceManifestEntry>,
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
    pub graph: GraphInstance,
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
    pub id: GraphId,
    pub name: String,
    #[serde(rename = "type")]
    pub graph_type: GraphDocumentKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVariableIndexEntry {
    pub id: String,
    pub name: String,
    pub data_type: crate::graph::value::DataType,
    pub data_value: crate::graph::value::DataValue,
    pub description: String,
    pub scope: VariableScope,
    pub tags: Vec<String>,
    pub owner_graph_id: Option<String>,
    pub owner_graph_name: Option<String>,
    #[serde(rename = "ownerGraphKind", skip_serializing_if = "Option::is_none")]
    pub owner_graph_kind: Option<GraphDocumentKind>,
}

impl From<VariableInstance> for ProjectVariableIndexEntry {
    fn from(value: VariableInstance) -> Self {
        Self {
            id: value.id.to_string(),
            name: value.name,
            data_type: value.data_type,
            data_value: value.data_value,
            description: value.description,
            scope: value.scope,
            tags: value.tags,
            owner_graph_id: None,
            owner_graph_name: None,
            owner_graph_kind: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIndex {
    pub project_name: String,
    pub app_version: String,
    pub export_time: String,
    pub graphs: Vec<ProjectGraphIndexEntry>,
    #[serde(default)]
    pub worksheets: Vec<ProjectWorksheetIndexEntry>,
    #[serde(default)]
    pub variables: Vec<ProjectVariableIndexEntry>,
}

impl From<&GraphKind> for GraphDocumentKind {
    fn from(value: &GraphKind) -> Self {
        match value {
            GraphKind::Event => GraphDocumentKind::Event,
            GraphKind::Function => GraphDocumentKind::Function,
        }
    }
}

impl From<GraphDocumentKind> for GraphKind {
    fn from(value: GraphDocumentKind) -> Self {
        match value {
            GraphDocumentKind::Event => GraphKind::Event,
            GraphDocumentKind::Function => GraphKind::Function,
        }
    }
}

/// 保存项目到文件
pub fn save_project_to_file(project_data: &ProjectData, path: &str) -> Result<(), ProjectError> {
    save_project_to_directory(project_data, project_root_from_path(path).as_path())
}

pub fn save_project_graph_to_file(
    project_data: &ProjectData,
    path: &str,
    graph_id: &GraphId,
) -> Result<(), ProjectError> {
    let root = project_root_from_path(path);
    std::fs::create_dir_all(root.as_path())?;
    std::fs::create_dir_all(root.join(EVENTS_DIR))?;
    std::fs::create_dir_all(root.join(FUNCTIONS_DIR))?;
    ensure_worksheets_dir(root.as_path())?;

    let graph = project_data.graphs.get(graph_id).ok_or_else(|| {
        ProjectError::InvalidProjectFormat(format!("graph '{}' not loaded", graph_id))
    })?;
    let local_variables = local_variables_for_graph(&project_data.variables, graph_id, &graph.kind);
    let (dir, extension, kind) = match graph.kind {
        GraphKind::Event => (EVENTS_DIR, EVENT_EXTENSION, GraphDocumentKind::Event),
        GraphKind::Function => (
            FUNCTIONS_DIR,
            FUNCTION_EXTENSION,
            GraphDocumentKind::Function,
        ),
    };
    let graph = graph.clone();
    graph.data_state.write().unwrap().prepare_for_persistence();
    let relative_path =
        graph_relative_path_for_save(root.as_path(), dir, extension, &graph.name, graph_id)?;
    write_json(
        root.join(&relative_path).as_path(),
        &GraphDocument {
            schema_version: SCHEMA_VERSION,
            kind,
            graph,
            local_variables,
        },
    )?;
    upsert_graph_resource_manifest_entry(
        root.as_path(),
        GraphResourceManifestEntry {
            id: *graph_id,
            path: relative_path,
            kind,
        },
    )
}

fn save_project_to_directory(project_data: &ProjectData, root: &Path) -> Result<(), ProjectError> {
    std::fs::create_dir_all(root)?;
    std::fs::create_dir_all(root.join(EVENTS_DIR))?;
    std::fs::create_dir_all(root.join(FUNCTIONS_DIR))?;
    ensure_worksheets_dir(root)?;
    ensure_project_database_dir(root)?;

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

    let mut graph_resources = Vec::new();
    for (graph_id, graph) in project_data.graphs.iter() {
        let local_variables =
            local_variables_for_graph(&project_data.variables, graph_id, &graph.kind);
        let (dir, extension, kind) = match graph.kind {
            GraphKind::Event => (EVENTS_DIR, EVENT_EXTENSION, GraphDocumentKind::Event),
            GraphKind::Function => (
                FUNCTIONS_DIR,
                FUNCTION_EXTENSION,
                GraphDocumentKind::Function,
            ),
        };
        let relative_path =
            graph_relative_path_for_save(root, dir, extension, &graph.name, graph_id)?;
        graph_resources.push(GraphResourceManifestEntry {
            id: *graph_id,
            path: relative_path.clone(),
            kind,
        });
        let graph = graph.clone();
        graph.data_state.write().unwrap().prepare_for_persistence();
        write_json(
            root.join(&relative_path).as_path(),
            &GraphDocument {
                schema_version: SCHEMA_VERSION,
                kind,
                graph,
                local_variables,
            },
        )?;
    }

    let manifest = ProjectManifest {
        schema_version: SCHEMA_VERSION,
        project_name: project_data.metadata.project_name.clone(),
        app_version: project_data.metadata.app_version.clone(),
        export_time: project_data.metadata.export_time.clone(),
        graphs: graph_resources,
    };
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

    let variables_path = root.join(GLOBAL_VARIABLES_FILE);
    if variables_path.exists() {
        let document: GlobalVariablesDocument = read_json(variables_path.as_path())?;
        project_data.variables.extend(document.variables);
    }

    Ok(project_data)
}

pub fn read_project_index(path: &str) -> Result<ProjectIndex, ProjectError> {
    let root = project_root_from_path(path);
    flatten_graph_layout(root.as_path())?;
    flatten_worksheet_layout(root.as_path())?;
    let mut manifest = read_project_manifest_from_root(root.as_path())?;
    let (graph_resources, changed) = reconcile_graph_resources(root.as_path(), &mut manifest)?;
    if changed {
        write_json(root.join(PROJECT_METADATA_FILE).as_path(), &manifest)?;
    }
    let mut graphs = Vec::new();
    graphs.extend(read_graph_index_entries(
        root.as_path(),
        EVENTS_DIR,
        EVENT_EXTENSION,
        GraphDocumentKind::Event,
        &graph_resources,
    )?);
    graphs.extend(read_graph_index_entries(
        root.as_path(),
        FUNCTIONS_DIR,
        FUNCTION_EXTENSION,
        GraphDocumentKind::Function,
        &graph_resources,
    )?);
    let worksheets = read_worksheet_index_entries(root.as_path())?;
    let variables = read_variable_index_entries(root.as_path())?;

    Ok(ProjectIndex {
        project_name: manifest.project_name,
        app_version: manifest.app_version,
        export_time: manifest.export_time,
        graphs,
        worksheets,
        variables,
    })
}

pub fn load_project_graph_from_file(
    path: &str,
    graph_id: &GraphId,
) -> Result<GraphDocument, ProjectError> {
    let root = project_root_from_path(path);
    let graph_resources = load_graph_resource_index(root.as_path())?;
    if let Some(resource) = graph_resources.get_by_id(graph_id) {
        let document = read_graph_document(root.join(&resource.path).as_path(), resource.kind)?;
        return Ok(bind_graph_document_resource_identity(
            document,
            resource.kind,
            resource.id,
        ));
    }

    Err(ProjectError::InvalidProjectFormat(format!(
        "graph '{}' not found in project graph files",
        graph_id
    )))
}

pub fn remove_project_graph_from_file(
    path: &str,
    graph_id: &GraphId,
) -> Result<Option<GraphDocumentKind>, ProjectError> {
    let root = project_root_from_path(path);
    let mut manifest = read_project_manifest_from_root(root.as_path())?;
    let (graph_resources, _) = reconcile_graph_resources(root.as_path(), &mut manifest)?;
    if let Some(resource) = graph_resources.get_by_id(graph_id) {
        std::fs::remove_file(root.join(&resource.path))?;
        manifest.graphs.retain(|entry| entry.id != *graph_id);
        write_json(root.join(PROJECT_METADATA_FILE).as_path(), &manifest)?;
        return Ok(Some(resource.kind));
    }
    Ok(None)
}

pub fn duplicate_project_graph_file(
    path: &str,
    graph_id: &GraphId,
) -> Result<GraphDocument, ProjectError> {
    let root = project_root_from_path(path);
    let (source_path, kind, mut document) = find_graph_document_path(root.as_path(), graph_id)?
        .ok_or_else(|| {
            ProjectError::InvalidProjectFormat(format!("graph '{}' not found", graph_id))
        })?;
    let source_dir = source_path.parent().unwrap_or_else(|| root.as_path());
    let graph_resources = load_graph_resource_index(root.as_path())?;
    let names: Vec<String> = read_graph_index_entries(
        root.as_path(),
        graph_dir_for_kind(kind),
        graph_extension_for_kind(kind),
        kind,
        &graph_resources,
    )?
    .into_iter()
    .map(|entry| entry.name)
    .collect();
    document.graph.id = GraphId::new();
    document.graph.name = crate::project::unique_name::unique_name(&document.graph.name, names);
    rebind_graph_document_local_variable_scopes(&mut document, kind);
    let file_name = unique_graph_file_name(
        source_dir,
        &document.graph.name,
        graph_extension_for_kind(kind),
        None,
    );
    let target_path = source_dir.join(file_name);
    write_json(target_path.as_path(), &document)?;
    let relative_path = target_path
        .strip_prefix(root.as_path())
        .map(path_to_slash_string)
        .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))?;
    upsert_graph_resource_manifest_entry(
        root.as_path(),
        GraphResourceManifestEntry {
            id: document.graph.id,
            path: relative_path,
            kind,
        },
    )?;
    Ok(document)
}

fn read_project_manifest_from_root(root: &Path) -> Result<ProjectManifest, ProjectError> {
    let manifest_path = root.join(PROJECT_METADATA_FILE);
    if !manifest_path.exists() {
        return Err(ProjectError::FileNotFound(manifest_path));
    }
    read_json(manifest_path.as_path())
}

fn load_graph_resource_index(root: &Path) -> Result<GraphResourceIndex, ProjectError> {
    flatten_graph_layout(root)?;
    let mut manifest = read_project_manifest_from_root(root)?;
    let (index, changed) = reconcile_graph_resources(root, &mut manifest)?;
    if changed {
        write_json(root.join(PROJECT_METADATA_FILE).as_path(), &manifest)?;
    }
    Ok(index)
}

fn upsert_graph_resource_manifest_entry(
    root: &Path,
    entry: GraphResourceManifestEntry,
) -> Result<(), ProjectError> {
    let mut manifest = read_project_manifest_from_root(root)?;
    manifest
        .graphs
        .retain(|item| item.id != entry.id && item.path != entry.path);
    manifest.graphs.push(entry);
    manifest.graphs.sort_by(|a, b| a.path.cmp(&b.path));
    write_json(root.join(PROJECT_METADATA_FILE).as_path(), &manifest)
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

/// Move a project directory to the system recycle bin after validating `metadata.yssbi` exists.
pub fn delete_project_directory(path: &str) -> Result<(), ProjectError> {
    let root = project_root_from_path(path);
    let manifest = root.join(PROJECT_METADATA_FILE);
    if !manifest.is_file() {
        return Err(ProjectError::InvalidProjectFormat(format!(
            "missing {PROJECT_METADATA_FILE} under {}",
            root.display()
        )));
    }
    if root.exists() {
        trash::delete(&root).map_err(|e| {
            ProjectError::InvalidProjectFormat(format!(
                "failed to move project to recycle bin: {e}"
            ))
        })?;
    }
    Ok(())
}

fn copy_project_directory(src: &Path, dst: &Path) -> Result<(), ProjectError> {
    if !src.is_dir() {
        return Err(ProjectError::InvalidProjectFormat(format!(
            "source project directory not found: {}",
            src.display()
        )));
    }
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_project_directory(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// 将当前项目复制到新目录，返回新 `metadata.yssbi` 绝对路径。
/// 调用方负责在复制后 reload 内存状态并切换 `ProjectState` 路径。
pub fn save_project_as_to_directory(
    state: &crate::project::ProjectState,
    new_root_path: &str,
) -> Result<String, ProjectError> {
    use crate::project::validate_new_project_path;

    let validation = validate_new_project_path(new_root_path);
    if !validation.ok {
        return Err(ProjectError::InvalidProjectFormat(
            validation.message.unwrap_or_else(|| "项目路径无效".into()),
        ));
    }

    let old_path = state
        .get_path()
        .ok_or_else(|| ProjectError::InvalidProjectFormat("项目尚未加载".into()))?;
    state
        .persist_current_project()
        .map_err(ProjectError::InvalidProjectFormat)?;

    let old_root = project_root_from_path(&old_path);
    let new_root = PathBuf::from(new_root_path.trim());
    if old_root == new_root {
        return Err(ProjectError::InvalidProjectFormat(
            "不能另存为当前项目目录".into(),
        ));
    }

    std::fs::create_dir_all(&new_root)?;
    copy_project_directory(old_root.as_path(), new_root.as_path())?;

    Ok(new_root
        .join(PROJECT_METADATA_FILE)
        .to_string_lossy()
        .into_owned())
}

fn local_variables_for_graph(
    variables: &HashMap<VariableId, VariableInstance>,
    graph_id: &GraphId,
    graph_kind: &GraphKind,
) -> HashMap<VariableId, VariableInstance> {
    let graph_id = graph_id.to_string();
    variables
        .iter()
        .filter(|(_, variable)| match (&variable.scope, graph_kind) {
            (VariableScope::Event { event_id }, GraphKind::Event) => event_id == &graph_id,
            (VariableScope::Function { function_id }, GraphKind::Function) => {
                function_id == &graph_id
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
    if document.kind != expected_kind {
        return Err(ProjectError::InvalidProjectFormat(format!(
            "graph file '{}' kind does not match manifest",
            path.display()
        )));
    }
    if let Some(name) = graph_name_from_file_path(path) {
        document.graph.name = name;
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
    Ok(bind_graph_document_resource_identity(
        document,
        expected_kind,
        resource.id,
    ))
}

fn bind_graph_document_resource_identity(
    mut document: GraphDocument,
    kind: GraphDocumentKind,
    resource_id: GraphId,
) -> GraphDocument {
    document.graph.id = resource_id;
    let graph_id_string = document.graph.id.to_string();
    let scope = scoped_variable_scope(kind, &graph_id_string);
    for variable in document.local_variables.values_mut() {
        variable.scope = scope.clone();
    }
    document
}

/// 仅读取当前图文件头部（`graph.name`）。
/// 用于索引显示名，避免对每个文件做完整反序列化。
#[derive(Deserialize)]
struct GraphFileHeader {
    graph: GraphFileHeaderGraph,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphFileHeaderGraph {
    #[serde(default)]
    name: String,
}

fn read_graph_file_header(path: &Path) -> Result<GraphFileHeader, ProjectError> {
    read_json(path)
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
        let header = match read_graph_file_header(path.as_path()) {
            Ok(header) => header,
            Err(_) => continue,
        };
        let relative_path = path_to_slash_string(
            path.strip_prefix(root)
                .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))?,
        );
        let Some(resource) = graph_resources.get_by_path(&relative_path) else {
            continue;
        };
        let name = graph_name_from_file_path(path.as_path()).unwrap_or(header.graph.name);
        entries.push(ProjectGraphIndexEntry {
            id: resource.id,
            name,
            graph_type: expected_kind,
        });
    }
    Ok(entries)
}

fn rebind_graph_document_local_variable_scopes(
    document: &mut GraphDocument,
    kind: GraphDocumentKind,
) {
    let graph_id_string = document.graph.id.to_string();
    let scope = scoped_variable_scope(kind, &graph_id_string);
    for variable in document.local_variables.values_mut() {
        variable.scope = scope.clone();
    }
}

fn scoped_variable_scope(kind: GraphDocumentKind, graph_id: &str) -> VariableScope {
    match kind {
        GraphDocumentKind::Event => VariableScope::Event {
            event_id: graph_id.to_string(),
        },
        GraphDocumentKind::Function => VariableScope::Function {
            function_id: graph_id.to_string(),
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
        let document = match read_graph_document_for_resource(root, path.as_path(), expected_kind) {
            Ok(document) => document,
            Err(_) => continue,
        };
        let graph_name = graph_name_from_file_path(path.as_path()).unwrap_or(document.graph.name);
        let owner_graph_id = document.graph.id.to_string();
        for variable in document.local_variables.into_values() {
            entries.push(ProjectVariableIndexEntry {
                id: variable.id.to_string(),
                name: variable.name,
                data_type: variable.data_type,
                data_value: variable.data_value,
                description: variable.description,
                scope: variable.scope,
                tags: variable.tags,
                owner_graph_id: Some(owner_graph_id.clone()),
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

fn graph_relative_path_for_save(
    root: &Path,
    dir: &str,
    extension: &str,
    graph_name: &str,
    graph_id: &GraphId,
) -> Result<String, ProjectError> {
    let target_dir = root.join(dir);
    std::fs::create_dir_all(&target_dir)?;
    let existing_path = find_graph_file_path(root, dir, extension, graph_id)?;
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

fn find_graph_file_path(
    root: &Path,
    dir: &str,
    _extension: &str,
    graph_id: &GraphId,
) -> Result<Option<PathBuf>, ProjectError> {
    let graph_resources = match load_graph_resource_index(root) {
        Ok(index) => index,
        Err(ProjectError::FileNotFound(_)) => return Ok(None),
        Err(error) => return Err(error),
    };
    if let Some(resource) = graph_resources.get_by_id(graph_id) {
        if resource.path.starts_with(&format!("{dir}/")) {
            return Ok(Some(root.join(&resource.path)));
        }
    }
    Ok(None)
}

pub(crate) fn find_graph_document_path(
    root: &Path,
    graph_id: &GraphId,
) -> Result<Option<(PathBuf, GraphDocumentKind, GraphDocument)>, ProjectError> {
    if let Some(resource) = load_graph_resource_index(root)?.get_by_id(graph_id) {
        let path = root.join(&resource.path);
        let document = bind_graph_document_resource_identity(
            read_graph_document(path.as_path(), resource.kind)?,
            resource.kind,
            resource.id,
        );
        return Ok(Some((path, resource.kind, document)));
    }
    Ok(None)
}

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

fn graph_dir_for_kind(kind: GraphDocumentKind) -> &'static str {
    match kind {
        GraphDocumentKind::Event => EVENTS_DIR,
        GraphDocumentKind::Function => FUNCTIONS_DIR,
    }
}

fn graph_extension_for_kind(kind: GraphDocumentKind) -> &'static str {
    match kind {
        GraphDocumentKind::Event => EVENT_EXTENSION,
        GraphDocumentKind::Function => FUNCTION_EXTENSION,
    }
}

/// Hoists nested graph files under `events/` and `functions/` to each kind's root directory.
pub fn flatten_graph_layout(root: &Path) -> Result<bool, ProjectError> {
    let mut changed = false;
    changed |= flatten_kind_graph_layout(root, EVENTS_DIR, EVENT_EXTENSION)?;
    changed |= flatten_kind_graph_layout(root, FUNCTIONS_DIR, FUNCTION_EXTENSION)?;
    if changed {
        remove_empty_graph_subdirs(&root.join(EVENTS_DIR))?;
        remove_empty_graph_subdirs(&root.join(FUNCTIONS_DIR))?;
        let mut manifest = read_project_manifest_from_root(root)?;
        let (_, manifest_changed) = reconcile_graph_resources(root, &mut manifest)?;
        if manifest_changed {
            write_json(root.join(PROJECT_METADATA_FILE).as_path(), &manifest)?;
        }
    }
    Ok(changed)
}

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

    let mut manifest = read_project_manifest_from_root(root).ok();
    let mut changed = false;
    for nested_path in nested_paths {
        let graph_name = read_graph_file_header(nested_path.as_path())
            .ok()
            .map(|header| header.graph.name)
            .or_else(|| graph_name_from_file_path(nested_path.as_path()))
            .unwrap_or_else(|| "Untitled".to_string());
        let file_name = unique_graph_file_name(graph_dir.as_path(), &graph_name, extension, None);
        let target_path = graph_dir.join(&file_name);
        if nested_path == target_path {
            continue;
        }

        if let Some(manifest) = manifest.as_mut() {
            let nested_relative = nested_path
                .strip_prefix(root)
                .map(path_to_slash_string)
                .unwrap_or_default();
            let new_relative = target_path
                .strip_prefix(root)
                .map(path_to_slash_string)
                .unwrap_or_default();
            for entry in manifest.graphs.iter_mut() {
                if super::normalize_resource_path(&entry.path)
                    == super::normalize_resource_path(&nested_relative)
                {
                    entry.path = new_relative.clone();
                }
            }
        }

        std::fs::rename(&nested_path, &target_path)?;
        changed = true;
    }

    if changed {
        if let Some(manifest) = manifest {
            write_json(root.join(PROJECT_METADATA_FILE).as_path(), &manifest)?;
        }
    }

    Ok(changed)
}

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

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ProjectError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(value).map_err(ProjectError::Serialize)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn ensure_project_database_dir(root: &Path) -> Result<(), ProjectError> {
    std::fs::create_dir_all(root.join(DATABASE_DIR))?;
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
    use crate::graph::value::{DataType, DataValue};
    use crate::project::ProjectState;

    fn temp_project_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("yssbi-project-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn graph_index_id(root: &Path, name: &str) -> GraphId {
        read_project_index(root.to_string_lossy().as_ref())
            .unwrap()
            .graphs
            .into_iter()
            .find(|graph| graph.name == name)
            .map(|graph| graph.id)
            .unwrap_or_else(|| panic!("graph '{name}' should be indexed"))
    }

    #[test]
    fn saves_manifest_graph_files_and_variable_scopes() {
        let root = temp_project_dir();
        let state = ProjectState::new();
        let event = state.add_event("Startup");
        let function = state.add_function("Compute");

        state.add_variable(
            "Global Name",
            DataType::String,
            DataValue::String("global".into()),
            "",
            VariableScope::Global,
            vec![],
        );
        state.add_variable(
            "Event Local",
            DataType::Int64,
            DataValue::Int64(1),
            "",
            VariableScope::Event {
                event_id: event.id.to_string(),
            },
            vec![],
        );
        state.add_variable(
            "Function Local",
            DataType::Float64,
            DataValue::Float64(2.0),
            "",
            VariableScope::Function {
                function_id: function.id.to_string(),
            },
            vec![],
        );

        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();

        assert!(root.join(PROJECT_METADATA_FILE).is_file());
        assert!(root.join(GLOBAL_VARIABLES_FILE).is_file());
        assert!(
            root.join(EVENTS_DIR)
                .join(format!("Startup.{}", EVENT_EXTENSION))
                .is_file()
        );
        assert!(
            root.join(FUNCTIONS_DIR)
                .join(format!("Compute.{}", FUNCTION_EXTENSION))
                .is_file()
        );
        std::fs::rename(
            root.join(EVENTS_DIR)
                .join(format!("Startup.{}", EVENT_EXTENSION)),
            root.join(EVENTS_DIR)
                .join(format!("Startup 1.{}", EVENT_EXTENSION)),
        )
        .unwrap();
        let manifest_json: serde_json::Value =
            read_json(root.join(PROJECT_METADATA_FILE).as_path()).unwrap();
        assert!(manifest_json.get("events").is_none());
        assert!(manifest_json.get("functions").is_none());

        let index = read_project_index(root.to_string_lossy().as_ref()).unwrap();
        assert_eq!(index.graphs.len(), 2);
        assert!(index.graphs.iter().any(|graph| graph.name == "Startup 1"));

        let manifest_only = load_project_from_file(root.to_string_lossy().as_ref()).unwrap();
        assert!(manifest_only.graphs.is_empty());
        assert_eq!(manifest_only.variables.len(), 1);

        let event_resource_id = graph_index_id(root.as_path(), "Startup 1");
        let event_doc =
            load_project_graph_from_file(root.to_string_lossy().as_ref(), &event_resource_id)
                .unwrap();
        assert_eq!(event_doc.kind, GraphDocumentKind::Event);
        assert_eq!(event_doc.graph.name, "Startup 1");
        assert_eq!(event_doc.local_variables.len(), 1);

        let function_resource_id = graph_index_id(root.as_path(), "Compute");
        let function_doc =
            load_project_graph_from_file(root.to_string_lossy().as_ref(), &function_resource_id)
                .unwrap();
        assert_eq!(function_doc.kind, GraphDocumentKind::Function);
        assert_eq!(function_doc.local_variables.len(), 1);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn graph_index_identity_is_derived_from_project_path_not_file_graph_id() {
        let root = temp_project_dir();
        let state = ProjectState::new();
        let event = state.add_event("Path Identity");
        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();

        let original_path = root
            .join(EVENTS_DIR)
            .join(format!("Path Identity.{}", EVENT_EXTENSION));
        let renamed_path = root
            .join(EVENTS_DIR)
            .join(format!("Path Identity Copy.{}", EVENT_EXTENSION));
        std::fs::rename(&original_path, &renamed_path).unwrap();

        let index = read_project_index(root.to_string_lossy().as_ref()).unwrap();
        let entry = index
            .graphs
            .iter()
            .find(|graph| graph.name == "Path Identity Copy")
            .expect("renamed graph should be indexed");

        assert_ne!(
            entry.id, event.id,
            "resource identity should come from the project path, not the graph id persisted inside the file"
        );

        let loaded = load_project_graph_from_file(root.to_string_lossy().as_ref(), &entry.id)
            .expect("path-derived id should load renamed graph");
        assert_eq!(loaded.graph.id, entry.id);
        assert_eq!(loaded.graph.name, "Path Identity Copy");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn saved_graph_file_does_not_persist_graph_identity() {
        let root = temp_project_dir();
        let state = ProjectState::new();
        state.add_event("No Persisted Id");
        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();

        let graph_json: serde_json::Value = read_json(
            root.join(EVENTS_DIR)
                .join(format!("No Persisted Id.{}", EVENT_EXTENSION))
                .as_path(),
        )
        .unwrap();

        assert!(
            graph_json
                .get("graph")
                .and_then(|graph| graph.get("id"))
                .is_none(),
            "graph identity should be derived from project path, not persisted inside the graph file"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn round_trips_graph_with_dynamic_pins_values_and_connections() {
        use crate::graph::{PinDirection, PinKind};

        let root = temp_project_dir();
        let state = ProjectState::new();
        let event = state.add_event("RoundTrip");
        let graph = state.get_graph(&event.id).expect("event graph");

        // 两个带可重复 Operands（动态 pin）的 Add 节点
        let node_a = graph.create_node("Math:Operators:Add (+)").expect("node a");
        let node_b = graph.create_node("Math:Operators:Add (+)").expect("node b");

        // 收集 pin：A 的输出数据 pin、A 的某个 Operand 输入、B 的某个 Operand 输入
        let (a_output, a_operand_input, b_operand_input, total_pins) = {
            let ds = graph.data_state.read().unwrap();
            let pins_of = |node_id| -> Vec<crate::graph::PinInstance> {
                ds.nodes
                    .get(&node_id)
                    .unwrap()
                    .pin_ids
                    .iter()
                    .filter_map(|pid| ds.pins.get(pid).cloned())
                    .collect()
            };
            let a_pins = pins_of(node_a);
            let b_pins = pins_of(node_b);
            let a_output = a_pins
                .iter()
                .find(|p| p.definition.direction == PinDirection::Output && p.is_data())
                .expect("a output")
                .id;
            let a_operand_input = a_pins
                .iter()
                .find(|p| p.definition.direction == PinDirection::Input && p.is_data())
                .expect("a operand")
                .id;
            let b_operand_input = b_pins
                .iter()
                .find(|p| p.definition.direction == PinDirection::Input && p.is_data())
                .expect("b operand")
                .id;
            let total_pins = ds.pins.len();
            let _dynamic_pin = a_pins
                .iter()
                .find(|p| p.definition.should_persist_full_definition() && p.is_data())
                .map(|p| p.id)
                .expect("a has a dynamic operand pin");
            (a_output, a_operand_input, b_operand_input, total_pins)
        };

        // 设置一个 userValue，并建立一条连接
        {
            let mut ds = graph.data_state.write().unwrap();
            ds.pins.get_mut(&a_operand_input).unwrap().user_value = Some(DataValue::Int64(7));
            ds.connections.connect(a_output, b_operand_input);
        }

        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();

        // 重新从磁盘加载（新格式），与原图对比
        let event_resource_id = graph_index_id(root.as_path(), "RoundTrip");
        let doc = load_project_graph_from_file(root.to_string_lossy().as_ref(), &event_resource_id)
            .unwrap();
        let loaded = doc.graph;
        let lds = loaded.data_state.read().unwrap();

        assert_eq!(lds.nodes.len(), 2, "node count round-trips");
        assert_eq!(lds.pins.len(), total_pins, "pin count round-trips");

        // userValue 保留。路径身份绑定后 pin id 可重映射，因此按值语义断言。
        assert!(
            lds.pins
                .values()
                .any(|pin| pin.user_value == Some(DataValue::Int64(7))),
            "user value round-trips"
        );

        // 连接保留。路径身份绑定后 pin id 可重映射，因此按连接数量断言。
        assert_eq!(
            lds.connections.all_connections().len(),
            1,
            "connection round-trips"
        );

        // 动态/可重复 pin 保留完整定义（data_type 非空）；静态输出 pin 仅留契约（data_type 为空）
        assert!(
            lds.pins
                .values()
                .filter(|pin| pin.definition.should_persist_full_definition() && pin.is_data())
                .any(|pin| pin.definition.data_type.is_some()),
            "dynamic operand pin keeps its full definition override"
        );
        assert!(
            lds.pins
                .values()
                .any(|pin| pin.definition.kind == PinKind::Data
                    && pin.definition.direction == PinDirection::Output
                    && pin.definition.data_type.is_none()),
            "static pin persists only a contract; full definition re-attached from registry"
        );

        drop(lds);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn flatten_graph_layout_hoists_nested_graph_files() {
        let root = temp_project_dir();
        let state = ProjectState::new();
        let _event = state.add_event("Nested Event");
        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();

        let nested_dir = root.join(EVENTS_DIR).join("Sub");
        std::fs::create_dir_all(&nested_dir).unwrap();
        let flat_file = root
            .join(EVENTS_DIR)
            .join(format!("Nested Event.{}", EVENT_EXTENSION));
        let nested_file = nested_dir.join(format!("Nested Event.{}", EVENT_EXTENSION));
        std::fs::rename(&flat_file, &nested_file).unwrap();

        let mut manifest: ProjectManifest =
            read_json(root.join(PROJECT_METADATA_FILE).as_path()).unwrap();
        for entry in manifest.graphs.iter_mut() {
            if entry.path.starts_with(&format!("{EVENTS_DIR}/")) {
                entry.path = format!("{EVENTS_DIR}/Sub/Nested Event.{EVENT_EXTENSION}");
            }
        }
        write_json(root.join(PROJECT_METADATA_FILE).as_path(), &manifest).unwrap();

        flatten_graph_layout(root.as_path()).unwrap();

        assert!(
            root.join(EVENTS_DIR)
                .join(format!("Nested Event.{}", EVENT_EXTENSION))
                .is_file()
        );
        assert!(!nested_file.exists());
        assert!(!nested_dir.exists());

        let index = read_project_index(root.to_string_lossy().as_ref()).unwrap();
        assert_eq!(index.graphs.len(), 1);
        assert_eq!(index.graphs[0].name, "Nested Event");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn create_event_saves_graph_at_kind_root() {
        let root = temp_project_dir();
        let state = ProjectState::new();
        let _event = state.add_event("Root Event");
        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();

        let graph_path = root
            .join(EVENTS_DIR)
            .join(format!("Root Event.{}", EVENT_EXTENSION));
        assert!(graph_path.is_file());
        assert_eq!(
            graph_path
                .parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str()),
            Some(EVENTS_DIR)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn read_project_index_skips_invalid_graph_files() {
        let root = temp_project_dir();
        let state = ProjectState::new();
        let _event = state.add_event("Valid Event");
        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();
        std::fs::write(
            root.join(EVENTS_DIR)
                .join(format!("Broken.{}", EVENT_EXTENSION)),
            "",
        )
        .unwrap();

        let index = read_project_index(root.to_string_lossy().as_ref()).unwrap();

        assert_eq!(index.graphs.len(), 1);
        assert!(index.graphs.iter().any(|graph| graph.name == "Valid Event"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn read_project_index_includes_global_and_graph_local_variables() {
        let root = temp_project_dir();
        let state = ProjectState::new();
        let event = state.add_event("Indexed Event");
        let function = state.add_function("Indexed Function");

        state.add_variable(
            "Global Var",
            DataType::String,
            DataValue::String("g".into()),
            "",
            VariableScope::Global,
            vec![],
        );
        state.add_variable(
            "Event Local",
            DataType::Int32,
            DataValue::Int32(1),
            "",
            VariableScope::Event {
                event_id: event.id.to_string(),
            },
            vec![],
        );
        state.add_variable(
            "Function Local",
            DataType::Float64,
            DataValue::Float64(2.0),
            "",
            VariableScope::Function {
                function_id: function.id.to_string(),
            },
            vec![],
        );
        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();

        let index = read_project_index(root.to_string_lossy().as_ref()).unwrap();
        assert_eq!(index.variables.len(), 3);
        assert!(
            index
                .variables
                .iter()
                .any(|v| v.name == "Global Var" && v.owner_graph_id.is_none())
        );
        let event_resource_id = index
            .graphs
            .iter()
            .find(|graph| graph.name == "Indexed Event")
            .map(|graph| graph.id.to_string())
            .expect("event resource id");
        let function_resource_id = index
            .graphs
            .iter()
            .find(|graph| graph.name == "Indexed Function")
            .map(|graph| graph.id.to_string())
            .expect("function resource id");
        assert!(
            index.variables.iter().any(|v| v.name == "Event Local"
                && v.owner_graph_id.as_deref() == Some(&event_resource_id))
        );
        assert!(index.variables.iter().any(|v| {
            v.name == "Function Local" && v.owner_graph_id.as_deref() == Some(&function_resource_id)
        }));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn read_project_index_treats_explorer_copied_graph_files_as_distinct_resources() {
        let root = temp_project_dir();
        let state = ProjectState::new();
        let event = state.add_event("Copied Event");
        let _local = state.add_variable(
            "Copied Local",
            DataType::Int32,
            DataValue::Int32(7),
            "",
            VariableScope::Event {
                event_id: event.id.to_string(),
            },
            vec![],
        );
        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();

        let original_path = root
            .join(EVENTS_DIR)
            .join(format!("Copied Event.{}", EVENT_EXTENSION));
        let copied_path = root
            .join(EVENTS_DIR)
            .join(format!("Copied Event 1.{}", EVENT_EXTENSION));
        std::fs::copy(&original_path, &copied_path).unwrap();

        let index = read_project_index(root.to_string_lossy().as_ref()).unwrap();

        assert_eq!(index.graphs.len(), 2);
        let ids: std::collections::HashSet<_> = index.graphs.iter().map(|graph| graph.id).collect();
        assert_eq!(ids.len(), 2);
        let original_entry = index
            .graphs
            .iter()
            .find(|graph| graph.name == "Copied Event")
            .expect("original graph entry");
        let copied_entry = index
            .graphs
            .iter()
            .find(|graph| graph.name == "Copied Event 1")
            .expect("copied graph entry");
        let original_doc = read_graph_document_for_resource(
            root.as_path(),
            original_path.as_path(),
            GraphDocumentKind::Event,
        )
        .unwrap();
        let copied_doc = read_graph_document_for_resource(
            root.as_path(),
            copied_path.as_path(),
            GraphDocumentKind::Event,
        )
        .unwrap();
        assert_ne!(original_doc.graph.id, copied_doc.graph.id);
        assert_eq!(original_doc.graph.id, original_entry.id);
        assert_eq!(copied_doc.graph.id, copied_entry.id);

        assert_eq!(original_doc.local_variables.len(), 1);
        assert_eq!(copied_doc.local_variables.len(), 1);
        let original_local = original_doc.local_variables.values().next().unwrap();
        let copied_local = copied_doc.local_variables.values().next().unwrap();
        assert_eq!(original_local.name, "Copied Local");
        assert_eq!(copied_local.name, "Copied Local");
        assert_eq!(original_local.id, copied_local.id);
        assert_eq!(
            original_local.scope,
            VariableScope::Event {
                event_id: original_doc.graph.id.to_string(),
            }
        );
        assert_eq!(
            copied_local.scope,
            VariableScope::Event {
                event_id: copied_doc.graph.id.to_string(),
            }
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn duplicate_project_graph_file_preserves_graph_local_variable_ids() {
        let root = temp_project_dir();
        let state = ProjectState::new();
        let event = state.add_event("Duplicate Command Event");
        let local = state.add_variable(
            "Command Local",
            DataType::Int32,
            DataValue::Int32(7),
            "",
            VariableScope::Event {
                event_id: event.id.to_string(),
            },
            vec![],
        );
        let graph = state.get_graph(&event.id).unwrap();
        graph
            .create_node_raw(
                "Variables:Get Variable",
                0.0,
                0.0,
                Some(NodeInstanceParams::Variable {
                    variable_id: local.id.to_string(),
                    variable_name: Some(local.name.clone()),
                    variable_type: Some(local.data_type.to_string()),
                }),
            )
            .unwrap();
        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();

        let event_resource_id = graph_index_id(root.as_path(), "Duplicate Command Event");
        let duplicated =
            duplicate_project_graph_file(root.to_string_lossy().as_ref(), &event_resource_id)
                .unwrap();

        assert_ne!(duplicated.graph.id, event.id);
        assert_eq!(duplicated.local_variables.len(), 1);
        let duplicated_local = duplicated.local_variables.values().next().unwrap();
        assert_eq!(duplicated_local.name, "Command Local");
        assert_eq!(duplicated_local.id, local.id);
        assert_eq!(
            duplicated_local.scope,
            VariableScope::Event {
                event_id: duplicated.graph.id.to_string(),
            }
        );

        let data_state = duplicated.graph.data_state.read().unwrap();
        assert!(data_state.nodes.values().any(|node| {
            node.instance_params.variable_id() == Some(duplicated_local.id.to_string().as_str())
        }));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn duplicate_project_graph_file_preserves_graph_local_node_and_pin_ids() {
        let root = temp_project_dir();
        let state = ProjectState::new();
        let event = state.add_event("Duplicate Id Event");
        let local = state.add_variable(
            "Dup Id Local",
            DataType::Int32,
            DataValue::Int32(1),
            "",
            VariableScope::Event {
                event_id: event.id.to_string(),
            },
            vec![],
        );
        let graph = state.get_graph(&event.id).unwrap();
        graph
            .create_node_raw(
                "Variables:Get Variable",
                0.0,
                0.0,
                Some(NodeInstanceParams::Variable {
                    variable_id: local.id.to_string(),
                    variable_name: Some(local.name.clone()),
                    variable_type: Some(local.data_type.to_string()),
                }),
            )
            .unwrap();
        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();

        let (source_node_ids, source_pin_ids): (
            std::collections::HashSet<String>,
            std::collections::HashSet<String>,
        ) = {
            let data_state = graph.data_state.read().unwrap();
            (
                data_state.nodes.keys().map(|id| id.to_string()).collect(),
                data_state.pins.keys().map(|id| id.to_string()).collect(),
            )
        };
        assert!(!source_node_ids.is_empty());
        assert!(!source_pin_ids.is_empty());

        let event_resource_id = graph_index_id(root.as_path(), "Duplicate Id Event");
        let duplicated =
            duplicate_project_graph_file(root.to_string_lossy().as_ref(), &event_resource_id)
                .unwrap();
        let dup_state = duplicated.graph.data_state.read().unwrap();

        let dup_node_ids: std::collections::HashSet<String> =
            dup_state.nodes.keys().map(|id| id.to_string()).collect();
        let dup_pin_ids: std::collections::HashSet<String> =
            dup_state.pins.keys().map(|id| id.to_string()).collect();

        // 结构保持：节点/pin 数量不变
        assert_eq!(dup_node_ids.len(), source_node_ids.len());
        assert_eq!(dup_pin_ids.len(), source_pin_ids.len());

        // node/pin id 是 graph-local 标识；复制图保留本地图结构，不再为资源身份重写实体 id。
        assert_eq!(dup_node_ids, source_node_ids);
        assert_eq!(dup_pin_ids, source_pin_ids);

        // pin.node_id 与 node.pin_ids 仍自洽，且都指向复制图内实体
        for node in dup_state.nodes.values() {
            for pin_id in &node.pin_ids {
                let pin = dup_state.pins.get(pin_id).expect("node 引用的 pin 应存在");
                assert_eq!(pin.node_id, node.id, "pin.node_id 应指向其所属节点");
            }
        }

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn loading_graph_resolves_variable_node_pins_from_local_variable_table() {
        let root = temp_project_dir();
        let state = ProjectState::new();
        let event = state.add_event("Variable Pin Event");
        let local = state.add_variable(
            "Resolved Local",
            DataType::Float64,
            DataValue::Float64(1.5),
            "",
            VariableScope::Event {
                event_id: event.id.to_string(),
            },
            vec![],
        );
        let graph = state.get_graph(&event.id).unwrap();
        graph
            .create_node_raw(
                "Variables:Get Variable",
                0.0,
                0.0,
                Some(NodeInstanceParams::Variable {
                    variable_id: local.id.to_string(),
                    variable_name: None,
                    variable_type: None,
                }),
            )
            .unwrap();
        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();

        let loaded_data = load_project_from_file(root.to_string_lossy().as_ref()).unwrap();
        let loaded_state = ProjectState::new();
        loaded_state.set_path(Some(root.to_string_lossy().to_string()));
        loaded_state.set_data(loaded_data);
        let loaded_document = loaded_state
            .load_graph_from_current_project(&graph_index_id(root.as_path(), "Variable Pin Event"))
            .unwrap();

        let data_state = loaded_document.graph.data_state.read().unwrap();
        let variable_node = data_state
            .nodes
            .values()
            .find(|node| matches!(node.instance_params, NodeInstanceParams::Variable { .. }))
            .unwrap();
        let data_pin = variable_node
            .pin_ids
            .iter()
            .filter_map(|pin_id| data_state.pins.get(pin_id))
            .find(|pin| pin.definition.kind == crate::graph::PinKind::Data)
            .unwrap();

        assert_eq!(data_pin.definition.name, "Resolved Local");
        assert_eq!(
            data_state.pin_types.get(&data_pin.id),
            Some(&DataType::Float64)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn loading_graph_resolves_get_dataframe_pin_name_from_database_catalog() {
        let root = temp_project_dir();
        let state = ProjectState::new();
        let event = state.add_event("DataFrame Pin Event");
        {
            let mut data = state.project_data.write().unwrap();
            data.databases.insert(
                "df-1".to_string(),
                DatabaseDecl {
                    id: "df-1".to_string(),
                    engine: DatabaseEngine::DuckDb {
                        path: "database/project.duckdb".to_string(),
                        table: "SalesTable".to_string(),
                    },
                    schema_version: 1,
                    required: true,
                    name: Some("SalesData".to_string()),
                },
            );
        }
        let graph = state.get_graph(&event.id).unwrap();
        graph
            .create_node_raw(
                "Data:Get DataFrame",
                0.0,
                0.0,
                Some(NodeInstanceParams::DataFrame {
                    dataframe_id: "df-1".to_string(),
                }),
            )
            .unwrap();
        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();

        let mut loaded_data = load_project_from_file(root.to_string_lossy().as_ref()).unwrap();
        loaded_data.databases.insert(
            "df-1".to_string(),
            DatabaseDecl {
                id: "df-1".to_string(),
                engine: DatabaseEngine::DuckDb {
                    path: "database/project.duckdb".to_string(),
                    table: "SalesTable".to_string(),
                },
                schema_version: 1,
                required: true,
                name: Some("SalesData".to_string()),
            },
        );
        let loaded_state = ProjectState::new();
        loaded_state.set_path(Some(root.to_string_lossy().to_string()));
        loaded_state.set_data(loaded_data);
        let loaded_document = loaded_state
            .load_graph_from_current_project(&graph_index_id(root.as_path(), "DataFrame Pin Event"))
            .unwrap();

        let data_state = loaded_document.graph.data_state.read().unwrap();
        let dataframe_node = data_state
            .nodes
            .values()
            .find(|node| node.instance_params.dataframe_id() == Some("df-1"))
            .unwrap();
        assert_eq!(dataframe_node.definition.name, "Get DataFrame");

        let data_pin = dataframe_node
            .pin_ids
            .iter()
            .filter_map(|pin_id| data_state.pins.get(pin_id))
            .find(|pin| pin.definition.kind == crate::graph::PinKind::Data)
            .unwrap();
        assert_eq!(data_pin.definition.name, "SalesData");
        assert_eq!(
            data_pin.definition.data_type,
            Some(crate::graph::pin::PinDataTypeDefinition::concrete(
                DataType::DataFrame
            ))
        );
        assert_eq!(
            data_state.pin_types.get(&data_pin.id),
            Some(&DataType::DataFrame)
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
