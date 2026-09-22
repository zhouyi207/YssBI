use std::sync::Arc;

use yss_graph_analysis::GraphAnalysis;
use yss_graph_document::{GraphDocument, GraphResourcePath};
use yss_graph_runtime::GraphMaterializationError;
use yss_project::ProjectOperationError;
use yss_project_identity::ProjectInstanceId;

use super::inputs::{GraphContractMappingError, GraphInputError, GraphResolutionContext};
use crate::graph::catalog::ProjectCatalogReadError;
use crate::session::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use yss_graph_editor::projection::{
    EditorProjectionError, EditorProjectionInput, EditorProjectionModel, build_editor_projection,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenGraphRequest {
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    lifecycle_token: u64,
    locale: Box<str>,
}

impl OpenGraphRequest {
    pub fn new(
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        lifecycle_token: u64,
        locale: impl Into<Box<str>>,
    ) -> Self {
        Self {
            project_instance_id,
            graph_path,
            lifecycle_token,
            locale: locale.into(),
        }
    }

    pub fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.project_instance_id
    }

    pub fn graph_path(&self) -> &GraphResourcePath {
        &self.graph_path
    }

    pub const fn lifecycle_token(&self) -> u64 {
        self.lifecycle_token
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }
}

#[derive(Debug, thiserror::Error)]
#[error("graph-open project operation failed")]
pub struct OpenGraphProjectSource {
    #[source]
    reason: OpenGraphProjectSourceKind,
}

