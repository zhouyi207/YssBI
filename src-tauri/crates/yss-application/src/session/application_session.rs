use std::sync::Arc;

use yss_database_runtime::runtime::DatabaseRuntimeSession;
use yss_graph_execution::identity::{ExecutionSessionId, RuntimeGeneration};
use yss_graph_execution::resource_preparation::ResourceProviderFactory;
use yss_graph_execution::state::ExecutionRuntimeState;
use yss_graph_runtime::GraphRuntimeState;
use yss_project::ProjectState;
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ApplicationSessionEpoch(u64);

impl ApplicationSessionEpoch {
    pub const INITIAL: Self = Self(0);

    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub(super) fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

pub struct ApplicationSession {
    epoch: ApplicationSessionEpoch,
    project_instance_id: ProjectInstanceId,
    project_session_id: ProjectSessionId,
    execution_session_id: ExecutionSessionId,
    runtime_generation: RuntimeGeneration,
    project: Arc<ProjectState>,
    graph: Arc<GraphRuntimeState>,
    execution: Arc<ExecutionRuntimeState>,
    database: Arc<DatabaseRuntimeSession>,
    resource_provider_factory: Arc<ResourceProviderFactory>,
    graph_activity: crate::graph::editing::GraphActivitySource,
    graph_editing: crate::graph::editing::GraphEditingCoordinator,
    pub(crate) presentation: crate::presentation::PresentationSession,
}

impl ApplicationSession {
    #[allow(
        clippy::too_many_arguments,
        reason = "a session candidate must publish one complete cross-authority identity envelope"
    )]
    pub(super) fn from_candidate(
        epoch: ApplicationSessionEpoch,
        project_instance_id: ProjectInstanceId,
        project_session_id: ProjectSessionId,
        execution_session_id: ExecutionSessionId,
        runtime_generation: RuntimeGeneration,
        project: Arc<ProjectState>,
        graph: Arc<GraphRuntimeState>,
        execution: Arc<ExecutionRuntimeState>,
        database: Arc<DatabaseRuntimeSession>,
        resource_provider_factory: Arc<ResourceProviderFactory>,
    ) -> Self {
        Self {
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
            graph_activity: crate::graph::editing::GraphActivitySource::default(),
            graph_editing: crate::graph::editing::GraphEditingCoordinator::default(),
            presentation: crate::presentation::PresentationSession::default(),
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    #[allow(
        clippy::too_many_arguments,
        reason = "tests must construct the same complete session identity envelope as production"
    )]
    pub fn new_for_test(
        epoch: ApplicationSessionEpoch,
        project_instance_id: ProjectInstanceId,
        project_session_id: ProjectSessionId,
        execution_session_id: ExecutionSessionId,
        runtime_generation: RuntimeGeneration,
        project: Arc<ProjectState>,
        graph: Arc<GraphRuntimeState>,
        execution: Arc<ExecutionRuntimeState>,
        database: Arc<DatabaseRuntimeSession>,
        resource_provider_factory: Arc<ResourceProviderFactory>,
    ) -> Self {
        Self::from_candidate(
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
        )
    }

    pub(crate) fn project(&self) -> &ProjectState {
        &self.project
    }

    pub(crate) fn publish_graph_activity(&self, activity: crate::graph::editing::GraphActivity) {
        self.graph_activity.publish(activity);
    }

    pub(crate) fn execution_snapshot(&self) -> Vec<crate::graph::run::RunApplicationEvent> {
        self.graph_activity.execution_snapshot()
    }

    pub(crate) fn coordinate_graph_edit(
        &self,
        path: &yss_graph_document::GraphResourcePath,
    ) -> Result<crate::graph::editing::GraphEditingPermit, crate::graph::editing::GraphEditingBusy>
    {
        self.graph_editing.acquire(path)
    }

    pub(crate) fn subscribe_graph_activity(
        &self,
        observer: crate::graph::editing::GraphActivityObserver,
    ) -> Result<
        crate::graph::editing::GraphActivitySubscription,
        crate::graph::resources::ResourceMutationApplicationError,
    > {
        self.graph_activity.subscribe(observer)
    }

    pub(crate) fn graph(&self) -> &GraphRuntimeState {
        &self.graph
    }

    pub(crate) fn execution(&self) -> &ExecutionRuntimeState {
        &self.execution
    }

    pub(crate) fn database(&self) -> &DatabaseRuntimeSession {
        &self.database
    }

    pub(crate) fn resource_provider_factory(&self) -> &ResourceProviderFactory {
        &self.resource_provider_factory
    }

    pub fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.project_instance_id
    }

    pub fn project_session_id(&self) -> &ProjectSessionId {
        &self.project_session_id
    }

    pub fn execution_session_id(&self) -> ExecutionSessionId {
        self.execution_session_id
    }

    pub fn runtime_generation(&self) -> RuntimeGeneration {
        self.runtime_generation
    }

    pub fn epoch(&self) -> ApplicationSessionEpoch {
        self.epoch
    }
}
