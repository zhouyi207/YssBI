use super::*;

use crate::session::{ApplicationSession, ApplicationSessionEpoch, ApplicationSessionSlot};
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread;
use yss_database_contract::{
    DatabaseDecl, DatabaseDeclarationObservation, DatabaseDeclarationObservationSet, DatabaseId,
    DatabaseSessionIdentity, DatabaseSessionOpenRequest,
};
use yss_database_runtime::runtime::DatabaseRuntimeRegistry;
use yss_graph_document::GraphResourceKind;
use yss_graph_execution::identity::{ExecutionSessionId, RuntimeGeneration};
use yss_graph_execution::resource_preparation::ResourceProviderFactory;
use yss_graph_execution::state::ExecutionRuntimeState;
use yss_graph_runtime::{
    GraphMaterializationError, GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState,
    GraphRuntimeTestControl, GraphRuntimeTestEvent,
};
use yss_node_catalog::build_builtin_node_system;
use yss_project::ProjectState;
use yss_project_identity::ProjectSessionId;
use yss_project_model::{GraphResourceDocument, ProjectData};

struct TestProject {
    root: PathBuf,
    state: Arc<ProjectState>,
}

impl TestProject {
    fn active(label: &str, data: ProjectData) -> Self {
        let root = test_root(label);
        yss_project::fixtures::write_project(&data, root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), data);
        Self {
            root,
            state: Arc::new(state),
        }
    }

    fn unloaded(label: &str, data: ProjectData) -> Self {
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

struct StagedSession {
    _project: TestProject,
    session: Arc<ApplicationSession>,
    application: ApplicationState,
    slot: Arc<ApplicationSessionSlot>,
    control: GraphRuntimeTestControl,
}

fn test_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("yssbi-graph-open-{label}-{}", uuid::Uuid::new_v4()))
}

fn staged_session(project: TestProject, control: GraphRuntimeTestControl) -> StagedSession {
    let project_state = Arc::clone(&project.state);
    let project_instance_id = project_state.capture_project_session().unwrap().instance_id;
    let project_session_id =
        ProjectSessionId::new(format!("graph-open-project-{}", uuid::Uuid::new_v4()));
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
        _project: project,
        session,
        application,
        slot,
        control,
    }
}

fn graph_project(path: &GraphResourcePath) -> ProjectData {
    let mut data = ProjectData::new();
    let name = path
        .display_name()
        .split_once(".yssbi-")
        .map_or(path.display_name(), |(name, _)| name);
    data.graphs.insert(
        path.clone(),
        GraphResourceDocument::new(name, GraphResourceKind::EventGraph),
    );
    data
}

fn open_request(session: &StagedSession, path: &GraphResourcePath) -> OpenGraphRequest {
    OpenGraphRequest::new(
        session.session.project_instance_id().clone(),
        path.clone(),
        1,
        "en-US",
    )
}

