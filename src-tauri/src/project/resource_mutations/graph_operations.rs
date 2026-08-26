#[cfg(test)]
use super::ResourceMutationTestPoint;
use crate::graph_document::GraphResourcePath;
use crate::node_system::document::ResourceKey;
use crate::project::{
    GraphDocumentKind, GraphResourceDocument, ProjectFilesystemError, ProjectFilesystemTransaction,
    ProjectInstanceId, ProjectState, ProjectTransactionContext, ResourceDocumentPatch,
    StagedFilesystemMutation,
};
use crate::project::{OperationId, ResourceRevision};
use std::collections::{BTreeMap, HashMap};

fn graph_key(path: &GraphResourcePath) -> ResourceKey {
    ResourceKey::Graph(path.clone())
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
        let id = crate::graph_document::NodeId::new();
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
            crate::graph_document::DocumentNode {
                id,
                node_type: crate::node_system::protocol::NodeTypeId::new(*node_type).map_err(
                    |error| ProjectFilesystemError::TransactionPrepareFailed {
                        message: error.to_string(),
                    },
                )?,
                position: crate::graph_document::NodePosition { x: *x, y: 160.0 },
                parameters,
                user_label: None,
            },
        );
        shell_nodes.push(id);
    }
    if let [entry, returned] = shell_nodes.as_slice() {
        let id = crate::graph_document::ConnectionId::new();
        resource.document.connections.insert(
            id,
            crate::graph_document::DocumentConnection {
                id,
                output: crate::graph_document::PortAddress::declared(
                    *entry,
                    crate::node_system::protocol::PortKey::new("then").map_err(|error| {
                        ProjectFilesystemError::TransactionPrepareFailed {
                            message: error.to_string(),
                        }
                    })?,
                ),
                input: crate::graph_document::PortAddress::declared(
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
        .map(|id| (id, crate::graph_document::NodeId::new()))
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
            connection.id = crate::graph_document::ConnectionId::new();
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
    document.document.revision = crate::graph_document::GraphRevision::INITIAL;
    if let Some(function) = document.function.as_mut() {
        function.revision = ResourceRevision::INITIAL;
    }
    document
}

fn resource_from_disk_document(
    document: &crate::project::project_io::GraphDocument,
) -> GraphResourceDocument {
    let mut graph = document.document.clone();
    graph.revision = document.revision.to_graph_revision();
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
    if resource.document.revision != authority_revision.to_graph_revision() {
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
        document.document.revision = authority_revision.to_graph_revision();
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
    document.document.revision = authority_revision.to_graph_revision();
    document
        .function
        .as_mut()
        .expect("persisted function metadata was validated")
        .revision = authority_revision;
    Ok(())
}

impl ProjectState {
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
        let revision = ResourceRevision::from_graph_revision(resource.document.revision);
        let context = resource_context(self, session.clone(), operation_id, [], [graph_key(&path)]);
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.clone(),
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
        #[cfg(test)]
        self.run_resource_mutation_test_hook(
            ResourceMutationTestPoint::BeforePublication,
            Some(&path),
        );
        let result = match self.apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::DeclareGraph {
                path: path.clone(),
                revision,
            },
        ) {
            Ok(result) => {
                committed.finalize();
                Ok(result)
            }
            Err(error) => committed.rollback().and(Err(error)),
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
        let source_kind = source.kind().into();
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
        if authority_revision != expected_revision.to_graph_revision() {
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
            validate_loaded_duplicate_source_authority(
                source,
                resource,
                ResourceRevision::from_graph_revision(authority_revision),
            )?;
            crate::project::project_io::snapshot_graph_document(&current_data, source).map_err(
                |error| ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                },
            )?
        } else {
            let source_bytes = crate::project::read_secure_project_file(
                session.root.as_path(),
                std::path::Path::new(source.as_str()),
            )
            .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            })?;
            let mut persisted_source: crate::project::project_io::GraphDocument =
                serde_json::from_slice(&source_bytes).map_err(|error| {
                    ProjectFilesystemError::TransactionPrepareFailed {
                        message: error.to_string(),
                    }
                })?;
            bind_unloaded_duplicate_source_authority(
                source,
                &mut persisted_source,
                ResourceRevision::from_graph_revision(authority_revision),
            )?;
            persisted_source
        };
        let (target, unique_name) = Self::allocate_graph_path_from_snapshot(
            session.root.as_path().to_str(),
            &current_data,
            &source_document.name,
            source_document.kind,
        )?;
        let mut duplicate = rebind_duplicate(source_document, source, &target, unique_name);
        let mut target_resource = resource_from_disk_document(&duplicate);
        let retained_target_revision = self.graph_revisions.read().unwrap().get(&target).copied();
        let target_revision = crate::project::project_state::normalize_function_resource_revision(
            &target,
            &mut target_resource,
            retained_target_revision,
        )?;
        duplicate.revision = ResourceRevision::from_graph_revision(target_revision);
        duplicate.document.revision = target_revision;
        if let Some(function) = duplicate.function.as_mut() {
            function.revision = ResourceRevision::from_graph_revision(target_revision);
        }
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
            context.clone(),
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
        #[cfg(test)]
        self.run_resource_mutation_test_hook(
            ResourceMutationTestPoint::BeforePublication,
            Some(&target),
        );
        let result = match self.apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::DeclareGraph {
                path: target.clone(),
                revision: ResourceRevision::from_graph_revision(target_revision),
            },
        ) {
            Ok(result) => {
                committed.finalize();
                Ok(result)
            }
            Err(error) => committed.rollback().and(Err(error)),
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
        if tracked_revision.is_some_and(|revision| {
            ResourceRevision::from_graph_revision(revision) != expected_revision
        }) {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
                message: format!("revision for '{}' changed", graph_path),
            });
        }
        let expected = tracked_revision
            .map(|revision| {
                (
                    graph_key(graph_path),
                    ResourceRevision::from_graph_revision(revision),
                )
            })
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
}
