use std::sync::Arc;

use thiserror::Error;

use super::catalog_query::capture_localized_project_facts;
use super::editor_projection::{
    EditorProjectionError, EditorProjectionInput, build_editor_projection,
};
use super::events::{
    CommittedResourceMutation, GraphProjectionReplacement, HistoryStatus,
    committed_resource_mutation_from_project,
};
use super::execution::session_slot::{
    ApplicationSession, ApplicationSessionRefreshError, ApplicationState, SessionCaptureError,
    SessionRevalidationError,
};
use super::graph_contracts::{
    GraphContractMappingError, build_resource_catalog, graph_compilation_basis,
};
use std::collections::BTreeMap;
use yss_database_runtime::error::DatabaseError;
use yss_database_runtime::session_api::catalog_snapshot;
use yss_execution::plan::{PlanCompilationBasis, PlanProjectSessionId, PlanRegistryFingerprint};
use yss_function_editor_projection::FunctionEditorProjection;
use yss_graph_catalog::CatalogResourcePath;
use yss_graph_document::{GraphDocument, GraphResourceKind, GraphResourcePath};
use yss_graph_document_edit::{apply_graph_document_patch, validate_graph_document};
use yss_graph_editor::{
    CatalogFunctionParameter, CatalogFunctionSignature, CatalogMutationResource,
    CatalogMutationValidationSnapshot, ClipboardSubgraph, EditorGraphMutation, MutationConflict,
};
use yss_project_filesystem::ProjectFilesystemError;
use yss_project_history::{FunctionDocumentPatch, HistoryMutation, MutationRequest};
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};
use yss_project_model::GraphResourceDocument;

