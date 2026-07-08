use std::collections::HashSet;

use crate::graph::{PinChangeSet, PinId};

use super::ResultSourceStore;

/// Merge pin ids from dynamic pin change sets and deleted-node pin lists.
pub fn collect_invalidation_pins(
    change_sets: &[PinChangeSet],
    deleted_node_pin_ids: &[PinId],
) -> Vec<PinId> {
    let mut pins = HashSet::new();
    for cs in change_sets {
        pins.extend(cs.removed_pin_ids.iter().copied());
    }
    pins.extend(deleted_node_pin_ids.iter().copied());
    pins.into_iter().collect()
}

/// Invalidate runtime pin sources and return pin ids that were actually removed.
pub fn apply_runtime_pin_invalidation(
    store: &ResultSourceStore,
    graph_path: &str,
    pin_ids: &[PinId],
) -> Vec<PinId> {
    if pin_ids.is_empty() {
        return Vec::new();
    }
    store.invalidate_runtime_pins(graph_path, pin_ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn collect_invalidation_pins_merges_change_sets_and_deleted_pins() {
        let pin_a = PinId::from(Uuid::new_v4());
        let pin_b = PinId::from(Uuid::new_v4());
        let pin_c = PinId::from(Uuid::new_v4());
        let node_id = crate::graph::NodeId::from(Uuid::new_v4());

        let change_sets = vec![
            PinChangeSet {
                node_id,
                removed_pin_ids: vec![pin_a, pin_b],
                ..Default::default()
            },
            PinChangeSet {
                node_id,
                removed_pin_ids: vec![pin_b],
                ..Default::default()
            },
        ];

        let mut collected = collect_invalidation_pins(&change_sets, &[pin_c]);
        collected.sort_by_key(|id| id.to_string());
        let mut expected = vec![pin_a, pin_b, pin_c];
        expected.sort_by_key(|id| id.to_string());
        assert_eq!(collected, expected);
    }
}