#[test]
fn open_graph_preserves_referenced_declared_orphans_for_repair() {
    use yss_graph_document::{
        ConnectionId, DocumentConnection, DocumentNode, InputState, NodeId, NodePosition,
        PortAddress,
    };

    let path = GraphResourcePath::new("events/DeclaredOrphan.yssbi-event").unwrap();
    let mut project = graph_project(&path);
    let document = Arc::make_mut(&mut project.graphs.get_mut(&path).unwrap().document);
    let source = NodeId::new();
    let target = NodeId::new();
    for (id, x) in [(source, 0.0), (target, 200.0)] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: "yssbi.logic.not".parse().unwrap(),
                position: NodePosition { x, y: 0.0 },
                parameters: Default::default(),
                user_label: None,
            },
        );
    }
    let missing = PortAddress::declared(target, "missing_input".parse().unwrap());
    let connection = ConnectionId::new();
    document.connections.insert(
        connection,
        DocumentConnection {
            id: connection,
            output: PortAddress::declared(source, "result".parse().unwrap()),
            input: missing.clone(),
            order: None,
        },
    );
    let missing_literal = PortAddress::declared(target, "missing_literal".parse().unwrap());
    document.input_states.insert(
        missing_literal.clone(),
        InputState {
            literal_override: Some(yss_node_protocol::TypedValue {
                value_type: yss_node_protocol::TypeExpr::Concrete("core.binary".parse().unwrap()),
                value: yss_data_contract::DataValue::Bool(false),
            }),
        },
    );
    let session = staged_session(
        TestProject::unloaded("declared-orphan", project),
        GraphRuntimeTestControl::default(),
    );

    let receipt = session
        .application
        .open_graph(open_request(&session, &path))
        .expect("a structural graph with semantic problems remains openable for repair");
    for address in [&missing, &missing_literal] {
        let port = receipt
            .projection()
            .nodes
            .iter()
            .flat_map(|node| &node.ports)
            .find(|port| &port.address == address)
            .expect("the referenced missing port remains visible in the read projection");
        assert!(port.orphan);
        assert!(!port.connections.can_append);
    }
    assert!(receipt.projection().diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_ref() == "graph.port.unknown" && diagnostic.blocking
    }));
    assert!(receipt.analysis().semantic_snapshot().ready().is_none());
    assert_eq!(receipt.document().connections[&connection].input, missing);
    assert_eq!(
        receipt.document().input_states[&missing_literal]
            .literal_override
            .as_ref()
            .unwrap()
            .value,
        yss_data_contract::DataValue::Bool(false)
    );

    let request = |version| crate::graph::editing::GraphEditRequest {
        project_instance_id: session.session.project_instance_id().clone(),
        graph_path: path.clone(),
        version,
        operation_id: yss_project_identity::OperationId::new(),
        locale: "en-US".into(),
    };
    let cleared = session
        .application
        .edit_graph(
            request(receipt.editing().version),
            yss_graph_editor::EditorGraphMutation::SetLiteral {
                address: missing_literal.clone(),
                literal: None,
            },
        )
        .expect("an existing invalid literal can be removed without a current port declaration");
    assert!(
        !cleared
            .update
            .document
            .input_states
            .contains_key(&missing_literal)
    );
    let reopened = session
        .application
        .open_graph(open_request(&session, &path))
        .unwrap();
    assert!(
        !reopened
            .projection()
            .nodes
            .iter()
            .flat_map(|node| &node.ports)
            .any(|port| port.address == missing_literal)
    );
    assert!(
        session
            .application
            .edit_graph(
                request(cleared.editing.version),
                yss_graph_editor::EditorGraphMutation::SetLiteral {
                    address: missing_literal.clone(),
                    literal: Some(serde_json::json!(false)),
                },
            )
            .is_err()
    );
    let restored = session
        .application
        .change_graph_history(request(cleared.editing.version), false)
        .expect("undo restores the exact invalid literal as authored content");
    assert_eq!(
        restored.update.document.input_states[&missing_literal],
        receipt.document().input_states[&missing_literal]
    );
}

