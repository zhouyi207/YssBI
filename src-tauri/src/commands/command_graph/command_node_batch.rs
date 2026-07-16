use super::command_node::{
    index_call_site_after_create, node_create_dto_from_graph, preload_call_projection_signatures,
};
use crate::error::AppError;
use crate::event::{Event, EventConnection, EventNode, emit_project_event};
use crate::graph::core::GraphInstance;
use crate::graph::{
    DataType, DataValue, GraphRecompileScope, NodeId, NodeInstanceParams, PinChangeSet,
    PinDirection, PinId, PinResolveMode,
};
use crate::log::log_app;
use crate::project::{
    GraphResourcePath, ProjectState, emit_inferred_types, emit_pin_change_events,
};
use crate::schema::{GraphUndoPatch, NodeInstanceDTO, PinInstanceDTO};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tauri::{AppHandle, State};
use uuid::Uuid;

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

#[derive(Clone, Debug, Deserialize)]
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

struct BatchCreateWithConnectionsWork {
    node_ids: Vec<String>,
    pin_mapping: HashMap<String, String>,
    undo_patch: GraphUndoPatch,
    node_events: Vec<(NodeId, NodeInstanceDTO, Vec<PinInstanceDTO>)>,
    established_connections: Vec<(PinId, PinId)>,
    change_sets: Vec<PinChangeSet>,
    inferred: Vec<(PinId, DataType)>,
    graph: GraphInstance,
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

    let work = state.with_graph_mut(&graph_path, |mut ctx| {
        let mut node_events: Vec<(NodeId, NodeInstanceDTO, Vec<PinInstanceDTO>)> = Vec::new();
        let mut created_node_ids: Vec<NodeId> = Vec::new();
        let mut pin_mapping: HashMap<String, String> = HashMap::new();
        let mut used_new_pins: HashSet<String> = HashSet::new();

        for entry in &entries {
            let node_id = ctx.graph().create_node_raw(
                &entry.node_type,
                entry.x,
                entry.y,
                entry.params.clone(),
            )?;
            created_node_ids.push(node_id);

            map_entry_pins(
                ctx.graph(),
                node_id,
                entry,
                &mut pin_mapping,
                &mut used_new_pins,
            );
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
            node_events.push((node_id, node_dto, pins_dto));
        }

        let node_ids: Vec<String> = created_node_ids.iter().map(|id| id.to_string()).collect();
        let mut connection_state = ConnectionRestoreState::new(&connections, pin_mapping);

        while connection_state.has_pending() {
            let pass = connection_state.connect_pending(ctx.graph());
            if !pass.made_progress {
                break;
            }

            let result = ctx.recompile(GraphRecompileScope::TopologyEffects {
                seeds: pass.seed_nodes,
                mode: PinResolveMode::Interactive,
            });
            connection_state.change_sets.extend(result.change_sets);
            connection_state.inferred = result.inferred;

            for (entry_idx, entry) in entries.iter().enumerate() {
                map_entry_pins(
                    ctx.graph(),
                    created_node_ids[entry_idx],
                    entry,
                    &mut connection_state.pin_mapping,
                    &mut used_new_pins,
                );
            }
        }

        let undo_patch = ctx.graph().capture_subgraph(&created_node_ids);
        let graph = ctx.graph_ref().clone();
        Ok(BatchCreateWithConnectionsWork {
            node_ids,
            pin_mapping: connection_state.pin_mapping,
            undo_patch,
            node_events,
            established_connections: connection_state.established,
            change_sets: connection_state.change_sets,
            inferred: connection_state.inferred,
            graph,
        })
    })?;

