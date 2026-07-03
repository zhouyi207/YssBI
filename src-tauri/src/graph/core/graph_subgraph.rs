//! Subgraph capture / patch apply for structural undo (delete, disconnect, paste redo).

use super::graph_instance::{GraphInstance, PinChangeSet, PinResolveMode};
use crate::graph::node::{NodeDefinition, NodeInstance};
use crate::graph::pin::{PinDirection, PinInstance};
use crate::graph::{DataType, NodeId, PinId};
use crate::schema::{ConnectionRebuildDTO, GraphUndoPatch, NodeSubgraphDTO};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

pub struct ApplyGraphPatchResult {
    pub node_batches: Vec<(
        NodeId,
        crate::schema::NodeInstanceDTO,
        Vec<crate::schema::PinInstanceDTO>,
    )>,
    pub established_connections: Vec<(PinId, PinId)>,
    pub change_sets: Vec<PinChangeSet>,
    pub inferred: Vec<(PinId, DataType)>,
}

impl GraphInstance {
    /// Capture nodes and incident connections (paste redo / create snapshot).
    pub fn capture_subgraph(&self, node_ids: &[NodeId]) -> GraphUndoPatch {
        self.capture_subgraph_inner(node_ids, false)
    }

    /// Capture deleted nodes plus undo-closure connections for structural delete undo.
    pub fn capture_subgraph_for_delete(&self, node_ids: &[NodeId]) -> GraphUndoPatch {
        self.capture_subgraph_inner(node_ids, true)
    }

    /// Capture undo patch before disconnecting pins on dynamic nodes.
    pub fn capture_disconnect_undo_patch(&self, node_ids: &[NodeId]) -> GraphUndoPatch {
        let (neighbor_nodes, connections) = self.capture_pin_resolver_closure(node_ids);
        GraphUndoPatch {
            nodes: Vec::new(),
            neighbor_nodes,
            connections,
        }
    }

    fn capture_subgraph_inner(
        &self,
        node_ids: &[NodeId],
        delete_closure: bool,
    ) -> GraphUndoPatch {        let data_state = self.data_state.read().unwrap();

        let mut nodes = Vec::new();
        let mut primary_pin_set: HashSet<PinId> = HashSet::new();

        for &nid in node_ids {
            let Some(node) = data_state.nodes.get(&nid) else {
                continue;
            };
            let pins: Vec<PinInstance> = node
                .pin_ids
                .iter()
                .filter_map(|pid| data_state.pins.get(pid).cloned())
                .collect();
            for pin in &pins {
                primary_pin_set.insert(pin.id);
            }
            nodes.push(NodeSubgraphDTO {
                id: nid,
                node_type: node.definition.node_type.clone(),
                position: node.position.clone(),
                type_var_map: node.type_var_map.clone(),
                instance_params: node.instance_params.clone(),
                pins,
            });
        }

        let mut neighbor_nodes = Vec::new();
        if delete_closure {
            let deleted_nodes: HashSet<NodeId> = node_ids.iter().copied().collect();
            let mut closure_nodes = deleted_nodes.clone();

            for conn in data_state.connections.all_connections() {
                let touches_deleted = primary_pin_set.contains(&conn.from_pin)
                    || primary_pin_set.contains(&conn.to_pin);
                if !touches_deleted {
                    continue;
                }
                for pin_id in [conn.from_pin, conn.to_pin] {
                    if let Some(pin) = data_state.pins.get(&pin_id) {
                        closure_nodes.insert(pin.node_id);
                    }
                }
            }

            for &nid in &closure_nodes {
                if deleted_nodes.contains(&nid) {
                    continue;
                }
                let Some(node) = data_state.nodes.get(&nid) else {
                    continue;
                };
                let pins: Vec<PinInstance> = node
                    .pin_ids
                    .iter()
                    .filter_map(|pid| data_state.pins.get(pid).cloned())
                    .collect();
                neighbor_nodes.push(NodeSubgraphDTO {
                    id: nid,
                    node_type: node.definition.node_type.clone(),
                    position: node.position.clone(),
                    type_var_map: node.type_var_map.clone(),
                    instance_params: node.instance_params.clone(),
                    pins,
                });
            }
        }

        let connection_pin_set = if delete_closure {
            let deleted_nodes: HashSet<NodeId> = node_ids.iter().copied().collect();
            let mut closure_nodes = deleted_nodes.clone();

            for conn in data_state.connections.all_connections() {
                let touches_deleted = primary_pin_set.contains(&conn.from_pin)
                    || primary_pin_set.contains(&conn.to_pin);
                if !touches_deleted {
                    continue;
                }
                for pin_id in [conn.from_pin, conn.to_pin] {
                    if let Some(pin) = data_state.pins.get(&pin_id) {
                        closure_nodes.insert(pin.node_id);
                    }
                }
            }

            let mut closure_pins = HashSet::new();
            for &nid in &closure_nodes {
                if let Some(node) = data_state.nodes.get(&nid) {
                    for &pid in &node.pin_ids {
                        closure_pins.insert(pid);
                    }
                }
            }
            closure_pins
        } else {
            primary_pin_set.clone()
        };

        let mut connections = Vec::new();
        let mut seen: HashSet<(PinId, PinId)> = HashSet::new();
        for conn in data_state.connections.all_connections() {
            if !connection_pin_set.contains(&conn.from_pin)
                && !connection_pin_set.contains(&conn.to_pin)
            {
                continue;
            }
            if seen.insert((conn.from_pin, conn.to_pin)) {
                connections.push(ConnectionRebuildDTO {
                    from_pin: conn.from_pin.to_string(),
                    to_pin: conn.to_pin.to_string(),
                });
            }
        }

        GraphUndoPatch {
            nodes,
            neighbor_nodes,
            connections,
        }
    }