#[test]
fn open_graph_allows_removing_unavailable_nodes_with_exact_history() {
    use yss_graph_document::{
        ConnectionId, DocumentConnection, DocumentNode, InputState, NodeId, NodePosition,
        PortAddress,
    };
    let path = GraphResourcePath::new("events/UnavailableNode.yssbi-event").unwrap();
    let mut project = graph_project(&path);
    let document = Arc::make_mut(&mut project.graphs.get_mut(&path).unwrap().document);
    let unavailable = NodeId::new();
    let survivor = NodeId::new();
    for (id, node_type) in [
        (unavailable, "tests.unavailable.node"),
        (survivor, "yssbi.logic.not"),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: node_type.parse().unwrap(),
                position: NodePosition { x: 200.0, y: 10.0 },
                parameters: Default::default(),
                user_label: Some("authored label".into()),
            },
        );
    }
    document
        .nodes
        .get_mut(&unavailable)
        .unwrap()
        .parameters
        .insert(
            "setting".parse().unwrap(),
            serde_json::json!({"nested": [1, 2]}),
        );
    let connection = ConnectionId::new();
    document.connections.insert(
        connection,
        DocumentConnection {
            id: connection,
            output: PortAddress::declared(unavailable, "output".parse().unwrap()),
            input: PortAddress::declared(survivor, "input".parse().unwrap()),
            order: None,
        },
    );
    document.input_states.insert(
        PortAddress::declared(unavailable, "input".parse().unwrap()),
        InputState {
            literal_override: Some(yss_node_protocol::TypedValue {
                value_type: yss_node_protocol::TypeExpr::Concrete("core.binary".parse().unwrap()),
                value: yss_data_contract::DataValue::Bool(true),
            }),
        },
    );
    let session = staged_session(
        TestProject::unloaded("unavailable-node-delete", project),
        GraphRuntimeTestControl::default(),
    );
    let receipt = session
        .application
        .open_graph(open_request(&session, &path))
        .unwrap();
    assert!(receipt.projection().diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_ref() == "graph.node.unknown" && diagnostic.blocking
    }));
    let request = |version| crate::graph::editing::GraphEditRequest {
        project_instance_id: session.session.project_instance_id().clone(),
        graph_path: path.clone(),
        version,
        operation_id: yss_project_identity::OperationId::new(),
        locale: "en-US".into(),
    };
    let removed = session
        .application
        .edit_graph(
            request(receipt.editing().version),
            yss_graph_editor::EditorGraphMutation::DeleteNodes {
                node_ids: vec![unavailable],
            },
        )
        .expect("an unavailable node can be removed with its authored input state and connections");
    assert_eq!(removed.update.document.nodes.len(), 1);
    assert!(removed.update.document.nodes.contains_key(&survivor));
    assert!(removed.update.document.connections.is_empty());
    assert!(removed.update.document.input_states.is_empty());
    let restored = session
        .application
        .change_graph_history(request(removed.editing.version), false)
        .unwrap();
    assert_eq!(restored.update.document, *receipt.document());
    let redone = session
        .application
        .change_graph_history(request(restored.editing.version), true)
        .unwrap();
    assert_eq!(redone.update.document, removed.update.document);

    let mut guarded = receipt.document().as_ref().clone();
    guarded.nodes.get_mut(&survivor).unwrap().node_type =
        "yssbi.project.function.entry".parse().unwrap();
    assert!(matches!(
        yss_graph_editor::EditorGraphMutation::DeleteNodes { node_ids: vec![unavailable, survivor] }
            .into_patch(&path, &guarded, session.session.graph().registry()),
        Err(yss_graph_editor::MutationConflict::Editor(error))
            if error.code == yss_graph_editor::EditorMutationErrorCode::GraphManagedNodeDeleteForbidden
    ));
}

#[test]
fn materialization_failure_preserves_loaded_residency_and_skips_projection() {
    let path = GraphResourcePath::new("events/MaterializationFailure.yssbi-event").unwrap();
    let control = GraphRuntimeTestControl::default();
    let session = staged_session(
        TestProject::unloaded("materialization-failure", graph_project(&path)),
        control.clone(),
    );
    assert!(
        session
            .session
            .project()
            .get_data()
            .unwrap()
            .graphs
            .is_empty()
    );
    control.fail_next_materialization();

    let error = session
        .application
        .open_graph(open_request(&session, &path))
        .unwrap_err();

    assert!(matches!(
        error,
        OpenGraphApplicationError::Materialization(GraphMaterializationError)
    ));
    let data = session.session.project().get_data().unwrap();
    assert!(data.graphs.contains_key(&path));
    assert_eq!(control.events(), [GraphRuntimeTestEvent::Materialized]);
}

