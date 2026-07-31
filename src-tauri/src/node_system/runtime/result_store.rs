use super::RuntimeValue;
use crate::node_system::analysis::{CompilationBasis, CorrelationContext, RunId};
use crate::node_system::document::GraphRevision;
use crate::node_system::protocol::Value;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::artifact::{
    ArtifactId, ArtifactPage, ArtifactSnapshot, ArtifactSnapshotKind, ArtifactStore,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResultSourceId(u64);

impl ResultSourceId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultSourceDescriptor {
    pub source_id: ResultSourceId,
    pub artifact_id: ArtifactId,
    pub name: Box<str>,
    pub kind: ArtifactSnapshotKind,
    pub total_count: usize,
    pub correlation: CorrelationContext,
    pub basis: CompilationBasis<GraphRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultSourcePage {
    pub source_id: ResultSourceId,
    pub offset: usize,
    pub limit: usize,
    pub total_count: usize,
    pub values: Box<[Value]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingResultSource {
    correlation: CorrelationContext,
    basis: CompilationBasis<GraphRevision>,
    name: Box<str>,
    snapshot: ArtifactSnapshot,
}

#[derive(Clone)]
struct ResultSourceEntry {
    run_id: RunId,
    descriptor: ResultSourceDescriptor,
}

#[derive(Default)]
struct ResultSourceRegistry {
    sources: BTreeMap<ResultSourceId, ResultSourceEntry>,
}

struct ResultStoreInner {
    next_id: AtomicU64,
    max_sources: usize,
    registry: Mutex<ResultSourceRegistry>,
}

/// User-visible handles over immutable artifacts. A source owns one artifact hold;
/// run cleanup never invalidates it, and `release` ends that ownership explicitly.
#[derive(Clone)]
pub struct ResultStore {
    artifacts: ArtifactStore,
    inner: Arc<ResultStoreInner>,
}

impl Default for ResultStore {
    fn default() -> Self {
        Self::with_capacity(4096)
    }
}

impl ResultStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(max_sources: usize) -> Self {
        Self {
            artifacts: ArtifactStore::default(),
            inner: Arc::new(ResultStoreInner {
                next_id: AtomicU64::new(0),
                max_sources: max_sources.max(1),
                registry: Mutex::new(ResultSourceRegistry::default()),
            }),
        }
    }

    pub fn artifacts(&self) -> &ArtifactStore {
        &self.artifacts
    }

    fn prepare_snapshot(
        &self,
        correlation: CorrelationContext,
        basis: CompilationBasis<GraphRevision>,
        name: impl Into<Box<str>>,
        snapshot: ArtifactSnapshot,
    ) -> PendingResultSource {
        PendingResultSource {
            correlation,
            basis,
            name: name.into(),
            snapshot,
        }
    }

    pub(crate) fn prepare_runtime_value(
        &self,
        correlation: CorrelationContext,
        basis: CompilationBasis<GraphRevision>,
        name: impl Into<Box<str>>,
        value: &RuntimeValue,
    ) -> Option<PendingResultSource> {
        let snapshot = match value {
            RuntimeValue::Scalar(value) => ArtifactSnapshot::Value(value.clone()),
            RuntimeValue::Artifact(artifact) => {
                ArtifactSnapshot::Sequence(artifact.values().to_vec().into_boxed_slice())
            }
            RuntimeValue::Stream(_) => return None,
        };
        Some(self.prepare_snapshot(correlation, basis, name, snapshot))
    }

    pub(crate) fn commit_batch(
        &self,
        run_id: RunId,
        pending: Vec<PendingResultSource>,
    ) -> Vec<Option<ResultSourceDescriptor>> {
        let prepared = pending
            .into_iter()
            .map(|pending| {
                let PendingResultSource {
                    correlation,
                    basis,
                    name,
                    snapshot,
                } = pending;
                let artifact = self.artifacts.insert_retained_result_source(
                    run_id,
                    correlation.clone(),
                    basis.clone(),
                    snapshot,
                );
                (correlation, basis, name, artifact)
            })
            .collect::<Vec<_>>();
        if prepared.is_empty() {
            return Vec::new();
        }

        let (committed, evicted) = {
            let mut registry = self.registry();
            let first_id = self
                .inner
                .next_id
                .fetch_add(prepared.len() as u64, Ordering::Relaxed)
                + 1;
            let descriptors = prepared
                .into_iter()
                .enumerate()
                .map(
                    |(offset, (correlation, basis, name, artifact))| ResultSourceDescriptor {
                        source_id: ResultSourceId::new(first_id + offset as u64),
                        artifact_id: artifact.artifact_id,
                        name,
                        kind: artifact.kind,
                        total_count: artifact.total_count,
                        correlation,
                        basis,
                    },
                )
                .collect::<Vec<_>>();
            for descriptor in &descriptors {
                registry.sources.insert(
                    descriptor.source_id,
                    ResultSourceEntry {
                        run_id,
                        descriptor: descriptor.clone(),
                    },
                );
            }
            let mut evicted = Vec::new();
            while registry.sources.len() > self.inner.max_sources {
                let source_id = *registry
                    .sources
                    .keys()
                    .next()
                    .expect("over-capacity registry is not empty");
                evicted.push(
                    registry
                        .sources
                        .remove(&source_id)
                        .expect("oldest source must exist"),
                );
            }
            let committed = descriptors
                .into_iter()
                .map(|descriptor| {
                    registry
                        .sources
                        .contains_key(&descriptor.source_id)
                        .then_some(descriptor)
                })
                .collect();
            (committed, evicted)
        };
        for entry in evicted {
            self.artifacts.release(entry.descriptor.artifact_id);
        }
        committed
    }

    pub fn publish_snapshot(
        &self,
        run_id: RunId,
        correlation: CorrelationContext,
        basis: CompilationBasis<GraphRevision>,
        name: impl Into<Box<str>>,
        snapshot: ArtifactSnapshot,
    ) -> ResultSourceDescriptor {
        let pending = self.prepare_snapshot(correlation, basis, name, snapshot);
        self.commit_batch(run_id, vec![pending])
            .pop()
            .flatten()
            .expect("single result source commit must remain within capacity")
    }

    pub fn publish_runtime_value(
        &self,
        run_id: RunId,
        correlation: CorrelationContext,
        basis: CompilationBasis<GraphRevision>,
        name: impl Into<Box<str>>,
        value: &RuntimeValue,
    ) -> Option<ResultSourceDescriptor> {
        let pending = self.prepare_runtime_value(correlation, basis, name, value)?;
        self.commit_batch(run_id, vec![pending]).pop().flatten()
    }

    pub fn descriptor(&self, source_id: ResultSourceId) -> Option<ResultSourceDescriptor> {
        self.registry()
            .sources
            .get(&source_id)
            .map(|entry| entry.descriptor.clone())
    }

    pub fn value(&self, source_id: ResultSourceId) -> Option<Arc<ArtifactSnapshot>> {
        let artifact_id = self.descriptor(source_id)?.artifact_id;
        self.artifacts.snapshot(artifact_id)
    }

    pub fn page(
        &self,
        source_id: ResultSourceId,
        offset: usize,
        limit: usize,
    ) -> Option<ResultSourcePage> {
        let descriptor = self.descriptor(source_id)?;
        let ArtifactPage {
            offset,
            limit,
            total_count,
            values,
            ..
        } = self.artifacts.page(descriptor.artifact_id, offset, limit)?;
        Some(ResultSourcePage {
            source_id,
            offset,
            limit,
            total_count,
            values,
        })
    }

    pub fn source_count(&self) -> usize {
        self.registry().sources.len()
    }

    pub fn release(&self, source_id: ResultSourceId) -> bool {
        let entry = self.registry().sources.remove(&source_id);
        let Some(entry) = entry else {
            return false;
        };
        self.artifacts.release(entry.descriptor.artifact_id)
    }

    pub fn release_run_sources(&self, run_id: RunId) -> usize {
        let source_ids = self
            .registry()
            .sources
            .iter()
            .filter_map(|(source_id, entry)| (entry.run_id == run_id).then_some(*source_id))
            .collect::<Vec<_>>();
        let count = source_ids.len();
        for source_id in source_ids {
            self.release(source_id);
        }
        count
    }

    pub fn cleanup_run(&self, run_id: RunId) {
        self.artifacts.cleanup_run(run_id);
    }

    fn registry(&self) -> std::sync::MutexGuard<'_, ResultSourceRegistry> {
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
            graph_revision: GraphRevision::new(8),
            registry_fingerprint: RegistryFingerprint::from_bytes([4; 32]),
            resource_versions: BTreeMap::new(),
        };
        let correlation = CorrelationContext {
            project_session_id: ProjectSessionId::new("session"),
            graph_path: GraphResourcePath("functions/example".into()),
            graph_revision: basis.graph_revision,
            registry_fingerprint: basis.registry_fingerprint.clone(),
            resource_versions: basis.resource_versions.clone(),
            compile_id: CompileId::new(9),
            run_id: Some(run_id),
            node_id: None,
            node_type_id: None,
            parent_call: None,
        };
        (correlation, basis)
    }

    #[test]
    fn descriptor_page_and_release_replace_result_source_store_reads() {
        let store = ResultStore::new();
        let run_id = RunId::new(11);
        let (correlation, basis) = context(run_id);
        let descriptor = store.publish_snapshot(
            run_id,
            correlation,
            basis,
            "result",
            ArtifactSnapshot::Sequence(
                vec![Value::Integer(1), Value::Integer(2)].into_boxed_slice(),
            ),
        );

        assert_eq!(
            store.descriptor(descriptor.source_id),
            Some(descriptor.clone())
        );
        assert_eq!(
            store
                .page(descriptor.source_id, 1, 20)
                .unwrap()
                .values
                .as_ref(),
            &[Value::Integer(2)]
        );
        assert!(store.release(descriptor.source_id));
        assert!(store.descriptor(descriptor.source_id).is_none());
    }

    #[test]
    fn prepared_runtime_values_commit_as_an_ordered_atomic_batch() {
        let store = ResultStore::with_capacity(2);
        let old_run_id = RunId::new(13);
        let (old_correlation, old_basis) = context(old_run_id);
        let old = store.publish_snapshot(
            old_run_id,
            old_correlation,
            old_basis,
            "old",
            ArtifactSnapshot::Value(Value::Integer(0)),
        );
        store.cleanup_run(old_run_id);

        let run_id = RunId::new(14);
        let (correlation, basis) = context(run_id);
        let first = store
            .prepare_runtime_value(
                correlation.clone(),
                basis.clone(),
                "first",
                &RuntimeValue::from(Value::Integer(1)),
            )
            .unwrap();
        let second = store
            .prepare_runtime_value(
                correlation,
                basis,
                "second",
                &RuntimeValue::from(Value::Integer(2)),
            )
            .unwrap();

        assert_eq!(store.source_count(), 1);
        assert_eq!(store.descriptor(old.source_id), Some(old.clone()));
        let committed = store
            .commit_batch(run_id, vec![first, second])
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        assert_eq!(
            committed
                .iter()
                .map(|descriptor| (descriptor.source_id.get(), descriptor.name.as_ref()))
                .collect::<Vec<_>>(),
            vec![(2, "first"), (3, "second")]
        );
        assert!(store.descriptor(old.source_id).is_none());
        assert!(store.artifacts().descriptor(old.artifact_id).is_none());
        assert_eq!(store.source_count(), 2);
    }

    #[test]
    fn result_sources_evict_oldest_entry_at_capacity() {
        let store = ResultStore::with_capacity(2);
        let run_id = RunId::new(13);
        let (correlation, basis) = context(run_id);
        let first = store.publish_snapshot(
            run_id,
            correlation.clone(),
            basis.clone(),
            "first",
            ArtifactSnapshot::Value(Value::Integer(1)),
        );
        let second = store.publish_snapshot(
            run_id,
            correlation.clone(),
            basis.clone(),
            "second",
            ArtifactSnapshot::Value(Value::Integer(2)),
        );
        let third = store.publish_snapshot(
            run_id,
            correlation,
            basis,
            "third",
            ArtifactSnapshot::Value(Value::Integer(3)),
        );

        assert!(store.descriptor(first.source_id).is_none());
        assert!(store.descriptor(second.source_id).is_some());
        assert!(store.descriptor(third.source_id).is_some());
        assert_eq!(store.source_count(), 2);
    }

    #[test]
    fn run_cleanup_and_user_release_have_separate_lifetimes() {
        let store = ResultStore::new();
        let run_id = RunId::new(12);
        let (correlation, basis) = context(run_id);
        let descriptor = store.publish_snapshot(
            run_id,
            correlation,
            basis,
            "result",
            ArtifactSnapshot::Value(Value::String("private".into())),
        );

        store.cleanup_run(run_id);
        assert!(store.value(descriptor.source_id).is_some());
        assert_eq!(store.release_run_sources(run_id), 1);
        assert!(store.value(descriptor.source_id).is_none());
    }
}
