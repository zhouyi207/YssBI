use crate::graph::PinId;

use super::ResultSourceStore;

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

    #[test]
    fn empty_pin_invalidation_has_no_effect() {
        let store = ResultSourceStore::new();
        assert!(apply_runtime_pin_invalidation(&store, "events/main.yss", &[]).is_empty());
    }
}
