//! Staged application sessions and graph document helpers shared by graph use-case tests.
use crate::session::{
    ApplicationSession, ApplicationSessionEpoch, ApplicationSessionSlot, ApplicationState,
};
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::Arc;
use yss_database_contract::{
    DatabaseDecl, DatabaseDeclarationObservation, DatabaseDeclarationObservationSet, DatabaseId,
    DatabaseSessionIdentity, DatabaseSessionOpenRequest,
};
use yss_database_runtime::runtime::DatabaseRuntimeRegistry;
use yss_graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, GraphDocument, NodeId, NodePosition,
    ParameterValues, PortAddress,
};
use yss_graph_execution::identity::{ExecutionSessionId, RuntimeGeneration};
use yss_graph_execution::resource_preparation::ResourceProviderFactory;
use yss_graph_execution::state::ExecutionRuntimeState;
use yss_graph_runtime::{
    GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState, GraphRuntimeTestControl,
};
use yss_node_catalog::build_builtin_node_system;
use yss_project::ProjectState;
use yss_project_identity::ProjectSessionId;
use yss_project_model::ProjectData;

pub(crate) struct TestProject {
    pub(crate) root: PathBuf,
    pub(crate) state: Arc<ProjectState>,
}

impl TestProject {
    pub(crate) fn active(label: &str, data: ProjectData) -> Self {
        let root = test_root(label);
        yss_project::fixtures::write_project(&data, root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), data);
        Self {
            root,
            state: Arc::new(state),
        }
    }

    pub(crate) fn unloaded(label: &str, data: ProjectData) -> Self {
        let root = test_root(label);
        yss_project::fixtures::write_project(&data, root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_from_path(&root).unwrap();
        Self {
            root,
            state: Arc::new(state),
        }
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn test_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("yssbi-graph-{label}-{}", uuid::Uuid::new_v4()))
}

pub(crate) struct StagedSession {
    pub(crate) project: TestProject,
    pub(crate) session: Arc<ApplicationSession>,
    pub(crate) application: ApplicationState,
    pub(crate) slot: Arc<ApplicationSessionSlot>,
    pub(crate) control: GraphRuntimeTestControl,
}

pub(crate) fn staged_session(
    project: TestProject,
    control: GraphRuntimeTestControl,
) -> StagedSession {
    let project_state = Arc::clone(&project.state);
    let project_instance_id = project_state.capture_project_session().unwrap().instance_id;
    let project_session_id =
        ProjectSessionId::new(format!("graph-test-project-{}", uuid::Uuid::new_v4()));
    let execution_session_id = ExecutionSessionId::new(uuid::Uuid::new_v4());
    let builtin = build_builtin_node_system().unwrap();
    let graph = Arc::new(GraphRuntimeState::new_for_test(
        GraphRuntimeEpoch::from_existing(1),
        GraphRuntimeComponents {
            registry: builtin.registry,
            catalog: builtin.catalog,
        },
        control.clone(),
    ));
    let declarations: Arc<[DatabaseDecl]> = Vec::new().into();
    let observations = DatabaseDeclarationObservationSet::try_from_iter(std::iter::empty::<(
        DatabaseId,
        DatabaseDeclarationObservation,
    )>())
    .unwrap();
    let database = Arc::new(
        DatabaseRuntimeRegistry::new()
            .open_session(DatabaseSessionOpenRequest::new(
                DatabaseSessionIdentity::from_existing(project_session_id.as_str().into()),
                NonZeroU64::new(1).unwrap(),
                declarations,
                observations,
            ))
            .unwrap(),
    );
    let execution = Arc::new(ExecutionRuntimeState::new(
        execution_session_id,
        RuntimeGeneration::from_existing(1),
        yss_node_kernel::KernelRegistry::default().into(),
        yss_database_runtime::dataset_query_engine().unwrap(),
    ));
    let session = Arc::new(ApplicationSession::new_for_test(
        ApplicationSessionEpoch::from_existing(1),
        project_instance_id,
        project_session_id.clone(),
        execution_session_id,
        RuntimeGeneration::from_existing(1),
        project_state,
        graph,
        execution,
        database,
        Arc::new(ResourceProviderFactory::new(
            project_session_id.as_str().into(),
        )),
    ));
    let slot = Arc::new(ApplicationSessionSlot::new(
        crate::session::NodeComponents::builtins().unwrap(),
    ));
    slot.publish_for_test(Arc::clone(&session));
    let application = ApplicationState::new(Arc::clone(&slot));
    StagedSession {
        project,
        session,
        application,
        slot,
        control,
    }
}

pub(crate) fn node(document: &mut GraphDocument, kind: &str) -> NodeId {
    let id = NodeId::new();
    document.nodes.insert(
        id,
        DocumentNode {
            id,
            node_type: kind.parse().unwrap(),
            position: NodePosition { x: 0., y: 0. },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    id
}

pub(crate) fn port(node: NodeId, key: &str) -> PortAddress {
    PortAddress::declared(node, key.parse().unwrap())
}

pub(crate) fn connect(document: &mut GraphDocument, output: PortAddress, input: PortAddress) {
    let id = ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output,
            input,
            order: None,
        },
    );
}