#[derive(Debug, Error)]
pub enum ResourceMutationApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error(transparent)]
    Project(#[from] ProjectFilesystemError),
    #[error("graph resource mutation conflicted")]
    Mutation(#[source] MutationConflict),
    #[error("project history mutation conflicted")]
    History(#[source] yss_project_history::ProjectHistoryMutationError),
    #[error("graph operation capture failed")]
    GraphOperation(#[source] yss_project::ProjectGraphOperationError),
    #[error("graph operation commit failed")]
    GraphCommit(#[source] yss_project::ProjectGraphCommitError),
    #[error("graph resource is unavailable")]
    GraphUnavailable { graph: GraphResourcePath },
    #[error("project catalog facts could not be captured")]
    Catalog(#[source] crate::catalog_query::ProjectCatalogReadError),
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

#[derive(Debug, Clone, PartialEq)]
pub struct GraphDraftTransform {
    pub changed: bool,
    pub document: GraphDocument,
    pub projection_replacement: GraphProjectionReplacement,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphDraftSave {
    pub project_instance_id: ProjectInstanceId,
    pub operation_id: OperationId,
    pub resource_revision: ResourceRevision,
    pub document: GraphDocument,
    pub projection_replacement: GraphProjectionReplacement,
    pub history: HistoryStatus,
}

fn map_graph_save_error(
    error: yss_project::ProjectGraphSaveError,
) -> ResourceMutationApplicationError {
    match error {
        yss_project::ProjectGraphSaveError::Filesystem(error) => {
            ResourceMutationApplicationError::Project(error)
        }
        yss_project::ProjectGraphSaveError::Commit(error) => {
            ResourceMutationApplicationError::GraphCommit(error)
        }
    }
}

pub(crate) fn build_catalog_mutation_validation_snapshot(
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
    kind: GraphResourceKind,
) -> Result<GraphResourceDocument, ResourceMutationApplicationError> {
    let mut resource = GraphResourceDocument::new(name, kind);
    let shell_types: &[(&str, f64)] = match kind {
        GraphResourceKind::Event => &[],
        GraphResourceKind::Function => &[
            ("yssbi.project.function.entry", 120.0),
            ("yssbi.project.function.return", 560.0),
        ],
    };
    for (node_type, x) in shell_types {
        let id = yss_graph_document::NodeId::new();
        let parameters = if kind == GraphResourceKind::Function {
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
    }
    Ok(resource)
}

pub(crate) struct DraftResolutionContext {
    project: crate::catalog_query::LocalizedCatalogProjectFacts,
    database: yss_database_runtime::session_api::DatabaseCatalogSnapshot,
    graph_catalog: yss_graph_resource_contract::ResourceCatalogSnapshot,
    basis: yss_graph_analysis_contract::CompilationBasis,
    registry_fingerprint: [u8; 32],
}

impl DraftResolutionContext {
    pub(crate) fn capture(
        captured: &ApplicationSession,
        document: &GraphDocument,
    ) -> Result<Self, ResourceMutationApplicationError> {
        let project = capture_localized_project_facts(captured)
            .map_err(ResourceMutationApplicationError::Catalog)?;
        let database = catalog_snapshot(captured.database())
            .map_err(ResourceMutationApplicationError::Database)?;
        yss_database_runtime::session_api::revalidate_declaration_observations(
            captured.database(),
            project.resources().database_observations(),
        )
        .map_err(ResourceMutationApplicationError::Database)?;
        let graph_catalog = build_resource_catalog(project.resources().graph(), &database)
            .map_err(ResourceMutationApplicationError::Contract)?;
        let graph_catalog = crate::graph_contracts::capture_function_dependencies(
            captured,
            document,
            graph_catalog,
        )
        .map_err(ResourceMutationApplicationError::Contract)?;
        let registry_fingerprint = captured.graph().registry_fingerprint();
        let basis = graph_compilation_basis(&PlanCompilationBasis::new(
            PlanProjectSessionId::from_existing(captured.project_session_id().as_str().into()),
            PlanRegistryFingerprint::from_bytes(registry_fingerprint),
            Default::default(),
            Default::default(),
        ));
        Ok(Self {
            project,
            database,
            graph_catalog,
            basis,
            registry_fingerprint,
        })
    }

    fn include_functions(
        &mut self,
        captured: &ApplicationSession,
        document: &GraphDocument,
    ) -> Result<(), ResourceMutationApplicationError> {
        self.graph_catalog = crate::graph_contracts::capture_function_dependencies(
            captured,
            document,
            self.graph_catalog.clone(),
        )
        .map_err(ResourceMutationApplicationError::Contract)?;
        Ok(())
    }

    pub(crate) fn resolve(
        &self,
        captured: &ApplicationSession,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        locale: &str,
    ) -> yss_graph_analysis::GraphAnalysis {
        captured.graph().resolve_graph_draft(
            graph_path,
            document,
            &self.basis,
            &self.graph_catalog,
            self.project.resources().entries(),
            locale,
        )
    }

    pub(crate) fn revalidate(
        &self,
        captured: &ApplicationSession,
    ) -> Result<(), ResourceMutationApplicationError> {
        crate::catalog_query::revalidate_project_catalog_facts(captured, &self.project)
            .map_err(ResourceMutationApplicationError::Catalog)?;
        yss_database_runtime::session_api::revalidate_declaration_observations(
            captured.database(),
            self.project.resources().database_observations(),
        )
        .map_err(ResourceMutationApplicationError::Database)?;
        yss_database_runtime::session_api::revalidate_catalog_snapshot(
            captured.database(),
            &self.database,
        )
        .map_err(ResourceMutationApplicationError::Database)
    }
}

fn build_graph_projection_replacement(
    captured: &ApplicationSession,
    context: &mut DraftResolutionContext,
    graph_path: &GraphResourcePath,
    document: &yss_graph_document::GraphDocument,
    locale: &str,
) -> Result<GraphProjectionReplacement, ResourceMutationApplicationError> {
    context.include_functions(captured, document)?;
    let analysis = context.resolve(captured, graph_path, document, locale);
    let model = build_editor_projection(EditorProjectionInput {
        graph_path,
        document,
        analysis: &analysis,
        registry_fingerprint: context.registry_fingerprint,
    })
    .map_err(ResourceMutationApplicationError::Projection)?;
    let function_editor_projection = captured
        .project()
        .get_data()
        .map_err(ResourceMutationApplicationError::Project)?
        .graphs
        .get(graph_path)
        .and_then(|resource| resource.function.as_ref())
        .map(FunctionEditorProjection::try_from)
        .transpose()
        .map_err(|error| {
            ResourceMutationApplicationError::Project(
                ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                },
            )
        })?;
    Ok(GraphProjectionReplacement {
        graph_path: graph_path.as_str().into(),
        projection: model,
        function_editor_projection,
    })
}

impl ApplicationState {
    pub fn resolve_graph_draft(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        document: GraphDocument,
        locale: String,
    ) -> Result<crate::editor_projection::EditorProjectionModel, ResourceMutationApplicationError>
    {
        let captured = self.capture_resource_session(&project_instance_id)?;
        validate_graph_document(&document).map_err(|error| {
            ResourceMutationApplicationError::Mutation(MutationConflict::Document(error))
        })?;
        let mut context = DraftResolutionContext::capture(&captured, &document)?;
        let replacement = build_graph_projection_replacement(
            &captured,
            &mut context,
            &graph_path,
            &document,
            &locale,
        )?;
        context.revalidate(&captured)?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(replacement.projection)
    }

    pub fn export_graph_draft_subgraph(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        document: GraphDocument,
        node_ids: Vec<yss_graph_document::NodeId>,
    ) -> Result<ClipboardSubgraph, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let catalog = build_catalog_mutation_validation_snapshot(&captured)?;
        if !captured
            .project()
            .get_data()
            .map_err(ResourceMutationApplicationError::Project)?
            .graphs
            .contains_key(&graph_path)
        {
            return Err(ResourceMutationApplicationError::GraphUnavailable {
                graph: graph_path.clone(),
            });
        }
        validate_graph_document(&document).map_err(|error| {
            ResourceMutationApplicationError::Mutation(MutationConflict::Document(error))
        })?;
        let result = captured
            .graph()
            .export_subgraph(&graph_path, &document, &catalog, node_ids)
            .map_err(ResourceMutationApplicationError::Mutation)?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(result)
    }

    pub fn transform_graph_draft(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        locale: String,
        document: GraphDocument,
        mutation: EditorGraphMutation,
    ) -> Result<GraphDraftTransform, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        if !captured
            .project()
            .get_data()
            .map_err(ResourceMutationApplicationError::Project)?
            .graphs
            .contains_key(&graph_path)
        {
            return Err(ResourceMutationApplicationError::GraphUnavailable {
                graph: graph_path.clone(),
            });
        }
        validate_graph_document(&document).map_err(|error| {
            ResourceMutationApplicationError::Mutation(MutationConflict::Document(error))
        })?;
        let catalog = build_catalog_mutation_validation_snapshot(&captured)?;
        let mut context = DraftResolutionContext::capture(&captured, &document)?;
        let analysis = context.resolve(&captured, &graph_path, &document, &locale);
        let patch = captured
            .graph()
            .plan_editor_mutation(
                &graph_path,
                &document,
                mutation,
                &catalog,
                analysis.semantic_snapshot(),
            )
            .map_err(ResourceMutationApplicationError::Mutation)?;
        let changed = !patch.operations.is_empty();
        let mut candidate = document;
        apply_graph_document_patch(&mut candidate, &patch).map_err(|error| {
            ResourceMutationApplicationError::Mutation(MutationConflict::Document(error))
        })?;
        let projection_replacement = build_graph_projection_replacement(
            &captured,
            &mut context,
            &graph_path,
            &candidate,
            &locale,
        )?;
        context.revalidate(&captured)?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(GraphDraftTransform {
            changed,
            document: candidate,
            projection_replacement,
        })
    }

    pub fn save_graph_draft(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        locale: String,
        operation_id: OperationId,
        document: GraphDocument,
    ) -> Result<GraphDraftSave, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let submitted_document = document;
        let (document, receipt, projection) = {
            let mut saved = None;
            for attempt in 0..3 {
                let operation = captured
                    .project()
                    .capture_graph_overwrite_operation(
                        &project_instance_id,
                        &graph_path,
                        operation_id,
                    )
                    .map_err(ResourceMutationApplicationError::GraphOperation)?;
                let candidate = submitted_document.clone();
                let mut context = DraftResolutionContext::capture(&captured, &candidate)?;
                validate_graph_document(&candidate).map_err(|error| {
                    ResourceMutationApplicationError::Mutation(MutationConflict::Document(error))
                })?;
                let projection = build_graph_projection_replacement(
                    &captured,
                    &mut context,
                    &graph_path,
                    &candidate,
                    &locale,
                )?;
                context.revalidate(&captured)?;
                match captured.project().save_graph_candidate(
                    operation,
                    operation_id,
                    Arc::new(candidate.clone()),
                ) {
                    Ok(receipt) => {
                        saved = Some((candidate, receipt, projection));
                        break;
                    }
                    Err(yss_project::ProjectGraphSaveError::Commit(
                        yss_project::ProjectGraphCommitError::StaleAuthority { .. },
                    )) if attempt < 2 => {}
                    Err(error) => return Err(map_graph_save_error(error)),
                }
            }
            saved.ok_or_else(|| {
                ResourceMutationApplicationError::SessionChanged(SessionRevalidationError::Changed)
            })?
        };
        Ok(GraphDraftSave {
            project_instance_id,
            operation_id,
            resource_revision: receipt.to_revision,
            document,
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
            .undo_history(&project_instance_id, request)
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
            .redo_history(&project_instance_id, request)
            .map_err(ResourceMutationApplicationError::History)?;
        self.refresh_resource_session()?;
        let _ = locale;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn create_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        name: String,
        kind: GraphResourceKind,
        operation_id: OperationId,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let (path, unique_name) =
            captured
                .project()
                .allocate_graph_path(&project_instance_id, &name, kind)?;
        let resource = build_graph_shell(&path, unique_name, kind)?;
        let resource_name = resource.name.clone();
        let result = captured.project().create_graph_resource(
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
        let result = captured.project().duplicate_graph_resource(
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
        let result = captured.project().remove_graph_resource(
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
        let result = captured.project().rename_graph_resource(
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
            .update_function_signature(&project_instance_id, &function_path, request)
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
