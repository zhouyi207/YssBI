use super::{
    Artifact, ArtifactKind, ArtifactValueKind, DataSeriesMetadata, RunError, RuntimeValue,
};
use crate::node_system::protocol::Value;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredValueKind {
    Scalar,
    Sequence,
    DataSeries,
}

#[derive(Clone)]
pub struct StoredValue {
    backing: Arc<StoredValueBacking>,
    logical_digest: [u8; 32],
}

#[derive(Debug)]
enum StoredValueBacking {
    Scalar(Value),
    InMemory(Arc<InMemoryStorage>),
    SpillBacked(Arc<super::spill::SpillStorage>),
}

#[derive(Debug)]
pub(crate) struct InMemoryStorage {
    values: Box<[Value]>,
    value_kind: ArtifactValueKind,
    data_series_metadata: Option<DataSeriesMetadata>,
    memory_reservation: Option<super::materialization::MemoryReservation>,
}

impl InMemoryStorage {
    pub(crate) fn new(
        values: Box<[Value]>,
        value_kind: ArtifactValueKind,
        data_series_metadata: Option<DataSeriesMetadata>,
        memory_reservation: Option<super::materialization::MemoryReservation>,
    ) -> Self {
        Self {
            values,
            value_kind,
            data_series_metadata,
            memory_reservation,
        }
    }

    pub(crate) fn values(&self) -> &[Value] {
        &self.values
    }

    pub(crate) const fn value_kind(&self) -> ArtifactValueKind {
        self.value_kind
    }

    pub(crate) fn data_series_metadata(&self) -> Option<&DataSeriesMetadata> {
        self.data_series_metadata.as_ref()
    }
}

impl StoredValue {
    pub fn scalar(value: Value) -> Self {
        let logical_digest = scalar_logical_digest(&value);
        Self {
            backing: Arc::new(StoredValueBacking::Scalar(value)),
            logical_digest,
        }
    }

    pub fn sequence(values: Box<[Value]>) -> Self {
        Self::in_memory(values, ArtifactValueKind::Sequence, None, None)
    }

    pub(crate) fn in_memory(
        values: Box<[Value]>,
        value_kind: ArtifactValueKind,
        data_series_metadata: Option<DataSeriesMetadata>,
        reservation: Option<super::materialization::MemoryReservation>,
    ) -> Self {
        Self::from_in_memory_storage(Arc::new(InMemoryStorage::new(
            values,
            value_kind,
            data_series_metadata,
            reservation,
        )))
    }

    pub(crate) fn from_in_memory_storage(storage: Arc<InMemoryStorage>) -> Self {
        let logical_digest = logical_value_digest(
            storage.value_kind(),
            storage.data_series_metadata(),
            storage.values(),
        );
        Self {
            backing: Arc::new(StoredValueBacking::InMemory(storage)),
            logical_digest,
        }
    }

    pub(crate) fn in_memory_with_digest(
        storage: Arc<InMemoryStorage>,
        logical_digest: [u8; 32],
    ) -> Self {
        Self {
            backing: Arc::new(StoredValueBacking::InMemory(storage)),
            logical_digest,
        }
    }

    pub(crate) fn spill_backed(storage: Arc<super::spill::SpillStorage>) -> Self {
        let logical_digest = storage.logical_digest();
        Self {
            backing: Arc::new(StoredValueBacking::SpillBacked(storage)),
            logical_digest,
        }
    }

    pub(crate) fn prepare(
        value: RuntimeValue,
        owner: &super::RunResourceOwner,
    ) -> Result<Self, RunError> {
        match value {
            RuntimeValue::Scalar(value) => Ok(Self::scalar(value)),
            RuntimeValue::Stream(stream) => owner.store_stream(stream, None, None),
            RuntimeValue::Artifact(artifact) => {
                let metadata = artifact.data_series_metadata().cloned();
                let stored = artifact.into_stored_value();
                if stored.is_owner_backed() {
                    stored.promote(&owner.cancellation(), owner.deadline())?;
                    Ok(stored)
                } else {
                    owner.store_values(
                        stored
                            .open_reader()
                            .map_err(|error| RunError::Stream(error.to_string().into()))?
                            .map(|value| {
                                value.map_err(|error| RunError::Stream(error.to_string().into()))
                            }),
                        metadata,
                        None,
                    )
                }
            }
        }
    }

