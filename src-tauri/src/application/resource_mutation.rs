use std::sync::Arc;

use thiserror::Error;

use super::catalog_query::capture_localized_project_facts;
use super::editor_projection::{
    EditorProjectionError, EditorProjectionInput, build_editor_projection,
};
use super::events::{
    CommittedResourceMutation, GraphMutationResult, GraphProjectionReplacement, HistoryStatus,
    committed_resource_mutation_from_project,
};
use super::execution::session_slot::{
    ApplicationSession, ApplicationSessionRefreshError, ApplicationState, SessionCaptureError,
    SessionRevalidationError,
};
use super::graph_commit::{GraphCommitApplicationError, commit_captured_graph_candidate};
use super::graph_contracts::{
    GraphContractMappingError, build_resource_catalog, graph_compilation_basis,
};
use crate::database::error::DatabaseError;
use crate::database::session_api::catalog_snapshot;
use crate::project::project_writers::ProjectSaveResult;
use crate::project::{FunctionDocumentPatch, HistoryMutation, MutationRequest};
use crate::project::{GraphDocumentKind, GraphResourceDocument, ProjectFilesystemError};
use std::collections::BTreeMap;
use yss_execution::plan::{
    PlanCompilationBasis, PlanGraphRevision, PlanProjectSessionId, PlanRegistryFingerprint,
};
use yss_graph_catalog::CatalogResourcePath;
use yss_graph_document::GraphResourcePath;
use yss_graph_document_edit::apply_graph_document_patch;
use yss_graph_editor::{
    CatalogFunctionParameter, CatalogFunctionSignature, CatalogMutationResource,
    CatalogMutationValidationSnapshot, ClipboardSubgraph, EditorGraphMutation, MutationConflict,
};
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};

