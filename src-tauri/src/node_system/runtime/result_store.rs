use super::{CancellationToken, RunDeadline, RunError, RunPhase, RuntimeValue, check_terminal};
use crate::node_system::analysis::{CompilationBasis, CorrelationContext, RunId};
use crate::node_system::document::GraphRevision;
use crate::node_system::protocol::Value;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::artifact::{
    ArtifactId, ArtifactPage, ArtifactPublicationGuard, ArtifactSnapshot, ArtifactSnapshotKind,
    ArtifactStore,
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

static NEXT_RESULT_SOURCE_ID: AtomicU64 = AtomicU64::new(1);

fn allocate_result_source_range(count: usize) -> u64 {
    let count = u64::try_from(count).expect("result source batch length fits u64");
    NEXT_RESULT_SOURCE_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            next.checked_add(count)
        })
        .expect("result source id space exhausted")
}

struct ResultStoreInner {
    max_sources: usize,
    registry: Mutex<ResultSourceRegistry>,
    #[cfg(test)]
    commit_checkpoint: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    #[cfg(test)]
    publication_checkpoint: Mutex<Option<Arc<dyn Fn(ResultPublicationCheckpoint) + Send + Sync>>>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResultPublicationCheckpoint {
    AfterSourceIdAllocation,
    AfterArtifactInsert,
    AfterSourceInsert,
}

/// User-visible handles over immutable artifacts. A source owns one artifact hold;
/// run cleanup never invalidates it, and `release` ends that ownership explicitly.
#[derive(Clone)]
pub struct ResultStore {
    artifacts: ArtifactStore,
    inner: Arc<ResultStoreInner>,
}

pub(crate) struct ResultPublicationTransaction<'a> {
    store: &'a ResultStore,
    run_id: RunId,
    pending: Option<Vec<PendingResultSource>>,
    prepared: Option<Vec<PendingResultSource>>,
}

