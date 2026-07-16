use crate::error::AppError;
use crate::event::{Event, EventConnection, EventNode, emit_project_event};
use crate::execution::ResultSourceStore;
use crate::graph::core::GraphInstance;
use crate::graph::register::value::call::CALL_FUNCTION_NODE_TYPE;
use crate::graph::{
    DataType, DataValue, GraphRecompileScope, NodeId, NodeInstanceParams, PinChangeSet,
    PinDirection, PinId,
};
use crate::log::log_app;
use crate::project::FunctionSignatureEntry;
use crate::project::{
    GraphResourcePath, ProjectState, emit_graph_pin_mutation_sync, emit_inferred_types,
    emit_pin_change_events, emit_runtime_source_invalidation,
};
use crate::schema::{GraphUndoPatch, NodeInstanceDTO, PinInstanceDTO};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, State};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePositionUpdate {
    node_id: NodeId,
    x: f32,
    y: f32,
}

/// Return value from create_node — includes pin IDs for undo/redo context
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNodeResult {
    pub node_id: String,
    pub pin_ids: Vec<String>,
}

/// 从图实例构建 `NodeCreated` / `NodesBatchCreated` 所需的 DTO。
fn node_create_dto_from_graph(
    graph: &GraphInstance,
    node_id: NodeId,
) -> Result<(NodeInstanceDTO, Vec<PinInstanceDTO>, Vec<String>), AppError> {
    let node_instance = graph
        .get_node_instance(node_id)
        .ok_or_else(|| format!("Node '{}' not found", node_id))?;

    let mut node_dto: NodeInstanceDTO = (&node_instance).into();
    let pin_instances = graph.get_pin_instances_by_node_id(node_id);
    let data_state = graph.data_state.read().unwrap();
    let mut pins_dto = Vec::with_capacity(pin_instances.len());
    for pin in &pin_instances {
        match pin.definition.direction {
            PinDirection::Input => node_dto.inputs.push(pin.id.to_string()),
            PinDirection::Output => node_dto.outputs.push(pin.id.to_string()),
        }
        let resolved_type = data_state.pin_types.get(&pin.id);
        pins_dto.push(PinInstanceDTO::from_pin_with_context(pin, resolved_type));
    }
    let pin_id_strings: Vec<String> = pin_instances.iter().map(|p| p.id.to_string()).collect();
    Ok((node_dto, pins_dto, pin_id_strings))
}

/// 批量预载 Call 目标签名（必须在 `with_graph_mut` 之前完成；不加载整图）。
fn preload_call_projection_signatures(
    state: &ProjectState,
    entries: impl IntoIterator<Item = (impl AsRef<str>, Option<NodeInstanceParams>)>,
) -> Result<HashMap<GraphResourcePath, FunctionSignatureEntry>, AppError> {
    let mut targets = HashMap::new();
    for (node_type, params) in entries {
        let node_type = node_type.as_ref();
        let params_ref = params.as_ref();
        if let Some(signature) = state.resolve_call_projection_signature(node_type, params_ref)? {
            if let Some(target_path) =
                ProjectState::call_function_target_path(node_type, params_ref)
            {
                targets.entry(target_path).or_insert(signature);
            }
        }
    }
    Ok(targets)
}

fn index_call_site_after_create(
    state: &ProjectState,
    graph_path: GraphResourcePath,
    node_id: NodeId,
    node_type: &str,
    params: Option<&NodeInstanceParams>,
) {
    state.register_call_site_for_node(&graph_path, node_id, node_type, params);
}

fn index_call_sites_after_delete(
    state: &ProjectState,
    graph_path: GraphResourcePath,
    removed: impl IntoIterator<Item = (NodeId, String)>,
) {
    for (node_id, node_type) in removed {
        state.unregister_call_site_for_node(&graph_path, node_id, &node_type);
    }
}

