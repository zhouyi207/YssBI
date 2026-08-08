//! Synchronous, plan-only execution runtime.
//!
//! Runtime consumes immutable execution plans and plan-local handles. It never
//! queries the node registry or graph document and owns acquired resources only
//! for the lifetime of a run.

mod artifact;
mod builtin;
mod execution_event;
mod function_plan;
mod kernel;
mod kernels;
mod materialization;
mod memoization;
mod parameters;
mod production_relational;
mod project_resource;
mod project_run;
mod relational;
mod relational_dataframe;
mod resource;
mod result_store;
mod run;
mod scheduler;
mod stream;

pub use crate::node_system::analysis::RunId;
pub use artifact::{
    ArtifactDescriptor, ArtifactId, ArtifactPage, ArtifactSnapshot, ArtifactSnapshotKind,
    ArtifactStore,
};
pub use builtin::{
    BuiltinConstantParameters, BuiltinVariableParameters, build_builtin_kernel_registry,
};
#[cfg(test)]
pub(crate) use execution_event::RUN_EVENT_KIND_VARIANT_COUNT;
pub use execution_event::{
    NOOP_RUN_EVENT_SINK, NoopRunEventSink, RunErrorCode, RunEvent, RunEventKind, RunEventSink,
};
pub use function_plan::{
    FunctionPlanGeneration, FunctionPlanStore, FunctionPlanStoreError, PublishedFunctionPlan,
};
pub use kernel::{
    Kernel, KernelContext, KernelError, KernelErrorKind, KernelRegistrationError, KernelRegistry,
};
pub(crate) use kernels::dataframe_to_protocol_value_with_checkpoint;
pub use kernels::{
    ConvertParameters, ConvertTarget, DataframeKernelParameters, PlotKind, PlotPublishError,
    PlotSink, PlotSinkResource, StatisticsKernelParameters, dataframe_to_protocol_value,
};
pub use materialization::execute_planned_adapter;
#[cfg(test)]
pub(crate) use memoization::MemoCommitCheckpoint;
pub use memoization::{DemandFingerprint, OperationMemoKey, RunMemoization, ValueFingerprint};
pub use parameters::{
    CompiledParameterRegistrationError, CompiledParameterStore, CompiledParameterTypeError,
};
pub use production_relational::ProductionRelationalBackend;
#[cfg(test)]
pub(crate) use production_relational::{
    ProductionRelationalCheckpoint, ProductionRelationalObserver,
};
#[cfg(test)]
pub(crate) use project_resource::ProjectResourceLeaseObserver;
pub use project_resource::{
    ProjectDataFrameScan, ProjectDatabaseSnapshot, ProjectResourceLease, ProjectResourceProvider,
    ProjectResourceSnapshot, ProjectResourceValue, ProjectResourceVersionFingerprint,
    ProjectVariableAccess, VariableWriteEffect,
};
pub use project_run::{
    ProjectPreRunRegistration, ProjectRunDrainGuard, ProjectRunRegistration,
    ProjectRunRegistrationError, ProjectRunRegistry,
};
pub use relational::{
    RelationalBackend, RelationalBackendLease, RelationalBackendProvider,
    RelationalBackendRegistrationError, RelationalBackendRegistry, RelationalContext,
    RelationalError, RelationalErrorCode, RelationalExecution,
};
pub use resource::{
    ResourceError, ResourceErrorKind, ResourceLease, ResourceProvider, RunResourceSet,
};
pub(crate) use result_store::PendingResultSource;
pub use result_store::{ResultSourceDescriptor, ResultSourceId, ResultSourcePage, ResultStore};
pub use run::{
    ActivationId, Artifact, ArtifactKind, CancellationToken, FrameId, RunError, RunResult,
    RuntimeValue, StreamValue,
};
pub use scheduler::{FunctionPlanProvider, RunExecutor};
pub use stream::{
    BoundedStreamReceiver, BoundedStreamSender, InvalidStreamCapacity, StreamReceiveError,
    StreamSendError, bounded_stream_channel,
};

#[cfg(test)]
mod builtin_tests;
#[cfg(test)]
mod production_tests;
#[cfg(test)]
mod tests;
