#[cfg(test)]
use crate::database::DatabaseInstance;
#[cfg(test)]
pub use crate::graph::compatibility::{CatalogMutationResource, CatalogMutationValidationSnapshot};
#[cfg(test)]
use crate::project::FunctionSignature;
use crate::project::ResourceRevision;
use crate::project::{
    ProjectData, ProjectFilesystemError, ProjectIndex, ProjectInstanceId, ProjectSession,
    ProjectState, WorksheetDocument, WorksheetResourcePath,
};
#[cfg(test)]
use std::collections::HashMap;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
#[cfg(test)]
use yss_database_contract::DatabaseDecl;
#[cfg(test)]
use yss_graph_catalog::{
    BuiltinCatalog, CatalogResourceEntry, CatalogResourcePath, ResourceBoundCreateArgs,
};
#[cfg(test)]
use yss_graph_protocol::NodeTypeId;
#[cfg(test)]
use yss_graph_registry::NodeRegistry;
use yss_variable_contract::VariableScope;
#[cfg(test)]
use yss_variable_contract::{VariableId, VariableInstance};

#[cfg(test)]
#[derive(Clone)]
pub struct ProjectResourceSnapshot {
    pub project_instance_id: ProjectInstanceId,
    pub authority_generation: u64,
    pub databases: HashMap<String, DatabaseDecl>,
    pub variables: HashMap<VariableId, VariableInstance>,
    pub runtime_databases: HashMap<String, DatabaseInstance>,
}

#[derive(Debug)]
#[cfg(test)]
pub struct CatalogProjectSnapshot {
    pub project_instance_id: ProjectInstanceId,
    pub resource_publication_revision: u64,
    pub registry: Arc<NodeRegistry>,
    pub catalog: Arc<BuiltinCatalog>,
    pub resources: Vec<CatalogResourceEntry>,
    pub validation: CatalogMutationValidationSnapshot,
    pub(crate) authority_generation: u64,
}

#[cfg(test)]
struct CatalogCapture {
    project_instance_id: ProjectInstanceId,
    resource_publication_revision: u64,
    authority_generation: u64,
    registry: Arc<NodeRegistry>,
    catalog: Arc<BuiltinCatalog>,
    index: ProjectIndex,
    data: ProjectData,
    loaded_variable_revisions: std::collections::HashMap<
        yss_variable_contract::VariableId,
        crate::project::project_state::VariableRevisionEntry,
    >,
    database_authority_revisions: std::collections::HashMap<String, u64>,
    runtime_database_ids: BTreeSet<String>,
}

impl ProjectState {
    #[cfg(test)]
    pub fn project_resource_snapshot(
        &self,
    ) -> Result<ProjectResourceSnapshot, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let publication = self.mutation_publication.lock().unwrap();
        self.ensure_project_operational()?;
        let (databases, variables) = {
            let data = self.project_data.read().unwrap();
            (data.databases.clone(), data.variables.clone())
        };
        let runtime_databases = self.project_store.read().unwrap().databases.clone();
        Ok(ProjectResourceSnapshot {
            project_instance_id: ProjectInstanceId::from_existing(
                publication.project_instance_id.clone(),
            ),
            authority_generation: publication.authority_generation(),
            databases,
            variables,
            runtime_databases,
        })
    }

    pub fn read_project_index(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
    ) -> Result<ProjectIndex, ProjectFilesystemError> {
        read_project_index_with(self, expected_project_instance_id, |root| {
            crate::project::project_io::read_project_index_from_root(root).map_err(read_error)
        })
    }

    #[cfg(test)]
    pub fn catalog_snapshot(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
    ) -> Result<CatalogProjectSnapshot, ProjectFilesystemError> {
        catalog_snapshot_with_reader(self, expected_project_instance_id, |root| {
            crate::project::project_io::read_project_index_from_root(root).map_err(read_error)
        })
    }

    #[cfg(test)]
    pub fn loaded_graph_document_for_catalog(
        &self,
        snapshot: &CatalogProjectSnapshot,
        graph_path: &yss_graph_document::GraphResourcePath,
    ) -> Result<Option<yss_graph_document::GraphDocument>, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != snapshot.project_instance_id.as_str()
            || publication.authority_generation() != snapshot.authority_generation
        {
            return Err(stale_catalog(
                "catalog authority changed before loaded graph capture",
            ));
        }
        let document = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .get(graph_path)
            .map(|resource| resource.document.clone());
        self.ensure_project_operational()?;
        Ok(document)
    }

    #[cfg(test)]
    pub fn catalog_mutation_validation_snapshot(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
    ) -> Result<CatalogMutationValidationSnapshot, ProjectFilesystemError> {
        let capture = capture_catalog_with_reader(self, expected_project_instance_id, |root| {
            crate::project::project_io::read_project_index_from_root(root).map_err(read_error)
        })?;
        Ok(build_catalog_snapshots(capture).1)
    }

    pub fn load_worksheet_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        worksheet_path: &WorksheetResourcePath,
    ) -> Result<WorksheetDocument, ProjectFilesystemError> {
        let session = expected_session(self, expected_project_instance_id)?;
        let _lease = self.filesystem().acquire(session.root.clone())?;
        self.validate_project_session(&session)?;
        let (_, _, _, data) = self.coherent_project_read_snapshot(&session)?;
        let document = data
            .worksheets
            .get(worksheet_path)
            .cloned()
            .ok_or_else(|| ProjectFilesystemError::WorksheetNotFound {
                path: worksheet_path.clone(),
            })?;
        self.validate_project_session(&session)?;
        Ok(document)
    }
}

#[cfg(test)]
fn catalog_snapshot_with_reader(
    state: &ProjectState,
    expected_project_instance_id: &ProjectInstanceId,
    read: impl FnOnce(&std::path::Path) -> Result<ProjectIndex, ProjectFilesystemError>,
) -> Result<CatalogProjectSnapshot, ProjectFilesystemError> {
    let capture = capture_catalog_with_reader(state, expected_project_instance_id, read)?;
    Ok(build_catalog_snapshots(capture).0)
}