    pub(crate) fn promote(
        &self,
        cancellation: &super::CancellationToken,
        deadline: Option<super::RunDeadline>,
    ) -> Result<(), RunError> {
        if let StoredValueBacking::SpillBacked(storage) = self.backing.as_ref() {
            storage.promote(cancellation, deadline)?;
        }
        Ok(())
    }

    pub const fn logical_digest(&self) -> [u8; 32] {
        self.logical_digest
    }

    pub fn kind(&self) -> StoredValueKind {
        match self.backing.as_ref() {
            StoredValueBacking::Scalar(_) => StoredValueKind::Scalar,
            StoredValueBacking::InMemory(storage) => value_kind(storage.value_kind()),
            StoredValueBacking::SpillBacked(storage) => value_kind(storage.value_kind()),
        }
    }

    pub fn len(&self) -> usize {
        match self.backing.as_ref() {
            StoredValueBacking::Scalar(_) => 1,
            StoredValueBacking::InMemory(storage) => storage.values.len(),
            StoredValueBacking::SpillBacked(storage) => storage.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn page(&self, offset: usize, limit: usize) -> Result<Box<[Value]>, StoredValueReadError> {
        let start = offset.min(self.len());
        let count = limit.min(self.len().saturating_sub(start));
        match self.backing.as_ref() {
            StoredValueBacking::Scalar(value) if start == 0 && count == 1 => {
                Ok(vec![value.clone()].into_boxed_slice())
            }
            StoredValueBacking::Scalar(_) => Ok(Box::default()),
            StoredValueBacking::InMemory(storage) => Ok(storage.values[start..start + count]
                .to_vec()
                .into_boxed_slice()),
            StoredValueBacking::SpillBacked(storage) => storage
                .cursor()
                .map_err(StoredValueReadError::runtime)?
                .skip(start)
                .take(count)
                .collect::<Result<Vec<_>, _>>()
                .map(Vec::into_boxed_slice)
                .map_err(StoredValueReadError::runtime),
        }
    }

    pub fn open_reader(&self) -> Result<StoredValueReader, StoredValueReadError> {
        let inner = match self.backing.as_ref() {
            StoredValueBacking::Scalar(value) => {
                StoredValueReaderInner::Scalar(Some(value.clone()))
            }
            StoredValueBacking::InMemory(storage) => StoredValueReaderInner::InMemory {
                storage: Arc::clone(storage),
                index: 0,
            },
            StoredValueBacking::SpillBacked(storage) => StoredValueReaderInner::Spill(
                storage.cursor().map_err(StoredValueReadError::runtime)?,
            ),
        };
        Ok(StoredValueReader { inner })
    }

    pub fn data_series_metadata(&self) -> Option<&DataSeriesMetadata> {
        match self.backing.as_ref() {
            StoredValueBacking::Scalar(_) => None,
            StoredValueBacking::InMemory(storage) => storage.data_series_metadata(),
            StoredValueBacking::SpillBacked(storage) => storage.data_series_metadata(),
        }
    }

    pub(crate) fn value_kind(&self) -> ArtifactValueKind {
        match self.backing.as_ref() {
            StoredValueBacking::Scalar(_) => ArtifactValueKind::Sequence,
            StoredValueBacking::InMemory(storage) => storage.value_kind(),
            StoredValueBacking::SpillBacked(storage) => storage.value_kind(),
        }
    }

    pub(crate) fn to_runtime_value(&self) -> RuntimeValue {
        match self.backing.as_ref() {
            StoredValueBacking::Scalar(value) => RuntimeValue::Scalar(value.clone()),
            _ => RuntimeValue::Artifact(Artifact::from_stored_value(
                ArtifactKind::Collected,
                self.clone(),
            )),
        }
    }

    pub(crate) fn in_memory_storage(&self) -> Option<Arc<InMemoryStorage>> {
        match self.backing.as_ref() {
            StoredValueBacking::InMemory(storage) => Some(Arc::clone(storage)),
            _ => None,
        }
    }

    pub(crate) fn spill_storage(&self) -> Option<Arc<super::spill::SpillStorage>> {
        match self.backing.as_ref() {
            StoredValueBacking::SpillBacked(storage) => Some(Arc::clone(storage)),
            _ => None,
        }
    }

    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.backing, &other.backing)
    }

    #[cfg(test)]
    pub(crate) fn is_spill_backed(&self) -> bool {
        matches!(self.backing.as_ref(), StoredValueBacking::SpillBacked(_))
    }

