use thiserror::Error;

use crate::catalog_query::{capture_localized_project_facts, revalidate_project_catalog_facts};
use crate::editor_projection::{
    EditorProjectionError, EditorProjectionInput, EditorProjectionModel, build_editor_projection,
};
use crate::execution::{ApplicationState, SessionCaptureError, SessionRevalidationError};
use crate::graph_contracts::{
    GraphContractMappingError, build_resource_catalog, graph_compilation_basis,
};
use yss_database_runtime::error::DatabaseError;
use yss_database_runtime::session_api::{
    catalog_snapshot, revalidate_catalog_snapshot, revalidate_declaration_observations,
};
use yss_execution::plan::{PlanCompilationBasis, PlanProjectSessionId, PlanRegistryFingerprint};
use yss_graph_document::{GraphDocument, GraphResourcePath};
use yss_graph_document_edit::{DocumentError, validate_graph_document};
use yss_graph_runtime::GraphDraftCompilationError;
use yss_project_filesystem::ProjectFilesystemError;
use yss_project_identity::ProjectInstanceId;

#[derive(Clone, Debug, PartialEq)]
pub struct CompileGraphDraftReceipt {
    pub source_hash: [u8; 32],
    pub cache_hit: bool,
    pub document: GraphDocument,
    pub projection: EditorProjectionModel,
}

#[derive(Debug, Error)]
pub enum CompileGraphDraftError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("graph draft belongs to another project instance")]
    ProjectIdentityMismatch,
    #[error("graph draft is unavailable")]
    GraphUnavailable,
    #[error("graph draft document is invalid")]
    InvalidDocument(#[source] DocumentError),
    #[error("project facts could not be captured")]
    ProjectFacts(#[source] crate::catalog_query::ProjectCatalogReadError),
    #[error("project snapshot failed")]
    Project(#[source] ProjectFilesystemError),
    #[error("database catalog snapshot failed")]
    Database(#[source] DatabaseError),
    #[error("graph resource contract mapping failed")]
    Contract(#[source] GraphContractMappingError),
    #[error("graph draft compilation failed")]
    Compilation(#[source] GraphDraftCompilationError),
    #[error("graph draft projection failed")]
    Projection(#[source] EditorProjectionError),
    #[error("captured application session changed")]
    SessionChanged(#[source] SessionRevalidationError),
}

pub fn compile_graph_draft(
    state: &ApplicationState,
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    document: GraphDocument,
    locale: &str,
) -> Result<CompileGraphDraftReceipt, CompileGraphDraftError> {
    let captured = state.capture_session()?;
    if captured.project_instance_id() != &project_instance_id {
        return Err(CompileGraphDraftError::ProjectIdentityMismatch);
    }
    if !captured
        .project()
        .get_data()
        .map_err(CompileGraphDraftError::Project)?
        .graphs
        .contains_key(&graph_path)
    {
        return Err(CompileGraphDraftError::GraphUnavailable);
    }

    let project =
        capture_localized_project_facts(&captured).map_err(CompileGraphDraftError::ProjectFacts)?;
    let database =
        catalog_snapshot(captured.database()).map_err(CompileGraphDraftError::Database)?;
    revalidate_declaration_observations(
        captured.database(),
        project.resources().database_observations(),
    )
    .map_err(CompileGraphDraftError::Database)?;
    let graph_catalog = build_resource_catalog(project.resources().graph(), &database)
        .map_err(CompileGraphDraftError::Contract)?;
    let document = captured
        .graph()
        .materialize_draft(&document, &graph_catalog);
    validate_graph_document(&document).map_err(CompileGraphDraftError::InvalidDocument)?;
    let registry_fingerprint = captured.graph().registry_fingerprint();
    let basis = PlanCompilationBasis::new(
        PlanProjectSessionId::from_existing(captured.project_session_id().as_str().into()),
        PlanRegistryFingerprint::from_bytes(registry_fingerprint),
        Default::default(),
        Default::default(),
    );
    let graph_basis = graph_compilation_basis(&basis);
    let compilation = captured
        .graph()
        .compile_draft(&document, graph_path.clone(), &graph_catalog, &graph_basis)
        .map_err(CompileGraphDraftError::Compilation)?;
    let analysis = captured.graph().localize_analysis(
        &document,
        compilation.analysis().clone(),
        project.resources().entries(),
        locale,
    );
    let projection = build_editor_projection(EditorProjectionInput {
        graph_path: &graph_path,
        document: &document,
        analysis: &analysis,
        registry_fingerprint,
    })
    .map_err(CompileGraphDraftError::Projection)?;

    revalidate_project_catalog_facts(&captured, &project)
        .map_err(CompileGraphDraftError::ProjectFacts)?;
    revalidate_declaration_observations(
        captured.database(),
        project.resources().database_observations(),
    )
    .map_err(CompileGraphDraftError::Database)?;
    revalidate_catalog_snapshot(captured.database(), &database)
        .map_err(CompileGraphDraftError::Database)?;
    state
        .revalidate_captured_session(&captured)
        .map_err(CompileGraphDraftError::SessionChanged)?;

    Ok(CompileGraphDraftReceipt {
        source_hash: *compilation.source_hash(),
        cache_hit: compilation.cache_hit(),
        document,
        projection,
    })
}
