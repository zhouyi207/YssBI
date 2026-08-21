use super::*;
use crate::graph::value::{DataType, DataValue};
use crate::node_system::document::{
    ConnectionId, DocumentConnection, DocumentNode, NodePosition, ParameterValues, PortAddress,
};
use crate::node_system::protocol::{NodeTypeId, ParameterKey, PortKey};
use crate::project::{GraphDocumentKind, GraphResourceDocument, ProjectData};
use crate::variable::{VariableInstance, VariableScope};

#[derive(Default)]
struct RejectingMemoryDelivery(Mutex<Vec<GraphExecutionStreamEvent>>);

impl RejectingMemoryDelivery {
    fn deliver(&self, event: GraphExecutionStreamEvent) -> bool {
        self.0.lock().unwrap().push(event);
        false
    }

    fn event_count(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

#[test]
fn rejected_stream_delivery_preserves_committed_mutation_for_publication() {
    let variable = test_variable();
    let graph_path = GraphResourcePath::new("events/Delivery.yssbi-event").unwrap();
    let fixture = crate::project::fixtures::TempProject::activate(
        "execution-delivery-publication",
        project_with_variable_write(&graph_path, &variable),
    );
    let state = fixture.state();
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    let delivery = RejectingMemoryDelivery::default();

    let outcome = execute_graph(
        state,
        GraphExecutionRequest {
            project_instance_id,
            graph_path,
            demand: ExecutionDemand::Default,
        },
        |event| delivery.deliver(event),
    )
    .unwrap();

    assert!(outcome.delivery.delivery_failed());
    assert_eq!(outcome.delivery.delivered_event_count, 0);
    assert_eq!(
        outcome.delivery.rejected_event_count,
        delivery.event_count(),
        "every rejected delivery must be tracked",
    );
    assert_eq!(
        outcome.delivery.terminal,
        Some(TerminalRunEventDelivery {
            kind: TerminalRunEventKind::Completed,
            disposition: DeliveryDisposition::Rejected,
        }),
    );
    let mutation = outcome
        .resource_mutation
        .expect("committed mutation must remain available after stream rejection");
    assert_eq!(mutation.publication_revision, 1);
    assert_eq!(mutation.deltas.len(), 1);
    assert_eq!(
        state.get_data().unwrap().variables[&variable.id].data_value,
        DataValue::Int64(7),
    );
}

#[test]
fn stale_project_is_typed_and_rejected_before_stream_delivery() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-stale-application-execution-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let graph_path = GraphResourcePath::new("events/Delivery.yssbi-event").unwrap();
    let variable = test_variable();
    let state = ProjectState::new();
    state.activate_project_fixture(
        root.to_string_lossy().into_owned(),
        project_with_variable_write(&graph_path, &variable),
    );
    let stale_project_instance_id = state.capture_project_session().unwrap().instance_id;
    state.activate_project_fixture(
        root.to_string_lossy().into_owned(),
        project_with_variable_write(&graph_path, &variable),
    );
    let delivery = RejectingMemoryDelivery::default();

    let error = execute_graph(
        &state,
        GraphExecutionRequest {
            project_instance_id: stale_project_instance_id,
            graph_path,
            demand: ExecutionDemand::Default,
        },
        |event| delivery.deliver(event),
    )
    .unwrap_err();

    assert_eq!(
        error.project_error.kind(),
        crate::project::ProjectExecutionErrorKind::StaleProjectLifecycle,
    );
    assert_eq!(error.delivery, GraphExecutionDeliveryReport::default());
    assert_eq!(delivery.event_count(), 0);
    let _ = std::fs::remove_dir_all(root);
}

fn project_with_variable_write(
    graph_path: &GraphResourcePath,
    variable: &VariableInstance,
) -> ProjectData {
    let begin = node("yssbi.project.event.begin");
    let mut constant = node("yssbi.constant.int64");
    constant
        .parameters
        .insert(ParameterKey::new("value").unwrap(), serde_json::json!(7));
    let mut set = node("yssbi.project.variable.set");
    set.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", variable.id)),
    );

    let mut graph = GraphResourceDocument::new("Delivery", GraphDocumentKind::Event);
    graph.document.connections = [
        connection(
            PortAddress::declared(begin.id, PortKey::new("then").unwrap()),
            PortAddress::declared(set.id, PortKey::new("enter").unwrap()),
        ),
        connection(
            PortAddress::declared(constant.id, PortKey::new("value").unwrap()),
            PortAddress::declared(set.id, PortKey::new("value").unwrap()),
        ),
    ]
    .into_iter()
    .map(|connection| (connection.id, connection))
    .collect();
    graph.document.nodes = [begin, constant, set]
        .into_iter()
        .map(|node| (node.id, node))
        .collect();

    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    project.graphs.insert(graph_path.clone(), graph);
    project
}

fn node(node_type: &str) -> DocumentNode {
    DocumentNode {
        id: crate::node_system::document::NodeId::new(),
        node_type: NodeTypeId::new(node_type).unwrap(),
        position: NodePosition { x: 0.0, y: 0.0 },
        parameters: ParameterValues::new(),
        user_label: None,
    }
}

fn connection(output: PortAddress, input: PortAddress) -> DocumentConnection {
    DocumentConnection {
        id: ConnectionId::new(),
        output,
        input,
        order: None,
    }
}

fn test_variable() -> VariableInstance {
    VariableInstance {
        id: crate::variable::VariableId::new(),
        name: "Delivery".into(),
        data_type: DataType::Int64,
        data_value: DataValue::Int64(1),
        tabular: None,
        description: String::new(),
        scope: VariableScope::Global,
        tags: Vec::new(),
    }
}
