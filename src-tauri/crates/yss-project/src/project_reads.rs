use crate::{ProjectIndex, ProjectSession, ProjectState};
use yss_chart_document::{ChartDocument, ChartResourcePath};
use yss_function_editor_projection::FunctionEditorProjection;
use yss_project_filesystem::ProjectFilesystemError;
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::ResourceRevision;
use yss_project_model::ProjectData;

impl ProjectState {
    pub fn read_project_index(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
    ) -> Result<ProjectIndex, ProjectFilesystemError> {
        read_project_index_with(self, expected_project_instance_id, |root| {
            crate::project_io::read_project_index_from_root(root).map_err(read_error)
        })
    }

    pub fn load_chart_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        chart_path: &ChartResourcePath,
    ) -> Result<ChartDocument, ProjectFilesystemError> {
        let session = expected_session(self, expected_project_instance_id)?;
        let _lease = self.filesystem().acquire(session.root.clone())?;
        self.validate_project_session(&session)?;
        let (_, _, _, data) = self.coherent_project_read_snapshot(&session)?;
        let document = data.charts.get(chart_path).cloned().ok_or_else(|| {
            ProjectFilesystemError::ChartNotFound {
                path: chart_path.clone(),
            }
        })?;
        self.validate_project_session(&session)?;
        Ok(document)
    }
}

fn stale_project_lifecycle(message: impl Into<String>) -> ProjectFilesystemError {
    ProjectFilesystemError::StaleProjectLifecycle {
        message: message.into(),
    }
}

fn stale_catalog(message: impl Into<String>) -> ProjectFilesystemError {
    ProjectFilesystemError::CatalogResourceStale {
        message: message.into(),
    }
}

fn expected_session(
    state: &ProjectState,
    expected_project_instance_id: &ProjectInstanceId,
) -> Result<ProjectSession, ProjectFilesystemError> {
    let session = state.capture_project_session()?;
    if &session.instance_id != expected_project_instance_id {
        return Err(ProjectFilesystemError::StaleProjectLifecycle {
            message: format!(
                "requested project instance '{}' is no longer active",
                expected_project_instance_id
            ),
        });
    }
    Ok(session)
}

struct ProjectIndexAuthorityCapture {
    project_instance_id: String,
    publication_revision: u64,
    authority_generation: u64,
    history: yss_project_history::HistoryStatusDto,
    data: ProjectData,
    variable_revisions: std::collections::HashMap<
        yss_variable_contract::VariableId,
        crate::project_state::VariableRevisionEntry,
    >,
    database_revisions: std::collections::HashMap<String, u64>,
}

fn read_project_index_with(
    state: &ProjectState,
    expected_project_instance_id: &ProjectInstanceId,
    read: impl FnOnce(&std::path::Path) -> Result<ProjectIndex, ProjectFilesystemError>,
) -> Result<ProjectIndex, ProjectFilesystemError> {
    let session = expected_session(state, expected_project_instance_id)?;
    let _lease = state.filesystem().acquire(session.root.clone())?;
    state.validate_project_session(&session)?;
    let mut index = read(session.root.as_path())?;
    state.validate_project_session(&session)?;
    let capture = capture_project_index_authority(state, &session)?;
    overlay_authoritative_project_index(
        &capture.data,
        &capture.variable_revisions,
        &capture.database_revisions,
        &mut index,
    )?;
    index.project_instance_id = capture.project_instance_id.clone();
    index.publication_revision = capture.publication_revision;
    index.authority_generation = capture.authority_generation;
    index.history = capture.history;
    validate_project_index_authority(state, &session, &capture)?;
    Ok(index)
}

fn capture_project_index_authority(
    state: &ProjectState,
    session: &ProjectSession,
) -> Result<ProjectIndexAuthorityCapture, ProjectFilesystemError> {
    capture_project_index_authority_with(state, session, || {})
}

fn capture_project_index_authority_with(
    state: &ProjectState,
    session: &ProjectSession,
    after_declaration_capture: impl FnOnce(),
) -> Result<ProjectIndexAuthorityCapture, ProjectFilesystemError> {
    let publication = state.mutation_publication.lock().unwrap();
    if publication.project_instance_id != session.instance_id.as_str() {
        return Err(stale_project_lifecycle(
            "project changed before project index authority capture",
        ));
    }
    let data = state.project_data.read().unwrap().clone();
    after_declaration_capture();
    let variable_revisions = state.variable_revisions.read().unwrap().clone();
    let database_revisions = state.database_authority_revisions.read().unwrap().clone();
    if data.variables.keys().any(|id| {
        !variable_revisions
            .get(id)
            .is_some_and(|entry| entry.is_present())
    }) {
        return Err(stale_catalog(
            "loaded variable is missing its present revision authority",
        ));
    }
    if data
        .databases
        .keys()
        .any(|id| !database_revisions.contains_key(id))
    {
        return Err(stale_catalog(
            "loaded database is missing its revision authority",
        ));
    }
    Ok(ProjectIndexAuthorityCapture {
        project_instance_id: publication.project_instance_id.clone(),
        publication_revision: publication.resource_revision,
        authority_generation: publication.authority_generation(),
        history: state.history.read().unwrap().status(),
        data,
        variable_revisions,
        database_revisions,
    })
}

