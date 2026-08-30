use crate::project::{
    GraphDocumentKind, ProjectData, ProjectFilesystemCoordinator, ProjectFilesystemLeaseSet,
    ProjectSession,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use yss_graph_document::GraphResourcePath;
use yss_project_history::{
    HistoryMutation, HistoryPersistencePolicy, MutationRequest, ProjectDocumentState,
    ProjectHistory, ProjectHistoryMutationError, ProjectHistoryTransaction, ResourceKey,
    VariableDocument, VariableResourceKey, WorksheetResourceKey,
};
use yss_project_identity::{HistoryEntryId, ResourceRevision};
use yss_project_layout::{GLOBAL_VARIABLES_FILE, WORKSHEET_EXTENSION};
use yss_variable_contract::{VariableInstance, VariableScope};
use yss_worksheet_document::{WorksheetDocument, WorksheetResourcePath};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HistoryGraphResidency {
    Loaded,
    Unloaded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct HistoryPreparationBasis {
    pub session: ProjectSession,
    pub authority_generation: u64,
    pub history_id: HistoryEntryId,
    pub persistence: HistoryPersistencePolicy,
    pub undo: bool,
    pub expected_revisions: BTreeMap<ResourceKey, ResourceRevision>,
    pub expected_graph_revisions: BTreeMap<GraphResourcePath, yss_graph_document::GraphRevision>,
    pub residency: BTreeMap<GraphResourcePath, HistoryGraphResidency>,
}

pub(super) struct PreparedHistoryDocuments {
    pub lease: ProjectFilesystemLeaseSet,
    pub basis: HistoryPreparationBasis,
    pub before: ProjectDocumentState,
    pub after: ProjectDocumentState,
    pub after_data: ProjectData,
    pub loaded_after_data: ProjectData,
    pub after_variable_revisions: std::collections::HashMap<
        yss_variable_contract::VariableId,
        super::project_state::VariableRevisionEntry,
    >,
    pub after_worksheet_revisions:
        std::collections::HashMap<WorksheetResourcePath, ResourceRevision>,
    pub transaction: ProjectHistoryTransaction,
    pub proposed_history: ProjectHistory,
    pub touched_graphs: BTreeSet<GraphResourcePath>,
    pub contains_unloaded_graph: bool,
}

pub(super) struct HistoryPreparationSnapshot {
    session: ProjectSession,
    authority_generation: u64,
    undo: bool,
    transaction: ProjectHistoryTransaction,
    graph_revisions:
        std::collections::HashMap<GraphResourcePath, yss_graph_document::GraphRevision>,
    variable_revisions: std::collections::HashMap<
        yss_variable_contract::VariableId,
        super::project_state::VariableRevisionEntry,
    >,
    worksheet_revisions: std::collections::HashMap<WorksheetResourcePath, ResourceRevision>,
    history: ProjectHistory,
    data: ProjectData,
    documents: ProjectDocumentState,
    touched: TouchedHistoryResources,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct TouchedHistoryResources {
    pub graphs: BTreeMap<GraphResourcePath, HistoryGraphResidency>,
    pub local_variable_owners: BTreeMap<VariableResourceKey, GraphResourcePath>,
    pub global_variables: BTreeSet<VariableResourceKey>,
    pub worksheets: BTreeSet<WorksheetResourceKey>,
}

pub(super) fn discover_touched_resources(
    transaction: &ProjectHistoryTransaction,
    undo: bool,
    data: &ProjectData,
    known_graphs: &BTreeSet<GraphResourcePath>,
) -> Result<TouchedHistoryResources, String> {
    let mut touched = TouchedHistoryResources {
        graphs: BTreeMap::new(),
        local_variable_owners: BTreeMap::new(),
        global_variables: BTreeSet::new(),
        worksheets: BTreeSet::new(),
    };

    for change in &transaction.changes {
        match &change.resource {
            ResourceKey::Graph(key) => {
                let path = GraphResourcePath::new(key.as_str())
                    .map_err(|error| format!("invalid History graph owner: {error}"))?;
                insert_graph_residency(&mut touched.graphs, data, path);
            }
            ResourceKey::Function(key) => {
                let path = GraphResourcePath::new(key.0.as_ref())
                    .map_err(|error| format!("invalid Function owner graph: {error}"))?;
                if path.kind() != yss_graph_document::GraphResourceKind::Function
                    || !known_graphs.contains(&path)
                {
                    return Err(format!(
                        "Function '{}' has no authoritative owner graph",
                        key.0
                    ));
                }
                insert_graph_residency(&mut touched.graphs, data, path);
            }
            ResourceKey::Variable(key) => {
                let variable = authoritative_or_patched_variable(data, key, change, undo)?;
                match variable.scope {
                    VariableScope::Global => {
                        touched.global_variables.insert(key.clone());
                    }
                    VariableScope::Event { event_path } => {
                        insert_local_variable_owner(
                            &mut touched,
                            data,
                            known_graphs,
                            key,
                            event_path,
                            GraphDocumentKind::Event,
                        )?;
                    }
                    VariableScope::Function { function_path } => {
                        insert_local_variable_owner(
                            &mut touched,
                            data,
                            known_graphs,
                            key,
                            function_path,
                            GraphDocumentKind::Function,
                        )?;
                    }
                }
            }
            ResourceKey::Worksheet(key) => {
                touched.worksheets.insert(key.clone());
            }
            ResourceKey::Database(_) => {}
        }
    }
    if let Some(lifecycle) = &transaction.resource_lifecycle {
        let state = lifecycle
            .forward
            .before
            .as_ref()
            .or(lifecycle.forward.after.as_ref());
        if let Some(state) = state
            .filter(|state| state.kind == yss_project_history::ResourceLifecycleKind::Worksheet)
        {
            touched
                .worksheets
                .insert(WorksheetResourceKey(state.path.clone()));
        }
    }

    Ok(touched)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_history_preparation_snapshot(
    session: ProjectSession,
    authority_generation: u64,
    undo: bool,
    transaction: ProjectHistoryTransaction,
    anchor: &ResourceKey,
    data: ProjectData,
    graph_revisions: std::collections::HashMap<
        GraphResourcePath,
        yss_graph_document::GraphRevision,
    >,
    variable_revisions: std::collections::HashMap<
        yss_variable_contract::VariableId,
        super::project_state::VariableRevisionEntry,
    >,
    worksheet_revisions: std::collections::HashMap<WorksheetResourcePath, ResourceRevision>,
    history: ProjectHistory,
) -> Result<HistoryPreparationSnapshot, String> {
    let known_graphs = graph_revisions.keys().cloned().collect();
    let touched = discover_touched_resources(&transaction, undo, &data, &known_graphs)?;
    let mut documents = super::project_state::project_documents(&data, &variable_revisions);
    documents.worksheet_revisions = worksheet_revisions
        .iter()
        .map(|(path, revision)| (WorksheetResourceKey(path.as_str().into()), *revision))
        .collect();
    retain_required_documents(&mut documents, &transaction, anchor, &touched.graphs);
    let graph_revisions = graph_revisions
        .into_iter()
        .filter(|(path, _)| touched.graphs.contains_key(path))
        .collect();
    Ok(HistoryPreparationSnapshot {
        session,
        authority_generation,
        undo,
        transaction,
        graph_revisions,
        variable_revisions,
        worksheet_revisions,
        history,
        data,
        documents,
        touched,
    })
}

fn retain_required_documents(
    documents: &mut ProjectDocumentState,
    transaction: &ProjectHistoryTransaction,
    anchor: &ResourceKey,
    touched_graphs: &BTreeMap<GraphResourcePath, HistoryGraphResidency>,
) {
    let required = transaction
        .changes
        .iter()
        .map(|change| change.resource.clone())
        .chain(std::iter::once(anchor.clone()))
        .collect::<BTreeSet<_>>();
    documents.graphs.retain(|path, _| {
        required.contains(&ResourceKey::Graph(path.clone()))
            || GraphResourcePath::new(path.as_str())
                .ok()
                .is_some_and(|path| touched_graphs.contains_key(&path))
    });
    documents.functions.retain(|key, _| {
        required.contains(&ResourceKey::Function(key.clone()))
            || GraphResourcePath::new(key.0.as_ref())
                .ok()
                .is_some_and(|path| touched_graphs.contains_key(&path))
    });
    documents.variables.retain(|key, document| {
        required.contains(&ResourceKey::Variable(key.clone()))
            || document
                .value
                .as_ref()
                .and_then(|value| serde_json::from_value::<VariableInstance>(value.clone()).ok())
                .and_then(|variable| variable_owner_path(&variable))
                .is_some_and(|path| touched_graphs.contains_key(&path))
    });
    documents.worksheets.retain(|key, _| {
        required.contains(&ResourceKey::Worksheet(key.clone()))
            || transaction
                .resource_lifecycle
                .as_ref()
                .is_some_and(|lifecycle| {
                    lifecycle
                        .forward
                        .before
                        .as_ref()
                        .or(lifecycle.forward.after.as_ref())
                        .is_some_and(|state| state.path.as_ref() == key.0.as_ref())
                })
    });
}

fn variable_owner_path(variable: &VariableInstance) -> Option<GraphResourcePath> {
    match &variable.scope {
        VariableScope::Global => None,
        VariableScope::Event { event_path } => GraphResourcePath::new(event_path.clone()).ok(),
        VariableScope::Function { function_path } => {
            GraphResourcePath::new(function_path.clone()).ok()
        }
    }
}

pub(super) fn hydrate_history_preparation(
    mut snapshot: HistoryPreparationSnapshot,
    filesystem: &ProjectFilesystemCoordinator,
    request: &MutationRequest<HistoryMutation>,
) -> Result<PreparedHistoryDocuments, ProjectHistoryMutationError> {
    let lease = filesystem
        .acquire(snapshot.session.root.clone())
        .map_err(history_conflict)?;
    let unloaded = snapshot
        .touched
        .graphs
        .iter()
        .filter_map(|(path, residency)| {
            (*residency == HistoryGraphResidency::Unloaded).then_some(path.clone())
        })
        .collect::<Vec<_>>();

    for graph_path in &unloaded {
        hydrate_graph_document(&mut snapshot, graph_path)?;
    }
    validate_loaded_graph_revisions(&snapshot)?;
    install_touched_variable_tombstones(&mut snapshot)?;
    install_touched_worksheet_tombstone(&mut snapshot)?;
    let expected_revisions = expected_revisions(&snapshot)?;
    let current_revision =
        document_revision(&snapshot.documents, &request.resource).ok_or_else(|| {
            ProjectHistoryMutationError::History(
                format!(
                    "history anchor resource {:?} was not found",
                    request.resource
                )
                .into(),
            )
        })?;
    if current_revision != request.base_revision {
        return Err(ProjectHistoryMutationError::StaleRevision {
            base_revision: request.base_revision.get(),
            current_revision: current_revision.get(),
        });
    }

    let before = snapshot.documents.clone();
    let mut after = before.clone();
    let mut proposed_history = snapshot.history;
    let transaction = if snapshot.undo {
        proposed_history.undo(&mut after)
    } else {
        proposed_history.redo(&mut after)
    }
    .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
    if transaction.history_id != snapshot.transaction.history_id {
        return Err(ProjectHistoryMutationError::History(
            "history head changed during preparation".into(),
        ));
    }
    let mut after_worksheet_revisions = snapshot.worksheet_revisions.clone();
    for key in &snapshot.touched.worksheets {
        let path = WorksheetResourcePath::parse(key.0.as_ref())
            .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
        let revision = snapshot
            .worksheet_revisions
            .get(&path)
            .copied()
            .ok_or_else(|| {
                ProjectHistoryMutationError::History(
                    format!("Worksheet '{}' has no revision authority", key.0).into(),
                )
            })?
            .checked_next()
            .map_err(|error| {
                ProjectHistoryMutationError::History(
                    format!(
                        "Worksheet '{}' revision is exhausted at {}",
                        key.0, error.retained
                    )
                    .into(),
                )
            })?;
        after_worksheet_revisions.insert(path, revision);
        if let Some(document) = after.worksheets.get_mut(key) {
            document.revision = revision;
        }
    }

    let mut after_data = snapshot.data;
    super::project_state::replace_project_documents(
        &mut after_data,
        &mut snapshot.variable_revisions,
        after.clone(),
    );
    synchronize_function_owner_revisions(&mut after_data, &snapshot.transaction);
    let mut loaded_after_data = after_data.clone();
    let unloaded_graphs = snapshot
        .touched
        .graphs
        .iter()
        .filter_map(|(path, residency)| {
            (*residency == HistoryGraphResidency::Unloaded).then_some(path.clone())
        })
        .collect::<BTreeSet<_>>();
    loaded_after_data
        .graphs
        .retain(|path, _| !unloaded_graphs.contains(path));
    loaded_after_data.variables.retain(|_, variable| {
        variable_owner_path(variable).is_none_or(|path| !unloaded_graphs.contains(&path))
    });
    let touched_graphs = snapshot.touched.graphs.keys().cloned().collect();
    Ok(PreparedHistoryDocuments {
        lease,
        basis: HistoryPreparationBasis {
            session: snapshot.session,
            authority_generation: snapshot.authority_generation,
            history_id: snapshot.transaction.history_id,
            persistence: snapshot.transaction.persistence,
            undo: snapshot.undo,
            expected_revisions,
            expected_graph_revisions: snapshot.graph_revisions.into_iter().collect(),
            residency: snapshot.touched.graphs,
        },
        before,
        after,
        after_data,
        loaded_after_data,
        after_variable_revisions: snapshot.variable_revisions,
        after_worksheet_revisions,
        transaction,
        proposed_history,
        touched_graphs,
        contains_unloaded_graph: !unloaded.is_empty(),
    })
}

pub(super) fn synchronize_function_owner_revisions(
    data: &mut ProjectData,
    transaction: &ProjectHistoryTransaction,
) {
    for change in &transaction.changes {
        let ResourceKey::Function(key) = &change.resource else {
            continue;
        };
        let path = GraphResourcePath::new(key.0.as_ref())
            .expect("History Function owner paths were validated during preparation");
        let graph = data
            .graphs
            .get_mut(&path)
            .expect("History Function owner graph remains in the prepared document state");
        let revision = graph
            .function
            .as_ref()
            .expect("History Function owner retains its embedded Function document")
            .revision;
        graph.document.revision = revision.to_graph_revision();
    }
}

fn hydrate_graph_document(
    snapshot: &mut HistoryPreparationSnapshot,
    graph_path: &GraphResourcePath,
) -> Result<(), ProjectHistoryMutationError> {
    let root = snapshot.session.root.as_path().to_string_lossy();
    let disk = super::project_io::load_project_graph_document_from_file(&root, graph_path)
        .map_err(history_conflict)?;
    let expected_graph_revision = snapshot
        .graph_revisions
        .get(graph_path)
        .copied()
        .ok_or_else(|| {
            ProjectHistoryMutationError::History(
                format!("hydrated graph '{}' has no revision authority", graph_path).into(),
            )
        })?;
    if disk.revision.to_graph_revision() != expected_graph_revision {
        return Err(ProjectHistoryMutationError::History(
            format!(
                "hydrated graph '{}' revision mismatch: expected {}, found {}",
                graph_path,
                expected_graph_revision.get(),
                disk.revision.get()
            )
            .into(),
        ));
    }

    let document_key = graph_path.clone();
    let mut graph = disk.document;
    graph.revision = disk.revision.to_graph_revision();
    snapshot
        .documents
        .graphs
        .insert(document_key, graph.clone());
    snapshot.data.graphs.insert(
        graph_path.clone(),
        crate::project::GraphResourceDocument {
            name: disk.name,
            kind: disk.kind,
            document: graph,
            function: disk.function.clone(),
        },
    );
    if let Some(function) = disk.function {
        snapshot.documents.functions.insert(
            yss_project_history::FunctionResourceKey(graph_path.as_str().into()),
            function,
        );
    }
    for (id, variable) in disk.local_variables {
        verify_variable_owner(&variable, graph_path)?;
        snapshot.data.variables.insert(id, variable.clone());
        let revision = snapshot
            .variable_revisions
            .get(&id)
            .filter(|entry| entry.is_present())
            .map(|entry| entry.revision)
            .ok_or_else(|| {
                ProjectHistoryMutationError::History(
                    format!(
                        "hydrated local Variable '{}' has no present revision authority",
                        id
                    )
                    .into(),
                )
            })?;
        snapshot.documents.variables.insert(
            VariableResourceKey(format!("variables/{id}").into()),
            VariableDocument {
                revision,
                value: Some(
                    serde_json::to_value(variable)
                        .expect("hydrated Variable documents are serializable"),
                ),
            },
        );
    }

    for (key, owner) in &snapshot.touched.local_variable_owners {
        if owner != graph_path {
            continue;
        }
        let id = variable_id_from_key(key).map_err(ProjectHistoryMutationError::History)?;
        let entry = snapshot.variable_revisions.get(&id).ok_or_else(|| {
            ProjectHistoryMutationError::History(
                format!("Variable '{}' has no revision authority", key.0).into(),
            )
        })?;
        let hydrated = snapshot.documents.variables.get(key);
        if entry.is_present()
            && hydrated
                .and_then(|document| document.value.as_ref())
                .is_none()
        {
            return Err(ProjectHistoryMutationError::History(
                format!(
                    "Variable '{}' is present in authority but absent from hydrated graph '{}'",
                    key.0, graph_path
                )
                .into(),
            ));
        }
        if !entry.is_present()
            && hydrated
                .and_then(|document| document.value.as_ref())
                .is_some()
        {
            return Err(ProjectHistoryMutationError::History(
                format!(
                    "Variable '{}' is deleted in authority but present in hydrated graph '{}'",
                    key.0, graph_path
                )
                .into(),
            ));
        }
    }
    Ok(())
}

fn validate_loaded_graph_revisions(
    snapshot: &HistoryPreparationSnapshot,
) -> Result<(), ProjectHistoryMutationError> {
    for (path, residency) in &snapshot.touched.graphs {
        if *residency != HistoryGraphResidency::Loaded {
            continue;
        }
        let expected = snapshot.graph_revisions.get(path).copied().ok_or_else(|| {
            ProjectHistoryMutationError::History(
                format!("loaded graph '{}' has no revision authority", path).into(),
            )
        })?;
        let key = path.clone();
        let actual = snapshot
            .documents
            .graphs
            .get(&key)
            .map(|graph| graph.revision)
            .ok_or_else(|| {
                ProjectHistoryMutationError::History(
                    format!("loaded graph '{}' is absent", path).into(),
                )
            })?;
        if actual != expected {
            return Err(ProjectHistoryMutationError::History(
                format!("loaded graph '{}' revision authority mismatch", path).into(),
            ));
        }
    }
    Ok(())
}

fn install_touched_variable_tombstones(
    snapshot: &mut HistoryPreparationSnapshot,
) -> Result<(), ProjectHistoryMutationError> {
    for change in &snapshot.transaction.changes {
        let ResourceKey::Variable(key) = &change.resource else {
            continue;
        };
        if snapshot.documents.variables.contains_key(key) {
            continue;
        }
        let id = variable_id_from_key(key).map_err(ProjectHistoryMutationError::History)?;
        let entry = snapshot.variable_revisions.get(&id).ok_or_else(|| {
            ProjectHistoryMutationError::History(
                format!("Variable '{}' has no revision authority", key.0).into(),
            )
        })?;
        if entry.is_present() {
            return Err(ProjectHistoryMutationError::History(
                format!("present Variable '{}' has no document", key.0).into(),
            ));
        }
        snapshot.documents.variables.insert(
            key.clone(),
            VariableDocument {
                revision: entry.revision,
                value: None,
            },
        );
    }
    Ok(())
}

fn install_touched_worksheet_tombstone(
    snapshot: &mut HistoryPreparationSnapshot,
) -> Result<(), ProjectHistoryMutationError> {
    let Some(lifecycle) = &snapshot.transaction.resource_lifecycle else {
        return Ok(());
    };
    let yss_project_history::ResourceLifecycleHistoryPayload::Worksheet { document } =
        &lifecycle.payload
    else {
        return Ok(());
    };
    let state = lifecycle
        .forward
        .before
        .as_ref()
        .or(lifecycle.forward.after.as_ref())
        .ok_or_else(|| {
            ProjectHistoryMutationError::History("resource lifecycle patch is empty".into())
        })?;
    if state.kind != yss_project_history::ResourceLifecycleKind::Worksheet {
        return Err(ProjectHistoryMutationError::History(
            "worksheet lifecycle payload has a non-worksheet kind".into(),
        ));
    }
    let key = WorksheetResourceKey(state.path.clone());
    if snapshot.documents.worksheets.contains_key(&key) {
        return Ok(());
    }
    let path = WorksheetResourcePath::parse(state.path.as_ref())
        .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
    let revision = snapshot
        .worksheet_revisions
        .get(&path)
        .copied()
        .ok_or_else(|| {
            ProjectHistoryMutationError::History(
                format!("Worksheet '{}' has no tombstone revision", state.path).into(),
            )
        })?;
    let mut document = document.clone();
    document.revision = revision;
    snapshot.documents.worksheets.insert(key, document);
    Ok(())
}

fn expected_revisions(
    snapshot: &HistoryPreparationSnapshot,
) -> Result<BTreeMap<ResourceKey, ResourceRevision>, ProjectHistoryMutationError> {
    let mut revisions = BTreeMap::new();
    for change in &snapshot.transaction.changes {
        let revision =
            document_revision(&snapshot.documents, &change.resource).ok_or_else(|| {
                ProjectHistoryMutationError::History(
                    format!("touched resource {:?} was not hydrated", change.resource).into(),
                )
            })?;
        revisions.insert(change.resource.clone(), revision);
    }
    for key in &snapshot.touched.worksheets {
        let path = WorksheetResourcePath::parse(key.0.as_ref())
            .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
        let revision = snapshot
            .worksheet_revisions
            .get(&path)
            .copied()
            .ok_or_else(|| {
                ProjectHistoryMutationError::History(
                    format!("Worksheet '{}' has no revision authority", key.0).into(),
                )
            })?;
        revisions.insert(ResourceKey::Worksheet(key.clone()), revision);
    }
    Ok(revisions)
}

fn document_revision(
    documents: &ProjectDocumentState,
    resource: &ResourceKey,
) -> Option<ResourceRevision> {
    match resource {
        ResourceKey::Graph(path) => documents
            .graphs
            .get(path)
            .map(|document| ResourceRevision::from_graph_revision(document.revision)),
        ResourceKey::Function(key) => documents
            .functions
            .get(key)
            .map(|document| document.revision),
        ResourceKey::Variable(key) => documents
            .variables
            .get(key)
            .map(|document| document.revision),
        ResourceKey::Worksheet(key) => documents
            .worksheets
            .get(key)
            .map(|document| document.revision),
        ResourceKey::Database(_) => None,
    }
}

pub(super) fn durable_filesystem_mutations(
    prepared: &PreparedHistoryDocuments,
) -> Result<Vec<crate::project::StagedFilesystemMutation>, ProjectHistoryMutationError> {
    let mut mutations = Vec::new();
    for graph_path in &prepared.touched_graphs {
        let (relative_path, contents) =
            super::project_io::serialize_graph_document(&prepared.after_data, graph_path)
                .map_err(history_conflict)?;
        mutations.push(crate::project::StagedFilesystemMutation::Write {
            relative_path,
            contents,
        });
    }
    for key in &prepared
        .transaction
        .changes
        .iter()
        .filter_map(|change| {
            let ResourceKey::Worksheet(key) = &change.resource else {
                return None;
            };
            Some(key.clone())
        })
        .collect::<BTreeSet<_>>()
    {
        push_worksheet_filesystem_mutation(&mut mutations, &prepared.after, key)?;
    }
    if let Some(lifecycle) = &prepared.transaction.resource_lifecycle {
        if let yss_project_history::ResourceLifecycleHistoryPayload::Worksheet { .. } =
            lifecycle.payload
        {
            let state = lifecycle
                .forward
                .before
                .as_ref()
                .or(lifecycle.forward.after.as_ref())
                .ok_or_else(|| history_conflict("resource lifecycle patch is empty"))?;
            push_worksheet_filesystem_mutation(
                &mut mutations,
                &prepared.after,
                &WorksheetResourceKey(state.path.clone()),
            )?;
        }
    }
    if prepared.transaction.changes.iter().any(|change| {
        let ResourceKey::Variable(key) = &change.resource else {
            return false;
        };
        [
            prepared.before.variables.get(key),
            prepared.after.variables.get(key),
        ]
        .into_iter()
        .flatten()
        .filter_map(|document| document.value.as_ref())
        .filter_map(|value| serde_json::from_value::<VariableInstance>(value.clone()).ok())
        .any(|variable| matches!(variable.scope, VariableScope::Global))
    }) {
        mutations.push(crate::project::StagedFilesystemMutation::Write {
            relative_path: GLOBAL_VARIABLES_FILE.into(),
            contents: super::project_io::serialize_global_variables(&prepared.after_data)
                .map_err(history_conflict)?,
        });
    }
    Ok(mutations)
}

fn push_worksheet_filesystem_mutation(
    mutations: &mut Vec<crate::project::StagedFilesystemMutation>,
    documents: &ProjectDocumentState,
    key: &WorksheetResourceKey,
) -> Result<(), ProjectHistoryMutationError> {
    let path = WorksheetResourcePath::parse(key.0.as_ref())
        .map_err(|error| history_conflict(error.to_string()))?;
    if let Some(document) = documents.worksheets.get(key) {
        let (relative_path, contents) =
            crate::project::serialize_worksheet(&path, document).map_err(history_conflict)?;
        mutations.push(crate::project::StagedFilesystemMutation::Write {
            relative_path,
            contents,
        });
    } else {
        mutations.push(crate::project::StagedFilesystemMutation::RemoveFile {
            relative_path: path.relative_path().to_path_buf(),
        });
    }
    Ok(())
}

pub(super) fn validate_durable_history_document(
    relative_path: &Path,
    contents: &[u8],
) -> Result<(), String> {
    if relative_path
        .extension()
        .and_then(|extension| extension.to_str())
        == Some(WORKSHEET_EXTENSION)
    {
        return serde_json::from_slice::<WorksheetDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string());
    }
    if relative_path == Path::new(GLOBAL_VARIABLES_FILE) {
        return super::project_io::parse_global_variables_document(contents)
            .map(|_| ())
            .map_err(|error| error.to_string());
    }
    let graph_path = GraphResourcePath::new(relative_path.to_string_lossy().replace('\\', "/"))
        .map_err(|error| error.to_string())?;
    let kind = graph_path.kind().into();
    super::project_io::parse_graph_resource_document(contents, relative_path, kind)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn verify_variable_owner(
    variable: &VariableInstance,
    expected: &GraphResourcePath,
) -> Result<(), ProjectHistoryMutationError> {
    let owner = match &variable.scope {
        VariableScope::Event { event_path } => event_path,
        VariableScope::Function { function_path } => function_path,
        VariableScope::Global => {
            return Err(ProjectHistoryMutationError::History(
                format!(
                    "hydrated graph '{}' contains project-scoped Variable '{}'",
                    expected, variable.id
                )
                .into(),
            ));
        }
    };
    let owner = GraphResourcePath::new(owner.clone())
        .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
    if &owner != expected {
        return Err(ProjectHistoryMutationError::History(
            format!(
                "hydrated Variable '{}' owner '{}' does not match graph '{}'",
                variable.id, owner, expected
            )
            .into(),
        ));
    }
    Ok(())
}

fn variable_id_from_key(
    key: &VariableResourceKey,
) -> Result<yss_variable_contract::VariableId, Box<str>> {
    let id = key
        .0
        .strip_prefix("variables/")
        .ok_or_else(|| format!("invalid Variable resource key '{}'", key.0).into_boxed_str())?;
    uuid::Uuid::parse_str(id)
        .map(yss_variable_contract::VariableId::from)
        .map_err(|error| {
            format!("invalid Variable resource key '{}': {error}", key.0).into_boxed_str()
        })
}

fn history_conflict(error: impl std::fmt::Display) -> ProjectHistoryMutationError {
    ProjectHistoryMutationError::History(error.to_string().into())
}

fn insert_graph_residency(
    graphs: &mut BTreeMap<GraphResourcePath, HistoryGraphResidency>,
    data: &ProjectData,
    path: GraphResourcePath,
) {
    let residency = if data.graphs.contains_key(&path) {
        HistoryGraphResidency::Loaded
    } else {
        HistoryGraphResidency::Unloaded
    };
    graphs.insert(path, residency);
}

fn authoritative_or_patched_variable(
    data: &ProjectData,
    key: &VariableResourceKey,
    change: &yss_project_history::ResourcePatch,
    undo: bool,
) -> Result<VariableInstance, String> {
    let id_text = key
        .0
        .strip_prefix("variables/")
        .ok_or_else(|| format!("invalid Variable resource key '{}'", key.0))?;
    let id = uuid::Uuid::parse_str(id_text)
        .map(yss_variable_contract::VariableId::from)
        .map_err(|error| format!("invalid Variable resource key '{}': {error}", key.0))?;
    if let Some(variable) = data.variables.get(&id) {
        return Ok(variable.clone());
    }

    let yss_project_history::ResourceDocumentPatch::Variable(patch) = &change.forward else {
        return Err(format!("Variable '{}' has no scoped document patch", key.0));
    };
    let present_side = if undo {
        patch.after.as_ref().or(patch.before.as_ref())
    } else {
        patch.before.as_ref().or(patch.after.as_ref())
    }
    .ok_or_else(|| format!("Variable '{}' has no present scoped value", key.0))?;
    let variable: VariableInstance = serde_json::from_value(present_side.clone())
        .map_err(|error| format!("Variable '{}' has invalid scoped value: {error}", key.0))?;
    if variable.id != id {
        return Err(format!(
            "Variable '{}' scoped value has a different id",
            key.0
        ));
    }
    Ok(variable)
}

fn insert_local_variable_owner(
    touched: &mut TouchedHistoryResources,
    data: &ProjectData,
    known_graphs: &BTreeSet<GraphResourcePath>,
    key: &VariableResourceKey,
    owner: String,
    expected_kind: GraphDocumentKind,
) -> Result<(), String> {
    let path = GraphResourcePath::new(owner)
        .map_err(|error| format!("Variable '{}' has invalid owner graph: {error}", key.0))?;
    if GraphDocumentKind::from(path.kind()) != expected_kind || !known_graphs.contains(&path) {
        return Err(format!(
            "Variable '{}' owner graph '{}' is not authoritative",
            key.0, path
        ));
    }
    touched
        .local_variable_owners
        .insert(key.clone(), path.clone());
    insert_graph_residency(&mut touched.graphs, data, path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{HistoryGraphResidency, discover_touched_resources};
    use crate::project::{GraphDocumentKind, GraphResourceDocument, ProjectData};
    use std::collections::{BTreeMap, BTreeSet};
    use yss_data_contract::{DataType, DataValue};
    use yss_graph_document::GraphResourcePath;
    use yss_project_history::{
        FunctionDocumentPatch, FunctionResourceKey, FunctionSignature, ProjectHistoryTransaction,
        ResourcePatch, VariableDocumentPatch, VariableResourceKey,
    };
    use yss_project_identity::{OperationId, ResourceRevision};
    use yss_variable_contract::{VariableId, VariableInstance, VariableScope};

    const EVENT_PATH: &str = "events/Stable.yssbi-event";
    const FUNCTION_PATH: &str = "functions/Stable.yssbi-function";
    const VARIABLE_ID: &str = "7eea2f14-6d4a-4b1c-94c0-9934bf8bc244";

    fn event_path() -> GraphResourcePath {
        GraphResourcePath::new(EVENT_PATH).unwrap()
    }

    fn function_path() -> GraphResourcePath {
        GraphResourcePath::new(FUNCTION_PATH).unwrap()
    }

    fn variable_id() -> VariableId {
        uuid::Uuid::parse_str(VARIABLE_ID).unwrap().into()
    }

    fn variable(scope: VariableScope) -> VariableInstance {
        VariableInstance {
            id: variable_id(),
            name: "Stable variable".into(),
            data_type: DataType::Int64,
            data_value: DataValue::Int64(7),
            tabular: None,
            description: String::new(),
            scope,
            tags: Vec::new(),
        }
    }

    fn variable_key() -> VariableResourceKey {
        VariableResourceKey(format!("variables/{VARIABLE_ID}").into())
    }

    fn variable_patch(value: &VariableInstance) -> ResourcePatch {
        ResourcePatch::variable(
            variable_key(),
            ResourceRevision::INITIAL,
            VariableDocumentPatch::new(
                None,
                Some(serde_json::to_value(value).expect("variable serializes")),
            ),
        )
    }

    fn known_graphs(
        paths: impl IntoIterator<Item = GraphResourcePath>,
    ) -> BTreeSet<GraphResourcePath> {
        paths.into_iter().collect()
    }

    #[test]
    fn resolves_function_and_local_variable_to_exact_opaque_graph_paths() {
        let local = variable(VariableScope::Event {
            event_path: EVENT_PATH.into(),
        });
        let transaction = ProjectHistoryTransaction::new(
            OperationId::new(),
            vec![
                ResourcePatch::function(
                    FunctionResourceKey(FUNCTION_PATH.into()),
                    ResourceRevision::INITIAL,
                    FunctionDocumentPatch::new(
                        FunctionSignature::default(),
                        FunctionSignature::default(),
                    ),
                ),
                variable_patch(&local),
            ],
        );
        let mut data = ProjectData::new();
        data.graphs.insert(
            function_path(),
            GraphResourceDocument::new("Stable", GraphDocumentKind::Function),
        );
        data.variables.insert(local.id, local);

        let touched = discover_touched_resources(
            &transaction,
            true,
            &data,
            &known_graphs([event_path(), function_path()]),
        )
        .unwrap();

        assert_eq!(
            touched.graphs,
            BTreeMap::from([
                (event_path(), HistoryGraphResidency::Unloaded),
                (function_path(), HistoryGraphResidency::Loaded),
            ])
        );
        assert_eq!(
            touched.local_variable_owners,
            BTreeMap::from([(variable_key(), event_path())])
        );
        assert!(touched.global_variables.is_empty());
    }

    #[test]
    fn global_variable_remains_project_scoped() {
        let global = variable(VariableScope::Global);
        let transaction =
            ProjectHistoryTransaction::new(OperationId::new(), vec![variable_patch(&global)]);
        let mut data = ProjectData::new();
        data.variables.insert(global.id, global);

        let touched =
            discover_touched_resources(&transaction, true, &data, &BTreeSet::new()).unwrap();

        assert!(touched.graphs.is_empty());
        assert!(touched.local_variable_owners.is_empty());
        assert_eq!(touched.global_variables, BTreeSet::from([variable_key()]));
    }

    #[test]
    fn deduplicates_graph_touched_by_function_and_local_variable_patches() {
        let local = variable(VariableScope::Function {
            function_path: FUNCTION_PATH.into(),
        });
        let transaction = ProjectHistoryTransaction::new(
            OperationId::new(),
            vec![
                ResourcePatch::function(
                    FunctionResourceKey(FUNCTION_PATH.into()),
                    ResourceRevision::INITIAL,
                    FunctionDocumentPatch::new(
                        FunctionSignature::default(),
                        FunctionSignature::default(),
                    ),
                ),
                variable_patch(&local),
            ],
        );
        let data = ProjectData::new();

        let touched =
            discover_touched_resources(&transaction, true, &data, &known_graphs([function_path()]))
                .unwrap();

        assert_eq!(
            touched.graphs,
            BTreeMap::from([(function_path(), HistoryGraphResidency::Unloaded)])
        );
    }

    #[test]
    fn rejects_function_or_local_variable_with_unresolvable_owner_graph() {
        let function_transaction = ProjectHistoryTransaction::new(
            OperationId::new(),
            vec![ResourcePatch::function(
                FunctionResourceKey(FUNCTION_PATH.into()),
                ResourceRevision::INITIAL,
                FunctionDocumentPatch::new(
                    FunctionSignature::default(),
                    FunctionSignature::default(),
                ),
            )],
        );
        let local = variable(VariableScope::Event {
            event_path: EVENT_PATH.into(),
        });
        let variable_transaction =
            ProjectHistoryTransaction::new(OperationId::new(), vec![variable_patch(&local)]);

        let function_error = discover_touched_resources(
            &function_transaction,
            true,
            &ProjectData::new(),
            &BTreeSet::new(),
        )
        .unwrap_err();
        let variable_error = discover_touched_resources(
            &variable_transaction,
            true,
            &ProjectData::new(),
            &BTreeSet::new(),
        )
        .unwrap_err();

        assert!(function_error.contains(FUNCTION_PATH));
        assert!(variable_error.contains(EVENT_PATH));
    }
}
