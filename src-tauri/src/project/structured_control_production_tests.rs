use super::{
    GraphDocumentKind, GraphResourceDocument, GraphResourcePath, ProjectData, ProjectState,
};
use crate::node_system::document::{
    ConnectionId, DocumentConnection, DocumentNode, DynamicPortBinding, GraphDocumentOperation,
    GraphDocumentPatch, GraphRevision, NodeId, NodePosition, OperationId, OrderKey,
    ParameterValues, PortAddress, PortInstanceId, ResourceKey,
};
use crate::node_system::protocol::{NodeTypeId, ParameterKey, PortKey};
use crate::node_system::runtime::{
    OrdinaryRunErrorCode, ProjectResourceLeaseObserver, RunErrorOutcome, RunEvent, RunEventKind,
    RunEventSink,
};
use crate::variable::{VariableId, VariableInstance, VariableScope};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use uuid::Uuid;

mod function_fixtures;

const EVENT_PATH: &str = "events/BranchProduction.yssbi-event";
const BEGIN_NODE: u128 = 1;
const CONDITION_NODE: u128 = 2;
const THEN_VALUE_NODE: u128 = 3;
const ELSE_VALUE_NODE: u128 = 4;
const BRANCH_NODE: u128 = 5;
const TRUE_EFFECT_NODE: u128 = 6;
const FALSE_EFFECT_NODE: u128 = 7;
const MERGE_NODE: u128 = 8;
const RESULT_SET_NODE: u128 = 9;
const RESULT_VARIABLE: u128 = 500;
const LOOP_NODE: u128 = 20;
const LOOP_INITIAL_NODE: u128 = 21;
const LOOP_STEP_NODE: u128 = 22;
const LOOP_BODY_NODE: u128 = 26;
const LOOP_RESULT_SET_NODE: u128 = 27;
const LOOP_CARRIED_MEMBER: u128 = 60;
const LOOP_SECOND_NUMERIC_MEMBER: u128 = 61;
const LOOP_BRANCH_CONDITION_MEMBER: u128 = 62;
const LOOP_CONTINUE_CONDITION_MEMBER: u128 = 63;
const LOOP_BODY_BRANCH_NODE: u128 = 38;
const LOOP_FIRST_OBSERVER_SET_NODE: u128 = 39;
const LOOP_SECOND_OBSERVER_SET_NODE: u128 = 40;
const LOOP_BRANCH_RESULT_MEMBER: u128 = 70;
const FIRST_OBSERVER_VARIABLE: u128 = 501;
const SECOND_OBSERVER_VARIABLE: u128 = 502;
const EFFECT_BEFORE_NODE: u128 = 900;
const EFFECT_AFTER_NODE: u128 = 800;
const EFFECT_RESOURCE_NODE: u128 = 801;
const EFFECT_DIVIDE_NODE: u128 = 820;

const EFFECT_RESOURCE_VARIABLE: u128 = 503;
const EFFECT_SECOND_RESOURCE_VARIABLE: u128 = 504;
const EFFECT_RESULT_VARIABLE: u128 = 505;
const EFFECT_FINAL_SET_NODE: u128 = 840;

#[derive(Debug, Clone, Copy)]
enum InsertionOrder {
    Forward,
    Reverse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompletedOperation {
    source_node_id: NodeId,
    source_node_type_id: NodeTypeId,
    activation_id: u64,
}

#[derive(Default)]
struct RecordingRunEvents(Mutex<Vec<RunEvent>>);

impl RunEventSink for RecordingRunEvents {
    fn record(&self, event: RunEvent) {
        self.0.lock().unwrap().push(event);
    }
}

impl RecordingRunEvents {
    fn events(&self) -> Vec<RunEvent> {
        self.0.lock().unwrap().clone()
    }

    fn completed_operations(&self) -> Vec<CompletedOperation> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| {
                let RunEventKind::OperationCompleted { activation_id, .. } = event.kind else {
                    return None;
                };
                Some(CompletedOperation {
                    source_node_id: event
                        .correlation
                        .node_id
                        .expect("operation event must identify its source node"),
                    source_node_type_id: event
                        .correlation
                        .node_type_id
                        .clone()
                        .expect("operation event must identify its source node type"),
                    activation_id,
                })
            })
            .collect()
    }
}

struct BranchOutcome {
    expected_result_variable_id: VariableId,
    result: crate::graph::value::DataValue,
    committed_variable_ids: Box<[VariableId]>,
    completed_operations: Vec<CompletedOperation>,
}

impl BranchOutcome {
    fn completion_count(&self, node: u128, node_type: &str) -> usize {
        let expected = source(node, node_type);
        self.completed_operations
            .iter()
            .filter(|operation| {
                (
                    operation.source_node_id,
                    operation.source_node_type_id.clone(),
                ) == expected
            })
            .count()
    }

    fn normalized_completion_sequence(&self) -> Vec<(NodeId, NodeTypeId, usize)> {
        let mut activation_ordinals = BTreeMap::new();
        self.completed_operations
            .iter()
            .map(|operation| {
                let next_ordinal = activation_ordinals.len();
                let activation_ordinal = *activation_ordinals
                    .entry(operation.activation_id)
                    .or_insert(next_ordinal);
                (
                    operation.source_node_id,
                    operation.source_node_type_id.clone(),
                    activation_ordinal,
                )
            })
            .collect()
    }
}

struct BranchDocumentFixture {
    resource: GraphResourceDocument,
    node_insertions: Vec<NodeId>,
    connection_insertions: Vec<ConnectionId>,
    binding_insertions: Vec<PortAddress>,
}

struct TempProject {
    state: Option<ProjectState>,
    root: PathBuf,
}

