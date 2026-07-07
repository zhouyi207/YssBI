//! 函数调用集成测试：Call Function 执行、签名 exec 投影、pin 同步。

use std::sync::{Arc, Mutex};
use yssbi_lib::execution::{Executor, NoopEmitter, ResultSourceStore};
use yssbi_lib::graph::core::{
    DEFAULT_FUNCTION_EXEC_IN_ID, DEFAULT_FUNCTION_EXEC_OUT_ID, GraphInstance, GraphRuntime,
};
use yssbi_lib::graph::node::NodeInstanceParams;
use yssbi_lib::graph::pin::{DataRole, ExecRole, PinRole};
use yssbi_lib::graph::value::DataValue;
use yssbi_lib::graph::{FunctionSignaturePin, NodeId, PinId};
use yssbi_lib::graph::register::event::EVENT_BEGIN_NODE_TYPE;
use yssbi_lib::graph::register::function::{FUNCTION_ENTRY_NODE_TYPE, FUNCTION_RETURN_NODE_TYPE};
use yssbi_lib::graph::register::value::call::CALL_FUNCTION_NODE_TYPE;
use yssbi_lib::project::ProjectState;

fn sig(id: &str, name: &str, pin_type: &str) -> FunctionSignaturePin {
    FunctionSignaturePin {
        id: id.to_string(),
        name: name.to_string(),
        pin_type: pin_type.to_string(),
        container_type: None,
    }
}

fn exec_in_role() -> PinRole {
    PinRole::Exec(ExecRole::Custom(DEFAULT_FUNCTION_EXEC_IN_ID.to_string()))
}

fn exec_out_role() -> PinRole {
    PinRole::Exec(ExecRole::Custom(DEFAULT_FUNCTION_EXEC_OUT_ID.to_string()))
}

fn pin_by_role(graph: &GraphInstance, node: NodeId, role: &PinRole) -> PinId {
    graph
        .get_pin_instances_by_node_id(node)
        .into_iter()
        .find(|p| &p.definition.role == role)
        .unwrap_or_else(|| panic!("pin with role {:?} not found on node {:?}", role, node))
        .id
}

fn has_role(graph: &GraphInstance, node: NodeId, role: &PinRole) -> bool {
    graph
        .get_pin_instances_by_node_id(node)
        .into_iter()
        .any(|p| &p.definition.role == role)
}

/// 建一个恒等函数（input a -> output r）。`with_exec` 控制签名是否含 exec 入/出参。
fn build_identity_function(
    state: &ProjectState,
    pin_type: &str,
    with_exec: bool,
) -> yssbi_lib::graph::GraphId {
    let func = state.add_function("Identity");
    let entry = func
        .create_node_with_position(FUNCTION_ENTRY_NODE_TYPE, 0.0, 0.0, None)
        .expect("seed entry");
    let ret = func
        .create_node_with_position(FUNCTION_RETURN_NODE_TYPE, 0.0, 0.0, None)
        .expect("seed return");

    let inputs = if with_exec {
        vec![
            sig(DEFAULT_FUNCTION_EXEC_IN_ID, "In", "exec"),
            sig("a", "a", pin_type),
        ]
    } else {
        vec![sig("a", "a", pin_type)]
    };
    let outputs = if with_exec {
        vec![
            sig(DEFAULT_FUNCTION_EXEC_OUT_ID, "Out", "exec"),
            sig("r", "r", pin_type),
        ]
    } else {
        vec![sig("r", "r", pin_type)]
    };

    state
        .update_function_signature(&func.id, Some(inputs), Some(outputs))
        .expect("update signature");

    let func_graph = state.get_graph(&func.id).expect("function loaded");

    let entry_a = pin_by_role(&func_graph, entry, &PinRole::Data(DataRole::Custom("a".into())));
    let ret_r = pin_by_role(&func_graph, ret, &PinRole::Data(DataRole::Custom("r".into())));
    func_graph.connect(entry_a, ret_r).expect("connect data");

    if with_exec {
        let entry_exec = pin_by_role(&func_graph, entry, &exec_in_role());
        let ret_exec = pin_by_role(&func_graph, ret, &exec_out_role());
        func_graph.connect(entry_exec, ret_exec).expect("connect exec");
    }

    func.id
}

