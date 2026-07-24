use super::{BoundedStreamReceiver, StreamReceiveError, bounded_stream_channel};
use crate::node_system::analysis::{CompileProvenance, CorrelationContext, RunId};
use crate::node_system::plan::{
    OperationIndex, RelationalBackendId, RelationalFragmentId, ResourceId, ValueRef,
};
use crate::node_system::protocol::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Buffered,
    Collected,
    Spilled,
    Replayable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    kind: ArtifactKind,
    values: Box<[Value]>,
}

impl Artifact {
    pub fn new(kind: ArtifactKind, values: impl Into<Box<[Value]>>) -> Self {
        Self {
            kind,
            values: values.into(),
        }
    }

    pub const fn kind(&self) -> ArtifactKind {
        self.kind
    }

    pub fn values(&self) -> &[Value] {
        &self.values
    }
}

#[derive(Clone)]
pub struct StreamValue {
    receiver: BoundedStreamReceiver<Value>,
}

impl StreamValue {
    pub fn from_receiver(receiver: BoundedStreamReceiver<Value>) -> Self {
        Self { receiver }
    }

    pub fn from_values(
        values: impl IntoIterator<Item = Value>,
        cancellation: CancellationToken,
    ) -> Result<Self, RunError> {
        let values = values.into_iter().collect::<Vec<_>>();
        let capacity = values.len().max(1);
        let (sender, receiver) = bounded_stream_channel(capacity, cancellation)
            .map_err(|error| RunError::Stream(error.to_string().into()))?;
        for value in values {
            sender
                .send(value)
                .map_err(|_| RunError::Stream("stream closed while being initialized".into()))?;
        }
        sender.close();
        Ok(Self { receiver })
    }

    pub fn recv(&self) -> Result<Value, StreamReceiveError> {
        self.receiver.recv()
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

impl RuntimeValue {
    pub fn close_stream(&self) {
        if let Self::Stream(stream) = self {
            stream.close();
        }
    }
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

static NEXT_ACTIVATION_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_FRAME_ID: AtomicU64 = AtomicU64::new(1);

impl ActivationId {
    pub(crate) fn next() -> Self {
        Self(NEXT_ACTIVATION_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl FrameId {
    pub(crate) fn next() -> Self {
        Self(NEXT_FRAME_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug)]
struct CancellationState {
    cancelled: Arc<AtomicBool>,
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
        self.state.cancelled.store(true, Ordering::Release);
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

    pub(crate) fn shared_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.state.cancelled)
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
    pub correlation: CorrelationContext,
    pub values: BTreeMap<Box<str>, RuntimeValue>,
    pub committed_variable_ids: Box<[crate::variable::VariableId]>,
    pub resource_deltas: Vec<crate::node_system::document::ResourceDeltaEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    InvalidPlan(Box<str>),
    Cancelled,
    KernelNotFound(Box<str>),
    KernelFailed {
        operation: OperationIndex,
        message: Box<str>,
    },
    RelationalBackendNotFound(RelationalBackendId),
    RelationalAcquire {
        backend: RelationalBackendId,
        message: Box<str>,
    },
    RelationalFailed {
        operation: OperationIndex,
        message: Box<str>,
    },
    MissingRelationalFragment(RelationalFragmentId),
    BridgeFailed(Box<str>),
    Stream(Box<str>),
    MissingValue(ValueRef),
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

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlan(message) => write!(formatter, "invalid execution plan: {message}"),
            Self::Cancelled => formatter.write_str("run was cancelled"),
            Self::KernelNotFound(handle) => {
                write!(formatter, "kernel '{handle}' is not registered")
            }
            Self::KernelFailed { operation, message } => write!(
                formatter,
                "operation {} failed: {message}",
                operation.index()
            ),
            Self::RelationalBackendNotFound(backend) => write!(
                formatter,
                "relational backend '{}' is not registered",
                backend.as_str()
            ),
            Self::RelationalAcquire { backend, message } => write!(
                formatter,
                "failed to acquire relational backend '{}': {message}",
                backend.as_str()
            ),
            Self::RelationalFailed { operation, message } => write!(
                formatter,
                "relational operation {} failed: {message}",
                operation.index()
            ),
            Self::MissingRelationalFragment(fragment) => write!(
                formatter,
                "relational fragment '{}' has no runtime output",
                fragment.as_str()
            ),
            Self::BridgeFailed(message) => {
                write!(formatter, "materialization bridge failed: {message}")
            }
            Self::Stream(message) => write!(formatter, "stream failed: {message}"),
            Self::MissingValue(value) => {
                write!(formatter, "runtime value {} is unavailable", value.index())
            }
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