impl TempProject {
    fn activate(project: ProjectData) -> Self {
        let root =
            std::env::temp_dir().join(format!("yssbi-branch-production-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut fixture = Self { state: None, root };
        crate::project::fixtures::write_project(&project, fixture.root.to_string_lossy().as_ref())
            .unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(fixture.root.to_string_lossy().into_owned(), project);
        fixture.state = Some(state);
        fixture
    }

    fn state(&self) -> &ProjectState {
        self.state.as_ref().expect("temporary project is active")
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        drop(self.state.take());
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn variable_id(value: u128) -> VariableId {
    VariableId::from(Uuid::from_u128(value))
}

fn result_variable_id() -> VariableId {
    variable_id(RESULT_VARIABLE)
}

fn node_id(value: u128) -> NodeId {
    NodeId::from_uuid(Uuid::from_u128(value))
}

fn connection_id(value: u128) -> ConnectionId {
    ConnectionId::from_uuid(Uuid::from_u128(value))
}

fn instance_id(value: u128) -> PortInstanceId {
    PortInstanceId::from_uuid(Uuid::from_u128(value))
}

fn node(value: u128, node_type: &str) -> DocumentNode {
    DocumentNode {
        id: node_id(value),
        node_type: NodeTypeId::new(node_type).unwrap(),
        position: NodePosition {
            x: value as f64,
            y: 0.0,
        },
        parameters: ParameterValues::new(),
        user_label: None,
    }
}

fn declared(node: u128, port: &str) -> PortAddress {
    PortAddress::declared(node_id(node), PortKey::new(port).unwrap())
}

fn instance(node: u128, template: &str, member: u128) -> PortAddress {
    PortAddress::instance(
        node_id(node),
        PortKey::new(template).unwrap(),
        instance_id(member),
    )
}

fn connection(value: u128, output: PortAddress, input: PortAddress) -> DocumentConnection {
    DocumentConnection {
        id: connection_id(value),
        output,
        input,
        order: None,
    }
}

fn in_order<T>(mut values: Vec<T>, insertion_order: InsertionOrder) -> Vec<T> {
    if matches!(insertion_order, InsertionOrder::Reverse) {
        values.reverse();
    }
    values
}

fn insert_tracked<K: Clone + Ord, V>(
    map: &mut BTreeMap<K, V>,
    trace: &mut Vec<K>,
    key: K,
    value: V,
) {
    trace.push(key.clone());
    assert!(map.insert(key, value).is_none());
}

fn branch_fixture(condition: bool, insertion_order: InsertionOrder) -> BranchDocumentFixture {
    let mut condition_node = node(CONDITION_NODE, "yssbi.constant.bool");
    condition_node.parameters.insert(
        ParameterKey::new("value").unwrap(),
        serde_json::json!(condition),
    );
    let mut then_value = node(THEN_VALUE_NODE, "yssbi.constant.int64");
    then_value
        .parameters
        .insert(ParameterKey::new("value").unwrap(), serde_json::json!(11));
    let mut else_value = node(ELSE_VALUE_NODE, "yssbi.constant.int64");
    else_value
        .parameters
        .insert(ParameterKey::new("value").unwrap(), serde_json::json!(22));
    let mut result_set = node(RESULT_SET_NODE, "yssbi.project.variable.set");
    result_set.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", result_variable_id())),
    );

    let mut resource = GraphResourceDocument::new("Branch Production", GraphDocumentKind::Event);
    let node_entries = vec![
        node(BEGIN_NODE, "yssbi.project.event.begin"),
        condition_node,
        then_value,
        else_value,
        node(BRANCH_NODE, "yssbi.control.branch"),
        node(TRUE_EFFECT_NODE, "yssbi.control.do"),
        node(FALSE_EFFECT_NODE, "yssbi.control.do"),
        node(MERGE_NODE, "yssbi.control.merge"),
        result_set,
    ];
    let mut node_insertions = Vec::new();
    for node in in_order(node_entries, insertion_order) {
        insert_tracked(
            &mut resource.document.nodes,
            &mut node_insertions,
            node.id,
            node,
        );
    }

    let branch_member = 40;
    let binding_entries = vec![
        (
            instance(BRANCH_NODE, "then_source", branch_member),
            DynamicPortBinding::UserCreated {
                order: OrderKey("result".into()),
            },
        ),
        (
            instance(BRANCH_NODE, "else_source", branch_member),
            DynamicPortBinding::UserCreated {
                order: OrderKey("result".into()),
            },
        ),
        (
            instance(BRANCH_NODE, "result", branch_member),
            DynamicPortBinding::UserCreated {
                order: OrderKey("result".into()),
            },
        ),
        (
            instance(MERGE_NODE, "enter", 70),
            DynamicPortBinding::UserCreated {
                order: OrderKey("true".into()),
            },
        ),
        (
            instance(MERGE_NODE, "enter", 71),
            DynamicPortBinding::UserCreated {
                order: OrderKey("false".into()),
            },
        ),
    ];
    let mut binding_insertions = Vec::new();
    for (address, binding) in in_order(binding_entries, insertion_order) {
        insert_tracked(
            &mut resource.document.port_bindings,
            &mut binding_insertions,
            address,
            binding,
        );
    }

    let connection_entries = vec![
        connection(
            100,
            declared(BEGIN_NODE, "then"),
            declared(BRANCH_NODE, "enter"),
        ),
        connection(
            101,
            declared(CONDITION_NODE, "value"),
            declared(BRANCH_NODE, "condition"),
        ),
        connection(
            102,
            declared(THEN_VALUE_NODE, "value"),
            instance(BRANCH_NODE, "then_source", branch_member),
        ),
        connection(
            103,
            declared(ELSE_VALUE_NODE, "value"),
            instance(BRANCH_NODE, "else_source", branch_member),
        ),
        connection(
            104,
            instance(BRANCH_NODE, "result", branch_member),
            declared(RESULT_SET_NODE, "value"),
        ),
        connection(
            105,
            declared(BRANCH_NODE, "true"),
            declared(TRUE_EFFECT_NODE, "enter"),
        ),
        connection(
            106,
            declared(TRUE_EFFECT_NODE, "then"),
            instance(MERGE_NODE, "enter", 70),
        ),
        connection(
            107,
            declared(BRANCH_NODE, "false"),
            declared(FALSE_EFFECT_NODE, "enter"),
        ),
        connection(
            108,
            declared(FALSE_EFFECT_NODE, "then"),
            instance(MERGE_NODE, "enter", 71),
        ),
        connection(
            109,
            declared(MERGE_NODE, "then"),
            declared(RESULT_SET_NODE, "enter"),
        ),
    ];
    let mut connection_insertions = Vec::new();
    for connection in in_order(connection_entries, insertion_order) {
        insert_tracked(
            &mut resource.document.connections,
            &mut connection_insertions,
            connection.id,
            connection,
        );
    }

    BranchDocumentFixture {
        resource,
        node_insertions,
        connection_insertions,
        binding_insertions,
    }
}

fn result_variable() -> VariableInstance {
    int64_result_variable("Branch Result")
}

fn int64_result_variable(name: &str) -> VariableInstance {
    VariableInstance {
        id: result_variable_id(),
        name: name.into(),
        data_type: crate::graph::value::DataType::Int64,
        data_value: crate::graph::value::DataValue::Int64(0),
        tabular: None,
        description: String::new(),
        scope: VariableScope::Global,
        tags: Vec::new(),
    }
}

fn run_branch(document: GraphResourceDocument) -> BranchOutcome {
    let result_variable = result_variable();
    let mut project = ProjectData::new();
    project
        .variables
        .insert(result_variable.id, result_variable.clone());
    let fixture = TempProject::activate(project);
    let graph_path = GraphResourcePath::new(EVENT_PATH).unwrap();
    fixture
        .state()
        .insert_graph(graph_path.clone(), document)
        .unwrap();
    crate::project::fixtures::write_state_graph(fixture.state(), &graph_path).unwrap();
    let events = RecordingRunEvents::default();
    let run = fixture
        .state()
        .execute_graph_for_current_project_for_test(
            &graph_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &events,
        )
        .unwrap();
    let result = fixture.state().get_data().unwrap().variables[&result_variable.id]
        .data_value
        .clone();

    BranchOutcome {
        expected_result_variable_id: result_variable.id,
        result,
        committed_variable_ids: run.committed_variable_ids,
        completed_operations: events.completed_operations(),
    }
}

fn float64_variable(id: VariableId, name: &str) -> VariableInstance {
    VariableInstance {
        id,
        name: name.into(),
        data_type: crate::graph::value::DataType::Float64,
        data_value: crate::graph::value::DataValue::Float64(0.0),
        tabular: None,
        description: String::new(),
        scope: VariableScope::Global,
        tags: Vec::new(),
    }
}

fn float64_result_variable(name: &str) -> VariableInstance {
    float64_variable(result_variable_id(), name)
}

fn source(node: u128, node_type: &str) -> (NodeId, NodeTypeId) {
    (node_id(node), NodeTypeId::new(node_type).unwrap())
}

fn assert_branch_outcome(
    outcome: &BranchOutcome,
    expected_result: i64,
    selected_effect: u128,
    unselected_effect: u128,
) {
    assert_eq!(
        outcome.result,
        crate::graph::value::DataValue::Int64(expected_result)
    );
    assert_eq!(
        outcome.committed_variable_ids.as_ref(),
        &[outcome.expected_result_variable_id]
    );
    assert_eq!(
        outcome.completion_count(selected_effect, "yssbi.control.do"),
        1,
        "selected effectful Do scheduler operation must complete once"
    );
    assert_eq!(
        outcome.completion_count(unselected_effect, "yssbi.control.do"),
        0,
        "unselected effectful Do scheduler operation must not complete"
    );
    assert_eq!(
        outcome.completion_count(RESULT_SET_NODE, "yssbi.project.variable.set"),
        1,
        "authoritative Variable Set scheduler operation must complete once"
    );
}

fn loop_node(value: u128, node_type: &str, parameter: Option<serde_json::Value>) -> DocumentNode {
    let mut node = node(value, node_type);
    if let Some(value) = parameter {
        node.parameters
            .insert(ParameterKey::new("value").unwrap(), value);
    }
    node
}

fn loop_member_binding(
    resource: &mut GraphResourceDocument,
    template: &str,
    member: u128,
    order: &str,
) -> PortAddress {
    let address = instance(LOOP_NODE, template, member);
    assert!(
        resource
            .document
            .port_bindings
            .insert(
                address.clone(),
                DynamicPortBinding::UserCreated {
                    order: OrderKey(order.into()),
                },
            )
            .is_none()
    );
    address
}

fn loop_binding(resource: &mut GraphResourceDocument, template: &str) -> PortAddress {
    loop_member_binding(resource, template, LOOP_CARRIED_MEMBER, "carried")
}

fn variable_set_node(value: u128, variable: VariableId) -> DocumentNode {
    let mut set = node(value, "yssbi.project.variable.set");
    set.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{variable}")),
    );
    set
}

fn carried_observation_loop_fixture() -> GraphResourceDocument {
    let mut resource =
        GraphResourceDocument::new("Observed Carried Loop", GraphDocumentKind::Event);
    let mut loop_control = node(LOOP_NODE, "yssbi.control.loop");
    loop_control.parameters.insert(
        ParameterKey::new("max_iterations").unwrap(),
        serde_json::json!(4),
    );
    for entry in [
        node(BEGIN_NODE, "yssbi.project.event.begin"),
        loop_control,
        loop_node(30, "yssbi.constant.float64", Some(serde_json::json!(1.5))),
        loop_node(31, "yssbi.constant.float64", Some(serde_json::json!(11.5))),
        loop_node(32, "yssbi.constant.float64", Some(serde_json::json!(2.5))),
        loop_node(33, "yssbi.constant.float64", Some(serde_json::json!(22.5))),
        loop_node(34, "yssbi.constant.bool", Some(serde_json::json!(true))),
        loop_node(35, "yssbi.constant.bool", Some(serde_json::json!(false))),
        loop_node(36, "yssbi.constant.bool", Some(serde_json::json!(true))),
        loop_node(37, "yssbi.constant.bool", Some(serde_json::json!(false))),
        node(LOOP_BODY_BRANCH_NODE, "yssbi.control.branch"),
        variable_set_node(
            LOOP_FIRST_OBSERVER_SET_NODE,
            variable_id(FIRST_OBSERVER_VARIABLE),
        ),
        variable_set_node(
            LOOP_SECOND_OBSERVER_SET_NODE,
            variable_id(SECOND_OBSERVER_VARIABLE),
        ),
        variable_set_node(LOOP_RESULT_SET_NODE, result_variable_id()),
        loop_node(41, "yssbi.constant.float64", Some(serde_json::json!(101.5))),
        loop_node(42, "yssbi.constant.float64", Some(serde_json::json!(202.5))),
    ] {
        assert!(resource.document.nodes.insert(entry.id, entry).is_none());
    }

    let members = [
        (LOOP_CARRIED_MEMBER, "numeric-a"),
        (LOOP_SECOND_NUMERIC_MEMBER, "numeric-b"),
        (LOOP_BRANCH_CONDITION_MEMBER, "branch-condition"),
        (LOOP_CONTINUE_CONDITION_MEMBER, "continue-condition"),
    ]
    .map(|(member, order)| {
        (
            loop_member_binding(&mut resource, "initial_source", member, order),
            loop_member_binding(&mut resource, "body_input", member, order),
            loop_member_binding(&mut resource, "next_source", member, order),
            loop_member_binding(&mut resource, "result", member, order),
        )
    });
    let branch_ports = ["then_source", "else_source", "result"].map(|template| {
        let address = instance(LOOP_BODY_BRANCH_NODE, template, LOOP_BRANCH_RESULT_MEMBER);
        assert!(
            resource
                .document
                .port_bindings
                .insert(
                    address.clone(),
                    DynamicPortBinding::UserCreated {
                        order: OrderKey("observed".into()),
                    },
                )
                .is_none()
        );
        address
    });

    let edges = vec![
        connection(
            300,
            declared(BEGIN_NODE, "then"),
            declared(LOOP_NODE, "enter"),
        ),
        connection(301, declared(30, "value"), members[0].0.clone()),
        connection(302, declared(31, "value"), members[0].2.clone()),
        connection(303, declared(32, "value"), members[1].0.clone()),
        connection(304, declared(33, "value"), members[1].2.clone()),
        connection(305, declared(34, "value"), members[2].0.clone()),
        connection(306, declared(35, "value"), members[2].2.clone()),
        connection(307, declared(36, "value"), members[3].0.clone()),
        connection(308, declared(37, "value"), members[3].2.clone()),
        connection(309, members[3].1.clone(), declared(LOOP_NODE, "condition")),
        connection(
            310,
            declared(LOOP_NODE, "body"),
            declared(LOOP_BODY_BRANCH_NODE, "enter"),
        ),
        connection(
            311,
            members[2].1.clone(),
            declared(LOOP_BODY_BRANCH_NODE, "condition"),
        ),
        connection(312, declared(41, "value"), branch_ports[0].clone()),
        connection(313, declared(42, "value"), branch_ports[1].clone()),
        connection(
            314,
            declared(LOOP_BODY_BRANCH_NODE, "true"),
            declared(LOOP_FIRST_OBSERVER_SET_NODE, "enter"),
        ),
        connection(
            315,
            members[0].1.clone(),
            declared(LOOP_FIRST_OBSERVER_SET_NODE, "value"),
        ),
        connection(
            316,
            declared(LOOP_BODY_BRANCH_NODE, "false"),
            declared(LOOP_SECOND_OBSERVER_SET_NODE, "enter"),
        ),
        connection(
            317,
            members[1].1.clone(),
            declared(LOOP_SECOND_OBSERVER_SET_NODE, "value"),
        ),
        connection(
            318,
            declared(LOOP_NODE, "then"),
            declared(LOOP_RESULT_SET_NODE, "enter"),
        ),
        connection(
            319,
            members[1].3.clone(),
            declared(LOOP_RESULT_SET_NODE, "value"),
        ),
    ];
    for edge in edges {
        assert!(
            resource
                .document
                .connections
                .insert(edge.id, edge)
                .is_none()
        );
    }
    resource
}

fn loop_fixture(condition: bool, body_type: &str, max_iterations: u64) -> GraphResourceDocument {
    let mut resource = GraphResourceDocument::new("Loop Production", GraphDocumentKind::Event);
    let mut loop_control = node(LOOP_NODE, "yssbi.control.loop");
    loop_control.parameters.insert(
        ParameterKey::new("max_iterations").unwrap(),
        serde_json::json!(max_iterations),
    );
    let mut result_set = node(LOOP_RESULT_SET_NODE, "yssbi.project.variable.set");
    result_set.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", result_variable_id())),
    );
    for entry in [
        node(BEGIN_NODE, "yssbi.project.event.begin"),
        loop_control,
        loop_node(
            LOOP_INITIAL_NODE,
            "yssbi.constant.float64",
            Some(serde_json::json!(0.1)),
        ),
        loop_node(
            LOOP_STEP_NODE,
            "yssbi.constant.float64",
            Some(serde_json::json!(3.5)),
        ),
        loop_node(
            CONDITION_NODE,
            "yssbi.constant.bool",
            Some(serde_json::json!(condition)),
        ),
        node(LOOP_BODY_NODE, body_type),
        result_set,
    ] {
        assert!(resource.document.nodes.insert(entry.id, entry).is_none());
    }

    let initial = loop_binding(&mut resource, "initial_source");
    let body_input = loop_binding(&mut resource, "body_input");
    let next = loop_binding(&mut resource, "next_source");
    let result = loop_binding(&mut resource, "result");
    let mut edges = vec![
        connection(
            200,
            declared(BEGIN_NODE, "then"),
            declared(LOOP_NODE, "enter"),
        ),
        connection(
            201,
            declared(CONDITION_NODE, "value"),
            declared(LOOP_NODE, "condition"),
        ),
        connection(202, declared(LOOP_INITIAL_NODE, "value"), initial),
        connection(203, declared(LOOP_STEP_NODE, "value"), next),
        connection(
            204,
            declared(LOOP_NODE, "body"),
            declared(LOOP_BODY_NODE, "enter"),
        ),
        connection(
            205,
            declared(LOOP_NODE, "then"),
            declared(LOOP_RESULT_SET_NODE, "enter"),
        ),
        connection(206, result, declared(LOOP_RESULT_SET_NODE, "value")),
    ];
    if body_type == "yssbi.control.sleep" {
        edges.push(connection(
            207,
            body_input,
            declared(LOOP_BODY_NODE, "duration"),
        ));
    }
    for edge in edges {
        assert!(
            resource
                .document
                .connections
                .insert(edge.id, edge)
                .is_none()
        );
    }
    resource
}

fn run_loop(
    document: GraphResourceDocument,
    result_variable: VariableInstance,
) -> (
    Result<crate::node_system::runtime::RunResult, String>,
    crate::graph::value::DataValue,
    Vec<RunEvent>,
) {
    let mut project = ProjectData::new();
    project
        .variables
        .insert(result_variable.id, result_variable.clone());
    let fixture = TempProject::activate(project);
    let graph_path = GraphResourcePath::new(EVENT_PATH).unwrap();
    fixture
        .state()
        .insert_graph(graph_path.clone(), document)
        .unwrap();
    crate::project::fixtures::write_state_graph(fixture.state(), &graph_path).unwrap();
    let events = RecordingRunEvents::default();
    let run = fixture
        .state()
        .execute_graph_for_current_project_for_test(
            &graph_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &events,
        )
        .map_err(|error| error.to_string());
    let result = fixture.state().get_data().unwrap().variables[&result_variable.id]
        .data_value
        .clone();
    (run, result, events.events())
}

fn event_count(events: &[RunEvent], node: u128, node_type: &str, completed: bool) -> usize {
    let expected = source(node, node_type);
    events
        .iter()
        .filter(|event| {
            let matching_kind = if completed {
                matches!(event.kind, RunEventKind::OperationCompleted { .. })
            } else {
                matches!(event.kind, RunEventKind::OperationStarted { .. })
            };
            matching_kind
                && (
                    event.correlation.node_id,
                    event.correlation.node_type_id.clone(),
                ) == (Some(expected.0), Some(expected.1.clone()))
        })
        .count()
}

fn assert_no_completed_result(events: &[RunEvent]) {
    assert!(
        events
            .iter()
            .all(|event| !matches!(event.kind, RunEventKind::ResultReady { .. }))
    );
    assert_eq!(
        event_count(
            events,
            LOOP_RESULT_SET_NODE,
            "yssbi.project.variable.set",
            true,
        ),
        0
    );
    assert!(
        events
            .iter()
            .all(|event| event.kind != RunEventKind::RunCompleted)
    );
}

fn assert_reversed_insertions<T: std::fmt::Debug + PartialEq>(forward: &[T], reverse: &[T]) {
    assert!(
        forward.len() >= 2,
        "fixture BTreeMap must receive multiple inserts"
    );
    assert_eq!(
        forward.iter().rev().collect::<Vec<_>>(),
        reverse.iter().collect::<Vec<_>>()
    );
}

const FUNCTION_PATH: &str = "functions/CallProduction.yssbi-function";
const CALL_EVENT_PATH: &str = "events/CallProduction.yssbi-event";
const CALL_BODY_NODE: u128 = 250;
const CALL_ONE_NODE: u128 = 610;
const CALL_TWO_NODE: u128 = 620;
const CALL_ONE_SET_NODE: u128 = 710;
const CALL_TWO_SET_NODE: u128 = 720;
const CALL_ONE_VARIABLE: u128 = 601;
const CALL_TWO_VARIABLE: u128 = 602;

fn int64_variable(id: u128, name: &str, value: i64) -> VariableInstance {
    let mut variable = float64_variable(variable_id(id), name);
    variable.data_type = crate::graph::value::DataType::Int64;
    variable.data_value = crate::graph::value::DataValue::Int64(value);
    variable
}

fn int64_constant(id: u128, value: i64) -> DocumentNode {
    let mut constant = node(id, "yssbi.constant.int64");
    constant.parameters.insert(
        ParameterKey::new("value").unwrap(),
        serde_json::json!(value),
    );
    constant
}

fn call_node_with_ports(
    event: &mut GraphResourceDocument,
    node_value: u128,
    instance_base: u128,
    function_path: &GraphResourcePath,
) -> (DocumentNode, PortAddress, PortAddress) {
    let mut call = node(node_value, "yssbi.project.function.call");
    call.parameters.insert(
        ParameterKey::new("target").unwrap(),
        serde_json::json!(function_path.as_str()),
    );
    let argument = function_fixtures::resolved_function_port(
        event,
        node_value,
        "arguments",
        instance_base,
        function_path,
        &function_fixtures::parameter_id(),
        "caller-argument-a",
    );
    let result = function_fixtures::resolved_function_port(
        event,
        node_value,
        "results",
        instance_base + 1,
        function_path,
        &function_fixtures::return_id(),
        "caller-result-z",
    );
    (call, argument, result)
}

fn single_call_event(
    function_path: &GraphResourcePath,
    argument: i64,
    output_variable: VariableId,
) -> GraphResourceDocument {
    let mut event = GraphResourceDocument::new("Call Production", GraphDocumentKind::Event);
    let begin = node(BEGIN_NODE, "yssbi.project.event.begin");
    let source = int64_constant(500, argument);
    let (call, call_argument, call_result) =
        call_node_with_ports(&mut event, CALL_ONE_NODE, 8_001, function_path);
    let output = variable_set_node(CALL_ONE_SET_NODE, output_variable);
    for entry in [begin, source, call, output] {
        assert!(event.document.nodes.insert(entry.id, entry).is_none());
    }
    for edge in [
        connection(
            80_001,
            declared(BEGIN_NODE, "then"),
            declared(CALL_ONE_NODE, "enter"),
        ),
        connection(
            80_002,
            declared(CALL_ONE_NODE, "then"),
            declared(CALL_ONE_SET_NODE, "enter"),
        ),
        connection(80_003, declared(500, "value"), call_argument),
        connection(80_004, call_result, declared(CALL_ONE_SET_NODE, "value")),
    ] {
        assert!(event.document.connections.insert(edge.id, edge).is_none());
    }
    event
}

fn two_call_event(function_path: &GraphResourcePath) -> GraphResourceDocument {
    let mut event = GraphResourceDocument::new("Two Calls", GraphDocumentKind::Event);
    let begin = node(BEGIN_NODE, "yssbi.project.event.begin");
    let first_source = int64_constant(501, 11);
    let second_source = int64_constant(502, 22);
    let (first_call, first_argument, first_result) =
        call_node_with_ports(&mut event, CALL_ONE_NODE, 8_101, function_path);
    let (second_call, second_argument, second_result) =
        call_node_with_ports(&mut event, CALL_TWO_NODE, 8_201, function_path);
    let first_set = variable_set_node(CALL_ONE_SET_NODE, variable_id(CALL_ONE_VARIABLE));
    let second_set = variable_set_node(CALL_TWO_SET_NODE, variable_id(CALL_TWO_VARIABLE));
    for entry in [
        begin,
        first_source,
        second_source,
        first_call,
        second_call,
        first_set,
        second_set,
    ] {
        assert!(event.document.nodes.insert(entry.id, entry).is_none());
    }
    for edge in [
        connection(
            81_001,
            declared(BEGIN_NODE, "then"),
            declared(CALL_ONE_NODE, "enter"),
        ),
        connection(
            81_002,
            declared(CALL_ONE_NODE, "then"),
            declared(CALL_ONE_SET_NODE, "enter"),
        ),
        connection(
            81_003,
            declared(CALL_ONE_SET_NODE, "then"),
            declared(CALL_TWO_NODE, "enter"),
        ),
        connection(
            81_004,
            declared(CALL_TWO_NODE, "then"),
            declared(CALL_TWO_SET_NODE, "enter"),
        ),
        connection(81_005, declared(501, "value"), first_argument),
        connection(81_006, first_result, declared(CALL_ONE_SET_NODE, "value")),
        connection(81_007, declared(502, "value"), second_argument),
        connection(81_008, second_result, declared(CALL_TWO_SET_NODE, "value")),
    ] {
        assert!(event.document.connections.insert(edge.id, edge).is_none());
    }
    event
}

fn insert_persisted_graph(
    fixture: &TempProject,
    path: &GraphResourcePath,
    resource: GraphResourceDocument,
) {
    fixture
        .state()
        .insert_graph(path.clone(), resource)
        .unwrap();
    crate::project::fixtures::write_state_graph(fixture.state(), path).unwrap();
}

fn completed_call_correlations(
    events: &[RunEvent],
    node_value: u128,
    node_type: &str,
) -> Vec<(
    Option<crate::node_system::analysis::RunId>,
    Option<crate::node_system::analysis::ParentCallId>,
)> {
    let expected = source(node_value, node_type);
    events
        .iter()
        .filter(|event| {
            matches!(event.kind, RunEventKind::OperationCompleted { .. })
                && (
                    event.correlation.node_id,
                    event.correlation.node_type_id.clone(),
                ) == (Some(expected.0), Some(expected.1.clone()))
        })
        .map(|event| (event.correlation.run_id, event.correlation.parent_call))
        .collect()
}

fn root_run_id(
    events: &[RunEvent],
    expected_kind: RunEventKind,
) -> crate::node_system::analysis::RunId {
    let matching = events
        .iter()
        .filter(|event| event.correlation.parent_call.is_none() && event.kind == expected_kind)
        .map(|event| event.correlation.run_id)
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), 1, "expected exactly one root run event");
    matching[0].expect("root run event must carry run_id")
}