#[cfg(test)]
fn capture_catalog_with_reader(
    state: &ProjectState,
    expected_project_instance_id: &ProjectInstanceId,
    read: impl FnOnce(&std::path::Path) -> Result<ProjectIndex, ProjectFilesystemError>,
) -> Result<CatalogCapture, ProjectFilesystemError> {
    let session = expected_session(state, expected_project_instance_id)?;
    let (resource_publication_revision, authority_generation) = {
        let publication = state.mutation_publication.lock().unwrap();
        if publication.project_instance_id != session.instance_id.as_str() {
            return Err(stale_project_lifecycle(
                "project changed before Catalog capture",
            ));
        }
        (
            publication.resource_revision,
            publication.authority_generation(),
        )
    };

    let capture = {
        let _lease = state.filesystem().acquire(session.root.clone())?;
        state.validate_project_session(&session)?;
        let index = read(session.root.as_path())?;
        state.validate_project_session(&session)?;

        let publication = state.mutation_publication.lock().unwrap();
        if publication.project_instance_id != session.instance_id.as_str() {
            return Err(stale_project_lifecycle(
                "project changed while reading Catalog resources",
            ));
        }
        if publication.resource_revision != resource_publication_revision
            || publication.authority_generation() != authority_generation
        {
            return Err(stale_catalog(
                "Catalog authority changed while reading resources",
            ));
        }
        let data = state.project_data.read().unwrap().clone();
        let store = state.project_store.read().unwrap();
        let registry = Arc::clone(&store.node_registry);
        let catalog = Arc::clone(&store.catalog);
        let runtime_database_ids = data.databases.keys().cloned().collect::<BTreeSet<_>>();
        let loaded_variable_revisions = state.variable_revisions.read().unwrap().clone();
        let database_authority_revisions =
            state.database_authority_revisions.read().unwrap().clone();
        if data.variables.keys().any(|id| {
            !loaded_variable_revisions
                .get(id)
                .is_some_and(|entry| entry.is_present())
        }) {
            return Err(stale_catalog(
                "loaded variable is missing its authoritative revision",
            ));
        }
        if data.databases.keys().any(|id| {
            !runtime_database_ids.contains(id) || !database_authority_revisions.contains_key(id)
        }) {
            return Err(stale_catalog(
                "database is missing runtime or revision authority",
            ));
        }
        CatalogCapture {
            project_instance_id: session.instance_id.clone(),
            resource_publication_revision,
            authority_generation,
            registry,
            catalog,
            index,
            data,
            loaded_variable_revisions,
            database_authority_revisions,
            runtime_database_ids,
        }
    };

    state.validate_project_session(&session)?;
    let publication = state.mutation_publication.lock().unwrap();
    if publication.project_instance_id != session.instance_id.as_str() {
        return Err(stale_project_lifecycle(
            "project changed before Catalog snapshot publication",
        ));
    }
    if publication.resource_revision != resource_publication_revision
        || publication.authority_generation() != authority_generation
    {
        return Err(stale_catalog(
            "Catalog authority changed before snapshot publication",
        ));
    }
    drop(publication);
    Ok(capture)
}

