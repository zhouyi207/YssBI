use crate::node_system::document::{OperationId, ResourceKey, ResourceRevision};
use crate::project::{
    GraphDocumentKind, GraphResourceDocument, GraphResourcePath, ProjectFilesystemError,
    ProjectFilesystemTransaction, ProjectInstanceId, ProjectState, ProjectTransactionContext,
    ResourceDocumentPatch, StagedFilesystemMutation,
};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ResourceOperationOwner {
    project_instance_id: ProjectInstanceId,
    session_generation: u64,
}

pub(super) struct ResourceOperationLedger {
    owner: ResourceOperationOwner,
    in_flight: HashSet<OperationId>,
    completed: HashSet<OperationId>,
}

impl ResourceOperationLedger {
    pub(super) fn new(project_instance_id: ProjectInstanceId) -> Self {
        Self {
            owner: ResourceOperationOwner {
                project_instance_id,
                session_generation: 0,
            },
            in_flight: HashSet::new(),
            completed: HashSet::new(),
        }
    }

    pub(super) fn reset_for_project(&mut self, project_instance_id: ProjectInstanceId) {
        self.owner = ResourceOperationOwner {
            project_instance_id,
            session_generation: self.owner.session_generation.saturating_add(1),
        };
        self.in_flight.clear();
        self.completed.clear();
    }

    fn reserve(
        ledger: std::sync::Arc<std::sync::Mutex<Self>>,
        project_instance_id: &ProjectInstanceId,
        operation_id: OperationId,
    ) -> Result<ResourceOperationReservation, ProjectFilesystemError> {
        let mut state = ledger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.owner.project_instance_id != *project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "resource operation belongs to a replaced project session".into(),
            });
        }
        if state.in_flight.contains(&operation_id) || state.completed.contains(&operation_id) {
            return Err(ProjectFilesystemError::DuplicateOperation {
                message: format!(
                    "operation '{}' was already admitted for project '{}'",
                    operation_id, project_instance_id
                ),
            });
        }
        state.in_flight.insert(operation_id);
        let owner = state.owner.clone();
        drop(state);
        Ok(ResourceOperationReservation {
            ledger,
            owner,
            operation_id,
            completed: false,
        })
    }
}

pub(crate) struct ResourceOperationReservation {
    ledger: std::sync::Arc<std::sync::Mutex<ResourceOperationLedger>>,
    owner: ResourceOperationOwner,
    operation_id: OperationId,
    completed: bool,
}

impl ResourceOperationReservation {
    pub(crate) fn complete(mut self) {
        let mut state = self
            .ledger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.owner == self.owner {
            state.in_flight.remove(&self.operation_id);
            state.completed.insert(self.operation_id);
        }
        self.completed = true;
    }
}

impl Drop for ResourceOperationReservation {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let mut state = self
            .ledger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.owner == self.owner {
            state.in_flight.remove(&self.operation_id);
        }
    }
}

pub(crate) fn remap_variable_scope_path(
    scope: &mut crate::variable::VariableScope,
    from: &str,
    to: &str,
) -> bool {
    let from = crate::project::graph_resource_index::normalize_resource_path(from);
    let to = crate::project::graph_resource_index::normalize_resource_path(to);
    match scope {
        crate::variable::VariableScope::Event { event_path }
            if crate::project::graph_resource_index::normalize_resource_path(event_path)
                == from =>
        {
            *event_path = to;
            true
        }
        crate::variable::VariableScope::Function { function_path }
            if crate::project::graph_resource_index::normalize_resource_path(function_path)
                == from =>
        {
            *function_path = to;
            true
        }
        _ => false,
    }
}

pub(crate) fn remap_graph_document_references(
    document: &mut crate::node_system::document::GraphDocument,
    from: &str,
    to: &str,
) -> bool {
    let from = crate::project::graph_resource_index::normalize_resource_path(from);
    let to = crate::project::graph_resource_index::normalize_resource_path(to);
    let mut changed = false;
    for node in document.nodes.values_mut() {
        for value in node.parameters.values_mut() {
            if value.as_str().is_some_and(|path| {
                crate::project::graph_resource_index::normalize_resource_path(path) == from
            }) {
                *value = serde_json::Value::String(to.clone());
                changed = true;
            }
        }
    }
    changed
}

fn graph_key(path: &GraphResourcePath) -> ResourceKey {
    ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        path.as_str().into(),
    ))
}

fn resource_context(
    state: &ProjectState,
    session: crate::project::ProjectSession,
    operation_id: OperationId,
    expected: impl IntoIterator<Item = (ResourceKey, ResourceRevision)>,
    absent: impl IntoIterator<Item = ResourceKey>,
) -> ProjectTransactionContext {
    let expected_revisions = expected.into_iter().collect::<BTreeMap<_, _>>();
    ProjectTransactionContext {
        session,
        operation_id,
        affected_resources: expected_revisions.keys().cloned().collect(),
        expected_revisions,
        expected_absent_resources: absent.into_iter().collect(),
        recovery_marker: Some(state.project_recovery_marker()),
    }
}

fn build_graph_shell(
    path: &GraphResourcePath,
    name: String,
    kind: GraphDocumentKind,
) -> Result<GraphResourceDocument, ProjectFilesystemError> {
    let mut resource = GraphResourceDocument::new(name, kind);
    let shell_types: &[(&str, f64)] = match kind {
        GraphDocumentKind::Event => &[("yssbi.project.event.begin", 120.0)],
        GraphDocumentKind::Function => &[
            ("yssbi.project.function.entry", 120.0),
            ("yssbi.project.function.return", 560.0),
        ],
    };
    let mut shell_nodes = Vec::new();
    for (node_type, x) in shell_types {
        let id = crate::node_system::document::NodeId::new();
        let parameters = if kind == GraphDocumentKind::Function {
            [(
                crate::node_system::protocol::ParameterKey::new("function").map_err(|error| {
                    ProjectFilesystemError::TransactionPrepareFailed {
                        message: error.to_string(),
                    }
                })?,
                serde_json::Value::String(path.as_str().into()),
            )]
            .into_iter()
            .collect()
        } else {
            Default::default()
        };
        resource.document.nodes.insert(
            id,
            crate::node_system::document::DocumentNode {
                id,
                node_type: crate::node_system::protocol::NodeTypeId::new(*node_type).map_err(
                    |error| ProjectFilesystemError::TransactionPrepareFailed {
                        message: error.to_string(),
                    },
                )?,
                position: crate::node_system::document::NodePosition { x: *x, y: 160.0 },
                parameters,
                user_label: None,
            },
        );
        shell_nodes.push(id);
    }
    if let [entry, returned] = shell_nodes.as_slice() {
        let id = crate::node_system::document::ConnectionId::new();
        resource.document.connections.insert(
            id,
            crate::node_system::document::DocumentConnection {
                id,
                output: crate::node_system::document::PortAddress::declared(
                    *entry,
                    crate::node_system::protocol::PortKey::new("then").map_err(|error| {
                        ProjectFilesystemError::TransactionPrepareFailed {
                            message: error.to_string(),
                        }
                    })?,
                ),
                input: crate::node_system::document::PortAddress::declared(
                    *returned,
                    crate::node_system::protocol::PortKey::new("enter").map_err(|error| {
                        ProjectFilesystemError::TransactionPrepareFailed {
                            message: error.to_string(),
                        }
                    })?,
                ),
                order: None,
            },
        );
    }
    Ok(resource)
}