fn assert_call_correlations_share_root_run(
    correlations: &[(
        Option<crate::node_system::analysis::RunId>,
        Option<crate::node_system::analysis::ParentCallId>,
    )],
    root_run_id: crate::node_system::analysis::RunId,
    expected_count: usize,
) {
    assert_eq!(correlations.len(), expected_count);
    assert!(
        correlations
            .iter()
            .all(|(run_id, _)| *run_id == Some(root_run_id))
    );
    assert_eq!(
        correlations
            .iter()
            .map(|(_, parent_call)| parent_call.expect("callee event must identify its call frame"))
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        expected_count
    );
}

#[test]
fn builtin_call_binds_persisted_argument_and_result_across_distinct_layouts() {
    let output = int64_variable(CALL_ONE_VARIABLE, "Call Output", 0);
    let mut project = ProjectData::new();
    project.variables.insert(output.id, output.clone());
    let fixture = TempProject::activate(project);
    let function_path = GraphResourcePath::new(FUNCTION_PATH).unwrap();
    let event_path = GraphResourcePath::new(CALL_EVENT_PATH).unwrap();
    insert_persisted_graph(
        &fixture,
        &function_path,
        function_fixtures::unary_add_function(&function_path, "Call Production", 1).resource,
    );
    insert_persisted_graph(
        &fixture,
        &event_path,
        single_call_event(&function_path, 41, output.id),
    );
    let events = RecordingRunEvents::default();

    let run = fixture
        .state()
        .execute_graph_for_current_project_for_test(
            &event_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &events,
        )
        .unwrap();

    assert_eq!(
        fixture.state().get_data().unwrap().variables[&output.id].data_value,
        crate::graph::value::DataValue::Int64(42)
    );
    assert_eq!(run.committed_variable_ids.as_ref(), &[output.id]);
    let recorded = events.events();
    let root_run_id = root_run_id(&recorded, RunEventKind::RunStarted);
    assert_eq!(run.run_id, root_run_id);
    assert_call_correlations_share_root_run(
        &completed_call_correlations(&recorded, CALL_BODY_NODE, "yssbi.numeric.add.int64"),
        root_run_id,
        1,
    );
}