#[tauri::command]
pub fn create_node(
    app: AppHandle,
    state: State<ProjectState>,
    graph_path: GraphResourcePath,
    node_type: &str,
    x: Option<f32>,
    y: Option<f32>,
    params: Option<NodeInstanceParams>,
) -> Result<CreateNodeResult, AppError> {
    log_app::info!(
        "create_node called: graph_path={}, node_type={}, x={:?}, y={:?}",
        graph_path,
        node_type,
        x,
        y
    );

    // Call Function：锁外解析签名，单次 with_graph_mut 完成创建 + 投影 + DTO。
    let signature = state.resolve_call_projection_signature(node_type, params.as_ref())?;
    let params_for_index = params.clone();

    let (node_id, node_dto, pins_dto, pin_id_strings) =
        state.with_graph_mut(&graph_path, |mut ctx| {
            let node_id = ctx.graph().create_node_with_position(
                node_type,
                x.unwrap_or(0.0),
                y.unwrap_or(0.0),
                params,
            )?;
            ctx.sync_runtime_symbols();
            if let Some(ref sig) = signature {
                ctx.graph().sync_call_function_pins_from_signature(
                    node_id,
                    &sig.inputs,
                    &sig.outputs,
                    None,
                );
                ctx.recompile(GraphRecompileScope::InferOnly);
            }
            let (node_dto, pins_dto, pin_id_strings) =
                node_create_dto_from_graph(ctx.graph_ref(), node_id)?;
            Ok((node_id, node_dto, pins_dto, pin_id_strings))
        })?;

    emit_project_event(
        &app,
        Event::Node(EventNode::NodeCreated {
            graph_path: graph_path.as_str().to_string(),
            node_id,
            data: node_dto,
            pins: pins_dto,
        }),
    );

    index_call_site_after_create(
        &state,
        graph_path.clone(),
        node_id,
        node_type,
        params_for_index.as_ref(),
    );

    Ok(CreateNodeResult {
        node_id: node_id.to_string(),
        pin_ids: pin_id_strings,
    })
}

/// 批量创建节点请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateNodeRequest {
    node_type: String,
    x: Option<f32>,
    y: Option<f32>,
    params: Option<NodeInstanceParams>,
}

/// 批量创建节点（粘贴时使用），一次性发送所有节点信息，避免逐个创建的延迟
#[tauri::command]
pub fn batch_create_nodes(
    app: AppHandle,
    state: State<ProjectState>,
    graph_path: GraphResourcePath,
    requests: Vec<BatchCreateNodeRequest>,
) -> Result<Vec<String>, AppError> {
    log_app::info!(
        "batch_create_nodes called: graph_path={}, count={}",
        graph_path,
        requests.len()
    );

    let targets = preload_call_projection_signatures(
        &state,
        requests
            .iter()
            .map(|req| (&req.node_type, req.params.clone())),
    )?;

    let (results, node_ids) = state.with_graph_mut(&graph_path, |mut ctx| {
        let mut created_ids: Vec<NodeId> = Vec::with_capacity(requests.len());

        for req in &requests {
            let node_id = ctx.graph().create_node_raw(
                &req.node_type,
                req.x.unwrap_or(0.0),
                req.y.unwrap_or(0.0),
                req.params.clone(),
            )?;
            created_ids.push(node_id);
        }

        ctx.sync_runtime_symbols();

        for (req, &node_id) in requests.iter().zip(&created_ids) {
            if let Some(target_id) =
                ProjectState::call_function_target_path(&req.node_type, req.params.as_ref())
            {
                if let Some(target) = targets.get(&target_id) {
                    ctx.graph().sync_call_function_pins_from_signature(
                        node_id,
                        &target.inputs,
                        &target.outputs,
                        None,
                    );
                }
            }
        }

        ctx.recompile(GraphRecompileScope::InferOnly);

        let mut results: Vec<(NodeId, NodeInstanceDTO, Vec<PinInstanceDTO>)> =
            Vec::with_capacity(created_ids.len());
        for &node_id in &created_ids {
            let (node_dto, pins_dto, _) = node_create_dto_from_graph(ctx.graph_ref(), node_id)?;
            results.push((node_id, node_dto, pins_dto));
        }

        let node_ids: Vec<String> = results.iter().map(|(id, _, _)| id.to_string()).collect();
        Ok((results, node_ids))
    })?;

    for (req, (node_id, _, _)) in requests.iter().zip(&results) {
        index_call_site_after_create(
            &state,
            graph_path.clone(),
            *node_id,
            &req.node_type,
            req.params.as_ref(),
        );
    }

    emit_project_event(
        &app,
        Event::Node(EventNode::NodesBatchCreated {
            graph_path: graph_path.as_str().to_string(),
            nodes: results,
        }),
    );

    Ok(node_ids)
}