#[test]
fn exec_call_passes_value_through_control_flow() {
    let state = ProjectState::new();
    let func_id = build_identity_function(&state, "int", true);

    let event = state.add_event("Main");
    let begin = event
        .create_node_with_position(EVENT_BEGIN_NODE_TYPE, 0.0, 0.0, None)
        .expect("seed begin");
    let call = event
        .create_node_with_position(
            CALL_FUNCTION_NODE_TYPE,
            0.0,
            0.0,
            Some(NodeInstanceParams::SubGraph {
                sub_graph_id: func_id.to_string(),
            }),
        )
        .expect("create call");

    let (_graph, _cs) = state
        .sync_call_node(&event.id, call, &func_id)
        .expect("sync call pins");

    let event_graph = state.get_graph(&event.id).expect("event loaded");

    assert!(has_role(&event_graph, call, &exec_in_role()));
    assert!(has_role(&event_graph, call, &exec_out_role()));

    let call_a = pin_by_role(&event_graph, call, &PinRole::Data(DataRole::Custom("a".into())));
    event_graph
        .set_pin_user_value_by_pin_id(call_a, DataValue::Int64(42))
        .expect("set call input");

    let begin_out = pin_by_role(&event_graph, begin, &PinRole::Exec(ExecRole::ExecOut));
    let call_in = pin_by_role(&event_graph, call, &exec_in_role());
    event_graph.connect(begin_out, call_in).expect("begin -> call");

    let runtime = Arc::new(Mutex::new(GraphRuntime::new(
        Arc::new(event_graph.clone()),
        state.project_data.clone(),
        state.project_store.clone(),
    )));
    let mut executor = Executor::new(runtime.clone(), NoopEmitter, ResultSourceStore::new());
    executor.start(begin).expect("execute");

    let call_r = pin_by_role(&event_graph, call, &PinRole::Data(DataRole::Custom("r".into())));
    let value = runtime
        .lock()
        .unwrap()
        .get_pin_data_value_by_pin_id(call_r)
        .expect("call output value");
    assert_eq!(value, DataValue::Int64(42));
}

#[test]
fn data_only_call_has_no_exec_pins_and_is_pulled() {
    let state = ProjectState::new();
    let func_id = build_identity_function(&state, "string", false);

    let event = state.add_event("Main");
    let begin = event
        .create_node_with_position(EVENT_BEGIN_NODE_TYPE, 0.0, 0.0, None)
        .expect("seed begin");
    let call = event
        .create_node_with_position(
            CALL_FUNCTION_NODE_TYPE,
            0.0,
            0.0,
            Some(NodeInstanceParams::SubGraph {
                sub_graph_id: func_id.to_string(),
            }),
        )
        .expect("create call");
    let print = event
        .create_node_with_position("Debug:Print", 0.0, 0.0, None)
        .expect("create print");

    state
        .sync_call_node(&event.id, call, &func_id)
        .expect("sync call pins");

    let event_graph = state.get_graph(&event.id).expect("event loaded");

    assert!(!has_role(&event_graph, call, &exec_in_role()));
    assert!(!has_role(&event_graph, call, &exec_out_role()));

    let call_a = pin_by_role(&event_graph, call, &PinRole::Data(DataRole::Custom("a".into())));
    event_graph
        .set_pin_user_value_by_pin_id(call_a, DataValue::String("hi".to_string()))
        .expect("set call input");

    let begin_out = pin_by_role(&event_graph, begin, &PinRole::Exec(ExecRole::ExecOut));
    let print_in = pin_by_role(&event_graph, print, &PinRole::Exec(ExecRole::ExecIn));
    event_graph.connect(begin_out, print_in).expect("begin -> print");

    let call_r = pin_by_role(&event_graph, call, &PinRole::Data(DataRole::Custom("r".into())));
    let print_msg = pin_by_role(&event_graph, print, &PinRole::Data(DataRole::Inputs(0)));
    event_graph.connect(call_r, print_msg).expect("call -> print msg");

    let runtime = Arc::new(Mutex::new(GraphRuntime::new(
        Arc::new(event_graph.clone()),
        state.project_data.clone(),
        state.project_store.clone(),
    )));
    let mut executor = Executor::new(runtime.clone(), NoopEmitter, ResultSourceStore::new());
    executor.start(begin).expect("execute");

    let value = runtime
        .lock()
        .unwrap()
        .get_pin_data_value_by_pin_id(call_r)
        .expect("call output value");
    assert_eq!(value, DataValue::String("hi".to_string()));
}

fn pin_direction(
    graph: &GraphInstance,
    node: NodeId,
    role: &PinRole,
) -> yssbi_lib::graph::PinDirection {
    graph
        .get_pin_instances_by_node_id(node)
        .into_iter()
        .find(|p| &p.definition.role == role)
        .unwrap_or_else(|| panic!("pin {:?} not found", role))
        .definition
        .direction
}