#[derive(Debug, Error)]
pub enum ResourceMutationApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error(transparent)]
    Project(#[from] ProjectFilesystemError),
    #[error("graph resource mutation conflicted")]
    Mutation(#[source] MutationConflict),
    #[error("project history mutation conflicted")]
    History(#[source] crate::project::ProjectHistoryMutationError),
    #[error("graph operation capture failed")]
    GraphOperation(
        #[source] crate::project::project_state::graph_operation::ProjectGraphOperationError,
    ),
    #[error("graph operation commit failed")]
    GraphCommit(#[source] crate::project::project_state::graph_operation::ProjectGraphCommitError),
    #[error("graph resource is unavailable")]
    GraphUnavailable { graph: GraphResourcePath },
    #[error("project catalog facts could not be captured")]
    Catalog(#[source] crate::application::catalog_query::ProjectCatalogReadError),
    #[error("database catalog snapshot failed")]
    Database(#[source] DatabaseError),
    #[error("graph catalog mapping failed")]
    Contract(#[source] GraphContractMappingError),
    #[error("editor projection failed")]
    Projection(#[source] EditorProjectionError),
    #[error("captured application session changed")]
    SessionChanged(#[source] SessionRevalidationError),
    #[error("application session refresh failed")]
    SessionRefresh(#[source] ApplicationSessionRefreshError),
}

fn map_graph_commit_application_error(
    error: GraphCommitApplicationError,
) -> ResourceMutationApplicationError {
    match error {
        GraphCommitApplicationError::SessionCapture(error) => {
            ResourceMutationApplicationError::SessionCapture(error)
        }
        GraphCommitApplicationError::SessionChanged => {
            ResourceMutationApplicationError::SessionChanged(SessionRevalidationError::Changed)
        }
        GraphCommitApplicationError::Commit(error) => {
            ResourceMutationApplicationError::GraphCommit(error)
        }
    }
}

fn build_catalog_mutation_validation_snapshot(
    captured: &ApplicationSession,
) -> Result<CatalogMutationValidationSnapshot, ResourceMutationApplicationError> {
    let index = captured
        .project()
        .read_project_index(captured.project_instance_id())
        .map_err(ResourceMutationApplicationError::Project)?;
    let mut resources = BTreeMap::new();

    for graph in index.graphs {
        let Some(signature) = graph.function_signature else {
            continue;
        };
        resources.insert(
            CatalogResourcePath::new(graph.path),
            CatalogMutationResource::Function {
                revision: graph.function_revision.unwrap_or(graph.revision).get(),
                signature: CatalogFunctionSignature {
                    parameters: signature
                        .parameters
                        .into_iter()
                        .map(|parameter| CatalogFunctionParameter {
                            id: parameter.id,
                            name: parameter.name,
                            type_name: parameter.type_name,
                        })
                        .collect(),
                    return_type: signature.return_type,
                },
            },
        );
    }

    for variable in index.variables {
        resources.insert(
            CatalogResourcePath::new(variable.resource_path.as_str()),
            CatalogMutationResource::Variable {
                revision: variable.revision.get(),
                scope: variable.scope,
                data_type: variable.data_type,
            },
        );
    }

    for database in index.databases {
        resources.insert(
            CatalogResourcePath::new(database.resource_path.as_str()),
            CatalogMutationResource::Database {
                authority_revision: database.revision.get(),
            },
        );
    }

    Ok(CatalogMutationValidationSnapshot { resources })
}

fn build_graph_shell(
    path: &GraphResourcePath,
    name: String,
    kind: GraphDocumentKind,
) -> Result<GraphResourceDocument, ResourceMutationApplicationError> {
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
        let id = yss_graph_document::NodeId::new();
        let parameters = if kind == GraphDocumentKind::Function {
            [(
                yss_graph_protocol::ParameterKey::new("function").map_err(|error| {
                    ResourceMutationApplicationError::Project(
                        ProjectFilesystemError::TransactionPrepareFailed {
                            message: error.to_string(),
                        },
                    )
                })?,
                serde_json::Value::String(path.as_str().to_owned()),
            )]
            .into_iter()
            .collect()
        } else {
            Default::default()
        };
        resource.document.nodes.insert(
            id,
            yss_graph_document::DocumentNode {
                id,
                node_type: yss_graph_protocol::NodeTypeId::new(*node_type).map_err(|error| {
                    ResourceMutationApplicationError::Project(
                        ProjectFilesystemError::TransactionPrepareFailed {
                            message: error.to_string(),
                        },
                    )
                })?,
                position: yss_graph_document::NodePosition { x: *x, y: 160.0 },
                parameters,
                user_label: None,
            },
        );
        shell_nodes.push(id);
    }
    if let [entry, returned] = shell_nodes.as_slice() {
        let id = yss_graph_document::ConnectionId::new();
        resource.document.connections.insert(
            id,
            yss_graph_document::DocumentConnection {
                id,
                output: yss_graph_document::PortAddress::declared(
                    *entry,
                    yss_graph_protocol::PortKey::new("then").map_err(|error| {
                        ResourceMutationApplicationError::Project(
                            ProjectFilesystemError::TransactionPrepareFailed {
                                message: error.to_string(),
                            },
                        )
                    })?,
                ),
                input: yss_graph_document::PortAddress::declared(
                    *returned,
                    yss_graph_protocol::PortKey::new("enter").map_err(|error| {
                        ResourceMutationApplicationError::Project(
                            ProjectFilesystemError::TransactionPrepareFailed {
                                message: error.to_string(),
                            },
                        )
                    })?,
                ),
                order: None,
            },
        );
    }
    Ok(resource)
}

fn build_graph_projection_replacement(
    captured: &ApplicationSession,
    graph_path: &GraphResourcePath,
    document: &yss_graph_document::GraphDocument,
    locale: &str,
) -> Result<GraphProjectionReplacement, ResourceMutationApplicationError> {
    let project = capture_localized_project_facts(captured)
        .map_err(ResourceMutationApplicationError::Catalog)?;
    let database = catalog_snapshot(captured.database())
        .map_err(ResourceMutationApplicationError::Database)?;
    let _validated_graph_catalog = build_resource_catalog(project.resources().graph(), &database)
        .map_err(ResourceMutationApplicationError::Contract)?;
    let registry_fingerprint = captured.graph().registry_fingerprint();
    let basis = PlanCompilationBasis::new(
        PlanProjectSessionId::from_existing(captured.project_session_id().as_str().into()),
        PlanGraphRevision::from_existing(document.revision.get()),
        PlanRegistryFingerprint::from_bytes(registry_fingerprint),
        Default::default(),
        Default::default(),
    );
    let graph_basis = graph_compilation_basis(&basis);
    let analysis = captured.graph().analyze(document, &graph_basis);
    let model = build_editor_projection(EditorProjectionInput {
        graph_path,
        document,
        analysis: &analysis,
        registry_fingerprint,
    })
    .map_err(ResourceMutationApplicationError::Projection)?;
    let function_editor_projection = captured
        .project()
        .get_data()
        .map_err(ResourceMutationApplicationError::Project)?
        .graphs
        .get(graph_path)
        .and_then(|resource| resource.function.as_ref())
        .map(|function| {
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
        })
        .transpose()
        .map_err(|error| {
            ResourceMutationApplicationError::Project(
                ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                },
            )
        })?;
    let _ = locale;
    Ok(GraphProjectionReplacement {
        graph_path: graph_path.as_str().into(),
        projection: model,
        function_editor_projection,
    })
}

impl ApplicationState {
    pub fn export_graph_subgraph(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        node_ids: Vec<yss_graph_document::NodeId>,
    ) -> Result<ClipboardSubgraph, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let catalog = build_catalog_mutation_validation_snapshot(&captured)?;
        let data = captured
            .project()
            .get_data()
            .map_err(ResourceMutationApplicationError::Project)?;
        let document = data
            .graphs
            .get(&graph_path)
            .map(|resource| resource.document.clone())
            .ok_or_else(|| ResourceMutationApplicationError::GraphUnavailable {
                graph: graph_path.clone(),
            })?;
        let result = captured
            .graph()
            .export_subgraph(&graph_path, &document, &catalog, node_ids)
            .map_err(ResourceMutationApplicationError::Mutation)?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(result)
    }

    pub fn mutate_graph_document(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        locale: String,
        request: MutationRequest<EditorGraphMutation>,
    ) -> Result<GraphMutationResult, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let request_operation_id = request.operation_id;
        let expected_revision = request.base_revision.to_graph_revision();
        let capture = captured
            .project()
            .capture_graph_operation(
                &project_instance_id,
                &graph_path,
                expected_revision,
                request_operation_id,
            )
            .map_err(ResourceMutationApplicationError::GraphOperation)?;
        let catalog = build_catalog_mutation_validation_snapshot(&captured)?;
        let patch = captured
            .graph()
            .plan_editor_mutation(
                &graph_path,
                capture.document.as_ref(),
                request.payload,
                &catalog,
            )
            .map_err(ResourceMutationApplicationError::Mutation)?;
        let mut candidate = capture.document.as_ref().clone();
        apply_graph_document_patch(&mut candidate, &patch).map_err(|error| {
            ResourceMutationApplicationError::Mutation(MutationConflict::Document(error))
        })?;
        let receipt = commit_captured_graph_candidate(
            self,
            &captured,
            capture,
            std::sync::Arc::new(candidate.clone()),
        )
        .map_err(map_graph_commit_application_error)?;
        let projection =
            build_graph_projection_replacement(&captured, &graph_path, &candidate, &locale)?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(GraphMutationResult {
            project_instance_id,
            delta: crate::application::events::GraphDeltaEvent {
                graph_path,
                from_revision: ResourceRevision::from_graph_revision(receipt.from_revision),
                to_revision: ResourceRevision::from_graph_revision(receipt.to_revision),
                caused_by: Some(request_operation_id),
                payload: patch,
            },
            projection_replacement: projection,
            history: HistoryStatus {
                can_undo: receipt.history.can_undo,
                can_redo: receipt.history.can_redo,
            },
        })
    }

    pub fn query_history_status(
        &self,
        project_instance_id: ProjectInstanceId,
    ) -> Result<HistoryStatus, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured
            .project()
            .history_status_for_project(&project_instance_id)?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(HistoryStatus {
            can_undo: result.can_undo,
            can_redo: result.can_redo,
        })
    }

    pub fn undo_graph_document(
        &self,
        project_instance_id: ProjectInstanceId,
        locale: String,
        request: MutationRequest<HistoryMutation>,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured
            .project()
            .undo_history_for_application(&project_instance_id, request)
            .map_err(ResourceMutationApplicationError::History)?;
        self.refresh_resource_session()?;
        let _ = locale;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn redo_graph_document(
        &self,
        project_instance_id: ProjectInstanceId,
        locale: String,
        request: MutationRequest<HistoryMutation>,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured
            .project()
            .redo_history_for_application(&project_instance_id, request)
            .map_err(ResourceMutationApplicationError::History)?;
        self.refresh_resource_session()?;
        let _ = locale;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn create_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        name: String,
        kind: GraphDocumentKind,
        operation_id: OperationId,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let (path, unique_name) = captured.project().allocate_graph_path_for_application(
            &project_instance_id,
            &name,
            kind,
        )?;
        let resource = build_graph_shell(&path, unique_name, kind)?;
        let resource_name = resource.name.clone();
        let result = captured.project().create_graph_resource_for_application(
            &project_instance_id,
            &resource_name,
            resource,
            operation_id,
        )?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn duplicate_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured
            .project()
            .duplicate_graph_resource_for_application(
                &project_instance_id,
                &graph_path,
                expected_revision,
                operation_id,
            )?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn remove_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured.project().remove_graph_resource_for_application(
            &project_instance_id,
            &graph_path,
            expected_revision,
            operation_id,
        )?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn rename_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        expected_revision: ResourceRevision,
        new_name: String,
        lifecycle_token: u64,
        operation_id: OperationId,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured.project().rename_graph_resource_transaction_impl(
            &project_instance_id,
            &graph_path,
            expected_revision,
            &new_name,
            lifecycle_token,
            operation_id,
        )?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn save_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ProjectSaveResult, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured.project().save_graph_resource_for_application(
            &project_instance_id,
            &graph_path,
            expected_revision,
            operation_id,
        )?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(result)
    }

    pub fn unload_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        lifecycle_token: u64,
    ) -> Result<(), ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        captured.project().unload_graph_resource_for_lifecycle(
            &project_instance_id,
            &graph_path,
            lifecycle_token,
        )?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(())
    }

    pub fn update_function_signature(
        &self,
        project_instance_id: ProjectInstanceId,
        function_path: GraphResourcePath,
        locale: String,
        request: MutationRequest<FunctionDocumentPatch>,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let _ = locale;
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured
            .project()
            .update_function_signature_for_application(
                &project_instance_id,
                &function_path,
                request,
            )
            .map_err(ResourceMutationApplicationError::History)?;
        self.refresh_resource_session()?;
        Ok(committed_resource_mutation_from_project(result))
    }

    fn capture_resource_session(
        &self,
        project_instance_id: &ProjectInstanceId,
    ) -> Result<Arc<ApplicationSession>, ResourceMutationApplicationError> {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != project_instance_id {
            return Err(ResourceMutationApplicationError::Project(
                ProjectFilesystemError::StaleProjectLifecycle {
                    message: "resource mutation project instance is stale".into(),
                },
            ));
        }
        Ok(captured)
    }

    fn refresh_resource_session(&self) -> Result<(), ResourceMutationApplicationError> {
        self.refresh_current_project()
            .map_err(ResourceMutationApplicationError::SessionRefresh)
    }
}