#[tauri::command]
pub fn delete_node(
    app: AppHandle,
    state: State<ProjectState>,
    source_store: State<ResultSourceStore>,
    graph_path: GraphResourcePath,
    node_id: NodeId,
) -> Result<(), AppError> {
    let (deleted_pin_ids, removed_node_type) = state.with_graph_mut(&graph_path, |mut ctx| {
        if ctx.graph_ref().is_shell_node(node_id) {
            return Err("System-managed shell node cannot be deleted".to_string());
        }

        let removed_node_type = ctx
            .graph_ref()
            .get_node_instance(node_id)
            .map(|node| node.definition.node_type.clone())
            .unwrap_or_default();

        let deleted_pin_ids: Vec<PinId> = ctx
            .graph()
            .get_pin_instances_by_node_id(node_id)
            .into_iter()
            .map(|pin| pin.id)
            .collect();

        ctx.graph().remove_node_raw(node_id)?;
        ctx.recompile(GraphRecompileScope::InferOnly);

        Ok((deleted_pin_ids, removed_node_type))
    })?;

    index_call_sites_after_delete(&state, graph_path.clone(), [(node_id, removed_node_type)]);

    emit_project_event(
        &app,
        Event::Node(EventNode::NodeDeleted {
            graph_path: graph_path.as_str().to_string(),
            node_id,
        }),
    );

    emit_runtime_source_invalidation(&app, &source_store, &graph_path, &[], &deleted_pin_ids);

    Ok(())
}

/// 批量删除节点（单次 IPC + 单个事件）；返回删除前捕获的子图快照供 undo 使用。
#[tauri::command]
pub fn batch_delete_nodes(
    app: AppHandle,
    state: State<ProjectState>,
    source_store: State<ResultSourceStore>,
    graph_path: GraphResourcePath,
    node_ids: Vec<NodeId>,
) -> Result<GraphUndoPatch, AppError> {
    log_app::info!(
        "batch_delete_nodes called: graph_path={}, count={}",
        graph_path,
        node_ids.len()
    );

    let (snapshot, deleted_pin_ids, node_ids, removed_node_types) =
        state.with_graph_mut(&graph_path, |mut ctx| {
            // 壳节点（Event Begin / Function Entry/Return）不可删除，静默跳过。
            let node_ids: Vec<NodeId> = node_ids
                .into_iter()
                .filter(|&nid| !ctx.graph_ref().is_shell_node(nid))
                .collect();

            let removed_node_types: Vec<(NodeId, String)> = node_ids
                .iter()
                .filter_map(|&nid| {
                    ctx.graph_ref()
                        .get_node_instance(nid)
                        .map(|node| (nid, node.definition.node_type.clone()))
                })
                .collect();

            let snapshot = ctx.graph().capture_subgraph_for_delete(&node_ids);

            let mut deleted_pin_ids = Vec::new();
            for &nid in &node_ids {
                deleted_pin_ids.extend(
                    ctx.graph()
                        .get_pin_instances_by_node_id(nid)
                        .into_iter()
                        .map(|pin| pin.id),
                );
            }

            for &nid in &node_ids {
                ctx.graph().remove_node_raw(nid)?;
            }

            ctx.recompile(GraphRecompileScope::InferOnly);
            Ok((snapshot, deleted_pin_ids, node_ids, removed_node_types))
        })?;

    index_call_sites_after_delete(&state, graph_path.clone(), removed_node_types);

    emit_project_event(
        &app,
        Event::Node(EventNode::NodesBatchDeleted {
            graph_path: graph_path.as_str().to_string(),
            node_ids,
        }),
    );

    emit_runtime_source_invalidation(&app, &source_store, &graph_path, &[], &deleted_pin_ids);

    Ok(snapshot)
}

/// Apply a previously captured undo patch (DeleteNodes undo / DisconnectPin undo / Composite redo).
#[tauri::command]
pub fn apply_graph_patch(
    app: AppHandle,
    state: State<ProjectState>,
    graph_path: GraphResourcePath,
    patch: GraphUndoPatch,
) -> Result<(), AppError> {
    log_app::info!(
        "[apply_graph_patch] graph={}, nodes={}, neighbors={}, connections={}",
        graph_path,
        patch.nodes.len(),
        patch.neighbor_nodes.len(),
        patch.connections.len()
    );

    let (result, graph) = state.with_graph_mut(&graph_path, |mut ctx| {
        let variable_symbols = ctx.variable_symbols.clone();
        let dataframe_symbols = ctx.dataframe_symbols.clone();
        let result = ctx
            .graph()
            .apply_graph_patch(patch, &variable_symbols, &dataframe_symbols)?;
        let graph = ctx.graph_ref().clone();
        Ok((result, graph))
    })?;

    if !result.node_batches.is_empty() {
        emit_project_event(
            &app,
            Event::Node(EventNode::NodesBatchCreated {
                graph_path: graph_path.as_str().to_string(),
                nodes: result.node_batches,
            }),
        );
    }

    emit_pin_change_events(&app, &graph_path, &graph, &result.change_sets);

    if !result.established_connections.is_empty() {
        emit_project_event(
            &app,
            Event::Connection(EventConnection::ConnectionsBatchCreated {
                graph_path: graph_path.as_str().to_string(),
                connections: result.established_connections,
            }),
        );
    }
    emit_inferred_types(&app, &graph_path, result.inferred);

    state.refresh_call_sites_for_caller(&graph_path);

    Ok(())
}

