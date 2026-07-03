use crate::event::{emit_project_event, Event, EventNode};
use crate::execution::{
    apply_runtime_pin_invalidation, collect_invalidation_pins, ResultSourceStore,
};
use crate::graph::{GraphId, PinChangeSet, PinId};
use tauri::AppHandle;

pub fn emit_runtime_source_invalidation(
    app: &AppHandle,
    store: &ResultSourceStore,
    graph_id: GraphId,
    change_sets: &[PinChangeSet],
    deleted_node_pin_ids: &[PinId],
) {
    let pin_ids = collect_invalidation_pins(change_sets, deleted_node_pin_ids);
    if pin_ids.is_empty() {
        return;
    }
    let invalidated = apply_runtime_pin_invalidation(store, graph_id, &pin_ids);
    if invalidated.is_empty() {
        return;
    }
    emit_project_event(
        app,
        Event::Node(EventNode::RuntimeSourcesInvalidated {
            graph_id,
            pin_ids: invalidated,
        }),
    );
}