struct ResultAuthorityRollback<'a, 'b> {
    registry: &'a mut ResultSourceRegistry,
    artifacts: &'a mut ArtifactPublicationGuard<'b>,
    inserted_source_ids: Vec<ResultSourceId>,
    inserted_artifact_ids: Vec<ArtifactId>,
    evicted_prior: Vec<ResultSourceEntry>,
    committed: bool,
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
                max_sources: max_sources.max(1),
                registry: Mutex::new(ResultSourceRegistry::default()),
                #[cfg(test)]
                commit_checkpoint: Mutex::new(None),
                #[cfg(test)]
                publication_checkpoint: Mutex::new(None),
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
            RuntimeValue::Artifact(artifact) => ArtifactSnapshot::RuntimeArtifact(artifact.clone()),
            RuntimeValue::Stream(_) => return None,
        };
        Some(self.prepare_snapshot(correlation, basis, name, snapshot))
    }

    pub(crate) fn begin_publication(
        &self,
        run_id: RunId,
        pending: Vec<PendingResultSource>,
    ) -> ResultPublicationTransaction<'_> {
        ResultPublicationTransaction {
            store: self,
            run_id,
            pending: Some(pending),
            prepared: None,
        }
    }

    pub(crate) fn commit_batch(
        &self,
        run_id: RunId,
        pending: Vec<PendingResultSource>,
    ) -> Vec<Option<ResultSourceDescriptor>> {
        self.commit_batch_with_deadline(run_id, pending, &CancellationToken::new(), None)
            .expect("commit without a deadline cannot time out")
    }

    pub(crate) fn commit_batch_with_deadline(
        &self,
        run_id: RunId,
        pending: Vec<PendingResultSource>,
        cancellation: &CancellationToken,
        deadline: Option<RunDeadline>,
    ) -> Result<Vec<Option<ResultSourceDescriptor>>, RunError> {
        self.commit_batch_with_deadline_and_publish(run_id, pending, cancellation, deadline, |_| {})
    }

    pub(crate) fn commit_batch_with_deadline_and_publish(
        &self,
        run_id: RunId,
        pending: Vec<PendingResultSource>,
        cancellation: &CancellationToken,
        deadline: Option<RunDeadline>,
        publish: impl FnOnce(&[Option<ResultSourceDescriptor>]),
    ) -> Result<Vec<Option<ResultSourceDescriptor>>, RunError> {
        let mut transaction = self.begin_publication(run_id, pending);
        transaction.prepare(cancellation, deadline)?;
        transaction.publish_with_authority(cancellation, deadline, |descriptors| {
            publish(descriptors);
            Ok(())
        })
    }

    #[cfg(test)]
    pub(crate) fn set_commit_checkpoint_for_test(&self, checkpoint: Arc<dyn Fn() + Send + Sync>) {
        *self
            .inner
            .commit_checkpoint
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(checkpoint);
    }

    #[cfg(test)]
    pub(crate) fn set_publication_checkpoint_for_test(
        &self,
        checkpoint: Arc<dyn Fn(ResultPublicationCheckpoint) + Send + Sync>,
    ) {
        *self
            .inner
            .publication_checkpoint
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(checkpoint);
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
    ) -> Result<Option<ResultSourcePage>, super::RunError> {
        let Some(descriptor) = self.descriptor(source_id) else {
            return Ok(None);
        };
        let Some(ArtifactPage {
            offset,
            limit,
            total_count,
            values,
            ..
        }) = self.artifacts.page(descriptor.artifact_id, offset, limit)?
        else {
            return Ok(None);
        };
        Ok(Some(ResultSourcePage {
            source_id,
            offset,
            limit,
            total_count,
            values,
        }))
    }

    pub fn source_count(&self) -> usize {
        self.registry().sources.len()
    }

    #[cfg(test)]
    pub(crate) fn artifact_count_for_test(&self) -> usize {
        self.artifacts.entry_count()
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

impl ResultPublicationTransaction<'_> {
    pub(crate) fn prepare(
        &mut self,
        cancellation: &CancellationToken,
        deadline: Option<RunDeadline>,
    ) -> Result<(), RunError> {
        if self.prepared.is_some() {
            return Ok(());
        }
        let pending = self.pending.take().unwrap_or_default();
        if !pending.is_empty() {
            #[cfg(test)]
            if let Some(checkpoint) = self
                .store
                .inner
                .commit_checkpoint
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone()
            {
                checkpoint();
            }
            check_terminal(cancellation, deadline, RunPhase::ResultPublication)?;
        }
        self.prepared = Some(pending);
        Ok(())
    }

    pub(crate) fn publish_with_authority(
        &mut self,
        cancellation: &CancellationToken,
        deadline: Option<RunDeadline>,
        authority: impl FnOnce(&[Option<ResultSourceDescriptor>]) -> Result<(), RunError>,
    ) -> Result<Vec<Option<ResultSourceDescriptor>>, RunError> {
        self.prepare(cancellation, deadline)?;
        let pending = self
            .prepared
            .take()
            .expect("result snapshots must be prepared before publication");
        #[cfg(test)]
        let checkpoint = self
            .store
            .inner
            .publication_checkpoint
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();

        // Global authority lock order is result registry -> artifact registry -> project
        // authority. Result readers clone their ResultStore before taking either result
        // lock, so no project guard is retained while waiting here.
        let mut registry = self.store.registry();
        let mut artifacts = self.store.artifacts.publication_registry();
        check_terminal(cancellation, deadline, RunPhase::ResultPublication)?;
        let capacity = registry.sources.len().saturating_add(pending.len());
        let mut rollback = ResultAuthorityRollback {
            registry: &mut registry,
            artifacts: &mut artifacts,
            inserted_source_ids: Vec::with_capacity(pending.len()),
            inserted_artifact_ids: Vec::with_capacity(pending.len()),
            evicted_prior: Vec::with_capacity(capacity),
            committed: false,
        };

        let first_id = allocate_result_source_range(pending.len());
        #[cfg(test)]
        if let Some(checkpoint) = &checkpoint {
            checkpoint(ResultPublicationCheckpoint::AfterSourceIdAllocation);
        }
        let mut descriptors = Vec::with_capacity(pending.len());
        for (offset, pending) in pending.into_iter().enumerate() {
            let PendingResultSource {
                correlation,
                basis,
                name,
                snapshot,
            } = pending;
            let (artifact, prepared) = self.store.artifacts.prepare_retained_result_source(
                self.run_id,
                correlation.clone(),
                basis.clone(),
                snapshot,
            );
            rollback.inserted_artifact_ids.push(artifact.artifact_id);
            rollback.artifacts.insert(prepared);
            #[cfg(test)]
            if let Some(checkpoint) = &checkpoint {
                checkpoint(ResultPublicationCheckpoint::AfterArtifactInsert);
            }
            descriptors.push(ResultSourceDescriptor {
                source_id: ResultSourceId::new(first_id + offset as u64),
                artifact_id: artifact.artifact_id,
                name,
                kind: artifact.kind,
                total_count: artifact.total_count,
                correlation,
                basis,
            });
        }
        for descriptor in &descriptors {
            rollback.inserted_source_ids.push(descriptor.source_id);
            rollback.registry.sources.insert(
                descriptor.source_id,
                ResultSourceEntry {
                    run_id: self.run_id,
                    descriptor: descriptor.clone(),
                },
            );
            #[cfg(test)]
            if let Some(checkpoint) = &checkpoint {
                checkpoint(ResultPublicationCheckpoint::AfterSourceInsert);
            }
        }
        while rollback.registry.sources.len() > self.store.inner.max_sources {
            let source_id = *rollback
                .registry
                .sources
                .keys()
                .next()
                .expect("over-capacity registry is not empty");
            let entry = rollback
                .registry
                .sources
                .remove(&source_id)
                .expect("oldest source must exist");
            if !rollback.inserted_source_ids.contains(&source_id) {
                rollback.evicted_prior.push(entry);
            }
        }
        let committed = descriptors
            .into_iter()
            .map(|descriptor| {
                rollback
                    .registry
                    .sources
                    .contains_key(&descriptor.source_id)
                    .then_some(descriptor)
            })
            .collect::<Vec<_>>();

        authority(&committed)?;
        rollback.commit(&committed);
        Ok(committed)
    }
}