#[test]
fn call_node_input_output_directions_match_signature() {
    use yssbi_lib::graph::PinDirection;
    let state = ProjectState::new();
    let func_id = build_identity_function(&state, "int", true);

    let event = state.add_event("Main");
    let call = event
        .create_node_with_position(
            CALL_FUNCTION_NODE_TYPE,
            0.0,
            0.0,
            Some(NodeInstanceParams::SubGraph {
                sub_graph_id: func_id.to_string(),
            }),
        )
        .expect("create call");
    state
        .sync_call_node(&event.id, call, &func_id)
        .expect("sync call pins");

    let g = state.get_graph(&event.id).expect("event loaded");
    assert_eq!(
        pin_direction(&g, call, &PinRole::Data(DataRole::Custom("a".into()))),
        PinDirection::Input,
    );
    assert_eq!(
        pin_direction(&g, call, &PinRole::Data(DataRole::Custom("r".into()))),
        PinDirection::Output,
    );
    assert_eq!(pin_direction(&g, call, &exec_in_role()), PinDirection::Input);
    assert_eq!(pin_direction(&g, call, &exec_out_role()), PinDirection::Output);
}

#[test]
fn removing_exec_from_signature_updates_call_pins() {
    let state = ProjectState::new();
    let func_id = build_identity_function(&state, "int", true);

    let event = state.add_event("Main");
    let call = event
        .create_node_with_position(
            CALL_FUNCTION_NODE_TYPE,
            0.0,
            0.0,
            Some(NodeInstanceParams::SubGraph {
                sub_graph_id: func_id.to_string(),
            }),
        )
        .expect("create call");
    state
        .sync_call_node(&event.id, call, &func_id)
        .expect("sync call pins");

    let event_graph = state.get_graph(&event.id).expect("event loaded");
    assert!(has_role(&event_graph, call, &exec_out_role()));

    state
        .update_function_signature(
            &func_id,
            Some(vec![sig("a", "a", "int")]),
            Some(vec![sig("r", "r", "int")]),
        )
        .expect("remove exec from signature");
    for (gid, _graph, _sets) in state.sync_call_nodes_for_function(&func_id) {
        assert_eq!(gid, event.id);
    }

    let event_graph = state.get_graph(&event.id).expect("event loaded");
    assert!(!has_role(&event_graph, call, &exec_out_role()));
    assert!(has_role(
        &event_graph,
        call,
        &PinRole::Data(DataRole::Custom("a".into()))
    ));
}

#[test]
fn project_call_node_pins_after_create_with_id_path() {
    let state = ProjectState::new();
    let func_id = build_identity_function(&state, "int", false);

    let event = state.add_event("Main");
    let call = event
        .create_node_raw_with_ids(
            CALL_FUNCTION_NODE_TYPE,
            uuid::Uuid::new_v4().into(),
            &[],
            10.0,
            20.0,
            Some(NodeInstanceParams::SubGraph {
                sub_graph_id: func_id.to_string(),
            }),
        )
        .expect("create call raw");

    let event_graph = state.get_graph(&event.id).expect("event loaded");
    assert_eq!(event_graph.get_pin_instances_by_node_id(call).len(), 0);

    state
        .project_call_node_pins(
            &event.id,
            call,
            CALL_FUNCTION_NODE_TYPE,
            Some(&NodeInstanceParams::SubGraph {
                sub_graph_id: func_id.to_string(),
            }),
        )
        .expect("project pins")
        .expect("should project");

    let event_graph = state.get_graph(&event.id).expect("event loaded");
    assert!(has_role(
        &event_graph,
        call,
        &PinRole::Data(DataRole::Custom("a".into()))
    ));
    assert!(has_role(
        &event_graph,
        call,
        &PinRole::Data(DataRole::Custom("r".into()))
    ));
    assert!(!has_role(&event_graph, call, &exec_out_role()));
}

#[test]
fn data_only_function_signature_change_syncs_call_data_pins() {
    let state = ProjectState::new();
    let func_id = build_identity_function(&state, "int", false);

    let event = state.add_event("Main");
    let call = event
        .create_node_with_position(
            CALL_FUNCTION_NODE_TYPE,
            0.0,
            0.0,
            Some(NodeInstanceParams::SubGraph {
                sub_graph_id: func_id.to_string(),
            }),
        )
        .expect("create call");
    state
        .sync_call_node(&event.id, call, &func_id)
        .expect("sync call pins");

    let event_graph = state.get_graph(&event.id).expect("event loaded");
    assert!(!has_role(&event_graph, call, &exec_out_role()));
    assert!(has_role(
        &event_graph,
        call,
        &PinRole::Data(DataRole::Custom("a".into()))
    ));
    assert!(!has_role(
        &event_graph,
        call,
        &PinRole::Data(DataRole::Custom("b".into()))
    ));

    state
        .update_function_signature(
            &func_id,
            Some(vec![sig("a", "a", "float"), sig("b", "b", "int")]),
            None,
        )
        .expect("update signature");

    for (gid, _graph, sets) in state.sync_call_nodes_for_function(&func_id) {
        assert_eq!(gid, event.id);
        assert!(!sets.is_empty());
    }

    let event_graph = state.get_graph(&event.id).expect("event loaded");
    assert!(!has_role(&event_graph, call, &exec_out_role()));
    assert!(has_role(
        &event_graph,
        call,
        &PinRole::Data(DataRole::Custom("a".into()))
    ));
    assert!(has_role(
        &event_graph,
        call,
        &PinRole::Data(DataRole::Custom("b".into()))
    ));

    let a_pin = event_graph
        .get_pin_instances_by_node_id(call)
        .into_iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Custom("a".into())))
        .expect("input a pin");
    match &a_pin.definition.data_type {
        Some(yssbi_lib::graph::pin::PinDataTypeDefinition::Concrete(dt)) => {
            assert_eq!(*dt, yssbi_lib::graph::DataType::Float64);
        }
        other => panic!("expected concrete float pin type, got {:?}", other),
    }
}