    fn is_owner_backed(&self) -> bool {
        match self.backing.as_ref() {
            StoredValueBacking::Scalar(_) => true,
            StoredValueBacking::InMemory(storage) => storage.memory_reservation.is_some(),
            StoredValueBacking::SpillBacked(storage) => storage.has_reservation(),
        }
    }
}

pub(crate) fn logical_digest_seed(
    kind: ArtifactValueKind,
    metadata: Option<&DataSeriesMetadata>,
) -> [u8; 32] {
    crate::node_system::registry::hash_canonical(
        "yssbi.stored-value.logical.seed.v1",
        &(kind_name(kind), metadata),
    )
    .expect("stored value metadata is canonical")
}

pub(crate) fn extend_logical_digest(current: [u8; 32], value: &Value) -> [u8; 32] {
    crate::node_system::registry::hash_canonical(
        "yssbi.stored-value.logical.value.v1",
        &(current, value),
    )
    .expect("stored values are canonical")
}

pub(crate) fn finish_logical_digest(current: [u8; 32], count: u64) -> [u8; 32] {
    crate::node_system::registry::hash_canonical(
        "yssbi.stored-value.logical.finish.v1",
        &(current, count),
    )
    .expect("stored value length is canonical")
}

fn scalar_logical_digest(value: &Value) -> [u8; 32] {
    crate::node_system::registry::hash_canonical("yssbi.stored-value.logical.scalar.v1", value)
        .expect("stored scalar is canonical")
}

fn logical_value_digest(
    kind: ArtifactValueKind,
    metadata: Option<&DataSeriesMetadata>,
    values: &[Value],
) -> [u8; 32] {
    let digest = values
        .iter()
        .fold(logical_digest_seed(kind, metadata), |digest, value| {
            extend_logical_digest(digest, value)
        });
    finish_logical_digest(digest, values.len() as u64)
}

fn kind_name(kind: ArtifactValueKind) -> &'static str {
    match kind {
        ArtifactValueKind::Sequence => "sequence",
        ArtifactValueKind::DataSeries => "dataSeries",
    }
}

fn value_kind(kind: ArtifactValueKind) -> StoredValueKind {
    match kind {
        ArtifactValueKind::Sequence => StoredValueKind::Sequence,
        ArtifactValueKind::DataSeries => StoredValueKind::DataSeries,
    }
}

impl PartialEq for StoredValue {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}

impl Eq for StoredValue {}

impl fmt::Debug for StoredValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredValue")
            .field("kind", &self.kind())
            .field("len", &self.len())
            .finish()
    }
}

pub struct StoredValueReader {
    inner: StoredValueReaderInner,
}

enum StoredValueReaderInner {
    Scalar(Option<Value>),
    InMemory {
        storage: Arc<InMemoryStorage>,
        index: usize,
    },
    Spill(super::spill::SpillCursor),
}

impl Iterator for StoredValueReader {
    type Item = Result<Value, StoredValueReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            StoredValueReaderInner::Scalar(value) => value.take().map(Ok),
            StoredValueReaderInner::InMemory { storage, index } => {
                let value = storage.values.get(*index)?.clone();
                *index += 1;
                Some(Ok(value))
            }
            StoredValueReaderInner::Spill(cursor) => cursor
                .next()
                .map(|value| value.map_err(StoredValueReadError::runtime)),
        }
    }
}

#[derive(Debug)]
pub struct StoredValueReadError {
    message: Box<str>,
}

impl StoredValueReadError {
    fn runtime(error: RunError) -> Self {
        Self {
            message: error.to_string().into(),
        }
    }
}

