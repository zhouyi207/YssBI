use super::{
    Artifact, ArtifactKind, ArtifactValueKind, CancellationToken, DataSeriesMetadata, RunDeadline,
    RunError, RunId, RunPhase, RuntimeValue, StoredValue, StreamReceiveError, StreamSendError,
    StreamValue, bounded_stream_channel_with_deadline, check_terminal,
};
use crate::node_system::plan::{MaterializationLimits, PlannedAdapter};
use crate::node_system::protocol::Value;
use std::fs;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, mpsc};
use std::thread::{self, JoinHandle};

const DEFAULT_STREAM_CAPACITY: usize = 16;
const DEFAULT_MATERIALIZATION_MEMORY_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_SPILL_DIRECTORY_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunResourceBudgets {
    pub stream_capacity: NonZeroUsize,
    /// Aggregate serde-JSON bytes retained by live owner-created in-memory artifacts.
    /// This deterministic protocol metric is not an estimate of Rust allocator usage.
    pub materialization_memory_bytes: u64,
    pub spill_directory_bytes: u64,
}

impl Default for RunResourceBudgets {
    fn default() -> Self {
        Self {
            stream_capacity: NonZeroUsize::new(DEFAULT_STREAM_CAPACITY).unwrap(),
            materialization_memory_bytes: DEFAULT_MATERIALIZATION_MEMORY_BYTES,
            spill_directory_bytes: DEFAULT_SPILL_DIRECTORY_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnerPhase {
    Active,
    Cleaning,
    Cleaned,
}

struct ProducerTask {
    handle: JoinHandle<()>,
    done: mpsc::Receiver<()>,
}

struct OwnerLifecycle {
    phase: OwnerPhase,
    producers: Vec<ProducerTask>,
}

#[derive(Debug)]
pub(crate) struct MemoryReservation {
    used: Arc<Mutex<u64>>,
    bytes: u64,
}

#[derive(Debug)]
pub(crate) struct SpillReservation {
    used: Arc<Mutex<u64>>,
    bytes: u64,
}

impl SpillReservation {
    fn new(used: Arc<Mutex<u64>>) -> Self {
        Self { used, bytes: 0 }
    }

    fn try_extend(&mut self, bytes: u64, limit: u64) -> Result<(), RunError> {
        let reserved = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| RunError::Stream("spill reservation overflowed".into()))?;
        let mut used = self.used.lock().unwrap_or_else(|error| error.into_inner());
        let next = used
            .checked_add(bytes)
            .ok_or_else(|| RunError::Stream("spill directory usage overflowed".into()))?;
        if next > limit {
            return Err(RunError::Stream(
                "run spill directory budget exceeded".into(),
            ));
        }
        *used = next;
        self.bytes = reserved;
        Ok(())
    }

    fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl Drop for SpillReservation {
    fn drop(&mut self) {
        let mut used = self.used.lock().unwrap_or_else(|error| error.into_inner());
        *used = used.saturating_sub(self.bytes);
    }
}

impl MemoryReservation {
    fn new(used: Arc<Mutex<u64>>) -> Self {
        Self { used, bytes: 0 }
    }

    fn try_extend(&mut self, bytes: u64, limit: u64) -> Result<bool, RunError> {
        let reserved = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| RunError::Stream("materialization reservation overflowed".into()))?;
        let mut used = self.used.lock().unwrap_or_else(|error| error.into_inner());
        let next = used
            .checked_add(bytes)
            .ok_or_else(|| RunError::Stream("materialization memory usage overflowed".into()))?;
        if next > limit {
            return Ok(false);
        }
        *used = next;
        self.bytes = reserved;
        Ok(true)
    }
}

impl Drop for MemoryReservation {
    fn drop(&mut self) {
        let mut used = self.used.lock().unwrap_or_else(|error| error.into_inner());
        *used = used.saturating_sub(self.bytes);
    }
}

pub struct RunResourceOwner {
    budgets: RunResourceBudgets,
    cancellation: CancellationToken,
    deadline: Option<RunDeadline>,
    spill_root: PathBuf,
    remove_root: bool,
    next_spill: AtomicU64,
    spill_bytes: Arc<Mutex<u64>>,
    memory_bytes: Arc<Mutex<u64>>,
    lifecycle: Mutex<OwnerLifecycle>,
    cleanup_ready: Condvar,
    #[cfg(test)]
    pending_writer_count: AtomicU64,
}

impl RunResourceOwner {
    pub fn new(
        run_id: RunId,
        budgets: RunResourceBudgets,
        cancellation: CancellationToken,
    ) -> Result<Self, RunError> {
        let root = std::env::temp_dir().join("yssbi-runtime").join(format!(
            "run-{}-{}",
            run_id.get(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).map_err(resource_io_error)?;
        Ok(Self::from_root(budgets, cancellation, None, root, true))
    }

    pub(crate) fn new_with_deadline(
        run_id: RunId,
        budgets: RunResourceBudgets,
        cancellation: CancellationToken,
        deadline: Option<RunDeadline>,
    ) -> Result<Self, RunError> {
        let root = std::env::temp_dir().join("yssbi-runtime").join(format!(
            "run-{}-{}",
            run_id.get(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).map_err(resource_io_error)?;
        Ok(Self::from_root(budgets, cancellation, deadline, root, true))
    }

    #[cfg(test)]
    pub(crate) fn with_spill_root(
        _: RunId,
        budgets: RunResourceBudgets,
        cancellation: CancellationToken,
        spill_root: PathBuf,
    ) -> Result<Self, RunError> {
        fs::create_dir_all(&spill_root).map_err(resource_io_error)?;
        Ok(Self::from_root(
            budgets,
            cancellation,
            None,
            spill_root,
            false,
        ))
    }

    #[cfg(test)]
    pub(crate) fn with_spill_root_and_deadline(
        _: RunId,
        budgets: RunResourceBudgets,
        cancellation: CancellationToken,
        deadline: Option<RunDeadline>,
        spill_root: PathBuf,
    ) -> Result<Self, RunError> {
        fs::create_dir_all(&spill_root).map_err(resource_io_error)?;
        Ok(Self::from_root(
            budgets,
            cancellation,
            deadline,
            spill_root,
            false,
        ))
    }

    fn from_root(
        budgets: RunResourceBudgets,
        cancellation: CancellationToken,
        deadline: Option<RunDeadline>,
        spill_root: PathBuf,
        remove_root: bool,
    ) -> Self {
        Self {
            budgets,
            cancellation,
            deadline,
            spill_root,
            remove_root,
            next_spill: AtomicU64::new(1),
            spill_bytes: Arc::new(Mutex::new(0)),
            memory_bytes: Arc::new(Mutex::new(0)),
            lifecycle: Mutex::new(OwnerLifecycle {
                phase: OwnerPhase::Active,
                producers: Vec::new(),
            }),
            cleanup_ready: Condvar::new(),
            #[cfg(test)]
            pending_writer_count: AtomicU64::new(0),
        }
    }

    pub const fn budgets(&self) -> RunResourceBudgets {
        self.budgets
    }

    pub const fn deadline(&self) -> Option<RunDeadline> {
        self.deadline
    }

    pub(crate) fn cancellation(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub fn stream_from_values<I>(&self, values: I) -> Result<StreamValue, RunError>
    where
        I: IntoIterator<Item = Value> + Send + 'static,
        I::IntoIter: Send,
    {
        self.stream_from_results_with_checkpoint(values.into_iter().map(Ok), || {})
    }

    #[cfg(test)]
    pub(crate) fn stream_from_values_with_registration_checkpoint<I>(
        &self,
        values: I,
        checkpoint: impl FnOnce(),
    ) -> Result<StreamValue, RunError>
    where
        I: IntoIterator<Item = Value> + Send + 'static,
        I::IntoIter: Send,
    {
        self.stream_from_results_with_checkpoint(values.into_iter().map(Ok), checkpoint)
    }

    fn stream_from_results(
        &self,
        values: impl Iterator<Item = Result<Value, RunError>> + Send + 'static,
    ) -> Result<StreamValue, RunError> {
        self.stream_from_results_with_checkpoint(values, || {})
    }

    fn stream_from_results_with_checkpoint(
        &self,
        values: impl Iterator<Item = Result<Value, RunError>> + Send + 'static,
        checkpoint: impl FnOnce(),
    ) -> Result<StreamValue, RunError> {
        check_terminal(&self.cancellation, self.deadline, RunPhase::AdapterIo)?;
        let (sender, receiver) = bounded_stream_channel_with_deadline(
            self.budgets.stream_capacity.get(),
            self.cancellation.clone(),
            self.deadline,
        )
        .map_err(|error| RunError::Stream(error.to_string().into()))?;
        let producer_error = Arc::new(Mutex::new(None));
        let producer_error_for_thread = Arc::clone(&producer_error);
        let mut lifecycle = self.lifecycle_lock();
        if lifecycle.phase != OwnerPhase::Active {
            return Err(RunError::Stream("run resource owner is cleaning up".into()));
        }
        let (done_sender, done_receiver) = mpsc::sync_channel(1);
        let handle = thread::Builder::new()
            .name("yssbi-stream-producer".into())
            .spawn(move || {
                let produced = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    for value in values {
                        let value = value.map_err(|error| error.to_string().into_boxed_str())?;
                        if matches!(
                            sender.send(value),
                            Err(StreamSendError::Cancelled(_)
                                | StreamSendError::DeadlineExceeded(_)
                                | StreamSendError::Closed(_))
                        ) {
                            return Ok(());
                        }
                    }
                    Ok::<(), Box<str>>(())
                }));
                let error = match produced {
                    Ok(Ok(())) => None,
                    Ok(Err(error)) => Some(error),
                    Err(_) => Some(Box::<str>::from("stream producer panicked")),
                };
                if let Some(error) = error {
                    *producer_error_for_thread
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner()) = Some(error);
                }
                sender.close();
                let _ = done_sender.send(());
            })
            .map_err(|error| {
                RunError::Stream(format!("failed to spawn stream producer: {error}").into())
            })?;
        checkpoint();
        lifecycle.producers.push(ProducerTask {
            handle,
            done: done_receiver,
        });
        drop(lifecycle);
        Ok(StreamValue::from_receiver_with_error(
            receiver,
            producer_error,
        ))
    }

    pub(crate) fn store_stream(
        &self,
        stream: StreamValue,
        metadata: Option<DataSeriesMetadata>,
        limits: Option<&MaterializationLimits>,
    ) -> Result<StoredValue, RunError> {
        self.store_values(StreamRuntimeValues { stream }, metadata, limits)
    }

    pub(crate) fn store_values(
        &self,
        values: impl Iterator<Item = Result<Value, RunError>>,
        metadata: Option<DataSeriesMetadata>,
        limits: Option<&MaterializationLimits>,
    ) -> Result<StoredValue, RunError> {
        let mut writer = self.pending_value_writer(metadata, limits);
        for value in values {
            writer.push_result(value)?;
        }
        writer.finish()
    }

    /// Operation-local physical builder for an already allocated output-group slot.
    /// It has no independent result identity and cannot publish into `ResultStore`.
    pub(crate) fn pending_value_writer(
        &self,
        metadata: Option<DataSeriesMetadata>,
        limits: Option<&MaterializationLimits>,
    ) -> PendingValueWriter<'_> {
        self.pending_value_writer_with_memory_limit(
            metadata,
            limits,
            self.budgets.materialization_memory_bytes,
        )
    }

    fn pending_value_writer_with_memory_limit(
        &self,
        metadata: Option<DataSeriesMetadata>,
        limits: Option<&MaterializationLimits>,
        memory_limit: u64,
    ) -> PendingValueWriter<'_> {
        #[cfg(test)]
        self.pending_writer_count.fetch_add(1, Ordering::Relaxed);
        let logical_digest = super::stored_value::logical_digest_seed(
            if metadata.is_some() {
                ArtifactValueKind::DataSeries
            } else {
                ArtifactValueKind::Sequence
            },
            metadata.as_ref(),
        );
        PendingValueWriter {
            owner: self,
            metadata,
            limits: limits.cloned(),
            memory_limit,
            values: Vec::new(),
            reservation: Some(self.memory_reservation()),
            encoded_bytes: 0,
            value_count: 0,
            staged_path: None,
            spill_reservation: Some(self.spill_reservation()),
            max_record_bytes: 0,
            logical_digest,
            poisoned: None,
            #[cfg(test)]
            fail_next_append: false,
        }
    }

    fn spill_reservation(&self) -> SpillReservation {
        SpillReservation::new(Arc::clone(&self.spill_bytes))
    }

    #[cfg(test)]
    pub(crate) fn pending_writer_count_for_test(&self) -> u64 {
        self.pending_writer_count.load(Ordering::Relaxed)
    }

    pub(crate) fn cleanup(&self) -> Box<[Box<str>]> {
        let mut cleanup_timed_out = false;
        let producers = {
            let mut lifecycle = self.lifecycle_lock();
            loop {
                match lifecycle.phase {
                    OwnerPhase::Active => {
                        lifecycle.phase = OwnerPhase::Cleaning;
                        break std::mem::take(&mut lifecycle.producers);
                    }
                    OwnerPhase::Cleaning => {
                        lifecycle = match self.deadline {
                            Some(deadline) => {
                                match deadline.remaining_monotonic(RunPhase::Cleanup) {
                                    Ok(remaining) => {
                                        self.cleanup_ready
                                            .wait_timeout(lifecycle, remaining)
                                            .unwrap_or_else(|error| error.into_inner())
                                            .0
                                    }
                                    Err(_) => {
                                        cleanup_timed_out = true;
                                        self.cleanup_ready
                                            .wait(lifecycle)
                                            .unwrap_or_else(|error| error.into_inner())
                                    }
                                }
                            }
                            None => self
                                .cleanup_ready
                                .wait(lifecycle)
                                .unwrap_or_else(|error| error.into_inner()),
                        };
                    }
                    OwnerPhase::Cleaned => return Box::new([]),
                }
            }
        };
        let mut errors = Vec::new();
        if cleanup_timed_out {
            errors.push(Box::<str>::from(
                "cleanup deadline exceeded while waiting for owner",
            ));
        }
        for producer in producers {
            let completed_before_deadline = match self.deadline {
                Some(deadline) => match deadline.remaining_monotonic(RunPhase::Cleanup) {
                    Ok(remaining) => producer.done.recv_timeout(remaining).is_ok(),
                    Err(_) => false,
                },
                None => producer.done.recv().is_ok(),
            };
            if !completed_before_deadline {
                cleanup_timed_out = true;
                errors.push(Box::<str>::from(
                    "cleanup deadline exceeded while draining producer",
                ));
            }
            if producer.handle.join().is_err() {
                errors.push(Box::<str>::from("stream producer panicked"));
            }
        }
        if self
            .deadline
            .is_some_and(|deadline| deadline.remaining_monotonic(RunPhase::Cleanup).is_err())
            && !cleanup_timed_out
        {
            cleanup_timed_out = true;
            errors.push(Box::<str>::from(
                "cleanup deadline exceeded before spill removal",
            ));
        }
        let removal = if self.remove_root {
            fs::remove_dir_all(&self.spill_root)
        } else {
            remove_directory_contents(&self.spill_root)
        };
        if let Err(error) = removal
            && error.kind() != std::io::ErrorKind::NotFound
        {
            errors.push(format!("failed to remove run spill files: {error}").into());
        }
        if self
            .deadline
            .is_some_and(|deadline| deadline.remaining_monotonic(RunPhase::Cleanup).is_err())
            && !cleanup_timed_out
        {
            errors.push(Box::<str>::from(
                "cleanup deadline exceeded during spill removal",
            ));
        }
        let mut lifecycle = self.lifecycle_lock();
        lifecycle.phase = OwnerPhase::Cleaned;
        drop(lifecycle);
        self.cleanup_ready.notify_all();
        errors.into_boxed_slice()
    }

    fn lifecycle_lock(&self) -> MutexGuard<'_, OwnerLifecycle> {
        self.lifecycle
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    fn memory_reservation(&self) -> MemoryReservation {
        MemoryReservation::new(Arc::clone(&self.memory_bytes))
    }

    #[cfg(test)]
    pub(crate) fn memory_bytes_for_test(&self) -> u64 {
        *self
            .memory_bytes
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    #[cfg(test)]
    pub(crate) fn spill_bytes_for_test(&self) -> u64 {
        *self
            .spill_bytes
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    #[cfg(test)]
    pub(crate) fn register_cleanup_delay_for_test(&self, duration: std::time::Duration) {
        let mut lifecycle = self.lifecycle_lock();
        assert_eq!(lifecycle.phase, OwnerPhase::Active);
        let (done_sender, done) = mpsc::sync_channel(1);
        let handle = thread::spawn(move || {
            thread::sleep(duration);
            let _ = done_sender.send(());
        });
        lifecycle.producers.push(ProducerTask { handle, done });
    }

    #[cfg(test)]
    pub(crate) fn register_panicking_cleanup_task_for_test(&self) {
        let mut lifecycle = self.lifecycle_lock();
        assert_eq!(lifecycle.phase, OwnerPhase::Active);
        let (done_sender, done) = mpsc::sync_channel(1);
        let handle = thread::spawn(move || {
            let _ = done_sender.send(());
            panic!("cleanup task panic sentinel");
        });
        lifecycle.producers.push(ProducerTask { handle, done });
    }
}

impl Drop for RunResourceOwner {
    fn drop(&mut self) {
        let errors = self.cleanup();
        for error in errors {
            tauri_plugin_log::log::warn!(
                target: "yssbi::node_system::runtime::cleanup",
                "{error}"
            );
        }
    }
}

pub(crate) struct PendingValueWriter<'a> {
    owner: &'a RunResourceOwner,
    metadata: Option<DataSeriesMetadata>,
    limits: Option<MaterializationLimits>,
    memory_limit: u64,
    values: Vec<Value>,
    reservation: Option<MemoryReservation>,
    encoded_bytes: u64,
    value_count: u64,
    staged_path: Option<PathBuf>,
    spill_reservation: Option<SpillReservation>,
    max_record_bytes: u64,
    logical_digest: [u8; 32],
    poisoned: Option<Box<str>>,
    #[cfg(test)]
    fail_next_append: bool,
}

impl PendingValueWriter<'_> {
    fn push_result(&mut self, value: Result<Value, RunError>) -> Result<(), RunError> {
        match value {
            Ok(value) => self.push(value),
            Err(error) => self.poison(error),
        }
    }

    pub fn push(&mut self, value: Value) -> Result<(), RunError> {
        if let Some(message) = &self.poisoned {
            return Err(RunError::Stream(message.clone()));
        }
        match self.push_inner(value) {
            Ok(()) => Ok(()),
            Err(error) => self.poison(error),
        }
    }

    fn push_inner(&mut self, value: Value) -> Result<(), RunError> {
        check_terminal(
            &self.owner.cancellation,
            self.owner.deadline,
            RunPhase::AdapterIo,
        )?;
        let value_bytes = super::spill::serialized_value_len(&value)?;
        let next_count = self
            .value_count
            .checked_add(1)
            .ok_or_else(|| RunError::Stream("materialized value count overflowed".into()))?;
        let next_bytes = self
            .encoded_bytes
            .checked_add(value_bytes)
            .ok_or_else(|| RunError::Stream("materialized byte count overflowed".into()))?;
        if let Some(limits) = &self.limits {
            if next_count > limits.max_values {
                return Err(RunError::Stream(
                    "materialization value limit exceeded".into(),
                ));
            }
            if next_bytes > limits.max_bytes {
                return Err(RunError::Stream(
                    "materialization byte limit exceeded".into(),
                ));
            }
        }
        self.logical_digest =
            super::stored_value::extend_logical_digest(self.logical_digest, &value);
        self.value_count = next_count;
        self.encoded_bytes = next_bytes;
        self.max_record_bytes = self.max_record_bytes.max(value_bytes);

        if self.staged_path.is_some() {
            return self.append_spilled(&value, value_bytes);
        }
        let reservation = self
            .reservation
            .as_mut()
            .expect("memory writer reservation");
        if reservation.try_extend(value_bytes, self.memory_limit)? {
            self.values.push(value);
            return Ok(());
        }
        self.begin_spill(value)
    }

    fn begin_spill(&mut self, value: Value) -> Result<(), RunError> {
        let sequence = self.owner.next_spill.fetch_add(1, Ordering::Relaxed);
        let path = self
            .owner
            .spill_root
            .join(format!("pending-{sequence}.jsonf"));
        let values = self.values.drain(..).chain(std::iter::once(value));
        let reservation = self
            .spill_reservation
            .as_mut()
            .expect("pending writer spill reservation");
        let result =
            super::spill::write_spill(&path, values.map(Ok), &self.owner.cancellation, |bytes| {
                check_terminal(
                    &self.owner.cancellation,
                    self.owner.deadline,
                    RunPhase::AdapterIo,
                )?;
                reservation.try_extend(bytes, self.owner.budgets.spill_directory_bytes)
            });
        match result {
            Ok(_) => {
                if let Err(error) = check_terminal(
                    &self.owner.cancellation,
                    self.owner.deadline,
                    RunPhase::AdapterIo,
                ) {
                    let _ = fs::remove_file(path);
                    self.spill_reservation = Some(self.owner.spill_reservation());
                    return Err(error);
                }
                self.reservation.take();
                self.staged_path = Some(path);
                Ok(())
            }
            Err(error) => {
                let _ = fs::remove_file(path);
                self.spill_reservation = Some(self.owner.spill_reservation());
                Err(error)
            }
        }
    }

    fn append_spilled(&mut self, value: &Value, value_bytes: u64) -> Result<(), RunError> {
        #[cfg(test)]
        if std::mem::take(&mut self.fail_next_append) {
            return Err(RunError::Stream("injected spill append failure".into()));
        }
        let record_bytes = 8_u64
            .checked_add(value_bytes)
            .ok_or_else(|| RunError::Stream("spill record size overflowed".into()))?;
        let reservation = self
            .spill_reservation
            .as_mut()
            .expect("pending writer spill reservation");
        reservation.try_extend(record_bytes, self.owner.budgets.spill_directory_bytes)?;
        let path = self.staged_path.as_ref().expect("spill path exists");
        super::spill::append_spill_value(path, value, &self.owner.cancellation, self.owner.deadline)
            .map(|_| ())
    }

    fn poison<T>(&mut self, error: RunError) -> Result<T, RunError> {
        let message: Box<str> = format!("pending value writer failed: {error}").into();
        self.poisoned = Some(message.clone());
        if let Some(path) = self.staged_path.take() {
            let _ = fs::remove_file(path);
        }
        self.values.clear();
        self.reservation.take();
        self.spill_reservation = Some(self.owner.spill_reservation());
        Err(RunError::Stream(message))
    }

    pub fn finish(mut self) -> Result<StoredValue, RunError> {
        if let Some(message) = &self.poisoned {
            return Err(RunError::Stream(message.clone()));
        }
        if let Err(error) = check_terminal(
            &self.owner.cancellation,
            self.owner.deadline,
            RunPhase::AdapterIo,
        ) {
            return self.poison(error);
        }
        let value_kind = if self.metadata.is_some() {
            ArtifactValueKind::DataSeries
        } else {
            ArtifactValueKind::Sequence
        };
        let logical_digest =
            super::stored_value::finish_logical_digest(self.logical_digest, self.value_count);
        let stored = if let Some(path) = self.staged_path.take() {
            let count = usize::try_from(self.value_count)
                .map_err(|_| RunError::Stream("spill value count exceeds this platform".into()))?;
            let storage = Arc::new(super::spill::SpillStorage::new(
                path,
                super::spill::SpillMetadata {
                    bytes: self
                        .spill_reservation
                        .as_ref()
                        .expect("spill reservation")
                        .bytes(),
                    count,
                    max_record_bytes: self.max_record_bytes,
                },
                value_kind,
                self.metadata.take(),
                logical_digest,
                self.spill_reservation.take(),
            ));
            StoredValue::spill_backed(storage)
        } else {
            StoredValue::in_memory_with_digest(
                Arc::new(super::stored_value::InMemoryStorage::new(
                    self.values.drain(..).collect::<Vec<_>>().into_boxed_slice(),
                    value_kind,
                    self.metadata.take(),
                    self.reservation.take(),
                )),
                logical_digest,
            )
        };
        if let Err(error) = stored.promote(&self.owner.cancellation, self.owner.deadline) {
            return self.poison(error);
        }
        Ok(stored)
    }

    #[cfg(test)]
    pub(crate) fn spill_path_for_test(&self) -> Option<PathBuf> {
        self.staged_path.clone()
    }

    #[cfg(test)]
    pub(crate) fn fail_next_append_for_test(&mut self) {
        self.fail_next_append = true;
    }
}

impl Drop for PendingValueWriter<'_> {
    fn drop(&mut self) {
        if let Some(path) = self.staged_path.take() {
            let _ = fs::remove_file(path);
        }
        self.spill_reservation.take();
    }
}