#[cfg(test)]
fn build_catalog_snapshots(
    capture: CatalogCapture,
) -> (CatalogProjectSnapshot, CatalogMutationValidationSnapshot) {
    let CatalogCapture {
        project_instance_id,
        resource_publication_revision,
        authority_generation,
        registry,
        catalog,
        index,
        data,
        loaded_variable_revisions,
        database_authority_revisions,
        runtime_database_ids,
    } = capture;
    let mut resources = Vec::new();
    let mut validation_resources = BTreeMap::new();

    let mut functions = index
        .graphs
        .into_iter()
        .filter_map(|entry| {
            let revision = entry.function_revision?;
            let signature = entry.function_signature?;
            Some((entry.path, (entry.name, revision, signature)))
        })
        .collect::<BTreeMap<_, _>>();
    for (path, resource) in &data.graphs {
        let Some(function) = resource.function.as_ref() else {
            continue;
        };
        functions.insert(
            path.as_str().to_string(),
            (
                resource.name.clone(),
                function.revision,
                function.signature.clone(),
            ),
        );
    }
    for (path, (name, revision, signature)) in functions {
        let resource_path = CatalogResourcePath::new(path);
        let node_type_id = node_type("yssbi.project.function.call");
        resources.push(CatalogResourceEntry {
            name: name.into(),
            node_type_id: node_type_id.clone(),
            resource_path: resource_path.clone(),
            resource_revision: revision.get(),
            create_args: ResourceBoundCreateArgs::Function,
            technical_terms: vec!["call".into(), "function".into()],
        });
        validation_resources.insert(
            resource_path,
            CatalogMutationResource::Function {
                revision: revision.get(),
                signature: crate::graph::compatibility::CatalogFunctionSignature {
                    parameters: signature
                        .parameters
                        .into_iter()
                        .map(
                            |parameter| crate::graph::compatibility::CatalogFunctionParameter {
                                id: parameter.id,
                                name: parameter.name,
                                type_name: parameter.type_name,
                            },
                        )
                        .collect(),
                    return_type: signature.return_type,
                },
                allowed_node_type_id: node_type_id,
                parameter_binding: "target".into(),
            },
        );
    }

    let loaded_graph_paths = data
        .graphs
        .keys()
        .map(|path| path.as_str().to_string())
        .collect::<BTreeSet<_>>();
    let mut variables = index
        .variables
        .into_iter()
        .filter(|entry| {
            !matches!(entry.scope, VariableScope::Global)
                && entry
                    .owner_graph_path
                    .as_ref()
                    .is_none_or(|path| !loaded_graph_paths.contains(path))
        })
        .filter_map(|entry| {
            let retained = uuid::Uuid::parse_str(&entry.id)
                .ok()
                .map(yss_variable_contract::VariableId::from)
                .and_then(|id| loaded_variable_revisions.get(&id).copied());
            let revision = match retained {
                Some(entry) if entry.is_present() => entry.revision,
                Some(_) => return None,
                None => entry.revision,
            };
            Some((
                format!("variables/{}", entry.id),
                (entry.name, revision, entry.scope, entry.data_type),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    for variable in data.variables.values() {
        let revision = loaded_variable_revisions[&variable.id].revision;
        variables.insert(
            format!("variables/{}", variable.id),
            (
                variable.name.clone(),
                revision,
                variable.scope.clone(),
                variable.data_type.clone(),
            ),
        );
    }
    for (path, (name, revision, scope, data_type)) in variables {
        let resource_path = CatalogResourcePath::new(path);
        let get = node_type("yssbi.project.variable.get");
        let set = node_type("yssbi.project.variable.set");
        for node_type_id in [get.clone(), set.clone()] {
            resources.push(CatalogResourceEntry {
                name: name.clone().into(),
                node_type_id,
                resource_path: resource_path.clone(),
                resource_revision: revision.get(),
                create_args: ResourceBoundCreateArgs::Variable,
                technical_terms: vec!["variable".into()],
            });
        }
        validation_resources.insert(
            resource_path,
            CatalogMutationResource::Variable {
                revision: revision.get(),
                scope,
                data_type,
                allowed_node_type_ids: [get, set],
                parameter_binding: "variable".into(),
            },
        );
    }

    for (id, declaration) in data.databases {
        debug_assert!(runtime_database_ids.contains(&id));
        let authority_revision = database_authority_revisions[&id];
        let resource_path = CatalogResourcePath::new(format!("databases/{id}"));
        let node_type_id = node_type("yssbi.dataframe.source.get");
        resources.push(CatalogResourceEntry {
            name: declaration.name.into(),
            node_type_id: node_type_id.clone(),
            resource_path: resource_path.clone(),
            resource_revision: authority_revision,
            create_args: ResourceBoundCreateArgs::Database,
            technical_terms: vec!["dataframe".into(), "database".into()],
        });
        validation_resources.insert(
            resource_path,
            CatalogMutationResource::Database {
                authority_revision,
                allowed_node_type_id: node_type_id,
                parameter_binding: "dataframe".into(),
            },
        );
    }

    resources.sort_by(|left, right| {
        left.resource_path
            .cmp(&right.resource_path)
            .then_with(|| left.node_type_id.as_str().cmp(right.node_type_id.as_str()))
    });
    let validation = CatalogMutationValidationSnapshot {
        authority_generation,
        resources: validation_resources,
    };
    (
        CatalogProjectSnapshot {
            project_instance_id,
            resource_publication_revision,
            registry,
            catalog,
            resources,
            validation: validation.clone(),
            authority_generation,
        },
        validation,
    )
}

#[cfg(test)]
fn node_type(value: &'static str) -> NodeTypeId {
    NodeTypeId::new(value).expect("built-in resource node type ID")
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
    history: crate::project::HistoryStatusDto,
    data: ProjectData,
    variable_revisions: std::collections::HashMap<
        yss_variable_contract::VariableId,
        crate::project::project_state::VariableRevisionEntry,
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

#[cfg(test)]
fn capture_project_index_authority_with_test_hook(
    state: &ProjectState,
    session: &ProjectSession,
    after_declaration_capture: impl FnOnce(),
) -> Result<ProjectIndexAuthorityCapture, ProjectFilesystemError> {
    capture_project_index_authority_with(state, session, after_declaration_capture)
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
        crate::project::project_state::VariableRevisionEntry,
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
        let authority = variable_revisions[&variable.id];
        let mut entry = crate::project::ProjectVariableIndexEntry::from(variable.clone());
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
        .map(
            |(id, declaration)| crate::project::ProjectDatabaseIndexEntry {
                id: id.clone(),
                resource_path: crate::project::ProjectResourcePath::new(format!("databases/{id}")),
                revision: crate::project::ResourceRevision::new(database_revisions[id]),
                engine: declaration.engine.clone(),
                schema_version: declaration.schema_version,
                required: declaration.required,
                name: Some(declaration.name.to_string()),
            },
        )
        .collect();
    index
        .databases
        .sort_by(|left, right| left.id.cmp(&right.id));
    index.worksheets = data
        .worksheets
        .iter()
        .map(
            |(path, worksheet)| crate::project::ProjectWorksheetIndexEntry {
                worksheet_path: path.clone(),
                name: path.display_name().as_str().to_string(),
                database_id: worksheet.database_id.clone(),
                chart_type: worksheet.chart_type.clone(),
                revision: worksheet.revision,
            },
        )
        .collect();
    index
        .worksheets
        .sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
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
                crate::project::build_function_editor_projection(
                    function.revision.get(),
                    function.signature.parameters.iter().map(|parameter| {
                        (
                            parameter.id.clone(),
                            parameter.name.clone(),
                            parameter.type_name.clone(),
                        )
                    }),
                    function.signature.return_type.clone(),
                )
                .map_err(|message| {
                    ProjectFilesystemError::TransactionPrepareFailed {
                        message: message.to_string(),
                    }
                })?,
            );
        }
    }
    Ok(())
}

fn read_error(error: crate::project::ProjectError) -> ProjectFilesystemError {
    ProjectFilesystemError::TransactionPrepareFailed {
        message: error.to_string(),
    }
}

#[cfg(all(test, any()))]
mod tests {
    use crate::graph::document::{FunctionParameter, FunctionSignature};
    use crate::project::{
        GraphDocumentKind, GraphResourceDocument, ProjectData, ProjectFilesystemError,
        ProjectState, fixtures, read_project_index as read_project_index_from_disk,
    };
    use yss_data_contract::{DataType, DataValue};
    use yss_graph_document::FunctionParameterId;
    use yss_graph_document::GraphResourcePath;
    use yss_variable_contract::VariableScope;

    use std::time::Duration;

    fn project_root(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "yssbi-project-reads-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn delayed_project_index_read_has_zero_effects_after_project_replacement() {
        let root = project_root("delayed-index");
        let graph_path = GraphResourcePath::new("events/Current.yssbi-event").unwrap();
        let mut project = ProjectData::new();
        project.graphs.insert(
            graph_path.clone(),
            GraphResourceDocument::new("Current", GraphDocumentKind::Event),
        );
        fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
        fixtures::write_graph(&project, root.to_string_lossy().as_ref(), &graph_path).unwrap();
        let graph_file = root.join(graph_path.as_str());
        let graph_contents = std::fs::read(&graph_file).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let expected = state.capture_project_session().unwrap().instance_id;
        let replacement_state = state.clone();
        let replacement_root = crate::project::NormalizedProjectRoot::from_project_path(
            std::path::Path::new("project-b"),
        )
        .unwrap()
        .as_path()
        .to_string_lossy()
        .into_owned();

        let result = super::read_project_index_with(&state, &expected, move |root| {
            replacement_state.activate_project_fixture("project-b".into(), ProjectData::new());
            replacement_state
                .add_variable(
                    "project_b_global",
                    DataType::Int64,
                    DataValue::Int64(42),
                    "",
                    VariableScope::Global,
                    Vec::new(),
                )
                .unwrap();
            read_project_index_from_disk(root.to_string_lossy().as_ref()).map_err(|error| {
                ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                }
            })
        });

        assert_eq!(result.unwrap_err().code(), "stale_project_lifecycle");
        assert_eq!(state.get_path().as_deref(), Some(replacement_root.as_str()));
        assert_eq!(
            std::fs::read(&graph_file).unwrap(),
            graph_contents,
            "stale index read mutated the graph fixture"
        );
        let data = state.get_data().unwrap();
        assert_eq!(data.variables.len(), 1);
        assert_eq!(
            data.variables
                .values()
                .next()
                .map(|value| value.name.as_str()),
            Some("project_b_global")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_overlays_functions_and_globals_from_one_authoritative_snapshot() {
        let root = project_root("coherent-index");
        let function_path = GraphResourcePath::new("functions/Shared.yssbi-function").unwrap();
        let mut disk = ProjectData::new();
        let disk_global = yss_variable_contract::VariableInstance {
            id: yss_variable_contract::VariableId::new(),
            name: "stale_disk_global".into(),
            data_type: DataType::Int64,
            data_value: DataValue::Int64(1),
            tabular: None,
            description: String::new(),
            scope: VariableScope::Global,
            tags: Vec::new(),
        };
        disk.variables.insert(disk_global.id, disk_global);
        disk.graphs.insert(
            function_path.clone(),
            GraphResourceDocument::new("Shared", GraphDocumentKind::Function),
        );
        fixtures::write_project(&disk, root.to_string_lossy().as_ref()).unwrap();
        fixtures::write_graph(&disk, root.to_string_lossy().as_ref(), &function_path).unwrap();

        let mut authoritative = disk;
        let (worksheet_path, mut worksheet) =
            fixtures::worksheet("Authoritative worksheet", "db-1");
        worksheet.revision = crate::project::ResourceRevision::new(5);
        authoritative
            .worksheets
            .insert(worksheet_path.clone(), worksheet.clone());
        let global = authoritative.variables.values_mut().next().unwrap();
        global.name = "authoritative_global".into();
        let function = authoritative.graphs.get_mut(&function_path).unwrap();
        function.function = Some(crate::project::FunctionDocument {
            revision: crate::project::ResourceRevision::new(7),
            signature: FunctionSignature {
                parameters: vec![FunctionParameter {
                    id: FunctionParameterId::new("sales"),
                    name: "Observed sales".into(),
                    type_name: "DataSeries<Float64>".into(),
                }],
                return_type: Some("Array<String>".into()),
            },
        });
        let before = serde_json::to_value(&authoritative).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), authoritative);
        let expected = state.capture_project_session().unwrap().instance_id;

        let index = state.read_project_index(&expected).unwrap();

        assert_eq!(index.variables.len(), 1);
        assert_eq!(index.variables[0].name, "authoritative_global");
        assert_eq!(index.worksheets.len(), 1);
        assert_eq!(index.worksheets[0].worksheet_path, worksheet_path);
        assert_eq!(index.worksheets[0].name, "Authoritative worksheet");
        assert_eq!(index.worksheets[0].revision, worksheet.revision);
        let function = index
            .graphs
            .iter()
            .find(|entry| entry.path == function_path.as_str())
            .unwrap();
        assert_eq!(function.function_revision.unwrap().get(), 7);
        assert_eq!(
            function
                .function_signature
                .as_ref()
                .unwrap()
                .return_type
                .as_deref(),
            Some("Array<String>")
        );
        assert_eq!(
            serde_json::to_value(function.function_editor_projection.as_ref().unwrap()).unwrap(),
            serde_json::json!({
                "functionRevision": 7,
                "inputs": [{
                    "id": "sales",
                    "name": "Observed sales",
                    "dataType": {
                        "kind": "DataSeries",
                        "inner": { "kind": "Float64" }
                    }
                }],
                "outputs": [{
                    "id": "return",
                    "name": "Array<String>",
                    "dataType": {
                        "kind": "Array",
                        "inner": { "kind": "String" }
                    }
                }]
            })
        );
        assert_eq!(
            serde_json::to_value(state.get_data().unwrap()).unwrap(),
            before
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_carries_backend_issued_sidebar_resource_paths() {
        let (root, state, _, _, unloaded_variable_id, loaded_variable_id) = catalog_fixture();
        let expected = state.capture_project_session().unwrap().instance_id;

        let index = state.read_project_index(&expected).unwrap();

        assert!(index.variables.iter().any(|entry| {
            entry.id == unloaded_variable_id.to_string()
                && entry.resource_path.as_str() == format!("variables/{unloaded_variable_id}")
        }));
        assert!(index.variables.iter().any(|entry| {
            entry.id == loaded_variable_id.to_string()
                && entry.resource_path.as_str() == format!("variables/{loaded_variable_id}")
        }));
        assert_eq!(index.databases.len(), 1);
        assert_eq!(index.databases[0].id, "sales");
        assert_eq!(index.databases[0].resource_path.as_str(), "databases/sales");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_database_declaration_and_revision_are_one_captured_generation() {
        let (root, state, _, _, _, _) = catalog_fixture();
        let session = state.capture_project_session().unwrap();
        let changed_state = state.clone();
        let (capture_window_tx, capture_window_rx) = std::sync::mpsc::channel();
        let (writer_blocked_tx, writer_blocked_rx) = std::sync::mpsc::channel();
        let (mutated_tx, mutated_rx) = std::sync::mpsc::channel();
        let mutation = std::thread::spawn(move || {
            capture_window_rx.recv().unwrap();
            let mut publication = match changed_state.mutation_publication.try_lock() {
                Ok(publication) => {
                    writer_blocked_tx.send(false).unwrap();
                    publication
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    writer_blocked_tx.send(true).unwrap();
                    changed_state.mutation_publication.lock().unwrap()
                }
                Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            };
            let mut data = changed_state.project_data.write().unwrap();
            let mut revisions = changed_state.database_authority_revisions.write().unwrap();
            let database = data.databases.get_mut("sales").unwrap();
            database.engine = yss_database_contract::DatabaseEngine::DuckDb {
                path: "database/coherent.duckdb".into(),
                table: "sales_after".into(),
            };
            database.schema_version = 9;
            database.required = true;
            database.name = "After generation".into();
            publication.allocate_resource_revision().unwrap();
            revisions.insert("sales".into(), publication.authority_generation());
            mutated_tx.send(()).unwrap();
        });

        let before_capture =
            super::capture_project_index_authority_with_test_hook(&state, &session, move || {
                capture_window_tx.send(()).unwrap();
                assert!(
                    writer_blocked_rx.recv().unwrap(),
                    "database writer acquired publication lock inside authority capture window"
                );
            })
            .unwrap();
        mutated_rx.recv().unwrap();
        mutation.join().unwrap();
        let after_capture = super::capture_project_index_authority(&state, &session).unwrap();

        let project_generation = |capture: &super::ProjectIndexAuthorityCapture| {
            let mut index =
                crate::project::project_io::read_project_index_from_root(&root).unwrap();
            super::overlay_authoritative_project_index(
                &capture.data,
                &capture.variable_revisions,
                &capture.database_revisions,
                &mut index,
            )
            .unwrap();
            let database = index
                .databases
                .iter()
                .find(|database| database.id == "sales")
                .unwrap();
            (
                database.engine.clone(),
                database.schema_version,
                database.required,
                database.name.clone(),
                database.revision.get(),
            )
        };
        let before = (
            yss_database_contract::DatabaseEngine::InMemory {
                name: "sales".into(),
            },
            1,
            false,
            Some("Sales warehouse".into()),
            0,
        );
        let after = (
            yss_database_contract::DatabaseEngine::DuckDb {
                path: "database/coherent.duckdb".into(),
                table: "sales_after".into(),
            },
            9,
            true,
            Some("After generation".into()),
            1,
        );
        assert_eq!(project_generation(&before_capture), before);
        assert_eq!(project_generation(&after_capture), after);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_suppresses_dirty_deleted_local_after_unload_and_recovers_publication() {
        let (root, state, _, loaded_path, _, loaded_variable_id) = catalog_fixture();
        let expected = state.capture_project_session().unwrap().instance_id;
        let deleted = state
            .delete_local_variable_transaction(
                &expected,
                loaded_variable_id,
                crate::project::ResourceRevision::INITIAL,
                crate::project::OperationId::new(),
            )
            .unwrap();
        assert_eq!(deleted.result.publication_revision, 1);
        state.unload_graph_resource(&loaded_path).unwrap();

        let index = state.read_project_index(&expected).unwrap();
        assert_eq!(index.publication_revision, 1);
        assert!(
            !index
                .variables
                .iter()
                .any(|entry| entry.id == loaded_variable_id.to_string())
        );
        let retained = state
            .variable_revision_entry_for_test(&loaded_variable_id)
            .unwrap();
        assert_eq!(retained.revision, crate::project::ResourceRevision::new(1));
        assert!(!retained.is_present());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_uses_retained_revision_for_unloaded_local_variable() {
        let (root, state, _, loaded_path, _, loaded_variable_id) = catalog_fixture();
        let expected = state.capture_project_session().unwrap().instance_id;
        state
            .update_local_variable_transaction(
                &expected,
                loaded_variable_id,
                Some("Retained local".into()),
                None,
                None,
                None,
                None,
                crate::project::ResourceRevision::INITIAL,
                crate::project::OperationId::new(),
            )
            .unwrap();
        state.unload_graph_resource(&loaded_path).unwrap();

        let index = state.read_project_index(&expected).unwrap();
        let variable = index
            .variables
            .iter()
            .find(|entry| entry.id == loaded_variable_id.to_string())
            .unwrap();
        assert_eq!(variable.revision.get(), 1);
        assert_eq!(index.publication_revision, 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_concurrent_local_mutation_is_one_coherent_generation() {
        let (root, state, _, _, _, loaded_variable_id) = catalog_fixture();
        let expected = state.capture_project_session().unwrap().instance_id;
        let changed_state = state.clone();
        let changed_project = expected.clone();

        let index = super::read_project_index_with(&state, &expected, move |root| {
            let index = crate::project::project_io::read_project_index_from_root(root)
                .map_err(super::read_error)?;
            changed_state
                .update_local_variable_transaction(
                    &changed_project,
                    loaded_variable_id,
                    Some("Concurrent local".into()),
                    None,
                    Some(DataValue::Int64(9)),
                    None,
                    None,
                    crate::project::ResourceRevision::INITIAL,
                    crate::project::OperationId::new(),
                )
                .unwrap();
            Ok(index)
        })
        .unwrap();

        let variable = index
            .variables
            .iter()
            .find(|entry| entry.id == loaded_variable_id.to_string())
            .unwrap();
        assert_eq!(index.publication_revision, 1);
        assert_eq!(variable.revision.get(), 1);
        assert_eq!(variable.name, "Concurrent local");
        assert_eq!(variable.data_value, DataValue::Int64(9));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_waits_for_resource_writer_and_returns_committed_layout() {
        let root = project_root("writer-index");
        fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let lease = state.filesystem().acquire(session.root.clone()).unwrap();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let reader_state = state.clone();
        let expected = session.instance_id.clone();
        let reader = std::thread::spawn(move || {
            let result = reader_state.read_project_index(&expected);
            done_tx.send(()).unwrap();
            result
        });

        assert!(done_rx.recv_timeout(Duration::from_millis(100)).is_err());
        let graph_path = GraphResourcePath::new("events/Committed.yssbi-event").unwrap();
        let mut committed = ProjectData::new();
        committed.graphs.insert(
            graph_path.clone(),
            GraphResourceDocument::new("Committed", GraphDocumentKind::Event),
        );
        fixtures::write_graph(
            &committed,
            session.root.as_path().to_string_lossy().as_ref(),
            &graph_path,
        )
        .unwrap();
        drop(lease);

        let index = reader.join().unwrap().unwrap();
        assert_eq!(index.graphs.len(), 1);
        assert_eq!(index.graphs[0].path, graph_path.as_str());
        std::fs::remove_dir_all(root).unwrap();
    }

    fn catalog_fixture() -> (
        std::path::PathBuf,
        ProjectState,
        GraphResourcePath,
        GraphResourcePath,
        yss_variable_contract::VariableId,
        yss_variable_contract::VariableId,
    ) {
        let root = project_root("catalog");
        let unloaded_path = GraphResourcePath::new("functions/A-Unloaded.yssbi-function").unwrap();
        let loaded_path = GraphResourcePath::new("functions/Z-Loaded.yssbi-function").unwrap();
        let unloaded_variable_id = yss_variable_contract::VariableId::new();
        let loaded_variable_id = yss_variable_contract::VariableId::new();
        let unloaded_variable = yss_variable_contract::VariableInstance {
            id: unloaded_variable_id,
            name: "Unloaded local".into(),
            data_type: DataType::Int64,
            data_value: DataValue::Int64(1),
            tabular: None,
            description: String::new(),
            scope: VariableScope::Function {
                function_path: unloaded_path.as_str().into(),
            },
            tags: Vec::new(),
        };
        let loaded_variable = yss_variable_contract::VariableInstance {
            id: loaded_variable_id,
            name: "Loaded local".into(),
            data_type: DataType::Int64,
            data_value: DataValue::Int64(2),
            tabular: None,
            description: String::new(),
            scope: VariableScope::Function {
                function_path: loaded_path.as_str().into(),
            },
            tags: Vec::new(),
        };
        let mut disk = ProjectData::new();
        disk.graphs.insert(
            unloaded_path.clone(),
            GraphResourceDocument::new("A-Unloaded", GraphDocumentKind::Function),
        );
        disk.graphs.insert(
            loaded_path.clone(),
            GraphResourceDocument::new("Z-Loaded", GraphDocumentKind::Function),
        );
        disk.graphs
            .get_mut(&unloaded_path)
            .unwrap()
            .function
            .as_mut()
            .unwrap()
            .revision = crate::project::ResourceRevision::new(3);
        disk.graphs
            .get_mut(&unloaded_path)
            .unwrap()
            .function
            .as_mut()
            .unwrap()
            .signature
            .return_type = Some("Int64".into());
        disk.variables
            .insert(unloaded_variable_id, unloaded_variable);
        disk.variables
            .insert(loaded_variable_id, loaded_variable.clone());
        fixtures::write_project(&disk, root.to_string_lossy().as_ref()).unwrap();

        let mut authoritative = ProjectData::new();
        let mut loaded =
            GraphResourceDocument::new("Authoritative loaded", GraphDocumentKind::Function);
        loaded.function.as_mut().unwrap().revision = crate::project::ResourceRevision::new(7);
        loaded.function.as_mut().unwrap().signature.return_type = Some("Float64".into());
        authoritative.graphs.insert(loaded_path.clone(), loaded);
        authoritative
            .variables
            .insert(loaded_variable_id, loaded_variable);
        authoritative.databases.insert(
            "sales".into(),
            yss_database_contract::DatabaseDecl {
                id: yss_database_contract::DatabaseId::from_existing("sales".into()),
                engine: yss_database_contract::DatabaseEngine::InMemory {
                    name: "sales".into(),
                },
                schema_version: 1,
                required: false,
                name: "Sales warehouse".into(),
            },
        );
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), authoritative);
        (
            root,
            state,
            unloaded_path,
            loaded_path,
            unloaded_variable_id,
            loaded_variable_id,
        )
    }

    #[test]
    fn catalog_snapshot_combines_unloaded_loaded_and_database_resources_in_opaque_order() {
        let (root, state, unloaded_path, loaded_path, unloaded_variable_id, loaded_variable_id) =
            catalog_fixture();
        let expected = state.capture_project_session().unwrap().instance_id;

        let snapshot = state.catalog_snapshot(&expected).unwrap();

        assert_eq!(snapshot.project_instance_id, expected);
        assert!(std::sync::Arc::ptr_eq(
            &snapshot.registry,
            &state.project_store.read().unwrap().node_registry
        ));
        assert!(std::sync::Arc::ptr_eq(
            &snapshot.catalog,
            &state.project_store.read().unwrap().catalog
        ));
        let identities = snapshot
            .resources
            .iter()
            .map(|entry| {
                (
                    entry.resource_path.as_str().to_string(),
                    entry.node_type_id.as_str().to_string(),
                )
            })
            .collect::<Vec<_>>();
        let mut sorted = identities.clone();
        sorted.sort();
        assert_eq!(identities, sorted);
        assert!(snapshot.resources.iter().any(|entry| {
            entry.resource_path.as_str() == unloaded_path.as_str()
                && entry.name.as_ref() == "A-Unloaded"
                && entry.resource_revision == 3
        }));
        assert!(snapshot.resources.iter().any(|entry| {
            entry.resource_path.as_str() == loaded_path.as_str()
                && entry.name.as_ref() == "Authoritative loaded"
                && entry.resource_revision == 7
        }));
        for variable_id in [unloaded_variable_id, loaded_variable_id] {
            let path = format!("variables/{variable_id}");
            let entries = snapshot
                .resources
                .iter()
                .filter(|entry| entry.resource_path.as_str() == path)
                .collect::<Vec<_>>();
            assert_eq!(entries.len(), 2);
            assert!(
                entries
                    .iter()
                    .any(|entry| { entry.node_type_id.as_str() == "yssbi.project.variable.get" })
            );
            assert!(
                entries
                    .iter()
                    .any(|entry| { entry.node_type_id.as_str() == "yssbi.project.variable.set" })
            );
        }
        assert!(snapshot.resources.iter().any(|entry| {
            entry.resource_path.as_str() == "databases/sales"
                && entry.name.as_ref() == "Sales warehouse"
                && entry.node_type_id.as_str() == "yssbi.dataframe.source.get"
        }));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn catalog_snapshot_preserves_retained_local_revision_across_unload_and_shuffled_index() {
        let (root, state, _, loaded_path, _, loaded_variable_id) = catalog_fixture();
        let expected = state.capture_project_session().unwrap().instance_id;
        state
            .update_local_variable_transaction(
                &expected,
                loaded_variable_id,
                None,
                None,
                Some(DataValue::Int64(3)),
                None,
                None,
                crate::project::ResourceRevision::INITIAL,
                crate::project::OperationId::new(),
            )
            .unwrap();

        let loaded = state.catalog_snapshot(&expected).unwrap();
        assert_eq!(loaded.resource_publication_revision, 1);
        let variable_path = format!("variables/{loaded_variable_id}");
        assert!(loaded.resources.iter().any(|entry| {
            entry.resource_path.as_str() == variable_path && entry.resource_revision == 1
        }));

        state.unload_graph_resource(&loaded_path).unwrap();
        let retained = state.variable_revisions.read().unwrap()[&loaded_variable_id];
        assert_eq!(retained.revision, crate::project::ResourceRevision::new(1));
        assert!(retained.is_present());
        let unloaded = state.catalog_snapshot(&expected).unwrap();
        assert!(unloaded.resources.iter().any(|entry| {
            entry.resource_path.as_str() == variable_path && entry.resource_revision == 1
        }));

        let normal = super::catalog_snapshot_with_reader(&state, &expected, |root| {
            crate::project::project_io::read_project_index_from_root(root)
                .map_err(super::read_error)
        })
        .unwrap();
        let shuffled = super::catalog_snapshot_with_reader(&state, &expected, |root| {
            let mut index = crate::project::project_io::read_project_index_from_root(root)
                .map_err(super::read_error)?;
            index.graphs.reverse();
            index.variables.reverse();
            Ok(index)
        })
        .unwrap();
        let identities = |snapshot: &super::CatalogProjectSnapshot| {
            snapshot
                .resources
                .iter()
                .map(|entry| {
                    (
                        entry.resource_path.as_str().to_string(),
                        entry.node_type_id.as_str().to_string(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(identities(&normal), identities(&shuffled));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn catalog_snapshot_does_not_resurrect_dirty_deleted_local_variable_after_unload() {
        let (root, state, _, loaded_path, _, loaded_variable_id) = catalog_fixture();
        let expected = state.capture_project_session().unwrap().instance_id;
        state
            .delete_local_variable_transaction(
                &expected,
                loaded_variable_id,
                crate::project::ResourceRevision::INITIAL,
                crate::project::OperationId::new(),
            )
            .unwrap();
        let deleted = state.variable_revisions.read().unwrap()[&loaded_variable_id];
        assert_eq!(deleted.revision, crate::project::ResourceRevision::new(1));
        assert!(!deleted.is_present());
        state.unload_graph_resource(&loaded_path).unwrap();

        let snapshot = state.catalog_snapshot(&expected).unwrap();
        let variable_path = format!("variables/{loaded_variable_id}");
        assert!(
            !snapshot
                .resources
                .iter()
                .any(|entry| entry.resource_path.as_str() == variable_path)
        );
        assert!(
            !state
                .catalog_mutation_validation_snapshot(&expected)
                .unwrap()
                .resources
                .contains_key(&yss_graph_catalog::CatalogResourcePath::new(variable_path))
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn catalog_snapshot_waits_for_root_filesystem_lease() {
        let (root, state, _, _, _, _) = catalog_fixture();
        let session = state.capture_project_session().unwrap();
        let lease = state.filesystem().acquire(session.root.clone()).unwrap();
        let reader_state = state.clone();
        let expected = session.instance_id;
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            let result = reader_state.catalog_snapshot(&expected);
            done_tx.send(()).unwrap();
            result
        });

        assert!(
            done_rx
                .recv_timeout(std::time::Duration::from_millis(100))
                .is_err()
        );
        drop(lease);
        done_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        reader.join().unwrap().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn catalog_snapshot_rejects_stale_identity_and_authority_change_during_capture() {
        let (root, state, _, _, _, _) = catalog_fixture();
        let expected = state.capture_project_session().unwrap().instance_id;
        let stale = crate::project::ProjectInstanceId::new();
        assert_eq!(
            state.catalog_snapshot(&stale).unwrap_err().code(),
            "stale_project_lifecycle"
        );

        let changed_state = state.clone();
        let error = super::catalog_snapshot_with_reader(&state, &expected, move |root| {
            let index = crate::project::project_io::read_project_index_from_root(root)
                .map_err(super::read_error)?;
            changed_state
                .add_variable(
                    "concurrent",
                    DataType::Int64,
                    DataValue::Int64(1),
                    "",
                    VariableScope::Global,
                    Vec::new(),
                )
                .unwrap();
            Ok(index)
        })
        .unwrap_err();
        assert_eq!(error.code(), "catalog_resource_stale");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn catalog_snapshot_rejects_project_replacement_during_capture() {
        let (root, state, _, _, _, _) = catalog_fixture();
        let expected = state.capture_project_session().unwrap().instance_id;
        let replacement = project_root("catalog-replacement");
        fixtures::write_project(&ProjectData::new(), replacement.to_string_lossy().as_ref())
            .unwrap();
        let changed_state = state.clone();
        let replacement_for_change = replacement.clone();

        let error = super::catalog_snapshot_with_reader(&state, &expected, move |root| {
            let index = crate::project::project_io::read_project_index_from_root(root)
                .map_err(super::read_error)?;
            changed_state.activate_project_fixture(
                replacement_for_change.to_string_lossy().into_owned(),
                ProjectData::new(),
            );
            Ok(index)
        })
        .unwrap_err();

        assert_eq!(error.code(), "stale_project_lifecycle");
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(replacement).unwrap();
    }

    #[test]
    fn catalog_snapshot_rejects_missing_existing_revision_owners() {
        let (root, state, _, _, _, loaded_variable_id) = catalog_fixture();
        let expected = state.capture_project_session().unwrap().instance_id;
        state
            .variable_revisions
            .write()
            .unwrap()
            .remove(&loaded_variable_id);
        assert_eq!(
            state.catalog_snapshot(&expected).unwrap_err().code(),
            "catalog_resource_stale"
        );

        state.variable_revisions.write().unwrap().insert(
            loaded_variable_id,
            crate::project::project_state::VariableRevisionEntry::present(
                crate::project::ResourceRevision::INITIAL,
            ),
        );
        state
            .database_authority_revisions
            .write()
            .unwrap()
            .remove("sales");
        assert_eq!(
            state.catalog_snapshot(&expected).unwrap_err().code(),
            "catalog_resource_stale"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn catalog_mutation_validation_snapshot_captures_exact_immutable_authority_without_later_io() {
        let (root, state, unloaded_path, loaded_path, unloaded_variable_id, loaded_variable_id) =
            catalog_fixture();
        let expected = state.capture_project_session().unwrap().instance_id;

        let snapshot = state
            .catalog_mutation_validation_snapshot(&expected)
            .unwrap();
        std::fs::remove_dir_all(&root).unwrap();

        let unloaded = snapshot
            .resources
            .get(&yss_graph_catalog::CatalogResourcePath::new(
                unloaded_path.as_str(),
            ))
            .unwrap();
        let super::CatalogMutationResource::Function {
            revision,
            signature,
            allowed_node_type_id,
            parameter_binding,
        } = unloaded
        else {
            panic!("unloaded function fact")
        };
        assert_eq!(revision.get(), 3);
        assert_eq!(signature.return_type.as_deref(), Some("Int64"));
        assert_eq!(allowed_node_type_id.as_str(), "yssbi.project.function.call");
        assert_eq!(parameter_binding.as_ref(), "target");

        let loaded = snapshot
            .resources
            .get(&yss_graph_catalog::CatalogResourcePath::new(
                loaded_path.as_str(),
            ))
            .unwrap();
        let super::CatalogMutationResource::Function {
            revision,
            signature,
            ..
        } = loaded
        else {
            panic!("loaded function fact")
        };
        assert_eq!(revision.get(), 7);
        assert_eq!(signature.return_type.as_deref(), Some("Float64"));

        for variable_id in [unloaded_variable_id, loaded_variable_id] {
            let variable = snapshot
                .resources
                .get(&yss_graph_catalog::CatalogResourcePath::new(format!(
                    "variables/{variable_id}"
                )))
                .unwrap();
            let super::CatalogMutationResource::Variable {
                revision,
                scope,
                allowed_node_type_ids,
                parameter_binding,
                ..
            } = variable
            else {
                panic!("variable fact")
            };
            assert_eq!(*revision, crate::project::ResourceRevision::INITIAL);
            assert!(matches!(scope, VariableScope::Function { .. }));
            assert_eq!(
                allowed_node_type_ids
                    .iter()
                    .map(|id| id.as_str())
                    .collect::<Vec<_>>(),
                ["yssbi.project.variable.get", "yssbi.project.variable.set"]
            );
            assert_eq!(parameter_binding.as_ref(), "variable");
        }

        let database = snapshot
            .resources
            .get(&yss_graph_catalog::CatalogResourcePath::new(
                "databases/sales",
            ))
            .unwrap();
        let super::CatalogMutationResource::Database {
            authority_revision,
            allowed_node_type_id,
            parameter_binding,
        } = database
        else {
            panic!("database fact")
        };
        assert_eq!(
            *authority_revision,
            crate::project::ResourceRevision::INITIAL
        );
        assert_eq!(allowed_node_type_id.as_str(), "yssbi.dataframe.source.get");
        assert_eq!(parameter_binding.as_ref(), "dataframe");
    }

    #[test]
    fn catalog_mutation_validation_snapshot_waits_for_root_filesystem_lease() {
        let (root, state, _, _, _, _) = catalog_fixture();
        let session = state.capture_project_session().unwrap();
        let lease = state.filesystem().acquire(session.root.clone()).unwrap();
        let reader_state = state.clone();
        let expected = session.instance_id;
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            let result = reader_state.catalog_mutation_validation_snapshot(&expected);
            done_tx.send(()).unwrap();
            result
        });

        assert!(done_rx.recv_timeout(Duration::from_millis(100)).is_err());
        drop(lease);
        reader.join().unwrap().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn worksheet_load_reads_current_authority_without_disk_fallback() {
        let root = project_root("worksheet-authority");
        let (worksheet_path, worksheet) = fixtures::worksheet("Authoritative worksheet", "db-1");
        let mut project = ProjectData::new();
        project
            .worksheets
            .insert(worksheet_path.clone(), worksheet.clone());
        fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
        let expected = state.capture_project_session().unwrap().instance_id;

        let loaded = state
            .load_worksheet_document(&expected, &worksheet_path)
            .unwrap();

        assert_eq!(loaded, worksheet);
        std::fs::remove_dir_all(root).unwrap();
    }
}
