use super::{
    BoundedStreamReceiver, DataSeriesContractError, DataSeriesMetadata, KernelErrorKind,
    RelationalError, RelationalErrorCode, RunResourceBudgets, SchedulingPolicy, StoredValue,
    StreamReceiveError,
};
use crate::execution::plan::legacy::{OperationIndex, RelationalBackendId, ResourceId, ValueRef};
use crate::graph::analysis::contracts::CompileProvenance;
use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex, Weak};
use std::time::{Duration, Instant};
use yss_graph_protocol::Value;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct RunId(NonZeroU64);

impl RunId {
    pub fn try_new(value: u64) -> Result<Self, InvalidRunId> {
        NonZeroU64::new(value).map(Self).ok_or(InvalidRunId)
    }

    pub fn new(value: u64) -> Self {
        Self::try_new(value).expect("run IDs must be non-zero")
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidRunId;

impl fmt::Display for InvalidRunId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("run IDs must be non-zero")
    }
}

impl std::error::Error for InvalidRunId {}

#[cfg(test)]
mod identity_tests {
    use super::{InvalidRunId, RunId};

    #[test]
    fn run_id_rejects_zero() {
        assert_eq!(RunId::try_new(0), Err(InvalidRunId));
        assert_eq!(RunId::try_new(1).unwrap().get(), 1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunPhase {
    QueueWait,
    Kernel,
    StreamSend,
    StreamReceive,
    AdapterIo,
    ResultPublication,
    Cleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunDeadline(Instant);

impl RunDeadline {
    pub fn at(instant: Instant) -> Self {
        Self(instant)
    }

    pub fn after(duration: Duration) -> Self {
        Self(Instant::now() + duration)
    }

    pub(crate) fn exceeded_at(self, instant: Instant) -> bool {
        instant >= self.0
    }

    pub(crate) fn remaining_monotonic(self, phase: RunPhase) -> Result<Duration, RunError> {
        self.0
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(RunError::DeadlineExceeded { phase })
    }

    pub fn remaining(
        self,
        cancellation: &CancellationToken,
        phase: RunPhase,
    ) -> Result<Duration, RunError> {
        cancellation.check()?;
        self.remaining_monotonic(phase)
    }

    pub fn check(self, cancellation: &CancellationToken, phase: RunPhase) -> Result<(), RunError> {
        self.remaining(cancellation, phase).map(|_| ())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RunOptions {
    pub deadline: Option<RunDeadline>,
    pub budgets: RunResourceBudgets,
    pub scheduling: SchedulingPolicy,
}

pub(crate) fn check_terminal(
    cancellation: &CancellationToken,
    deadline: Option<RunDeadline>,
    phase: RunPhase,
) -> Result<(), RunError> {
    cancellation.check()?;
    if let Some(deadline) = deadline {
        deadline.check(cancellation, phase)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Buffered,
    Collected,
    Spilled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactValueKind {
    Sequence,
    DataSeries,
}

#[derive(Debug, Clone)]
pub(crate) enum MaterializedArtifact {
    InMemory(Arc<super::stored_value::InMemoryStorage>),
    Spilled(Arc<super::spill::SpillStorage>),
}

#[cfg(test)]
impl MaterializedArtifact {
    pub(crate) fn promote(
        &self,
        cancellation: &CancellationToken,
        deadline: Option<RunDeadline>,
    ) -> Result<(), RunError> {
        match self {
            Self::InMemory(_) => Ok(()),
            Self::Spilled(storage) => storage.promote(cancellation, deadline),
        }
    }
}

impl PartialEq for MaterializedArtifact {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InMemory(left), Self::InMemory(right)) => Arc::ptr_eq(left, right),
            (Self::Spilled(left), Self::Spilled(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }
}

impl Eq for MaterializedArtifact {}

#[derive(Debug, Clone)]
pub struct Artifact {
    kind: ArtifactKind,
    stored: StoredValue,
    materialized: MaterializedArtifact,
}

impl Artifact {
    pub fn new(kind: ArtifactKind, values: impl Into<Box<[Value]>>) -> Self {
        Self::from_stored_value(kind, StoredValue::sequence(values.into()))
    }

    pub fn new_data_series(
        kind: ArtifactKind,
        metadata: DataSeriesMetadata,
        values: impl Into<Box<[Value]>>,
    ) -> Result<Self, DataSeriesContractError> {
        let values = values.into();
        super::data_series::validate_data_series_values(&metadata, &values)?;
        Ok(Self::from_stored_value(
            kind,
            StoredValue::in_memory(values, ArtifactValueKind::DataSeries, Some(metadata), None),
        ))
    }

    pub(crate) fn from_stored_value(kind: ArtifactKind, stored: StoredValue) -> Self {
        let materialized = if let Some(storage) = stored.in_memory_storage() {
            MaterializedArtifact::InMemory(storage)
        } else if let Some(storage) = stored.spill_storage() {
            MaterializedArtifact::Spilled(storage)
        } else {
            unreachable!("scalar StoredValue cannot be exposed as an Artifact")
        };
        Self {
            kind,
            stored,
            materialized,
        }
    }

    pub const fn kind(&self) -> ArtifactKind {
        self.kind
    }

    pub fn value_kind(&self) -> ArtifactValueKind {
        self.stored.value_kind()
    }

    pub fn data_series_metadata(&self) -> Option<&DataSeriesMetadata> {
        self.stored.data_series_metadata()
    }

    #[cfg(test)]
    pub(crate) fn materialized(&self) -> &MaterializedArtifact {
        &self.materialized
    }

    pub fn in_memory_values(&self) -> Option<&[Value]> {
        match &self.materialized {
            MaterializedArtifact::InMemory(storage) => Some(storage.values()),
            MaterializedArtifact::Spilled(_) => None,
        }
    }

    pub(crate) fn cursor(&self) -> Result<ArtifactCursor, RunError> {
        ArtifactCursor::from_stored_value(self.stored.clone())
    }

    pub(crate) fn into_cursor(self) -> Result<ArtifactCursor, RunError> {
        ArtifactCursor::from_stored_value(self.stored)
    }

    pub(crate) fn into_stored_value(self) -> StoredValue {
        self.stored
    }

    #[cfg(test)]
    pub(crate) fn promote(
        &self,
        cancellation: &CancellationToken,
        deadline: Option<RunDeadline>,
    ) -> Result<(), RunError> {
        self.materialized.promote(cancellation, deadline)
    }
}

impl PartialEq for Artifact {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.materialized == other.materialized
    }
}

impl Eq for Artifact {}

pub(crate) struct ArtifactCursor {
    reader: super::StoredValueReader,
}

impl ArtifactCursor {
    fn from_stored_value(stored: StoredValue) -> Result<Self, RunError> {
        let reader = stored
            .open_reader()
            .map_err(|error| RunError::Stream(error.to_string().into()))?;
        Ok(Self { reader })
    }
}

impl Iterator for ArtifactCursor {
    type Item = Result<Value, RunError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader
            .next()
            .map(|value| value.map_err(|error| RunError::Stream(error.to_string().into())))
    }
}

#[derive(Clone)]
pub struct StreamValue {
    receiver: BoundedStreamReceiver<Value>,
    producer_error: Arc<Mutex<Option<Box<str>>>>,
}

impl StreamValue {
    pub(crate) fn from_receiver_with_error(
        receiver: BoundedStreamReceiver<Value>,
        producer_error: Arc<Mutex<Option<Box<str>>>>,
    ) -> Self {
        Self {
            receiver,
            producer_error,
        }
    }

    pub fn recv(&self) -> Result<Value, StreamReceiveError> {
        match self.receiver.recv() {
            Err(StreamReceiveError::Closed) => {
                let error = self
                    .producer_error
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .clone();
                match error {
                    Some(error) => Err(StreamReceiveError::Failed(error)),
                    None => Err(StreamReceiveError::Closed),
                }
            }
            result => result,
        }
    }

    pub fn close(&self) {
        self.receiver.close();
    }

    pub fn is_closed(&self) -> bool {
        self.receiver.is_closed()
    }
}

impl fmt::Debug for StreamValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StreamValue")
            .field("closed", &self.is_closed())
            .finish_non_exhaustive()
    }
}

impl PartialEq for StreamValue {
    fn eq(&self, other: &Self) -> bool {
        self.receiver.same_channel(&other.receiver)
    }
}

impl Eq for StreamValue {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeValue {
    Scalar(Value),
    Artifact(Artifact),
    Stream(StreamValue),
}

impl From<Value> for RuntimeValue {
    fn from(value: Value) -> Self {
        Self::Scalar(value)
    }
}

macro_rules! runtime_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

runtime_id!(ActivationId);
runtime_id!(FrameId);

pub(crate) static ACTIVATION_IDS: LazyLock<ActivationIdAllocator> =
    LazyLock::new(|| ActivationIdAllocator::new(NonZeroU64::MIN));
pub(crate) static FRAME_IDS: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub(crate) struct ActivationIdAllocator {
    next: Mutex<Option<NonZeroU64>>,
}

impl ActivationIdAllocator {
    const fn new(next: NonZeroU64) -> Self {
        Self {
            next: Mutex::new(Some(next)),
        }
    }

    pub(crate) fn allocate(&self) -> Result<ActivationId, RunError> {
        let mut next = self.next.lock().unwrap_or_else(|error| error.into_inner());
        let current = next.ok_or(RunError::ActivationIdExhausted)?;
        *next = current.get().checked_add(1).and_then(NonZeroU64::new);
        Ok(ActivationId(current.get()))
    }

    #[cfg(test)]
    pub(crate) const fn for_test(next: NonZeroU64) -> Self {
        Self::new(next)
    }
}

#[cfg(test)]
impl ActivationId {
    pub(crate) fn next() -> Result<Self, RunError> {
        ACTIVATION_IDS.allocate()
    }
}

impl FrameId {
    pub(crate) fn allocate(allocator: &AtomicU64) -> Result<Self, RunError> {
        allocator
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                NonZeroU64::new(current)?.get().checked_add(1)
            })
            .map(Self)
            .map_err(|_| RunError::RuntimeIdExhausted)
    }

    #[cfg(test)]
    pub(crate) fn next() -> Result<Self, RunError> {
        Self::allocate(&FRAME_IDS)
    }
}

#[derive(Debug)]
struct CancellationState {
    cancelled: Arc<AtomicBool>,
    cancelled_at: Mutex<Option<Instant>>,
    wait_lock: Mutex<()>,
    cancelled_ready: Condvar,
    waiters: Mutex<Vec<Weak<Condvar>>>,
}

#[derive(Debug, Clone)]
pub struct CancellationToken {
    state: Arc<CancellationState>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self {
            state: Arc::new(CancellationState {
                cancelled: Arc::new(AtomicBool::new(false)),
                cancelled_at: Mutex::new(None),
                wait_lock: Mutex::new(()),
                cancelled_ready: Condvar::new(),
                waiters: Mutex::new(Vec::new()),
            }),
        }
    }
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        let wait = self
            .state
            .wait_lock
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !self.state.cancelled.load(Ordering::Acquire) {
            *self
                .state
                .cancelled_at
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Some(Instant::now());
            self.state.cancelled.store(true, Ordering::Release);
        }
        self.state.cancelled_ready.notify_all();
        drop(wait);
        let mut waiters = self
            .state
            .waiters
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        waiters.retain(|waiter| {
            let Some(waiter) = waiter.upgrade() else {
                return false;
            };
            waiter.notify_all();
            true
        });
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn cancelled_at(&self) -> Option<Instant> {
        *self
            .state
            .cancelled_at
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    pub(crate) fn shared_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.state.cancelled)
    }

    pub(crate) fn shares_state_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }

