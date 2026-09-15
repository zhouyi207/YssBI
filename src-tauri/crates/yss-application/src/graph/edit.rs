use super::inputs::GraphResolutionContext;
use super::resources::ResourceMutationApplicationError;
use crate::events::GraphProjectionReplacement;
use crate::session::{ApplicationSession, ApplicationState};
use std::collections::BTreeMap;
use std::sync::Arc;
use yss_function_editor_projection::FunctionEditorProjection;
use yss_graph_document::{GraphDocument, GraphResourcePath};
use yss_graph_document_edit::{apply_graph_document_patch, validate_graph_document};
use yss_graph_editor::projection::{EditorProjectionInput, build_editor_projection};
use yss_graph_editor::{
    CatalogFunctionParameter, CatalogFunctionSignature, CatalogMutationResource,
    CatalogMutationValidationSnapshot, ClipboardSubgraph, EditorGraphMutation, MutationConflict,
};
use yss_node_catalog::CatalogResourcePath;
use yss_project::ProjectOperationError;
use yss_project_identity::ProjectInstanceId;

#[derive(Debug, Clone, PartialEq)]
pub struct GraphDocumentChange {
    pub changed: bool,
    pub document: GraphDocument,
    pub projection_replacement: GraphProjectionReplacement,
    pub patch: yss_graph_document::GraphDocumentPatch,
    pub result_inputs: yss_graph_execution::result::GraphResultInputs,
}

fn build_catalog_mutation_validation_snapshot(
    index: &yss_project::ProjectIndex,
) -> CatalogMutationValidationSnapshot {
    let mut resources = BTreeMap::new();

    for graph in &index.graphs {
        let Some(signature) = graph.function_signature.clone() else {
            continue;
        };
        resources.insert(
            CatalogResourcePath::new(graph.path.clone()),
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

    for database in &index.databases {
        resources.insert(
            CatalogResourcePath::new(database.resource_path.as_str()),
            CatalogMutationResource::Database {
                authority_revision: database.revision.get(),
            },
        );
    }

    CatalogMutationValidationSnapshot { resources }
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
    original: GraphDocument,
    document: GraphDocument,
    context: GraphResolutionContext,
    catalog: CatalogMutationValidationSnapshot,
    patch: yss_graph_document::GraphDocumentPatch,
}

impl<'a> GraphDocumentEditor<'a> {
    pub(crate) fn new(
        captured: &'a Arc<ApplicationSession>,
        graph: &'a GraphResourcePath,
        locale: &'a str,
        document: GraphDocument,
    ) -> Result<Self, ResourceMutationApplicationError> {
        if !captured.project().has_resident_graph(graph)? {
            return Err(ResourceMutationApplicationError::GraphUnavailable {
                graph: graph.clone(),
            });
        }
        validate_graph_document(&document).map_err(|error| {
            ResourceMutationApplicationError::Mutation(MutationConflict::Document(error))
        })?;
        let index = captured
            .project()
            .read_project_index(captured.project_instance_id())?;
        let catalog = build_catalog_mutation_validation_snapshot(&index);
        let project = super::catalog::localized_project_facts_from_index(captured, index)
            .map_err(ResourceMutationApplicationError::Catalog)?;
        let context = GraphResolutionContext::from_project_facts(captured, project)?;
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
    ) -> Result<(), ResourceMutationApplicationError> {
        if !mutation.referenced_ports().is_empty() {
            self.context
                .include_functions(self.captured, &self.document)?;
        }
        let patch = self
            .captured
            .graph()
            .plan_editor_mutation(self.graph, &self.document, mutation, &self.catalog, || {
                self.context
                    .resolve(self.captured, self.graph, &self.document, self.locale)
            })
            .map_err(ResourceMutationApplicationError::Mutation)?;
        apply_graph_document_patch(&mut self.document, &patch).map_err(|error| {
            ResourceMutationApplicationError::Mutation(MutationConflict::Document(error))
        })?;
        self.patch.operations.extend(patch.operations);
        Ok(())
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
            changed: self.document != self.original,
            document: self.document,
            projection_replacement,
            patch: self.patch,
            result_inputs,
        })
    }
}

impl ApplicationState {
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
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        document: GraphDocument,
        node_ids: Vec<yss_graph_document::NodeId>,
    ) -> Result<ClipboardSubgraph, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let index = captured
            .project()
            .read_project_index(captured.project_instance_id())?;
        let catalog = build_catalog_mutation_validation_snapshot(&index);
        if !captured
            .project()
            .has_resident_graph(&graph_path)
            .map_err(ResourceMutationApplicationError::Project)?
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
            .export_subgraph(&document, &catalog, node_ids)
            .map_err(ResourceMutationApplicationError::Mutation)?;
        captured.project().validate_project_index_version(
            captured.project_instance_id(),
            index.publication_revision,
            index.authority_generation(),
        )?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
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
        let mut editor = GraphDocumentEditor::new(&captured, &graph_path, &locale, document)?;
        editor.apply(mutation)?;
        let result = editor.finish(self)?;
        captured
            .execution()
            .observe_graph_result_inputs(graph_path.as_str(), result.result_inputs.clone());
        Ok(result)
    }
}