    fn apply_neighbor_patch(
        &self,
        neighbor_nodes: &[NodeSubgraphDTO],
    ) -> Vec<PinChangeSet> {
        if neighbor_nodes.is_empty() {
            return Vec::new();
        }
        let mut data_state = self.data_state.write().unwrap();
        let change_sets = Self::apply_neighbor_snapshots(&mut data_state, neighbor_nodes);
        GraphInstance::sync_static_pin_definitions(&mut data_state, self.registry());
        change_sets
    }

    fn reconnect_patch_connections(
        &self,
        connections: &[ConnectionRebuildDTO],
        mut seed_set: HashSet<NodeId>,
    ) -> Result<(Vec<(PinId, PinId)>, Vec<NodeId>), String> {
        let parse_pin = |s: &str| -> Result<PinId, String> {
            Uuid::parse_str(s)
                .map(PinId::from)
                .map_err(|e| format!("Invalid pin id '{s}': {e}"))
        };

        let mut established: Vec<(PinId, PinId)> = Vec::new();
        let mut failed_connections: Vec<ConnectionRebuildDTO> = Vec::new();

        {
            let data_state = self.data_state.write().unwrap();
            for conn in connections {
                let from_pin = parse_pin(&conn.from_pin)?;
                let to_pin = parse_pin(&conn.to_pin)?;
                if !data_state.pins.contains_key(&from_pin)
                    || !data_state.pins.contains_key(&to_pin)
                {
                    failed_connections.push(conn.clone());
                    continue;
                }

                if let Some(p) = data_state.pins.get(&from_pin) {
                    seed_set.insert(p.node_id);
                }
                if let Some(p) = data_state.pins.get(&to_pin) {
                    seed_set.insert(p.node_id);
                }

                let already = data_state
                    .connections
                    .get_downstream(from_pin)
                    .contains(&to_pin)
                    && data_state.connections.get_upstream(to_pin) == Some(from_pin);
                if already {
                    continue;
                }

                data_state.connections.connect(from_pin, to_pin);
                established.push((from_pin, to_pin));
            }
        }

        if !failed_connections.is_empty() {
            return Err(format!(
                "Failed to restore {} of {} connections",
                failed_connections.len(),
                connections.len()
            ));
        }

        Ok((established, seed_set.into_iter().collect()))
    }

