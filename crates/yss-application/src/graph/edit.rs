use super::inputs::GraphResolutionContext;
use super::resources::ResourceMutationApplicationError;
use crate::events::GraphProjectionReplacement;
use crate::session::{ApplicationSession, ApplicationState};
use std::sync::Arc;
use yss_function_editor_projection::FunctionEditorProjection;
use yss_graph_document::{GraphDocument, GraphResourcePath};
use yss_graph_document_edit::{prepare_graph_document_patch, validate_graph_document};
use yss_graph_editor::projection::{EditorProjectionInput, build_editor_projection};
use yss_graph_editor::{
    CatalogMutationValidationSnapshot, ClipboardSubgraph, EditorGraphMutation, MutationConflict,
};
use yss_project::ProjectOperationError;
use yss_project_identity::ProjectInstanceId;

#[derive(Debug, Clone, PartialEq)]
pub struct GraphDocumentChange {
    pub changed: bool,
    pub document: Arc<GraphDocument>,
    pub projection_replacement: GraphProjectionReplacement,
    pub patch: yss_graph_document::GraphDocumentPatch,
    pub result_inputs: yss_graph_execution::result::GraphResultInputs,
}

fn capture_editor_context(
    captured: &ApplicationSession,
    document: &GraphDocument,
) -> Result<
    (GraphResolutionContext, CatalogMutationValidationSnapshot),
    ResourceMutationApplicationError,
> {
    validate_graph_document(document).map_err(|error| {
        ResourceMutationApplicationError::Mutation(MutationConflict::Document(error))
    })?;
    let index = captured
        .project()
        .read_project_index(captured.project_instance_id())?;
    let project = super::catalog::localized_project_facts_from_index(captured, index)
        .map_err(ResourceMutationApplicationError::Catalog)?;
    let catalog = project
        .resources()
        .mutation_validation_snapshot()
        .map_err(ResourceMutationApplicationError::Catalog)?;
    Ok((
        GraphResolutionContext::from_project_facts(captured, project)?,
        catalog,
    ))
}

fn build_graph_projection_replacement(
    captured: &ApplicationSession,
    context: &mut GraphResolutionContext,
    graph_path: &GraphResourcePath,
    document: &yss_graph_document::GraphDocument,
    locale: &str,
) -> Result<
    (
        GraphProjectionReplacement,
        yss_graph_execution::result::GraphResultInputs,
    ),
    ResourceMutationApplicationError,
> {
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
        .read_resident_graph(graph_path)
        .map_err(ResourceMutationApplicationError::Project)?
        .as_ref()
        .and_then(|resource| resource.function.as_ref())
        .map(FunctionEditorProjection::try_from)
        .transpose()
        .map_err(|error| {
            ResourceMutationApplicationError::Project(
                ProjectOperationError::TransactionPrepareFailed {
                    message: error.to_string(),
                },
            )
        })?;
    let result_inputs = crate::graph::inputs::graph_result_inputs(
        graph_path,
        &analysis,
        &context.database,
        context.registry_fingerprint,
    );
    Ok((
        GraphProjectionReplacement {
            graph_path: graph_path.as_str().into(),
            projection: model,
            function_editor_projection,
        },
        result_inputs,
    ))
}

/// One staged edit, used by both a single canvas mutation and an automation batch.
/// Nothing is published until all mutations and the final currentness checks succeed.
pub(crate) struct GraphDocumentEditor<'a> {
    captured: &'a Arc<ApplicationSession>,
    graph: &'a GraphResourcePath,
    locale: &'a str,
    original: Arc<GraphDocument>,
    document: Arc<GraphDocument>,
    context: GraphResolutionContext,
    catalog: CatalogMutationValidationSnapshot,
    patch: yss_graph_document::GraphDocumentPatch,
}

impl<'a> GraphDocumentEditor<'a> {
    pub(crate) fn new(
        captured: &'a Arc<ApplicationSession>,
        graph: &'a GraphResourcePath,
        locale: &'a str,
        document: Arc<GraphDocument>,
    ) -> Result<Self, ResourceMutationApplicationError> {
        if !captured.project().has_resident_graph(graph)? {
            return Err(ResourceMutationApplicationError::GraphUnavailable {
                graph: graph.clone(),
            });
        }
        let (context, catalog) = capture_editor_context(captured, &document)?;
        Ok(Self {
            captured,
            graph,
            locale,
            original: document.clone(),
            document,
            context,
            catalog,
            patch: yss_graph_document::GraphDocumentPatch::new(Vec::new()),
        })
    }

    pub(crate) fn document(&self) -> &GraphDocument {
        &self.document
    }

    pub(crate) fn localized_catalog(&self) -> yss_node_catalog::LocalizedCatalog {
        self.captured.graph().localized_catalog_with_resources(
            self.context.project.resources().entries(),
            self.locale,
        )
    }

