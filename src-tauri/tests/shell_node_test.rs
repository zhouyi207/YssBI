//! 壳节点协议（graph_scope + shell_role）测试

use std::sync::Arc;
use yssbi_lib::graph::{
    FunctionSignaturePin, GraphKind, core::GraphInstance, pin::PinRole, register::NodeRegistry,
};

const EVENT_BEGIN: &str = "Event:Event Begin";
const FUNCTION_ENTRY: &str = "Functions:Function Entry";
const FUNCTION_RETURN: &str = "Functions:Function Return";

fn test_registry() -> Arc<NodeRegistry> {
    let registry = Arc::new(NodeRegistry::new());
    yssbi_lib::graph::register::catalog::register_builtin_nodes(&registry);
    registry
}

fn sig(id: &str, name: &str, pin_type: &str) -> FunctionSignaturePin {
    FunctionSignaturePin {
        id: id.to_string(),
        name: name.to_string(),
        pin_type: pin_type.to_string(),
        container_type: None,
    }
}

#[test]
fn event_begin_is_shell_and_singleton_in_event_graph() {
    let registry = test_registry();
    let graph = GraphInstance::new("E", GraphKind::Event, registry);

    let node_id = graph
        .create_node(EVENT_BEGIN)
        .expect("Event Begin should be creatable in an Event graph");

    assert!(
        graph.is_shell_node(node_id),
        "Event Begin must be flagged as a shell node"
    );

    // 每图至多一个壳节点。
    let second = graph.create_node(EVENT_BEGIN);
    assert!(
        second.is_err(),
        "a second Event Begin must be rejected (singleton)"
    );
}

#[test]
fn event_begin_rejected_in_function_graph_by_scope() {
    let registry = test_registry();
    let graph = GraphInstance::new("F", GraphKind::Function, registry);

    let result = graph.create_node(EVENT_BEGIN);
    assert!(
        result.is_err(),
        "Event-scoped node must not be creatable in a Function graph"
    );
}

#[test]
fn regular_node_is_not_shell_and_allowed_anywhere() {
    let registry = test_registry();
    let graph = GraphInstance::new("F", GraphKind::Function, registry);

    let node_id = graph
        .create_node("Debug:Print")
        .expect("Any-scope node should be creatable in any graph kind");
    assert!(
        !graph.is_shell_node(node_id),
        "regular nodes must not be shell nodes"
    );
}

#[test]
fn function_entry_projects_inputs_as_output_pins() {
    let registry = test_registry();
    let mut graph = GraphInstance::new("F", GraphKind::Function, registry);

    let entry = graph.create_node(FUNCTION_ENTRY).expect("seed entry");
    assert!(graph.is_shell_node(entry));
    assert!(
        graph.get_pin_instances_by_node_id(entry).is_empty(),
        "entry starts with no projected pins"
    );

    graph.function_inputs = vec![sig("s1", "count", "int"), sig("s2", "label", "string")];
    graph.sync_function_shell_pins();

    let all_pins = graph.get_pin_instances_by_node_id(entry);
    // 仅数据签名：Entry 无 exec pin。
    assert!(
        !all_pins.iter().any(|p| p.is_exec()),
        "data-only entry should have no exec pins"
    );

    let data_pins: Vec<_> = all_pins.iter().filter(|p| p.is_data()).collect();
    assert_eq!(data_pins.len(), 2, "two inputs should project to two data pins");
    for pin in &data_pins {
        assert!(
            matches!(
                pin.definition.direction,
                yssbi_lib::graph::PinDirection::Output
            ),
            "entry pins project inputs as OUTPUT pins"
        );
    }
    let names: Vec<&str> = data_pins.iter().map(|p| p.definition.name.as_str()).collect();
    assert!(names.contains(&"count") && names.contains(&"label"));
}

#[test]
fn function_signature_rename_preserves_pin_identity() {
    let registry = test_registry();
    let mut graph = GraphInstance::new("F", GraphKind::Function, registry);
    let entry = graph.create_node(FUNCTION_ENTRY).expect("seed entry");

    let data_pins = |g: &GraphInstance| -> Vec<yssbi_lib::graph::PinInstance> {
        g.get_pin_instances_by_node_id(entry)
            .into_iter()
            .filter(|p| p.is_data())
            .collect()
    };

    graph.function_inputs = vec![sig("s1", "count", "int")];
    graph.sync_function_shell_pins();
    let pin_id_before = data_pins(&graph)[0].id;

    // Same signature id, new name + type: pin identity (id) must be preserved.
    graph.function_inputs = vec![sig("s1", "renamed", "float")];
    graph.sync_function_shell_pins();

    let pins = data_pins(&graph);
    assert_eq!(pins.len(), 1);
    assert_eq!(
        pins[0].id, pin_id_before,
        "renaming must keep the same pin id"
    );
    assert_eq!(pins[0].definition.name, "renamed");

    // Removing the signature pin drops the projected data pin.
    graph.function_inputs = vec![];
    graph.sync_function_shell_pins();
    assert!(data_pins(&graph).is_empty());
}

#[test]
fn function_entry_projects_exec_from_signature() {
    let registry = test_registry();
    let mut graph = GraphInstance::new("F", GraphKind::Function, registry);
    let entry = graph.create_node(FUNCTION_ENTRY).expect("seed entry");

    graph.function_inputs = vec![sig("flow", "Then", "exec"), sig("s1", "count", "int")];
    graph.sync_function_shell_pins();

    let all_pins = graph.get_pin_instances_by_node_id(entry);
    assert!(
        all_pins.iter().any(|p| {
            p.is_exec()
                && p.is_output()
                && p.definition.role
                    == PinRole::Exec(yssbi_lib::graph::pin::ExecRole::Custom("flow".into()))
        }),
        "exec signature item should project as exec output on entry"
    );
    assert_eq!(all_pins.iter().filter(|p| p.is_data()).count(), 1);
}

#[test]
fn function_return_projects_outputs_as_input_pins() {
    let registry = test_registry();
    let mut graph = GraphInstance::new("F", GraphKind::Function, registry);
    let ret = graph.create_node(FUNCTION_RETURN).expect("seed return");

    graph.function_outputs = vec![sig("r1", "result", "float")];
    graph.sync_function_shell_pins();

    let all_pins = graph.get_pin_instances_by_node_id(ret);
    assert!(
        !all_pins.iter().any(|p| p.is_exec() && p.is_input()),
        "data-only return should have no exec input"
    );

    let data_pins: Vec<_> = all_pins.iter().filter(|p| p.is_data()).collect();
    assert_eq!(data_pins.len(), 1);
    assert!(
        matches!(
            data_pins[0].definition.direction,
            yssbi_lib::graph::PinDirection::Input
        ),
        "return pins project outputs as INPUT pins"
    );
    assert_eq!(data_pins[0].definition.name, "result");
}