    /// Apply a previously captured undo patch (delete undo / disconnect undo / composite redo).
    pub fn apply_graph_patch(
        &self,
        patch: GraphUndoPatch,
        variable_symbols: &HashMap<String, (String, DataType)>,
        dataframe_symbols: &HashMap<String, String>,
    ) -> Result<ApplyGraphPatchResult, String> {
        if !patch.nodes.is_empty() {
            let data_state = self.data_state.read().unwrap();
            for node in &patch.nodes {
                if data_state.nodes.contains_key(&node.id) {
                    return Err(format!("Node '{}' already exists in graph", node.id));
                }
            }
        }

        let neighbor_change_sets = self.apply_neighbor_patch(&patch.neighbor_nodes);

        let mut restored_ids: Vec<NodeId> = Vec::new();
        if !patch.nodes.is_empty() {
            let mut data_state = self.data_state.write().unwrap();
            for node_snap in &patch.nodes {
                let definition = self
                    .registry()
                    .get(&node_snap.node_type)
                    .unwrap_or_else(|| {
                        Arc::new(NodeDefinition::placeholder(node_snap.node_type.clone()))
                    });

                let pin_ids: Vec<PinId> = node_snap.pins.iter().map(|p| p.id).collect();

                for pin in &node_snap.pins {
                    data_state.connections.register_pin(pin.id, node_snap.id);
                    data_state.pins.insert(pin.id, pin.clone());
                }

                data_state.add_node(NodeInstance {
                    id: node_snap.id,
                    definition,
                    type_var_map: node_snap.type_var_map.clone(),
                    position: node_snap.position.clone(),
                    instance_params: node_snap.instance_params.clone(),
                    pin_ids,
                });
                restored_ids.push(node_snap.id);
            }

            GraphInstance::sync_static_pin_definitions(&mut data_state, self.registry());
        }

        self.resolve_variable_nodes(variable_symbols);
        self.resolve_dataframe_nodes(dataframe_symbols);

        let mut node_batches = Vec::new();
        if !patch.nodes.is_empty() {
            let data_state = self.data_state.read().unwrap();
            for node_snap in &patch.nodes {
                let Some(node_instance) = data_state.nodes.get(&node_snap.id) else {
                    continue;
                };
                let mut node_dto: crate::schema::NodeInstanceDTO = node_instance.into();
                let pin_instances: Vec<PinInstance> = node_snap
                    .pins
                    .iter()
                    .filter_map(|p| data_state.pins.get(&p.id).cloned())
                    .collect();
                let mut pins_dto = Vec::with_capacity(pin_instances.len());
                for pin in &pin_instances {
                    match pin.definition.direction {
                        PinDirection::Input => node_dto.inputs.push(pin.id.to_string()),
                        PinDirection::Output => node_dto.outputs.push(pin.id.to_string()),
                    }
                    let resolved_type = data_state.pin_types.get(&pin.id);
                    pins_dto.push(crate::schema::PinInstanceDTO::from_pin_with_context(
                        pin,
                        resolved_type,
                    ));
                }
                node_batches.push((node_snap.id, node_dto, pins_dto));
            }
        }

        let mut seed_set: HashSet<NodeId> = patch.neighbor_nodes.iter().map(|n| n.id).collect();
        seed_set.extend(restored_ids.iter().copied());

        let (established, seeds) =
            self.reconnect_patch_connections(&patch.connections, seed_set)?;

        let (mut change_sets, inferred) = if seeds.is_empty() {
            (Vec::new(), Vec::new())
        } else {
            self.finish_graph_effects_with_mode(&seeds, PinResolveMode::Materialize)
        };
        change_sets.splice(0..0, neighbor_change_sets);

        Ok(ApplyGraphPatchResult {
            node_batches,
            established_connections: established,
            change_sets,
            inferred,
        })
    }

