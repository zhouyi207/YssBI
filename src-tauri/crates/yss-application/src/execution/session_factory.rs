use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use thiserror::Error;

use super::session_slot::{ApplicationSession, ApplicationSessionEpoch};
use yss_database_contract::{
    DatabaseDeclarationFingerprint, DatabaseDeclarationObservation,
    DatabaseDeclarationObservationSet, DatabaseDeclarationRevision,
};
use yss_database_runtime::runtime::DatabaseRuntimeSession;
use yss_database_runtime::{DatabaseInstance, DatabaseState, bind_duckdb_instance};
use yss_execution::identity::{ExecutionSessionId, RuntimeGeneration};
use yss_execution::plan::PlanProjectSessionId;
use yss_execution::ports::scientific::ScientificBackend;
use yss_execution::resource_preparation::ResourceProviderFactory;
use yss_execution::state::ExecutionRuntimeState;
use yss_graph_catalog::build_builtin_node_system;
use yss_graph_runtime::{GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState};
use yss_project::ProjectState;
use yss_project_filesystem::ProjectFilesystemError;
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::ProjectSessionId;

#[derive(Debug, Eq, PartialEq, Error)]
pub(crate) enum ReplacementCandidateInputError {
    #[error("candidate Project and Plan session identities differ")]
    ProjectSessionIdentity,
    #[error("candidate Database session identity differs from the Plan session")]
    DatabaseSessionIdentity,
    #[error("candidate Graph epoch differs from the Application session epoch")]
    GraphEpoch,
    #[error("candidate Execution session identity differs from the supplied identity")]
    ExecutionSessionIdentity,
    #[error("candidate Execution generation differs from the supplied generation")]
    ExecutionGeneration,
}

#[derive(Debug, Eq, PartialEq, Error)]
pub(crate) enum SessionCandidateBuildError {
    #[error(transparent)]
    InvalidInput(#[from] ReplacementCandidateInputError),
}

/// All state needed to form a dormant replacement candidate.
///
/// The fields are private so callers cannot construct a live Application
/// session by assembling partially validated components. The only transition
/// out is `build_replacement_candidate`, which validates the complete identity
/// tuple before it invokes the composition-injected builder.
pub(crate) struct ReplacementCandidateInput {
    epoch: ApplicationSessionEpoch,
    project_instance_id: ProjectInstanceId,
    project_session_id: ProjectSessionId,
    bound_project_session: PlanProjectSessionId,
    execution_session_id: ExecutionSessionId,
    runtime_generation: RuntimeGeneration,
    project: Arc<ProjectState>,
    graph: Arc<GraphRuntimeState>,
    execution: Arc<ExecutionRuntimeState>,
    database: Arc<DatabaseRuntimeSession>,
}

impl ReplacementCandidateInput {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        epoch: ApplicationSessionEpoch,
        project_instance_id: ProjectInstanceId,
        project_session_id: ProjectSessionId,
        bound_project_session: PlanProjectSessionId,
        execution_session_id: ExecutionSessionId,
        runtime_generation: RuntimeGeneration,
        project: Arc<ProjectState>,
        graph: Arc<GraphRuntimeState>,
        execution: Arc<ExecutionRuntimeState>,
        database: Arc<DatabaseRuntimeSession>,
    ) -> Self {
        Self {
            epoch,
            project_instance_id,
            project_session_id,
            bound_project_session,
            execution_session_id,
            runtime_generation,
            project,
            graph,
            execution,
            database,
        }
    }

    fn validate(&self) -> Result<(), ReplacementCandidateInputError> {
        if self.project_session_id.as_str() != self.bound_project_session.as_str() {
            return Err(ReplacementCandidateInputError::ProjectSessionIdentity);
        }
        if self.database.identity().as_str() != self.bound_project_session.as_str() {
            return Err(ReplacementCandidateInputError::DatabaseSessionIdentity);
        }
        if self.graph.epoch().get() != self.epoch.get() {
            return Err(ReplacementCandidateInputError::GraphEpoch);
        }
        if self.execution.session_id() != self.execution_session_id {
            return Err(ReplacementCandidateInputError::ExecutionSessionIdentity);
        }
        if self.execution.generation() != self.runtime_generation {
            return Err(ReplacementCandidateInputError::ExecutionGeneration);
        }
        Ok(())
    }
}

/// A fully validated but not-yet-published Application session.
///
/// Dropping this value releases only dormant component owners. It has no public
/// admission or publication operation; the composition root must consume it
/// through the Application session-slot installation seam.
pub struct UnpublishedApplicationSession {
    session: ApplicationSession,
}

#[derive(Debug, Error)]
#[error("database session could not be opened for the application session")]
pub struct DatabaseSessionCandidateError {
    #[source]
    source: super::super::database_session::DatabaseSessionApplicationError,
}

impl From<super::super::database_session::DatabaseSessionApplicationError>
    for DatabaseSessionCandidateError
{
    fn from(source: super::super::database_session::DatabaseSessionApplicationError) -> Self {
        Self { source }
    }
}

