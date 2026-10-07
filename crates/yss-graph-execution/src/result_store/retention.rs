use std::collections::{BTreeSet, btree_map::Entry};
use uuid::Uuid;

use super::{ResultEntry, ResultId, ResultLease, ResultStore, ResultStoreRegistry};
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

    fn unindex_owner(&mut self, owner: &str, lease_id: Uuid) {
        if let Some(leases) = self.owner_leases.get_mut(owner) {
            leases.remove(&lease_id);
            if leases.is_empty() {
                self.owner_leases.remove(owner);
            }
        }
    }

    fn remove_lease(&mut self, lease_id: Uuid, retired: &mut Vec<ResultEntry>) {
        let Some(lease) = self.leases.remove(&lease_id) else {
            return;
        };
        self.unindex_owner(&lease.owner, lease_id);
        if let Some(target) = lease.handoff {
            self.unindex_owner(&target, lease_id);
        }
        if let Some(entry) = self.values.get_mut(&lease.result_id) {
            entry.leases.remove(&lease_id);
            self.collect(lease.result_id, retired);
        }
    }
}

impl ResultStore {
    pub fn retain(
        &self,
        id: ResultId,
        lease_id: Uuid,
        owner: &str,
        handoff: Option<&str>,
    ) -> Result<StoredResultSnapshot, ResultRetentionError> {
        self.update(|registry, _retired| {
            if registry.closed_owners.contains(owner)
                || handoff.is_some_and(|target| registry.closed_owners.contains(target))
            {
                return Err(ResultRetentionError::OwnerClosed);
            }
            let snapshot = registry
                .values
                .get(&id)
                .ok_or(ResultRetentionError::Unavailable)?
                .snapshot
                .clone();
            if let Some(existing) = registry.leases.get(&lease_id) {
                return if existing.result_id == id
                    && existing.owner.as_ref() == owner
                    && existing.handoff.as_deref() == handoff
                {
                    Ok(snapshot)
                } else {
                    Err(ResultRetentionError::LeaseConflict)
                };
            }
            registry
                .values
                .get_mut(&id)
                .unwrap()
                .leases
                .insert(lease_id);
            registry
                .owner_leases
                .entry(owner.into())
                .or_default()
                .insert(lease_id);
            if let Some(target) = handoff {
                registry
                    .owner_leases
                    .entry(target.into())
                    .or_default()
                    .insert(lease_id);
            }
            registry.leases.insert(
                lease_id,
                ResultLease {
                    result_id: id,
                    owner: owner.into(),
                    handoff: handoff.map(Into::into),
                },
            );
            Ok(snapshot)
        })
    }

    pub fn claim(
        &self,
        lease_id: Uuid,
        owner: &str,
    ) -> Result<StoredResultSnapshot, ResultRetentionError> {
        self.update(|registry, _retired| {
            let lease = registry
                .leases
                .get(&lease_id)
                .ok_or(ResultRetentionError::Unavailable)?;
            let id = lease.result_id;
            if lease.owner.as_ref() == owner && lease.handoff.is_none() {
                return registry
                    .values
                    .get(&id)
                    .map(|entry| entry.snapshot.clone())
                    .ok_or(ResultRetentionError::Unavailable);
            }
            if lease.handoff.as_deref() != Some(owner) {
                return Err(ResultRetentionError::WrongOwner);
            }
            let old_owner = lease.owner.clone();
            registry.unindex_owner(&old_owner, lease_id);
            let lease = registry.leases.get_mut(&lease_id).unwrap();
            lease.owner = owner.into();
            lease.handoff = None;
            registry
                .owner_leases
                .entry(owner.into())
                .or_default()
                .insert(lease_id);
            Ok(registry.values[&id].snapshot.clone())
        })
    }

    pub fn release(&self, lease_id: Uuid, owner: &str) -> Result<(), ResultRetentionError> {
        self.update(|registry, retired| {
            if let Some(lease) = registry.leases.get(&lease_id) {
                if lease.owner.as_ref() != owner {
                    return Err(ResultRetentionError::WrongOwner);
                }
                registry.remove_lease(lease_id, retired);
            }
            Ok(())
        })
    }

    pub fn reconcile(&self, owner: &str, active: &BTreeSet<Uuid>) {
        self.update(|registry, retired| {
            let removed = registry
                .owner_leases
                .get(owner)
                .into_iter()
                .flatten()
                .copied()
                .filter(|id| {
                    !active.contains(id)
                        && registry.leases.get(id).is_some_and(|lease| {
                            lease.owner.as_ref() == owner && lease.handoff.is_none()
                        })
                })
                .collect::<Vec<_>>();
            for id in removed {
                registry.remove_lease(id, retired);
            }
        })
    }

    pub fn close_owner(&self, owner: &str) {
        self.update(|registry, retired| {
            registry.closed_owners.insert(owner.into());
            let ids = registry
                .owner_leases
                .get(owner)
                .cloned()
                .unwrap_or_default();
            // Includes unclaimed handoffs: destroying either endpoint must not leak a lease.
            for id in ids {
                registry.remove_lease(id, retired);
            }
        })
    }
}
