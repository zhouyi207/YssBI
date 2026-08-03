use crate::node_system::analysis::{CompilationBasis, CorrelationContext, RunId};
use crate::node_system::document::GraphRevision;
use crate::node_system::protocol::Value;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArtifactId(u64);

impl ArtifactId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactSnapshot {
    Value(Value),
    Sequence(Box<[Value]>),
}

impl ArtifactSnapshot {
    pub fn len(&self) -> usize {
        match self {
            Self::Value(_) => 1,
            Self::Sequence(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn page(&self, offset: usize, limit: usize) -> Box<[Value]> {
        let start = offset.min(self.len());
        let end = start.saturating_add(limit).min(self.len());
        match self {
            Self::Value(value) if start == 0 && end == 1 => vec![value.clone()].into_boxed_slice(),
            Self::Value(_) => Box::default(),
            Self::Sequence(values) => values[start..end].to_vec().into_boxed_slice(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactSnapshotKind {
    Value,
    Sequence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDescriptor {
    pub artifact_id: ArtifactId,
    pub kind: ArtifactSnapshotKind,
    pub total_count: usize,
    pub correlation: CorrelationContext,
    pub basis: CompilationBasis<GraphRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactPage {
    pub artifact_id: ArtifactId,
    pub offset: usize,
    pub limit: usize,
    pub total_count: usize,
    pub values: Box<[Value]>,
}

struct ArtifactEntry {
    run_id: RunId,
    run_owned: bool,
    result_source_holds: usize,
    descriptor: ArtifactDescriptor,
    snapshot: Arc<ArtifactSnapshot>,
}

#[derive(Default)]
struct ArtifactRegistry {
    entries: BTreeMap<ArtifactId, ArtifactEntry>,
}

#[derive(Default)]
struct ArtifactStoreInner {
    next_id: AtomicU64,
    registry: Mutex<ArtifactRegistry>,
}

/// Stores immutable snapshots. Run cleanup removes run-only artifacts, while an
/// explicit result-source hold keeps a snapshot alive until that source is released.
#[derive(Clone, Default)]
pub struct ArtifactStore {
    inner: Arc<ArtifactStoreInner>,
}

impl ArtifactStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &self,
        run_id: RunId,
        correlation: CorrelationContext,
        basis: CompilationBasis<GraphRevision>,
        snapshot: ArtifactSnapshot,
    ) -> ArtifactDescriptor {
        self.insert_with_result_source_holds(run_id, correlation, basis, snapshot, 0)
    }

    pub(crate) fn insert_retained_result_source(
        &self,
        run_id: RunId,
        correlation: CorrelationContext,
        basis: CompilationBasis<GraphRevision>,
        snapshot: ArtifactSnapshot,
    ) -> ArtifactDescriptor {
        self.insert_with_result_source_holds(run_id, correlation, basis, snapshot, 1)
    }

    fn insert_with_result_source_holds(
        &self,
        run_id: RunId,
        correlation: CorrelationContext,
        basis: CompilationBasis<GraphRevision>,
        snapshot: ArtifactSnapshot,
        result_source_holds: usize,
    ) -> ArtifactDescriptor {
        let artifact_id = ArtifactId::new(self.inner.next_id.fetch_add(1, Ordering::Relaxed) + 1);
        let descriptor = ArtifactDescriptor {
            artifact_id,
            kind: match &snapshot {
                ArtifactSnapshot::Value(_) => ArtifactSnapshotKind::Value,
                ArtifactSnapshot::Sequence(_) => ArtifactSnapshotKind::Sequence,
            },
            total_count: snapshot.len(),
            correlation,
            basis,
        };
        self.registry().entries.insert(
            artifact_id,
            ArtifactEntry {
                run_id,
                run_owned: true,
                result_source_holds,
                descriptor: descriptor.clone(),
                snapshot: Arc::new(snapshot),
            },
        );
        descriptor
    }

    pub fn descriptor(&self, artifact_id: ArtifactId) -> Option<ArtifactDescriptor> {
        self.registry()
            .entries
            .get(&artifact_id)
            .map(|entry| entry.descriptor.clone())
    }

    pub fn snapshot(&self, artifact_id: ArtifactId) -> Option<Arc<ArtifactSnapshot>> {
        self.registry()
            .entries
            .get(&artifact_id)
            .map(|entry| Arc::clone(&entry.snapshot))
    }

    pub fn page(
        &self,
        artifact_id: ArtifactId,
        offset: usize,
        limit: usize,
    ) -> Option<ArtifactPage> {
        let limit = limit.max(1);
        let registry = self.registry();
        let entry = registry.entries.get(&artifact_id)?;
        let start = offset.min(entry.snapshot.len());
        Some(ArtifactPage {
            artifact_id,
            offset: start,
            limit,
            total_count: entry.snapshot.len(),
            values: entry.snapshot.page(start, limit),
        })
    }

    pub fn retain_result_source(&self, artifact_id: ArtifactId) -> bool {
        let mut registry = self.registry();
        let Some(entry) = registry.entries.get_mut(&artifact_id) else {
            return false;
        };
        entry.result_source_holds = entry.result_source_holds.saturating_add(1);
        true
    }

    pub fn release(&self, artifact_id: ArtifactId) -> bool {
        let mut registry = self.registry();
        let remove = {
            let Some(entry) = registry.entries.get_mut(&artifact_id) else {
                return false;
            };
            if entry.result_source_holds == 0 {
                return false;
            }
            entry.result_source_holds -= 1;
            !entry.run_owned && entry.result_source_holds == 0
        };
        if remove {
            registry.entries.remove(&artifact_id);
        }
        true
    }

    /// Ends run ownership without invalidating snapshots retained by result sources.
    pub fn cleanup_run(&self, run_id: RunId) {
        let mut registry = self.registry();
        for entry in registry.entries.values_mut() {
            if entry.run_id == run_id {
                entry.run_owned = false;
            }
        }
        registry
            .entries
            .retain(|_, entry| entry.run_owned || entry.result_source_holds > 0);
    }

    fn registry(&self) -> std::sync::MutexGuard<'_, ArtifactRegistry> {
        self.inner
            .registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::analysis::{CompileId, ProjectSessionId};
    use crate::node_system::document::GraphResourcePath;
    use crate::node_system::registry::RegistryFingerprint;

    fn context(run_id: RunId) -> (CorrelationContext, CompilationBasis<GraphRevision>) {
        let basis = CompilationBasis {
            graph_revision: GraphRevision::new(3),
            registry_fingerprint: RegistryFingerprint::from_bytes([7; 32]),
            resource_versions: BTreeMap::new(),
        };
        let correlation = CorrelationContext {
            project_session_id: ProjectSessionId::new("session"),
            graph_path: GraphResourcePath("events/main".into()),
            graph_revision: basis.graph_revision,
            registry_fingerprint: basis.registry_fingerprint.clone(),
            resource_versions: basis.resource_versions.clone(),
            compile_id: CompileId::new(5),
            selection_digest: None,
            run_id: Some(run_id),
            node_id: None,
            node_type_id: None,
            parent_call: None,
        };
        (correlation, basis)
    }

    #[test]
    fn pages_are_read_from_an_immutable_snapshot() {
        let store = ArtifactStore::new();
        let run_id = RunId::new(1);
        let (correlation, basis) = context(run_id);
        let mut values = vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)];
        let descriptor = store.insert(
            run_id,
            correlation,
            basis,
            ArtifactSnapshot::Sequence(values.clone().into_boxed_slice()),
        );
        values[1] = Value::Integer(99);

        let page = store.page(descriptor.artifact_id, 1, 2).unwrap();
        assert_eq!(
            page.values.as_ref(),
            &[Value::Integer(2), Value::Integer(3)]
        );
        assert_eq!(page.total_count, 3);
    }

    #[test]
    fn retained_insert_survives_immediate_run_cleanup() {
        let store = ArtifactStore::new();
        let run_id = RunId::new(2);
        let (correlation, basis) = context(run_id);
        let descriptor = store.insert_retained_result_source(
            run_id,
            correlation,
            basis,
            ArtifactSnapshot::Value(Value::Integer(41)),
        );

        store.cleanup_run(run_id);

        assert_eq!(
            store.descriptor(descriptor.artifact_id),
            Some(descriptor.clone())
        );
        assert_eq!(
            store.snapshot(descriptor.artifact_id).as_deref(),
            Some(&ArtifactSnapshot::Value(Value::Integer(41)))
        );
        assert!(store.release(descriptor.artifact_id));
        assert!(store.descriptor(descriptor.artifact_id).is_none());
    }

    #[test]
    fn run_cleanup_preserves_only_result_source_held_snapshots() {
        let store = ArtifactStore::new();
        let run_id = RunId::new(2);
        let (correlation, basis) = context(run_id);
        let retained = store.insert(
            run_id,
            correlation.clone(),
            basis.clone(),
            ArtifactSnapshot::Value(Value::Integer(1)),
        );
        let transient = store.insert(
            run_id,
            correlation,
            basis,
            ArtifactSnapshot::Value(Value::Integer(2)),
        );
        assert!(store.retain_result_source(retained.artifact_id));

        store.cleanup_run(run_id);

        assert!(store.descriptor(retained.artifact_id).is_some());
        assert!(store.descriptor(transient.artifact_id).is_none());
        assert!(store.release(retained.artifact_id));
        assert!(store.descriptor(retained.artifact_id).is_none());
    }
}