#[derive(Debug, Error)]
#[error("application session candidate is internally inconsistent")]
pub struct InvalidSessionCandidateError {
    #[source]
    source: SessionCandidateBuildError,
}

impl From<SessionCandidateBuildError> for InvalidSessionCandidateError {
    fn from(source: SessionCandidateBuildError) -> Self {
        Self { source }
    }
}

#[derive(Debug, Error)]
pub enum ProjectSessionCandidateError {
    #[error("project snapshot could not be captured for the application session")]
    ProjectSnapshot(#[source] ProjectFilesystemError),
    #[error("project database declaration observations could not be captured")]
    DatabaseObservations(#[source] yss_database_contract::DatabaseDeclarationObservationSetError),
    #[error("database session could not be opened for the application session")]
    DatabaseSession(#[source] DatabaseSessionCandidateError),
    #[error("application session generation is exhausted")]
    GenerationExhausted,
    #[error("graph runtime built-ins could not be constructed")]
    GraphRuntime(#[source] yss_graph_catalog::BuiltinInitializationError),
    #[error(transparent)]
    Candidate(#[from] InvalidSessionCandidateError),
}

impl UnpublishedApplicationSession {
    pub(super) fn into_session(self) -> ApplicationSession {
        self.session
    }
}

pub(crate) fn build_replacement_candidate(
    input: ReplacementCandidateInput,
) -> Result<UnpublishedApplicationSession, SessionCandidateBuildError> {
    input.validate()?;
    let ReplacementCandidateInput {
        epoch,
        project_instance_id,
        project_session_id,
        bound_project_session,
        execution_session_id,
        runtime_generation,
        project,
        graph,
        execution,
        database,
    } = input;
    let resource_provider_factory = Arc::new(ResourceProviderFactory::from_project_session(
        bound_project_session,
    ));
    Ok(UnpublishedApplicationSession {
        session: ApplicationSession::from_candidate(
            epoch,
            project_instance_id,
            project_session_id,
            execution_session_id,
            runtime_generation,
            project,
            graph,
            execution,
            database,
            resource_provider_factory,
        ),
    })
}

pub fn build_current_project_candidate(
    epoch: ApplicationSessionEpoch,
    project: Arc<ProjectState>,
    reusable_instances: impl IntoIterator<Item = DatabaseInstance>,
    scientific_backend: Arc<dyn ScientificBackend>,
) -> Result<UnpublishedApplicationSession, ProjectSessionCandidateError> {
    let data = project
        .get_data()
        .map_err(ProjectSessionCandidateError::ProjectSnapshot)?;
    let project_instance_id = ProjectInstanceId::from_existing(project.project_instance_id());
    let project_session_id = project.project_session_id();
    let builtin =
        build_builtin_node_system().map_err(ProjectSessionCandidateError::GraphRuntime)?;
    let (root, database_revisions) = match project.get_path() {
        Some(_) => {
            let session = project
                .capture_project_session()
                .map_err(ProjectSessionCandidateError::ProjectSnapshot)?;
            let index = project
                .read_project_index(&session.instance_id)
                .map_err(ProjectSessionCandidateError::ProjectSnapshot)?;
            let revisions = index
                .databases
                .into_iter()
                .map(|entry| (entry.id, entry.revision.get()))
                .collect::<BTreeMap<_, _>>();
            (Some(session.root), revisions)
        }
        None => (None, BTreeMap::new()),
    };
    let reusable_instances = reusable_instances
        .into_iter()
        .map(|instance| (instance.decl.id.clone(), instance))
        .collect::<BTreeMap<_, _>>();
    let database_instances = data
        .databases
        .values()
        .map(|declaration| {
            reusable_instances
                .get(&declaration.id)
                .filter(|instance| instance.decl == *declaration)
                .cloned()
                .unwrap_or_else(|| match &declaration.engine {
                    yss_database_contract::DatabaseEngine::DuckDb { .. } => {
                        bind_duckdb_instance(declaration, root.as_ref().map(|root| root.as_path()))
                    }
                    _ => DatabaseInstance {
                        decl: declaration.clone(),
                        state: DatabaseState::Failed {
                            error: "Only DuckDb datasets are supported; re-import the data".into(),
                        },
                    },
                })
        })
        .collect::<Vec<_>>();
    let declarations = data
        .databases
        .values()
        .cloned()
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let observations = DatabaseDeclarationObservationSet::try_from_iter(
        data.databases.values().map(|declaration| {
            (
                declaration.id.clone(),
                DatabaseDeclarationObservation::new(
                    DatabaseDeclarationRevision::from_existing(
                        database_revisions
                            .get(declaration.id.as_str())
                            .copied()
                            .unwrap_or(0),
                    ),
                    DatabaseDeclarationFingerprint::from_decl(declaration),
                ),
            )
        }),
    )
    .map_err(ProjectSessionCandidateError::DatabaseObservations)?;
    let database_facts = super::super::database_session::ProjectDatabaseSessionFacts::new(
        project_session_id.clone(),
        NonZeroU64::new(epoch.get().saturating_add(1))
            .ok_or(ProjectSessionCandidateError::GenerationExhausted)?,
        root,
        declarations.into(),
        observations,
    );
    let database = super::super::database_session::prepare_database_session_with_instances(
        &database_facts,
        database_instances,
    )
    .map_err(|source| ProjectSessionCandidateError::DatabaseSession(source.into()))?;
    let graph = Arc::new(GraphRuntimeState::from_components(
        GraphRuntimeEpoch::from_existing(epoch.get()),
        GraphRuntimeComponents {
            registry: builtin.registry,
            catalog: builtin.catalog,
        },
    ));
    let execution_session_id = ExecutionSessionId::new(uuid::Uuid::new_v4());
    let runtime_generation = RuntimeGeneration::from_existing(epoch.get().saturating_add(1));
    let execution = Arc::new(ExecutionRuntimeState::from_composition(
        execution_session_id,
        runtime_generation,
        scientific_backend,
    ));
    let bound_project_session =
        PlanProjectSessionId::from_existing(project_session_id.as_str().into());
    build_replacement_candidate(ReplacementCandidateInput::new(
        epoch,
        project_instance_id,
        project_session_id,
        bound_project_session,
        execution_session_id,
        runtime_generation,
        project,
        graph,
        execution,
        database,
    ))
    .map_err(|source| ProjectSessionCandidateError::Candidate(source.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU64;
    use std::sync::Arc;
    use yss_database_contract::{
        DatabaseDecl, DatabaseDeclarationObservationSet, DatabaseSessionIdentity,
        DatabaseSessionOpenRequest,
    };
    use yss_database_runtime::runtime::DatabaseRuntimeRegistry;
    use yss_execution::identity::{ExecutionSessionId, RuntimeGeneration};
    use yss_execution::plan::PlanProjectSessionId;
    use yss_execution::state::ExecutionRuntimeState;
    use yss_graph_catalog::build_builtin_node_system;
    use yss_project::ProjectState;
    use yss_project_identity::ProjectInstanceId;
    use yss_project_identity::ProjectSessionId;

    fn candidate_input() -> ReplacementCandidateInput {
        let project_session_id = ProjectSessionId::new("candidate-session");
        let graph_components = build_builtin_node_system().expect("test built-ins are valid");
        let graph = Arc::new(GraphRuntimeState::from_components(
            GraphRuntimeEpoch::from_existing(7),
            GraphRuntimeComponents {
                registry: graph_components.registry,
                catalog: graph_components.catalog,
            },
        ));
        let observations = DatabaseDeclarationObservationSet::try_from_iter(std::iter::empty())
            .expect("empty observation set is valid");
        let database = Arc::new(
            DatabaseRuntimeRegistry::new()
                .open_session(DatabaseSessionOpenRequest::new(
                    DatabaseSessionIdentity::from_existing("candidate-session".into()),
                    NonZeroU64::new(1).expect("test generation is non-zero"),
                    None,
                    Vec::<DatabaseDecl>::new().into(),
                    observations,
                ))
                .expect("empty database session is valid"),
        );
        let execution_session_id = ExecutionSessionId::new(uuid::Uuid::from_u128(7));
        let runtime_generation = RuntimeGeneration::from_existing(7);
        ReplacementCandidateInput::new(
            ApplicationSessionEpoch::from_existing(7),
            ProjectInstanceId::from_existing("candidate-project".into()),
            project_session_id,
            PlanProjectSessionId::from_existing("candidate-session".into()),
            execution_session_id,
            runtime_generation,
            Arc::new(ProjectState::new()),
            graph,
            Arc::new(ExecutionRuntimeState::new(
                execution_session_id,
                runtime_generation,
            )),
            database,
        )
    }

    #[test]
    fn candidate_build_binds_one_session_identity_and_rejects_mismatch() {
        let unpublished = build_replacement_candidate(candidate_input())
            .expect("matching candidate is buildable");
        let session = unpublished.into_session();
        assert_eq!(session.project_session_id().as_str(), "candidate-session");
        assert_eq!(
            session.database().identity().as_str(),
            session.project_session_id().as_str()
        );
        assert_eq!(session.graph().epoch().get(), session.epoch().get());
        assert_eq!(
            session.execution_session_id(),
            session.execution().session_id()
        );
        assert_eq!(
            session.runtime_generation(),
            session.execution().generation()
        );
        let _factory = session.resource_provider_factory();

        let mut mismatched = candidate_input();
        mismatched.bound_project_session =
            PlanProjectSessionId::from_existing("other-session".into());
        match build_replacement_candidate(mismatched) {
            Err(error) => assert_eq!(
                error,
                SessionCandidateBuildError::InvalidInput(
                    ReplacementCandidateInputError::ProjectSessionIdentity
                )
            ),
            Ok(_) => panic!("mismatched candidate must be rejected"),
        }
    }
}
