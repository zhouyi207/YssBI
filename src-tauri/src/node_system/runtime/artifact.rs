use super::{Artifact, ArtifactValueKind, DataSeriesMetadata, SpillArtifact};
use crate::node_system::analysis::{CompilationBasis, CorrelationContext, RunId};
use crate::node_system::document::GraphRevision;
use crate::node_system::protocol::Value;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

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
    Spilled(SpillArtifact),
    RuntimeArtifact(Artifact),
}

impl ArtifactSnapshot {
    pub fn len(&self) -> usize {
        match self {
            Self::Value(_) => 1,
            Self::Sequence(values) => values.len(),
            Self::Spilled(spill) => spill.len(),
            Self::RuntimeArtifact(artifact) => artifact.materialized().len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn value_kind(&self) -> ArtifactValueKind {
        match self {
            Self::RuntimeArtifact(artifact) => artifact.value_kind(),
            Self::Value(_) | Self::Sequence(_) | Self::Spilled(_) => ArtifactValueKind::Sequence,
        }
    }

    fn kind(&self) -> ArtifactSnapshotKind {
        match self {
            Self::Value(_) => ArtifactSnapshotKind::Value,
            Self::RuntimeArtifact(artifact)
                if artifact.value_kind() == ArtifactValueKind::DataSeries =>
            {
                ArtifactSnapshotKind::DataSeries
            }
            Self::Sequence(_) | Self::Spilled(_) | Self::RuntimeArtifact(_) => {
                ArtifactSnapshotKind::Sequence
            }
        }
    }

    fn data_series_metadata(&self) -> Option<&DataSeriesMetadata> {
        match self {
            Self::RuntimeArtifact(artifact) => artifact.data_series_metadata(),
            Self::Value(_) | Self::Sequence(_) | Self::Spilled(_) => None,
        }
    }

    fn page(&self, offset: usize, limit: usize) -> Result<Box<[Value]>, super::RunError> {
        let start = offset.min(self.len());
        let end = start.saturating_add(limit).min(self.len());
        match self {
            Self::Value(value) if start == 0 && end == 1 => {
                Ok(vec![value.clone()].into_boxed_slice())
            }
            Self::Value(_) => Ok(Box::default()),
            Self::Sequence(values) => Ok(values[start..end].to_vec().into_boxed_slice()),
            Self::Spilled(spill) => spill
                .cursor()?
                .skip(start)
                .take(end.saturating_sub(start))
                .collect::<Result<Vec<_>, _>>()
                .map(Vec::into_boxed_slice),
            Self::RuntimeArtifact(artifact) => artifact
                .cursor()?
                .skip(start)
                .take(end.saturating_sub(start))
                .collect::<Result<Vec<_>, _>>()
                .map(Vec::into_boxed_slice),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactSnapshotKind {
    Value,
    Sequence,
    DataSeries,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDescriptor {
    pub artifact_id: ArtifactId,
    pub kind: ArtifactSnapshotKind,
    pub value_kind: ArtifactValueKind,
    pub data_series_metadata: Option<DataSeriesMetadata>,
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
    pub value_kind: ArtifactValueKind,
    pub data_series_metadata: Option<DataSeriesMetadata>,
    pub values: Box<[Value]>,
}

struct ArtifactEntry {
    run_id: RunId,
    run_owned: bool,
    result_source_holds: usize,
    descriptor: ArtifactDescriptor,
    snapshot: Arc<ArtifactSnapshot>,
}

pub(crate) struct PreparedArtifactEntry {
    artifact_id: ArtifactId,
    entry: ArtifactEntry,
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

pub(crate) struct ArtifactPublicationGuard<'a> {
    registry: MutexGuard<'a, ArtifactRegistry>,
}

impl ArtifactPublicationGuard<'_> {
    pub(crate) fn insert(&mut self, prepared: PreparedArtifactEntry) {
        self.registry
            .entries
            .insert(prepared.artifact_id, prepared.entry);
    }

    pub(crate) fn remove(&mut self, artifact_id: ArtifactId) {
        self.registry.entries.remove(&artifact_id);
    }

    pub(crate) fn release(&mut self, artifact_id: ArtifactId) -> bool {
        let remove = {
            let Some(entry) = self.registry.entries.get_mut(&artifact_id) else {
                return false;
            };
            if entry.result_source_holds == 0 {
                return false;
            }
            entry.result_source_holds -= 1;
            !entry.run_owned && entry.result_source_holds == 0
        };
        if remove {
            self.registry.entries.remove(&artifact_id);
        }
        true
    }
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

    #[cfg(test)]
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
        let (descriptor, prepared) =
            self.prepare_entry(run_id, correlation, basis, snapshot, result_source_holds);
        self.publication_registry().insert(prepared);
        descriptor
    }

    pub(crate) fn prepare_retained_result_source(
        &self,
        run_id: RunId,
        correlation: CorrelationContext,
        basis: CompilationBasis<GraphRevision>,
        snapshot: ArtifactSnapshot,
    ) -> (ArtifactDescriptor, PreparedArtifactEntry) {
        self.prepare_entry(run_id, correlation, basis, snapshot, 1)
    }

    fn prepare_entry(
        &self,
        run_id: RunId,
        correlation: CorrelationContext,
        basis: CompilationBasis<GraphRevision>,
        snapshot: ArtifactSnapshot,
        result_source_holds: usize,
    ) -> (ArtifactDescriptor, PreparedArtifactEntry) {
        let artifact_id = ArtifactId::new(self.inner.next_id.fetch_add(1, Ordering::Relaxed) + 1);
        let descriptor = ArtifactDescriptor {
            artifact_id,
            kind: snapshot.kind(),
            value_kind: snapshot.value_kind(),
            data_series_metadata: snapshot.data_series_metadata().cloned(),
            total_count: snapshot.len(),
            correlation,
            basis,
        };
        let prepared = PreparedArtifactEntry {
            artifact_id,
            entry: ArtifactEntry {
                run_id,
                run_owned: true,
                result_source_holds,
                descriptor: descriptor.clone(),
                snapshot: Arc::new(snapshot),
            },
        };
        (descriptor, prepared)
    }

    pub(crate) fn publication_registry(&self) -> ArtifactPublicationGuard<'_> {
        ArtifactPublicationGuard {
            registry: self.registry(),
        }
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
    ) -> Result<Option<ArtifactPage>, super::RunError> {
        let limit = limit.max(1);
        let snapshot = {
            let registry = self.registry();
            let Some(entry) = registry.entries.get(&artifact_id) else {
                return Ok(None);
            };
            Arc::clone(&entry.snapshot)
        };
        let start = offset.min(snapshot.len());
        Ok(Some(ArtifactPage {
            artifact_id,
            offset: start,
            limit,
            total_count: snapshot.len(),
            value_kind: snapshot.value_kind(),
            data_series_metadata: snapshot.data_series_metadata().cloned(),
            values: snapshot.page(start, limit)?,
        }))
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

    #[cfg(test)]
    pub(crate) fn entry_count(&self) -> usize {
        self.registry().entries.len()
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
            resource_observations: BTreeMap::new(),
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
            trace_parent_span_id: None,
        };
        (correlation, basis)
    }

    #[test]
    fn runtime_data_series_descriptor_and_page_publish_metadata() {
        let store = ArtifactStore::new();
        let run_id = RunId::new(8);
        let (correlation, basis) = context(run_id);
        let metadata = crate::node_system::runtime::DataSeriesMetadata {
            element_type: crate::node_system::runtime::DataSeriesElementType::Int64,
            length: 2,
            null_count: 1,
            name: Some("published".into()),
            format: None,
        };
        let artifact = Artifact::new_data_series(
            crate::node_system::runtime::ArtifactKind::Collected,
            metadata.clone(),
            [Value::Integer(4), Value::Null],
        )
        .unwrap();

        let descriptor = store.insert(
            run_id,
            correlation,
            basis,
            ArtifactSnapshot::RuntimeArtifact(artifact),
        );
        let page = store.page(descriptor.artifact_id, 0, 10).unwrap().unwrap();

        assert_eq!(
            descriptor.value_kind,
            crate::node_system::runtime::ArtifactValueKind::DataSeries
        );
        assert_eq!(descriptor.data_series_metadata, Some(metadata.clone()));
        assert_eq!(
            page.value_kind,
            crate::node_system::runtime::ArtifactValueKind::DataSeries
        );
        assert_eq!(page.data_series_metadata, Some(metadata));
        assert_eq!(page.values.as_ref(), &[Value::Integer(4), Value::Null]);
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

        let page = store.page(descriptor.artifact_id, 1, 2).unwrap().unwrap();
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