    pub(crate) fn wait_timeout(&self, timeout: Duration) -> bool {
        let wait = self
            .state
            .wait_lock
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if self.is_cancelled() {
            return true;
        }
        let _ = self
            .state
            .cancelled_ready
            .wait_timeout_while(wait, timeout, |_| !self.is_cancelled())
            .unwrap_or_else(|error| error.into_inner());
        self.is_cancelled()
    }

    pub(crate) fn register_waiter(&self, waiter: &Arc<Condvar>) {
        let mut waiters = self
            .state
            .waiters
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        waiters.push(Arc::downgrade(waiter));
        if self.is_cancelled() {
            waiter.notify_all();
        }
    }

    pub(crate) fn check(&self) -> Result<(), RunError> {
        if self.is_cancelled() {
            Err(RunError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    pub run_id: RunId,
    pub provenance: CompileProvenance,
    pub result_ids: BTreeMap<Box<str>, super::ResultId>,
    pub(crate) results: super::ResultStore,
    pub committed_variable_ids: Box<[yss_variable_contract::VariableId]>,
    pub resource_mutation: Option<crate::schema::application_event::ResourceMutationResultDto>,
}

impl RunResult {
    pub fn result(&self, name: &str) -> Option<Arc<super::StoredResult>> {
        self.result_ids
            .get(name)
            .and_then(|result_id| self.results.result(*result_id))
    }

    #[cfg(test)]
    pub(crate) fn value_for_test(&self, name: &str) -> Option<RuntimeValue> {
        let result = self.result(name)?;
        match &result.state {
            super::ResultState::Ready(value) => Some(value.to_runtime_value()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    InvalidPlan(Box<str>),
    MemoizationRetry,
    Cancelled,
    ActivationIdExhausted,
    RuntimeIdExhausted,
    DeadlineExceeded {
        phase: RunPhase,
    },
    KernelNotFound(Box<str>),
    KernelFailed {
        operation: OperationIndex,
        kind: KernelErrorKind,
        message: Box<str>,
    },
    RelationalBackendNotFound(RelationalBackendId),
    RelationalAcquire {
        backend: RelationalBackendId,
        code: RelationalErrorCode,
        message: Box<str>,
    },
    RelationalFailed {
        operation: OperationIndex,
        code: RelationalErrorCode,
        message: Box<str>,
    },
    Stream(Box<str>),
    MissingValue(ValueRef),
    UpstreamResultFailed {
        source_result_id: super::ResultId,
        message: Box<str>,
    },
    UpstreamResultCancelled {
        source_result_id: super::ResultId,
    },
    InvalidCondition {
        value: ValueRef,
    },
    OutputCount {
        operation: OperationIndex,
        expected: usize,
        actual: usize,
    },
    OperationAlreadyExecuted {
        operation: OperationIndex,
        activation: ActivationId,
    },
    UnsatisfiedEffectDependency {
        operation: OperationIndex,
        required: OperationIndex,
    },
    LoopLimitExceeded {
        max_iterations: u64,
    },
    FunctionPlanNotFound(Box<str>),
    FunctionPlanFailed(Box<str>),
    RecursionLimitExceeded {
        recursion_limit: usize,
    },
    ProjectDraining(Box<str>),
    ResourceSnapshotMismatch(Box<str>),
    ResourceAcquire {
        resource: ResourceId,
        message: Box<str>,
    },
}

impl RunError {
    pub fn from_relational_acquire(backend: RelationalBackendId, error: RelationalError) -> Self {
        if error.code() == RelationalErrorCode::Cancelled {
            Self::Cancelled
        } else if error.code() == RelationalErrorCode::DeadlineExceeded {
            Self::DeadlineExceeded {
                phase: RunPhase::Kernel,
            }
        } else {
            Self::RelationalAcquire {
                backend,
                code: error.code(),
                message: error.message().into(),
            }
        }
    }

    pub fn from_relational(operation: OperationIndex, error: RelationalError) -> Self {
        if error.code() == RelationalErrorCode::Cancelled {
            Self::Cancelled
        } else if error.code() == RelationalErrorCode::DeadlineExceeded {
            Self::DeadlineExceeded {
                phase: RunPhase::Kernel,
            }
        } else {
            Self::RelationalFailed {
                operation,
                code: error.code(),
                message: error.message().into(),
            }
        }
    }
}

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlan(message) => write!(formatter, "invalid execution plan: {message}"),
            Self::MemoizationRetry => formatter.write_str("memoization flight must be retried"),
            Self::Cancelled => formatter.write_str("run was cancelled"),
            Self::ActivationIdExhausted => formatter.write_str("activation ID space is exhausted"),
            Self::RuntimeIdExhausted => formatter.write_str("runtime ID space is exhausted"),
            Self::DeadlineExceeded { phase } => {
                write!(formatter, "run deadline exceeded during {phase:?}")
            }
            Self::KernelNotFound(handle) => {
                write!(formatter, "kernel '{handle}' is not registered")
            }
            Self::KernelFailed {
                operation, message, ..
            } => write!(
                formatter,
                "operation {} failed: {message}",
                operation.index()
            ),
            Self::RelationalBackendNotFound(backend) => write!(
                formatter,
                "relational backend '{}' is not registered",
                backend.as_str()
            ),
            Self::RelationalAcquire {
                backend, message, ..
            } => write!(
                formatter,
                "failed to acquire relational backend '{}': {message}",
                backend.as_str()
            ),
            Self::RelationalFailed {
                operation, message, ..
            } => write!(
                formatter,
                "relational operation {} failed: {message}",
                operation.index()
            ),

            Self::Stream(message) => write!(formatter, "stream failed: {message}"),
            Self::MissingValue(value) => {
                write!(formatter, "runtime value {} is unavailable", value.index())
            }
            Self::UpstreamResultFailed {
                source_result_id,
                message,
            } => write!(
                formatter,
                "upstream result {} failed: {message}",
                source_result_id.get()
            ),
            Self::UpstreamResultCancelled { source_result_id } => write!(
                formatter,
                "upstream result {} was cancelled",
                source_result_id.get()
            ),
            Self::InvalidCondition { value } => write!(
                formatter,
                "condition value {} is not boolean",
                value.index()
            ),
            Self::OutputCount {
                operation,
                expected,
                actual,
            } => write!(
                formatter,
                "operation {} produced {actual} outputs; expected {expected}",
                operation.index()
            ),
            Self::OperationAlreadyExecuted {
                operation,
                activation,
            } => write!(
                formatter,
                "operation {} ran more than once in activation {}",
                operation.index(),
                activation.get()
            ),
            Self::UnsatisfiedEffectDependency {
                operation,
                required,
            } => write!(
                formatter,
                "operation {} ran before effect dependency {}",
                operation.index(),
                required.index()
            ),
            Self::LoopLimitExceeded { max_iterations } => write!(
                formatter,
                "loop exceeded its {max_iterations} iteration limit"
            ),
            Self::FunctionPlanNotFound(handle) => {
                write!(formatter, "function plan '{handle}' was not found")
            }
            Self::FunctionPlanFailed(message) => {
                write!(formatter, "function plan provider failed: {message}")
            }
            Self::RecursionLimitExceeded { recursion_limit } => write!(
                formatter,
                "call recursion exceeded its {recursion_limit} frame limit"
            ),
            Self::ProjectDraining(message) => write!(formatter, "project is draining: {message}"),
            Self::ResourceSnapshotMismatch(message) => {
                write!(
                    formatter,
                    "project resource snapshot does not match the plan: {message}"
                )
            }
            Self::ResourceAcquire { resource, message } => write!(
                formatter,
                "failed to acquire resource '{}': {message}",
                resource.as_str()
            ),
        }
    }
}

impl std::error::Error for RunError {}
