use thiserror::Error;

use super::inputs::{DraftResolutionContext, GraphContractMappingError, GraphInputError};
use crate::session::{ApplicationState, SessionCaptureError, SessionRevalidationError};
use yss_database_runtime::error::DatabaseError;
use yss_graph_document::{GraphDocument, GraphResourcePath};
use yss_graph_document_edit::{DocumentError, validate_graph_document};
use yss_graph_editor::projection::{
    EditorProjectionError, EditorProjectionInput, EditorProjectionModel, build_editor_projection,
};
use yss_graph_runtime::GraphDraftCompilationError;
use yss_project::ProjectOperationError;
use yss_project_identity::ProjectInstanceId;

#[derive(Clone, Debug, PartialEq)]
pub enum CompileGraphDraftReceipt {
    Ready {
        artifact_id: [u8; 32],
        cache_hit: bool,
        projection: EditorProjectionModel,
    },
    Blocked {
        projection: EditorProjectionModel,
    },
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
    ProjectFacts(#[source] crate::graph::catalog::ProjectCatalogReadError),
    #[error("project snapshot failed")]
    Project(#[source] ProjectOperationError),
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

impl From<GraphInputError> for CompileGraphDraftError {
    fn from(error: GraphInputError) -> Self {
        match error {
            GraphInputError::Catalog(error) => Self::ProjectFacts(error),
            GraphInputError::Database(error) => Self::Database(error),
            GraphInputError::Contract(error) => Self::Contract(error),
        }
    }
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
        .has_resident_graph(&graph_path)
        .map_err(CompileGraphDraftError::Project)?
    {
        return Err(CompileGraphDraftError::GraphUnavailable);
    }

    let context = DraftResolutionContext::capture(&captured, &document)?;
    validate_graph_document(&document).map_err(CompileGraphDraftError::InvalidDocument)?;
    let registry_fingerprint = context.registry_fingerprint;
    let compilation = captured
        .graph()
        .compile_draft(
            &document,
            graph_path.clone(),
            &context.graph_catalog,
            &context.basis,
            &|id| captured.execution().kernels().supports(id),
        )
        .map_err(CompileGraphDraftError::Compilation)?;
    let analysis = captured.graph().localize_analysis(
        &document,
        compilation.analysis().clone(),
        context.project.resources().entries(),
        locale,
    );
    let projection = build_editor_projection(EditorProjectionInput {
        graph_path: &graph_path,
        document: &document,
        analysis: &analysis,
        registry_fingerprint,
    })
    .map_err(CompileGraphDraftError::Projection)?;

    context.revalidate(&captured)?;
    state
        .revalidate_captured_session(&captured)
        .map_err(CompileGraphDraftError::SessionChanged)?;

    captured.execution().observe_graph_result_inputs(
        graph_path.as_str(),
        crate::graph::inputs::graph_result_inputs(
            &graph_path,
            &analysis,
            &context.database,
            registry_fingerprint,
        ),
    );

    Ok(match compilation.artifact_id() {
        Some(artifact_id) => CompileGraphDraftReceipt::Ready {
            artifact_id: *artifact_id,
            cache_hit: compilation.cache_hit(),
            projection,
        },
        None => CompileGraphDraftReceipt::Blocked { projection },
    })
}