impl ResultAuthorityRollback<'_, '_> {
    fn commit(&mut self, committed: &[Option<ResultSourceDescriptor>]) {
        for entry in self.evicted_prior.drain(..) {
            self.artifacts.release(entry.descriptor.artifact_id);
        }
        for artifact_id in self.inserted_artifact_ids.iter().copied() {
            if !committed
                .iter()
                .flatten()
                .any(|descriptor| descriptor.artifact_id == artifact_id)
            {
                self.artifacts.remove(artifact_id);
            }
        }
        self.committed = true;
    }
}

impl Drop for ResultAuthorityRollback<'_, '_> {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for source_id in &self.inserted_source_ids {
            self.registry.sources.remove(source_id);
        }
        for entry in self.evicted_prior.drain(..) {
            self.registry
                .sources
                .insert(entry.descriptor.source_id, entry);
        }
        for artifact_id in &self.inserted_artifact_ids {
            self.artifacts.remove(*artifact_id);
        }
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
            resource_observations: BTreeMap::new(),
        };
        let correlation = CorrelationContext {
            project_session_id: ProjectSessionId::new("session"),
            graph_path: GraphResourcePath("functions/example".into()),
            graph_revision: basis.graph_revision,
            registry_fingerprint: basis.registry_fingerprint.clone(),
            resource_versions: basis.resource_versions.clone(),
            compile_id: CompileId::new(9),
            selection_digest: None,
            run_id: Some(run_id),
            node_id: None,
            node_type_id: None,
            parent_call: None,
            trace_parent_span_id: None,
        };
        (correlation, basis)
    }

    fn pending_test_snapshot(store: &ResultStore, run_id: RunId) -> PendingResultSource {
        let (correlation, basis) = context(run_id);
        store.prepare_snapshot(
            correlation,
            basis,
            "result",
            ArtifactSnapshot::Value(Value::Integer(run_id.get() as i64)),
        )
    }

    fn publish_test_snapshot(store: &ResultStore, run_id: RunId) -> ResultSourceDescriptor {
        let (correlation, basis) = context(run_id);
        store.publish_snapshot(
            run_id,
            correlation,
            basis,
            "result",
            ArtifactSnapshot::Value(Value::Integer(run_id.get() as i64)),
        )
    }

    #[test]
    fn result_source_ids_are_process_global_across_stores() {
        let first_store = ResultStore::new();
        let replacement_store = ResultStore::new();
        let first = publish_test_snapshot(&first_store, RunId::new(1));
        let replacement = publish_test_snapshot(&replacement_store, RunId::new(2));

        assert_ne!(first.source_id, replacement.source_id);
        assert!(replacement.source_id.get() > first.source_id.get());
    }

    #[test]
    fn result_source_ids_are_process_global_when_allocated_concurrently() {
        const PUBLICATION_COUNT: usize = 16;

        let rendezvous = Arc::new(std::sync::Barrier::new(PUBLICATION_COUNT + 1));
        let publications = (0..PUBLICATION_COUNT)
            .map(|index| {
                let rendezvous = Arc::clone(&rendezvous);
                std::thread::spawn(move || {
                    let store = ResultStore::new();
                    rendezvous.wait();
                    publish_test_snapshot(&store, RunId::new(index as u64 + 1))
                        .source_id
                        .get()
                })
            })
            .collect::<Vec<_>>();

        rendezvous.wait();
        let source_ids = publications
            .into_iter()
            .map(|publication| publication.join().unwrap())
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(source_ids.len(), PUBLICATION_COUNT);
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
                .map(|descriptor| descriptor.name.as_ref())
                .collect::<Vec<_>>(),
            vec!["first", "second"]
        );
        assert_eq!(
            committed[1].source_id.get(),
            committed[0].source_id.get() + 1
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
    fn scoped_authority_serializes_capacity_one_rollback_before_next_publication() {
        let store = Arc::new(ResultStore::with_capacity(1));
        let prior = publish_test_snapshot(&store, RunId::new(20));
        store.cleanup_run(RunId::new(20));
        let (a_entered_tx, a_entered_rx) = std::sync::mpsc::sync_channel(1);
        let (release_a_tx, release_a_rx) = std::sync::mpsc::sync_channel(1);
        let (b_done_tx, b_done_rx) = std::sync::mpsc::sync_channel(1);

        let a_store = Arc::clone(&store);
        let a = std::thread::spawn(move || {
            let cancellation = CancellationToken::new();
            let mut transaction = a_store.begin_publication(
                RunId::new(21),
                vec![pending_test_snapshot(&a_store, RunId::new(21))],
            );
            transaction.prepare(&cancellation, None).unwrap();
            transaction.publish_with_authority(&cancellation, None, |_| {
                a_entered_tx.send(()).unwrap();
                release_a_rx.recv().unwrap();
                Err(RunError::ResourceSnapshotMismatch("A rolled back".into()))
            })
        });
        a_entered_rx.recv().unwrap();

        let b_store = Arc::clone(&store);
        let b = std::thread::spawn(move || {
            let descriptor = publish_test_snapshot(&b_store, RunId::new(22));
            b_done_tx.send(descriptor.clone()).unwrap();
            descriptor
        });
        assert!(
            b_done_rx
                .recv_timeout(std::time::Duration::from_millis(30))
                .is_err()
        );
        release_a_tx.send(()).unwrap();

        assert!(matches!(
            a.join().unwrap(),
            Err(RunError::ResourceSnapshotMismatch(_))
        ));
        let b_descriptor = b.join().unwrap();
        assert_eq!(store.source_count(), 1);
        assert_eq!(store.artifact_count_for_test(), 1);
        assert!(store.descriptor(prior.source_id).is_none());
        assert_eq!(store.descriptor(b_descriptor.source_id), Some(b_descriptor));
    }

    #[test]
    fn scoped_authority_keeps_provisional_source_unreadable_until_success() {
        let store = Arc::new(ResultStore::with_capacity(1));
        let (source_tx, source_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let (reader_done_tx, reader_done_rx) = std::sync::mpsc::sync_channel(1);
        let publisher_store = Arc::clone(&store);
        let publisher = std::thread::spawn(move || {
            let cancellation = CancellationToken::new();
            let mut transaction = publisher_store.begin_publication(
                RunId::new(23),
                vec![pending_test_snapshot(&publisher_store, RunId::new(23))],
            );
            transaction.prepare(&cancellation, None).unwrap();
            transaction
                .publish_with_authority(&cancellation, None, |descriptors| {
                    source_tx
                        .send(descriptors[0].as_ref().unwrap().source_id)
                        .unwrap();
                    release_rx.recv().unwrap();
                    Ok(())
                })
                .unwrap()
        });
        let source_id = source_rx.recv().unwrap();
        let reader_store = Arc::clone(&store);
        let reader = std::thread::spawn(move || {
            let descriptor = reader_store.descriptor(source_id);
            reader_done_tx.send(descriptor.clone()).unwrap();
            descriptor
        });
        assert!(
            reader_done_rx
                .recv_timeout(std::time::Duration::from_millis(30))
                .is_err()
        );
        release_tx.send(()).unwrap();

        let published = publisher.join().unwrap();
        assert_eq!(published[0].as_ref().unwrap().source_id, source_id);
        assert_eq!(reader.join().unwrap(), published[0]);
    }

    #[test]
    fn scoped_authority_serializes_empty_publication_through_final_authority() {
        let store = Arc::new(ResultStore::with_capacity(1));
        let (empty_entered_tx, empty_entered_rx) = std::sync::mpsc::sync_channel(1);
        let (release_empty_tx, release_empty_rx) = std::sync::mpsc::sync_channel(1);
        let (publisher_done_tx, publisher_done_rx) = std::sync::mpsc::sync_channel(1);

        let empty_store = Arc::clone(&store);
        let empty = std::thread::spawn(move || {
            let cancellation = CancellationToken::new();
            let mut transaction = empty_store.begin_publication(RunId::new(24), Vec::new());
            transaction.prepare(&cancellation, None).unwrap();
            transaction
                .publish_with_authority(&cancellation, None, |descriptors| {
                    assert!(descriptors.is_empty());
                    empty_entered_tx.send(()).unwrap();
                    release_empty_rx.recv().unwrap();
                    Ok(())
                })
                .unwrap()
        });
        empty_entered_rx.recv().unwrap();

        let publisher_store = Arc::clone(&store);
        let publisher = std::thread::spawn(move || {
            let descriptor = publish_test_snapshot(&publisher_store, RunId::new(25));
            publisher_done_tx.send(descriptor.clone()).unwrap();
            descriptor
        });
        assert!(
            publisher_done_rx
                .recv_timeout(std::time::Duration::from_millis(30))
                .is_err()
        );
        release_empty_tx.send(()).unwrap();

        assert!(empty.join().unwrap().is_empty());
        let descriptor = publisher.join().unwrap();
        assert_eq!(store.source_count(), 1);
        assert_eq!(store.descriptor(descriptor.source_id), Some(descriptor));
    }

    #[test]
    fn scoped_authority_rolls_back_panics_during_artifact_and_source_construction() {
        for checkpoint in [
            ResultPublicationCheckpoint::AfterArtifactInsert,
            ResultPublicationCheckpoint::AfterSourceIdAllocation,
            ResultPublicationCheckpoint::AfterSourceInsert,
        ] {
            let store = ResultStore::with_capacity(1);
            let prior = publish_test_snapshot(&store, RunId::new(24));
            store.set_publication_checkpoint_for_test(Arc::new(move |observed| {
                if observed == checkpoint {
                    panic!("injected result publication construction panic")
                }
            }));
            let cancellation = CancellationToken::new();
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut transaction = store.begin_publication(
                    RunId::new(25),
                    vec![pending_test_snapshot(&store, RunId::new(25))],
                );
                transaction.prepare(&cancellation, None).unwrap();
                let _ = transaction.publish_with_authority(&cancellation, None, |_| Ok(()));
            }));

            assert!(panic.is_err());
            assert_eq!(store.source_count(), 1);
            assert_eq!(store.artifact_count_for_test(), 1);
            assert_eq!(store.descriptor(prior.source_id), Some(prior));
        }
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
