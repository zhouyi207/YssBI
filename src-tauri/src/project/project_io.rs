use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    PROJECT_METADATA_FILE, ProjectData, ProjectError, ProjectWorksheetIndexEntry,
    ensure_worksheets_dir, read_worksheet_index_entries,
};
use crate::database::{DatabaseDecl, DatabaseEngine};
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
    pub folder_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFolderIndexEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub graph_type: GraphDocumentKind,
    pub folder_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIndex {
    pub project_name: String,
    pub app_version: String,
    pub export_time: String,
    pub graphs: Vec<ProjectGraphIndexEntry>,
    pub folders: Vec<ProjectFolderIndexEntry>,
    #[serde(default)]
    pub worksheets: Vec<ProjectWorksheetIndexEntry>,
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
    let mut folders = Vec::new();
    folders.extend(read_folder_index_entries(
        root.as_path(),
        EVENTS_DIR,
        GraphDocumentKind::Event,
    )?);
    folders.extend(read_folder_index_entries(
        root.as_path(),
        FUNCTIONS_DIR,
        GraphDocumentKind::Function,
    )?);
    let worksheets = read_worksheet_index_entries(root.as_path())?;

    Ok(ProjectIndex {
        project_name: manifest.project_name,
        app_version: manifest.app_version,
        export_time: manifest.export_time,
        graphs,
        folders,
        worksheets,
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

pub fn create_project_graph_folder(
    path: &str,
    kind: GraphDocumentKind,
    folder_path: &str,
) -> Result<String, ProjectError> {
    let root = project_root_from_path(path);
    let graph_dir = root.join(graph_dir_for_kind(kind));
    let normalized = normalize_folder_path(folder_path);
    if normalized.as_os_str().is_empty() {
        return Err(ProjectError::InvalidProjectFormat(
            "folder path cannot be empty".into(),
        ));
    }
    std::fs::create_dir_all(graph_dir.join(&normalized))?;
    Ok(path_to_slash_string(normalized.as_path()))
}

pub fn rename_project_graph_folder(
    path: &str,
    kind: GraphDocumentKind,
    folder_path: &str,
    new_name: &str,
) -> Result<String, ProjectError> {
    let root = project_root_from_path(path);
    let graph_dir = root.join(graph_dir_for_kind(kind));
    let old_relative = normalize_folder_path(folder_path);
    let old_path = graph_dir.join(&old_relative);
    if old_relative.as_os_str().is_empty() || !old_path.is_dir() {
        return Err(ProjectError::FileNotFound(old_path));
    }

    let parent = old_relative
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let new_relative = parent.join(sanitize_file_stem(new_name));
    let new_path = graph_dir.join(&new_relative);
    if new_path.exists() {
        return Err(ProjectError::InvalidProjectFormat(format!(
            "folder '{}' already exists",
            new_relative.to_string_lossy()
        )));
    }
    std::fs::rename(old_path, &new_path)?;
    Ok(path_to_slash_string(new_relative.as_path()))
}

pub fn delete_project_graph_folder(
    path: &str,
    kind: GraphDocumentKind,
    folder_path: &str,
) -> Result<(), ProjectError> {
    let root = project_root_from_path(path);
    let graph_dir = root.join(graph_dir_for_kind(kind));
    let relative = normalize_folder_path(folder_path);
    let target = graph_dir.join(&relative);
    if relative.as_os_str().is_empty() || !target.exists() {
        return Ok(());
    }
    if target.is_dir() {
        std::fs::remove_dir_all(target)?;
    }
    Ok(())
}

pub fn move_project_graph_to_folder(
    path: &str,
    graph_id: &GraphId,
    folder_path: &str,
) -> Result<String, ProjectError> {
    let root = project_root_from_path(path);
    let (current_path, kind, document) = find_graph_document_path(root.as_path(), graph_id)?
        .ok_or_else(|| {
            ProjectError::InvalidProjectFormat(format!("graph '{}' not found", graph_id))
        })?;
    let graph_dir = root.join(graph_dir_for_kind(kind));
    let target_folder = normalize_folder_path(folder_path);
    let target_dir = graph_dir.join(&target_folder);
    std::fs::create_dir_all(&target_dir)?;
    let file_name = unique_graph_file_name(
        target_dir.as_path(),
        &document.graph.name,
        graph_extension_for_kind(kind),
        None,
    );
    let target_path = target_dir.join(file_name);
    if current_path != target_path {
        std::fs::rename(current_path, &target_path)?;
    }
    Ok(folder_path_from_graph_file(
        root.as_path(),
        graph_dir_for_kind(kind),
        target_path.as_path(),
    ))
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
    let names: Vec<String> = read_graph_index_entries(
        root.as_path(),
        graph_dir_for_kind(kind),
        graph_extension_for_kind(kind),
        kind,
    )?
    .into_iter()
    .map(|entry| entry.name)
    .collect();
    document.graph.id = GraphId::new();
    document.graph.name = crate::project::unique_name::unique_name(&document.graph.name, names);
    document.local_variables.clear();
    let file_name = unique_graph_file_name(
        source_dir,
        &document.graph.name,
        graph_extension_for_kind(kind),
        None,
    );
    write_json(source_dir.join(file_name).as_path(), &document)?;
    Ok(document)
}

fn read_project_manifest_from_root(root: &Path) -> Result<ProjectManifest, ProjectError> {
    let manifest_path = root.join(PROJECT_METADATA_FILE);
    if !manifest_path.exists() {
        return Err(ProjectError::FileNotFound(manifest_path));
    }
    read_json(manifest_path.as_path())
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

/// 仅读取当前图文件头部（`graph.id` / `graph.name`）。
/// 用于索引与按 id 查找，避免对每个文件做完整反序列化。
#[derive(Deserialize)]
struct GraphFileHeader {
    graph: GraphFileHeaderGraph,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphFileHeaderGraph {
    id: GraphId,
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
) -> Result<Vec<ProjectGraphIndexEntry>, ProjectError> {
    let mut entries = Vec::new();
    let mut seen_ids = HashSet::new();
    for path in list_graph_files(root, dir, extension)? {
        let mut header = match read_graph_file_header(path.as_path()) {
            Ok(header) => header,
            Err(_) => continue,
        };
        if seen_ids.contains(&header.graph.id) {
            header = match repair_copied_graph_document(path.as_path(), expected_kind, &seen_ids) {
                Ok(header) => header,
                Err(_) => continue,
            };
        }
        seen_ids.insert(header.graph.id);
        let name = graph_name_from_file_path(path.as_path()).unwrap_or(header.graph.name);
        entries.push(ProjectGraphIndexEntry {
            id: header.graph.id,
            name,
            graph_type: expected_kind,
            folder_path: folder_path_from_graph_file(root, dir, path.as_path()),
        });
    }
    Ok(entries)
}

fn repair_copied_graph_document(
    path: &Path,
    expected_kind: GraphDocumentKind,
    existing_ids: &HashSet<GraphId>,
) -> Result<GraphFileHeader, ProjectError> {
    let mut document = read_graph_document(path, expected_kind)?;
    while existing_ids.contains(&document.graph.id) {
        document.graph.id = GraphId::new();
    }
    document.local_variables.clear();
    write_json(path, &document)?;
    Ok(GraphFileHeader {
        graph: GraphFileHeaderGraph {
            id: document.graph.id,
            name: document.graph.name,
        },
    })
}

fn read_folder_index_entries(
    root: &Path,
    dir: &str,
    kind: GraphDocumentKind,
) -> Result<Vec<ProjectFolderIndexEntry>, ProjectError> {
    let graph_dir = root.join(dir);
    if !graph_dir.exists() {
        return Ok(Vec::new());
    }
    let mut folders = Vec::new();
    collect_graph_folders(graph_dir.as_path(), graph_dir.as_path(), kind, &mut folders)?;
    folders.sort_by(|a, b| a.folder_path.cmp(&b.folder_path));
    Ok(folders)
}

fn list_graph_files(root: &Path, dir: &str, extension: &str) -> Result<Vec<PathBuf>, ProjectError> {
    let graph_dir = root.join(dir);
    if !graph_dir.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    collect_graph_files(graph_dir.as_path(), extension, &mut paths)?;
    paths.sort_by_key(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });
    Ok(paths)
}

fn collect_graph_files(
    dir: &Path,
    extension: &str,
    paths: &mut Vec<PathBuf>,
) -> Result<(), ProjectError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_graph_files(path.as_path(), extension, paths)?;
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

fn collect_graph_folders(
    root_dir: &Path,
    dir: &Path,
    kind: GraphDocumentKind,
    folders: &mut Vec<ProjectFolderIndexEntry>,
) -> Result<(), ProjectError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let folder_path = path
            .strip_prefix(root_dir)
            .map(path_to_slash_string)
            .unwrap_or_default();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        folders.push(ProjectFolderIndexEntry {
            name,
            graph_type: kind,
            folder_path,
        });
        collect_graph_folders(root_dir, path.as_path(), kind, folders)?;
    }
    Ok(())
}

fn graph_relative_path_for_save(
    root: &Path,
    dir: &str,
    extension: &str,
    graph_name: &str,
    graph_id: &GraphId,
) -> Result<String, ProjectError> {
    let existing_path = find_graph_file_path(root, dir, extension, graph_id)?;
    let target_dir = existing_path
        .as_ref()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| root.join(dir));
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
    extension: &str,
    graph_id: &GraphId,
) -> Result<Option<PathBuf>, ProjectError> {
    for path in list_graph_files(root, dir, extension)? {
        let header = read_graph_file_header(path.as_path())?;
        if header.graph.id == *graph_id {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

pub(crate) fn find_graph_document_path(
    root: &Path,
    graph_id: &GraphId,
) -> Result<Option<(PathBuf, GraphDocumentKind, GraphDocument)>, ProjectError> {
    for (dir, extension, kind) in [
        (EVENTS_DIR, EVENT_EXTENSION, GraphDocumentKind::Event),
        (
            FUNCTIONS_DIR,
            FUNCTION_EXTENSION,
            GraphDocumentKind::Function,
        ),
    ] {
        for path in list_graph_files(root, dir, extension)? {
            let document = read_graph_document(path.as_path(), kind)?;
            if document.graph.id == *graph_id {
                return Ok(Some((path, kind, document)));
            }
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

fn normalize_folder_path(folder_path: &str) -> PathBuf {
    folder_path
        .replace('\\', "/")
        .split('/')
        .filter_map(|segment| {
            let sanitized = sanitize_file_stem(segment);
            if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
                None
            } else {
                Some(sanitized)
            }
        })
        .collect()
}

fn folder_path_from_graph_file(root: &Path, dir: &str, path: &Path) -> String {
    let graph_dir = root.join(dir);
    path.parent()
        .and_then(|parent| parent.strip_prefix(graph_dir).ok())
        .map(path_to_slash_string)
        .unwrap_or_default()
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
        let (a_output, a_operand_input, b_operand_input, total_pins, dynamic_pin) = {
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
            let dynamic_pin = a_pins
                .iter()
                .find(|p| p.definition.should_persist_full_definition() && p.is_data())
                .map(|p| p.id)
                .expect("a has a dynamic operand pin");
            (
                a_output,
                a_operand_input,
                b_operand_input,
                total_pins,
                dynamic_pin,
            )
        };

        // 设置一个 userValue，并建立一条连接
        {
            let mut ds = graph.data_state.write().unwrap();
            ds.pins.get_mut(&a_operand_input).unwrap().user_value = Some(DataValue::Int64(7));
            ds.connections.connect(a_output, b_operand_input);
        }

        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();

        // 重新从磁盘加载（新格式），与原图对比
        let doc = load_project_graph_from_file(root.to_string_lossy().as_ref(), &event.id).unwrap();
        let loaded = doc.graph;
        let lds = loaded.data_state.read().unwrap();

        assert_eq!(lds.nodes.len(), 2, "node count round-trips");
        assert_eq!(lds.pins.len(), total_pins, "pin count round-trips");

        // userValue 保留
        assert_eq!(
            lds.pins.get(&a_operand_input).unwrap().user_value,
            Some(DataValue::Int64(7)),
            "user value round-trips"
        );

        // 连接保留（pin id 不变）
        assert!(
            lds.connections
                .all_connections()
                .iter()
                .any(|c| c.from_pin == a_output && c.to_pin == b_operand_input),
            "connection round-trips"
        );

        // 动态/可重复 pin 保留完整定义（data_type 非空）；静态输出 pin 仅留契约（data_type 为空）
        assert!(
            lds.pins
                .get(&dynamic_pin)
                .unwrap()
                .definition
                .data_type
                .is_some(),
            "dynamic operand pin keeps its full definition override"
        );
        let output_pin = lds.pins.get(&a_output).unwrap();
        assert_eq!(output_pin.definition.kind, PinKind::Data);
        assert!(
            output_pin.definition.data_type.is_none(),
            "static pin persists only a contract; full definition re-attached from registry"
        );

        drop(lds);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn scans_and_manages_graph_folders_recursively() {
        let root = temp_project_dir();
        let state = ProjectState::new();
        let event = state.add_event("Nested Event");
        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();

        create_project_graph_folder(
            root.to_string_lossy().as_ref(),
            GraphDocumentKind::Event,
            "Folder A/Sub",
        )
        .unwrap();
        move_project_graph_to_folder(root.to_string_lossy().as_ref(), &event.id, "Folder A/Sub")
            .unwrap();

        let index = read_project_index(root.to_string_lossy().as_ref()).unwrap();
        assert!(
            index
                .folders
                .iter()
                .any(|folder| folder.folder_path == "Folder A")
        );
        assert!(
            index
                .folders
                .iter()
                .any(|folder| folder.folder_path == "Folder A/Sub")
        );
        let nested = index
            .graphs
            .iter()
            .find(|graph| graph.id == event.id)
            .unwrap();
        assert_eq!(nested.folder_path, "Folder A/Sub");

        let renamed = rename_project_graph_folder(
            root.to_string_lossy().as_ref(),
            GraphDocumentKind::Event,
            "Folder A/Sub",
            "Renamed",
        )
        .unwrap();
        assert_eq!(renamed, "Folder A/Renamed");
        let index = read_project_index(root.to_string_lossy().as_ref()).unwrap();
        assert!(
            index
                .graphs
                .iter()
                .any(|graph| graph.folder_path == "Folder A/Renamed")
        );

        delete_project_graph_folder(
            root.to_string_lossy().as_ref(),
            GraphDocumentKind::Event,
            "Folder A",
        )
        .unwrap();
        let index = read_project_index(root.to_string_lossy().as_ref()).unwrap();
        assert!(index.graphs.is_empty());
        assert!(index.folders.is_empty());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn read_project_index_skips_invalid_graph_files() {
        let root = temp_project_dir();
        let state = ProjectState::new();
        let event = state.add_event("Valid Event");
        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();
        std::fs::write(
            root.join(EVENTS_DIR)
                .join(format!("Broken.{}", EVENT_EXTENSION)),
            "",
        )
        .unwrap();

        let index = read_project_index(root.to_string_lossy().as_ref()).unwrap();

        assert_eq!(index.graphs.len(), 1);
        assert!(index.graphs.iter().any(|graph| graph.id == event.id));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn read_project_index_repairs_copied_graph_files_with_duplicate_ids() {
        let root = temp_project_dir();
        let state = ProjectState::new();
        let event = state.add_event("Copied Event");
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
        assert!(ids.contains(&event.id));
        let original_doc =
            read_graph_document(original_path.as_path(), GraphDocumentKind::Event).unwrap();
        let copied_doc =
            read_graph_document(copied_path.as_path(), GraphDocumentKind::Event).unwrap();
        assert_ne!(original_doc.graph.id, copied_doc.graph.id);
        assert!(ids.contains(&original_doc.graph.id));
        assert!(ids.contains(&copied_doc.graph.id));
        assert!(copied_doc.local_variables.is_empty());

        let _ = std::fs::remove_dir_all(root);
    }
}