    fn apply_neighbor_snapshots(
        data_state: &mut crate::graph::GraphDataState,
        neighbor_nodes: &[NodeSubgraphDTO],
    ) -> Vec<PinChangeSet> {
        let mut change_sets = Vec::new();

        for node_snap in neighbor_nodes {
            let Some(existing) = data_state.nodes.get(&node_snap.id) else {
                continue;
            };
            let snapshot_ids: HashSet<PinId> = node_snap.pins.iter().map(|p| p.id).collect();

            let mut removed_pin_ids = Vec::new();
            let mut removed_connections = Vec::new();
            for &pid in &existing.pin_ids {
                if snapshot_ids.contains(&pid) {
                    continue;
                }
                for to in data_state.connections.get_downstream(pid) {
                    removed_connections.push((pid, to));
                }
                if let Some(from) = data_state.connections.get_upstream(pid) {
                    removed_connections.push((from, pid));
                }
                data_state.connections.disconnect_all(pid);
                data_state.pins.remove(&pid);
                data_state.pin_types.remove(&pid);
                removed_pin_ids.push(pid);
            }

            let mut added_pins = Vec::new();
            let mut updated_pins = Vec::new();
            for pin in &node_snap.pins {
                data_state.connections.register_pin(pin.id, node_snap.id);
                let existed = data_state.pins.contains_key(&pin.id);
                data_state.pins.insert(pin.id, pin.clone());
                if existed {
                    updated_pins.push(pin.clone());
                } else {
                    added_pins.push(pin.clone());
                }
            }

            if let Some(node) = data_state.nodes.get_mut(&node_snap.id) {
                node.pin_ids = node_snap.pins.iter().map(|p| p.id).collect();
                node.type_var_map = node_snap.type_var_map.clone();
                node.instance_params = node_snap.instance_params.clone();
            }

            if !removed_pin_ids.is_empty() || !added_pins.is_empty() || !updated_pins.is_empty() {
                change_sets.push(PinChangeSet {
                    node_id: node_snap.id,
                    removed_pin_ids,
                    added_pins,
                    updated_pins,
                    removed_connections,
                });
            }
        }

        change_sets
    }