#[test]
fn builtin_call_two_calls_in_one_run_keep_arguments_results_and_frames_independent() {
    let first = int64_variable(CALL_ONE_VARIABLE, "First Call", 0);
    let second = int64_variable(CALL_TWO_VARIABLE, "Second Call", 0);
    let mut project = ProjectData::new();
    for variable in [&first, &second] {
        project.variables.insert(variable.id, variable.clone());
    }
    let fixture = TempProject::activate(project);
    let function_path = GraphResourcePath::new(FUNCTION_PATH).unwrap();
    let event_path = GraphResourcePath::new(CALL_EVENT_PATH).unwrap();
    insert_persisted_graph(
        &fixture,
        &function_path,
        function_fixtures::unary_add_function(&function_path, "Two Calls", 0).resource,
    );
    insert_persisted_graph(&fixture, &event_path, two_call_event(&function_path));
    let events = RecordingRunEvents::default();

    let run = fixture
        .state()
        .execute_graph_for_current_project_for_test(
            &event_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &events,
        )
        .unwrap();
    let data = fixture.state().get_data().unwrap();
    let recorded = events.events();

    assert_eq!(
        data.variables[&first.id].data_value,
        crate::graph::value::DataValue::Int64(11)
    );
    assert_eq!(
        data.variables[&second.id].data_value,
        crate::graph::value::DataValue::Int64(22)
    );
    assert_eq!(
        run.committed_variable_ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        [first.id, second.id].into_iter().collect()
    );
    assert_eq!(
        event_count(&recorded, CALL_BODY_NODE, "yssbi.numeric.add.int64", true,),
        2
    );
    let root_run_id = root_run_id(&recorded, RunEventKind::RunStarted);
    assert_eq!(run.run_id, root_run_id);
    assert_call_correlations_share_root_run(
        &completed_call_correlations(&recorded, CALL_BODY_NODE, "yssbi.numeric.add.int64"),
        root_run_id,
        2,
    );
    assert_eq!(
        event_count(
            &recorded,
            CALL_ONE_SET_NODE,
            "yssbi.project.variable.set",
            true,
        ),
        1
    );
    assert_eq!(
        event_count(
            &recorded,
            CALL_TWO_SET_NODE,
            "yssbi.project.variable.set",
            true,
        ),
        1
    );
}