    for (entry, (node_id, _, _)) in entries.iter().zip(&work.node_events) {
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
            nodes: work.node_events,
        }),
    );

    if !work.established_connections.is_empty() {
        emit_project_event(
            &app,
            Event::Connection(EventConnection::ConnectionsBatchCreated {
                graph_path: graph_path.as_str().to_string(),
                connections: work.established_connections,
            }),
        );
    }
    emit_pin_change_events(&app, &graph_path, &work.graph, &work.change_sets);
    emit_inferred_types(&app, &graph_path, work.inferred);

    Ok(BatchCreateWithConnectionsResult {
        node_ids: work.node_ids,
        pin_mapping: work.pin_mapping,
        undo_patch: work.undo_patch,
    })
}

fn map_entry_pins(
    graph: &mut GraphInstance,
    node_id: NodeId,
    entry: &BatchNodeEntry,
    pin_mapping: &mut HashMap<String, String>,
    used_new_pins: &mut HashSet<String>,
) {
    let current_pins = graph.get_pin_instances_by_node_id(node_id);
    for old_pin in &entry.pins {
        if pin_mapping.contains_key(&old_pin.pin_id) {
            continue;
        }

        let Some(new_pin_id) = current_pins
            .iter()
            .find(|new_pin| {
                new_pin.definition.name == old_pin.name
                    && new_pin.definition.direction == old_pin.direction
                    && !used_new_pins.contains(&new_pin.id.to_string())
            })
            .map(|new_pin| new_pin.id)
        else {
            continue;
        };

        let new_id = new_pin_id.to_string();
        pin_mapping.insert(old_pin.pin_id.clone(), new_id.clone());
        used_new_pins.insert(new_id);
        restore_pin_user_value(graph, new_pin_id, old_pin);
    }
}

fn restore_pin_user_value(graph: &mut GraphInstance, pin_id: PinId, old_pin: &BatchPinEntry) {
    let Some(raw_value) = &old_pin.user_value else {
        return;
    };
    if let Ok(value) = serde_json::from_value::<DataValue>(raw_value.clone()) {
        let _ = graph.set_pin_user_value_by_pin_id(pin_id, value);
    }
}

struct ConnectionRestoreState {
    pending: Vec<usize>,
    connections: Vec<BatchConnectionEntry>,
    pin_mapping: HashMap<String, String>,
    established: Vec<(PinId, PinId)>,
    change_sets: Vec<PinChangeSet>,
    inferred: Vec<(PinId, DataType)>,
}

struct ConnectionRestorePass {
    made_progress: bool,
    seed_nodes: Vec<NodeId>,
}

impl ConnectionRestoreState {
    fn new(connections: &[BatchConnectionEntry], pin_mapping: HashMap<String, String>) -> Self {
        Self {
            pending: (0..connections.len()).collect(),
            connections: connections.to_vec(),
            pin_mapping,
            established: Vec::new(),
            change_sets: Vec::new(),
            inferred: Vec::new(),
        }
    }

    fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    fn connect_pending(&mut self, graph: &mut GraphInstance) -> ConnectionRestorePass {
        let mut next_pending = Vec::new();
        let mut seed_nodes = Vec::new();
        let mut made_progress = false;

        for &idx in &self.pending {
            let conn = &self.connections[idx];
            if let Some((from_pin, to_pin)) = mapped_connection(conn, &self.pin_mapping) {
                if let Ok(topo) = graph.connect_topology(from_pin, to_pin) {
                    made_progress = true;
                    self.established.push((topo.from_pin, topo.to_pin));
                    seed_nodes.extend(topo.seed_nodes);
                    continue;
                }
            }
            next_pending.push(idx);
        }

        self.pending = next_pending;
        ConnectionRestorePass {
            made_progress,
            seed_nodes,
        }
    }
}

fn mapped_connection(
    conn: &BatchConnectionEntry,
    pin_mapping: &HashMap<String, String>,
) -> Option<(PinId, PinId)> {
    let from = parse_mapped_pin(pin_mapping.get(&conn.from_pin)?)?;
    let to = parse_mapped_pin(pin_mapping.get(&conn.to_pin)?)?;
    Some((from, to))
}

fn parse_mapped_pin(value: &str) -> Option<PinId> {
    Uuid::parse_str(value).ok().map(PinId::from)
}
