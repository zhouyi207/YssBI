use std::collections::btree_map::Entry;
use uuid::Uuid;

use super::{ResultEntry, ResultId, ResultStore, ResultStoreRegistry};
use crate::result::{ResultRetentionError, StoredResultSnapshot};

impl ResultStoreRegistry {
    fn collect(&mut self, id: ResultId, retired: &mut Vec<ResultEntry>) {
        if let Entry::Occupied(entry) = self.values.entry(id)
            && !entry.get().cached
            && entry.get().leases.is_empty()
        {
            retired.push(entry.remove());
        }
    }

    pub(super) fn detach(&mut self, id: ResultId, retired: &mut Vec<ResultEntry>) {
        if let Some(entry) = self.values.get_mut(&id) {
            entry.cached = false;
            self.collect(id, retired);
        }
    }
}

impl ResultStore {
    /// Keeps a result readable after its output changes until the returned lease is released.
    pub fn retain(
        &self,
        id: ResultId,
    ) -> Result<(Uuid, StoredResultSnapshot), ResultRetentionError> {
        self.update(|registry, _retired| {
            let entry = registry
                .values
                .get_mut(&id)
                .ok_or(ResultRetentionError::Unavailable)?;
            let lease = Uuid::new_v4();
            entry.leases.insert(lease);
            registry.leases.insert(lease, id);
            Ok((lease, entry.snapshot.clone()))
        })
    }

    pub fn release(&self, lease: Uuid) {
        self.update(|registry, retired| {
            let Some(id) = registry.leases.remove(&lease) else {
                return;
            };
            if let Some(entry) = registry.values.get_mut(&id) {
                entry.leases.remove(&lease);
                registry.collect(id, retired);
            }
        })
    }
}