    fn capture_pin_resolver_closure(
        &self,
        node_ids: &[NodeId],
    ) -> (Vec<NodeSubgraphDTO>, Vec<ConnectionRebuildDTO>) {
        let data_state = self.data_state.read().unwrap();

        let mut neighbor_nodes = Vec::new();
        let mut pin_set: HashSet<PinId> = HashSet::new();

        for &nid in node_ids {
            let Some(node) = data_state.nodes.get(&nid) else {
                continue;
            };
            if node.definition.pin_resolver.is_none() {
                continue;
            }
            let pins: Vec<PinInstance> = node
                .pin_ids
                .iter()
                .filter_map(|pid| data_state.pins.get(pid).cloned())
                .collect();
            for pin in &pins {
                pin_set.insert(pin.id);
            }
            neighbor_nodes.push(NodeSubgraphDTO {
                id: nid,
                node_type: node.definition.node_type.clone(),
                position: node.position.clone(),
                type_var_map: node.type_var_map.clone(),
                instance_params: node.instance_params.clone(),
                pins,
            });
        }

        if pin_set.is_empty() {
            return (Vec::new(), Vec::new());
        }

        let mut connections = Vec::new();
        let mut seen: HashSet<(PinId, PinId)> = HashSet::new();
        for conn in data_state.connections.all_connections() {
            if !pin_set.contains(&conn.from_pin) && !pin_set.contains(&conn.to_pin) {
                continue;
            }
            if seen.insert((conn.from_pin, conn.to_pin)) {
                connections.push(ConnectionRebuildDTO {
                    from_pin: conn.from_pin.to_string(),
                    to_pin: conn.to_pin.to_string(),
                });
            }
        }

        (neighbor_nodes, connections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::node::{ColumnSchema, DataSchema, NodeInstanceParams, SchemaProvider};
    use crate::graph::pin::{DataRole, PinRole};
    use crate::graph::register::catalog::register_builtin_nodes;
    use crate::graph::register::NodeRegistry;
    use crate::graph::value::DataType;
    use crate::graph::GraphKind;

    fn test_graph() -> GraphInstance {
        let registry = Arc::new(NodeRegistry::new());
        register_builtin_nodes(&registry);
        GraphInstance::new("test", GraphKind::Event, registry)
    }

    fn test_graph_with_schema() -> GraphInstance {
        let registry = Arc::new(NodeRegistry::new());
        register_builtin_nodes(&registry);
        let mut graph = GraphInstance::new("test", GraphKind::Event, registry);
        let provider: SchemaProvider = Arc::new(|dataframe_id| {
            if dataframe_id == "test_df" {
                Some(DataSchema {
                    columns: vec![
                        ColumnSchema {
                            name: "time".to_string(),
                            data_type: DataType::Int64,
                        },
                        ColumnSchema {
                            name: "value".to_string(),
                            data_type: DataType::Float64,
                        },
                    ],
                })
            } else {
                None
            }
        });
        graph.set_schema_provider(provider);
        graph
    }

    #[test]
    fn capture_and_merge_roundtrip_preserves_node_and_connection() {
        let graph = test_graph();
        let n1 = graph.create_node("Value:Constants:Int64").unwrap();
        let n2 = graph.create_node("Math:Operators:Add (+)").unwrap();
        let pins1 = graph.get_pin_instances_by_node_id(n1);
        let pins2 = graph.get_pin_instances_by_node_id(n2);
        let out = pins1
            .iter()
            .find(|p| p.is_output() && p.is_data())
            .unwrap()
            .id;
        let inp = pins2
            .iter()
            .find(|p| p.is_input() && p.is_data())
            .unwrap()
            .id;
        graph.connect(out, inp).unwrap();

        let snapshot = graph.capture_subgraph_for_delete(&[n1]);
        assert_eq!(snapshot.nodes.len(), 1);
        assert_eq!(snapshot.connections.len(), 1);

        graph.remove_node_raw(n1).unwrap();
        assert!(graph.get_node_instance(n1).is_none());

        let result = graph
            .apply_graph_patch(snapshot, &HashMap::new(), &HashMap::new())
            .unwrap();
        assert_eq!(result.node_batches.len(), 1);
        assert!(graph.get_node_instance(n1).is_some());
        assert_eq!(result.established_connections.len(), 1);
    }

    #[test]
    fn capture_merge_roundtrip_preserves_hub_connections() {
        let graph = test_graph();
        let hub = graph.create_node("Value:Constants:Int64").unwrap();
        let hub_out = graph
            .get_pin_instances_by_node_id(hub)
            .into_iter()
            .find(|p| p.is_output() && p.is_data())
            .unwrap()
            .id;

        let mut targets = Vec::new();
        for _ in 0..50 {
            let n = graph.create_node("Math:Operators:Add (+)").unwrap();
            let inp = graph
                .get_pin_instances_by_node_id(n)
                .into_iter()
                .find(|p| p.is_input() && p.is_data())
                .unwrap()
                .id;
            graph.connect(hub_out, inp).unwrap();
            targets.push(n);
        }

        let snapshot = graph.capture_subgraph_for_delete(&[hub]);
        assert_eq!(snapshot.nodes.len(), 1);
        assert_eq!(snapshot.connections.len(), 50);

        graph.remove_node_raw(hub).unwrap();

        let result = graph
            .apply_graph_patch(snapshot, &HashMap::new(), &HashMap::new())
            .unwrap();
        assert_eq!(result.established_connections.len(), 50);
        assert!(graph.get_node_instance(hub).is_some());
        for n in targets {
            assert!(graph.get_node_instance(n).is_some());
        }
    }

    #[test]
    fn delete_undo_preserves_decompose_columns_and_downstream_without_neighbor_resolve() {
        let graph = test_graph_with_schema();

        let source = graph
            .create_node_with_position(
                "Data:Get DataFrame",
                0.0,
                0.0,
                Some(NodeInstanceParams::DataFrame {
                    dataframe_id: "test_df".to_string(),
                }),
            )
            .unwrap();
        let decompose = graph.create_node("Data:Decompose DataFrame").unwrap();
        let add_time = graph.create_node("Math:Operators:Add (+)").unwrap();
        let add_value = graph.create_node("Math:Operators:Add (+)").unwrap();

        let source_out = graph
            .get_pin_instances_by_node_id(source)
            .into_iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Output))
            .unwrap()
            .id;
        let decompose_in = graph
            .get_pin_instances_by_node_id(decompose)
            .into_iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
            .unwrap()
            .id;

        graph.connect(source_out, decompose_in).unwrap();
        graph.materialize_dynamic_pins();

        let decompose_pins: HashMap<String, PinId> = graph
            .get_pin_instances_by_node_id(decompose)
            .into_iter()
            .filter(|p| p.is_output() && p.is_data())
            .map(|p| (p.definition.name.clone(), p.id))
            .collect();
        assert!(decompose_pins.contains_key("time"));
        assert!(decompose_pins.contains_key("value"));

        let add_time_in = graph
            .get_pin_instances_by_node_id(add_time)
            .into_iter()
            .find(|p| p.is_input() && p.is_data())
            .unwrap()
            .id;
        let add_value_in = graph
            .get_pin_instances_by_node_id(add_value)
            .into_iter()
            .find(|p| p.is_input() && p.is_data())
            .unwrap()
            .id;

        graph
            .connect(decompose_pins["time"], add_time_in)
            .unwrap();
        graph
            .connect(decompose_pins["value"], add_value_in)
            .unwrap();

        let snapshot = graph.capture_subgraph_for_delete(&[source]);
        assert_eq!(snapshot.nodes.len(), 1);
        assert!(
            snapshot.connections.len() >= 3,
            "closure should include source->decompose and column fan-out"
        );
        assert!(
            snapshot.neighbor_nodes.iter().any(|n| n.id == decompose),
            "decompose should be captured as neighbor"
        );

        graph.remove_node_raw(source).unwrap();
        // Intentionally skip resolve_dynamic_pins on decompose (batch_delete policy).

        let columns_after_delete: HashMap<String, PinId> = graph
            .get_pin_instances_by_node_id(decompose)
            .into_iter()
            .filter(|p| p.is_output() && p.is_data())
            .map(|p| (p.definition.name.clone(), p.id))
            .collect();
        assert_eq!(columns_after_delete.get("time"), decompose_pins.get("time"));
        assert_eq!(columns_after_delete.get("value"), decompose_pins.get("value"));

        let ds = graph.data_state.read().unwrap();
        assert!(ds
            .connections
            .get_downstream(decompose_pins["time"])
            .contains(&add_time_in));
        assert!(ds
            .connections
            .get_downstream(decompose_pins["value"])
            .contains(&add_value_in));
        drop(ds);

        let merge_result = graph
            .apply_graph_patch(snapshot, &HashMap::new(), &HashMap::new())
            .unwrap();
        assert!(graph.get_node_instance(source).is_some());
        assert_eq!(merge_result.established_connections.len(), 1);

        let ds = graph.data_state.read().unwrap();
        assert!(ds
            .connections
            .get_downstream(source_out)
            .contains(&decompose_in));
        assert!(ds
            .connections
            .get_downstream(decompose_pins["time"])
            .contains(&add_time_in));
        assert!(ds
            .connections
            .get_downstream(decompose_pins["value"])
            .contains(&add_value_in));
    }

