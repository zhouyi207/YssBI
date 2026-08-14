use super::mutate_graph_document_with_emitter;
use crate::event::{Event, EventProject};
use crate::node_system::catalog::build_builtin_node_system;
use crate::node_system::document::{
    ConnectionId, DocumentConnection, DocumentNode, GraphDocument, GraphRevision, NodeId,
    NodePosition, OperationId, OrderKey, ParameterValues, PortAddress, ResourceRevision,
};
use crate::node_system::protocol::{NodeTypeId, PortKey};
use crate::project::{
    GraphDocumentKind, GraphResourceDocument, GraphResourcePath, ProjectData, ProjectInstanceId,
    ProjectState,
};

const ORIGINAL_ID: u128 = 0x8101;
const SOURCE_ID: u128 = 0x8201;
const TARGET_ID: u128 = 0x8202;

#[test]
fn phase2_reroute_command_event_success_emits_once() {
    let fixture = Fixture::new();
    let request = fixture.request(fixture.revision(), ORIGINAL_ID, 1);
    let mut events = Vec::new();
    let result = mutate_graph_document_with_emitter(
        &fixture.state,
        fixture.project_instance_id.clone(),
        fixture.graph_path.as_str().to_owned(),
        "en-US",
        request,
        |event| events.push(event),
    )
    .unwrap();

    assert_eq!(events.len(), 1);
    let Event::Project(EventProject::GraphDelta {
        project_instance_id,
        delta,
    }) = &events[0]
    else {
        panic!("reroute command emitted a non-GraphDelta event");
    };
    assert_eq!(project_instance_id, fixture.project_instance_id.as_str());
    assert_eq!(delta, &result.delta);
    assert_eq!(delta.payload.operations.len(), 4);
}

#[test]
fn phase2_reroute_command_event_failure_emits_zero() {
    let fixture = Fixture::new();
    let before = fixture.serialized_authority();
    let mut events = Vec::new();
    let error = mutate_graph_document_with_emitter(
        &fixture.state,
        fixture.project_instance_id.clone(),
        fixture.graph_path.as_str().to_owned(),
        "en-US",
        fixture.request(fixture.revision(), 0xffff, 2),
        |event| events.push(event),
    )
    .unwrap_err();
    let serialized = serde_json::to_value(&error).unwrap();
    assert_eq!(
        serialized,
        serde_json::json!({
            "code": "graph_connection_not_found",
            "message": "Graph mutation rejected",
            "details": { "category": "graphMutation" }
        })
    );
    let wire = serialized.to_string();
    assert!(!wire.contains("00000000-0000-0000-0000-00000000ffff"));
    assert!(!wire.contains("mutation patch failed"));
    assert!(events.is_empty());
    assert_eq!(fixture.serialized_authority(), before);
}

#[test]
fn phase2_reroute_command_event_stale_emits_zero() {
    let fixture = Fixture::new();
    let before = fixture.serialized_authority();
    let mut events = Vec::new();
    let error = mutate_graph_document_with_emitter(
        &fixture.state,
        fixture.project_instance_id.clone(),
        fixture.graph_path.as_str().to_owned(),
        "en-US",
        fixture.request(fixture.revision().next(), ORIGINAL_ID, 3),
        |event| events.push(event),
    )
    .unwrap_err();
    assert_eq!(error.code, "graph_revision_conflict");
    assert!(events.is_empty());
    assert_eq!(fixture.serialized_authority(), before);
}

struct Fixture {
    state: ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    _project: crate::project::fixtures::TempProject,
}

impl Fixture {
    fn new() -> Self {
        let project = crate::project::fixtures::TempProject::activate(
            "phase2-reroute-command",
            ProjectData::new(),
        );
        let state = project.state().clone();
        state.project_store.write().unwrap().node_registry =
            build_builtin_node_system().unwrap().registry;
        let graph_path = GraphResourcePath::new("events/Phase2RerouteCommand.yssbi-event").unwrap();
        let mut graph =
            GraphResourceDocument::new("Phase 2 Reroute Command", GraphDocumentKind::Event);
        graph.document = base_document();
        state.insert_graph(graph_path.clone(), graph).unwrap();
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        Self {
            state,
            project_instance_id,
            graph_path,
            _project: project,
        }
    }

    fn request(
        &self,
        revision: ResourceRevision,
        connection: u128,
        flavor: u128,
    ) -> serde_json::Value {
        serde_json::json!({
            "resource": { "kind": "graph", "key": self.graph_path.as_str() },
            "baseRevision": revision,
            "operationId": OperationId::from_uuid(uuid::Uuid::from_u128(0x8300 + flavor)),
            "payload": {
                "type": "insertReroute",
                "payload": {
                    "connectionId": connection_id(connection),
                    "position": { "x": 120.0, "y": 30.0 }
                }
            }
        })
    }

    fn revision(&self) -> ResourceRevision {
        self.state.get_data().unwrap().graphs[&self.graph_path]
            .document
            .revision
    }

    fn serialized_authority(&self) -> serde_json::Value {
        serde_json::json!({
            "data": self.state.get_data().unwrap(),
            "history": self.state.history_status(),
            "historyLengths": self.state.history_lengths_for_test(),
            "revisions": self.state.revision_state_for_test(),
            "publication": self.state.publication_state_for_test(),
        })
    }
}

fn base_document() -> GraphDocument {
    let mut document = GraphDocument::default();
    for (id, x) in [(SOURCE_ID, 0.0), (TARGET_ID, 240.0)] {
        document.nodes.insert(
            node_id(id),
            DocumentNode {
                id: node_id(id),
                node_type: NodeTypeId::new("yssbi.debug.view").unwrap(),
                position: NodePosition { x, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    let connection = DocumentConnection {
        id: connection_id(ORIGINAL_ID),
        output: declared(node_id(SOURCE_ID), "snapshot"),
        input: declared(node_id(TARGET_ID), "data"),
        order: Some(OrderKey("original-order".into())),
    };
    document.connections.insert(connection.id, connection);
    document.revision = GraphRevision::INITIAL;
    document
}

fn node_id(value: u128) -> NodeId {
    NodeId::from_uuid(uuid::Uuid::from_u128(value))
}
fn connection_id(value: u128) -> ConnectionId {
    ConnectionId::from_uuid(uuid::Uuid::from_u128(value))
}
fn declared(node: NodeId, key: &str) -> PortAddress {
    PortAddress::declared(node, PortKey::new(key).unwrap())
}