impl fmt::Display for StoredValueReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for StoredValueReadError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::runtime::{
        CancellationToken, DataSeriesElementType, DataSeriesMetadata, RunId, RunResourceBudgets,
        RunResourceOwner,
    };

    #[test]
    fn scalar_converts_to_kernel_runtime_value() {
        let stored = StoredValue::scalar(Value::Integer(7));
        assert_eq!(
            stored.to_runtime_value(),
            RuntimeValue::Scalar(Value::Integer(7))
        );
    }

    #[test]
    fn scalar_and_single_value_sequence_have_distinct_logical_digests() {
        let scalar = StoredValue::scalar(Value::Integer(7));
        let sequence = StoredValue::sequence(vec![Value::Integer(7)].into_boxed_slice());

        assert_ne!(scalar.logical_digest(), sequence.logical_digest());
    }

    #[test]
    fn sequence_digest_includes_order_and_data_series_metadata() {
        let ordered =
            StoredValue::sequence(vec![Value::Integer(1), Value::Integer(2)].into_boxed_slice());
        let reversed =
            StoredValue::sequence(vec![Value::Integer(2), Value::Integer(1)].into_boxed_slice());
        let metadata = DataSeriesMetadata {
            element_type: DataSeriesElementType::Int64,
            length: 2,
            null_count: 0,
            name: Some("observations".into()),
            format: None,
        };
        let series = StoredValue::in_memory(
            vec![Value::Integer(1), Value::Integer(2)].into_boxed_slice(),
            ArtifactValueKind::DataSeries,
            Some(metadata),
            None,
        );

        assert_ne!(ordered.logical_digest(), reversed.logical_digest());
        assert_ne!(ordered.logical_digest(), series.logical_digest());
    }

    #[test]
    fn data_series_conversion_preserves_metadata_and_storage() {
        let metadata = DataSeriesMetadata {
            element_type: DataSeriesElementType::Int64,
            length: 2,
            null_count: 0,
            name: Some("observations".into()),
            format: None,
        };
        let stored = StoredValue::in_memory(
            vec![Value::Integer(1), Value::Integer(2)].into_boxed_slice(),
            ArtifactValueKind::DataSeries,
            Some(metadata.clone()),
            None,
        );
        let RuntimeValue::Artifact(converted) = stored.to_runtime_value() else {
            panic!("data series must remain a kernel artifact view");
        };
        assert_eq!(converted.data_series_metadata(), Some(&metadata));
        assert!(stored.ptr_eq(&converted.into_stored_value()));
    }

    fn test_owner(
        run_id: u64,
        memory_bytes: u64,
        root: std::path::PathBuf,
        cancellation: CancellationToken,
    ) -> RunResourceOwner {
        RunResourceOwner::with_spill_root(
            RunId::new(run_id),
            RunResourceBudgets {
                stream_capacity: std::num::NonZeroUsize::new(1).unwrap(),
                materialization_memory_bytes: memory_bytes,
                spill_directory_bytes: 1024,
            },
            cancellation,
            root,
        )
        .unwrap()
    }

    #[test]
    fn spill_backed_stored_value_supports_two_independent_passes() {
        let root = std::env::temp_dir().join(format!("stored-spill-{}", uuid::Uuid::new_v4()));
        let cancellation = CancellationToken::new();
        let owner = test_owner(401, 1, root.clone(), cancellation);
        let stored = owner
            .store_values(
                [Value::Integer(1), Value::Integer(2)].into_iter().map(Ok),
                None,
                None,
            )
            .unwrap();

        let first = stored
            .open_reader()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let second = stored
            .open_reader()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(stored.is_spill_backed());
        assert_eq!(first, second);
        assert_eq!(first, [Value::Integer(1), Value::Integer(2)]);
        drop(stored);
        drop(owner);
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn owner_backed_artifact_prepare_is_zero_copy() {
        let value = Value::String("owner backed".into());
        let bytes = serde_json::to_vec(&value).unwrap().len() as u64;
        let root = std::env::temp_dir().join(format!("stored-zero-copy-{}", uuid::Uuid::new_v4()));
        let owner = test_owner(407, bytes, root.clone(), CancellationToken::new());
        let stored = owner
            .store_values(std::iter::once(Ok(value)), None, None)
            .unwrap();
        let artifact = Artifact::from_stored_value(ArtifactKind::Collected, stored.clone());

        let prepared = StoredValue::prepare(RuntimeValue::Artifact(artifact), &owner).unwrap();

        assert!(stored.ptr_eq(&prepared));
        assert_eq!(owner.memory_bytes_for_test(), bytes);
        drop(prepared);
        drop(stored);
        assert_eq!(owner.memory_bytes_for_test(), 0);
        drop(owner);
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn in_memory_clones_share_one_payload_and_reservation() {
        let value = Value::String("shared reservation".into());
        let bytes = serde_json::to_vec(&value).unwrap().len() as u64;
        let root = std::env::temp_dir().join(format!("stored-memory-{}", uuid::Uuid::new_v4()));
        let owner = test_owner(402, bytes, root.clone(), CancellationToken::new());
        let stored = owner
            .store_values(std::iter::once(Ok(value)), None, None)
            .unwrap();
        let clone = stored.clone();

        assert!(stored.ptr_eq(&clone));
        assert_eq!(owner.memory_bytes_for_test(), bytes);
        drop(stored);
        assert_eq!(owner.memory_bytes_for_test(), bytes);
        drop(clone);
        assert_eq!(owner.memory_bytes_for_test(), 0);
        drop(owner);
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn cancelled_pending_writer_removes_uncommitted_spill() {
        let root = std::env::temp_dir().join(format!("stored-cancel-{}", uuid::Uuid::new_v4()));
        let cancellation = CancellationToken::new();
        let owner = test_owner(403, 1, root.clone(), cancellation.clone());
        let mut writer = owner.pending_value_writer(None, None);
        writer.push(Value::Integer(1)).unwrap();
        let path = writer.spill_path_for_test().expect("writer spilled");
        cancellation.cancel();
        assert!(writer.push(Value::Integer(2)).is_err());
        assert!(writer.finish().is_err());
        assert!(!path.exists());
        assert_eq!(owner.spill_bytes_for_test(), 0);
        drop(owner);
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn failed_pending_writer_is_poisoned_and_cannot_finish() {
        let root = std::env::temp_dir().join(format!("stored-poison-{}", uuid::Uuid::new_v4()));
        let owner = test_owner(404, 1, root.clone(), CancellationToken::new());
        let mut writer = owner.pending_value_writer(None, None);
        writer.push(Value::Integer(1)).unwrap();
        let path = writer.spill_path_for_test().expect("writer spilled");
        writer.fail_next_append_for_test();

        assert!(writer.push(Value::Integer(2)).is_err());
        assert!(!path.exists());
        assert_eq!(owner.spill_bytes_for_test(), 0);
        assert!(writer.finish().is_err());

        drop(owner);
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn spill_readers_interleave_and_keep_file_alive_after_value_drop() {
        let root = std::env::temp_dir().join(format!("stored-readers-{}", uuid::Uuid::new_v4()));
        let owner = test_owner(405, 1, root.clone(), CancellationToken::new());
        let stored = owner
            .store_values(
                [Value::Integer(1), Value::Integer(2), Value::Integer(3)]
                    .into_iter()
                    .map(Ok),
                None,
                None,
            )
            .unwrap();
        stored.promote(&CancellationToken::new(), None).unwrap();
        let path = stored.spill_storage().unwrap().path_for_test();
        let mut first = stored.open_reader().unwrap();
        let mut second = stored.open_reader().unwrap();

        assert_eq!(first.next().unwrap().unwrap(), Value::Integer(1));
        assert_eq!(second.next().unwrap().unwrap(), Value::Integer(1));
        assert_eq!(second.next().unwrap().unwrap(), Value::Integer(2));
        drop(stored);
        assert!(path.exists());
        assert_eq!(first.next().unwrap().unwrap(), Value::Integer(2));
        assert_eq!(first.next().unwrap().unwrap(), Value::Integer(3));
        drop(first);
        assert!(path.exists());
        assert_eq!(second.next().unwrap().unwrap(), Value::Integer(3));
        drop(second);
        assert!(!path.exists());

        drop(owner);
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn spill_readers_run_concurrently_after_value_drop() {
        let root = std::env::temp_dir().join(format!("stored-concurrent-{}", uuid::Uuid::new_v4()));
        let owner = test_owner(406, 1, root.clone(), CancellationToken::new());
        let stored = owner
            .store_values((0..32).map(|value| Ok(Value::Integer(value))), None, None)
            .unwrap();
        stored.promote(&CancellationToken::new(), None).unwrap();
        let path = stored.spill_storage().unwrap().path_for_test();
        let first = stored.open_reader().unwrap();
        let second = stored.open_reader().unwrap();
        drop(stored);

        let first = std::thread::spawn(move || first.collect::<Result<Vec<_>, _>>().unwrap());
        let second = std::thread::spawn(move || second.collect::<Result<Vec<_>, _>>().unwrap());
        let expected = (0..32).map(Value::Integer).collect::<Vec<_>>();
        assert_eq!(first.join().unwrap(), expected);
        assert_eq!(second.join().unwrap(), expected);
        assert!(!path.exists());

        drop(owner);
        std::fs::remove_dir(root).unwrap();
    }
}