/// 批量更新节点位置（拖拽结束时调用，CQRS 模式）
#[tauri::command]
pub fn update_node_positions(
    app: AppHandle,
    state: State<ProjectState>,
    graph_path: GraphResourcePath,
    updates: Vec<NodePositionUpdate>,
) -> Result<(), AppError> {
    let updates_tuple: Vec<(NodeId, f32, f32)> =
        updates.iter().map(|u| (u.node_id, u.x, u.y)).collect();

    state.with_graph_mut(&graph_path, |mut ctx| {
        ctx.graph().set_node_positions(&updates_tuple)?;
        Ok(())
    })?;

    emit_project_event(
        &app,
        Event::Node(EventNode::NodePositionsUpdated {
            graph_path: graph_path.as_str().to_string(),
            updates: updates_tuple,
        }),
    );

    Ok(())
}

// ==================== Undo/Redo 支持命令 ====================

/// Create a node with specific IDs (for redo — preserves node/pin identity).
#[tauri::command]
pub fn create_node_with_id(
    app: AppHandle,
    state: State<ProjectState>,
    graph_path: GraphResourcePath,
    node_id: String,
    pin_ids: Vec<String>,
    node_type: String,
    x: Option<f32>,
    y: Option<f32>,
    params: Option<NodeInstanceParams>,
) -> Result<(), AppError> {
    let nid =
        NodeId::from(Uuid::parse_str(&node_id).map_err(|e| format!("Invalid node_id: {}", e))?);
    let pids: Vec<PinId> = pin_ids
        .iter()
        .map(|s| {
            Uuid::parse_str(s)
                .map(PinId::from)
                .map_err(|e| format!("Invalid pin_id '{}': {}", s, e))
        })
        .collect::<Result<Vec<_>, _>>()?;

    log_app::info!(
        "[create_node_with_id] graph={}, node={}, type={}, pins={}",
        graph_path,
        node_id,
        node_type,
        pids.len()
    );

    let projection_params = params.clone();

    let signature =
        state.resolve_call_projection_signature(&node_type, projection_params.as_ref())?;

    let is_call_function = node_type == CALL_FUNCTION_NODE_TYPE;
    let call_projection_pin_ids = if is_call_function && !pids.is_empty() {
        Some(pids.as_slice())
    } else {
        None
    };
    let create_pin_ids: &[PinId] = if is_call_function { &[] } else { &pids };

    let (node_dto, pins_dto) = state.with_graph_mut(&graph_path, |mut ctx| {
        ctx.graph().create_node_raw_with_ids(
            &node_type,
            nid,
            create_pin_ids,
            x.unwrap_or(0.0),
            y.unwrap_or(0.0),
            params,
        )?;
        ctx.sync_runtime_symbols();
        if let Some(ref sig) = signature {
            ctx.graph().sync_call_function_pins_from_signature(
                nid,
                &sig.inputs,
                &sig.outputs,
                call_projection_pin_ids,
            );
            ctx.recompile(GraphRecompileScope::InferOnly);
        }
        let (node_dto, pins_dto, _) = node_create_dto_from_graph(ctx.graph_ref(), nid)?;
        Ok((node_dto, pins_dto))
    })?;

    emit_project_event(
        &app,
        Event::Node(EventNode::NodeCreated {
            graph_path: graph_path.as_str().to_string(),
            node_id: nid,
            data: node_dto,
            pins: pins_dto,
        }),
    );

    index_call_site_after_create(
        &state,
        graph_path.clone(),
        nid,
        &node_type,
        projection_params.as_ref(),
    );

    Ok(())
}