#[test]
fn builtin_call_uses_current_persisted_function_generation_after_body_replacement() {
    let output = int64_variable(CALL_ONE_VARIABLE, "Generation Output", 0);
    let mut project = ProjectData::new();
    project.variables.insert(output.id, output.clone());
    let fixture = TempProject::activate(project);
    let function_path = GraphResourcePath::new(FUNCTION_PATH).unwrap();
    let event_path = GraphResourcePath::new(CALL_EVENT_PATH).unwrap();
    let function = function_fixtures::unary_add_function(&function_path, "Generation", 1);
    let mut replacement = function.resource.document.nodes[&function.offset_node].clone();
    replacement
        .parameters
        .insert(ParameterKey::new("value").unwrap(), serde_json::json!(2));
    insert_persisted_graph(&fixture, &function_path, function.resource);
    insert_persisted_graph(
        &fixture,
        &event_path,
        single_call_event(&function_path, 41, output.id),
    );

    let first = fixture
        .state()
        .execute_graph_for_current_project_for_test(
            &event_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &RecordingRunEvents::default(),
        )
        .unwrap();
    assert_eq!(
        fixture.state().get_data().unwrap().variables[&output.id].data_value,
        crate::graph::value::DataValue::Int64(42)
    );
    let before = fixture.state().get_data().unwrap().graphs[&function_path]
        .document
        .nodes[&function.offset_node]
        .clone();
    fixture
        .state()
        .apply_graph_patch(
            &function_path,
            crate::node_system::document::MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    function_path.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::UpdateNode {
                    before,
                    after: replacement,
                }]),
            ),
        )
        .unwrap();

    let second = fixture
        .state()
        .execute_graph_for_current_project_for_test(
            &event_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &RecordingRunEvents::default(),
        )
        .unwrap();

    assert_ne!(first.provenance.compile_id, second.provenance.compile_id);
    assert_eq!(
        fixture.state().get_data().unwrap().variables[&output.id].data_value,
        crate::graph::value::DataValue::Int64(43)
    );
}

#[test]
fn builtin_recursive_call_stops_at_project_recursion_limit() {
    let output = int64_variable(CALL_ONE_VARIABLE, "Recursive Output", 0);
    let mut project = ProjectData::new();
    project.variables.insert(output.id, output.clone());
    let fixture = TempProject::activate(project);
    let function_path = GraphResourcePath::new(FUNCTION_PATH).unwrap();
    let event_path = GraphResourcePath::new(CALL_EVENT_PATH).unwrap();
    insert_persisted_graph(
        &fixture,
        &function_path,
        function_fixtures::recursive_function(&function_path, "Recursive"),
    );
    insert_persisted_graph(
        &fixture,
        &event_path,
        single_call_event(&function_path, 1, output.id),
    );
    let events = RecordingRunEvents::default();
    let (runs, _) = fixture.state().current_run_registry();

    let error = fixture
        .state()
        .execute_graph_for_current_project_for_test(
            &event_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &events,
        )
        .unwrap_err();
    let recorded = events.events();

    assert_eq!(error.to_string(), "call recursion limit exceeded");
    assert!(matches!(
        error.run_error(),
        Some(
            crate::node_system::runtime::RunError::RecursionLimitExceeded {
                recursion_limit: 64
            }
        )
    ));
    let root_started_run_id = root_run_id(&recorded, RunEventKind::RunStarted);
    let root_errored_run_id = root_run_id(
        &recorded,
        RunEventKind::RunErrored {
            outcome: RunErrorOutcome::Ordinary {
                code: OrdinaryRunErrorCode::RecursionLimitExceeded,
            },
        },
    );
    assert_eq!(root_started_run_id, root_errored_run_id);
    let completed_frames = completed_call_correlations(
        &recorded,
        function_fixtures::RECURSIVE_DO_NODE,
        "yssbi.control.do",
    );
    assert_call_correlations_share_root_run(&completed_frames, root_started_run_id, 63);
    assert_eq!(
        1 + completed_frames.len(),
        64,
        "root frame plus successful child function frames must fill the limit"
    );
    assert!(recorded.iter().any(|event| {
        event.kind
            == RunEventKind::RunErrored {
                outcome: RunErrorOutcome::Ordinary {
                    code: OrdinaryRunErrorCode::RecursionLimitExceeded,
                },
            }
    }));
    assert_eq!(
        fixture.state().get_data().unwrap().variables[&output.id].data_value,
        crate::graph::value::DataValue::Int64(0)
    );
    assert_eq!(
        event_count(
            &recorded,
            CALL_ONE_SET_NODE,
            "yssbi.project.variable.set",
            true,
        ),
        0
    );
    assert!(
        recorded
            .iter()
            .all(|event| !matches!(event.kind, RunEventKind::ResultReady { .. }))
    );
    assert!(
        recorded
            .iter()
            .all(|event| event.kind != RunEventKind::RunCompleted)
    );
    assert_eq!(runs.active_run_count(), 0);
}