    pub(crate) fn apply(
        &mut self,
        mutation: EditorGraphMutation,
    ) -> Result<Option<yss_graph_editor::SubgraphCopyMap>, ResourceMutationApplicationError> {
        if !mutation.referenced_ports(&self.document).is_empty() {
            self.context
                .include_functions(self.captured, &self.document)?;
        }
        let (patch, copied) =
            if let EditorGraphMutation::DuplicateSubgraph { node_ids, offset } = mutation {
                let duplicated = yss_graph_editor::duplicate_subgraph(
                    self.graph,
                    &self.document,
                    self.captured.graph().registry(),
                    &self.catalog,
                    node_ids,
                    offset,
                )
                .map_err(ResourceMutationApplicationError::Mutation)?;
                (duplicated.patch, Some(duplicated.identities))
            } else {
                (
                    self.captured
                        .graph()
                        .plan_editor_mutation(
                            self.graph,
                            &self.document,
                            mutation,
                            &self.catalog,
                            || {
                                self.context.resolve(
                                    self.captured,
                                    self.graph,
                                    &self.document,
                                    self.locale,
                                )
                            },
                        )
                        .map_err(ResourceMutationApplicationError::Mutation)?,
                    None,
                )
            };
        if !patch.is_empty() {
            self.document = Arc::new(
                prepare_graph_document_patch(&self.document, &patch).map_err(|error| {
                    ResourceMutationApplicationError::Mutation(MutationConflict::Document(error))
                })?,
            );
        }
        self.patch.operations.extend(patch.operations);
        Ok(copied)
    }

    pub(crate) fn finish(
        mut self,
        application: &ApplicationState,
    ) -> Result<GraphDocumentChange, ResourceMutationApplicationError> {
        let (projection_replacement, result_inputs) = build_graph_projection_replacement(
            self.captured,
            &mut self.context,
            self.graph,
            &self.document,
            self.locale,
        )?;
        self.context.revalidate(self.captured)?;
        application
            .revalidate_captured_session(self.captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(GraphDocumentChange {
            changed: !Arc::ptr_eq(&self.document, &self.original) && self.document != self.original,
            document: self.document,
            projection_replacement,
            patch: self.patch,
            result_inputs,
        })
    }
}

impl ApplicationState {
    pub fn graph_connection_candidates(
        &self,
        project: &ProjectInstanceId,
        graph: &GraphResourcePath,
        version: yss_project::GraphEditVersion,
        source: &yss_graph_document::PortAddress,
        intent: yss_graph_editor::projection::ConnectionIntent,
    ) -> Result<yss_graph_editor::projection::ConnectionCandidates, ResourceMutationApplicationError>
    {
        let captured = self.capture_resource_session(project)?;
        let document = self.current_graph_document(project, graph, version)?;
        let (mut context, catalog) = capture_editor_context(&captured, &document)?;
        context.include_functions(&captured, &document)?;
        let analysis = context.resolve(&captured, graph, &document, "en-US");
        let candidates = captured
            .graph()
            .connection_candidates(graph, &document, source, intent, &catalog, &analysis)
            .map_err(ResourceMutationApplicationError::Mutation)?;
        context.revalidate(&captured)?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        // Candidate computation may race an edit or history operation. A read
        // never grants permission to use a result from the preceding revision.
        self.current_graph_document(project, graph, version)?;
        Ok(candidates)
    }

    pub fn resolve_graph_document(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        document: GraphDocument,
        locale: String,
    ) -> Result<yss_graph_editor::projection::EditorProjectionModel, ResourceMutationApplicationError>
    {
        let captured = self.capture_resource_session(&project_instance_id)?;
        validate_graph_document(&document).map_err(|error| {
            ResourceMutationApplicationError::Mutation(MutationConflict::Document(error))
        })?;
        let mut context = GraphResolutionContext::capture(&captured, &document)?;
        let (replacement, _) = build_graph_projection_replacement(
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

    pub fn export_graph_subgraph(
        &self,
        project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        version: yss_project::GraphEditVersion,
        node_ids: Vec<yss_graph_document::NodeId>,
    ) -> Result<ClipboardSubgraph, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(project_instance_id)?;
        let document = self.current_graph_document(project_instance_id, graph_path, version)?;
        let index = captured
            .project()
            .read_project_index(captured.project_instance_id())?;
        let publication_revision = index.publication_revision;
        let authority_generation = index.authority_generation();
        let project = super::catalog::localized_project_facts_from_index(&captured, index)
            .map_err(ResourceMutationApplicationError::Catalog)?;
        let catalog = project
            .resources()
            .mutation_validation_snapshot()
            .map_err(ResourceMutationApplicationError::Catalog)?;
        let result = captured
            .graph()
            .export_subgraph(&document, &catalog, node_ids)
            .map_err(ResourceMutationApplicationError::Mutation)?;
        captured.project().validate_project_index_version(
            captured.project_instance_id(),
            publication_revision,
            authority_generation,
        )?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        self.current_graph_document(project_instance_id, graph_path, version)?;
        Ok(result)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn transform_graph_document(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        locale: String,
        document: GraphDocument,
        mutation: EditorGraphMutation,
    ) -> Result<GraphDocumentChange, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let mut editor =
            GraphDocumentEditor::new(&captured, &graph_path, &locale, Arc::new(document))?;
        let _ = editor.apply(mutation)?;
        let result = editor.finish(self)?;
        captured
            .execution()
            .observe_graph_result_inputs(graph_path.as_str(), result.result_inputs.clone());
        Ok(result)
    }
}
