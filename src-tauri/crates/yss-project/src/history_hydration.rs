use crate::ProjectSession;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use yss_chart_document::{ChartDocument, ChartResourcePath};
use yss_graph_document::GraphResourcePath;
use yss_project_filesystem::{ProjectFilesystemCoordinator, ProjectFilesystemLeaseSet};
use yss_project_history::{
    ChartResourceKey, HistoryMutation, HistoryPersistencePolicy, MutationRequest,
    ProjectDocumentState, ProjectHistory, ProjectHistoryMutationError, ProjectHistoryTransaction,
    ResourceKey,
};
use yss_project_identity::{HistoryEntryId, ResourceRevision};
use yss_project_layout::CHART_EXTENSION;
use yss_project_model::ProjectData;

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
    pub expected_graph_resource_revisions: BTreeMap<GraphResourcePath, ResourceRevision>,
    pub residency: BTreeMap<GraphResourcePath, HistoryGraphResidency>,
}

pub(super) struct PreparedHistoryDocuments {
    pub lease: ProjectFilesystemLeaseSet,
    pub basis: HistoryPreparationBasis,
    pub before: ProjectDocumentState,
    pub after: ProjectDocumentState,
    pub after_data: ProjectData,
    pub loaded_after_data: ProjectData,
    pub after_chart_revisions: std::collections::HashMap<ChartResourcePath, ResourceRevision>,
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
    graph_resource_revisions: std::collections::HashMap<GraphResourcePath, ResourceRevision>,
    chart_revisions: std::collections::HashMap<ChartResourcePath, ResourceRevision>,
    history: ProjectHistory,
    data: ProjectData,
    documents: ProjectDocumentState,
    touched: TouchedHistoryResources,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct TouchedHistoryResources {
    pub graphs: BTreeMap<GraphResourcePath, HistoryGraphResidency>,
    pub charts: BTreeSet<ChartResourceKey>,
}

pub(super) fn discover_touched_resources(
    transaction: &ProjectHistoryTransaction,
    _undo: bool,
    data: &ProjectData,
    known_graphs: &BTreeSet<GraphResourcePath>,
) -> Result<TouchedHistoryResources, String> {
    let mut touched = TouchedHistoryResources {
        graphs: BTreeMap::new(),
        charts: BTreeSet::new(),
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
            ResourceKey::Chart(key) => {
                touched.charts.insert(key.clone());
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
        if let Some(state) =
            state.filter(|state| state.kind == yss_project_history::ResourceLifecycleKind::Chart)
        {
            touched.charts.insert(ChartResourceKey(state.path.clone()));
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
    graph_resource_revisions: std::collections::HashMap<GraphResourcePath, ResourceRevision>,
    chart_revisions: std::collections::HashMap<ChartResourcePath, ResourceRevision>,
    history: ProjectHistory,
) -> Result<HistoryPreparationSnapshot, ProjectHistoryMutationError> {
    let known_graphs = graph_resource_revisions.keys().cloned().collect();
    let touched = discover_touched_resources(&transaction, undo, &data, &known_graphs)
        .map_err(|error| ProjectHistoryMutationError::History(error.into()))?;
    let mut documents = super::project_state::project_documents(&data)?;
    documents.chart_revisions = chart_revisions
        .iter()
        .map(|(path, revision)| (ChartResourceKey(path.as_str().into()), *revision))
        .collect();
    retain_required_documents(&mut documents, &transaction, anchor, &touched.graphs);
    let graph_resource_revisions = graph_resource_revisions
        .into_iter()
        .filter(|(path, _)| touched.graphs.contains_key(path))
        .collect();
    Ok(HistoryPreparationSnapshot {
        session,
        authority_generation,
        undo,
        transaction,
        graph_resource_revisions,
        chart_revisions,
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
    documents.charts.retain(|key, _| {
        required.contains(&ResourceKey::Chart(key.clone()))
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
    install_touched_chart_tombstone(&mut snapshot)?;
    let expected_revisions = expected_revisions(&snapshot)?;
    let current_revision = resource_revision(&snapshot, &request.resource).ok_or_else(|| {
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
    let mut after_chart_revisions = snapshot.chart_revisions.clone();
    for key in &snapshot.touched.charts {
        let path = ChartResourcePath::parse(key.0.as_ref())
            .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
        let revision = snapshot
            .chart_revisions
            .get(&path)
            .copied()
            .ok_or_else(|| {
                ProjectHistoryMutationError::History(
                    format!("Chart '{}' has no revision authority", key.0).into(),
                )
            })?
            .checked_next()
            .map_err(|error| {
                ProjectHistoryMutationError::History(
                    format!(
                        "Chart '{}' revision is exhausted at {}",
                        key.0, error.retained
                    )
                    .into(),
                )
            })?;
        after_chart_revisions.insert(path, revision);
        if let Some(document) = after.charts.get_mut(key) {
            document.revision = revision;
        }
    }

    let mut after_data = snapshot.data;
    super::project_state::replace_project_documents(&mut after_data, after.clone())?;
    synchronize_function_owner_revisions(&mut after_data, &snapshot.transaction)?;
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
            expected_graph_resource_revisions: snapshot
                .graph_resource_revisions
                .into_iter()
                .collect(),
            residency: snapshot.touched.graphs,
        },
        before,
        after,
        after_data,
        loaded_after_data,
        after_chart_revisions,
        transaction,
        proposed_history,
        touched_graphs,
        contains_unloaded_graph: !unloaded.is_empty(),
    })
}

pub(super) fn synchronize_function_owner_revisions(
    data: &mut ProjectData,
    transaction: &ProjectHistoryTransaction,
) -> Result<(), ProjectHistoryMutationError> {
    for change in &transaction.changes {
        let ResourceKey::Function(key) = &change.resource else {
            continue;
        };
        let path = GraphResourcePath::new(key.0.as_ref()).map_err(|error| {
            ProjectHistoryMutationError::History(
                format!("Function '{}' has an invalid owner graph: {error}", key.0).into(),
            )
        })?;
        let graph = data.graphs.get_mut(&path).ok_or_else(|| {
            ProjectHistoryMutationError::History(
                format!("Function '{}' owner graph '{path}' is not loaded", key.0).into(),
            )
        })?;
        graph.function.as_ref().ok_or_else(|| {
            ProjectHistoryMutationError::History(
                format!("Function '{}' owner graph has no Function document", key.0).into(),
            )
        })?;
    }
    Ok(())
}

fn hydrate_graph_document(
    snapshot: &mut HistoryPreparationSnapshot,
    graph_path: &GraphResourcePath,
) -> Result<(), ProjectHistoryMutationError> {
    let root = snapshot.session.root.as_path().to_string_lossy();
    let disk = super::project_io::load_project_graph_document_from_file(&root, graph_path)
        .map_err(history_conflict)?;
    let document_key = graph_path.clone();
    let graph = disk.document;
    snapshot
        .documents
        .graphs
        .insert(document_key, graph.clone());
    snapshot.data.graphs.insert(
        graph_path.clone(),
        yss_project_model::GraphResourceDocument {
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

    Ok(())
}

fn install_touched_chart_tombstone(
    snapshot: &mut HistoryPreparationSnapshot,
) -> Result<(), ProjectHistoryMutationError> {
    let Some(lifecycle) = &snapshot.transaction.resource_lifecycle else {
        return Ok(());
    };
    let yss_project_history::ResourceLifecycleHistoryPayload::Chart { document } =
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
    if state.kind != yss_project_history::ResourceLifecycleKind::Chart {
        return Err(ProjectHistoryMutationError::History(
            "chart lifecycle payload has a non-chart kind".into(),
        ));
    }
    let key = ChartResourceKey(state.path.clone());
    if snapshot.documents.charts.contains_key(&key) {
        return Ok(());
    }
    let path = ChartResourcePath::parse(state.path.as_ref())
        .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
    let revision = snapshot
        .chart_revisions
        .get(&path)
        .copied()
        .ok_or_else(|| {
            ProjectHistoryMutationError::History(
                format!("Chart '{}' has no tombstone revision", state.path).into(),
            )
        })?;
    let mut document = document.clone();
    document.revision = revision;
    snapshot.documents.charts.insert(key, document);
    Ok(())
}

fn expected_revisions(
    snapshot: &HistoryPreparationSnapshot,
) -> Result<BTreeMap<ResourceKey, ResourceRevision>, ProjectHistoryMutationError> {
    let mut revisions = BTreeMap::new();
    for change in &snapshot.transaction.changes {
        let revision = resource_revision(snapshot, &change.resource).ok_or_else(|| {
            ProjectHistoryMutationError::History(
                format!("touched resource {:?} was not hydrated", change.resource).into(),
            )
        })?;
        revisions.insert(change.resource.clone(), revision);
    }
    for key in &snapshot.touched.charts {
        let path = ChartResourcePath::parse(key.0.as_ref())
            .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
        let revision = snapshot
            .chart_revisions
            .get(&path)
            .copied()
            .ok_or_else(|| {
                ProjectHistoryMutationError::History(
                    format!("Chart '{}' has no revision authority", key.0).into(),
                )
            })?;
        revisions.insert(ResourceKey::Chart(key.clone()), revision);
    }
    Ok(revisions)
}

fn resource_revision(
    snapshot: &HistoryPreparationSnapshot,
    resource: &ResourceKey,
) -> Option<ResourceRevision> {
    match resource {
        ResourceKey::Graph(path) => snapshot.graph_resource_revisions.get(path).copied(),
        _ => document_revision(&snapshot.documents, resource),
    }
}

fn document_revision(
    documents: &ProjectDocumentState,
    resource: &ResourceKey,
) -> Option<ResourceRevision> {
    match resource {
        ResourceKey::Graph(_) => None,
        ResourceKey::Function(key) => documents
            .functions
            .get(key)
            .map(|document| document.revision),
        ResourceKey::Chart(key) => documents.charts.get(key).map(|document| document.revision),
        ResourceKey::Database(_) => None,
    }
}

pub(super) fn durable_filesystem_mutations(
    prepared: &PreparedHistoryDocuments,
) -> Result<Vec<yss_project_filesystem::StagedFilesystemMutation>, ProjectHistoryMutationError> {
    let mut mutations = Vec::new();
    for graph_path in &prepared.touched_graphs {
        let (relative_path, contents) =
            super::project_io::serialize_graph_document(&prepared.after_data, graph_path)
                .map_err(history_conflict)?;
        mutations.push(yss_project_filesystem::StagedFilesystemMutation::Write {
            relative_path,
            contents,
        });
    }
    for key in &prepared
        .transaction
        .changes
        .iter()
        .filter_map(|change| {
            let ResourceKey::Chart(key) = &change.resource else {
                return None;
            };
            Some(key.clone())
        })
        .collect::<BTreeSet<_>>()
    {
        push_chart_filesystem_mutation(&mut mutations, &prepared.after, key)?;
    }
    if let Some(lifecycle) = &prepared.transaction.resource_lifecycle
        && let yss_project_history::ResourceLifecycleHistoryPayload::Chart { .. } =
            lifecycle.payload
    {
        let state = lifecycle
            .forward
            .before
            .as_ref()
            .or(lifecycle.forward.after.as_ref())
            .ok_or_else(|| history_conflict("resource lifecycle patch is empty"))?;
        push_chart_filesystem_mutation(
            &mut mutations,
            &prepared.after,
            &ChartResourceKey(state.path.clone()),
        )?;
    }
    Ok(mutations)
}

fn push_chart_filesystem_mutation(
    mutations: &mut Vec<yss_project_filesystem::StagedFilesystemMutation>,
    documents: &ProjectDocumentState,
    key: &ChartResourceKey,
) -> Result<(), ProjectHistoryMutationError> {
    let path = ChartResourcePath::parse(key.0.as_ref())
        .map_err(|error| history_conflict(error.to_string()))?;
    if let Some(document) = documents.charts.get(key) {
        let (relative_path, contents) =
            crate::serialize_chart(&path, document).map_err(history_conflict)?;
        mutations.push(yss_project_filesystem::StagedFilesystemMutation::Write {
            relative_path,
            contents,
        });
    } else {
        mutations.push(
            yss_project_filesystem::StagedFilesystemMutation::RemoveFile {
                relative_path: path.relative_path().to_path_buf(),
            },
        );
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
        == Some(CHART_EXTENSION)
    {
        return serde_json::from_slice::<ChartDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string());
    }
    let graph_path = GraphResourcePath::new(relative_path.to_string_lossy().replace('\\', "/"))
        .map_err(|error| error.to_string())?;
    let kind = graph_path.kind();
    super::project_io::parse_graph_resource_document(contents, relative_path, kind)
        .map(|_| ())
        .map_err(|error| error.to_string())
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

#[cfg(test)]
mod tests {
    use super::{HistoryGraphResidency, discover_touched_resources};
    use std::collections::{BTreeMap, BTreeSet};
    use yss_graph_document::GraphResourcePath;
    use yss_project_history::{
        FunctionDocumentPatch, FunctionResourceKey, FunctionSignature, ProjectHistoryTransaction,
        ResourcePatch,
    };
    use yss_project_identity::{OperationId, ResourceRevision};
    use yss_project_model::{GraphResourceDocument, ProjectData};

    const FUNCTION_PATH: &str = "functions/Stable.yssbi-function";

    fn function_path() -> GraphResourcePath {
        GraphResourcePath::new(FUNCTION_PATH).unwrap()
    }

    fn known_graphs(
        paths: impl IntoIterator<Item = GraphResourcePath>,
    ) -> BTreeSet<GraphResourcePath> {
        paths.into_iter().collect()
    }

    #[test]
    fn resolves_function_to_exact_opaque_graph_path() {
        let transaction = ProjectHistoryTransaction::new(
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
        let mut data = ProjectData::new();
        data.graphs.insert(
            function_path(),
            GraphResourceDocument::new("Stable", yss_graph_document::GraphResourceKind::Function),
        );

        let touched =
            discover_touched_resources(&transaction, true, &data, &known_graphs([function_path()]))
                .unwrap();

        assert_eq!(
            touched.graphs,
            BTreeMap::from([(function_path(), HistoryGraphResidency::Loaded),])
        );
    }

    #[test]
    fn rejects_function_with_unresolvable_owner_graph() {
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

        let function_error = discover_touched_resources(
            &function_transaction,
            true,
            &ProjectData::new(),
            &BTreeSet::new(),
        )
        .unwrap_err();
        assert!(function_error.contains(FUNCTION_PATH));
    }
}