pub fn execute_planned_adapter(
    adapter: &PlannedAdapter,
    value: RuntimeValue,
    owner: &RunResourceOwner,
    cancellation: &CancellationToken,
) -> Result<RuntimeValue, RunError> {
    check_terminal(cancellation, owner.deadline, RunPhase::AdapterIo)?;
    let result = match adapter {
        PlannedAdapter::Identity => Ok(value),
        PlannedAdapter::StreamBridge { .. } => match value {
            RuntimeValue::Stream(stream) => Ok(RuntimeValue::Stream(stream)),
            value => Ok(RuntimeValue::Stream(
                owner.stream_from_results(runtime_values(value)?)?,
            )),
        },
        PlannedAdapter::Buffer { .. } => materialize(
            ArtifactKind::Buffered,
            value,
            owner.budgets.materialization_memory_bytes,
            None,
            owner,
            cancellation,
        ),
        PlannedAdapter::Collect { limits } => materialize(
            ArtifactKind::Collected,
            value,
            owner.budgets.materialization_memory_bytes,
            Some(limits),
            owner,
            cancellation,
        ),
        PlannedAdapter::Spill { .. } => {
            let (_, metadata) = artifact_payload(&value);
            let mut writer = owner.pending_value_writer_with_memory_limit(metadata, None, 0);
            for value in runtime_values(value)? {
                writer.push_result(value)?;
            }
            writer.finish().map(|stored| {
                RuntimeValue::Artifact(Artifact::from_stored_value(ArtifactKind::Spilled, stored))
            })
        }
    };
    check_terminal(cancellation, owner.deadline, RunPhase::AdapterIo)?;
    result
}