#[test]
fn builtin_branch_executes_only_selected_effect_branch_and_binds_result() {
    let outcome = run_branch(branch_fixture(true, InsertionOrder::Forward).resource);

    assert_branch_outcome(&outcome, 11, TRUE_EFFECT_NODE, FALSE_EFFECT_NODE);
}

#[test]
fn builtin_branch_false_path_executes_only_selected_effect_and_binds_result() {
    let outcome = run_branch(branch_fixture(false, InsertionOrder::Forward).resource);

    assert_branch_outcome(&outcome, 22, FALSE_EFFECT_NODE, TRUE_EFFECT_NODE);
}

#[test]
fn builtin_branch_commit_conflict_publishes_no_result_or_completion() {
    let variable = result_variable();
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    let fixture = TempProject::activate(project);
    let graph_path = GraphResourcePath::new(EVENT_PATH).unwrap();
    fixture
        .state()
        .insert_graph(
            graph_path.clone(),
            branch_fixture(true, InsertionOrder::Forward).resource,
        )
        .unwrap();
    crate::project::fixtures::write_state_graph(fixture.state(), &graph_path).unwrap();
    let winning_state = fixture.state().clone();
    let winning_variable = variable.clone();
    let (_, session) = fixture.state().current_run_registry();
    fixture
        .state()
        .set_execution_before_commit_gate_test_hook(std::sync::Arc::new(move || {
            winning_state
                .commit_variable_effects(
                    &session,
                    vec![crate::node_system::runtime::VariableWriteEffect {
                        resource: crate::node_system::plan::ResourceId::new(format!(
                            "variables/{}",
                            winning_variable.id
                        ))
                        .unwrap(),
                        expected_revision: GraphRevision::INITIAL,
                        before: winning_variable.clone(),
                        after: crate::graph::value::DataValue::Int64(99),
                    }],
                )
                .unwrap();
        }));
    let events = RecordingRunEvents::default();

    let error = fixture
        .state()
        .execute_graph_for_current_project_for_test(
            &graph_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &events,
        )
        .expect_err("the staged Variable Set effect must lose the revision race");
    fixture
        .state()
        .set_execution_before_commit_gate_test_hook(std::sync::Arc::new(|| {}));
    let recorded = events.events();

    assert_eq!(error.to_string(), "project resource snapshot changed");
    assert!(matches!(
        error.run_error(),
        Some(crate::node_system::runtime::RunError::ResourceSnapshotMismatch(message))
            if message.contains("revision")
    ));
    assert_eq!(
        fixture.state().get_data().unwrap().variables[&variable.id].data_value,
        crate::graph::value::DataValue::Int64(99)
    );
    assert_no_completed_result(&recorded);
}

#[test]
fn builtin_branch_drain_before_commit_gate_commits_nothing() {
    let variable = result_variable();
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    let fixture = TempProject::activate(project);
    let graph_path = GraphResourcePath::new(EVENT_PATH).unwrap();
    fixture
        .state()
        .insert_graph(
            graph_path.clone(),
            branch_fixture(true, InsertionOrder::Forward).resource,
        )
        .unwrap();
    crate::project::fixtures::write_state_graph(fixture.state(), &graph_path).unwrap();
    let (gate_reached_tx, gate_reached_rx) = std::sync::mpsc::channel();
    let (release_gate_tx, release_gate_rx) = std::sync::mpsc::channel();
    let release_gate_rx = Mutex::new(release_gate_rx);
    fixture
        .state()
        .set_execution_before_commit_gate_test_hook(std::sync::Arc::new(move || {
            gate_reached_tx.send(()).unwrap();
            release_gate_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .expect("test must release the commit gate");
        }));
    let events = std::sync::Arc::new(RecordingRunEvents::default());
    let execution_state = fixture.state().clone();
    let execution_path = graph_path.clone();
    let execution_events = std::sync::Arc::clone(&events);
    let execution = std::thread::spawn(move || {
        execution_state.execute_graph_for_current_project_for_test(
            &execution_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            execution_events.as_ref(),
        )
    });

    gate_reached_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("execution must reach the commit gate");
    let (runs, session) = fixture.state().current_run_registry();
    let drain_runs = std::sync::Arc::clone(&runs);
    let drain_session = session.clone();
    let (drained_tx, drained_rx) = std::sync::mpsc::channel();
    let drain = std::thread::spawn(move || {
        drain_runs.cancel_and_drain(&drain_session);
        drained_tx.send(()).unwrap();
    });
    assert!(runs.wait_until_draining_for_test(&session, Duration::from_secs(5)));
    release_gate_tx.send(()).unwrap();
    let error = execution.join().unwrap().unwrap_err();
    drained_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("drain must finish after execution rejects finalization");
    drain.join().unwrap();
    let recorded = events.events();

    assert!(error.contains("project is draining"));
    assert_eq!(
        fixture.state().get_data().unwrap().variables[&variable.id].data_value,
        crate::graph::value::DataValue::Int64(0)
    );
    assert_no_completed_result(&recorded);
}

#[test]
fn builtin_branch_reverse_insertion_is_equivalent() {
    let forward = branch_fixture(true, InsertionOrder::Forward);
    let reverse = branch_fixture(true, InsertionOrder::Reverse);

    assert_reversed_insertions(&forward.node_insertions, &reverse.node_insertions);
    assert_reversed_insertions(
        &forward.connection_insertions,
        &reverse.connection_insertions,
    );
    assert_reversed_insertions(&forward.binding_insertions, &reverse.binding_insertions);
    assert_eq!(forward.resource.document, reverse.resource.document);

    let forward = run_branch(forward.resource);
    let reverse = run_branch(reverse.resource);
    assert_branch_outcome(&forward, 11, TRUE_EFFECT_NODE, FALSE_EFFECT_NODE);
    assert_branch_outcome(&reverse, 11, TRUE_EFFECT_NODE, FALSE_EFFECT_NODE);
    assert_eq!(forward.result, reverse.result);
    assert_eq!(
        forward.committed_variable_ids,
        reverse.committed_variable_ids
    );
    assert_eq!(
        forward.normalized_completion_sequence(),
        reverse.normalized_completion_sequence()
    );
}

#[test]
fn builtin_loop_carries_initial_and_subsequent_values_across_observable_iterations() {
    let result = float64_result_variable("Loop Result");
    let first_observer = float64_variable(variable_id(FIRST_OBSERVER_VARIABLE), "First Observer");
    let second_observer =
        float64_variable(variable_id(SECOND_OBSERVER_VARIABLE), "Second Observer");
    let mut project = ProjectData::new();
    for variable in [&result, &first_observer, &second_observer] {
        project.variables.insert(variable.id, variable.clone());
    }
    let fixture = TempProject::activate(project);
    let graph_path = GraphResourcePath::new(EVENT_PATH).unwrap();
    fixture
        .state()
        .insert_graph(graph_path.clone(), carried_observation_loop_fixture())
        .unwrap();
    crate::project::fixtures::write_state_graph(fixture.state(), &graph_path).unwrap();
    let events = RecordingRunEvents::default();

    let run = fixture
        .state()
        .execute_graph_for_current_project_for_test(
            &graph_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &events,
        )
        .unwrap();
    let data = fixture.state().get_data().unwrap();
    let recorded = events.events();

    assert_eq!(
        data.variables[&first_observer.id].data_value,
        crate::graph::value::DataValue::Float64(1.5)
    );
    assert_eq!(
        data.variables[&second_observer.id].data_value,
        crate::graph::value::DataValue::Float64(22.5)
    );
    assert_eq!(
        data.variables[&result.id].data_value,
        crate::graph::value::DataValue::Float64(22.5)
    );
    assert_eq!(run.committed_variable_ids.len(), 3);
    assert_eq!(
        event_count(
            &recorded,
            LOOP_FIRST_OBSERVER_SET_NODE,
            "yssbi.project.variable.set",
            true,
        ),
        1
    );
    assert_eq!(
        event_count(
            &recorded,
            LOOP_SECOND_OBSERVER_SET_NODE,
            "yssbi.project.variable.set",
            true,
        ),
        1
    );
    assert_eq!(
        event_count(
            &recorded,
            LOOP_RESULT_SET_NODE,
            "yssbi.project.variable.set",
            true,
        ),
        1
    );
    assert!(
        recorded
            .iter()
            .any(|event| event.kind == RunEventKind::RunCompleted)
    );
}