/// 回归：Call 投影须锁外解析签名，锁内只改调用方图。
#[test]
fn resolve_call_projection_signature_then_sync_inside_graph_mut() {
    let state = ProjectState::new();
    let func_id = build_identity_function(&state, "int", true);
    let event = state.add_event("Main");
    let params = NodeInstanceParams::SubGraph {
        sub_graph_id: func_id.to_string(),
    };

    let signature = state
        .resolve_call_projection_signature(CALL_FUNCTION_NODE_TYPE, Some(&params))
        .expect("resolve signature")
        .expect("signature");

    let call = state
        .with_graph_mut(&event.id, |mut ctx| {
            let call = ctx
                .graph()
                .create_node_with_position(
                    CALL_FUNCTION_NODE_TYPE,
                    0.0,
                    0.0,
                    Some(params.clone()),
                )
                .expect("create call");
            ctx.graph().sync_call_function_pins_from_signature(
                call,
                &signature.inputs,
                &signature.outputs,
                None,
            );
            Ok(call)
        })
        .expect("graph mut");

    let event_graph = state.get_graph(&event.id).expect("event loaded");
    assert!(has_role(&event_graph, call, &exec_in_role()));
    assert!(has_role(
        &event_graph,
        call,
        &PinRole::Data(DataRole::Custom("a".into()))
    ));
}

#[test]
fn call_site_index_tracks_create_and_delete_without_full_graph_scan() {
    let state = ProjectState::new();
    let func_id = build_identity_function(&state, "int", false);
    let other_func = build_identity_function(&state, "float", false);
    let event = state.add_event("Main");

    let call = event
        .create_node_with_position(
            CALL_FUNCTION_NODE_TYPE,
            0.0,
            0.0,
            Some(NodeInstanceParams::SubGraph {
                sub_graph_id: func_id.to_string(),
            }),
        )
        .expect("create call");

    state.register_call_site_for_node(
        &event.id,
        call,
        CALL_FUNCTION_NODE_TYPE,
        Some(&NodeInstanceParams::SubGraph {
            sub_graph_id: func_id.to_string(),
        }),
    );

    let sites = state.get_function_call_sites(&func_id);
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].0, event.id);
    assert_eq!(sites[0].1, vec![call]);

    assert!(state.get_function_call_sites(&other_func).is_empty());

    state
        .with_graph_mut(&event.id, |mut ctx| {
            ctx.graph().remove_node_raw(call)?;
            Ok(())
        })
        .expect("remove call node");

    state.unregister_call_site_for_node(&event.id, call, CALL_FUNCTION_NODE_TYPE);
    assert!(state.get_function_call_sites(&func_id).is_empty());
}

#[test]
fn sync_call_with_predetermined_pin_ids_uses_client_ids() {
    let state = ProjectState::new();
    let func_id = state.add_function("F").id;
    let signature = state
        .get_function_signature(&func_id)
        .expect("signature");

    let event = state.add_event("Main");
    let call = event
        .create_node_raw_with_ids(
            CALL_FUNCTION_NODE_TYPE,
            uuid::Uuid::new_v4().into(),
            &[],
            0.0,
            0.0,
            Some(NodeInstanceParams::SubGraph {
                sub_graph_id: func_id.to_string(),
            }),
        )
        .expect("create call");

    let exec_in_id: PinId = uuid::Uuid::new_v4().into();
    let exec_out_id: PinId = uuid::Uuid::new_v4().into();

    event.sync_call_function_pins_from_signature(
        call,
        &signature.inputs,
        &signature.outputs,
        Some(&[exec_in_id, exec_out_id]),
    );

    let event_graph = state.get_graph(&event.id).expect("event loaded");
    let pin_ids: Vec<PinId> = event_graph
        .get_pin_instances_by_node_id(call)
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(pin_ids, vec![exec_in_id, exec_out_id]);
}