fn validate_graph_bytes(_: &std::path::Path, contents: &[u8]) -> Result<(), String> {
    serde_json::from_slice::<crate::project::project_io::GraphDocument>(contents)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn rebind_duplicate(
    mut document: crate::project::project_io::GraphDocument,
    source: &GraphResourcePath,
    target: &GraphResourcePath,
    name: String,
) -> crate::project::project_io::GraphDocument {
    let node_ids = document
        .document
        .nodes
        .keys()
        .copied()
        .map(|id| (id, crate::node_system::document::NodeId::new()))
        .collect::<HashMap<_, _>>();
    document.document.nodes = document
        .document
        .nodes
        .into_values()
        .map(|mut node| {
            node.id = node_ids[&node.id];
            for value in node.parameters.values_mut() {
                if value.as_str().is_some_and(|value| {
                    crate::project::graph_resource_index::normalize_resource_path(value)
                        == source.as_str()
                }) {
                    *value = serde_json::Value::String(target.as_str().into());
                }
            }
            (node.id, node)
        })
        .collect();
    document.document.connections = document
        .document
        .connections
        .into_values()
        .map(|mut connection| {
            connection.id = crate::node_system::document::ConnectionId::new();
            connection.output.node_id = node_ids[&connection.output.node_id];
            connection.input.node_id = node_ids[&connection.input.node_id];
            (connection.id, connection)
        })
        .collect();
    document.local_variables = document
        .local_variables
        .into_values()
        .map(|mut variable| {
            variable.id = crate::variable::VariableId::new();
            variable.scope = match document.kind {
                GraphDocumentKind::Event => crate::variable::VariableScope::Event {
                    event_path: target.as_str().into(),
                },
                GraphDocumentKind::Function => crate::variable::VariableScope::Function {
                    function_path: target.as_str().into(),
                },
            };
            (variable.id, variable)
        })
        .collect();
    document.name = name;
    document.revision = ResourceRevision::INITIAL;
    document.document.revision = ResourceRevision::INITIAL;
    if let Some(function) = document.function.as_mut() {
        function.revision = ResourceRevision::INITIAL;
    }
    document
}

fn resource_from_disk_document(
    document: &crate::project::project_io::GraphDocument,
) -> GraphResourceDocument {
    let mut graph = document.document.clone();
    graph.revision = document.revision;
    GraphResourceDocument {
        name: document.name.clone(),
        kind: document.kind,
        document: graph,
        function: document.function.clone(),
    }
}

fn duplicate_revision_conflict(
    source: &GraphResourcePath,
    message: impl Into<String>,
) -> ProjectFilesystemError {
    ProjectFilesystemError::ResourceRevisionConflict {
        message: format!("duplicate source '{}': {}", source, message.into()),
    }
}

fn validate_loaded_duplicate_source_authority(
    source: &GraphResourcePath,
    resource: &GraphResourceDocument,
    authority_revision: ResourceRevision,
) -> Result<(), ProjectFilesystemError> {
    if resource.document.revision != authority_revision {
        return Err(duplicate_revision_conflict(
            source,
            format!(
                "owner revision {} differs from ledger revision {}",
                resource.document.revision.get(),
                authority_revision.get()
            ),
        ));
    }
    if resource.kind == GraphDocumentKind::Function {
        let embedded_revision = resource
            .function
            .as_ref()
            .map(|function| function.revision)
            .ok_or_else(|| {
                duplicate_revision_conflict(source, "loaded function metadata is missing")
            })?;
        if embedded_revision != authority_revision {
            return Err(duplicate_revision_conflict(
                source,
                format!(
                    "embedded function revision {} differs from ledger revision {}",
                    embedded_revision.get(),
                    authority_revision.get()
                ),
            ));
        }
    }
    Ok(())
}

fn bind_unloaded_duplicate_source_authority(
    source: &GraphResourcePath,
    document: &mut crate::project::project_io::GraphDocument,
    authority_revision: ResourceRevision,
) -> Result<(), ProjectFilesystemError> {
    if document.kind != GraphDocumentKind::Function {
        if document.revision != authority_revision {
            return Err(duplicate_revision_conflict(
                source,
                "persisted event revision differs from the ledger revision",
            ));
        }
        document.document.revision = authority_revision;
        return Ok(());
    }

    let embedded_revision = document
        .function
        .as_ref()
        .map(|function| function.revision)
        .ok_or_else(|| {
            duplicate_revision_conflict(source, "persisted function metadata is missing")
        })?;
    if document.revision != embedded_revision {
        return Err(duplicate_revision_conflict(
            source,
            format!(
                "persisted owner {} and embedded function {} revisions are incoherent",
                document.revision.get(),
                embedded_revision.get()
            ),
        ));
    }
    if document.revision > authority_revision || embedded_revision > authority_revision {
        return Err(duplicate_revision_conflict(
            source,
            format!(
                "persisted revision {} is ahead of retained ledger revision {}",
                document.revision.get(),
                authority_revision.get()
            ),
        ));
    }

    document.revision = authority_revision;
    document.document.revision = authority_revision;
    document
        .function
        .as_mut()
        .expect("persisted function metadata was validated")
        .revision = authority_revision;
    Ok(())
}

impl ProjectState {
    pub(crate) fn reserve_resource_operation(
        &self,
        project_instance_id: &ProjectInstanceId,
        operation_id: OperationId,
    ) -> Result<ResourceOperationReservation, ProjectFilesystemError> {
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != project_instance_id.as_str() {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project changed before resource operation admission".into(),
            });
        }
        let reservation = ResourceOperationLedger::reserve(
            std::sync::Arc::clone(&self.resource_operations),
            project_instance_id,
            operation_id,
        );
        drop(publication);
        reservation
    }

    pub fn create_graph_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        name: &str,
        kind: GraphDocumentKind,
        operation_id: OperationId,
    ) -> Result<crate::event::ResourceMutationResultDto, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project changed before graph creation".into(),
            });
        }
        let reservation = self.reserve_resource_operation(&session.instance_id, operation_id)?;
        let planning_data = self.get_data()?;
        let (_planned, _) =
            Self::allocate_graph_path_from_snapshot(None, &planning_data, name, kind)?;
        #[cfg(test)]
        self.run_resource_mutation_test_hook(ResourceMutationTestPoint::Planned, Some(&_planned));
        let lease = self.filesystem().acquire(session.root.clone())?;
        self.validate_project_session(&session)?;
        let current_data = self.get_data()?;
        let (path, unique_name) = Self::allocate_graph_path_from_snapshot(
            session.root.as_path().to_str(),
            &current_data,
            name,
            kind,
        )?;
        let mut resource = build_graph_shell(&path, unique_name, kind)?;
        let retained_revision = self.graph_revisions.read().unwrap().get(&path).copied();
        crate::project::project_state::normalize_function_resource_revision(
            &path,
            &mut resource,
            retained_revision,
        )?;
        let contents = crate::project::project_io::serialize_graph_resource_document(
            &resource,
            HashMap::new(),
        )
        .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
            message: error.to_string(),
        })?;
        let context = resource_context(self, session.clone(), operation_id, [], [graph_key(&path)]);
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context,
            lease,
            vec![StagedFilesystemMutation::Write {
                relative_path: path.as_str().into(),
                contents,
            }],
            validate_graph_bytes,
        )?;
        #[cfg(test)]
        self.run_resource_mutation_test_hook(ResourceMutationTestPoint::Prepared, Some(&path));
        self.validate_project_session(&session)?;
        let committed = prepared.commit()?;
        #[cfg(test)]
        self.run_resource_mutation_test_hook(ResourceMutationTestPoint::Committed, Some(&path));
        let inserted = self.insert_graph(path.clone(), resource)?;
        let unload_context = resource_context(
            self,
            session,
            operation_id,
            [(graph_key(&path), inserted.document.revision)],
            [],
        );
        #[cfg(test)]
        self.run_resource_mutation_test_hook(
            ResourceMutationTestPoint::BeforePublication,
            Some(&path),
        );
        let result = match self.apply_resource_document_patch(
            &unload_context,
            ResourceDocumentPatch::UnloadGraph { path: path.clone() },
        ) {
            Ok(result) => {
                committed.finalize();
                Ok(result)
            }
            Err(error) => {
                let _ = self.unload_graph_resource(&path);
                committed
                    .rollback()
                    .map_err(|rollback| rollback)
                    .and(Err(error))
            }
        };
        if result.is_ok() {
            reservation.complete();
        }
        result
    }

    pub fn duplicate_graph_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        source: &GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<crate::event::ResourceMutationResultDto, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project changed before graph duplication".into(),
            });
        }
        let reservation = self.reserve_resource_operation(&session.instance_id, operation_id)?;
        let source_name = self
            .get_data()?
            .graphs
            .get(source)
            .map(|graph| graph.name.clone())
            .unwrap_or_else(|| "Copy".into());
        let source_kind =
            source
                .kind()
                .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                })?;
        let planning_data = self.get_data()?;
        let (_planned, _) = Self::allocate_graph_path_from_snapshot(
            None,
            &planning_data,
            &source_name,
            source_kind,
        )?;
        #[cfg(test)]
        self.run_resource_mutation_test_hook(ResourceMutationTestPoint::Planned, Some(&_planned));
        let lease = self.filesystem().acquire(session.root.clone())?;
        self.validate_project_session(&session)?;
        let source_bytes = crate::project::read_secure_project_file(
            session.root.as_path(),
            std::path::Path::new(source.as_str()),
        )
        .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
            message: error.to_string(),
        })?;
        let persisted_source: crate::project::project_io::GraphDocument =
            serde_json::from_slice(&source_bytes).map_err(|error| {
                ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                }
            })?;
        let (current_data, authority_revision) = {
            let publication = self.mutation_publication.lock().unwrap();
            if publication.project_instance_id != session.instance_id.as_str() {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "project changed during graph duplication".into(),
                });
            }
            let data = self.project_data.read().unwrap();
            let revisions = self.graph_revisions.read().unwrap();
            (data.clone(), revisions.get(source).copied())
        };
        let authority_revision = authority_revision.ok_or_else(|| {
            duplicate_revision_conflict(source, "authoritative ledger revision is missing")
        })?;
        if authority_revision != expected_revision {
            return Err(duplicate_revision_conflict(
                source,
                format!(
                    "caller expected revision {}, ledger is {}",
                    expected_revision.get(),
                    authority_revision.get()
                ),
            ));
        }
        let source_document = if let Some(resource) = current_data.graphs.get(source) {
            validate_loaded_duplicate_source_authority(source, resource, authority_revision)?;
            crate::project::project_io::snapshot_graph_document(&current_data, source).map_err(
                |error| ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                },
            )?
        } else {
            let mut persisted_source = persisted_source;
            bind_unloaded_duplicate_source_authority(
                source,
                &mut persisted_source,
                authority_revision,
            )?;
            persisted_source
        };
        let (target, unique_name) = Self::allocate_graph_path_from_snapshot(
            session.root.as_path().to_str(),
            &current_data,
            &source_document.name,
            source_document.kind,
        )?;
        let duplicate = rebind_duplicate(source_document, source, &target, unique_name);
        let contents = serde_json::to_vec_pretty(&duplicate).map_err(|error| {
            ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            }
        })?;
        let context = resource_context(
            self,
            session.clone(),
            operation_id,
            [(graph_key(source), expected_revision)],
            [graph_key(&target)],
        );
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context,
            lease,
            vec![StagedFilesystemMutation::Write {
                relative_path: target.as_str().into(),
                contents,
            }],
            validate_graph_bytes,
        )?;
        #[cfg(test)]
        self.run_resource_mutation_test_hook(ResourceMutationTestPoint::Prepared, Some(&target));
        self.validate_project_session(&session)?;
        let committed = prepared.commit()?;
        #[cfg(test)]
        self.run_resource_mutation_test_hook(ResourceMutationTestPoint::Committed, Some(&target));
        self.insert_graph(target.clone(), resource_from_disk_document(&duplicate))?;
        let unload_context = resource_context(
            self,
            session,
            operation_id,
            [(graph_key(&target), ResourceRevision::INITIAL)],
            [],
        );
        #[cfg(test)]
        self.run_resource_mutation_test_hook(
            ResourceMutationTestPoint::BeforePublication,
            Some(&target),
        );
        let result = match self.apply_resource_document_patch(
            &unload_context,
            ResourceDocumentPatch::UnloadGraph {
                path: target.clone(),
            },
        ) {
            Ok(result) => {
                committed.finalize();
                Ok(result)
            }
            Err(error) => {
                let _ = self.unload_graph_resource(&target);
                committed
                    .rollback()
                    .map_err(|rollback| rollback)
                    .and(Err(error))
            }
        };
        if result.is_ok() {
            reservation.complete();
        }
        result
    }

    pub fn remove_graph_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<crate::event::ResourceMutationResultDto, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project changed before graph removal".into(),
            });
        }
        let reservation = self.reserve_resource_operation(&session.instance_id, operation_id)?;
        let tracked_revision = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .get(graph_path)
            .map(|graph| graph.document.revision)
            .or_else(|| {
                self.graph_revisions
                    .read()
                    .unwrap()
                    .get(graph_path)
                    .copied()
            });
        if tracked_revision.is_some_and(|revision| revision != expected_revision) {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
                message: format!("revision for '{}' changed", graph_path),
            });
        }
        let expected = tracked_revision
            .map(|revision| (graph_key(graph_path), revision))
            .into_iter();
        let context = resource_context(self, session.clone(), operation_id, expected, []);
        let lease = self.filesystem().acquire(session.root.clone())?;
        self.validate_project_session(&session)?;
        let disk: crate::project::project_io::GraphDocument = serde_json::from_slice(
            &crate::project::read_secure_project_file(
                session.root.as_path(),
                std::path::Path::new(graph_path.as_str()),
            )
            .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            })?,
        )
        .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
            message: error.to_string(),
        })?;
        if disk.revision != expected_revision {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
                message: format!("revision for '{}' changed", graph_path),
            });
        }
        crate::project::project_state::validate_context_revisions(
            &context,
            &self.project_data.read().unwrap(),
            &self.graph_revisions.read().unwrap(),
            &self.variable_revisions.read().unwrap(),
            &self.worksheet_revisions.read().unwrap(),
        )?;
        let prepared = ProjectFilesystemTransaction::prepare(
            context.clone(),
            lease,
            vec![StagedFilesystemMutation::RemoveFile {
                relative_path: graph_path.as_str().into(),
            }],
        )?;
        #[cfg(test)]
        self.run_resource_mutation_test_hook(ResourceMutationTestPoint::Prepared, Some(graph_path));
        let committed = prepared.commit()?;
        #[cfg(test)]
        self.run_resource_mutation_test_hook(
            ResourceMutationTestPoint::Committed,
            Some(graph_path),
        );
        #[cfg(test)]
        self.run_resource_mutation_test_hook(
            ResourceMutationTestPoint::BeforePublication,
            Some(graph_path),
        );
        let result = match self.apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::RemoveGraph {
                path: graph_path.clone(),
                revision: expected_revision,
            },
        ) {
            Ok(result) => {
                committed.finalize();
                Ok(result)
            }
            Err(error) => match committed.rollback() {
                Ok(()) => Err(error),
                Err(rollback) => Err(rollback),
            },
        };
        if result.is_ok() {
            reservation.complete();
        }
        result
    }

    pub fn rename_graph_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        expected_revision: ResourceRevision,
        new_name: &str,
        lifecycle_token: u64,
        operation_id: OperationId,
    ) -> Result<crate::event::ResourceMutationResultDto, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project changed before graph rename".into(),
            });
        }
        let reservation = self.reserve_resource_operation(&session.instance_id, operation_id)?;
        let result = self.rename_graph_resource_transaction_impl(
            expected_project_instance_id,
            graph_path,
            expected_revision,
            new_name,
            lifecycle_token,
            operation_id,
        );
        if result.is_ok() {
            reservation.complete();
        }
        result
    }
}

