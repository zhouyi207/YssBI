use std::sync::Arc;

use crate::database::session_api::{
    catalog_snapshot, revalidate_catalog_snapshot, revalidate_declaration_observations,
};
use crate::execution::plan::{
    PlanCompilationBasis, PlanGraphRevision, PlanProjectSessionId, PlanRegistryFingerprint,
};
use crate::graph::analysis::GraphAnalysis;
use crate::graph::error::GraphMutationError;
use crate::graph_document::{GraphDocument, GraphResourcePath, GraphRevision};
use crate::project::{OperationId, ProjectFilesystemError, ProjectInstanceId};

use super::catalog_query::revalidate_project_catalog_facts;
use super::catalog_query::{ProjectCatalogReadError, capture_localized_project_facts};
use super::editor_projection::{
    EditorProjectionError, EditorProjectionInput, EditorProjectionModel, build_editor_projection,
};
use super::execution::session_slot::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use super::graph_contracts::{
    GraphContractMappingError, build_resource_catalog, graph_compile_settings,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenGraphRequest {
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    lifecycle_token: u64,
    operation_id: OperationId,
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
            operation_id: OperationId::new(),
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

    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
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
    Filesystem(#[source] ProjectFilesystemError),
    #[error("project catalog facts could not be captured")]
    Catalog(#[source] ProjectCatalogReadError),
    #[error("graph-open project invariant failed")]
    Invariant,
}

impl OpenGraphProjectSource {
    fn filesystem(error: ProjectFilesystemError) -> Self {
        Self {
            reason: OpenGraphProjectSourceKind::Filesystem(error),
        }
    }

    fn catalog(error: ProjectCatalogReadError) -> Self {
        Self {
            reason: OpenGraphProjectSourceKind::Catalog(error),
        }
    }

    fn invariant() -> Self {
        Self {
            reason: OpenGraphProjectSourceKind::Invariant,
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
        revision: GraphRevision,
    },
    #[error("graph-open operation is duplicated")]
    DuplicateOperation { operation_id: OperationId },
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
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("captured graph-open session changed")]
    SessionChanged,
    #[error(transparent)]
    Project(#[from] OpenGraphProjectError),
    #[error(transparent)]
    Database(#[from] crate::database::error::DatabaseError),
    #[error(transparent)]
    Contract(#[from] GraphContractMappingError),
    #[error(transparent)]
    Materialization(#[from] GraphMutationError),
    #[error(transparent)]
    Projection(#[from] EditorProjectionError),
}

#[derive(Clone, Debug)]
pub struct OpenGraphApplicationReceipt {
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    graph_revision: GraphRevision,
    document: Arc<GraphDocument>,
    analysis: GraphAnalysis,
    projection: EditorProjectionModel,
}

impl OpenGraphApplicationReceipt {
    fn new(
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        document: Arc<GraphDocument>,
        analysis: GraphAnalysis,
        projection: EditorProjectionModel,
    ) -> Self {
        Self {
            project_instance_id,
            graph_path,
            graph_revision: document.revision,
            document,
            analysis,
            projection,
        }
    }

    pub fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.project_instance_id
    }

    pub fn graph_path(&self) -> &GraphResourcePath {
        &self.graph_path
    }

    pub const fn graph_revision(&self) -> GraphRevision {
        self.graph_revision
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
}

impl ApplicationState {
    pub fn open_graph(
        &self,
        request: OpenGraphRequest,
    ) -> Result<OpenGraphApplicationReceipt, OpenGraphApplicationError> {
        let captured = self.capture_session()?;
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
        .get_data()
        .map_err(|error| {
            map_project_open_error(request.graph_path(), request.operation_id(), error)
        })?
        .graphs
        .contains_key(request.graph_path());
    if !already_resident {
        captured
            .project()
            .load_graph_projection(
                captured.project_instance_id(),
                request.graph_path(),
                request.lifecycle_token(),
                request.locale(),
            )
            .map_err(|error| {
                map_project_open_error(request.graph_path(), request.operation_id(), error)
            })?;
    }

    let data = captured.project().get_data().map_err(|error| {
        map_project_open_error(request.graph_path(), request.operation_id(), error)
    })?;
    let resource = data.graphs.get(request.graph_path()).ok_or_else(|| {
        OpenGraphApplicationError::Project(OpenGraphProjectError::PrepareFailed(
            OpenGraphProjectSource::invariant(),
        ))
    })?;
    let loaded_document = Arc::new(resource.document.clone());

    // Staged Graph boundary: the old Project load above remains the only
    // active lower-level load/commit operation. Graph owns the lock-free
    // binding and candidate materialization stage; the candidate is never
    // sent through the mutation-only graph commit primitive.
    captured.graph().bind_open_graph();
    let candidate_document = captured
        .graph()
        .materialize_open_candidate(&loaded_document)?;

    let project = capture_localized_project_facts(captured)
        .map_err(|error| map_project_facts_open_error(request.graph_path(), error))?;
    let database = catalog_snapshot(captured.database())?;
    revalidate_declaration_observations(
        captured.database(),
        project.resources().database_observations(),
    )?;
    let graph_catalog = build_resource_catalog(project.resources().graph(), &database)?;
    let settings = graph_compile_settings(&data.computation_settings);
    let registry_fingerprint = captured.graph().registry_fingerprint();
    let basis = PlanCompilationBasis::new(
        PlanProjectSessionId::from_existing(captured.project_session_id().as_str().into()),
        PlanGraphRevision::from_existing(candidate_document.revision.get()),
        PlanRegistryFingerprint::from_bytes(registry_fingerprint),
        Default::default(),
        Default::default(),
    );
    let analysis = captured
        .graph()
        .analyze(&candidate_document, &graph_catalog, &settings, &basis);

    // This is the final staged commit gate. A replacement that wins before
    // it suppresses the candidate; once it passes, the old Project load has
    // already linearized and the derived projection must not be relabeled by
    // a later session replacement.
    revalidate_project_catalog_facts(captured, &project)
        .map_err(|error| map_project_facts_open_error(request.graph_path(), error))?;
    revalidate_declaration_observations(
        captured.database(),
        project.resources().database_observations(),
    )?;
    revalidate_catalog_snapshot(captured.database(), &database)?;
    revalidate_application_session(application, captured)?;

    let projection = build_editor_projection(EditorProjectionInput {
        graph_path: request.graph_path(),
        document: &candidate_document,
        analysis: &analysis,
        registry_fingerprint,
    })?;
    Ok(OpenGraphApplicationReceipt::new(
        captured.project_instance_id().clone(),
        request.graph_path().clone(),
        candidate_document,
        analysis,
        projection,
    ))
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
    operation_id: OperationId,
    error: ProjectFilesystemError,
) -> OpenGraphApplicationError {
    match &error {
        ProjectFilesystemError::InvalidGraphDocument { path, .. } => {
            OpenGraphProjectError::InvalidGraphDocument {
                graph: path.clone(),
            }
            .into()
        }
        ProjectFilesystemError::StaleProjectLifecycle { .. } => {
            OpenGraphProjectError::StaleProjectAuthority {
                graph: graph.clone(),
            }
            .into()
        }
        ProjectFilesystemError::StaleResourceLifecycle { .. } => {
            OpenGraphProjectError::ResourceLifecycleChanged {
                graph: graph.clone(),
            }
            .into()
        }
        ProjectFilesystemError::ProjectLifecycleAdmissionClosed { .. } => {
            OpenGraphProjectError::AdmissionClosed.into()
        }
        ProjectFilesystemError::ProjectRecoveryRequired { .. } => {
            OpenGraphProjectError::RecoveryRequired.into()
        }
        ProjectFilesystemError::ResourceRevisionOverflow { retained, .. } => {
            OpenGraphProjectError::RevisionExhausted {
                graph: graph.clone(),
                revision: GraphRevision::new(*retained),
            }
            .into()
        }
        ProjectFilesystemError::DuplicateOperation { .. } => {
            OpenGraphProjectError::DuplicateOperation { operation_id }.into()
        }
        ProjectFilesystemError::FilesystemTransactionBusy { .. } => {
            OpenGraphProjectError::FilesystemBusy.into()
        }
        ProjectFilesystemError::TransactionCommitFailed { .. } => {
            OpenGraphProjectError::CommitFailed(OpenGraphProjectSource::filesystem(error)).into()
        }
        ProjectFilesystemError::TransactionRollbackFailed {
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