    #[test]
    fn delete_undo_restores_decompose_node_with_column_fanout() {
        let graph = test_graph_with_schema();

        let source = graph
            .create_node_with_position(
                "Data:Get DataFrame",
                0.0,
                0.0,
                Some(NodeInstanceParams::DataFrame {
                    dataframe_id: "test_df".to_string(),
                }),
            )
            .unwrap();
        let decompose = graph.create_node("Data:Decompose DataFrame").unwrap();
        let add_time = graph.create_node("Math:Operators:Add (+)").unwrap();
        let add_value = graph.create_node("Math:Operators:Add (+)").unwrap();

        let source_out = graph
            .get_pin_instances_by_node_id(source)
            .into_iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Output))
            .unwrap()
            .id;
        let decompose_in = graph
            .get_pin_instances_by_node_id(decompose)
            .into_iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
            .unwrap()
            .id;
        graph.connect(source_out, decompose_in).unwrap();
        graph.materialize_dynamic_pins();

        let decompose_pins: HashMap<String, PinId> = graph
            .get_pin_instances_by_node_id(decompose)
            .into_iter()
            .filter(|p| p.is_output() && p.is_data())
            .map(|p| (p.definition.name.clone(), p.id))
            .collect();

        let add_time_in = graph
            .get_pin_instances_by_node_id(add_time)
            .into_iter()
            .find(|p| p.is_input() && p.is_data())
            .unwrap()
            .id;
        let add_value_in = graph
            .get_pin_instances_by_node_id(add_value)
            .into_iter()
            .find(|p| p.is_input() && p.is_data())
            .unwrap()
            .id;
        graph.connect(decompose_pins["time"], add_time_in).unwrap();
        graph.connect(decompose_pins["value"], add_value_in).unwrap();

        let snapshot = graph.capture_subgraph_for_delete(&[decompose]);
        assert_eq!(snapshot.nodes.len(), 1);
        assert_eq!(snapshot.nodes[0].pins.len(), 3); // input + time + value

        graph.remove_node_raw(decompose).unwrap();

        let merge_result = graph
            .apply_graph_patch(snapshot, &HashMap::new(), &HashMap::new())
            .unwrap();
        assert!(graph.get_node_instance(decompose).is_some());
        assert_eq!(merge_result.established_connections.len(), 3);

        let restored_cols: HashMap<String, PinId> = graph
            .get_pin_instances_by_node_id(decompose)
            .into_iter()
            .filter(|p| p.is_output() && p.is_data())
            .map(|p| (p.definition.name.clone(), p.id))
            .collect();
        assert_eq!(restored_cols.get("time"), decompose_pins.get("time"));
        assert_eq!(restored_cols.get("value"), decompose_pins.get("value"));

        let ds = graph.data_state.read().unwrap();
        assert!(ds
            .connections
            .get_downstream(decompose_pins["time"])
            .contains(&add_time_in));
        assert!(ds
            .connections
            .get_downstream(source_out)
            .contains(&decompose_in));
    }

    #[test]
    fn delete_undo_neighbor_patch_restores_decompose_after_interactive_resolve() {
        let graph = test_graph_with_schema();

        let source = graph
            .create_node_with_position(
                "Data:Get DataFrame",
                0.0,
                0.0,
                Some(NodeInstanceParams::DataFrame {
                    dataframe_id: "test_df".to_string(),
                }),
            )
            .unwrap();
        let decompose = graph.create_node("Data:Decompose DataFrame").unwrap();

        let source_out = graph
            .get_pin_instances_by_node_id(source)
            .into_iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Output))
            .unwrap()
            .id;
        let decompose_in = graph
            .get_pin_instances_by_node_id(decompose)
            .into_iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
            .unwrap()
            .id;
        graph.connect(source_out, decompose_in).unwrap();
        graph.materialize_dynamic_pins();

        let before_cols: Vec<String> = graph
            .get_pin_instances_by_node_id(decompose)
            .into_iter()
            .filter(|p| p.is_output() && p.is_data())
            .map(|p| p.definition.name.clone())
            .collect();

        let snapshot = graph.capture_subgraph_for_delete(&[source]);
        graph.remove_node_raw(source).unwrap();
        let _ = graph.resolve_dynamic_pins(decompose);

        let cols_after_resolve: Vec<String> = graph
            .get_pin_instances_by_node_id(decompose)
            .into_iter()
            .filter(|p| p.is_output() && p.is_data())
            .map(|p| p.definition.name.clone())
            .collect();
        assert_ne!(cols_after_resolve, before_cols, "test setup: resolve should mutate neighbor");

        graph
            .apply_graph_patch(snapshot, &HashMap::new(), &HashMap::new())
            .unwrap();

        let restored_cols: Vec<String> = graph
            .get_pin_instances_by_node_id(decompose)
            .into_iter()
            .filter(|p| p.is_output() && p.is_data())
            .map(|p| p.definition.name.clone())
            .collect();
        assert_eq!(restored_cols, before_cols);
    }

    #[test]
    fn disconnect_undo_restores_decompose_with_column_fanout() {
        let graph = test_graph_with_schema();

        let source = graph
            .create_node_with_position(
                "Data:Get DataFrame",
                0.0,
                0.0,
                Some(NodeInstanceParams::DataFrame {
                    dataframe_id: "test_df".to_string(),
                }),
            )
            .unwrap();
        let decompose = graph.create_node("Data:Decompose DataFrame").unwrap();
        let add_time = graph.create_node("Math:Operators:Add (+)").unwrap();
        let add_value = graph.create_node("Math:Operators:Add (+)").unwrap();

        let source_out = graph
            .get_pin_instances_by_node_id(source)
            .into_iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Output))
            .unwrap()
            .id;
        let decompose_in = graph
            .get_pin_instances_by_node_id(decompose)
            .into_iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
            .unwrap()
            .id;

        graph.connect(source_out, decompose_in).unwrap();
        graph.materialize_dynamic_pins();

        let decompose_pins: HashMap<String, PinId> = graph
            .get_pin_instances_by_node_id(decompose)
            .into_iter()
            .filter(|p| p.is_output() && p.is_data())
            .map(|p| (p.definition.name.clone(), p.id))
            .collect();
        assert!(decompose_pins.contains_key("time"));
        assert!(decompose_pins.contains_key("value"));

        let add_time_in = graph
            .get_pin_instances_by_node_id(add_time)
            .into_iter()
            .find(|p| p.is_input() && p.is_data())
            .unwrap()
            .id;
        let add_value_in = graph
            .get_pin_instances_by_node_id(add_value)
            .into_iter()
            .find(|p| p.is_input() && p.is_data())
            .unwrap()
            .id;

        graph
            .connect(decompose_pins["time"], add_time_in)
            .unwrap();
        graph
            .connect(decompose_pins["value"], add_value_in)
            .unwrap();

        let (removed, undo_patch, _change_sets, _inferred) = graph.disconnect_pin(decompose_in);

        assert_eq!(removed.len(), 1);
        assert!(
            undo_patch.neighbor_nodes.iter().any(|n| n.id == decompose),
            "decompose should be captured before disconnect"
        );
        assert!(
            undo_patch.connections.len() >= 3,
            "closure should include source->decompose and column fan-out"
        );

        let columns_after_disconnect: Vec<String> = graph
            .get_pin_instances_by_node_id(decompose)
            .into_iter()
            .filter(|p| p.is_output() && p.is_data())
            .map(|p| p.definition.name.clone())
            .collect();
        assert!(
            columns_after_disconnect.is_empty()
                || columns_after_disconnect != vec!["time".to_string(), "value".to_string()],
            "disconnect should strip or shrink decompose columns"
        );

        let ds = graph.data_state.read().unwrap();
        assert!(!ds
            .connections
            .get_downstream(decompose_pins["time"])
            .contains(&add_time_in));
        drop(ds);

        graph
            .apply_graph_patch(undo_patch, &HashMap::new(), &HashMap::new())
            .unwrap();

        let restored_cols: Vec<String> = graph
            .get_pin_instances_by_node_id(decompose)
            .into_iter()
            .filter(|p| p.is_output() && p.is_data())
            .map(|p| p.definition.name.clone())
            .collect();
        assert_eq!(restored_cols, vec!["time".to_string(), "value".to_string()]);

        let ds = graph.data_state.read().unwrap();
        assert!(ds
            .connections
            .get_downstream(source_out)
            .contains(&decompose_in));
        assert!(ds
            .connections
            .get_downstream(decompose_pins["time"])
            .contains(&add_time_in));
        assert!(ds
            .connections
            .get_downstream(decompose_pins["value"])
            .contains(&add_value_in));
    }
}