fn materialize(
    kind: ArtifactKind,
    value: RuntimeValue,
    memory_limit: u64,
    limits: Option<&MaterializationLimits>,
    owner: &RunResourceOwner,
    cancellation: &CancellationToken,
) -> Result<RuntimeValue, RunError> {
    let (_, metadata) = artifact_payload(&value);
    let mut writer = owner.pending_value_writer_with_memory_limit(metadata, limits, memory_limit);
    for value in runtime_values(value)? {
        writer.push_result(value)?;
    }
    check_terminal(cancellation, owner.deadline, RunPhase::AdapterIo)?;
    writer
        .finish()
        .map(|stored| RuntimeValue::Artifact(Artifact::from_stored_value(kind, stored)))
}

fn artifact_payload(value: &RuntimeValue) -> (ArtifactValueKind, Option<DataSeriesMetadata>) {
    match value {
        RuntimeValue::Artifact(artifact) => (
            artifact.value_kind(),
            artifact.data_series_metadata().cloned(),
        ),
        RuntimeValue::Scalar(_) | RuntimeValue::Stream(_) => (ArtifactValueKind::Sequence, None),
    }
}

fn runtime_values(
    value: RuntimeValue,
) -> Result<Box<dyn Iterator<Item = Result<Value, RunError>> + Send>, RunError> {
    match value {
        RuntimeValue::Scalar(value) => Ok(Box::new(std::iter::once(Ok(value)))),
        RuntimeValue::Artifact(artifact) => Ok(Box::new(artifact.into_cursor()?)),
        RuntimeValue::Stream(stream) => Ok(Box::new(StreamRuntimeValues { stream })),
    }
}

struct StreamRuntimeValues {
    stream: StreamValue,
}

impl Iterator for StreamRuntimeValues {
    type Item = Result<Value, RunError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.stream.recv() {
            Ok(value) => Some(Ok(value)),
            Err(StreamReceiveError::Closed) => None,
            Err(StreamReceiveError::Cancelled) => Some(Err(RunError::Cancelled)),
            Err(StreamReceiveError::DeadlineExceeded) => Some(Err(RunError::DeadlineExceeded {
                phase: RunPhase::StreamReceive,
            })),
            Err(StreamReceiveError::Failed(message)) => Some(Err(RunError::Stream(message))),
            Err(StreamReceiveError::Empty) => unreachable!("blocking receive is not empty"),
        }
    }
}

fn remove_directory_contents(path: &PathBuf) -> std::io::Result<()> {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn resource_io_error(error: std::io::Error) -> RunError {
    RunError::Stream(format!("run materialization resource I/O failed: {error}").into())
}
