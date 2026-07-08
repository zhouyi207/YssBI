use crate::event::event_node::InferredPinType;
use crate::event::{Event, EventNode, emit_project_event};
use crate::execution::{
    ResultSourceStore, apply_runtime_pin_invalidation, collect_invalidation_pins,
};
use crate::graph::{DataType, GraphInstance, PinChangeSet, PinId};
use crate::project::GraphResourcePath;
use crate::schema::PinInstanceDTO;
use crate::schema::pin::{data_type_to_container, data_type_to_pin_type};
use tauri::AppHandle;

pub fn emit_pin_change_events(
    app: &AppHandle,
    graph_path: &GraphResourcePath,
    graph: &GraphInstance,
    change_sets: &[PinChangeSet],
) {
    for cs in change_sets {
        let added_dtos: Vec<PinInstanceDTO> = cs
            .added_pins
            .iter()
            .map(|pin| {
                let resolved_type = graph.get_pin_data_type_by_pin_id(pin.id);
                PinInstanceDTO::from_pin_with_context(pin, resolved_type.as_ref())
            })
            .collect();

        let updated_dtos: Vec<PinInstanceDTO> = cs
            .updated_pins
            .iter()
            .map(|pin| {
                let resolved_type = graph.get_pin_data_type_by_pin_id(pin.id);
                PinInstanceDTO::from_pin_with_context(pin, resolved_type.as_ref())
            })
            .collect();

        let removed_pin_ids: Vec<PinId> = cs.removed_pin_ids.clone();

        let pin_order = graph
            .get_node_instance(cs.node_id)
            .map(|node| node.pin_ids.clone());

        emit_project_event(
            app,
            Event::Node(EventNode::NodePinsUpdated {
                graph_path: graph_path.as_str().to_string(),
                node_id: cs.node_id,
                removed_pin_ids,
                added_pins: added_dtos,
                updated_pins: updated_dtos,
                removed_connections: cs.removed_connections.clone(),
                pin_order,
            }),
        );
    }
}

pub fn emit_inferred_types(
    app: &AppHandle,
    graph_path: &GraphResourcePath,
    inferred: Vec<(PinId, DataType)>,
) {
    if inferred.is_empty() {
        return;
    }
    let pin_types: Vec<InferredPinType> = inferred
        .into_iter()
        .map(|(pin_id, dt)| InferredPinType {
            pin_id,
            pin_type: data_type_to_pin_type(&dt).to_string(),
            container_type: data_type_to_container(&dt).map(|s| s.to_string()),
            type_display: Some(dt.to_string()),
            data_type: Some(dt.clone()),
        })
        .collect();
    emit_project_event(
        app,
        Event::Node(EventNode::PinTypesInferred {
            graph_path: graph_path.as_str().to_string(),
            pin_types,
        }),
    );
}

pub fn emit_runtime_source_invalidation(
    app: &AppHandle,
    store: &ResultSourceStore,
    graph_path: &GraphResourcePath,
    change_sets: &[PinChangeSet],
    deleted_node_pin_ids: &[PinId],
) {
    let pin_ids = collect_invalidation_pins(change_sets, deleted_node_pin_ids);
    if pin_ids.is_empty() {
        return;
    }
    let invalidated = apply_runtime_pin_invalidation(store, graph_path.as_str(), &pin_ids);
    if invalidated.is_empty() {
        return;
    }
    emit_project_event(
        app,
        Event::Node(EventNode::RuntimeSourcesInvalidated {
            graph_path: graph_path.as_str().to_string(),
            pin_ids: invalidated,
        }),
    );
}

/// Standard post-mutation fan-out after connect/disconnect/pin-structure changes.
pub fn emit_graph_pin_mutation_sync(
    app: &AppHandle,
    source_store: &ResultSourceStore,
    graph_path: &GraphResourcePath,
    graph: &GraphInstance,
    change_sets: &[PinChangeSet],
    inferred: Vec<(PinId, DataType)>,
    deleted_pin_ids: &[PinId],
) {
    emit_pin_change_events(app, graph_path, graph, change_sets);
    emit_inferred_types(app, graph_path, inferred);
    emit_runtime_source_invalidation(app, source_store, graph_path, change_sets, deleted_pin_ids);
}
