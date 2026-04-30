use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{ProjectData, ProjectError, PROJECT_METADATA_FILE};
use crate::database::DatabaseDecl;
use crate::graph::{GraphId, GraphInstance, GraphKind};
use crate::variable::{VariableId, VariableInstance, VariableScope};

pub const SCHEMA_VERSION: u32 = 1;
pub const EVENTS_DIR: &str = "events";
pub const FUNCTIONS_DIR: &str = "functions";
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
    pub databases: HashMap<String, DatabaseDecl>,
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
pub struct ProjectIndex {
    pub project_name: String,
    pub app_version: String,
    pub export_time: String,
    pub graphs: Vec<ProjectGraphIndexEntry>,
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

fn save_project_to_directory(project_data: &ProjectData, root: &Path) -> Result<(), ProjectError> {
    std::fs::create_dir_all(root)?;
    std::fs::create_dir_all(root.join(EVENTS_DIR))?;
    std::fs::create_dir_all(root.join(FUNCTIONS_DIR))?;

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
        write_json(
            root.join(&relative_path).as_path(),
            &GraphDocument {
                schema_version: SCHEMA_VERSION,
                kind,
                graph: graph.clone(),
                local_variables,
            },
        )?;
    }

    let manifest = ProjectManifest {
        schema_version: SCHEMA_VERSION,
        project_name: project_data.metadata.project_name.clone(),
        app_version: project_data.metadata.app_version.clone(),
        export_time: project_data.metadata.export_time.clone(),
        databases: project_data.databases.clone(),
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
    project_data.databases = manifest.databases;

    let variables_path = root.join(GLOBAL_VARIABLES_FILE);
    if variables_path.exists() {
        let document: GlobalVariablesDocument = read_json(variables_path.as_path())?;
        project_data.variables.extend(document.variables);
    }

    Ok(project_data)
}

pub fn read_project_index(path: &str) -> Result<ProjectIndex, ProjectError> {
    let root = project_root_from_path(path);
    let manifest = read_project_manifest_from_root(root.as_path())?;
    let mut graphs = Vec::new();
    graphs.extend(read_graph_index_entries(
        root.as_path(),
        EVENTS_DIR,
        EVENT_EXTENSION,
        GraphDocumentKind::Event,
    )?);
    graphs.extend(read_graph_index_entries(
        root.as_path(),
        FUNCTIONS_DIR,
        FUNCTION_EXTENSION,
        GraphDocumentKind::Function,
    )?);

    Ok(ProjectIndex {
        project_name: manifest.project_name,
        app_version: manifest.app_version,
        export_time: manifest.export_time,
        graphs,
    })
}

pub fn load_project_graph_from_file(
    path: &str,
    graph_id: &GraphId,
) -> Result<GraphDocument, ProjectError> {
    let root = project_root_from_path(path);
    for path in list_graph_files(root.as_path(), EVENTS_DIR, EVENT_EXTENSION)? {
        let document = read_graph_document(path.as_path(), GraphDocumentKind::Event)?;
        if document.graph.id == *graph_id {
            return Ok(document);
        }
    }
    for path in list_graph_files(root.as_path(), FUNCTIONS_DIR, FUNCTION_EXTENSION)? {
        let document = read_graph_document(path.as_path(), GraphDocumentKind::Function)?;
        if document.graph.id == *graph_id {
            return Ok(document);
        }
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
    for (dir, extension, kind) in [
        (EVENTS_DIR, EVENT_EXTENSION, GraphDocumentKind::Event),
        (
            FUNCTIONS_DIR,
            FUNCTION_EXTENSION,
            GraphDocumentKind::Function,
        ),
    ] {
        for path in list_graph_files(root.as_path(), dir, extension)? {
            let document = read_graph_document(path.as_path(), kind)?;
            if document.graph.id == *graph_id {
                std::fs::remove_file(path)?;
                return Ok(Some(kind));
            }
        }
    }
    Ok(None)
}

fn read_project_manifest_from_root(root: &Path) -> Result<ProjectManifest, ProjectError> {
    let manifest_path = root.join(PROJECT_METADATA_FILE);
    if !manifest_path.exists() {
        return Err(ProjectError::FileNotFound(manifest_path));
    }
    read_json(manifest_path.as_path())
}

fn project_root_from_path(path: &str) -> PathBuf {
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
    let mut document: GraphDocument = read_json(path)?;
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
) -> Result<Vec<ProjectGraphIndexEntry>, ProjectError> {
    let mut entries = Vec::new();
    for path in list_graph_files(root, dir, extension)? {
        let document = read_graph_document(path.as_path(), expected_kind)?;
        entries.push(ProjectGraphIndexEntry {
            id: document.graph.id,
            name: document.graph.name,
            graph_type: expected_kind,
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
    for entry in std::fs::read_dir(graph_dir)? {
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
    let existing_path = find_graph_file_path(root, dir, extension, graph_id)?;
    let file_name = unique_graph_file_name(
        root.join(dir).as_path(),
        graph_name,
        extension,
        existing_path.as_deref(),
    );
    let next_path = root.join(dir).join(&file_name);
    if let Some(existing_path) = existing_path {
        if existing_path != next_path && existing_path.exists() {
            std::fs::remove_file(existing_path)?;
        }
    }
    Ok(format!("{dir}/{file_name}"))
}

fn find_graph_file_path(
    root: &Path,
    dir: &str,
    extension: &str,
    graph_id: &GraphId,
) -> Result<Option<PathBuf>, ProjectError> {
    for path in list_graph_files(root, dir, extension)? {
        let document: GraphDocument = read_json(path.as_path())?;
        if document.graph.id == *graph_id {
            return Ok(Some(path));
        }
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

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ProjectError> {
    let content = std::fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(ProjectError::Deserialize)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ProjectError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value).map_err(ProjectError::Serialize)?;
    std::fs::write(path, json)?;
    Ok(())
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
        assert!(root
            .join(EVENTS_DIR)
            .join(format!("Startup.{}", EVENT_EXTENSION))
            .is_file());
        assert!(root
            .join(FUNCTIONS_DIR)
            .join(format!("Compute.{}", FUNCTION_EXTENSION))
            .is_file());
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

        let event_doc =
            load_project_graph_from_file(root.to_string_lossy().as_ref(), &event.id).unwrap();
        assert_eq!(event_doc.kind, GraphDocumentKind::Event);
        assert_eq!(event_doc.graph.name, "Startup 1");
        assert_eq!(event_doc.local_variables.len(), 1);

        let function_doc =
            load_project_graph_from_file(root.to_string_lossy().as_ref(), &function.id).unwrap();
        assert_eq!(function_doc.kind, GraphDocumentKind::Function);
        assert_eq!(function_doc.local_variables.len(), 1);

        let _ = std::fs::remove_dir_all(root);
    }
}
