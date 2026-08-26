use super::{export_graph_subgraph_from_state, mutation_conflict_to_command_error};
use crate::graph_document::GraphResourcePath;
use crate::graph_document::{DocumentNode, GraphDocument, NodeId, NodePosition, ParameterValues};
use crate::node_system::catalog::build_builtin_node_system;
use crate::node_system::document::MutationConflict;
use crate::node_system::protocol::NodeTypeId;
use crate::project::{
    GraphDocumentKind, GraphResourceDocument, ProjectData, ProjectInstanceId, ProjectState,
};

#[test]
fn export_graph_subgraph_is_a_read_only_authoritative_query_and_emits_zero_events() {
    let fixture = Fixture::new();
    let before = fixture.serialized_authority();

    let snapshot = export_graph_subgraph_from_state(
        &fixture.state,
        fixture.project_instance_id.clone(),
        fixture.graph_path.as_str().to_owned(),
        vec![fixture.node_id],
    )
    .unwrap();

    let command_source = include_str!("command_node_system/editor.rs");
    let export_command_source = command_source
        .split_once("pub(crate) fn export_graph_subgraph_from_state")
        .unwrap()
        .1
        .split_once("fn parse_editor_mutation_request")
        .unwrap()
        .0;
    let event_count = export_command_source.matches("emit_project_event").count();

    assert_eq!(snapshot.nodes.len(), 1);
    assert_eq!(event_count, 0);
    assert_eq!(fixture.serialized_authority(), before);
}

#[test]
fn export_graph_subgraph_serializes_camel_case_dto_fields() {
    let fixture = Fixture::new();
    let snapshot = export_graph_subgraph_from_state(
        &fixture.state,
        fixture.project_instance_id.clone(),
        fixture.graph_path.as_str().to_owned(),
        vec![fixture.node_id],
    )
    .unwrap();

    let wire = serde_json::to_value(snapshot).unwrap();
    assert_eq!(wire["schemaVersion"], 1);
    assert!(wire.get("schema_version").is_none());
    assert!(wire.get("portBindings").unwrap().is_array());
    assert!(wire.get("inputStates").unwrap().is_array());
    assert!(wire["nodes"][0].get("localId").is_some());
    assert!(wire["nodes"][0].get("relativePosition").is_some());
}

#[test]
fn export_graph_subgraph_rejects_stale_project_identity_without_side_effects() {
    let fixture = Fixture::new();
    let before = fixture.serialized_authority();

    let error = export_graph_subgraph_from_state(
        &fixture.state,
        ProjectInstanceId::new(),
        fixture.graph_path.as_str().to_owned(),
        vec![fixture.node_id],
    )
    .unwrap_err();

    assert_eq!(error.code(), "stale_project_lifecycle");
    assert_eq!(fixture.serialized_authority(), before);
}

#[test]
fn export_graph_subgraph_maps_clipboard_conflicts_to_stable_public_codes() {
    for error in [
        MutationConflict::ClipboardSubgraphInvalid("invalid clipboard".into()),
        MutationConflict::ReferencedResourceUnavailable("missing resource".into()),
    ] {
        let expected_code = error.code();
        let command_error = mutation_conflict_to_command_error(error, "graph_revision_conflict");
        assert_eq!(command_error.code(), expected_code);
        assert!(command_error.details().is_none());
    }
}

struct Fixture {
    state: ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    node_id: NodeId,
    _project: crate::project::fixtures::TempProject,
}

impl Fixture {
    fn new() -> Self {
        let project = crate::project::fixtures::TempProject::activate(
            "task5-export-graph-subgraph",
            ProjectData::new(),
        );
        let state = project.state().clone();
        state.project_store.write().unwrap().node_registry =
            build_builtin_node_system().unwrap().registry;
        let graph_path = GraphResourcePath::new("events/Task5Export.yssbi-event").unwrap();
        let node_id = NodeId::from_uuid(uuid::Uuid::from_u128(0x5_001));
        let mut document = GraphDocument::default();
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: NodeTypeId::new("yssbi.constant.int64").unwrap(),
                position: NodePosition { x: 10.0, y: 20.0 },
                parameters: ParameterValues::new(),
                user_label: Some("Export me".to_owned()),
            },
        );
        let mut graph = GraphResourceDocument::new("Task 5 Export", GraphDocumentKind::Event);
        graph.document = document;
        state.insert_graph(graph_path.clone(), graph).unwrap();
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        Self {
            state,
            project_instance_id,
            graph_path,
            node_id,
            _project: project,
        }
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