#[test]
fn builtin_loop_reports_iteration_limit_without_committing_result() {
    let variable = float64_result_variable("Loop Limit Result");
    let (run, result, events) = run_loop(loop_fixture(true, "yssbi.control.do", 3), variable);

    assert_eq!(run.unwrap_err(), "loop iteration limit exceeded");
    assert_eq!(result, crate::graph::value::DataValue::Float64(0.0));
    assert_eq!(
        event_count(&events, LOOP_BODY_NODE, "yssbi.control.do", true),
        3
    );
    assert!(events.iter().any(|event| {
        event.kind
            == RunEventKind::RunErrored {
                outcome: RunErrorOutcome::Ordinary {
                    code: OrdinaryRunErrorCode::LoopLimitExceeded,
                },
            }
    }));
    assert_no_completed_result(&events);
}

struct CancellationRunEvents {
    events: Mutex<Vec<RunEvent>>,
    first_body_completed: Mutex<Option<std::sync::mpsc::Sender<()>>>,
    release_body: Mutex<std::sync::mpsc::Receiver<()>>,
}

impl RunEventSink for CancellationRunEvents {
    fn record(&self, event: RunEvent) {
        let is_first_body_completion = event.correlation.node_id == Some(node_id(LOOP_BODY_NODE))
            && event.correlation.node_type_id == Some(NodeTypeId::new("yssbi.control.do").unwrap())
            && matches!(event.kind, RunEventKind::OperationCompleted { .. });
        self.events.lock().unwrap().push(event);
        if is_first_body_completion
            && let Some(sender) = self.first_body_completed.lock().unwrap().take()
        {
            sender.send(()).unwrap();
            self.release_body
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .expect("test must release the blocked body completion callback");
        }
    }
}

#[test]
fn builtin_loop_project_drain_cancels_between_iterations_without_publishing_result() {
    let variable = float64_result_variable("Loop Cancellation Result");
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    let fixture = TempProject::activate(project);
    let graph_path = GraphResourcePath::new(EVENT_PATH).unwrap();
    fixture
        .state()
        .insert_graph(
            graph_path.clone(),
            loop_fixture(true, "yssbi.control.do", 10),
        )
        .unwrap();
    crate::project::fixtures::write_state_graph(fixture.state(), &graph_path).unwrap();
    let (first_body_completed_tx, first_body_completed_rx) = std::sync::mpsc::channel();
    let (release_body_tx, release_body_rx) = std::sync::mpsc::channel();
    let events = std::sync::Arc::new(CancellationRunEvents {
        events: Mutex::new(Vec::new()),
        first_body_completed: Mutex::new(Some(first_body_completed_tx)),
        release_body: Mutex::new(release_body_rx),
    });
    let execution_state = fixture.state().clone();
    let execution_path = graph_path.clone();
    let execution_events = std::sync::Arc::clone(&events);
    let execution = std::thread::spawn(move || {
        execution_state.execute_graph_for_current_project_for_test(
            &execution_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            execution_events.as_ref(),
        )
    });

    first_body_completed_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("first body completion must be observed");
    let (runs, session) = fixture.state().current_run_registry();
    let draining_runs = std::sync::Arc::clone(&runs);
    let draining_session = session.clone();
    let (drained_tx, drained_rx) = std::sync::mpsc::channel();
    let drain = std::thread::spawn(move || {
        draining_runs.cancel_and_drain(&draining_session);
        drained_tx.send(()).unwrap();
    });
    assert!(
        runs.wait_until_draining_for_test(&session, Duration::from_secs(5)),
        "project run registry must enter draining"
    );
    release_body_tx.send(()).unwrap();
    drained_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("project drain must finish after the run observes cancellation");
    drain.join().unwrap();
    let run = execution.join().unwrap();
    let recorded = events.events.lock().unwrap().clone();

    assert_eq!(run.unwrap_err(), "run was cancelled");
    assert_eq!(runs.active_run_count(), 0);
    assert_eq!(
        event_count(&recorded, LOOP_BODY_NODE, "yssbi.control.do", false),
        1
    );
    assert_eq!(
        event_count(&recorded, LOOP_BODY_NODE, "yssbi.control.do", true),
        1
    );
    assert_eq!(
        fixture.state().get_data().unwrap().variables[&variable.id].data_value,
        crate::graph::value::DataValue::Float64(0.0)
    );
    assert!(
        recorded
            .iter()
            .any(|event| event.kind == RunEventKind::RunCancelled)
    );
    assert_no_completed_result(&recorded);
}

fn effect_order_fixture(insertion_order: InsertionOrder) -> (GraphResourceDocument, Vec<NodeId>) {
    let mut resource = GraphResourceDocument::new("Effect Ordering", GraphDocumentKind::Event);
    let mut node_insertions = Vec::new();
    for entry in in_order(
        vec![
            node(EFFECT_AFTER_NODE, "yssbi.control.do"),
            node(EFFECT_BEFORE_NODE, "yssbi.control.do"),
        ],
        insertion_order,
    ) {
        insert_tracked(
            &mut resource.document.nodes,
            &mut node_insertions,
            entry.id,
            entry,
        );
    }
    let edge = connection(
        90_001,
        declared(EFFECT_BEFORE_NODE, "effect_out"),
        declared(EFFECT_AFTER_NODE, "effect_in"),
    );
    assert!(
        resource
            .document
            .connections
            .insert(edge.id, edge)
            .is_none()
    );
    (resource, node_insertions)
}

fn project_variable_get_node(value: u128, variable: VariableId) -> DocumentNode {
    let mut get = node(value, "yssbi.project.variable.get");
    get.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{variable}")),
    );
    get
}

fn effect_failure_fixture(
    first_resource: VariableId,
    second_resource: VariableId,
    result: VariableId,
) -> GraphResourceDocument {
    let mut resource = GraphResourceDocument::new("Effect Failure", GraphDocumentKind::Event);
    for entry in [
        project_variable_get_node(EFFECT_RESOURCE_NODE, first_resource),
        project_variable_get_node(EFFECT_RESOURCE_NODE + 1, second_resource),
        int64_constant(810, 1),
        int64_constant(811, 0),
        node(EFFECT_DIVIDE_NODE, "yssbi.numeric.divide.int64"),
        variable_set_node(EFFECT_FINAL_SET_NODE, result),
    ] {
        assert!(resource.document.nodes.insert(entry.id, entry).is_none());
    }
    for edge in [
        connection(
            91_001,
            declared(810, "value"),
            declared(EFFECT_DIVIDE_NODE, "left"),
        ),
        connection(
            91_002,
            declared(811, "value"),
            declared(EFFECT_DIVIDE_NODE, "right"),
        ),
        connection(
            91_003,
            declared(EFFECT_DIVIDE_NODE, "result"),
            declared(EFFECT_FINAL_SET_NODE, "value"),
        ),
    ] {
        assert!(
            resource
                .document
                .connections
                .insert(edge.id, edge)
                .is_none()
        );
    }
    resource
}

fn effect_cancellation_fixture(
    first_resource: VariableId,
    second_resource: VariableId,
) -> GraphResourceDocument {
    let mut resource = loop_fixture(true, "yssbi.control.do", 10);
    for entry in [
        project_variable_get_node(EFFECT_RESOURCE_NODE, first_resource),
        project_variable_get_node(EFFECT_RESOURCE_NODE + 1, second_resource),
    ] {
        assert!(resource.document.nodes.insert(entry.id, entry).is_none());
    }
    resource
}

fn activate_effect_fixture(
    document: GraphResourceDocument,
    variables: impl IntoIterator<Item = VariableInstance>,
) -> (TempProject, GraphResourcePath, ProjectResourceLeaseObserver) {
    let mut project = ProjectData::new();
    for variable in variables {
        assert!(project.variables.insert(variable.id, variable).is_none());
    }
    let fixture = TempProject::activate(project);
    let graph_path = GraphResourcePath::new(EVENT_PATH).unwrap();
    insert_persisted_graph(&fixture, &graph_path, document);
    let observer = ProjectResourceLeaseObserver::default();
    fixture
        .state()
        .set_project_resource_lease_observer(observer.clone());
    (fixture, graph_path, observer)
}

