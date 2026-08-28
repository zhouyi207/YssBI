use std::sync::Arc;

use thiserror::Error;

use super::session_slot::{ApplicationSession, ApplicationSessionEpoch};
use crate::database::runtime::DatabaseRuntimeSession;
use crate::execution::identity::{ExecutionSessionId, RuntimeGeneration};
use crate::execution::plan::PlanProjectSessionId;
use crate::execution::resource_preparation::ResourceProviderFactory;
use crate::execution::state::ExecutionRuntimeState;
use crate::graph::runtime_state::GraphRuntimeState;
use crate::node_system::ProjectSessionId;
use crate::project::{ProjectInstanceId, ProjectState};

/// Composition-root supplied construction for one session-bound resource factory.
///
/// The function pointer is deliberately the only state carried by this owner. It
/// cannot capture a registry, a latest session, or any other ambient state, and
/// the owner is consumed by the eventual composition root rather than stored in
/// an `ApplicationSession`.
pub(crate) struct SessionResourceFactoryBuilder(
    fn(Arc<DatabaseRuntimeSession>, PlanProjectSessionId) -> ResourceProviderFactory,
);

impl SessionResourceFactoryBuilder {
    pub(crate) fn from_composition(
        build: fn(Arc<DatabaseRuntimeSession>, PlanProjectSessionId) -> ResourceProviderFactory,
    ) -> Self {
        Self(build)
    }

    pub(super) fn build(
        &self,
        session: Arc<DatabaseRuntimeSession>,
        bound_project_session: PlanProjectSessionId,
    ) -> ResourceProviderFactory {
        (self.0)(session, bound_project_session)
    }
}

#[derive(Debug, Eq, PartialEq, Error)]
pub(super) enum ReplacementCandidateInputError {
    #[error("candidate Project and Plan session identities differ")]
    ProjectSessionIdentityMismatch,
    #[error("candidate Database session identity differs from the Plan session")]
    DatabaseSessionIdentityMismatch,
    #[error("candidate Graph epoch differs from the Application session epoch")]
    GraphEpochMismatch,
    #[error("candidate Execution session identity differs from the supplied identity")]
    ExecutionSessionIdentityMismatch,
    #[error("candidate Execution generation differs from the supplied generation")]
    ExecutionGenerationMismatch,
}

#[derive(Debug, Eq, PartialEq, Error)]
pub(super) enum SessionCandidateBuildError {
    #[error(transparent)]
    InvalidInput(#[from] ReplacementCandidateInputError),
}

/// All state needed to form a dormant replacement candidate.
///
/// The fields are private so callers cannot construct a live Application
/// session by assembling partially validated components. The only transition
/// out is `build_replacement_candidate`, which validates the complete identity
/// tuple before it invokes the composition-injected builder.
pub(super) struct ReplacementCandidateInput {
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
    pub(super) fn new(
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
            return Err(ReplacementCandidateInputError::ProjectSessionIdentityMismatch);
        }
        if self.database.identity().as_str() != self.bound_project_session.as_str() {
            return Err(ReplacementCandidateInputError::DatabaseSessionIdentityMismatch);
        }
        if self.graph.epoch().get() != self.epoch.get() {
            return Err(ReplacementCandidateInputError::GraphEpochMismatch);
        }
        if self.execution.session_id() != self.execution_session_id {
            return Err(ReplacementCandidateInputError::ExecutionSessionIdentityMismatch);
        }
        if self.execution.generation() != self.runtime_generation {
            return Err(ReplacementCandidateInputError::ExecutionGenerationMismatch);
        }
        Ok(())
    }
}

/// A fully validated but not-yet-published Application session.
///
/// Dropping this value releases only dormant component owners. It has no public
/// admission or publication operation; the composition root must consume it
/// through the Application session-slot installation seam.
pub(crate) struct UnpublishedApplicationSession {
    session: ApplicationSession,
}

impl UnpublishedApplicationSession {
    pub(super) fn into_session(self) -> ApplicationSession {
        self.session
    }
}

pub(super) fn build_replacement_candidate(
    builder: &SessionResourceFactoryBuilder,
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
    let resource_provider_factory =
        Arc::new(builder.build(Arc::clone(&database), bound_project_session));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::runtime::DatabaseRuntimeRegistry;
    use crate::database_contract::{
        DatabaseDecl, DatabaseDeclarationObservationSet, DatabaseSessionIdentity,
        DatabaseSessionOpenRequest,
    };
    use crate::execution::identity::{ExecutionSessionId, RuntimeGeneration};
    use crate::execution::plan::PlanProjectSessionId;
    use crate::execution::resource_preparation::ResourceProviderFactory;
    use crate::execution::state::ExecutionRuntimeState;
    use crate::graph::resource_catalog::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};
    use crate::graph::runtime_state::{
        GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState,
    };
    use crate::node_system::ProjectSessionId;
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::compiler::ProjectCompileCoordinator;
    use crate::project::{ProjectInstanceId, ProjectState};
    use std::collections::BTreeMap;
    use std::num::NonZeroU64;
    use std::sync::Arc;

    fn test_factory(
        _database: Arc<crate::database::runtime::DatabaseRuntimeSession>,
        project_session: PlanProjectSessionId,
    ) -> ResourceProviderFactory {
        ResourceProviderFactory::new(project_session.as_str().into())
    }

    fn candidate_input() -> ReplacementCandidateInput {
        let project_session_id = ProjectSessionId::new("candidate-session");
        let graph_components = build_builtin_node_system().expect("test built-ins are valid");
        let graph = Arc::new(GraphRuntimeState::from_components(
            GraphRuntimeEpoch::from_existing(7),
            GraphRuntimeComponents {
                registry: graph_components.registry,
                catalog: graph_components.catalog,
                compiler: Arc::new(ProjectCompileCoordinator::new()),
                resource_catalog: Arc::new(ResourceCatalogSnapshot::new(
                    BTreeMap::new(),
                    BTreeMap::new(),
                    BTreeMap::new(),
                    ResourceCatalogFingerprint::from_bytes([7; 32]),
                )),
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
        let builder = SessionResourceFactoryBuilder::from_composition(test_factory);
        let unpublished = build_replacement_candidate(&builder, candidate_input())
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
        match build_replacement_candidate(&builder, mismatched) {
            Err(error) => assert_eq!(
                error,
                SessionCandidateBuildError::InvalidInput(
                    ReplacementCandidateInputError::ProjectSessionIdentityMismatch
                )
            ),
            Ok(_) => panic!("mismatched candidate must be rejected"),
        }
    }
}