#[cfg(test)]
pub(crate) struct GraphRenameFixtureResult {
    pub(crate) path: GraphResourcePath,
    pub(crate) publication: crate::event::ResourceMutationResultDto,
}

#[cfg(test)]
impl std::ops::Deref for GraphRenameFixtureResult {
    type Target = GraphResourcePath;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

#[cfg(test)]
impl PartialEq<GraphResourcePath> for GraphRenameFixtureResult {
    fn eq(&self, other: &GraphResourcePath) -> bool {
        self.path == *other
    }
}

#[cfg(test)]
impl std::fmt::Debug for GraphRenameFixtureResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GraphRenameFixtureResult")
            .field("path", &self.path)
            .field(
                "publication_revision",
                &self.publication.publication_revision,
            )
            .finish()
    }
}

#[cfg(test)]
impl ProjectState {
    pub(crate) fn create_graph_resource_fixture(
        &self,
        name: &str,
        kind: GraphDocumentKind,
    ) -> Result<GraphResourcePath, String> {
        let session = self
            .capture_project_session()
            .map_err(|error| error.to_string())?;
        let result = self
            .create_graph_resource_transaction(&session.instance_id, name, kind, OperationId::new())
            .map_err(|error| error.to_string())?;
        fixture_result_path(&result).ok_or_else(|| "create result omitted graph path".into())
    }

    pub(crate) fn duplicate_graph_resource_fixture(
        &self,
        source: &GraphResourcePath,
    ) -> Result<GraphResourcePath, String> {
        let session = self
            .capture_project_session()
            .map_err(|error| error.to_string())?;
        let revision = self
            .graph_revisions
            .read()
            .unwrap()
            .get(source)
            .copied()
            .unwrap_or(ResourceRevision::INITIAL);
        let result = self
            .duplicate_graph_resource_transaction(
                &session.instance_id,
                source,
                revision,
                OperationId::new(),
            )
            .map_err(|error| error.to_string())?;
        fixture_result_path(&result).ok_or_else(|| "duplicate result omitted graph path".into())
    }