fn operation_event_sequence(events: &[RunEvent]) -> Vec<(NodeId, &'static str)> {
    events
        .iter()
        .filter_map(|event| {
            let kind = match event.kind {
                RunEventKind::OperationStarted { .. } => "started",
                RunEventKind::OperationCompleted { .. } => "completed",
                _ => return None,
            };
            Some((event.correlation.node_id.expect("operation node"), kind))
        })
        .collect()
}

#[test]
fn builtin_effect_edge_orders_real_builtins_independent_of_document_insertion() {
    let (after_before, after_before_insertions) = effect_order_fixture(InsertionOrder::Forward);
    let (before_after, before_after_insertions) = effect_order_fixture(InsertionOrder::Reverse);
    assert_reversed_insertions(&after_before_insertions, &before_after_insertions);

    for document in [after_before, before_after] {
        let fixture = TempProject::activate(ProjectData::new());
        let graph_path = GraphResourcePath::new(EVENT_PATH).unwrap();
        insert_persisted_graph(&fixture, &graph_path, document);
        let events = RecordingRunEvents::default();

        fixture
            .state()
            .execute_graph_for_current_project_for_test(
                &graph_path,
                &crate::node_system::plan::ExecutionDemand::Default,
                &events,
            )
            .unwrap();

        assert_eq!(
            operation_event_sequence(&events.events()),
            vec![
                (node_id(EFFECT_BEFORE_NODE), "started"),
                (node_id(EFFECT_BEFORE_NODE), "completed"),
                (node_id(EFFECT_AFTER_NODE), "started"),
                (node_id(EFFECT_AFTER_NODE), "completed"),
            ]
        );
    }
}

#[test]
fn builtin_effect_failure_attempts_once_and_drops_every_retained_project_resource() {
    let first_resource = int64_variable(EFFECT_RESOURCE_VARIABLE, "First Failure Resource", 7);
    let second_resource = int64_variable(
        EFFECT_SECOND_RESOURCE_VARIABLE,
        "Second Failure Resource",
        8,
    );
    let result = int64_variable(EFFECT_RESULT_VARIABLE, "Failure Result", 0);
    let document = effect_failure_fixture(first_resource.id, second_resource.id, result.id);
    let (fixture, graph_path, observer) =
        activate_effect_fixture(document, [first_resource, second_resource, result.clone()]);
    let events = RecordingRunEvents::default();

    let error = fixture
        .state()
        .execute_graph_for_current_project_for_test(
            &graph_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &events,
        )
        .unwrap_err();

    assert_eq!(error.to_string(), "operation failed");
    assert!(matches!(
        error.run_error(),
        Some(crate::node_system::runtime::RunError::KernelFailed { message, .. })
            if message.contains("int64 division by zero")
    ));
    assert_eq!(
        event_count(
            &events.events(),
            EFFECT_DIVIDE_NODE,
            "yssbi.numeric.divide.int64",
            false,
        ),
        1
    );
    let recorded = events.events();
    assert_eq!(
        event_count(
            &recorded,
            EFFECT_DIVIDE_NODE,
            "yssbi.numeric.divide.int64",
            true,
        ),
        0
    );
    assert_eq!(
        recorded
            .iter()
            .filter(|event| {
                event.correlation.node_id == Some(node_id(EFFECT_DIVIDE_NODE))
                    && matches!(
                        event.kind,
                        RunEventKind::OperationErrored {
                            outcome: RunErrorOutcome::Ordinary {
                                code: OrdinaryRunErrorCode::KernelFailed,
                            },
                            ..
                        }
                    )
            })
            .count(),
        1
    );
    assert_eq!(
        recorded
            .iter()
            .filter(|event| {
                event.kind
                    == RunEventKind::RunErrored {
                        outcome: RunErrorOutcome::Ordinary {
                            code: OrdinaryRunErrorCode::KernelFailed,
                        },
                    }
            })
            .count(),
        1
    );
    assert_eq!(
        recorded
            .iter()
            .filter(|event| event.kind == RunEventKind::RunCompleted)
            .count(),
        0
    );
    assert_eq!(
        recorded
            .iter()
            .filter(|event| matches!(event.kind, RunEventKind::ResultReady { .. }))
            .count(),
        0
    );
    assert_eq!(
        event_count(
            &recorded,
            EFFECT_FINAL_SET_NODE,
            "yssbi.project.variable.set",
            true,
        ),
        0
    );
    assert_eq!(
        fixture.state().get_data().unwrap().variables[&result.id].data_value,
        crate::graph::value::DataValue::Int64(0)
    );
    for resource_node in [EFFECT_RESOURCE_NODE, EFFECT_RESOURCE_NODE + 1] {
        assert_eq!(
            event_count(
                &recorded,
                resource_node,
                "yssbi.project.variable.get",
                false,
            ),
            0,
        );
    }
    assert_eq!(observer.acquired(), 1);
    assert_eq!(observer.dropped(), 1);
    assert_eq!(observer.active(), 0);
}

#[test]
fn builtin_effect_cancellation_attempts_once_drops_retained_resources_and_drains_run() {
    let first_resource = int64_variable(EFFECT_RESOURCE_VARIABLE, "First Cancel Resource", 7);
    let second_resource =
        int64_variable(EFFECT_SECOND_RESOURCE_VARIABLE, "Second Cancel Resource", 8);
    let result = float64_result_variable("Cancellation Result");
    let document = effect_cancellation_fixture(first_resource.id, second_resource.id);
    let (fixture, graph_path, observer) =
        activate_effect_fixture(document, [first_resource, second_resource, result.clone()]);
    let (first_body_completed_tx, first_body_completed_rx) = std::sync::mpsc::channel();
    let (release_body_tx, release_body_rx) = std::sync::mpsc::channel();
    let events = std::sync::Arc::new(CancellationRunEvents {
        events: Mutex::new(Vec::new()),
        first_body_completed: Mutex::new(Some(first_body_completed_tx)),
        release_body: Mutex::new(release_body_rx),
    });
    let execution_state = fixture.state().clone();
    let execution_path = graph_path.clone();
    let execution_events = std::sync::Arc::clone(&events);
    let execution = std::thread::spawn(move || {
        execution_state.execute_graph_for_current_project_for_test(
            &execution_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            execution_events.as_ref(),
        )
    });

    first_body_completed_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("first Do completion must reach the event boundary");
    let (runs, session) = fixture.state().current_run_registry();
    let drain_runs = std::sync::Arc::clone(&runs);
    let drain_session = session.clone();
    let (drained_tx, drained_rx) = std::sync::mpsc::channel();
    let drain = std::thread::spawn(move || {
        drain_runs.cancel_and_drain(&drain_session);
        drained_tx.send(()).unwrap();
    });
    assert!(
        runs.wait_until_draining_for_test(&session, Duration::from_secs(5)),
        "project run registry must enter draining"
    );
    release_body_tx.send(()).unwrap();
    drained_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("project drain must finish after the iteration observes cancellation");
    drain.join().unwrap();
    let error = execution.join().unwrap().unwrap_err();
    let recorded = events.events.lock().unwrap().clone();

    assert_eq!(error, "run was cancelled");
    assert_eq!(
        event_count(&recorded, LOOP_BODY_NODE, "yssbi.control.do", false),
        1
    );
    assert_eq!(
        event_count(&recorded, LOOP_BODY_NODE, "yssbi.control.do", true),
        1
    );
    assert_eq!(
        recorded
            .iter()
            .filter(|event| event.kind == RunEventKind::RunCancelled)
            .count(),
        1
    );
    assert_eq!(
        recorded
            .iter()
            .filter(|event| event.kind == RunEventKind::RunCompleted)
            .count(),
        0
    );
    assert_eq!(
        recorded
            .iter()
            .filter(|event| matches!(event.kind, RunEventKind::ResultReady { .. }))
            .count(),
        0
    );
    assert_eq!(
        event_count(
            &recorded,
            LOOP_RESULT_SET_NODE,
            "yssbi.project.variable.set",
            true,
        ),
        0
    );
    assert_eq!(
        fixture.state().get_data().unwrap().variables[&result.id].data_value,
        crate::graph::value::DataValue::Float64(0.0)
    );
    for resource_node in [EFFECT_RESOURCE_NODE, EFFECT_RESOURCE_NODE + 1] {
        assert_eq!(
            event_count(
                &recorded,
                resource_node,
                "yssbi.project.variable.get",
                false,
            ),
            0,
        );
    }
    assert_eq!(observer.acquired(), 1);
    assert_eq!(observer.dropped(), 1);
    assert_eq!(observer.active(), 0);
    assert_eq!(runs.active_run_count(), 0);
}