#[test]
fn graph_open_replacement_respects_final_materialization_commit_boundary() {
    let path = GraphResourcePath::new("events/ReplacementBoundary.yssbi-event").unwrap();
    let project = graph_project(&path);

    let control = GraphRuntimeTestControl::default();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    control.pause_after_materialization(Arc::clone(&entered), Arc::clone(&release));
    let session = staged_session(
        TestProject::unloaded("replacement-before-commit", project.clone()),
        control.clone(),
    );
    let replacement = staged_session(
        TestProject::active("replacement-before-commit-target", project.clone()),
        GraphRuntimeTestControl::default(),
    );
    let worker_application = session.application.clone();
    let request = open_request(&session, &path);
    let worker = thread::spawn(move || worker_application.open_graph(request));

    entered.wait();
    session
        .slot
        .publish_for_test(Arc::clone(&replacement.session));
    release.wait();
    let error = worker.join().unwrap().unwrap_err();

    assert!(matches!(error, OpenGraphApplicationError::SessionChanged));
    assert!(
        session
            .session
            .project()
            .get_data()
            .unwrap()
            .graphs
            .contains_key(&path)
    );
    assert_eq!(control.events(), [GraphRuntimeTestEvent::Materialized]);

    let control = GraphRuntimeTestControl::default();
    let session = staged_session(
        TestProject::unloaded("replacement-after-commit", project.clone()),
        control.clone(),
    );
    let replacement = staged_session(
        TestProject::active("replacement-after-commit-target", project),
        GraphRuntimeTestControl::default(),
    );
    let receipt = session
        .application
        .open_graph(open_request(&session, &path))
        .expect("the final materialization commit owns the successful result");
    session
        .slot
        .publish_for_test(Arc::clone(&replacement.session));

    assert_eq!(receipt.graph_path(), &path);
    assert_eq!(control.events(), [GraphRuntimeTestEvent::Materialized]);
}

#[test]
fn open_graph_rejects_a_replaced_captured_session_before_project_load() {
    let path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
    let session = staged_session(
        TestProject::active("stale-before-load", ProjectData::new()),
        GraphRuntimeTestControl::default(),
    );
    let replacement = staged_session(
        TestProject::active("stale-before-load-target", ProjectData::new()),
        GraphRuntimeTestControl::default(),
    );
    let captured = Arc::clone(&session.session);
    session
        .slot
        .publish_for_test(Arc::clone(&replacement.session));

    let request = OpenGraphRequest::new(captured.project_instance_id().clone(), path, 1, "en-US");
    let error = open_graph_in_session(&session.application, &captured, request).unwrap_err();

    assert!(matches!(error, OpenGraphApplicationError::SessionChanged));
    assert!(session.control.events().is_empty());
}

#[test]
fn chart_edits_preserve_the_active_graph_and_execution_session() {
    let active = staged_session(
        TestProject::unloaded("chart-edits", ProjectData::new()),
        GraphRuntimeTestControl::default(),
    );
    let instance = active.session.project_instance_id().clone();
    active
        .application
        .create_chart_resource(
            instance.clone(),
            yss_project_identity::OperationId::new(),
            "Chart".into(),
            None,
        )
        .unwrap();
    let path = yss_chart_document::ChartResourcePath::parse("charts/Chart.yssbi-chart").unwrap();
    let mut document = active
        .application
        .load_chart_resource(instance.clone(), path.clone(), None)
        .unwrap();
    document.chart_type = yss_chart_document::ChartType::Scatter;
    active
        .application
        .save_chart_resource(
            instance.clone(),
            yss_project_identity::OperationId::new(),
            path.clone(),
            document,
            None,
        )
        .unwrap();
    assert_eq!(
        active
            .application
            .load_chart_resource(instance, path, None)
            .unwrap()
            .chart_type,
        yss_chart_document::ChartType::Scatter
    );
    assert!(Arc::ptr_eq(
        &active.session,
        &active.application.capture_session().unwrap()
    ));
}