fn validate_project_index_authority(
    state: &ProjectState,
    session: &ProjectSession,
    capture: &ProjectIndexAuthorityCapture,
) -> Result<(), ProjectFilesystemError> {
    state.validate_project_session(session)?;
    let publication = state.mutation_publication.lock().unwrap();
    if publication.project_instance_id != capture.project_instance_id {
        return Err(stale_project_lifecycle(
            "project changed before project index publication",
        ));
    }
    if publication.resource_revision != capture.publication_revision
        || publication.authority_generation() != capture.authority_generation
    {
        return Err(stale_catalog(
            "project index authority changed before publication",
        ));
    }
    Ok(())
}

fn variable_owner_graph_path(
    scope: &yss_variable_contract::VariableScope,
) -> Option<yss_graph_document::GraphResourcePath> {
    match scope {
        yss_variable_contract::VariableScope::Global => None,
        yss_variable_contract::VariableScope::Event { event_path } => {
            yss_graph_document::GraphResourcePath::new(event_path).ok()
        }
        yss_variable_contract::VariableScope::Function { function_path } => {
            yss_graph_document::GraphResourcePath::new(function_path).ok()
        }
    }
}

fn overlay_authoritative_project_index(
    data: &ProjectData,
    variable_revisions: &std::collections::HashMap<
        yss_variable_contract::VariableId,
        crate::project_state::VariableRevisionEntry,
    >,
    database_revisions: &std::collections::HashMap<String, u64>,
    index: &mut ProjectIndex,
) -> Result<(), ProjectFilesystemError> {
    let mut variables = std::collections::BTreeMap::new();
    for mut variable in std::mem::take(&mut index.variables) {
        if matches!(variable.scope, yss_variable_contract::VariableScope::Global) {
            continue;
        }
        let retained = uuid::Uuid::parse_str(&variable.id)
            .ok()
            .map(yss_variable_contract::VariableId::from)
            .and_then(|id| variable_revisions.get(&id).copied());
        match retained {
            Some(entry) if entry.is_present() => variable.revision = entry.revision,
            Some(_) => continue,
            None => {}
        }
        variables.insert(variable.id.clone(), variable);
    }
    for variable in data.variables.values() {
        let authority = variable_revisions.get(&variable.id).ok_or_else(|| {
            ProjectFilesystemError::ResourceRevisionConflict {
                message: format!(
                    "Variable '{}' is present in project data but missing revision authority",
                    variable.id
                ),
            }
        })?;
        let mut entry = crate::ProjectVariableIndexEntry::from(variable.clone());
        entry.revision = authority.revision;
        if let Some(persisted) = variables.get(&entry.id) {
            entry.owner_graph_path = persisted.owner_graph_path.clone();
            entry.owner_graph_name = persisted.owner_graph_name.clone();
            entry.owner_graph_kind = persisted.owner_graph_kind;
        } else if let Some(path) = variable_owner_graph_path(&variable.scope) {
            entry.owner_graph_path = Some(path.as_str().to_string());
            if let Some(graph) = data.graphs.get(&path) {
                entry.owner_graph_name = Some(graph.name.clone());
                entry.owner_graph_kind = Some(graph.kind);
            }
        }
        variables.insert(entry.id.clone(), entry);
    }
    index.variables = variables.into_values().collect();
    index.variables.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });
    index.databases = data
        .databases
        .iter()
        .map(|(id, declaration)| crate::ProjectDatabaseIndexEntry {
            id: id.clone(),
            resource_path: yss_project_identity::ProjectResourcePath::new(format!(
                "databases/{id}"
            )),
            revision: yss_project_identity::ResourceRevision::new(database_revisions[id]),
            engine: declaration.engine.clone(),
            schema_version: declaration.schema_version,
            required: declaration.required,
            name: Some(declaration.name.to_string()),
        })
        .collect();
    index
        .databases
        .sort_by(|left, right| left.id.cmp(&right.id));
    index.charts = data
        .charts
        .iter()
        .map(|(path, chart)| crate::ProjectChartIndexEntry {
            chart_path: path.clone(),
            name: path.display_name().as_str().to_string(),
            database_id: chart.database_id.clone(),
            chart_type: chart.chart_type.clone(),
            revision: chart.revision,
        })
        .collect();
    index.charts.sort_by_key(|entry| entry.name.to_lowercase());
    for entry in &mut index.graphs {
        let Ok(path) = yss_graph_document::GraphResourcePath::new(&entry.path) else {
            continue;
        };
        let Some(resource) = data.graphs.get(&path) else {
            continue;
        };
        entry.revision = ResourceRevision::from_graph_revision(resource.document.revision);
        if let Some(function) = resource.function.as_ref() {
            entry.function_revision = Some(function.revision);
            entry.function_signature = Some(function.signature.clone());
            entry.function_editor_projection = Some(
                FunctionEditorProjection::try_from(function).map_err(|message| {
                    ProjectFilesystemError::TransactionPrepareFailed {
                        message: message.to_string(),
                    }
                })?,
            );
        }
    }
    Ok(())
}

fn read_error(error: crate::ProjectError) -> ProjectFilesystemError {
    ProjectFilesystemError::TransactionPrepareFailed {
        message: error.to_string(),
    }
}