// ==================== Batch Create with Connections ====================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchPinEntry {
    pub pin_id: String,
    pub name: String,
    pub direction: PinDirection,
    pub user_value: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchNodeEntry {
    pub node_type: String,
    pub x: f32,
    pub y: f32,
    pub params: Option<NodeInstanceParams>,
    pub pins: Vec<BatchPinEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchConnectionEntry {
    pub from_pin: String,
    pub to_pin: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateWithConnectionsResult {
    pub node_ids: Vec<String>,
    pub pin_mapping: HashMap<String, String>,
    pub undo_patch: GraphUndoPatch,
}

/// Batch-create nodes with pin remapping and connection restoration.
/// Used by paste, template import, and similar bulk-creation scenarios.
#[tauri::command]
pub fn batch_create_with_connections(
    app: AppHandle,
    state: State<ProjectState>,
    graph_path: GraphResourcePath,
    entries: Vec<BatchNodeEntry>,
    connections: Vec<BatchConnectionEntry>,
) -> Result<BatchCreateWithConnectionsResult, AppError> {
    log_app::info!(
        "[batch_create_with_connections] graph={}, entries={}, connections={}",
        graph_path,
        entries.len(),
        connections.len()
    );

    let targets = preload_call_projection_signatures(
        &state,
        entries
            .iter()
            .map(|entry| (&entry.node_type, entry.params.clone())),
    )?;

    let (
        node_id_strings,
        pin_mapping,
        undo_patch,
        all_results,
        established,
        all_change_sets,
        last_inferred,
        graph,
    ) = state.with_graph_mut(&graph_path, |mut ctx| {
        let mut all_results: Vec<(NodeId, NodeInstanceDTO, Vec<PinInstanceDTO>)> = Vec::new();
        let mut created_node_ids: Vec<NodeId> = Vec::new();
        let mut pin_mapping: HashMap<String, String> = HashMap::new();
        let mut used_new_pins: std::collections::HashSet<String> = std::collections::HashSet::new();

        for entry in &entries {
            let node_id = ctx.graph().create_node_raw(
                &entry.node_type,
                entry.x,
                entry.y,
                entry.params.clone(),
            )?;
            created_node_ids.push(node_id);

            let new_pins = ctx.graph().get_pin_instances_by_node_id(node_id);

            for old_pin in &entry.pins {
                if let Some(new_pin) = new_pins.iter().find(|np| {
                    np.definition.name == old_pin.name
                        && np.definition.direction == old_pin.direction
                        && !used_new_pins.contains(&np.id.to_string())
                }) {
                    let new_id = new_pin.id.to_string();
                    pin_mapping.insert(old_pin.pin_id.clone(), new_id.clone());
                    used_new_pins.insert(new_id);

                    if let Some(ref raw_val) = old_pin.user_value {
                        if let Ok(dv) = serde_json::from_value::<DataValue>(raw_val.clone()) {
                            let _ = ctx.graph().set_pin_user_value_by_pin_id(new_pin.id, dv);
                        }
                    }
                }
            }
            ctx.sync_runtime_symbols();

            if let Some(target_id) =
                ProjectState::call_function_target_path(&entry.node_type, entry.params.as_ref())
            {
                if let Some(target) = targets.get(&target_id) {
                    ctx.graph().sync_call_function_pins_from_signature(
                        node_id,
                        &target.inputs,
                        &target.outputs,
                        None,
                    );
                }
            }

            let (node_dto, pins_dto, _) = node_create_dto_from_graph(ctx.graph_ref(), node_id)?;
            all_results.push((node_id, node_dto, pins_dto));
        }

        let node_id_strings: Vec<String> =
            created_node_ids.iter().map(|id| id.to_string()).collect();

        let mut pending: Vec<usize> = (0..connections.len()).collect();
        let mut established: Vec<(PinId, PinId)> = Vec::new();
        let mut all_change_sets: Vec<PinChangeSet> = Vec::new();
        let mut last_inferred: Vec<(PinId, DataType)> = Vec::new();

        loop {
            let mut next_pending = Vec::new();
            let mut pass_seeds: Vec<NodeId> = Vec::new();
            let mut made_progress = false;

            for &idx in &pending {
                let conn = &connections[idx];
                let new_from = pin_mapping.get(&conn.from_pin).cloned();
                let new_to = pin_mapping.get(&conn.to_pin).cloned();

                let topo = match (new_from, new_to) {
                    (Some(from_str), Some(to_str)) => {
                        match (Uuid::parse_str(&from_str), Uuid::parse_str(&to_str)) {
                            (Ok(from_uuid), Ok(to_uuid)) => ctx
                                .graph()
                                .connect_topology(PinId::from(from_uuid), PinId::from(to_uuid))
                                .ok(),
                            _ => None,
                        }
                    }
                    _ => None,
                };

                if let Some(topo) = topo {
                    made_progress = true;
                    established.push((topo.from_pin, topo.to_pin));
                    pass_seeds.extend(topo.seed_nodes);
                } else {
                    next_pending.push(idx);
                }
            }

            if made_progress {
                let result = ctx.recompile(GraphRecompileScope::TopologyEffects {
                    seeds: pass_seeds,
                    mode: crate::graph::PinResolveMode::Interactive,
                });
                all_change_sets.extend(result.change_sets);
                last_inferred = result.inferred;

                for (entry_idx, entry) in entries.iter().enumerate() {
                    let nid = created_node_ids[entry_idx];
                    let current_pins = ctx.graph().get_pin_instances_by_node_id(nid);
                    for old_pin in &entry.pins {
                        if pin_mapping.contains_key(&old_pin.pin_id) {
                            continue;
                        }
                        if let Some(new_pin) = current_pins.iter().find(|np| {
                            np.definition.name == old_pin.name
                                && np.definition.direction == old_pin.direction
                                && !used_new_pins.contains(&np.id.to_string())
                        }) {
                            let new_id = new_pin.id.to_string();
                            pin_mapping.insert(old_pin.pin_id.clone(), new_id.clone());
                            used_new_pins.insert(new_id);
                            if let Some(ref raw_val) = old_pin.user_value {
                                if let Ok(dv) = serde_json::from_value::<DataValue>(raw_val.clone())
                                {
                                    let _ =
                                        ctx.graph().set_pin_user_value_by_pin_id(new_pin.id, dv);
                                }
                            }
                        }
                    }
                }
            }

            if !made_progress || next_pending.is_empty() {
                break;
            }
            pending = next_pending;
        }

        let undo_patch = ctx.graph().capture_subgraph(&created_node_ids);
        let graph = ctx.graph_ref().clone();
        Ok((
            node_id_strings,
            pin_mapping,
            undo_patch,
            all_results,
            established,
            all_change_sets,
            last_inferred,
            graph,
        ))
    })?;

    for (entry, (node_id, _, _)) in entries.iter().zip(&all_results) {
        index_call_site_after_create(
            &state,
            graph_path.clone(),
            *node_id,
            &entry.node_type,
            entry.params.as_ref(),
        );
    }

    emit_project_event(
        &app,
        Event::Node(EventNode::NodesBatchCreated {
            graph_path: graph_path.as_str().to_string(),
            nodes: all_results,
        }),
    );

    if !established.is_empty() {
        emit_project_event(
            &app,
            Event::Connection(EventConnection::ConnectionsBatchCreated {
                graph_path: graph_path.as_str().to_string(),
                connections: established,
            }),
        );
    }
    emit_pin_change_events(&app, &graph_path, &graph, &all_change_sets);
    emit_inferred_types(&app, &graph_path, last_inferred);

    Ok(BatchCreateWithConnectionsResult {
        node_ids: node_id_strings,
        pin_mapping,
        undo_patch,
    })
}

/// 将 Call Function 节点重绑定到另一目标函数（更新 subGraphPath + 重投影 pin + call-site 索引）。
#[tauri::command]
pub fn update_call_function_target(
    app: AppHandle,
    state: State<ProjectState>,
    source_store: State<ResultSourceStore>,
    graph_path: String,
    node_id: NodeId,
    function_path: String,
) -> Result<(), AppError> {
    let graph_path = GraphResourcePath::new(&graph_path).map_err(|e| e.to_string())?;
    let function_path = GraphResourcePath::new(&function_path).map_err(|e| e.to_string())?;

    log_app::info!(
        "update_call_function_target: graph={}, node={}, target={}",
        graph_path,
        node_id,
        function_path
    );

    let (graph, change_set, inferred) =
        state.update_call_function_target(&graph_path, node_id, &function_path)?;

    let has_pin_changes = !change_set.removed_pin_ids.is_empty()
        || !change_set.added_pins.is_empty()
        || !change_set.updated_pins.is_empty()
        || !change_set.removed_connections.is_empty();
    if has_pin_changes || !inferred.is_empty() {
        emit_graph_pin_mutation_sync(
            &app,
            &source_store,
            &graph_path,
            &graph,
            &[change_set],
            inferred,
            &[],
        );
    }

    Ok(())
}