#[derive(Debug, thiserror::Error)]
enum OpenGraphProjectSourceKind {
    #[error("project filesystem operation failed")]
    Filesystem(#[source] ProjectOperationError),
    #[error("project catalog facts could not be captured")]
    Catalog(#[source] ProjectCatalogReadError),
}

impl OpenGraphProjectSource {
    fn filesystem(error: ProjectOperationError) -> Self {
        Self {
            reason: OpenGraphProjectSourceKind::Filesystem(error),
        }
    }

    fn catalog(error: ProjectCatalogReadError) -> Self {
        Self {
            reason: OpenGraphProjectSourceKind::Catalog(error),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OpenGraphProjectError {
    #[error("graph belongs to another project instance")]
    ProjectIdentityMismatch { requested: ProjectInstanceId },
    #[error("graph-open project authority changed")]
    StaleProjectAuthority { graph: GraphResourcePath },
    #[error("graph resource lifecycle changed")]
    ResourceLifecycleChanged { graph: GraphResourcePath },
    #[error("project lifecycle admission is closed")]
    AdmissionClosed,
    #[error("project recovery is required")]
    RecoveryRequired,
    #[error("graph document is invalid")]
    InvalidGraphDocument { graph: GraphResourcePath },
    #[error("graph revision is exhausted")]
    RevisionExhausted {
        graph: GraphResourcePath,
        revision: yss_project_identity::ResourceRevision,
    },
    #[error("project filesystem transaction is busy")]
    FilesystemBusy,
    #[error("graph-open transaction preparation failed")]
    PrepareFailed(#[source] OpenGraphProjectSource),
    #[error("graph-open transaction commit failed")]
    CommitFailed(#[source] OpenGraphProjectSource),
    #[error("graph-open transaction rollback failed")]
    RollbackFailed {
        recovery_required: bool,
        #[source]
        source: OpenGraphProjectSource,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum OpenGraphApplicationError {
    #[error("graph editing is busy")]
    EditingBusy,
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("captured graph-open session changed")]
    SessionChanged,
    #[error(transparent)]
    Project(#[from] OpenGraphProjectError),
    #[error(transparent)]
    Database(#[from] yss_database_runtime::error::DatabaseError),
    #[error(transparent)]
    Contract(#[from] GraphContractMappingError),
    #[error(transparent)]
    Materialization(#[from] GraphMaterializationError),
    #[error(transparent)]
    Projection(#[from] EditorProjectionError),
}

#[derive(Clone, Debug)]
pub struct OpenGraphApplicationReceipt {
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    document: Arc<GraphDocument>,
    analysis: GraphAnalysis,
    projection: EditorProjectionModel,
    function_editor_projection: Option<yss_function_editor_projection::FunctionEditorProjection>,
    editing: yss_project::GraphEditingState,
    result_state: super::results::GraphResultState,
}

impl OpenGraphApplicationReceipt {
    pub fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.project_instance_id
    }

    pub fn graph_path(&self) -> &GraphResourcePath {
        &self.graph_path
    }

    pub fn document(&self) -> &GraphDocument {
        &self.document
    }

    pub fn analysis(&self) -> &GraphAnalysis {
        &self.analysis
    }

    pub fn projection(&self) -> &EditorProjectionModel {
        &self.projection
    }

    pub fn editing(&self) -> &yss_project::GraphEditingState {
        &self.editing
    }

    pub fn result_state(&self) -> &super::results::GraphResultState {
        &self.result_state
    }

    pub fn function_editor_projection(
        &self,
    ) -> Option<&yss_function_editor_projection::FunctionEditorProjection> {
        self.function_editor_projection.as_ref()
    }
}

impl ApplicationState {
    pub fn open_graph(
        &self,
        request: OpenGraphRequest,
    ) -> Result<OpenGraphApplicationReceipt, OpenGraphApplicationError> {
        let captured = self.capture_session()?;
        let _editing = captured
            .coordinate_graph_edit(request.graph_path())
            .map_err(|_| OpenGraphApplicationError::EditingBusy)?;
        open_graph_in_session(self, &captured, request)
    }
}

pub(crate) fn open_graph_in_session(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    request: OpenGraphRequest,
) -> Result<OpenGraphApplicationReceipt, OpenGraphApplicationError> {
    if request.project_instance_id() != captured.project_instance_id() {
        return Err(OpenGraphProjectError::ProjectIdentityMismatch {
            requested: request.project_instance_id().clone(),
        }
        .into());
    }
    revalidate_application_session(application, captured)?;

    let already_resident = captured
        .project()
        .has_resident_graph(request.graph_path())
        .map_err(|error| map_project_open_error(request.graph_path(), error))?;
    if !already_resident {
        captured
            .project()
            .load_graph_document(
                captured.project_instance_id(),
                request.graph_path(),
                request.lifecycle_token(),
            )
            .map_err(|error| map_project_open_error(request.graph_path(), error))?;
    }

    let editing = captured
        .project()
        .read_graph_editing(request.project_instance_id(), request.graph_path())
        .map_err(|error| map_project_open_error(request.graph_path(), error))?;
    let loaded_document = editing.document;

    let input_error = |error| match error {
        GraphInputError::Catalog(error) => {
            map_project_facts_open_error(request.graph_path(), error)
        }
        GraphInputError::Database(error) => OpenGraphApplicationError::Database(error),
        GraphInputError::Contract(error) => OpenGraphApplicationError::Contract(error),
    };
    let context =
        GraphResolutionContext::capture(captured, &loaded_document).map_err(input_error)?;
    let candidate_document = captured
        .graph()
        .materialize_open_candidate(&loaded_document)?;
    let registry_fingerprint = context.registry_fingerprint;
    let analysis = context.resolve(
        captured,
        request.graph_path(),
        &candidate_document,
        request.locale(),
    );

    let function_editor_projection = captured
        .project()
        .read_resident_graph(request.graph_path())
        .map_err(|error| map_project_open_error(request.graph_path(), error))?
        .as_ref()
        .and_then(|resource| resource.function.as_ref())
        .map(yss_function_editor_projection::FunctionEditorProjection::try_from)
        .transpose()
        .map_err(|error| {
            map_project_open_error(
                request.graph_path(),
                ProjectOperationError::TransactionPrepareFailed {
                    message: error.to_string(),
                },
            )
        })?;

    // This is the final staged commit gate. A replacement that wins before
    // it suppresses the candidate; once it passes, the old Project load has
    // already linearized and the derived projection must not be relabeled by
    // a later session replacement.
    context.revalidate(captured).map_err(input_error)?;
    revalidate_application_session(application, captured)?;

    let projection = build_editor_projection(EditorProjectionInput {
        graph_path: request.graph_path(),
        document: &candidate_document,
        analysis: &analysis,
        registry_fingerprint,
    })?;
    let result_state = super::results::observe_graph_result_inputs(
        captured,
        request.graph_path().as_str(),
        crate::graph::inputs::graph_result_inputs(
            request.graph_path(),
            &analysis,
            &context.database,
            registry_fingerprint,
        ),
    );
    Ok(OpenGraphApplicationReceipt {
        project_instance_id: captured.project_instance_id().clone(),
        graph_path: request.graph_path().clone(),
        document: candidate_document,
        analysis,
        projection,
        function_editor_projection,
        editing: editing.state,
        result_state,
    })
}

fn revalidate_application_session(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
) -> Result<(), OpenGraphApplicationError> {
    application
        .revalidate_captured_session(captured)
        .map_err(|error| match error {
            SessionRevalidationError::Unavailable(error) => {
                OpenGraphApplicationError::SessionCapture(error)
            }
            SessionRevalidationError::Changed => OpenGraphApplicationError::SessionChanged,
        })
}

fn map_project_facts_open_error(
    graph: &GraphResourcePath,
    error: ProjectCatalogReadError,
) -> OpenGraphApplicationError {
    match error {
        ProjectCatalogReadError::ProjectLifecycleChanged
        | ProjectCatalogReadError::CatalogResourceStale { .. } => {
            OpenGraphProjectError::StaleProjectAuthority {
                graph: graph.clone(),
            }
            .into()
        }
        ProjectCatalogReadError::AdmissionClosed => OpenGraphProjectError::AdmissionClosed.into(),
        ProjectCatalogReadError::RecoveryRequired => OpenGraphProjectError::RecoveryRequired.into(),
        error => {
            OpenGraphProjectError::PrepareFailed(OpenGraphProjectSource::catalog(error)).into()
        }
    }
}

fn map_project_open_error(
    graph: &GraphResourcePath,
    error: ProjectOperationError,
) -> OpenGraphApplicationError {
    match &error {
        ProjectOperationError::InvalidGraphDocument { path, .. } => {
            OpenGraphProjectError::InvalidGraphDocument {
                graph: path.clone(),
            }
            .into()
        }
        ProjectOperationError::StaleProjectLifecycle { .. } => {
            OpenGraphProjectError::StaleProjectAuthority {
                graph: graph.clone(),
            }
            .into()
        }
        ProjectOperationError::StaleResourceLifecycle { .. } => {
            OpenGraphProjectError::ResourceLifecycleChanged {
                graph: graph.clone(),
            }
            .into()
        }
        ProjectOperationError::ProjectLifecycleAdmissionClosed { .. } => {
            OpenGraphProjectError::AdmissionClosed.into()
        }
        ProjectOperationError::ProjectRecoveryRequired { .. } => {
            OpenGraphProjectError::RecoveryRequired.into()
        }
        ProjectOperationError::ResourceRevisionOverflow { retained, .. } => {
            OpenGraphProjectError::RevisionExhausted {
                graph: graph.clone(),
                revision: yss_project_identity::ResourceRevision::new(*retained),
            }
            .into()
        }
        ProjectOperationError::FilesystemTransactionBusy { .. } => {
            OpenGraphProjectError::FilesystemBusy.into()
        }
        ProjectOperationError::TransactionCommitFailed { .. } => {
            OpenGraphProjectError::CommitFailed(OpenGraphProjectSource::filesystem(error)).into()
        }
        ProjectOperationError::TransactionRollbackFailed {
            recovery_required, ..
        } => OpenGraphProjectError::RollbackFailed {
            recovery_required: *recovery_required,
            source: OpenGraphProjectSource::filesystem(error),
        }
        .into(),
        _ => OpenGraphProjectError::PrepareFailed(OpenGraphProjectSource::filesystem(error)).into(),
    }
}

#[cfg(test)]
mod tests;
