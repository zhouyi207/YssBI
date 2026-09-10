use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};

use crate::DatasetSnapshot;

#[derive(Default)]
pub(crate) struct SnapshotLeases {
    snapshots: BTreeMap<Box<str>, Vec<Weak<DatasetSnapshot>>>,
    preparations: BTreeMap<PathBuf, Weak<()>>,
}

impl SnapshotLeases {
    pub fn retain(&mut self, snapshot: &Arc<DatasetSnapshot>) {
        let leases = self
            .snapshots
            .entry(snapshot.metadata().snapshot_id.clone())
            .or_default();
        leases.retain(|lease| lease.strong_count() > 0);
        leases.push(Arc::downgrade(snapshot));
    }
    pub fn protected(&mut self) -> BTreeSet<Box<str>> {
        self.snapshots.retain(|_, leases| {
            leases.retain(|lease| lease.strong_count() > 0);
            !leases.is_empty()
        });
        self.snapshots.keys().cloned().collect()
    }

    pub fn prepare(&mut self, directory: PathBuf) -> Arc<()> {
        let lease = Arc::new(());
        self.preparations.insert(directory, Arc::downgrade(&lease));
        lease
    }

    pub fn prepared_directories(&mut self) -> BTreeSet<PathBuf> {
        self.preparations
            .retain(|_, lease| lease.strong_count() > 0);
        self.preparations.keys().cloned().collect()
    }
}

type LeaseRegistry = BTreeMap<PathBuf, Weak<Mutex<SnapshotLeases>>>;

pub(crate) fn for_root(root: &Path) -> Arc<Mutex<SnapshotLeases>> {
    static REGISTRY: OnceLock<Mutex<LeaseRegistry>> = OnceLock::new();
    let mut registry = REGISTRY
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    registry.retain(|_, leases| leases.strong_count() > 0);
    if let Some(leases) = registry.get(root).and_then(Weak::upgrade) {
        return leases;
    }
    let leases = Arc::new(Mutex::new(SnapshotLeases::default()));
    registry.insert(root.to_owned(), Arc::downgrade(&leases));
    leases
}