    pub(crate) fn remove_graph_resource_fixture(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<(), String> {
        let session = self
            .capture_project_session()
            .map_err(|error| error.to_string())?;
        let revision = self
            .graph_revisions
            .read()
            .unwrap()
            .get(graph_path)
            .copied()
            .unwrap_or(ResourceRevision::INITIAL);
        self.remove_graph_resource_transaction(
            &session.instance_id,
            graph_path,
            revision,
            OperationId::new(),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
    }

    pub(crate) fn rename_graph_resource_fixture(
        &self,
        expected_project_instance_id: &str,
        graph_path: &GraphResourcePath,
        new_name: &str,
    ) -> Result<GraphRenameFixtureResult, ProjectFilesystemError> {
        let revision = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .get(graph_path)
            .map(|graph| graph.document.revision)
            .or_else(|| {
                self.graph_revisions
                    .read()
                    .unwrap()
                    .get(graph_path)
                    .copied()
            })
            .unwrap_or(ResourceRevision::INITIAL);
        let expected_project_instance_id =
            ProjectInstanceId::from_existing(expected_project_instance_id.to_string());
        let mut token = 1;
        let publication = loop {
            match self.rename_graph_resource_transaction(
                &expected_project_instance_id,
                graph_path,
                revision,
                new_name,
                token,
                OperationId::new(),
            ) {
                Ok(publication) => break publication,
                Err(ProjectFilesystemError::StaleResourceLifecycle { .. })
                    if self.project_instance_id() == expected_project_instance_id.as_str()
                        && token < 16 =>
                {
                    token += 1;
                }
                Err(error) => return Err(error),
            }
        };
        let path = publication
            .moves
            .first()
            .map(|moved| GraphResourcePath::new(moved.to.clone()))
            .transpose()
            .map_err(|error| ProjectFilesystemError::TransactionCommitFailed {
                message: error.to_string(),
            })?
            .ok_or_else(|| ProjectFilesystemError::TransactionCommitFailed {
                message: "rename result omitted move target".into(),
            })?;
        Ok(GraphRenameFixtureResult { path, publication })
    }
}

#[cfg(test)]
fn fixture_result_path(
    result: &crate::event::ResourceMutationResultDto,
) -> Option<GraphResourcePath> {
    let paths = match &result.projection_status {
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths,
        } => expected_graph_paths,
        crate::event::ProjectionStatusDto::Incomplete {
            invalidated_graph_paths,
        } => invalidated_graph_paths,
    };
    paths
        .iter()
        .find(|path| path.starts_with("events/") || path.starts_with("functions/"))
        .and_then(|path| GraphResourcePath::new(path.clone()).ok())
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResourceMutationTestPoint {
    Planned,
    Prepared,
    Committed,
    BeforePublication,
}

#[cfg(test)]
pub(crate) type ResourceMutationTestHook = std::sync::Arc<
    dyn Fn(ResourceMutationTestPoint, Option<&crate::project::GraphResourcePath>) + Send + Sync,
>;

#[cfg(test)]
impl ProjectState {
    pub(crate) fn set_resource_mutation_test_hook(&self, hook: Option<ResourceMutationTestHook>) {
        *self.resource_mutation_test_hook.write().unwrap() = hook;
    }

    pub(crate) fn run_resource_mutation_test_hook(
        &self,
        point: ResourceMutationTestPoint,
        path: Option<&crate::project::GraphResourcePath>,
    ) {
        let hook = self.resource_mutation_test_hook.read().unwrap().clone();
        if let Some(hook) = hook {
            hook(point, path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ResourceMutationTestPoint;
    use crate::graph::value::{DataType, DataValue};
    use crate::node_system::document::{
        DocumentNode, NodeId, OperationId, ParameterValues, ResourceRevision,
    };
    use crate::node_system::protocol::NodeTypeId;
    use crate::project::{
        GraphDocument, GraphDocumentKind, GraphResourceDocument, GraphResourcePath, ProjectData,
        ProjectFilesystemError, ProjectState, ResourceNameError,
    };
    use crate::variable::{VariableId, VariableInstance, VariableScope};
    use std::collections::{BTreeMap, BTreeSet, HashMap};
    use std::sync::{Arc, Mutex};

    struct TestProject {
        root: std::path::PathBuf,
    }

    impl TestProject {
        fn new(label: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "yssbi-resource-mutation-{label}-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn state(&self, data: ProjectData) -> ProjectState {
            crate::project::fixtures::write_project(&data, self.root.to_string_lossy().as_ref())
                .unwrap();
            for graph_path in data.graphs.keys() {
                crate::project::fixtures::write_graph(
                    &data,
                    self.root.to_string_lossy().as_ref(),
                    graph_path,
                )
                .unwrap();
            }
            let state = ProjectState::new();
            state.activate_project_fixture(self.root.to_string_lossy().into_owned(), data);
            state
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn graph_path(path: &str) -> GraphResourcePath {
        GraphResourcePath::new(path).unwrap()
    }

    fn function_data(
        path: &GraphResourcePath,
        owner_revision: ResourceRevision,
        embedded_revision: ResourceRevision,
    ) -> ProjectData {
        let mut resource = GraphResourceDocument::new("Source", GraphDocumentKind::Function);
        resource.document.revision = owner_revision;
        resource.function.as_mut().unwrap().revision = embedded_revision;
        let mut data = ProjectData::new();
        data.graphs.insert(path.clone(), resource);
        data
    }

    fn function_files(root: &std::path::Path) -> BTreeMap<String, Vec<u8>> {
        let directory = root.join("functions");
        std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    std::fs::read(entry.path()).unwrap(),
                )
            })
            .collect()
    }

    fn duplicate_boundary_snapshot(
        state: &ProjectState,
        root: &std::path::Path,
    ) -> (
        serde_json::Value,
        HashMap<GraphResourcePath, ResourceRevision>,
        (u64, u64),
        BTreeMap<String, Vec<u8>>,
    ) {
        let data = serde_json::to_value(state.get_data().unwrap()).unwrap();
        let graph_revisions = state.graph_revisions.read().unwrap().clone();
        let publication = state.mutation_publication.lock().unwrap();
        let publication_state = (
            publication.resource_revision,
            publication.authority_generation(),
        );
        drop(publication);
        (
            data,
            graph_revisions,
            publication_state,
            function_files(root),
        )
    }

    fn assert_duplicate_revision_conflict_without_effects(
        state: &ProjectState,
        root: &std::path::Path,
        source: &GraphResourcePath,
        expected_revision: ResourceRevision,
    ) {
        let session = state.capture_project_session().unwrap();
        let operation_id = OperationId::new();
        let before = duplicate_boundary_snapshot(state, root);
        for _ in 0..2 {
            let error = state
                .duplicate_graph_resource_transaction(
                    &session.instance_id,
                    source,
                    expected_revision,
                    operation_id,
                )
                .unwrap_err();
            assert_eq!(error.code(), "resource_revision_conflict", "{error}");
            assert_eq!(duplicate_boundary_snapshot(state, root), before);
        }
    }

    fn rewrite_persisted_function_revisions(
        root: &std::path::Path,
        source: &GraphResourcePath,
        owner_revision: ResourceRevision,
        graph_revision: ResourceRevision,
        embedded_revision: ResourceRevision,
    ) {
        let path = root.join(source.as_str());
        let mut document: GraphDocument =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        document.revision = owner_revision;
        document.document.revision = graph_revision;
        document.function.as_mut().unwrap().revision = embedded_revision;
        std::fs::write(path, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
    }

    #[test]
    fn graph_rename_preserves_case_only_target_without_suffixing() {
        let source = graph_path("events/Sales.yssbi-event");
        let mut data = ProjectData::new();
        data.graphs.insert(
            source.clone(),
            GraphResourceDocument::new("Sales", GraphDocumentKind::Event),
        );
        let project = TestProject::new("case-only-rename-allocation");
        let state = project.state(data);

        let renamed = state
            .rename_graph_resource_fixture(&state.project_instance_id(), &source, "sales")
            .unwrap();

        assert_eq!(renamed.path.as_str(), "events/sales.yssbi-event");
    }

    #[test]
    fn graph_rename_rejects_exact_portable_conflict_without_suffixing() {
        let source = graph_path("events/Sales.yssbi-event");
        let existing = graph_path("events/Report.yssbi-event");
        let mut data = ProjectData::new();
        data.graphs.insert(
            source.clone(),
            GraphResourceDocument::new("Sales", GraphDocumentKind::Event),
        );
        data.graphs.insert(
            existing.clone(),
            GraphResourceDocument::new("Report", GraphDocumentKind::Event),
        );
        let project = TestProject::new("rename-portable-conflict");
        let state = project.state(data);
        let before = serde_json::to_value(state.get_data().unwrap()).unwrap();

        let error = state
            .rename_graph_resource_fixture(&state.project_instance_id(), &source, "report")
            .unwrap_err();

        assert_eq!(error.code(), "resource_name_conflict");
        assert_eq!(
            serde_json::to_value(state.get_data().unwrap()).unwrap(),
            before
        );
        assert!(project.root.join(source.as_str()).is_file());
        assert!(project.root.join(existing.as_str()).is_file());
        assert!(!project.root.join("events/report 1.yssbi-event").exists());
    }

    #[test]
    fn graph_rename_still_rejects_invalid_resource_name() {
        let source = graph_path("events/Sales.yssbi-event");
        let mut data = ProjectData::new();
        data.graphs.insert(
            source.clone(),
            GraphResourceDocument::new("Sales", GraphDocumentKind::Event),
        );
        let project = TestProject::new("invalid-rename-name");
        let state = project.state(data);

        let error = state
            .rename_graph_resource_fixture(&state.project_instance_id(), &source, "Sales/Report")
            .unwrap_err();

        assert_eq!(
            error,
            ProjectFilesystemError::InvalidResourceName(ResourceNameError::ForbiddenCharacter('/'))
        );
    }

    #[test]
    fn graph_create_rejects_invalid_resource_name_without_effects() {
        let project = TestProject::new("invalid-create-name");
        let state = project.state(ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let before = serde_json::to_value(state.get_data().unwrap()).unwrap();

        let error = state
            .create_graph_resource_transaction(
                &session.instance_id,
                "Sales/Report",
                GraphDocumentKind::Event,
                OperationId::new(),
            )
            .unwrap_err();

        assert_eq!(
            error,
            ProjectFilesystemError::InvalidResourceName(ResourceNameError::ForbiddenCharacter('/'))
        );
        assert_eq!(
            serde_json::to_value(state.get_data().unwrap()).unwrap(),
            before
        );
        assert!(
            !project
                .root
                .join("events/Sales_Report.yssbi-event")
                .exists()
        );
    }

    #[test]
    fn function_create_after_same_path_removal_continues_the_tombstone_revision() {
        let project = TestProject::new("function-recreate-tombstone");
        let state = project.state(ProjectData::new());
        let session = state.capture_project_session().unwrap();

        let created = state
            .create_graph_resource_transaction(
                &session.instance_id,
                "Reusable",
                GraphDocumentKind::Function,
                OperationId::new(),
            )
            .unwrap();
        let path = super::fixture_result_path(&created).unwrap();
        state
            .load_graph_resource(&session.instance_id, &path, 1)
            .unwrap();
        let created_revision = state.graph_revisions.read().unwrap()[&path];
        state
            .remove_graph_resource_transaction(
                &session.instance_id,
                &path,
                created_revision,
                OperationId::new(),
            )
            .unwrap();
        let tombstone_revision = state.graph_revisions.read().unwrap()[&path];

        let recreated = state
            .create_graph_resource_transaction(
                &session.instance_id,
                "Reusable",
                GraphDocumentKind::Function,
                OperationId::new(),
            )
            .unwrap();
        let recreated_path = super::fixture_result_path(&recreated).unwrap();
        let recreated_revision = state.graph_revisions.read().unwrap()[&recreated_path];
        let persisted: GraphDocument = serde_json::from_slice(
            &std::fs::read(project.root.join(recreated_path.as_str())).unwrap(),
        )
        .unwrap();

        assert_eq!(recreated_path, path);
        assert_eq!(recreated_revision, tombstone_revision.next());
        assert_eq!(persisted.revision, recreated_revision);
        assert_eq!(persisted.function.unwrap().revision, recreated_revision);
    }

    #[test]
    fn resource_mutation_hooks_are_scoped_to_independent_project_states() {
        let state_a = Arc::new(ProjectState::new());
        let state_b = Arc::new(ProjectState::new());
        let hits_a = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let hits_b = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let expected_a = graph_path("events/A.yssbi-event");
        let expected_b = graph_path("events/B.yssbi-event");

        let observed_a = Arc::clone(&hits_a);
        let hook_path_a = expected_a.clone();
        state_a.set_resource_mutation_test_hook(Some(Arc::new(move |point, path| {
            assert_eq!(point, ResourceMutationTestPoint::Planned);
            assert_eq!(path, Some(&hook_path_a));
            observed_a.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })));
        let observed_b = Arc::clone(&hits_b);
        let hook_path_b = expected_b.clone();
        state_b.set_resource_mutation_test_hook(Some(Arc::new(move |point, path| {
            assert_eq!(point, ResourceMutationTestPoint::Committed);
            assert_eq!(path, Some(&hook_path_b));
            observed_b.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })));

        let barrier = Arc::new(std::sync::Barrier::new(3));
        let run_a = {
            let state = Arc::clone(&state_a);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                state.run_resource_mutation_test_hook(
                    ResourceMutationTestPoint::Planned,
                    Some(&expected_a),
                );
            })
        };
        let run_b = {
            let state = Arc::clone(&state_b);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                state.run_resource_mutation_test_hook(
                    ResourceMutationTestPoint::Committed,
                    Some(&expected_b),
                );
            })
        };
        barrier.wait();
        run_a.join().unwrap();
        run_b.join().unwrap();

        assert_eq!(hits_a.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(hits_b.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    fn result_graph_path(result: &crate::event::ResourceMutationResultDto) -> GraphResourcePath {
        if let Some(resource_move) = result.moves.first() {
            return graph_path(&resource_move.to);
        }
        let paths = match &result.projection_status {
            crate::event::ProjectionStatusDto::Complete {
                expected_graph_paths,
            } => expected_graph_paths,
            crate::event::ProjectionStatusDto::Incomplete {
                invalidated_graph_paths,
            } => invalidated_graph_paths,
        };
        let path = paths
            .iter()
            .find(|path| path.starts_with("events/") || path.starts_with("functions/"))
            .expect("resource result must identify its graph path");
        graph_path(path)
    }

    fn reference_node(path: &GraphResourcePath) -> DocumentNode {
        let mut parameters = ParameterValues::new();
        parameters.insert(
            crate::node_system::protocol::ParameterKey::new("target").unwrap(),
            serde_json::json!(path.as_str()),
        );
        DocumentNode {
            id: NodeId::new(),
            node_type: NodeTypeId::new("yssbi.project.function.call").unwrap(),
            position: crate::node_system::document::NodePosition { x: 10.0, y: 20.0 },
            parameters,
            user_label: None,
        }
    }

    #[cfg(windows)]
    fn link_test_file(link: &std::path::Path, target: &std::path::Path) {
        std::os::windows::fs::symlink_file(target, link).unwrap();
    }

    #[cfg(unix)]
    fn link_test_file(link: &std::path::Path, target: &std::path::Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    #[cfg(windows)]
    fn link_test_directory(link: &std::path::Path, target: &std::path::Path) {
        let status = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link.to_string_lossy().as_ref(),
                target.to_string_lossy().as_ref(),
            ])
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[cfg(unix)]
    fn link_test_directory(link: &std::path::Path, target: &std::path::Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    fn scoped_variable(name: &str, path: &GraphResourcePath) -> VariableInstance {
        VariableInstance {
            id: VariableId::new(),
            name: name.into(),
            data_type: DataType::Int64,
            data_value: DataValue::Int64(1),
            tabular: None,
            description: String::new(),
            scope: VariableScope::Function {
                function_path: path.as_str().into(),
            },
            tags: Vec::new(),
        }
    }

    #[test]
    fn duplicate_operation_is_rejected_while_in_flight_and_after_success() {
        let project = TestProject::new("duplicate-operation-admission");
        let state = Arc::new(project.state(ProjectData::new()));
        let session = state.capture_project_session().unwrap();
        let operation_id = OperationId::new();
        let (committed_tx, committed_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let resume_rx = Mutex::new(resume_rx);
        let first_commit = Arc::new(std::sync::atomic::AtomicBool::new(true));
        state.set_resource_mutation_test_hook(Some(Arc::new(move |point, _| {
            if point == ResourceMutationTestPoint::Committed
                && first_commit.swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                committed_tx.send(()).unwrap();
                resume_rx.lock().unwrap().recv().unwrap();
            }
        })));

        let first_state = Arc::clone(&state);
        let first_session = session.clone();
        let first = std::thread::spawn(move || {
            first_state.create_graph_resource_transaction(
                &first_session.instance_id,
                "Once",
                GraphDocumentKind::Event,
                operation_id,
            )
        });
        committed_rx.recv().unwrap();

        let second_state = Arc::clone(&state);
        let second_session = session.clone();
        let (second_tx, second_rx) = std::sync::mpsc::channel();
        let second = std::thread::spawn(move || {
            let result = second_state.create_graph_resource_transaction(
                &second_session.instance_id,
                "Once",
                GraphDocumentKind::Event,
                operation_id,
            );
            second_tx.send(result).unwrap();
        });
        let concurrent = second_rx.recv_timeout(std::time::Duration::from_millis(100));
        resume_tx.send(()).unwrap();
        let first_result = first.join().unwrap().unwrap();
        let concurrent = concurrent.unwrap_or_else(|_| second_rx.recv().unwrap());
        second.join().unwrap();

        assert_eq!(concurrent.unwrap_err().code(), "duplicate_operation");
        let replay = state
            .create_graph_resource_transaction(
                &session.instance_id,
                "Once",
                GraphDocumentKind::Event,
                operation_id,
            )
            .unwrap_err();
        assert_eq!(replay.code(), "duplicate_operation");
        assert_eq!(
            first_result.project_instance_id,
            session.instance_id.as_str()
        );
        assert_eq!(
            std::fs::read_dir(project.root.join("events"))
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_file())
                .count(),
            1
        );
    }

    #[test]
    fn completed_operation_ids_are_retained_for_the_entire_project_session() {
        let project = TestProject::new("operation-ledger-lifecycle");
        let state = project.state(ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let oldest = OperationId::new();
        state
            .create_graph_resource_transaction(
                &session.instance_id,
                "Retained",
                GraphDocumentKind::Event,
                oldest,
            )
            .unwrap();
        for _ in 0..513 {
            state
                .reserve_resource_operation(&session.instance_id, OperationId::new())
                .unwrap()
                .complete();
        }

        let replay = state
            .create_graph_resource_transaction(
                &session.instance_id,
                "Retained",
                GraphDocumentKind::Event,
                oldest,
            )
            .unwrap_err();
        assert_eq!(replay.code(), "duplicate_operation");
        assert_eq!(
            std::fs::read_dir(project.root.join("events"))
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_file())
                .count(),
            1
        );
        let ledger = state.resource_operations.lock().unwrap();
        assert_eq!(ledger.completed.len(), 514);
        assert!(ledger.in_flight.is_empty());
    }

    #[test]
    fn old_session_reservations_cannot_clear_same_uuid_new_session_reservations() {
        let project = TestProject::new("operation-ledger-owner-old");
        let state = project.state(ProjectData::new());
        let old_session = state.capture_project_session().unwrap();
        let completed_id = OperationId::new();
        let dropped_id = OperationId::new();
        let old_complete = state
            .reserve_resource_operation(&old_session.instance_id, completed_id)
            .unwrap();
        let old_drop = state
            .reserve_resource_operation(&old_session.instance_id, dropped_id)
            .unwrap();

        let replacement = TestProject::new("operation-ledger-owner-new");
        state.activate_project_fixture(
            replacement.root.to_string_lossy().into_owned(),
            ProjectData::new(),
        );
        let new_session = state.capture_project_session().unwrap();
        let new_complete = state
            .reserve_resource_operation(&new_session.instance_id, completed_id)
            .unwrap();
        let new_drop = state
            .reserve_resource_operation(&new_session.instance_id, dropped_id)
            .unwrap();

        old_complete.complete();
        drop(old_drop);

        assert_eq!(
            state
                .reserve_resource_operation(&new_session.instance_id, completed_id)
                .err()
                .unwrap()
                .code(),
            "duplicate_operation"
        );
        assert_eq!(
            state
                .reserve_resource_operation(&new_session.instance_id, dropped_id)
                .err()
                .unwrap()
                .code(),
            "duplicate_operation"
        );
        drop(new_complete);
        drop(new_drop);
    }

    #[test]
    fn activation_swaps_the_operation_ledger_inside_the_publication_boundary() {
        let project = TestProject::new("operation-ledger-atomic-old");
        let state = project.state(ProjectData::new());
        let hook_state = state.clone();
        state.set_activation_store_replaced_test_hook(std::sync::Arc::new(move || {
            assert!(hook_state.resource_operations.try_lock().is_err());
        }));

        let replacement = TestProject::new("operation-ledger-atomic-new");
        state.activate_project_fixture(
            replacement.root.to_string_lossy().into_owned(),
            ProjectData::new(),
        );

        let new_session = state.capture_project_session().unwrap();
        let operation_id = OperationId::new();
        let reservation = state
            .reserve_resource_operation(&new_session.instance_id, operation_id)
            .unwrap();
        assert_eq!(
            state
                .reserve_resource_operation(&new_session.instance_id, operation_id)
                .err()
                .unwrap()
                .code(),
            "duplicate_operation"
        );
        drop(reservation);
    }

    #[test]
    fn failed_operation_releases_its_reservation_for_retry() {
        let project = TestProject::new("failed-operation-release");
        let state = project.state(ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let operation_id = OperationId::new();
        state.set_project_filesystem_fault(Some(
            crate::project::ProjectFilesystemFaultPoint::FirstLiveReplacement,
        ));

        let first = state.create_graph_resource_transaction(
            &session.instance_id,
            "Retry",
            GraphDocumentKind::Event,
            operation_id,
        );
        state.set_project_filesystem_fault(None);
        assert_eq!(first.unwrap_err().code(), "transaction_commit_failed");

        let retry = state
            .create_graph_resource_transaction(
                &session.instance_id,
                "Retry",
                GraphDocumentKind::Event,
                operation_id,
            )
            .unwrap();
        assert_eq!(retry.project_instance_id, session.instance_id.as_str());
    }

    #[test]
    fn create_rechecks_destination_under_lease_and_routes_insert_through_project_state() {
        let project = TestProject::new("create-destination-race");
        let state = Arc::new(project.state(ProjectData::new()));
        let session = state.capture_project_session().unwrap();
        let root = project.root.clone();
        state.set_resource_mutation_test_hook(Some(Arc::new(move |point, candidate| {
            if point == ResourceMutationTestPoint::Planned {
                let candidate = candidate.expect("create planning exposes candidate");
                let target = root.join(candidate.as_str());
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                let competing = GraphResourceDocument::new("Race", GraphDocumentKind::Event);
                let contents = crate::project::project_io::serialize_graph_resource_document(
                    &competing,
                    HashMap::new(),
                )
                .unwrap();
                std::fs::write(target, contents).unwrap();
            }
        })));

        let result = state
            .create_graph_resource_transaction(
                &session.instance_id,
                "Race",
                GraphDocumentKind::Event,
                OperationId::new(),
            )
            .unwrap();
        let created = result_graph_path(&result);

        assert_ne!(created, graph_path("events/Race.yssbi-event"));
        let competing: GraphDocument = serde_json::from_slice(
            &std::fs::read(project.root.join("events/Race.yssbi-event")).unwrap(),
        )
        .unwrap();
        assert_eq!(competing.name, "Race");
        assert!(project.root.join(created.as_str()).is_file());
        assert!(!state.get_data().unwrap().graphs.contains_key(&created));
        assert!(state.graph_revisions.read().unwrap().contains_key(&created));
    }

    #[test]
    fn duplicate_rejects_redirected_source_under_root_lease() {
        let project = TestProject::new("duplicate-redirected-source");
        let source = graph_path("events/Source.yssbi-event");
        let mut data = ProjectData::new();
        data.graphs.insert(
            source.clone(),
            GraphResourceDocument::new("Source", GraphDocumentKind::Event),
        );
        let state = project.state(data);
        let source_file = project.root.join(source.as_str());
        let outside = std::env::temp_dir().join(format!(
            "yssbi-duplicate-external-{}.yssbi-event",
            uuid::Uuid::new_v4()
        ));
        std::fs::copy(&source_file, &outside).unwrap();
        std::fs::remove_file(&source_file).unwrap();
        link_test_file(&source_file, &outside);
        let session = state.capture_project_session().unwrap();

        let result = state.duplicate_graph_resource_transaction(
            &session.instance_id,
            &source,
            ResourceRevision::INITIAL,
            OperationId::new(),
        );

        let _ = std::fs::remove_file(&outside);
        assert_eq!(result.unwrap_err().code(), "transaction_prepare_failed");
        assert_eq!(
            std::fs::read_dir(project.root.join("events"))
                .unwrap()
                .filter_map(Result::ok)
                .count(),
            1
        );
    }

    fn assert_remove_rejects_redirected_file(label: &str) {
        let project = TestProject::new(label);
        let source = graph_path("events/Source.yssbi-event");
        let mut data = ProjectData::new();
        data.graphs.insert(
            source.clone(),
            GraphResourceDocument::new("Source", GraphDocumentKind::Event),
        );
        let state = project.state(data);
        let source_file = project.root.join(source.as_str());
        let outside = std::env::temp_dir().join(format!(
            "yssbi-remove-external-{}.yssbi-event",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&outside, b"external contents must not be parsed").unwrap();
        std::fs::remove_file(&source_file).unwrap();
        link_test_file(&source_file, &outside);
        let session = state.capture_project_session().unwrap();

        let error = state
            .remove_graph_resource_transaction(
                &session.instance_id,
                &source,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
            .unwrap_err();

        assert_eq!(error.code(), "transaction_prepare_failed");
        assert!(
            error.to_string().contains("redirect"),
            "redirect must be rejected before parsing external bytes: {error}"
        );
        assert_eq!(
            std::fs::read(&outside).unwrap(),
            b"external contents must not be parsed"
        );
        let _ = std::fs::remove_file(&source_file);
        let _ = std::fs::remove_file(outside);
    }

    fn assert_remove_rejects_redirected_directory(label: &str) {
        let project = TestProject::new(label);
        let source = graph_path("events/Source.yssbi-event");
        let mut data = ProjectData::new();
        data.graphs.insert(
            source.clone(),
            GraphResourceDocument::new("Source", GraphDocumentKind::Event),
        );
        let state = project.state(data);
        let events = project.root.join("events");
        std::fs::remove_dir_all(&events).unwrap();
        let outside = std::env::temp_dir().join(format!(
            "yssbi-remove-external-directory-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(
            outside.join("Source.yssbi-event"),
            b"external contents must not be parsed",
        )
        .unwrap();
        link_test_directory(&events, &outside);
        let session = state.capture_project_session().unwrap();

        let error = state
            .remove_graph_resource_transaction(
                &session.instance_id,
                &source,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
            .unwrap_err();

        assert_eq!(error.code(), "transaction_prepare_failed");
        assert!(
            error.to_string().contains("redirect"),
            "redirect ancestor must be rejected before parsing external bytes: {error}"
        );
        assert_eq!(
            std::fs::read(outside.join("Source.yssbi-event")).unwrap(),
            b"external contents must not be parsed"
        );
        let _ = std::fs::remove_dir(&events);
        let _ = std::fs::remove_dir_all(outside);
    }

    #[cfg(windows)]
    #[test]
    fn remove_rejects_real_windows_file_reparse_point_before_read() {
        assert_remove_rejects_redirected_file("remove-windows-file-reparse");
    }

    #[cfg(windows)]
    #[test]
    fn remove_rejects_real_windows_directory_junction_before_read() {
        assert_remove_rejects_redirected_directory("remove-windows-directory-junction");
    }

    #[cfg(unix)]
    #[test]
    fn remove_rejects_unix_file_symlink_before_read() {
        assert_remove_rejects_redirected_file("remove-unix-file-symlink");
    }

    #[cfg(unix)]
    #[test]
    fn remove_rejects_unix_directory_symlink_before_read() {
        assert_remove_rejects_redirected_directory("remove-unix-directory-symlink");
    }

    #[test]
    fn duplicate_rechecks_destination_and_allocates_persistent_identities_in_rust() {
        let project = TestProject::new("duplicate-identities");
        let source = graph_path("functions/Source.yssbi-function");
        let mut resource = GraphResourceDocument::new("Source", GraphDocumentKind::Function);
        let call = reference_node(&source);
        resource.document.nodes.insert(call.id, call);
        let variable = scoped_variable("Local", &source);
        let source_variable_id = variable.id;
        let mut data = ProjectData::new();
        data.graphs.insert(source.clone(), resource);
        data.variables.insert(variable.id, variable);
        let state = project.state(data);
        let session = state.capture_project_session().unwrap();
        let root = project.root.clone();
        state.set_resource_mutation_test_hook(Some(Arc::new(move |point, candidate| {
            if point == ResourceMutationTestPoint::Planned {
                let candidate = candidate.expect("duplicate planning exposes candidate");
                let target = root.join(candidate.as_str());
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                let competing = GraphResourceDocument::new("Source 1", GraphDocumentKind::Function);
                let contents = crate::project::project_io::serialize_graph_resource_document(
                    &competing,
                    HashMap::new(),
                )
                .unwrap();
                std::fs::write(target, contents).unwrap();
            }
        })));

        let result = state
            .duplicate_graph_resource_transaction(
                &session.instance_id,
                &source,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
            .unwrap();
        let duplicated = result_graph_path(&result);
        let source_disk: GraphDocument =
            serde_json::from_slice(&std::fs::read(project.root.join(source.as_str())).unwrap())
                .unwrap();
        let duplicate_disk: GraphDocument =
            serde_json::from_slice(&std::fs::read(project.root.join(duplicated.as_str())).unwrap())
                .unwrap();

        assert_ne!(duplicated, graph_path("functions/Source 1.yssbi-function"));
        assert!(
            source_disk
                .document
                .nodes
                .keys()
                .all(|id| !duplicate_disk.document.nodes.contains_key(id))
        );
        assert!(
            source_disk
                .document
                .connections
                .keys()
                .all(|id| !duplicate_disk.document.connections.contains_key(id))
        );
        assert!(
            !duplicate_disk
                .local_variables
                .contains_key(&source_variable_id)
        );
        assert!(duplicate_disk.local_variables.values().all(|variable| {
            matches!(&variable.scope, VariableScope::Function { function_path } if function_path == duplicated.as_str())
        }));
        assert!(duplicate_disk.document.nodes.values().any(|node| {
            node.parameters
                .values()
                .any(|value| value.as_str() == Some(duplicated.as_str()))
        }));
        assert!(!state.get_data().unwrap().graphs.contains_key(&duplicated));
    }

    #[test]
    fn duplicate_loaded_function_requires_owner_embedded_and_ledger_exact() {
        let source = graph_path("functions/LoadedAuthority.yssbi-function");

        let owner_mismatch = TestProject::new("duplicate-loaded-owner-ledger-mismatch");
        let owner_state = owner_mismatch.state(function_data(
            &source,
            ResourceRevision::INITIAL,
            ResourceRevision::INITIAL,
        ));
        owner_state
            .graph_revisions
            .write()
            .unwrap()
            .insert(source.clone(), ResourceRevision::new(1));
        assert_duplicate_revision_conflict_without_effects(
            &owner_state,
            &owner_mismatch.root,
            &source,
            ResourceRevision::new(1),
        );

        let embedded_mismatch = TestProject::new("duplicate-loaded-embedded-mismatch");
        let embedded_state = embedded_mismatch.state(function_data(
            &source,
            ResourceRevision::INITIAL,
            ResourceRevision::INITIAL,
        ));
        embedded_state
            .project_data
            .write()
            .unwrap()
            .graphs
            .get_mut(&source)
            .unwrap()
            .function
            .as_mut()
            .unwrap()
            .revision = ResourceRevision::new(1);
        assert_duplicate_revision_conflict_without_effects(
            &embedded_state,
            &embedded_mismatch.root,
            &source,
            ResourceRevision::INITIAL,
        );
    }

    #[test]
    fn duplicate_unloaded_function_rejects_ahead_or_incoherent_persisted_revisions() {
        let cases = [
            (
                "owner-ahead",
                ResourceRevision::new(1),
                ResourceRevision::new(2),
                ResourceRevision::new(2),
                ResourceRevision::new(2),
            ),
            (
                "embedded-ahead",
                ResourceRevision::new(1),
                ResourceRevision::INITIAL,
                ResourceRevision::INITIAL,
                ResourceRevision::new(2),
            ),
            (
                "embedded-incoherent",
                ResourceRevision::new(2),
                ResourceRevision::INITIAL,
                ResourceRevision::INITIAL,
                ResourceRevision::new(1),
            ),
        ];

        for (label, authority, owner, graph, embedded) in cases {
            let project = TestProject::new(&format!("duplicate-unloaded-{label}"));
            let source = graph_path("functions/UnloadedAuthority.yssbi-function");
            let state = project.state(function_data(
                &source,
                ResourceRevision::INITIAL,
                ResourceRevision::INITIAL,
            ));
            state.project_data.write().unwrap().graphs.remove(&source);
            state
                .graph_revisions
                .write()
                .unwrap()
                .insert(source.clone(), authority);
            rewrite_persisted_function_revisions(&project.root, &source, owner, graph, embedded);

            assert_duplicate_revision_conflict_without_effects(
                &state,
                &project.root,
                &source,
                authority,
            );
        }
    }

    #[test]
    fn duplicate_unloaded_function_uses_exact_retained_token_and_initial_target() {
        let project = TestProject::new("duplicate-unloaded-retained-happy");
        let source = graph_path("functions/Retained.yssbi-function");
        let variable = scoped_variable("Retained local", &source);
        let source_variable_id = variable.id;
        let mut data = function_data(
            &source,
            ResourceRevision::INITIAL,
            ResourceRevision::INITIAL,
        );
        data.variables.insert(variable.id, variable);
        let state = project.state(data);
        state.project_data.write().unwrap().graphs.remove(&source);
        let retained = ResourceRevision::new(5);
        state
            .graph_revisions
            .write()
            .unwrap()
            .insert(source.clone(), retained);
        rewrite_persisted_function_revisions(
            &project.root,
            &source,
            ResourceRevision::new(1),
            ResourceRevision::new(1),
            ResourceRevision::new(1),
        );

        assert_duplicate_revision_conflict_without_effects(
            &state,
            &project.root,
            &source,
            ResourceRevision::new(4),
        );

        let session = state.capture_project_session().unwrap();
        let result = state
            .duplicate_graph_resource_transaction(
                &session.instance_id,
                &source,
                retained,
                OperationId::new(),
            )
            .unwrap();
        let target = result_graph_path(&result);
        let target_document: GraphDocument =
            serde_json::from_slice(&std::fs::read(project.root.join(target.as_str())).unwrap())
                .unwrap();

        assert_eq!(target_document.revision, ResourceRevision::INITIAL);
        assert_eq!(target_document.document.revision, ResourceRevision::INITIAL);
        assert_eq!(
            target_document.function.as_ref().unwrap().revision,
            ResourceRevision::INITIAL
        );
        assert!(
            !target_document
                .local_variables
                .contains_key(&source_variable_id)
        );
        assert!(target_document.local_variables.values().all(|variable| {
            matches!(&variable.scope, VariableScope::Function { function_path } if function_path == target.as_str())
        }));
        assert_eq!(state.graph_revisions.read().unwrap()[&source], retained);
        assert_eq!(
            state.graph_revisions.read().unwrap()[&target],
            ResourceRevision::INITIAL
        );
        assert!(!state.get_data().unwrap().graphs.contains_key(&target));
    }

    #[test]
    fn remove_rolls_back_file_when_authoritative_revision_changed() {
        let project = TestProject::new("remove-stale-publication");
        let path = graph_path("events/Remove.yssbi-event");
        let mut data = ProjectData::new();
        data.graphs.insert(
            path.clone(),
            GraphResourceDocument::new("Remove", GraphDocumentKind::Event),
        );
        let state = project.state(data);
        let before = std::fs::read(project.root.join(path.as_str())).unwrap();
        let concurrent = state.clone();
        let concurrent_path = path.clone();
        state.set_resource_mutation_test_hook(Some(Arc::new(move |point, _| {
            if point == ResourceMutationTestPoint::BeforePublication {
                let mut data = concurrent.project_data.write().unwrap();
                data.graphs
                    .get_mut(&concurrent_path)
                    .unwrap()
                    .document
                    .revision = ResourceRevision::new(1);
            }
        })));
        let session = state.capture_project_session().unwrap();

        let error = state
            .remove_graph_resource_transaction(
                &session.instance_id,
                &path,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
            .unwrap_err();

        assert_eq!(error.code(), "resource_revision_conflict");
        assert_eq!(
            std::fs::read(project.root.join(path.as_str())).unwrap(),
            before
        );
        assert!(state.get_data().unwrap().graphs.contains_key(&path));
    }

    #[test]
    fn unloaded_source_rename_preserves_persisted_local_variables_on_reload() {
        let project = TestProject::new("rename-unloaded-source-locals");
        let source = graph_path("functions/Source.yssbi-function");
        let mut data = ProjectData::new();
        data.graphs.insert(
            source.clone(),
            GraphResourceDocument::new("Source", GraphDocumentKind::Function),
        );
        let mut first = scoped_variable("First", &source);
        first.data_value = DataValue::Int64(41);
        first.description = "first persisted local".into();
        first.tags = vec!["alpha".into()];
        let first_id = first.id;
        let mut second = scoped_variable("Second", &source);
        second.data_value = DataValue::Int64(42);
        second.description = "second persisted local".into();
        second.tags = vec!["beta".into()];
        let second_id = second.id;
        data.variables.insert(first_id, first.clone());
        data.variables.insert(second_id, second.clone());
        let state = project.state(data);
        state.unload_graph_resource(&source).unwrap();
        assert!(state.get_data().unwrap().variables.is_empty());
        let session = state.capture_project_session().unwrap();

        let result = state
            .rename_graph_resource_transaction(
                &session.instance_id,
                &source,
                ResourceRevision::INITIAL,
                "Renamed",
                1,
                OperationId::new(),
            )
            .unwrap();
        let target = result_graph_path(&result);
        assert!(!project.root.join(source.as_str()).exists());

        let reloaded = ProjectState::new();
        let reloaded_session = reloaded.activate_project_from_path(&project.root).unwrap();
        reloaded
            .load_graph_projection(&reloaded_session.instance_id, &target, 1, "en-US")
            .unwrap();
        let variables = reloaded.get_data().unwrap().variables;
        assert_eq!(variables.len(), 2);
        assert_eq!(variables[&first_id].data_value, first.data_value);
        assert_eq!(variables[&first_id].description, first.description);
        assert_eq!(variables[&first_id].tags, first.tags);
        assert_eq!(variables[&second_id].data_value, second.data_value);
        assert_eq!(variables[&second_id].description, second.description);
        assert_eq!(variables[&second_id].tags, second.tags);
        assert!(variables.values().all(|variable| {
            matches!(&variable.scope, VariableScope::Function { function_path } if function_path == target.as_str())
        }));
    }

    #[test]
    fn loaded_caller_rename_cascade_survives_fresh_reload() {
        let project = TestProject::new("rename-loaded-caller-persistence");
        let source = graph_path("functions/Source.yssbi-function");
        let caller = graph_path("events/Caller.yssbi-event");
        let mut caller_resource = GraphResourceDocument::new("Caller", GraphDocumentKind::Event);
        let call = reference_node(&source);
        caller_resource.document.nodes.insert(call.id, call);
        let mut data = ProjectData::new();
        data.graphs.insert(
            source.clone(),
            GraphResourceDocument::new("Source", GraphDocumentKind::Function),
        );
        data.graphs.insert(caller.clone(), caller_resource);
        let state = project.state(data);
        let session = state.capture_project_session().unwrap();

        let result = state
            .rename_graph_resource_transaction(
                &session.instance_id,
                &source,
                ResourceRevision::INITIAL,
                "Renamed",
                1,
                OperationId::new(),
            )
            .unwrap();
        let target = graph_path(&result.moves.first().unwrap().to);
        let authority = state.get_data().unwrap();
        let target_revision = authority.graphs[&target].document.revision.get();
        let caller_revision = authority.graphs[&caller].document.revision.get();
        drop(authority);
        let target_replacement = result
            .projection_replacements
            .iter()
            .find(|replacement| replacement.graph_path == target.as_str())
            .expect("rename result must replace the loaded destination");
        assert_eq!(
            target_replacement.projection.source_revision,
            target_revision
        );
        let caller_replacement = result
            .projection_replacements
            .iter()
            .find(|replacement| replacement.graph_path == caller.as_str())
            .expect("rename result must replace every loaded affected caller");
        assert_eq!(
            caller_replacement.projection.source_revision,
            caller_revision
        );
        assert!(caller_replacement.projection.nodes.iter().any(|node| {
            node.parameter_editors.iter().any(|editor| {
                editor.value.as_ref().and_then(serde_json::Value::as_str) == Some(target.as_str())
            })
        }));
        assert!(caller_replacement.projection.nodes.iter().all(|node| {
            node.parameter_editors.iter().all(|editor| {
                editor.value.as_ref().and_then(serde_json::Value::as_str) != Some(source.as_str())
            })
        }));

        let persisted: GraphDocument =
            serde_json::from_slice(&std::fs::read(project.root.join(caller.as_str())).unwrap())
                .unwrap();
        assert!(persisted.document.nodes.values().any(|node| {
            node.parameters
                .values()
                .any(|value| value.as_str() == Some(target.as_str()))
        }));
        assert!(persisted.document.nodes.values().all(|node| {
            node.parameters
                .values()
                .all(|value| value.as_str() != Some(source.as_str()))
        }));

        let reloaded = ProjectState::new();
        let reloaded_session = reloaded.activate_project_from_path(&project.root).unwrap();
        reloaded
            .load_graph_projection(&reloaded_session.instance_id, &caller, 1, "en-US")
            .unwrap();
        let authority = reloaded.get_data().unwrap();
        assert!(
            authority.graphs[&caller]
                .document
                .nodes
                .values()
                .any(|node| {
                    node.parameters
                        .values()
                        .any(|value| value.as_str() == Some(target.as_str()))
                })
        );
        assert!(
            authority.graphs[&caller]
                .document
                .nodes
                .values()
                .all(|node| {
                    node.parameters
                        .values()
                        .all(|value| value.as_str() != Some(source.as_str()))
                })
        );
    }

    #[test]
    fn rename_stages_complete_reference_cascade_before_live_mutation() {
        let project = TestProject::new("rename-prepared-cascade");
        let source = graph_path("functions/Source.yssbi-function");
        let caller = graph_path("events/Caller.yssbi-event");
        let mut caller_resource = GraphResourceDocument::new("Caller", GraphDocumentKind::Event);
        let call = reference_node(&source);
        caller_resource.document.nodes.insert(call.id, call);
        let global = scoped_variable("Scoped", &source);
        let mut data = ProjectData::new();
        data.graphs.insert(
            source.clone(),
            GraphResourceDocument::new("Source", GraphDocumentKind::Function),
        );
        data.graphs.insert(caller.clone(), caller_resource);
        data.variables.insert(global.id, global);
        let state = project.state(data);
        state.unload_graph_resource(&caller).unwrap();
        let source_before = std::fs::read(project.root.join(source.as_str())).unwrap();
        let caller_before = std::fs::read(project.root.join(caller.as_str())).unwrap();
        let globals_before =
            std::fs::read(project.root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap();
        let hook_state = state.clone();
        state.set_resource_mutation_test_hook(Some(Arc::new(move |point, _| {
            if point == ResourceMutationTestPoint::Prepared {
                hook_state.set_project_filesystem_fault(Some(
                    crate::project::ProjectFilesystemFaultPoint::SecondLiveReplacement,
                ));
            }
        })));
        let session = state.capture_project_session().unwrap();

        let error = state
            .rename_graph_resource_transaction(
                &session.instance_id,
                &source,
                ResourceRevision::INITIAL,
                "Renamed",
                1,
                OperationId::new(),
            )
            .unwrap_err();
        state.set_project_filesystem_fault(None);

        assert_eq!(
            error.code(),
            "transaction_commit_failed",
            "unexpected rename failure: {error}"
        );
        assert_eq!(
            std::fs::read(project.root.join(source.as_str())).unwrap(),
            source_before
        );
        assert_eq!(
            std::fs::read(project.root.join(caller.as_str())).unwrap(),
            caller_before
        );
        assert_eq!(
            std::fs::read(project.root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
            globals_before
        );
        assert!(
            !project
                .root
                .join("functions/Renamed.yssbi-function")
                .exists()
        );
    }

    #[test]
    fn rename_rollback_restores_only_target_graph_global_and_worksheet_paths() {
        let project = TestProject::new("rename-precise-rollback");
        let source = graph_path("events/Source.yssbi-event");
        let unrelated = project.root.join("events/unrelated.bin");
        let worksheet = project.root.join("worksheets/unrelated.yssbi-worksheet");
        let mut data = ProjectData::new();
        data.graphs.insert(
            source.clone(),
            GraphResourceDocument::new("Source", GraphDocumentKind::Event),
        );
        let state = project.state(data);
        std::fs::write(&unrelated, b"unrelated graph sentinel").unwrap();
        std::fs::create_dir_all(worksheet.parent().unwrap()).unwrap();
        std::fs::write(&worksheet, b"worksheet sentinel").unwrap();
        let before = std::fs::read(project.root.join(source.as_str())).unwrap();
        let concurrent = state.clone();
        let source_for_hook = source.clone();
        state.set_resource_mutation_test_hook(Some(Arc::new(move |point, _| {
            if point == ResourceMutationTestPoint::BeforePublication {
                concurrent
                    .project_data
                    .write()
                    .unwrap()
                    .graphs
                    .get_mut(&source_for_hook)
                    .unwrap()
                    .document
                    .revision = ResourceRevision::new(1);
            }
        })));
        let session = state.capture_project_session().unwrap();

        let error = state
            .rename_graph_resource_transaction(
                &session.instance_id,
                &source,
                ResourceRevision::INITIAL,
                "Renamed",
                1,
                OperationId::new(),
            )
            .unwrap_err();

        assert_eq!(error.code(), "resource_revision_conflict");
        assert_eq!(
            std::fs::read(project.root.join(source.as_str())).unwrap(),
            before
        );
        assert!(!project.root.join("events/Renamed.yssbi-event").exists());
        assert_eq!(
            std::fs::read(unrelated).unwrap(),
            b"unrelated graph sentinel"
        );
        assert_eq!(std::fs::read(worksheet).unwrap(), b"worksheet sentinel");
    }

    #[test]
    fn rename_narrow_patch_preserves_unrelated_graph_variable_and_history_mutations() {
        let project = TestProject::new("rename-narrow-publication");
        let source = graph_path("events/Source.yssbi-event");
        let unrelated_path = graph_path("events/Unrelated.yssbi-event");
        let mut data = ProjectData::new();
        data.graphs.insert(
            source.clone(),
            GraphResourceDocument::new("Source", GraphDocumentKind::Event),
        );
        data.graphs.insert(
            unrelated_path.clone(),
            GraphResourceDocument::new("Unrelated", GraphDocumentKind::Event),
        );
        let state = project.state(data);
        let mut variable = scoped_variable("Concurrent", &source);
        variable.scope = VariableScope::Global;
        let variable_id = variable.id;
        let concurrent = state.clone();
        let unrelated_for_hook = unrelated_path.clone();
        state.set_resource_mutation_test_hook(Some(Arc::new(move |point, _| {
            if point == ResourceMutationTestPoint::BeforePublication {
                concurrent
                    .project_data
                    .write()
                    .unwrap()
                    .graphs
                    .get_mut(&unrelated_for_hook)
                    .unwrap()
                    .name = "Concurrent graph".into();
                concurrent
                    .project_data
                    .write()
                    .unwrap()
                    .variables
                    .insert(variable_id, variable.clone());
                concurrent
                    .graph_revisions
                    .write()
                    .unwrap()
                    .insert(unrelated_for_hook.clone(), ResourceRevision::new(9));
                concurrent.variable_revisions.write().unwrap().insert(
                    variable_id,
                    crate::project::project_state::VariableRevisionEntry::present(
                        ResourceRevision::new(7),
                    ),
                );
                concurrent.append_history_head_for_test();
            }
        })));
        let session = state.capture_project_session().unwrap();

        let result = state
            .rename_graph_resource_transaction(
                &session.instance_id,
                &source,
                ResourceRevision::INITIAL,
                "Renamed",
                1,
                OperationId::new(),
            )
            .unwrap();

        let authority = state.get_data().unwrap();
        assert_eq!(authority.graphs[&unrelated_path].name, "Concurrent graph");
        assert_eq!(authority.variables[&variable_id].name, "Concurrent");
        assert_eq!(
            state.graph_revisions.read().unwrap()[&unrelated_path],
            ResourceRevision::new(9)
        );
        assert_eq!(
            state.variable_revisions.read().unwrap()[&variable_id].revision,
            ResourceRevision::new(7)
        );
        assert_eq!(state.history.read().unwrap().undo_len(), 2);
        assert!(result.history.can_undo);
    }

    #[test]
    fn save_flush_and_index_cannot_enter_during_rename_commit_or_rollback() {
        for rollback in [false, true] {
            let project = TestProject::new(if rollback {
                "rename-exclusive-rollback"
            } else {
                "rename-exclusive-commit"
            });
            let source = graph_path("events/Source.yssbi-event");
            let mut data = ProjectData::new();
            data.graphs.insert(
                source.clone(),
                GraphResourceDocument::new("Source", GraphDocumentKind::Event),
            );
            let state = Arc::new(project.state(data));
            let session = state.capture_project_session().unwrap();
            let (barrier_tx, barrier_rx) = std::sync::mpsc::channel();
            let (resume_tx, resume_rx) = std::sync::mpsc::channel();
            let resume_rx = Arc::new(Mutex::new(resume_rx));
            if rollback {
                let conflict_state = Arc::clone(&state);
                let conflict_source = source.clone();
                state.set_resource_mutation_test_hook(Some(Arc::new(move |point, _| {
                    if point == ResourceMutationTestPoint::BeforePublication {
                        conflict_state
                            .project_data
                            .write()
                            .unwrap()
                            .graphs
                            .get_mut(&conflict_source)
                            .unwrap()
                            .document
                            .revision = ResourceRevision::new(1);
                    }
                })));
                let resume = Arc::clone(&resume_rx);
                state.set_project_filesystem_rollback_test_hook(Some(Arc::new(move || {
                    barrier_tx.send(()).unwrap();
                    resume.lock().unwrap().recv().unwrap();
                })));
            } else {
                let resume = Arc::clone(&resume_rx);
                state.set_resource_mutation_test_hook(Some(Arc::new(move |point, _| {
                    if point == ResourceMutationTestPoint::Committed {
                        barrier_tx.send(()).unwrap();
                        resume.lock().unwrap().recv().unwrap();
                    }
                })));
            }
            let rename_state = Arc::clone(&state);
            let rename_source = source.clone();
            let rename_session = session.clone();
            let rename = std::thread::spawn(move || {
                rename_state.rename_graph_resource_transaction(
                    &rename_session.instance_id,
                    &rename_source,
                    ResourceRevision::INITIAL,
                    "Renamed",
                    1,
                    OperationId::new(),
                )
            });
            barrier_rx.recv().unwrap();

            let (started_tx, started_rx) = std::sync::mpsc::channel();
            let (finished_tx, finished_rx) = std::sync::mpsc::channel();
            let save_state = Arc::clone(&state);
            let save_session = session.clone();
            let save_source = source.clone();
            let save_expected_revision = if rollback {
                ResourceRevision::new(1)
            } else {
                ResourceRevision::INITIAL
            };
            let save_started = started_tx.clone();
            let save_finished = finished_tx.clone();
            let save = std::thread::spawn(move || {
                save_started.send("save").unwrap();
                let result = save_state.save_graph_document(
                    &save_session.instance_id,
                    &save_source,
                    save_expected_revision,
                    OperationId::new(),
                );
                save_finished.send("save").unwrap();
                result
            });
            let flush_state = Arc::clone(&state);
            let flush_session = session.clone();
            let flush_started = started_tx.clone();
            let flush_finished = finished_tx.clone();
            let flush = std::thread::spawn(move || {
                flush_started.send("flush").unwrap();
                let result = flush_state
                    .flush_project_documents(&flush_session.instance_id, OperationId::new());
                flush_finished.send("flush").unwrap();
                result
            });
            let index_state = Arc::clone(&state);
            let index_session = session.clone();
            let index = std::thread::spawn(move || {
                started_tx.send("index").unwrap();
                let result = index_state.read_project_index(&index_session.instance_id);
                finished_tx.send("index").unwrap();
                result
            });
            let mut started = BTreeSet::new();
            for _ in 0..3 {
                started.insert(started_rx.recv().unwrap());
            }
            assert_eq!(started, BTreeSet::from(["save", "flush", "index"]));
            let early = finished_rx.recv_timeout(std::time::Duration::from_millis(100));
            resume_tx.send(()).unwrap();
            assert!(
                early.is_err(),
                "{} completed while rename owned the root lease",
                early.unwrap()
            );

            let rename_result = rename.join().unwrap();
            assert_eq!(rename_result.is_err(), rollback);
            for _ in 0..3 {
                finished_rx.recv().unwrap();
            }
            let _ = save.join().unwrap();
            let _ = flush.join().unwrap();
            let _ = index.join().unwrap();
            state
                .filesystem()
                .set_project_filesystem_rollback_test_hook(None);
            state.set_resource_mutation_test_hook(None);
        }
    }

    #[test]
    fn old_project_create_duplicate_remove_and_rename_have_zero_effects() {
        let old = TestProject::new("old-project");
        let source = graph_path("events/Source.yssbi-event");
        let mut data = ProjectData::new();
        data.graphs.insert(
            source.clone(),
            GraphResourceDocument::new("Source", GraphDocumentKind::Event),
        );
        let state = old.state(data);
        let old_session = state.capture_project_session().unwrap();
        let old_files = std::fs::read(old.root.join(source.as_str())).unwrap();
        let replacement = TestProject::new("replacement-project");
        state.activate_project_fixture(
            replacement.root.to_string_lossy().into_owned(),
            ProjectData::new(),
        );

        let create = state.create_graph_resource_transaction(
            &old_session.instance_id,
            "Stale",
            GraphDocumentKind::Event,
            OperationId::new(),
        );
        let duplicate = state.duplicate_graph_resource_transaction(
            &old_session.instance_id,
            &source,
            ResourceRevision::INITIAL,
            OperationId::new(),
        );
        let remove = state.remove_graph_resource_transaction(
            &old_session.instance_id,
            &source,
            ResourceRevision::INITIAL,
            OperationId::new(),
        );
        let rename = state.rename_graph_resource_transaction(
            &old_session.instance_id,
            &source,
            ResourceRevision::INITIAL,
            "Stale rename",
            1,
            OperationId::new(),
        );

        for result in [create, duplicate, remove, rename] {
            assert_eq!(result.unwrap_err().code(), "stale_project_lifecycle");
        }
        assert_eq!(
            std::fs::read(old.root.join(source.as_str())).unwrap(),
            old_files
        );
        assert_eq!(state.get_data().unwrap().graphs.len(), 0);
        assert_eq!(state.authority_generation_for_test(), 0);
        assert!(!state.history_status().can_undo);
        assert_eq!(
            state
                .graph_revisions
                .read()
                .unwrap()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            BTreeSet::new()
        );
    }
}
