use super::scheduler::SchedulerCheckpoint;
use super::*;
use crate::node_system::analysis::{
    CompilationBasis, CompileId, CompileProvenance, CorrelationContext, ProjectSessionId,
    ResourceKey, ResourceVersion, SYSTEM_TRACE_CLOCK, SpanGuard, SpanKind, SpanOutcome, SpanSpec,
    TraceSink, TraceSpan,
};
use crate::node_system::document::{
    FunctionParameterId, GraphResourcePath, GraphRevision, NodeId, PortAddress,
};
use crate::node_system::plan::*;
use crate::node_system::protocol::{
    CachePolicy, CanonicalDecimal, InputConsumption, NodeTypeId, OutputProduction, PortKey,
    RetryPolicy, TypeExpr, TypeId, Value,
};
use crate::node_system::registry::RegistryFingerprint;
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Condvar, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

fn id<T>(value: &str, constructor: impl FnOnce(Box<str>) -> Result<T, InvalidPlanId>) -> T {
    constructor(value.into()).unwrap()
}

fn stable_output(port_key: &str) -> GraphOutputRef {
    GraphOutputRef {
        graph_path: GraphResourcePath("events/test".into()),
        port: PortAddress::declared(
            NodeId::from_uuid(uuid::Uuid::nil()),
            PortKey::new(port_key).unwrap(),
        ),
    }
}

fn operation(kernel: &str, inputs: &[u32], outputs: &[u32]) -> PlannedOperation {
    PlannedOperation {
        stable_id: OperationStableId::new(format!("test.operation.{kernel}")).unwrap(),
        source_node_id: NodeId::from_uuid(uuid::Uuid::nil()),
        source_node_type_id: NodeTypeId::new(format!("yssbi.test.{kernel}")).unwrap(),
        kernel: PlannedKernel::Native(id(kernel, KernelHandle::new)),
        inputs: inputs
            .iter()
            .map(|value| PlannedInput {
                value: ValueRef::new(*value),
                contract: crate::node_system::plan::PlannedValueContract::opaque(),
                consumption: InputConsumption::FullyMaterialized,
                bound_value: None,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        outputs: outputs
            .iter()
            .map(|value| PlannedOutput {
                value: ValueRef::new(*value),
                contract: crate::node_system::plan::PlannedValueContract::opaque(),
                production: OutputProduction::FullyMaterialized,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        params: id("params", CompiledParameterHandle::new),
        resource_dependencies: Box::new([]),
        cache_policy: CachePolicy::Disabled,
        semantics_version: ExecutionSemanticsVersion::from_bytes([1; 32]),
        workload: WorkloadClass::Cpu,
        retry: PlannedRetry::default(),
    }
}

fn adapter_operation(
    stable: &str,
    input: u32,
    output: u32,
    production: OutputProduction,
    consumption: InputConsumption,
) -> PlannedOperation {
    let contract = MaterializationAdapterPlan::for_contract(production, consumption);
    PlannedOperation {
        stable_id: OperationStableId::new(stable).unwrap(),
        source_node_id: NodeId::from_uuid(uuid::Uuid::nil()),
        source_node_type_id: NodeTypeId::new("yssbi.test.materialization_adapter").unwrap(),
        kernel: PlannedKernel::Adapter(contract.adapter),
        inputs: Box::new([PlannedInput {
            value: ValueRef::new(input),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            consumption: contract.input_consumption,
            bound_value: None,
        }]),
        outputs: Box::new([PlannedOutput {
            value: ValueRef::new(output),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            production: contract.output_production,
        }]),
        params: id("adapter.test", CompiledParameterHandle::new),
        resource_dependencies: Box::new([]),
        cache_policy: CachePolicy::Disabled,
        semantics_version: ExecutionSemanticsVersion::from_bytes([6; 32]),
        workload: WorkloadClass::AdapterIo,
        retry: PlannedRetry::default(),
    }
}

fn publish_graph_results(plan: &mut ExecutionPlan) {
    plan.publications = plan
        .results
        .iter()
        .map(|result| PlannedPublication::GraphResult {
            name: result.name.clone(),
            output: result.output.clone(),
            value: result.value,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
}

fn plan(
    operations: Vec<PlannedOperation>,
    value_count: u32,
    root_region: StructuredControlRegion,
) -> ExecutionPlan {
    ExecutionPlan {
        provenance: CompileProvenance {
            project_session_id: ProjectSessionId::new("test-session"),
            graph_path: GraphResourcePath("events/test".into()),
            basis: CompilationBasis {
                graph_revision: GraphRevision::new(1),
                registry_fingerprint: RegistryFingerprint::from_bytes([1; 32]),
                resource_versions: BTreeMap::new(),
                resource_observations: BTreeMap::new(),
            },
            compile_id: CompileId::new(1),
        },
        value_count,
        value_contracts: (0..value_count)
            .map(|value| (ValueRef::new(value), PlannedValueContract::opaque()))
            .collect(),
        value_sources: Box::new([]),
        bound_values: BTreeMap::new(),
        operations: operations.into_boxed_slice(),
        value_dependencies: Box::new([]),
        root_region,
        effect_dependencies: Box::new([]),
        relational_subplans: Box::new([]),
        resources: Box::new([]),
        results: Box::new([]),
        publications: Box::new([]),
    }
}

struct FnKernel<F>(F);

struct OwnedStreamKernel {
    values: Box<[Value]>,
    executions: Option<Arc<AtomicUsize>>,
}

impl Kernel for OwnedStreamKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        if let Some(executions) = &self.executions {
            executions.fetch_add(1, Ordering::SeqCst);
        }
        let stream = context
            .resource_owner
            .stream_from_values(self.values.to_vec())
            .map_err(|error| KernelError::new(error.to_string()))?;
        Ok(vec![RuntimeValue::Stream(stream)])
    }
}

struct ErrorKernel {
    cancel_token: bool,
    cancelled_error: bool,
}

impl Kernel for ErrorKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        if self.cancel_token {
            context.cancellation.cancel();
        }
        Err(if self.cancelled_error {
            KernelError::cancelled("kernel cancelled")
        } else {
            KernelError::new("ordinary failure")
        })
    }
}

impl<F> Kernel for FnKernel<F>
where
    F: for<'a> Fn(&'a [RuntimeValue]) -> Result<Vec<RuntimeValue>, KernelError> + Send + Sync,
{
    fn execute(
        &self,
        _context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        (self.0)(inputs)
    }
}

#[derive(Default)]
struct RecordingTrace(Mutex<Vec<TraceSpan>>);

impl TraceSink for RecordingTrace {
    fn start_span(&self, spec: SpanSpec) -> SpanGuard<'_> {
        SpanGuard::new(self, spec, &SYSTEM_TRACE_CLOCK)
    }

    fn complete_span(&self, span: TraceSpan) {
        self.0.lock().unwrap().push(span);
    }
}

struct PanickingCompletionTrace;

impl TraceSink for PanickingCompletionTrace {
    fn start_span(&self, spec: SpanSpec) -> SpanGuard<'_> {
        SpanGuard::new(self, spec, &SYSTEM_TRACE_CLOCK)
    }

    fn complete_span(&self, _: TraceSpan) {
        panic!("trace completion sink failed")
    }
}

#[test]
fn trace_sink_completion_panic_does_not_replace_successful_run() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("trace_sink_success", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(1).into()])),
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("trace_sink_success", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    let result = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_trace_sink(&PanickingCompletionTrace)
        .run(&execution_plan, CancellationToken::new());

    assert!(result.is_ok());
}

#[derive(Default)]
struct RecordingRunEvents(Mutex<Vec<RunEvent>>);

impl RunEventSink for RecordingRunEvents {
    fn record(&self, event: RunEvent) {
        self.0.lock().unwrap().push(event);
    }
}

fn assert_cancelled_without_completion(events: &RecordingRunEvents) {
    let events = events.0.lock().unwrap();
    assert!(
        events
            .iter()
            .any(|event| event.kind == RunEventKind::RunCancelled)
    );
    assert!(
        events
            .iter()
            .all(|event| event.kind != RunEventKind::RunCompleted)
    );
    assert!(events.iter().all(|event| !matches!(
        event.kind,
        RunEventKind::ResultReady { .. } | RunEventKind::OutputReady { .. }
    )));
}

struct NoFunctions;

impl FunctionPlanProvider for NoFunctions {
    fn get_function(
        &self,
        _: &FunctionPlanHandle,
    ) -> Result<Option<Arc<PublishedFunctionPlan>>, Box<str>> {
        Ok(None)
    }
}

struct OneFunction(Arc<PublishedFunctionPlan>);

impl FunctionPlanProvider for OneFunction {
    fn get_function(
        &self,
        _: &FunctionPlanHandle,
    ) -> Result<Option<Arc<PublishedFunctionPlan>>, Box<str>> {
        Ok(Some(Arc::clone(&self.0)))
    }
}

fn published_function(
    mut plan: ExecutionPlan,
    target: &str,
    parameters: &[u32],
    results: &[u32],
) -> Arc<PublishedFunctionPlan> {
    plan.provenance.graph_path = GraphResourcePath(target.into());
    let provenance = plan.provenance.clone();
    let parameters: BTreeMap<FunctionParameterId, ValueRef> = parameters
        .iter()
        .enumerate()
        .map(|(index, value)| {
            (
                FunctionParameterId(format!("parameter-{index}").into()),
                ValueRef::new(*value),
            )
        })
        .collect();
    let results: BTreeMap<FunctionParameterId, ValueRef> = results
        .iter()
        .enumerate()
        .map(|(index, value)| {
            (
                FunctionParameterId(format!("result-{index}").into()),
                ValueRef::new(*value),
            )
        })
        .collect();
    Arc::new(PublishedFunctionPlan {
        plan: Arc::new(plan),
        abi: Arc::new(FunctionPlanAbi {
            provenance,
            parameter_contracts: parameters
                .keys()
                .cloned()
                .map(|parameter| (parameter, PlannedValueContract::opaque()))
                .collect(),
            parameters,
            result_productions: results
                .keys()
                .cloned()
                .map(|parameter| (parameter, OutputProduction::FullyMaterialized))
                .collect(),
            result_contracts: results
                .keys()
                .cloned()
                .map(|parameter| (parameter, PlannedValueContract::opaque()))
                .collect(),
            results,
        }),
    })
}

struct TrackingResources {
    acquired: Arc<AtomicUsize>,
    released: Arc<AtomicUsize>,
    fail_at: Option<usize>,
}

struct TrackingLease {
    resource: ResourceId,
    released: Arc<AtomicUsize>,
}

impl Drop for TrackingLease {
    fn drop(&mut self) {
        self.released.fetch_add(1, Ordering::SeqCst);
    }
}

impl ResourceLease for TrackingLease {
    fn resource_id(&self) -> &ResourceId {
        &self.resource
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ResourceProvider for TrackingResources {
    fn acquire(
        &self,
        requirement: &CompiledResourceRequirement,
    ) -> Result<Box<dyn ResourceLease>, ResourceError> {
        let attempt = self.acquired.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_at == Some(attempt) {
            return Err(ResourceError::new("acquire failed"));
        }
        Ok(Box::new(TrackingLease {
            resource: requirement.resource.clone(),
            released: self.released.clone(),
        }))
    }
}

fn no_resources() -> TrackingResources {
    TrackingResources {
        acquired: Arc::new(AtomicUsize::new(0)),
        released: Arc::new(AtomicUsize::new(0)),
        fail_at: None,
    }
}

fn materialization_test_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("yssbi-task-13-{label}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn materialization_test_budgets(stream_capacity: usize, memory_bytes: u64) -> RunResourceBudgets {
    RunResourceBudgets {
        stream_capacity: std::num::NonZeroUsize::new(stream_capacity).unwrap(),
        materialization_memory_bytes: memory_bytes,
        spill_directory_bytes: 1024 * 1024,
    }
}

fn materialization_test_owner() -> Arc<RunResourceOwner> {
    Arc::new(
        RunResourceOwner::new(
            RunId::new(99),
            RunResourceBudgets::default(),
            CancellationToken::new(),
        )
        .unwrap(),
    )
}

fn decimal(value: &str) -> Value {
    Value::Decimal(CanonicalDecimal::new(value).unwrap())
}

fn data_series_metadata(
    element_type: DataSeriesElementType,
    length: usize,
    null_count: usize,
) -> DataSeriesMetadata {
    DataSeriesMetadata {
        element_type,
        length,
        null_count,
        name: Some("x".into()),
        format: Some("test-format".into()),
    }
}

#[test]
fn data_series_constructor_validates_metadata_and_storage_contract() {
    let length_error = Artifact::new_data_series(
        ArtifactKind::Collected,
        data_series_metadata(DataSeriesElementType::Int64, 2, 0),
        [Value::Integer(1)],
    )
    .unwrap_err();
    assert_eq!(
        length_error.to_string(),
        "DataSeries metadata length 2 does not match 1 values"
    );

    let null_error = Artifact::new_data_series(
        ArtifactKind::Collected,
        data_series_metadata(DataSeriesElementType::Int64, 1, 0),
        [Value::Null],
    )
    .unwrap_err();
    assert_eq!(
        null_error.to_string(),
        "DataSeries metadata null count 0 does not match 1 nulls"
    );

    let type_error = Artifact::new_data_series(
        ArtifactKind::Collected,
        data_series_metadata(DataSeriesElementType::Int64, 1, 0),
        [decimal("1")],
    )
    .unwrap_err();
    assert_eq!(
        type_error.to_string(),
        "DataSeries Int64 element at index 0 has incompatible Decimal storage"
    );
}

#[test]
fn data_series_artifact_preserves_metadata_through_spill_and_replay() {
    let root = materialization_test_root("data-series-replay");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(104),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let metadata = data_series_metadata(DataSeriesElementType::Float64, 3, 1);
    let series = Artifact::new_data_series(
        ArtifactKind::Collected,
        metadata.clone(),
        [decimal("1"), Value::Null, decimal("3")],
    )
    .unwrap();

    let spilled = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Artifact(series),
        &owner,
        &cancellation,
    )
    .unwrap();
    let replayed =
        execute_planned_adapter(&PlannedAdapter::Replay, spilled, &owner, &cancellation).unwrap();
    let RuntimeValue::Artifact(artifact) = replayed else {
        panic!("replay must produce an artifact");
    };

    assert_eq!(artifact.kind(), ArtifactKind::Replayable);
    assert_eq!(artifact.value_kind(), ArtifactValueKind::DataSeries);
    assert_eq!(artifact.data_series_metadata(), Some(&metadata));
    assert_eq!(
        artifact
            .cursor()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        [decimal("1"), Value::Null, decimal("3")]
    );

    drop(artifact);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn scalar_list_is_rejected_as_data_series() {
    let value = RuntimeValue::Scalar(Value::List(vec![Value::Integer(1)]));

    assert_eq!(
        require_data_series(&value).unwrap_err().message(),
        "expected DataSeries Artifact, received scalar"
    );
}

#[test]
fn scalar_object_is_rejected_as_data_series() {
    let value = RuntimeValue::Scalar(Value::Object(BTreeMap::new()));

    assert_eq!(
        require_data_series(&value).unwrap_err().message(),
        "expected DataSeries Artifact, received scalar"
    );
}

#[test]
fn typed_data_series_readers_preserve_numeric_kind_and_apply_null_policy() {
    let integers = Artifact::new_data_series(
        ArtifactKind::Collected,
        data_series_metadata(DataSeriesElementType::Int64, 3, 1),
        [Value::Integer(1), Value::Null, Value::Integer(3)],
    )
    .unwrap();
    let floats = DataSeriesBuilder::new(DataSeriesElementType::Float64)
        .name("float values")
        .format("0.00")
        .values([decimal("1.5"), Value::Null, decimal("2.5")])
        .build(ArtifactKind::Collected)
        .unwrap();

    let NumericSeriesView::Int64(propagated) =
        numeric_series(&integers, NullPolicy::Propagate).unwrap()
    else {
        panic!("Int64 metadata must produce an Int64 view");
    };
    assert_eq!(propagated.values(), &[Some(1), None, Some(3)]);
    let NumericSeriesView::Int64(skipped) = numeric_series(&integers, NullPolicy::Skip).unwrap()
    else {
        panic!("Int64 metadata must produce an Int64 view");
    };
    assert_eq!(skipped.values(), &[Some(1), Some(3)]);
    assert_eq!(
        numeric_series(&integers, NullPolicy::Reject)
            .unwrap_err()
            .message(),
        "DataSeries contains null at index 1"
    );

    let NumericSeriesView::Float64(floats) =
        numeric_series(&floats, NullPolicy::Propagate).unwrap()
    else {
        panic!("Float64 metadata must produce a Float64 view");
    };
    assert_eq!(floats.values(), &[Some(1.5), None, Some(2.5)]);
    assert_eq!(floats.metadata().name.as_deref(), Some("float values"));
    assert_eq!(floats.metadata().format.as_deref(), Some("0.00"));
}

#[test]
fn string_and_boolean_readers_are_typed_and_reject_wrong_metadata() {
    let strings = Artifact::new_data_series(
        ArtifactKind::Collected,
        data_series_metadata(DataSeriesElementType::String, 2, 1),
        [Value::String("a".into()), Value::Null],
    )
    .unwrap();
    let booleans = Artifact::new_data_series(
        ArtifactKind::Collected,
        data_series_metadata(DataSeriesElementType::Boolean, 2, 1),
        [Value::Bool(true), Value::Null],
    )
    .unwrap();

    assert_eq!(
        string_series(&strings, NullPolicy::Propagate)
            .unwrap()
            .values(),
        &[Some(Box::<str>::from("a")), None]
    );
    assert_eq!(
        boolean_series(&booleans, NullPolicy::Skip)
            .unwrap()
            .values(),
        &[Some(true)]
    );
    assert_eq!(
        boolean_series(&strings, NullPolicy::Propagate)
            .unwrap_err()
            .message(),
        "expected Boolean DataSeries, received String"
    );
}

#[test]
fn checked_int64_float_promotion_rejects_inexact_values() {
    assert_eq!(
        checked_int64_to_f64(9_007_199_254_740_992).unwrap(),
        9_007_199_254_740_992.0
    );
    assert_eq!(
        checked_int64_to_f64(9_007_199_254_740_993)
            .unwrap_err()
            .message(),
        "Int64 value 9007199254740993 cannot be represented exactly as Float64"
    );
}

fn requirement(name: &str) -> CompiledResourceRequirement {
    CompiledResourceRequirement {
        resource: id(name, ResourceId::new),
        kind: ResourceKind::TemporaryStorage,
        access: ResourceAccess::Exclusive,
        optional: false,
    }
}

#[test]
fn bounded_materialization_capacity_one_applies_backpressure() {
    let root = materialization_test_root("backpressure");
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(1),
        materialization_test_budgets(1, 1024),
        CancellationToken::new(),
        root.clone(),
    )
    .unwrap();
    let (observed_tx, observed_rx) = mpsc::channel();
    let values = (0..3).map(move |value| {
        observed_tx.send(value).unwrap();
        Value::Integer(value)
    });

    let stream = owner.stream_from_values(values).unwrap();

    assert_eq!(observed_rx.recv_timeout(Duration::from_secs(1)).unwrap(), 0);
    assert_eq!(observed_rx.recv_timeout(Duration::from_secs(1)).unwrap(), 1);
    assert!(observed_rx.recv_timeout(Duration::from_millis(50)).is_err());
    assert_eq!(stream.recv().unwrap(), Value::Integer(0));
    assert_eq!(observed_rx.recv_timeout(Duration::from_secs(1)).unwrap(), 2);
    assert_eq!(stream.recv().unwrap(), Value::Integer(1));
    assert_eq!(stream.recv().unwrap(), Value::Integer(2));
    assert_eq!(stream.recv(), Err(StreamReceiveError::Closed));

    drop(owner);
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_producer_panic_is_not_partial_success() {
    let root = materialization_test_root("producer-panic");
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(5),
        materialization_test_budgets(1, 1024),
        CancellationToken::new(),
        root.clone(),
    )
    .unwrap();
    let stream = owner
        .stream_from_values((0..2).map(|value| {
            if value == 1 {
                panic!("producer iterator panic sentinel");
            }
            Value::Integer(value)
        }))
        .unwrap();

    assert_eq!(stream.recv().unwrap(), Value::Integer(0));
    assert!(matches!(
        stream.recv(),
        Err(StreamReceiveError::Failed(message))
            if message.as_ref() == "stream producer panicked"
    ));

    drop(stream);
    drop(owner);
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn spill_memory_threshold_preserves_stable_disk_order() {
    let root = materialization_test_root("spill-order");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(2),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let input = RuntimeValue::Artifact(Artifact::new(
        ArtifactKind::Collected,
        [
            Value::Integer(3),
            Value::String("two".into()),
            Value::Bool(true),
        ],
    ));

    let output = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 10,
                max_bytes: 1024,
            },
        },
        input,
        &owner,
        &cancellation,
    )
    .unwrap();

    let RuntimeValue::Artifact(artifact) = output else {
        panic!("collect must produce an artifact");
    };
    assert!(matches!(
        artifact.materialized(),
        MaterializedArtifact::Spilled(_)
    ));
    assert_eq!(
        artifact
            .cursor()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        [
            Value::Integer(3),
            Value::String("two".into()),
            Value::Bool(true)
        ]
    );
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);

    drop(artifact);
    drop(owner);
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn spilled_artifact_exposes_only_explicit_non_panicking_consumption() {
    let root = materialization_test_root("explicit-artifact-consumption");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(13),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let spilled = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Scalar(Value::Integer(7)),
        &owner,
        &cancellation,
    )
    .unwrap();
    let RuntimeValue::Artifact(artifact) = spilled else {
        panic!("spill adapter must return an artifact");
    };

    assert_eq!(artifact.in_memory_values(), None);
    assert_eq!(
        artifact
            .cursor()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        [Value::Integer(7)]
    );

    drop(artifact);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn spill_replay_supports_two_independent_passes() {
    let root = materialization_test_root("replay");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(3),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let spilled = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Artifact(Artifact::new(
            ArtifactKind::Buffered,
            [Value::Integer(1), Value::Integer(2), Value::Integer(3)],
        )),
        &owner,
        &cancellation,
    )
    .unwrap();
    let replayable =
        execute_planned_adapter(&PlannedAdapter::Replay, spilled, &owner, &cancellation).unwrap();
    let RuntimeValue::Artifact(artifact) = replayable else {
        panic!("replay must produce an artifact");
    };
    let MaterializedArtifact::Replayable(replay) = artifact.materialized() else {
        panic!("replay adapter must produce replayable storage");
    };

    let first = replay.cursor().unwrap();
    let second = replay.cursor().unwrap();
    assert_eq!(
        first.collect::<Result<Vec<_>, _>>().unwrap(),
        [Value::Integer(1), Value::Integer(2), Value::Integer(3),]
    );
    assert_eq!(
        second.collect::<Result<Vec<_>, _>>().unwrap(),
        [Value::Integer(1), Value::Integer(2), Value::Integer(3),]
    );

    drop(artifact);
    drop(owner);
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

fn disk_backed_result_plan(terminal_kernel: &str) -> ExecutionPlan {
    let mut source = operation("disk_result_source", &[], &[0]);
    source.outputs[0].production = OutputProduction::Streaming;
    let collect = adapter_operation(
        "disk.result.collect",
        1,
        2,
        OutputProduction::Streaming,
        InputConsumption::FullyMaterialized,
    );
    let terminal = operation(terminal_kernel, &[3], &[4]);
    let mut execution_plan = plan(
        vec![source, collect, terminal],
        5,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
            ControlStep::Operation(OperationIndex::new(2)),
        ])),
    );
    execution_plan.value_dependencies = Box::new([
        ValueDependency {
            source: ValueRef::new(0),
            destination: ValueRef::new(1),
        },
        ValueDependency {
            source: ValueRef::new(2),
            destination: ValueRef::new(3),
        },
    ]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(4),
    }]);
    publish_graph_results(&mut execution_plan);
    execution_plan
}

#[test]
fn bounded_materialization_run_result_and_result_store_keep_spill_durable() {
    let root = materialization_test_root("durable-result");
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("disk_result_source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::String("durable".into()).into()])),
        )
        .unwrap();
    kernels
        .register(
            id("disk_result_passthrough", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| Ok(vec![inputs[0].clone()])),
        )
        .unwrap();
    let results = ResultStore::new();
    let events = RecordingRunEvents::default();
    let resources = no_resources();
    let functions = NoFunctions;

    let run_result = RunExecutor::new(&kernels, &resources, &functions)
        .with_result_store(&results)
        .with_event_sink(&events)
        .with_resource_budgets(materialization_test_budgets(1, 1))
        .with_test_spill_root(root.clone())
        .run(
            &disk_backed_result_plan("disk_result_passthrough"),
            CancellationToken::new(),
        )
        .unwrap();

    let RuntimeValue::Artifact(artifact) = &run_result.values["result"] else {
        panic!("collected result must be an artifact");
    };
    assert_eq!(
        artifact
            .cursor()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        [Value::String("durable".into())]
    );
    let source_id = events
        .0
        .lock()
        .unwrap()
        .iter()
        .find_map(|event| match event.kind {
            RunEventKind::ResultReady { source_id, .. } => Some(source_id),
            _ => None,
        })
        .expect("published result source");
    assert_eq!(
        results
            .page(source_id, 0, 10)
            .unwrap()
            .unwrap()
            .values
            .as_ref(),
        &[Value::String("durable".into())]
    );
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn result_store_paging_propagates_spill_read_failures() {
    let root = materialization_test_root("result-page-error");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(14),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let spilled = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Scalar(Value::Integer(9)),
        &owner,
        &cancellation,
    )
    .unwrap();
    let execution_plan = plan(
        Vec::new(),
        0,
        StructuredControlRegion::Sequence(Box::new([])),
    );
    let run_id = RunId::new(14);
    let correlation = CorrelationContext::compile(&execution_plan.provenance).for_run(run_id, None);
    let results = ResultStore::new();
    let descriptor = results
        .publish_runtime_value(
            run_id,
            correlation,
            execution_plan.provenance.basis.clone(),
            "spilled",
            &spilled,
        )
        .unwrap();
    assert!(owner.cleanup().is_empty());

    assert!(matches!(
        results.page(descriptor.source_id, 0, 1),
        Err(RunError::Stream(message)) if message.contains("spill I/O failed")
    ));

    drop(spilled);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_debug_view_consumes_spilled_collect() {
    let root = materialization_test_root("debug-consumer");
    let mut kernels = build_builtin_kernel_registry();
    kernels
        .register(
            id("disk_result_source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(41).into()])),
        )
        .unwrap();
    let execution_plan = disk_backed_result_plan("yssbi.debug.view");

    let run_result = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_resource_budgets(materialization_test_budgets(1, 1))
        .with_test_spill_root(root.clone())
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    let RuntimeValue::Artifact(artifact) = &run_result.values["result"] else {
        panic!("view result must be an artifact");
    };
    assert_eq!(
        artifact
            .cursor()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        [Value::Integer(41)]
    );
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_relational_ingress_consumes_spilled_single_value() {
    let root = materialization_test_root("relational-consumer");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(6),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let dataframe = Value::Object(BTreeMap::from([(
        Box::<str>::from("value"),
        Value::List(vec![Value::Integer(1), Value::Integer(2)]),
    )]));
    let spilled = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 10,
                max_bytes: 1024 * 1024,
            },
        },
        RuntimeValue::Artifact(Artifact::new(ArtifactKind::Buffered, [dataframe])),
        &owner,
        &cancellation,
    )
    .unwrap();

    let converted = super::relational_dataframe::tabular_runtime_to_dataframe(spilled).unwrap();

    assert_eq!(converted.height(), 2);
    assert_eq!(converted.width(), 1);
    drop(owner);
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_memory_exact_boundary_and_drop_release() {
    let value = Value::String("exact".into());
    let bytes = serde_json::to_vec(&value).unwrap().len() as u64;
    let root = materialization_test_root("memory-exact");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(7),
        materialization_test_budgets(1, bytes),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();

    let artifact = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 1,
                max_bytes: bytes,
            },
        },
        RuntimeValue::Scalar(value),
        &owner,
        &cancellation,
    )
    .unwrap();

    assert!(matches!(
        artifact,
        RuntimeValue::Artifact(ref artifact)
            if matches!(artifact.materialized(), MaterializedArtifact::InMemory(_))
    ));
    assert_eq!(owner.memory_bytes_for_test(), bytes);
    drop(artifact);
    assert_eq!(owner.memory_bytes_for_test(), 0);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_clone_shares_values_and_one_live_reservation() {
    let value = Value::String("shared-clone".into());
    let bytes = serde_json::to_vec(&value).unwrap().len() as u64;
    let root = materialization_test_root("memory-clone-sharing");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(16),
        materialization_test_budgets(1, bytes),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let RuntimeValue::Artifact(artifact) = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 1,
                max_bytes: bytes,
            },
        },
        RuntimeValue::Scalar(value),
        &owner,
        &cancellation,
    )
    .unwrap() else {
        panic!("collect must return an artifact");
    };

    let cloned = artifact.clone();
    assert!(std::ptr::eq(
        artifact.in_memory_values().unwrap().as_ptr(),
        cloned.in_memory_values().unwrap().as_ptr(),
    ));
    assert_eq!(owner.memory_bytes_for_test(), bytes);
    drop(artifact);
    assert_eq!(owner.memory_bytes_for_test(), bytes);
    drop(cloned);
    assert_eq!(owner.memory_bytes_for_test(), 0);

    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_result_store_retains_shared_artifact_storage() {
    let value = Value::String("shared-result".into());
    let bytes = serde_json::to_vec(&value).unwrap().len() as u64;
    let root = materialization_test_root("memory-result-sharing");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(20),
        materialization_test_budgets(1, bytes),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let RuntimeValue::Artifact(artifact) = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 1,
                max_bytes: bytes,
            },
        },
        RuntimeValue::Scalar(value),
        &owner,
        &cancellation,
    )
    .unwrap() else {
        panic!("collect must return an artifact");
    };
    let run_id = RunId::new(20);
    let execution_plan = plan(
        Vec::new(),
        0,
        StructuredControlRegion::Sequence(Box::new([])),
    );
    let correlation = CorrelationContext::compile(&execution_plan.provenance).for_run(run_id, None);
    let results = ResultStore::new();
    let descriptor = results
        .publish_runtime_value(
            run_id,
            correlation,
            execution_plan.provenance.basis.clone(),
            "shared",
            &RuntimeValue::Artifact(artifact.clone()),
        )
        .unwrap();
    let snapshot = results.value(descriptor.source_id).unwrap();
    let ArtifactSnapshot::RuntimeArtifact(stored) = snapshot.as_ref() else {
        panic!("runtime artifact snapshots must retain shared storage");
    };

    assert!(std::ptr::eq(
        artifact.in_memory_values().unwrap().as_ptr(),
        stored.in_memory_values().unwrap().as_ptr(),
    ));
    drop(artifact);
    assert_eq!(owner.memory_bytes_for_test(), bytes);
    drop(snapshot);
    results.cleanup_run(run_id);
    assert!(results.release(descriptor.source_id));
    assert_eq!(owner.memory_bytes_for_test(), 0);

    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_adapter_consumption_retains_source_reservation() {
    let value = Value::String("adapter-handoff".into());
    let bytes = serde_json::to_vec(&value).unwrap().len() as u64;
    let root = materialization_test_root("memory-adapter-handoff");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(17),
        materialization_test_budgets(1, bytes),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let collect = PlannedAdapter::Collect {
        limits: MaterializationLimits {
            max_values: 1,
            max_bytes: bytes,
        },
    };
    let first =
        execute_planned_adapter(&collect, RuntimeValue::Scalar(value), &owner, &cancellation)
            .unwrap();
    assert_eq!(owner.memory_bytes_for_test(), bytes);

    let second = execute_planned_adapter(&collect, first, &owner, &cancellation).unwrap();

    assert!(matches!(
        second,
        RuntimeValue::Artifact(ref artifact)
            if matches!(artifact.materialized(), MaterializedArtifact::Spilled(_))
    ));
    assert_eq!(owner.memory_bytes_for_test(), 0);
    drop(second);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_aggregate_memory_spills_then_reuses_released_capacity() {
    let value = Value::String("aggregate".into());
    let bytes = serde_json::to_vec(&value).unwrap().len() as u64;
    let root = materialization_test_root("memory-aggregate");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(8),
        materialization_test_budgets(1, bytes),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let collect = |value| {
        execute_planned_adapter(
            &PlannedAdapter::Collect {
                limits: MaterializationLimits {
                    max_values: 1,
                    max_bytes: bytes,
                },
            },
            RuntimeValue::Scalar(value),
            &owner,
            &cancellation,
        )
        .unwrap()
    };

    let first = collect(value.clone());
    let second = collect(value.clone());
    assert!(matches!(
        second,
        RuntimeValue::Artifact(ref artifact)
            if matches!(artifact.materialized(), MaterializedArtifact::Spilled(_))
    ));
    assert_eq!(owner.memory_bytes_for_test(), bytes);
    drop(first);
    assert_eq!(owner.memory_bytes_for_test(), 0);
    let third = collect(value);
    assert!(matches!(
        third,
        RuntimeValue::Artifact(ref artifact)
            if matches!(artifact.materialized(), MaterializedArtifact::InMemory(_))
    ));
    drop(second);
    drop(third);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_single_oversized_value_spills_without_reserving_memory() {
    let oversized = Value::Bytes(vec![7; 4096]);
    let small = Value::Bool(true);
    let small_bytes = serde_json::to_vec(&small).unwrap().len() as u64;
    let root = materialization_test_root("memory-oversized");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(15),
        materialization_test_budgets(1, small_bytes),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();

    let oversized = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 1,
                max_bytes: 64 * 1024,
            },
        },
        RuntimeValue::Scalar(oversized),
        &owner,
        &cancellation,
    )
    .unwrap();
    assert!(matches!(
        oversized,
        RuntimeValue::Artifact(ref artifact)
            if matches!(artifact.materialized(), MaterializedArtifact::Spilled(_))
    ));
    assert_eq!(owner.memory_bytes_for_test(), 0);

    let subsequent = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 1,
                max_bytes: small_bytes,
            },
        },
        RuntimeValue::Scalar(small),
        &owner,
        &cancellation,
    )
    .unwrap();
    assert!(matches!(
        subsequent,
        RuntimeValue::Artifact(ref artifact)
            if matches!(artifact.materialized(), MaterializedArtifact::InMemory(_))
    ));
    assert_eq!(owner.memory_bytes_for_test(), small_bytes);

    drop(oversized);
    drop(subsequent);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_limit_failure_rolls_back_memory_for_next_allocation() {
    let value = Value::String("rollback".into());
    let bytes = serde_json::to_vec(&value).unwrap().len() as u64;
    let root = materialization_test_root("memory-rollback");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(9),
        materialization_test_budgets(1, bytes),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();

    let failed = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 0,
                max_bytes: bytes,
            },
        },
        RuntimeValue::Scalar(value.clone()),
        &owner,
        &cancellation,
    );
    assert!(failed.is_err());
    assert_eq!(owner.memory_bytes_for_test(), 0);

    let next = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 1,
                max_bytes: bytes,
            },
        },
        RuntimeValue::Scalar(value),
        &owner,
        &cancellation,
    )
    .unwrap();
    assert!(matches!(
        next,
        RuntimeValue::Artifact(ref artifact)
            if matches!(artifact.materialized(), MaterializedArtifact::InMemory(_))
    ));
    drop(next);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn spill_typed_fidelity_covers_all_value_variants_and_nested_data() {
    use crate::node_system::protocol::CanonicalDecimal;

    let values = vec![
        Value::Null,
        Value::Bool(true),
        Value::Integer(-7),
        Value::Unsigned(9),
        Value::Decimal(CanonicalDecimal::new("12.34").unwrap()),
        Value::String("text".into()),
        Value::Bytes(vec![0, 1, 255]),
        Value::List(vec![Value::Integer(1), Value::String("nested".into())]),
        Value::Object(BTreeMap::from([(
            Box::<str>::from("nested"),
            Value::List(vec![Value::Bool(false), Value::Null]),
        )])),
    ];
    let root = materialization_test_root("typed-fidelity");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(10),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();

    let spilled = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Artifact(Artifact::new(ArtifactKind::Buffered, values.clone())),
        &owner,
        &cancellation,
    )
    .unwrap();
    let RuntimeValue::Artifact(artifact) = spilled else {
        panic!("spill adapter must return an artifact");
    };
    assert_eq!(
        artifact
            .cursor()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        values
    );
    drop(artifact);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn spill_cursor_keeps_promoted_file_alive_until_cursor_drop() {
    let root = materialization_test_root("spill-cursor-lifetime");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(18),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let RuntimeValue::Artifact(artifact) = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Scalar(Value::Integer(1)),
        &owner,
        &cancellation,
    )
    .unwrap() else {
        panic!("spill adapter must return an artifact");
    };
    artifact.promote(&cancellation, None).unwrap();
    let MaterializedArtifact::Spilled(spill) = artifact.materialized() else {
        panic!("spill adapter must use spill storage");
    };
    let path = spill.path_for_test();
    let ArtifactCursor::Spilled(cursor) = artifact.cursor().unwrap() else {
        panic!("spill artifact must return a spill cursor");
    };

    drop(artifact);
    assert!(path.exists());
    drop(cursor);
    assert!(!path.exists());

    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn deadline_during_spill_promotion_publishes_no_durable_artifact() {
    let root = materialization_test_root("spill-promotion-deadline");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(20),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let RuntimeValue::Artifact(artifact) = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Scalar(Value::Integer(1)),
        &owner,
        &cancellation,
    )
    .unwrap() else {
        panic!("spill adapter must return an artifact");
    };
    let MaterializedArtifact::Spilled(spill) = artifact.materialized() else {
        panic!("spill adapter must use spill storage");
    };
    let staged_path = spill.path_for_test();
    spill.set_promotion_checkpoint_for_test(Arc::new(|| {
        thread::sleep(Duration::from_millis(20));
    }));

    assert_eq!(
        artifact.promote(
            &cancellation,
            Some(RunDeadline::after(Duration::from_millis(5))),
        ),
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::ResultPublication,
        })
    );
    assert_eq!(spill.path_for_test(), staged_path);
    assert!(!staged_path.exists());

    drop(artifact);
    drop(owner);
    let _ = std::fs::remove_dir(root);
}

#[test]
fn spill_cursor_close_surfaces_and_retries_transient_delete_failure() {
    let root = materialization_test_root("spill-cursor-delete-retry");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(19),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let RuntimeValue::Artifact(artifact) = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Scalar(Value::Integer(2)),
        &owner,
        &cancellation,
    )
    .unwrap() else {
        panic!("spill adapter must return an artifact");
    };
    artifact.promote(&cancellation, None).unwrap();
    let MaterializedArtifact::Spilled(spill) = artifact.materialized() else {
        panic!("spill adapter must use spill storage");
    };
    spill.fail_next_deletions_for_test(1);
    let path = spill.path_for_test();
    let ArtifactCursor::Spilled(mut cursor) = artifact.cursor().unwrap() else {
        panic!("spill artifact must return a spill cursor");
    };
    drop(artifact);

    let error = cursor.close().unwrap_err();

    assert!(
        error
            .to_string()
            .contains("injected spill deletion failure")
    );
    assert!(path.exists());
    cursor.close().unwrap();
    assert!(!path.exists());
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn spill_quota_exact_boundary_and_failure_rollback_allow_subsequent_write() {
    let first = Value::String("first".into());
    let second = Value::String("second".into());
    let first_bytes = 8 + serde_json::to_vec(&first).unwrap().len() as u64;
    let second_bytes = 8 + serde_json::to_vec(&second).unwrap().len() as u64;
    let root = materialization_test_root("spill-quota");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(11),
        RunResourceBudgets {
            stream_capacity: std::num::NonZeroUsize::new(1).unwrap(),
            materialization_memory_bytes: 1,
            spill_directory_bytes: first_bytes,
        },
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();

    let failed = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Artifact(Artifact::new(
            ArtifactKind::Buffered,
            [first.clone(), second],
        )),
        &owner,
        &cancellation,
    );
    assert!(failed.is_err());
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());

    let exact = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Scalar(first),
        &owner,
        &cancellation,
    )
    .unwrap();
    assert!(matches!(exact, RuntimeValue::Artifact(_)));
    assert_eq!(owner.spill_bytes_for_test(), first_bytes);
    assert!(second_bytes > 0);
    drop(exact);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_producer_registration_is_linearized_with_cleanup() {
    let root = materialization_test_root("producer-race");
    let owner = Arc::new(
        RunResourceOwner::with_spill_root(
            RunId::new(12),
            materialization_test_budgets(1, 1024),
            CancellationToken::new(),
            root.clone(),
        )
        .unwrap(),
    );
    let registration_reached = Arc::new(Barrier::new(2));
    let release_registration = Arc::new(Barrier::new(2));
    let producer = {
        let owner = Arc::clone(&owner);
        let registration_reached = Arc::clone(&registration_reached);
        let release_registration = Arc::clone(&release_registration);
        thread::spawn(move || {
            owner.stream_from_values_with_registration_checkpoint([Value::Integer(1)], move || {
                registration_reached.wait();
                release_registration.wait();
            })
        })
    };
    registration_reached.wait();
    let (cleanup_done_tx, cleanup_done_rx) = mpsc::channel();
    let cleanup = {
        let owner = Arc::clone(&owner);
        thread::spawn(move || {
            let errors = owner.cleanup();
            cleanup_done_tx.send(errors).unwrap();
        })
    };
    assert!(
        cleanup_done_rx
            .recv_timeout(Duration::from_millis(50))
            .is_err()
    );
    release_registration.wait();
    let stream = producer.join().unwrap().unwrap();
    drop(stream);
    assert!(
        cleanup_done_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .is_empty()
    );
    cleanup.join().unwrap();
    assert!(owner.stream_from_values([Value::Integer(2)]).is_err());
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

struct KernelOwnedStreamSource;

impl Kernel for KernelOwnedStreamSource {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        Ok(vec![RuntimeValue::Stream(
            context
                .resource_owner
                .stream_from_values([Value::Integer(1), Value::Integer(2)])
                .map_err(|error| KernelError::new(error.to_string()))?,
        )])
    }
}

#[test]
fn bounded_materialization_kernel_streams_use_the_scheduler_owner() {
    let root = materialization_test_root("kernel-owner");
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("kernel_owned_stream_source", KernelHandle::new),
            KernelOwnedStreamSource,
        )
        .unwrap();
    kernels
        .register(
            id("kernel_owned_stream_sink", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| {
                let RuntimeValue::Artifact(artifact) = &inputs[0] else {
                    return Err(KernelError::new("expected collected stream"));
                };
                Ok(vec![
                    Value::Integer(artifact.cursor().unwrap().count() as i64).into(),
                ])
            }),
        )
        .unwrap();
    let mut source = operation("kernel_owned_stream_source", &[], &[0]);
    source.outputs[0].production = OutputProduction::Streaming;
    let collect = adapter_operation(
        "kernel.owner.collect",
        1,
        2,
        OutputProduction::Streaming,
        InputConsumption::FullyMaterialized,
    );
    let sink = operation("kernel_owned_stream_sink", &[3], &[4]);
    let mut execution_plan = plan(
        vec![source, collect, sink],
        5,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
            ControlStep::Operation(OperationIndex::new(2)),
        ])),
    );
    execution_plan.value_dependencies = Box::new([
        ValueDependency {
            source: ValueRef::new(0),
            destination: ValueRef::new(1),
        },
        ValueDependency {
            source: ValueRef::new(2),
            destination: ValueRef::new(3),
        },
    ]);
    execution_plan.results = Box::new([PlanResult {
        name: "count".into(),
        output: stable_output("count"),
        value: ValueRef::new(4),
    }]);
    publish_graph_results(&mut execution_plan);

    let result = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_resource_budgets(materialization_test_budgets(1, 1024))
        .with_test_spill_root(root.clone())
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(result.values["count"], Value::Integer(2).into());
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_unowned_stream_constructor_is_not_public() {
    let source = include_str!("run.rs");
    assert!(!source.contains("pub fn from_receiver("));
}

#[test]
fn bounded_materialization_has_no_unbounded_read_all_api() {
    assert!(!include_str!("run.rs").contains("pub fn read_all("));
    assert!(!include_str!("spill.rs").contains("pub fn read_all("));
}

struct PanicWithCleanupFailureKernel;

impl Kernel for PanicWithCleanupFailureKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        context
            .resource_owner
            .register_panicking_cleanup_task_for_test();
        panic!("primary kernel panic sentinel")
    }
}

#[test]
fn bounded_materialization_panic_cleanup_errors_are_traced_without_replacing_panic() {
    let root = materialization_test_root("panic-trace");
    let trace = RecordingTrace::default();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("panic_with_cleanup_failure", KernelHandle::new),
            PanicWithCleanupFailureKernel,
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("panic_with_cleanup_failure", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    let resources = no_resources();
    let functions = NoFunctions;
    let executor = RunExecutor::new(&kernels, &resources, &functions)
        .with_trace_sink(&trace)
        .with_test_spill_root(root.clone());

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = executor.run(&execution_plan, CancellationToken::new());
    }));

    assert!(panic.is_err());
    let cleanup = trace
        .0
        .lock()
        .unwrap()
        .iter()
        .find(|span| span.kind == SpanKind::Cleanup)
        .cloned()
        .expect("panic cleanup trace");
    assert_eq!(
        cleanup.outcome,
        SpanOutcome::Cleanup {
            error_count: 1,
            panicking: true,
        }
    );
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn spill_artifacts_never_enter_per_run_memoization() {
    let root = materialization_test_root("spill-memo");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(4),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let spilled = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Artifact(Artifact::new(
            ArtifactKind::Buffered,
            [Value::Integer(1), Value::Integer(2)],
        )),
        &owner,
        &cancellation,
    )
    .unwrap();
    assert!(
        OperationMemoKey::from_inputs(
            OperationStableId::new("events/test::spilled-input").unwrap(),
            std::slice::from_ref(&spilled),
            BTreeMap::new(),
            ExecutionSemanticsVersion::from_bytes([7; 32]),
            DemandFingerprint::from_bytes([9; 32]),
        )
        .is_none()
    );

    let memo = RunMemoization::new();
    let key = per_run_memo_key(&[Value::Integer(1).into()], "1");
    let calls = AtomicUsize::new(0);
    for _ in 0..2 {
        let output = spilled.clone();
        memo.get_or_produce(key.clone(), &CancellationToken::new(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(vec![output].into_boxed_slice())
        })
        .unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    drop(memo);
    drop(spilled);
    drop(owner);
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

fn spilling_terminal_plan(terminal: PlannedOperation) -> ExecutionPlan {
    let mut source = operation("spill_terminal_source", &[], &[0]);
    source.outputs[0].production = OutputProduction::Streaming;
    let collect = adapter_operation(
        "spill.terminal.collect",
        1,
        2,
        OutputProduction::Streaming,
        InputConsumption::FullyMaterialized,
    );
    let mut execution_plan = plan(
        vec![source, collect, terminal],
        5,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
            ControlStep::Operation(OperationIndex::new(2)),
        ])),
    );
    execution_plan.value_dependencies = Box::new([
        ValueDependency {
            source: ValueRef::new(0),
            destination: ValueRef::new(1),
        },
        ValueDependency {
            source: ValueRef::new(2),
            destination: ValueRef::new(3),
        },
    ]);
    execution_plan
}

fn spilling_terminal_kernels(terminal: impl Kernel + 'static) -> KernelRegistry {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("spill_terminal_source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::String("spill me".into()).into()])),
        )
        .unwrap();
    kernels
        .register(id("spill_terminal", KernelHandle::new), terminal)
        .unwrap();
    kernels
}

fn spilling_terminal_operation() -> PlannedOperation {
    operation("spill_terminal", &[3], &[4])
}

#[test]
fn bounded_materialization_cleanup_covers_success_error_cancel_and_deadline() {
    enum Terminal {
        Success,
        Error,
        Cancel,
        Deadline,
    }

    for terminal in [
        Terminal::Success,
        Terminal::Error,
        Terminal::Cancel,
        Terminal::Deadline,
    ] {
        let root = materialization_test_root("terminal");
        let (kernels, expected, deadline) = match terminal {
            Terminal::Success => (
                spilling_terminal_kernels(FnKernel(|_: &[RuntimeValue]| {
                    Ok(vec![Value::Integer(1).into()])
                })),
                None,
                false,
            ),
            Terminal::Deadline => (
                spilling_terminal_kernels(FnKernel(|_: &[RuntimeValue]| {
                    Ok(vec![Value::Integer(1).into()])
                })),
                Some(RunErrorCode::Cancelled),
                true,
            ),
            Terminal::Error => (
                spilling_terminal_kernels(ErrorKernel {
                    cancel_token: false,
                    cancelled_error: false,
                }),
                Some(RunErrorCode::KernelFailed),
                false,
            ),
            Terminal::Cancel => (
                spilling_terminal_kernels(ErrorKernel {
                    cancel_token: true,
                    cancelled_error: true,
                }),
                Some(RunErrorCode::Cancelled),
                false,
            ),
        };
        let resources = no_resources();
        let functions = NoFunctions;
        let mut executor = RunExecutor::new(&kernels, &resources, &functions)
            .with_resource_budgets(materialization_test_budgets(1, 1))
            .with_test_spill_root(root.clone());
        if deadline {
            executor = executor.with_test_checkpoint(Arc::new(|checkpoint, cancellation| {
                if checkpoint == SchedulerCheckpoint::FinalResultPublication {
                    cancellation.cancel();
                }
            }));
        }
        let execution_plan = spilling_terminal_plan(spilling_terminal_operation());
        assert!(
            execution_plan.validate().is_ok(),
            "{:?}",
            execution_plan.validate()
        );
        let result = executor.run(&execution_plan, CancellationToken::new());
        match expected {
            None => assert!(result.is_ok(), "{result:?}"),
            Some(code) => assert_eq!(RunErrorCode::from(&result.unwrap_err()), code),
        }
        assert!(std::fs::read_dir(&root).unwrap().next().is_none());
        std::fs::remove_dir(root).unwrap();
    }
}

#[test]
fn bounded_materialization_cleanup_runs_during_panic_unwind() {
    let root = materialization_test_root("panic");
    let kernels = spilling_terminal_kernels(FnKernel(|_: &[RuntimeValue]| {
        panic!("spill terminal panic sentinel")
    }));
    let resources = no_resources();
    let functions = NoFunctions;
    let executor = RunExecutor::new(&kernels, &resources, &functions)
        .with_resource_budgets(materialization_test_budgets(1, 1))
        .with_test_spill_root(root.clone());

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = executor.run(
            &spilling_terminal_plan(spilling_terminal_operation()),
            CancellationToken::new(),
        );
    }));

    assert!(result.is_err());
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

struct ProjectReplacementKernel {
    started: mpsc::Sender<()>,
}

impl Kernel for ProjectReplacementKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        self.started.send(()).unwrap();
        while !context.cancellation.is_cancelled() {
            thread::yield_now();
        }
        Err(KernelError::cancelled("project replacement cancelled run"))
    }
}

#[test]
fn bounded_materialization_cleanup_precedes_project_replacement_drain_completion() {
    let root = materialization_test_root("replacement");
    let registry = Arc::new(ProjectRunRegistry::new());
    let (started_tx, started_rx) = mpsc::channel();
    let kernels = spilling_terminal_kernels(ProjectReplacementKernel {
        started: started_tx,
    });
    let execution_plan = spilling_terminal_plan(spilling_terminal_operation());
    let project = execution_plan.provenance.project_session_id.clone();
    let run_registry = Arc::clone(&registry);
    let run_root = root.clone();
    let run = thread::spawn(move || {
        RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
            .with_run_registry(&run_registry)
            .with_resource_budgets(materialization_test_budgets(1, 1))
            .with_test_spill_root(run_root)
            .run(&execution_plan, CancellationToken::new())
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();

    registry.cancel_and_drain(&project);

    assert_eq!(run.join().unwrap(), Err(RunError::Cancelled));
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bound_input_operation_executes_downstream_and_publishes_result_without_fallback() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("bound_source", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| Ok(vec![inputs[0].clone()])),
        )
        .unwrap();
    kernels
        .register(
            id("increment", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| {
                let RuntimeValue::Scalar(Value::Integer(value)) = inputs[0] else {
                    return Err(KernelError::new("expected integer"));
                };
                Ok(vec![Value::Integer(value + 1).into()])
            }),
        )
        .unwrap();
    let mut source = operation("bound_source", &[0], &[1]);
    source.inputs[0].bound_value = Some(Value::Integer(7));
    let mut execution_plan = plan(
        vec![source, operation("increment", &[1], &[2])],
        3,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(2),
    }]);
    publish_graph_results(&mut execution_plan);

    let result = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(result.values["result"], Value::Integer(8).into());
}

#[test]
fn bound_input_blocked_by_effect_dependency_reports_effect_error() {
    let mut blocked = operation("blocked", &[0], &[]);
    blocked.inputs[0].bound_value = Some(Value::Integer(7));
    let mut execution_plan = plan(
        vec![operation("required", &[], &[]), blocked],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            1,
        ))])),
    );
    execution_plan.effect_dependencies = Box::new([EffectDependency {
        before: OperationIndex::new(0),
        after: OperationIndex::new(1),
    }]);

    let error = RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(
        error,
        RunError::UnsatisfiedEffectDependency {
            operation,
            required,
        } if operation == OperationIndex::new(1) && required == OperationIndex::new(0)
    ));
}

#[test]
fn bound_input_blocked_by_value_dependency_reports_dependency_source() {
    let mut blocked = operation("blocked", &[0], &[1]);
    blocked.inputs[0].bound_value = Some(Value::Integer(7));
    let mut execution_plan = plan(
        vec![blocked],
        3,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.value_sources = Box::new([PlanValueSource::ExternalInput(
        ValueRef::new(2),
        OutputProduction::FullyMaterialized,
    )]);
    execution_plan.value_dependencies = Box::new([ValueDependency {
        source: ValueRef::new(2),
        destination: ValueRef::new(1),
    }]);

    let error = RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, RunError::MissingValue(value) if value == ValueRef::new(2)));
}

#[test]
fn truly_missing_operation_input_still_reports_missing_value() {
    let mut execution_plan = plan(
        vec![operation("blocked", &[0], &[])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.value_sources = Box::new([PlanValueSource::ExternalInput(
        ValueRef::new(0),
        OutputProduction::FullyMaterialized,
    )]);

    let error = RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, RunError::MissingValue(value) if value == ValueRef::new(0)));
}

#[test]
fn runtime_admission_rejects_sequence_artifact_for_data_series_contract() {
    let series_contract = PlannedValueContract {
        kind: PlannedValueKind::DataSeries,
        type_expr: crate::node_system::protocol::data_series_type(TypeExpr::Concrete(
            TypeId::new("core.int64").unwrap(),
        )),
    };
    let downstream_executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("sequence_artifact_source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| {
                Ok(vec![RuntimeValue::Artifact(Artifact::new(
                    ArtifactKind::Collected,
                    [Value::Integer(1)],
                ))])
            }),
        )
        .unwrap();
    let observed = Arc::clone(&downstream_executed);
    kernels
        .register(
            id("data_series_sink", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.store(true, Ordering::SeqCst);
                Ok(Vec::new())
            }),
        )
        .unwrap();
    let mut source = operation("sequence_artifact_source", &[], &[0]);
    source.outputs[0].contract = series_contract.clone();
    source.outputs[0].production = OutputProduction::Streaming;
    let mut adapter = adapter_operation(
        "data_series_identity",
        1,
        2,
        OutputProduction::Streaming,
        InputConsumption::FullyMaterialized,
    );
    adapter.inputs[0].contract = series_contract.clone();
    adapter.outputs[0].contract = series_contract.clone();
    let mut sink = operation("data_series_sink", &[3], &[]);
    sink.inputs[0].contract = series_contract.clone();
    let mut execution_plan = plan(
        vec![source, adapter, sink],
        4,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    execution_plan.value_contracts = BTreeMap::from([
        (ValueRef::new(0), series_contract.clone()),
        (ValueRef::new(1), series_contract.clone()),
        (ValueRef::new(2), series_contract.clone()),
        (ValueRef::new(3), series_contract),
    ]);
    execution_plan.value_dependencies = Box::new([
        ValueDependency {
            source: ValueRef::new(0),
            destination: ValueRef::new(1),
        },
        ValueDependency {
            source: ValueRef::new(2),
            destination: ValueRef::new(3),
        },
    ]);

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(
        matches!(&error, RunError::InvalidPlan(message) if message.contains("DataSeries Artifact")),
        "unexpected admission error: {error:?}"
    );
    assert!(!downstream_executed.load(Ordering::SeqCst));
}

#[test]
fn executes_sequence_deterministically() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut kernels = KernelRegistry::new();
    for (name, number) in [("first", 1_i64), ("second", 2)] {
        let events = events.clone();
        kernels
            .register(
                id(name, KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| {
                    events.lock().unwrap().push(number);
                    Ok(vec![Value::Integer(number).into()])
                }),
            )
            .unwrap();
    }
    let mut execution_plan = plan(
        vec![
            operation("first", &[], &[0]),
            operation("second", &[0], &[1]),
        ],
        2,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    execution_plan.value_dependencies = Box::new([ValueDependency {
        source: ValueRef::new(0),
        destination: ValueRef::new(1),
    }]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(1),
    }]);
    publish_graph_results(&mut execution_plan);

    let result = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(*events.lock().unwrap(), vec![1, 2]);
    assert_eq!(
        result.values["result"],
        RuntimeValue::from(Value::Integer(2))
    );
}

#[test]
fn demand_driven_publication_exposes_only_the_requested_final_output() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(3).into()])),
        )
        .unwrap();
    kernels
        .register(
            id("target", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| {
                let RuntimeValue::Scalar(Value::Integer(value)) = &inputs[0] else {
                    panic!("expected integer input")
                };
                Ok(vec![Value::Integer(value + 4).into()])
            }),
        )
        .unwrap();
    let output = stable_output("final");
    let mut execution_plan = plan(
        vec![
            operation("source", &[], &[0]),
            operation("target", &[0], &[2]),
        ],
        3,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "final".into(),
        output: output.clone(),
        value: ValueRef::new(2),
    }]);
    execution_plan.publications = Box::new([PlannedPublication::GraphResult {
        name: "final".into(),
        output: output.clone(),
        value: ValueRef::new(2),
    }]);
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();

    let run = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_event_sink(&events)
        .with_result_store(&results)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(run.values["final"], RuntimeValue::from(Value::Integer(7)));
    assert_eq!(
        results.source_count(),
        1,
        "intermediate values must not be readable sources"
    );
    let recorded = events.0.lock().unwrap();
    assert!(
        recorded
            .iter()
            .all(|event| { serde_json::to_value(&event.kind).unwrap()["type"] != "valueReady" })
    );
    assert_eq!(
        recorded
            .iter()
            .filter(|event| matches!(event.kind, RunEventKind::ResultReady { .. }))
            .count(),
        1
    );
    assert_eq!(recorded.iter().filter(|event| matches!(&event.kind, RunEventKind::OutputReady { output: emitted, .. } if emitted == &output)).count(), 1);
}

#[test]
fn demand_driven_publication_pin_preview_emits_only_generation_bound_output() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("preview", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(7).into()])),
        )
        .unwrap();
    let output = stable_output("preview");
    let mut execution_plan = plan(
        vec![operation("preview", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "requested.preview".into(),
        output: output.clone(),
        value: ValueRef::new(0),
    }]);
    execution_plan.publications = Box::new([PlannedPublication::PinPreview {
        output: output.clone(),
        generation: 17,
        value: ValueRef::new(0),
    }]);
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();

    RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_event_sink(&events)
        .with_result_store(&results)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(results.source_count(), 1);
    let recorded = events.0.lock().unwrap();
    assert!(
        recorded
            .iter()
            .all(|event| !matches!(event.kind, RunEventKind::ResultReady { .. }))
    );
    assert_eq!(
        recorded
            .iter()
            .filter(|event| matches!(
                &event.kind,
                RunEventKind::OutputReady {
                    output: emitted,
                    generation: Some(17),
                    ..
                } if emitted == &output
            ))
            .count(),
        1,
    );
}

#[test]
fn invalid_publication_returns_typed_invalid_plan_without_panicking() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("invalid_publication", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(7).into()])),
        )
        .unwrap();
    let output = stable_output("result");
    let mut execution_plan = plan(
        vec![operation("invalid_publication", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: output.clone(),
        value: ValueRef::new(0),
    }]);
    execution_plan.publications = Box::new([PlannedPublication::GraphResult {
        name: "missing-result".into(),
        output,
        value: ValueRef::new(0),
    }]);
    let results = ResultStore::new();

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_result_store(&results)
        .run(&execution_plan, CancellationToken::new())
        .expect_err("invalid publication must be rejected before execution");

    assert!(matches!(error, RunError::InvalidPlan(_)));
    assert_eq!(results.source_count(), 0);
}

#[test]
fn missing_publications_return_typed_invalid_plan_before_execution() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("missing_publication", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(7).into()])),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("missing_publication", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(0),
    }]);
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_event_sink(&events)
        .with_result_store(&results)
        .run(&execution_plan, CancellationToken::new())
        .expect_err("results without publications must be rejected before execution");

    assert!(matches!(error, RunError::InvalidPlan(_)));
    assert_eq!(results.source_count(), 0);
    assert!(events.0.lock().unwrap().iter().all(|event| !matches!(
        event.kind,
        RunEventKind::ResultReady { .. } | RunEventKind::OutputReady { .. }
    )));
}

#[test]
fn stable_output_ready_is_published_before_completion() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("value", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(7).into()])),
        )
        .unwrap();
    let output = stable_output("value");
    let mut execution_plan = plan(
        vec![operation("value", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "value".into(),
        output: output.clone(),
        value: ValueRef::new(0),
    }]);
    execution_plan.publications = Box::new([PlannedPublication::GraphResult {
        name: "value".into(),
        output: output.clone(),
        value: ValueRef::new(0),
    }]);
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();

    RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_event_sink(&events)
        .with_result_store(&results)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    let recorded = events.0.lock().unwrap();
    let output_index = recorded
        .iter()
        .position(|event| {
            matches!(
                &event.kind,
                RunEventKind::OutputReady {
                    output: emitted,
                    ..
                } if emitted == &output
            )
        })
        .expect("stable output event must be published");
    let completion_index = recorded
        .iter()
        .position(|event| event.kind == RunEventKind::RunCompleted)
        .expect("run completion must be published");
    assert!(output_index < completion_index);
}

#[test]
fn if_uses_a_plan_bound_condition_value() {
    let counts = Arc::new(Mutex::new(BTreeMap::<&'static str, usize>::new()));
    let mut kernels = KernelRegistry::new();
    for name in ["then", "else"] {
        let counts = counts.clone();
        kernels
            .register(
                id(name, KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| {
                    *counts.lock().unwrap().entry(name).or_default() += 1;
                    Ok(Vec::new())
                }),
            )
            .unwrap();
    }
    let mut execution_plan = plan(
        vec![operation("then", &[], &[]), operation("else", &[], &[])],
        1,
        StructuredControlRegion::If {
            condition: ValueRef::new(0),
            then_region: Box::new(StructuredControlRegion::Sequence(Box::new([
                ControlStep::Operation(OperationIndex::new(0)),
            ]))),
            else_region: Box::new(StructuredControlRegion::Sequence(Box::new([
                ControlStep::Operation(OperationIndex::new(1)),
            ]))),
            results: Box::new([]),
        },
    );
    execution_plan
        .bound_values
        .insert(ValueRef::new(0), Value::Bool(true));

    RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(counts.lock().unwrap().get("then"), Some(&1));
    assert_eq!(counts.lock().unwrap().get("else"), None);
}

#[test]
fn if_executes_only_selected_branch() {
    let counts = Arc::new(Mutex::new(BTreeMap::<&'static str, usize>::new()));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("condition", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Bool(true).into()])),
        )
        .unwrap();
    for name in ["then", "else"] {
        let counts = counts.clone();
        kernels
            .register(
                id(name, KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| {
                    *counts.lock().unwrap().entry(name).or_default() += 1;
                    Ok(vec![Value::String(name.into()).into()])
                }),
            )
            .unwrap();
    }
    let mut execution_plan = plan(
        vec![
            operation("condition", &[], &[0]),
            operation("then", &[], &[1]),
            operation("else", &[], &[2]),
        ],
        4,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(StructuredControlRegion::If {
                condition: ValueRef::new(0),
                then_region: Box::new(StructuredControlRegion::Sequence(Box::new([
                    ControlStep::Operation(OperationIndex::new(1)),
                ]))),
                else_region: Box::new(StructuredControlRegion::Sequence(Box::new([
                    ControlStep::Operation(OperationIndex::new(2)),
                ]))),
                results: Box::new([BranchResultBinding {
                    destination: ValueRef::new(3),
                    then_source: ValueRef::new(1),
                    else_source: ValueRef::new(2),
                    production: Some(OutputProduction::FullyMaterialized),
                }]),
            })),
        ])),
    );
    execution_plan.value_sources = Box::new([PlanValueSource::ControlProduced(
        ValueRef::new(3),
        OutputProduction::FullyMaterialized,
    )]);

    RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(counts.lock().unwrap().get("then"), Some(&1));
    assert_eq!(counts.lock().unwrap().get("else"), None);
}

fn execute_nested_branch_sequence_switch(
    first_matches: bool,
    second_matches: bool,
) -> (RunResult, Vec<&'static str>) {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut kernels = KernelRegistry::new();
    for (name, selected) in [
        ("first_condition", first_matches),
        ("second_condition", second_matches),
    ] {
        kernels
            .register(
                id(name, KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| Ok(vec![Value::Bool(selected).into()])),
            )
            .unwrap();
    }
    for (name, value) in [("first_case", 10_i64), ("second_case", 20), ("default", 30)] {
        let observed = Arc::clone(&observed);
        kernels
            .register(
                id(name, KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| {
                    observed.lock().unwrap().push(name);
                    Ok(vec![Value::Integer(value).into()])
                }),
            )
            .unwrap();
    }

    let inner_switch = StructuredControlRegion::If {
        condition: ValueRef::new(2),
        then_region: Box::new(StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(3)),
        ]))),
        else_region: Box::new(StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(4)),
        ]))),
        results: Box::new([BranchResultBinding {
            destination: ValueRef::new(5),
            then_source: ValueRef::new(3),
            else_source: ValueRef::new(4),
            production: Some(OutputProduction::FullyMaterialized),
        }]),
    };
    let outer_switch = StructuredControlRegion::If {
        condition: ValueRef::new(0),
        then_region: Box::new(StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(1)),
        ]))),
        else_region: Box::new(StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(2)),
            ControlStep::Region(Box::new(inner_switch)),
        ]))),
        results: Box::new([BranchResultBinding {
            destination: ValueRef::new(6),
            then_source: ValueRef::new(1),
            else_source: ValueRef::new(5),
            production: Some(OutputProduction::FullyMaterialized),
        }]),
    };
    let mut execution_plan = plan(
        vec![
            operation("first_condition", &[], &[0]),
            operation("first_case", &[], &[1]),
            operation("second_condition", &[], &[2]),
            operation("second_case", &[], &[3]),
            operation("default", &[], &[4]),
        ],
        7,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(outer_switch)),
        ])),
    );
    execution_plan.value_sources = Box::new([
        PlanValueSource::ControlProduced(ValueRef::new(5), OutputProduction::FullyMaterialized),
        PlanValueSource::ControlProduced(ValueRef::new(6), OutputProduction::FullyMaterialized),
    ]);
    execution_plan.results = Box::new([PlanResult {
        name: "selected".into(),
        output: stable_output("selected"),
        value: ValueRef::new(6),
    }]);
    publish_graph_results(&mut execution_plan);

    let result = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();
    let observed = observed.lock().unwrap().clone();
    (result, observed)
}

#[test]
fn nested_sibling_regions_produce_complete_data_exactly_once() {
    let (result, observed) = execute_nested_branch_sequence_switch(true, true);

    assert_eq!(observed, vec!["first_case"]);
    assert_eq!(result.values["selected"], Value::Integer(10).into());
}

#[test]
fn nested_branch_sequence_switch_executes_only_n_way_match() {
    let (result, observed) = execute_nested_branch_sequence_switch(false, true);

    assert_eq!(observed, vec!["second_case"]);
    assert_eq!(result.values["selected"], Value::Integer(20).into());
}

#[test]
fn nested_branch_sequence_switch_executes_default_when_no_case_matches() {
    let (result, observed) = execute_nested_branch_sequence_switch(false, false);

    assert_eq!(observed, vec!["default"]);
    assert_eq!(result.values["selected"], Value::Integer(30).into());
}

#[test]
fn cancellation_stops_run_and_releases_resources() {
    let token = CancellationToken::new();
    let kernel_token = token.clone();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("cancel", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                kernel_token.cancel();
                Ok(vec![Value::Null.into()])
            }),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("cancel", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.resources = Box::new([requirement("temporary")]);
    let resources = no_resources();
    let released = resources.released.clone();

    let error = RunExecutor::new(&kernels, &resources, &NoFunctions)
        .run(&execution_plan, token)
        .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
    assert_eq!(released.load(Ordering::SeqCst), 1);
}

#[test]
fn cancelled_kernel_error_maps_to_run_cancelled() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("cancelled_error", KernelHandle::new),
            ErrorKernel {
                cancel_token: false,
                cancelled_error: true,
            },
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("cancelled_error", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
}

#[test]
fn cancellation_before_ordinary_outcome_wins() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("ordinary_error", KernelHandle::new),
            ErrorKernel {
                cancel_token: true,
                cancelled_error: false,
            },
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("ordinary_error", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
}

#[test]
fn simultaneous_or_later_ordinary_outcome_cannot_replace_cancellation() {
    let cancellation = Instant::now();
    let before = cancellation.checked_sub(Duration::from_nanos(1)).unwrap();
    let after = cancellation.checked_add(Duration::from_nanos(1)).unwrap();

    assert!(super::scheduler::ordinary_error_precedes_cancellation_at(
        true,
        before,
        Some(cancellation),
    ));
    assert!(!super::scheduler::ordinary_error_precedes_cancellation_at(
        true,
        cancellation,
        Some(cancellation),
    ));
    assert!(!super::scheduler::ordinary_error_precedes_cancellation_at(
        true,
        after,
        Some(cancellation),
    ));
}

#[test]
fn ordinary_outcome_produced_before_cancellation_is_preserved() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("ordinary_before_cancel", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Err(KernelError::new("ordinary first"))),
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("ordinary_before_cancel", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_test_checkpoint(Arc::new(|checkpoint, cancellation| {
            if checkpoint == SchedulerCheckpoint::WorkerOutcomeProduced {
                cancellation.cancel();
            }
        }))
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(
        error,
        RunError::KernelFailed { message, .. } if message.as_ref() == "ordinary first"
    ));
}

#[test]
fn successful_run_releases_all_resources() {
    let resources = no_resources();
    let released = resources.released.clone();
    let mut execution_plan = plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([])));
    execution_plan.resources = Box::new([requirement("one"), requirement("two")]);

    RunExecutor::new(&KernelRegistry::new(), &resources, &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(released.load(Ordering::SeqCst), 2);
}

#[test]
fn acquire_failure_releases_previously_acquired_resources() {
    let trace = RecordingTrace::default();
    let released = Arc::new(AtomicUsize::new(0));
    let resources = TrackingResources {
        acquired: Arc::new(AtomicUsize::new(0)),
        released: released.clone(),
        fail_at: Some(2),
    };
    let mut execution_plan = plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([])));
    execution_plan.resources = Box::new([requirement("one"), requirement("two")]);

    let error = RunExecutor::new(&KernelRegistry::new(), &resources, &NoFunctions)
        .with_trace_sink(&trace)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, RunError::ResourceAcquire { .. }));
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert_run_phase_coverage(
        &trace.0.lock().unwrap(),
        SpanOutcome::Error,
        SpanOutcome::NotReached,
    );
}

#[test]
fn run_result_keeps_the_plan_basis_and_compile_id() {
    let execution_plan = plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([])));

    let result = RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(result.provenance, execution_plan.provenance);
    assert_eq!(
        result.correlation.compile_id,
        execution_plan.provenance.compile_id
    );
    assert_eq!(
        result.correlation.graph_revision,
        execution_plan.provenance.basis.graph_revision
    );
    assert_eq!(result.correlation.run_id, Some(result.run_id));
}

#[test]
fn cleanup_spans_cover_success_failure_and_cancellation() {
    let trace = RecordingTrace::default();
    let resources = no_resources();
    let kernels = KernelRegistry::new();
    let functions = NoFunctions;
    let executor = RunExecutor::new(&kernels, &resources, &functions).with_trace_sink(&trace);
    let valid = plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([])));
    executor.run(&valid, CancellationToken::new()).unwrap();

    let invalid = plan(
        vec![operation("missing", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    assert!(matches!(
        executor.run(&invalid, CancellationToken::new()),
        Err(RunError::KernelNotFound(_))
    ));

    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert_eq!(executor.run(&valid, cancelled), Err(RunError::Cancelled));

    let cleanup = trace
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|span| span.kind == SpanKind::Cleanup && span.correlation.parent_call.is_none())
        .map(|span| span.outcome.clone())
        .collect::<Vec<_>>();
    assert_eq!(cleanup.len(), 3);
    assert!(cleanup.iter().all(|outcome| matches!(
        outcome,
        SpanOutcome::Cleanup {
            error_count: 0,
            panicking: false,
        }
    )));
}

fn assert_run_phase_coverage(
    spans: &[TraceSpan],
    resource_outcome: SpanOutcome,
    publication_outcome: SpanOutcome,
) {
    let run = spans
        .iter()
        .find(|span| span.kind == SpanKind::Run)
        .unwrap();
    let phase = |kind| {
        let matches = spans
            .iter()
            .filter(|span| span.kind == kind)
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "expected exactly one {kind:?} span");
        assert_eq!(matches[0].parent_span_id, Some(run.span_id));
        matches[0]
    };
    let resource = phase(SpanKind::ResourceAcquire);
    let publication = phase(SpanKind::ResultPublication);
    let cleanup = phase(SpanKind::Cleanup);
    assert_eq!(resource.outcome, resource_outcome);
    assert_eq!(publication.outcome, publication_outcome);
    assert!(matches!(cleanup.outcome, SpanOutcome::Cleanup { .. }));
    assert!(resource.started_at <= publication.started_at);
    assert!(publication.started_at <= cleanup.started_at);
    for span in spans.iter().filter(|span| span.span_id != run.span_id) {
        assert!(span.started_at >= run.started_at);
        assert!(span.finished_at <= run.finished_at);
    }
}

#[test]
fn run_phase_spans_cover_success_error_cancellation_deadline_retry_exhaustion_and_panic() {
    let success_trace = RecordingTrace::default();
    RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
        .with_trace_sink(&success_trace)
        .with_cleanup_delay_for_test(Duration::from_millis(100))
        .run(
            &plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([]))),
            CancellationToken::new(),
        )
        .unwrap();
    assert_run_phase_coverage(
        &success_trace.0.lock().unwrap(),
        SpanOutcome::Success,
        SpanOutcome::Success,
    );
    let success_cleanup = success_trace
        .0
        .lock()
        .unwrap()
        .iter()
        .find(|span| span.kind == SpanKind::Cleanup)
        .unwrap()
        .clone();
    assert!(
        success_cleanup.finished_at.get() - success_cleanup.started_at.get() >= 50_000_000,
        "cleanup span must include the registered cleanup delay"
    );

    let error_trace = RecordingTrace::default();
    let error_plan = plan(
        vec![operation("missing_phase_kernel", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    assert!(
        RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
            .with_trace_sink(&error_trace)
            .run(&error_plan, CancellationToken::new())
            .is_err()
    );
    assert_run_phase_coverage(
        &error_trace.0.lock().unwrap(),
        SpanOutcome::Success,
        SpanOutcome::NotReached,
    );

    let cancelled_trace = RecordingTrace::default();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert_eq!(
        RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
            .with_trace_sink(&cancelled_trace)
            .run(
                &plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([]))),
                cancelled,
            ),
        Err(RunError::Cancelled)
    );
    assert_run_phase_coverage(
        &cancelled_trace.0.lock().unwrap(),
        SpanOutcome::NotReached,
        SpanOutcome::NotReached,
    );

    let deadline_trace = RecordingTrace::default();
    assert!(matches!(
        RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
            .with_trace_sink(&deadline_trace)
            .with_deadline(RunDeadline::after(Duration::ZERO))
            .run(
                &plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([]))),
                CancellationToken::new(),
            ),
        Err(RunError::DeadlineExceeded { .. })
    ));
    assert_run_phase_coverage(
        &deadline_trace.0.lock().unwrap(),
        SpanOutcome::NotReached,
        SpanOutcome::NotReached,
    );

    let retry_trace = RecordingTrace::default();
    let mut retry_kernels = KernelRegistry::new();
    retry_kernels
        .register(
            id("phase_retry_exhausted", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Err(KernelError::transient("retry exhausted"))),
        )
        .unwrap();
    assert!(
        RunExecutor::new(&retry_kernels, &no_resources(), &NoFunctions)
            .with_trace_sink(&retry_trace)
            .run(
                &retry_plan("phase_retry_exhausted", 2, Duration::ZERO),
                CancellationToken::new(),
            )
            .is_err()
    );
    assert_run_phase_coverage(
        &retry_trace.0.lock().unwrap(),
        SpanOutcome::Success,
        SpanOutcome::NotReached,
    );
    let retry_attempts = retry_trace
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|span| span.kind == SpanKind::OperationAttempt)
        .map(|span| span.outcome.clone())
        .collect::<Vec<_>>();
    assert_eq!(retry_attempts, [SpanOutcome::Retry, SpanOutcome::Error]);

    let panic_trace = RecordingTrace::default();
    let mut panic_kernels = KernelRegistry::new();
    panic_kernels
        .register(
            id("phase_worker_panic", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| panic!("phase worker panic sentinel")),
        )
        .unwrap();
    let panic_plan = plan(
        vec![operation("phase_worker_panic", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = RunExecutor::new(&panic_kernels, &no_resources(), &NoFunctions)
            .with_trace_sink(&panic_trace)
            .run(&panic_plan, CancellationToken::new());
    }));
    let panic = panic.expect_err("worker panic must resume");
    assert_eq!(
        panic.downcast_ref::<&str>().copied(),
        Some("phase worker panic sentinel")
    );
    assert_run_phase_coverage(
        &panic_trace.0.lock().unwrap(),
        SpanOutcome::Success,
        SpanOutcome::NotReached,
    );
    assert!(
        panic_trace
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|span| span.kind == SpanKind::OperationAttempt
                && span.outcome == SpanOutcome::InternalAborted)
    );
}

#[test]
fn nested_call_spans_record_the_parent_call_and_callee_compile() {
    let mut callee = plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([])));
    callee.provenance.compile_id = CompileId::new(22);
    callee.provenance.graph_path = GraphResourcePath("functions/callee".into());
    let mut caller = plan(
        vec![],
        0,
        StructuredControlRegion::Call {
            target: id("functions/callee", FunctionPlanHandle::new),
            arguments: Box::new([]),
            results: Box::new([]),
            mandatory: true,
        },
    );
    caller.provenance.compile_id = CompileId::new(11);
    let trace = RecordingTrace::default();

    RunExecutor::new(
        &KernelRegistry::new(),
        &no_resources(),
        &OneFunction(published_function(callee, "functions/callee", &[], &[])),
    )
    .with_trace_sink(&trace)
    .run(&caller, CancellationToken::new())
    .unwrap();

    let events = trace.0.lock().unwrap();
    let child = events
        .iter()
        .find(|span| {
            span.kind == SpanKind::Run
                && span.outcome == SpanOutcome::Success
                && span.correlation.compile_id == CompileId::new(22)
        })
        .expect("callee run span");
    assert!(child.correlation.parent_call.is_some());
    assert_eq!(child.correlation.graph_path.0.as_ref(), "functions/callee");
}

#[test]
fn loop_carries_values_through_fresh_activations() {
    let activations = Arc::new(Mutex::new(Vec::new()));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("initial", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(0).into()])),
        )
        .unwrap();
    let seen = activations.clone();
    struct LoopKernel(Arc<Mutex<Vec<ActivationId>>>);
    impl Kernel for LoopKernel {
        fn execute(
            &self,
            context: &KernelContext<'_>,
            inputs: &[RuntimeValue],
        ) -> Result<Vec<RuntimeValue>, KernelError> {
            self.0.lock().unwrap().push(context.activation_id);
            let Some(RuntimeValue::Scalar(Value::Integer(value))) = inputs.first() else {
                return Err(KernelError::new("expected integer"));
            };
            let next = *value + 1;
            Ok(vec![
                Value::Integer(next).into(),
                Value::Bool(next < 3).into(),
            ])
        }
    }
    kernels
        .register(id("loop", KernelHandle::new), LoopKernel(seen))
        .unwrap();
    kernels
        .register(
            id("loop_continuation", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| {
                let RuntimeValue::Scalar(Value::Integer(value)) = &inputs[0] else {
                    return Err(KernelError::new("expected loop result"));
                };
                Ok(vec![Value::Integer(value + 10).into()])
            }),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![
            operation("initial", &[], &[0]),
            operation("loop", &[1], &[2, 3]),
            operation("loop_continuation", &[4], &[5]),
        ],
        6,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(StructuredControlRegion::Loop {
                body: Box::new(StructuredControlRegion::Sequence(Box::new([
                    ControlStep::Operation(OperationIndex::new(1)),
                ]))),
                carried: Box::new([LoopCarriedBinding {
                    body_input: ValueRef::new(1),
                    initial_source: ValueRef::new(0),
                    next_source: ValueRef::new(2),
                    result: ValueRef::new(4),
                    production: Some(OutputProduction::FullyMaterialized),
                }]),
                continue_condition: ValueRef::new(3),
                max_iterations: 4,
            })),
            ControlStep::Operation(OperationIndex::new(2)),
        ])),
    );
    execution_plan.value_sources = Box::new([
        PlanValueSource::ControlProduced(ValueRef::new(1), OutputProduction::FullyMaterialized),
        PlanValueSource::ControlProduced(ValueRef::new(4), OutputProduction::FullyMaterialized),
    ]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(5),
    }]);
    publish_graph_results(&mut execution_plan);

    let result = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(
        result.values["result"],
        RuntimeValue::from(Value::Integer(13))
    );
    let activations = activations.lock().unwrap();
    assert_eq!(activations.len(), 3);
    assert!(activations.windows(2).all(|pair| pair[0] != pair[1]));
}

#[test]
fn loop_does_not_reuse_an_unselected_branch_value_from_a_prior_iteration() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("initial", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(0).into()])),
        )
        .unwrap();
    kernels
        .register(
            id("selector", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| {
                let RuntimeValue::Scalar(Value::Integer(value)) = &inputs[0] else {
                    return Err(KernelError::new("expected integer"));
                };
                let next = value + 1;
                Ok(vec![
                    Value::Integer(next).into(),
                    Value::Bool(*value == 0).into(),
                    Value::Bool(next < 2).into(),
                ])
            }),
        )
        .unwrap();
    kernels
        .register(
            id("branch_value", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(41).into()])),
        )
        .unwrap();
    kernels
        .register(
            id("consume", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| Ok(vec![inputs[0].clone()])),
        )
        .unwrap();

    let mut execution_plan = plan(
        vec![
            operation("initial", &[], &[1]),
            operation("selector", &[0], &[2, 3, 4]),
            operation("branch_value", &[], &[5]),
            operation("consume", &[5], &[6]),
        ],
        8,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(StructuredControlRegion::Loop {
                body: Box::new(StructuredControlRegion::Sequence(Box::new([
                    ControlStep::Operation(OperationIndex::new(1)),
                    ControlStep::Region(Box::new(StructuredControlRegion::If {
                        condition: ValueRef::new(3),
                        then_region: Box::new(StructuredControlRegion::Sequence(Box::new([
                            ControlStep::Operation(OperationIndex::new(2)),
                        ]))),
                        else_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
                        results: Box::new([]),
                    })),
                    ControlStep::Operation(OperationIndex::new(3)),
                ]))),
                carried: Box::new([LoopCarriedBinding {
                    body_input: ValueRef::new(0),
                    initial_source: ValueRef::new(1),
                    next_source: ValueRef::new(2),
                    result: ValueRef::new(7),
                    production: Some(OutputProduction::FullyMaterialized),
                }]),
                continue_condition: ValueRef::new(4),
                max_iterations: 3,
            })),
        ])),
    );
    execution_plan.value_sources = Box::new([
        PlanValueSource::ControlProduced(ValueRef::new(0), OutputProduction::FullyMaterialized),
        PlanValueSource::ControlProduced(ValueRef::new(7), OutputProduction::FullyMaterialized),
    ]);

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .expect_err("an unselected branch must not leak a prior activation value");

    assert!(matches!(error, RunError::InvalidPlan(_)));
}

#[test]
fn call_missing_caller_value_does_not_acquire_callee_resources() {
    let mut callee = plan(vec![], 1, StructuredControlRegion::Sequence(Box::new([])));
    callee.value_sources = Box::new([PlanValueSource::ExternalInput(
        ValueRef::new(0),
        OutputProduction::FullyMaterialized,
    )]);
    callee.resources = Box::new([CompiledResourceRequirement {
        resource: id("external/callee", ResourceId::new),
        kind: ResourceKind::ExternalArtifact,
        access: ResourceAccess::Shared,
        optional: false,
    }]);
    let published = published_function(callee, "functions/callee", &[0], &[]);
    let caller = plan(
        vec![operation("source_not_in_region", &[], &[0])],
        1,
        StructuredControlRegion::Call {
            target: id("functions/callee", FunctionPlanHandle::new),
            arguments: Box::new([CallArgumentBinding {
                caller_source: ValueRef::new(0),
                callee_destination: ValueRef::new(0),
            }]),
            results: Box::new([]),
            mandatory: true,
        },
    );
    let resources = no_resources();

    let error = RunExecutor::new(&KernelRegistry::new(), &resources, &OneFunction(published))
        .run(&caller, CancellationToken::new())
        .expect_err("the caller value is unavailable");

    assert!(matches!(error, RunError::InvalidPlan(_)));
    assert_eq!(resources.acquired.load(Ordering::SeqCst), 0);
}

#[test]
fn call_copies_values_across_different_caller_and_callee_layouts() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(41).into()])),
        )
        .unwrap();
    kernels
        .register(
            id("increment", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| {
                let RuntimeValue::Scalar(Value::Integer(value)) = &inputs[0] else {
                    return Err(KernelError::new("expected integer"));
                };
                Ok(vec![Value::Integer(value + 1).into()])
            }),
        )
        .unwrap();

    let mut callee = plan(
        vec![operation("increment", &[1], &[3])],
        4,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    callee.value_sources = Box::new([PlanValueSource::ExternalInput(
        ValueRef::new(1),
        OutputProduction::FullyMaterialized,
    )]);
    let mut caller = plan(
        vec![operation("source", &[], &[4])],
        5,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(StructuredControlRegion::Call {
                target: id("functions/callee", FunctionPlanHandle::new),
                arguments: Box::new([CallArgumentBinding {
                    caller_source: ValueRef::new(4),
                    callee_destination: ValueRef::new(1),
                }]),
                results: Box::new([CallResultBinding {
                    callee_source: ValueRef::new(3),
                    caller_destination: ValueRef::new(0),
                    production: Some(OutputProduction::FullyMaterialized),
                }]),
                mandatory: true,
            })),
        ])),
    );
    caller.value_sources = Box::new([PlanValueSource::ControlProduced(
        ValueRef::new(0),
        OutputProduction::FullyMaterialized,
    )]);
    caller.results = Box::new([PlanResult {
        name: "answer".into(),
        output: stable_output("answer"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut caller);

    let function = published_function(callee, "functions/callee", &[1], &[3]);
    let result = RunExecutor::new(&kernels, &no_resources(), &OneFunction(function))
        .run(&caller, CancellationToken::new())
        .unwrap();

    assert_eq!(
        result.values["answer"],
        RuntimeValue::from(Value::Integer(42))
    );
}

#[test]
fn call_rejects_stale_published_abi_before_entering_the_callee_frame() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("callee", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(vec![])
            }),
        )
        .unwrap();
    let mut callee = plan(
        vec![operation("callee", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    callee.provenance.graph_path = GraphResourcePath("functions/callee".into());
    let mut stale_provenance = callee.provenance.clone();
    stale_provenance.compile_id = CompileId::new(999);
    let published = Arc::new(PublishedFunctionPlan {
        plan: Arc::new(callee),
        abi: Arc::new(FunctionPlanAbi {
            provenance: stale_provenance,
            parameters: BTreeMap::new(),
            parameter_contracts: BTreeMap::new(),
            results: BTreeMap::new(),
            result_productions: BTreeMap::new(),
            result_contracts: BTreeMap::new(),
        }),
    });
    let caller = plan(
        vec![],
        0,
        StructuredControlRegion::Call {
            target: id("functions/callee", FunctionPlanHandle::new),
            arguments: Box::new([]),
            results: Box::new([]),
            mandatory: true,
        },
    );

    let error = RunExecutor::new(&kernels, &no_resources(), &OneFunction(published))
        .run(&caller, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, RunError::FunctionPlanFailed(_)));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn call_preflight_rejects_invalid_public_bindings_before_callee_side_effects() {
    #[derive(Clone, Copy)]
    enum InvalidCall {
        MissingArgument,
        MissingResult,
        DuplicateCalleeArgument,
        DuplicateCalleeResult,
        DuplicateCallerResult,
        OutOfBoundsParameter,
        UnsourcedResult,
        StaleResultProduction,
    }

    for case in [
        InvalidCall::MissingArgument,
        InvalidCall::MissingResult,
        InvalidCall::DuplicateCalleeArgument,
        InvalidCall::DuplicateCalleeResult,
        InvalidCall::DuplicateCallerResult,
        InvalidCall::OutOfBoundsParameter,
        InvalidCall::UnsourcedResult,
        InvalidCall::StaleResultProduction,
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&calls);
        let mut kernels = KernelRegistry::new();
        kernels
            .register(
                id("source", KernelHandle::new),
                FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(7).into()])),
            )
            .unwrap();
        kernels
            .register(
                id("callee_preflight", KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| {
                    observed.fetch_add(1, Ordering::SeqCst);
                    Ok(vec![Value::Integer(8).into()])
                }),
            )
            .unwrap();

        let mut callee = plan(
            vec![operation("callee_preflight", &[], &[2])],
            4,
            StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(
                OperationIndex::new(0),
            )])),
        );
        callee.value_sources = Box::new([PlanValueSource::ExternalInput(
            ValueRef::new(0),
            OutputProduction::FullyMaterialized,
        )]);
        callee.resources = Box::new([CompiledResourceRequirement {
            resource: id("external/callee", ResourceId::new),
            kind: ResourceKind::ExternalArtifact,
            access: ResourceAccess::Shared,
            optional: false,
        }]);
        if matches!(case, InvalidCall::StaleResultProduction) {
            callee.operations[0].outputs[0].production = OutputProduction::Streaming;
        }
        let mut published = published_function(callee, "functions/callee", &[0], &[2]);

        let standard_argument = CallArgumentBinding {
            caller_source: ValueRef::new(0),
            callee_destination: ValueRef::new(0),
        };
        let standard_result = CallResultBinding {
            callee_source: ValueRef::new(2),
            caller_destination: ValueRef::new(1),
            production: Some(OutputProduction::FullyMaterialized),
        };
        let (arguments, results) = match case {
            InvalidCall::MissingArgument => (vec![], vec![standard_result]),
            InvalidCall::MissingResult => (vec![standard_argument], vec![]),
            InvalidCall::DuplicateCalleeArgument => (
                vec![standard_argument, standard_argument],
                vec![standard_result],
            ),
            InvalidCall::DuplicateCalleeResult => (
                vec![standard_argument],
                vec![
                    standard_result,
                    CallResultBinding {
                        callee_source: ValueRef::new(2),
                        caller_destination: ValueRef::new(3),
                        production: Some(OutputProduction::FullyMaterialized),
                    },
                ],
            ),
            InvalidCall::DuplicateCallerResult => (
                vec![standard_argument],
                vec![standard_result, standard_result],
            ),
            InvalidCall::OutOfBoundsParameter => {
                let published_mut = Arc::make_mut(&mut published);
                Arc::make_mut(&mut published_mut.abi).parameters =
                    BTreeMap::from([(FunctionParameterId("parameter-0".into()), ValueRef::new(9))]);
                (
                    vec![CallArgumentBinding {
                        caller_source: ValueRef::new(0),
                        callee_destination: ValueRef::new(9),
                    }],
                    vec![standard_result],
                )
            }
            InvalidCall::UnsourcedResult => {
                let published_mut = Arc::make_mut(&mut published);
                Arc::make_mut(&mut published_mut.abi).results =
                    BTreeMap::from([(FunctionParameterId("result-0".into()), ValueRef::new(3))]);
                (
                    vec![standard_argument],
                    vec![CallResultBinding {
                        callee_source: ValueRef::new(3),
                        caller_destination: ValueRef::new(1),
                        production: Some(OutputProduction::FullyMaterialized),
                    }],
                )
            }
            InvalidCall::StaleResultProduction => {
                let published_mut = Arc::make_mut(&mut published);
                Arc::make_mut(&mut published_mut.abi).result_productions = BTreeMap::from([(
                    FunctionParameterId("result-0".into()),
                    OutputProduction::Streaming,
                )]);
                (vec![standard_argument], vec![standard_result])
            }
        };
        let mut caller = plan(
            vec![operation("source", &[], &[0])],
            4,
            StructuredControlRegion::Sequence(Box::new([
                ControlStep::Operation(OperationIndex::new(0)),
                ControlStep::Region(Box::new(StructuredControlRegion::Call {
                    target: id("functions/callee", FunctionPlanHandle::new),
                    arguments: arguments.into_boxed_slice(),
                    results: results.into_boxed_slice(),
                    mandatory: true,
                })),
            ])),
        );
        let mut destinations = BTreeMap::new();
        if !matches!(case, InvalidCall::MissingResult) {
            destinations.insert(
                ValueRef::new(1),
                PlanValueSource::ControlProduced(
                    ValueRef::new(1),
                    OutputProduction::FullyMaterialized,
                ),
            );
        }
        if matches!(case, InvalidCall::DuplicateCalleeResult) {
            destinations.insert(
                ValueRef::new(3),
                PlanValueSource::ControlProduced(
                    ValueRef::new(3),
                    OutputProduction::FullyMaterialized,
                ),
            );
        }
        caller.value_sources = destinations
            .into_values()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let resources = no_resources();

        let error = RunExecutor::new(&kernels, &resources, &OneFunction(published))
            .run(&caller, CancellationToken::new())
            .expect_err("invalid public Call bindings must fail preflight");

        assert!(matches!(
            error,
            RunError::FunctionPlanFailed(_) | RunError::InvalidPlan(_)
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(resources.acquired.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn call_preflight_allows_reusing_one_caller_source_for_distinct_parameters() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(7).into()])),
        )
        .unwrap();
    kernels
        .register(
            id("callee_fan_in", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(vec![])
            }),
        )
        .unwrap();
    let mut callee = plan(
        vec![operation("callee_fan_in", &[], &[])],
        2,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    callee.value_sources = Box::new([
        PlanValueSource::ExternalInput(ValueRef::new(0), OutputProduction::FullyMaterialized),
        PlanValueSource::ExternalInput(ValueRef::new(1), OutputProduction::FullyMaterialized),
    ]);
    let published = published_function(callee, "functions/callee", &[0, 1], &[]);
    let caller = plan(
        vec![operation("source", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(StructuredControlRegion::Call {
                target: id("functions/callee", FunctionPlanHandle::new),
                arguments: Box::new([
                    CallArgumentBinding {
                        caller_source: ValueRef::new(0),
                        callee_destination: ValueRef::new(0),
                    },
                    CallArgumentBinding {
                        caller_source: ValueRef::new(0),
                        callee_destination: ValueRef::new(1),
                    },
                ]),
                results: Box::new([]),
                mandatory: true,
            })),
        ])),
    );

    RunExecutor::new(&kernels, &no_resources(), &OneFunction(published))
        .run(&caller, CancellationToken::new())
        .unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn call_uses_an_independent_frame() {
    struct ContextKernel(Arc<Mutex<Vec<FrameId>>>);
    impl Kernel for ContextKernel {
        fn execute(
            &self,
            context: &KernelContext<'_>,
            _: &[RuntimeValue],
        ) -> Result<Vec<RuntimeValue>, KernelError> {
            self.0.lock().unwrap().push(context.frame_id);
            Ok(vec![Value::Null.into()])
        }
    }

    let frames = Arc::new(Mutex::new(Vec::new()));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("context", KernelHandle::new),
            ContextKernel(frames.clone()),
        )
        .unwrap();
    let callee = Arc::new(plan(
        vec![operation("context", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    ));
    let caller = plan(
        vec![operation("context", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(StructuredControlRegion::Call {
                target: id("functions/callee", FunctionPlanHandle::new),
                arguments: Box::new([]),
                results: Box::new([]),
                mandatory: true,
            })),
        ])),
    );

    RunExecutor::new(
        &kernels,
        &no_resources(),
        &OneFunction(published_function(
            Arc::unwrap_or_clone(callee),
            "functions/callee",
            &[],
            &[],
        )),
    )
    .run(&caller, CancellationToken::new())
    .unwrap();

    let frames = frames.lock().unwrap();
    assert_eq!(frames.len(), 2);
    assert_ne!(frames[0], frames[1]);
}

#[test]
fn effect_dependencies_determine_ready_queue_order() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut kernels = KernelRegistry::new();
    for name in ["after", "before"] {
        let events = events.clone();
        kernels
            .register(
                id(name, KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| {
                    events.lock().unwrap().push(name);
                    Ok(vec![Value::Null.into()])
                }),
            )
            .unwrap();
    }
    let mut execution_plan = plan(
        vec![
            operation("after", &[], &[0]),
            operation("before", &[], &[1]),
        ],
        2,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    execution_plan.effect_dependencies = Box::new([EffectDependency {
        before: OperationIndex::new(1),
        after: OperationIndex::new(0),
    }]);

    RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(*events.lock().unwrap(), vec!["before", "after"]);
}

struct ParallelGate {
    open: Mutex<bool>,
    ready: Condvar,
}

impl ParallelGate {
    fn closed() -> Arc<Self> {
        Arc::new(Self {
            open: Mutex::new(false),
            ready: Condvar::new(),
        })
    }

    fn wait(&self) {
        let open = self.open.lock().unwrap();
        drop(self.ready.wait_while(open, |open| !*open).unwrap());
    }

    fn release(&self) {
        *self.open.lock().unwrap() = true;
        self.ready.notify_all();
    }
}

struct GatedKernel {
    name: &'static str,
    started: mpsc::Sender<&'static str>,
    finished: Option<mpsc::Sender<&'static str>>,
    gate: Arc<ParallelGate>,
    active: Arc<AtomicUsize>,
    maximum: Arc<AtomicUsize>,
    output: Value,
}

impl Kernel for GatedKernel {
    fn execute(
        &self,
        _: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(active, Ordering::SeqCst);
        self.started.send(self.name).unwrap();
        self.gate.wait();
        self.active.fetch_sub(1, Ordering::SeqCst);
        if let Some(finished) = &self.finished {
            finished.send(self.name).unwrap();
        }
        Ok(vec![self.output.clone().into()])
    }
}

fn parallel_policy(cpu: usize, io: usize, adapter: usize) -> SchedulingPolicy {
    SchedulingPolicy {
        cpu_parallelism: NonZeroUsize::new(cpu).unwrap(),
        io_parallelism: NonZeroUsize::new(io).unwrap(),
        adapter_parallelism: NonZeroUsize::new(adapter).unwrap(),
    }
}

fn independent_parallel_plan(classes: &[WorkloadClass]) -> ExecutionPlan {
    let mut operations = Vec::new();
    let mut steps = Vec::new();
    for (index, workload) in classes.iter().copied().enumerate() {
        let kernel = format!("parallel{index}");
        let mut planned = operation(&kernel, &[], &[index as u32]);
        planned.workload = workload;
        operations.push(planned);
        steps.push(ControlStep::Operation(OperationIndex::new(index as u32)));
    }
    plan(
        operations,
        classes.len() as u32,
        StructuredControlRegion::Sequence(steps.into_boxed_slice()),
    )
}

fn register_gated_kernels(
    kernels: &mut KernelRegistry,
    gates: &[Arc<ParallelGate>],
    started: &mpsc::Sender<&'static str>,
    finished: Option<&mpsc::Sender<&'static str>>,
    active: &Arc<AtomicUsize>,
    maximum: &Arc<AtomicUsize>,
) {
    const NAMES: [&str; 8] = [
        "parallel0",
        "parallel1",
        "parallel2",
        "parallel3",
        "parallel4",
        "parallel5",
        "parallel6",
        "parallel7",
    ];
    for (index, gate) in gates.iter().enumerate() {
        kernels
            .register(
                id(NAMES[index], KernelHandle::new),
                GatedKernel {
                    name: NAMES[index],
                    started: started.clone(),
                    finished: finished.cloned(),
                    gate: Arc::clone(gate),
                    active: Arc::clone(active),
                    maximum: Arc::clone(maximum),
                    output: Value::Integer(index as i64),
                },
            )
            .unwrap();
    }
}

fn release_all(gates: &[Arc<ParallelGate>]) {
    for gate in gates {
        gate.release();
    }
}

#[test]
fn parallel_scheduler_independent_cpu_operations_overlap() {
    let gates = [ParallelGate::closed(), ParallelGate::closed()];
    let (started_tx, started_rx) = mpsc::channel();
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    register_gated_kernels(&mut kernels, &gates, &started_tx, None, &active, &maximum);
    let execution_plan = independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Cpu]);

    let run = thread::spawn(move || {
        RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
            .with_scheduling_policy(parallel_policy(2, 1, 1))
            .run(&execution_plan, CancellationToken::new())
    });
    let first = started_rx.recv_timeout(Duration::from_secs(2));
    let second = started_rx.recv_timeout(Duration::from_secs(2));
    release_all(&gates);
    let result = run.join().unwrap();

    assert!(first.is_ok(), "first CPU operation did not start");
    assert!(second.is_ok(), "independent CPU operations did not overlap");
    result.unwrap();
    assert_eq!(maximum.load(Ordering::SeqCst), 2);
}

fn assert_parallel_class_limit(class: WorkloadClass, policy: SchedulingPolicy) {
    let gates = [
        ParallelGate::closed(),
        ParallelGate::closed(),
        ParallelGate::closed(),
    ];
    let (started_tx, started_rx) = mpsc::channel();
    let (blocked_tx, blocked_rx) = mpsc::channel();
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    register_gated_kernels(&mut kernels, &gates, &started_tx, None, &active, &maximum);
    let execution_plan = independent_parallel_plan(&[class, class, class]);

    let run = thread::spawn(move || {
        RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
            .with_scheduling_policy(policy)
            .with_test_checkpoint(Arc::new(move |checkpoint, _| {
                if checkpoint == SchedulerCheckpoint::AdmissionBlocked(class) {
                    let _ = blocked_tx.send(());
                }
            }))
            .run(&execution_plan, CancellationToken::new())
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    blocked_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(
        started_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    release_all(&gates);
    let result = run.join().unwrap();

    result.unwrap();
    assert_eq!(maximum.load(Ordering::SeqCst), 2);
}

#[test]
fn parallel_scheduler_enforces_hard_cpu_limit_after_blocked_admission() {
    assert_parallel_class_limit(WorkloadClass::Cpu, parallel_policy(2, 1, 1));
}

#[test]
fn parallel_scheduler_enforces_hard_io_limit_after_blocked_admission() {
    assert_parallel_class_limit(WorkloadClass::Io, parallel_policy(1, 2, 1));
}

#[test]
fn parallel_scheduler_enforces_hard_adapter_limit_after_blocked_admission() {
    assert_parallel_class_limit(WorkloadClass::AdapterIo, parallel_policy(1, 1, 2));
}

#[test]
fn parallel_scheduler_io_has_a_separate_budget() {
    let gates = [ParallelGate::closed(), ParallelGate::closed()];
    let (started_tx, started_rx) = mpsc::channel();
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    register_gated_kernels(&mut kernels, &gates, &started_tx, None, &active, &maximum);
    let execution_plan = independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Io]);

    let run = thread::spawn(move || {
        RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
            .with_scheduling_policy(parallel_policy(1, 1, 1))
            .run(&execution_plan, CancellationToken::new())
    });
    let first = started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let second = started_rx.recv_timeout(Duration::from_secs(2));
    release_all(&gates);
    run.join().unwrap().unwrap();

    assert_eq!(first, "parallel0");
    assert_eq!(second.unwrap(), "parallel1");
    assert_eq!(maximum.load(Ordering::SeqCst), 2);
}

#[test]
fn parallel_scheduler_exclusive_work_never_overlaps_other_work() {
    let gates = [ParallelGate::closed(), ParallelGate::closed()];
    let (started_tx, started_rx) = mpsc::channel();
    let (blocked_tx, blocked_rx) = mpsc::channel();
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    register_gated_kernels(&mut kernels, &gates, &started_tx, None, &active, &maximum);
    let execution_plan = independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Exclusive]);

    let run = thread::spawn(move || {
        RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
            .with_scheduling_policy(parallel_policy(2, 1, 1))
            .with_test_checkpoint(Arc::new(move |checkpoint, _| {
                if checkpoint == SchedulerCheckpoint::AdmissionBlocked(WorkloadClass::Exclusive) {
                    let _ = blocked_tx.send(());
                }
            }))
            .run(&execution_plan, CancellationToken::new())
    });
    assert_eq!(
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        "parallel0"
    );
    blocked_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(
        started_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    gates[0].release();
    let exclusive = started_rx.recv_timeout(Duration::from_secs(2));
    gates[1].release();
    run.join().unwrap().unwrap();

    assert_eq!(exclusive.unwrap(), "parallel1");
    assert_eq!(maximum.load(Ordering::SeqCst), 1);
}

#[test]
fn parallel_scheduler_io_is_not_starved_by_sustained_cpu_load() {
    let gates = [
        ParallelGate::closed(),
        ParallelGate::closed(),
        ParallelGate::closed(),
        ParallelGate::closed(),
    ];
    let (started_tx, started_rx) = mpsc::channel();
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    register_gated_kernels(&mut kernels, &gates, &started_tx, None, &active, &maximum);
    let execution_plan = independent_parallel_plan(&[
        WorkloadClass::Cpu,
        WorkloadClass::Cpu,
        WorkloadClass::Cpu,
        WorkloadClass::Io,
    ]);

    let run = thread::spawn(move || {
        RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
            .with_scheduling_policy(parallel_policy(1, 1, 1))
            .run(&execution_plan, CancellationToken::new())
    });
    let first = started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let second = started_rx.recv_timeout(Duration::from_secs(2));
    release_all(&gates);
    run.join().unwrap().unwrap();

    assert_eq!(
        BTreeSet::from([first, second.unwrap()]),
        BTreeSet::from(["parallel0", "parallel3"])
    );
}

struct ThreadIdentityKernel(Arc<Mutex<HashSet<thread::ThreadId>>>);

impl Kernel for ThreadIdentityKernel {
    fn execute(
        &self,
        _: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        self.0.lock().unwrap().insert(thread::current().id());
        Ok(vec![Value::Null.into()])
    }
}

#[test]
fn parallel_scheduler_reuses_a_policy_bounded_worker_pool() {
    let worker_threads = Arc::new(Mutex::new(HashSet::new()));
    let mut kernels = KernelRegistry::new();
    for index in 0..8 {
        kernels
            .register(
                id(&format!("parallel{index}"), KernelHandle::new),
                ThreadIdentityKernel(Arc::clone(&worker_threads)),
            )
            .unwrap();
    }
    let execution_plan = independent_parallel_plan(&[WorkloadClass::Cpu; 8]);

    RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_scheduling_policy(parallel_policy(2, 1, 1))
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert!(worker_threads.lock().unwrap().len() <= 4);
}

#[test]
fn parallel_scheduler_completion_order_does_not_change_value_mapping() {
    let gates = [ParallelGate::closed(), ParallelGate::closed()];
    let (started_tx, started_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    register_gated_kernels(
        &mut kernels,
        &gates,
        &started_tx,
        Some(&finished_tx),
        &active,
        &maximum,
    );
    let mut execution_plan = independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Cpu]);
    execution_plan.results = Box::new([
        PlanResult {
            name: "first".into(),
            value: ValueRef::new(0),
            output: stable_output("first"),
        },
        PlanResult {
            name: "second".into(),
            value: ValueRef::new(1),
            output: stable_output("second"),
        },
    ]);
    publish_graph_results(&mut execution_plan);

    let run = thread::spawn(move || {
        RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
            .with_scheduling_policy(parallel_policy(2, 1, 1))
            .run(&execution_plan, CancellationToken::new())
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    gates[1].release();
    assert_eq!(
        finished_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        "parallel1"
    );
    gates[0].release();
    let result = run.join().unwrap().unwrap();

    assert_eq!(result.values["first"], Value::Integer(0).into());
    assert_eq!(result.values["second"], Value::Integer(1).into());
}

struct CancellationDrainKernel {
    started: mpsc::Sender<()>,
    exited: mpsc::Sender<()>,
}

impl Kernel for CancellationDrainKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        let waiter = Arc::new(Condvar::new());
        context.cancellation.register_waiter(&waiter);
        self.started.send(()).unwrap();
        let lock = Mutex::new(());
        let guard = lock.lock().unwrap();
        drop(
            waiter
                .wait_while(guard, |_| !context.cancellation.is_cancelled())
                .unwrap(),
        );
        self.exited.send(()).unwrap();
        Err(KernelError::cancelled("cancelled for drain"))
    }
}

enum MultiWorkerTerminalKind {
    Error,
    Panic,
    WaitForCancellation,
}

struct MultiWorkerTerminalKernel {
    kind: MultiWorkerTerminalKind,
    entered: Arc<Barrier>,
    exited: mpsc::Sender<()>,
}

impl Kernel for MultiWorkerTerminalKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        let waiter = Arc::new(Condvar::new());
        context.cancellation.register_waiter(&waiter);
        self.entered.wait();
        match self.kind {
            MultiWorkerTerminalKind::Error => Err(KernelError::new("multi-worker failure")),
            MultiWorkerTerminalKind::Panic => panic!("multi-worker panic"),
            MultiWorkerTerminalKind::WaitForCancellation => {
                let lock = Mutex::new(());
                let guard = lock.lock().unwrap();
                drop(
                    waiter
                        .wait_while(guard, |_| !context.cancellation.is_cancelled())
                        .unwrap(),
                );
                self.exited.send(()).unwrap();
                Err(KernelError::cancelled("peer drained"))
            }
        }
    }
}

fn multi_worker_terminal_fixture(
    terminal: MultiWorkerTerminalKind,
) -> (
    KernelRegistry,
    ExecutionPlan,
    Arc<Barrier>,
    mpsc::Receiver<()>,
) {
    let entered = Arc::new(Barrier::new(3));
    let (exited_tx, exited_rx) = mpsc::channel();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("parallel0", KernelHandle::new),
            MultiWorkerTerminalKernel {
                kind: terminal,
                entered: Arc::clone(&entered),
                exited: exited_tx.clone(),
            },
        )
        .unwrap();
    kernels
        .register(
            id("parallel1", KernelHandle::new),
            MultiWorkerTerminalKernel {
                kind: MultiWorkerTerminalKind::WaitForCancellation,
                entered: Arc::clone(&entered),
                exited: exited_tx,
            },
        )
        .unwrap();
    (
        kernels,
        independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Cpu]),
        entered,
        exited_rx,
    )
}

#[test]
fn parallel_scheduler_ordinary_error_drains_and_joins_peer_worker() {
    let (kernels, execution_plan, entered, exited) =
        multi_worker_terminal_fixture(MultiWorkerTerminalKind::Error);
    let run = thread::spawn(move || {
        RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
            .with_scheduling_policy(parallel_policy(2, 1, 1))
            .run(&execution_plan, CancellationToken::new())
    });
    entered.wait();

    exited.recv_timeout(Duration::from_secs(2)).unwrap();
    let error = run.join().unwrap().unwrap_err();

    assert!(
        matches!(error, RunError::KernelFailed { message, .. } if message.as_ref() == "multi-worker failure")
    );
}

#[test]
fn parallel_scheduler_panic_drains_and_joins_peer_worker_before_unwind() {
    let (kernels, execution_plan, entered, exited) =
        multi_worker_terminal_fixture(MultiWorkerTerminalKind::Panic);
    let run = thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
                .with_scheduling_policy(parallel_policy(2, 1, 1))
                .run(&execution_plan, CancellationToken::new());
        }))
    });
    entered.wait();

    exited.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(run.join().unwrap().is_err());
}

#[test]
fn parallel_scheduler_cancellation_drains_all_workers() {
    let (started_tx, started_rx) = mpsc::channel();
    let (exited_tx, exited_rx) = mpsc::channel();
    let mut kernels = KernelRegistry::new();
    for index in 0..2 {
        kernels
            .register(
                id(&format!("parallel{index}"), KernelHandle::new),
                CancellationDrainKernel {
                    started: started_tx.clone(),
                    exited: exited_tx.clone(),
                },
            )
            .unwrap();
    }
    let execution_plan = independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Cpu]);
    let cancellation = CancellationToken::new();
    let run_cancellation = cancellation.clone();
    let run = thread::spawn(move || {
        RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
            .with_scheduling_policy(parallel_policy(2, 1, 1))
            .run(&execution_plan, run_cancellation)
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();

    cancellation.cancel();
    exited_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    exited_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let result = run.join().unwrap();

    assert_eq!(result.unwrap_err(), RunError::Cancelled);
}

#[test]
fn duplicate_operation_in_one_activation_is_rejected() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("once", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(vec![Value::Null.into()])
            }),
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("once", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(0)),
        ])),
    );

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, RunError::OperationAlreadyExecuted { .. }));
    assert!(calls.load(Ordering::SeqCst) <= 1);
}

#[test]
fn reversed_two_function_publication_is_equivalent_and_callable() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("function_a", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(vec![])
            }),
        )
        .unwrap();

    let versions = BTreeMap::from([
        (
            ResourceKey::new("functions/a"),
            ResourceVersion::new("a-v1"),
        ),
        (
            ResourceKey::new("functions/b"),
            ResourceVersion::new("b-v1"),
        ),
    ]);
    let make_function = |path: &str, root_region: StructuredControlRegion, operations| {
        let mut function = plan(operations, 0, root_region);
        function.provenance.graph_path = GraphResourcePath(path.into());
        function.provenance.basis.resource_versions = versions.clone();
        let abi = FunctionPlanAbi {
            provenance: function.provenance.clone(),
            parameters: BTreeMap::new(),
            parameter_contracts: BTreeMap::new(),
            results: BTreeMap::new(),
            result_productions: BTreeMap::new(),
            result_contracts: BTreeMap::new(),
        };
        (Arc::new(function), Arc::new(abi))
    };
    let (plan_a, abi_a) = make_function(
        "functions/a",
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
        vec![operation("function_a", &[], &[])],
    );
    let (plan_b, abi_b) = make_function(
        "functions/b",
        StructuredControlRegion::Call {
            target: id("functions/a", FunctionPlanHandle::new),
            arguments: Box::new([]),
            results: Box::new([]),
            mandatory: true,
        },
        vec![],
    );
    let entries = vec![
        (
            GraphResourcePath("functions/a".into()),
            ResourceVersion::new("a-v1"),
            plan_a,
            abi_a,
        ),
        (
            GraphResourcePath("functions/b".into()),
            ResourceVersion::new("b-v1"),
            plan_b,
            abi_b,
        ),
    ];
    let store = FunctionPlanStore::new(ProjectSessionId::new("test-session"), 64);
    let forward = store
        .generation(
            RegistryFingerprint::from_bytes([1; 32]),
            versions.clone(),
            entries.clone(),
        )
        .unwrap();
    let reverse = store
        .generation(
            RegistryFingerprint::from_bytes([1; 32]),
            versions.clone(),
            entries.into_iter().rev().collect(),
        )
        .unwrap();
    let mut caller = plan(
        vec![],
        0,
        StructuredControlRegion::Call {
            target: id("functions/b", FunctionPlanHandle::new),
            arguments: Box::new([]),
            results: Box::new([]),
            mandatory: true,
        },
    );
    caller.provenance.basis.resource_versions = versions;

    RunExecutor::new(&kernels, &no_resources(), &forward)
        .run(&caller, CancellationToken::new())
        .unwrap();
    RunExecutor::new(&kernels, &no_resources(), &reverse)
        .run(&caller, CancellationToken::new())
        .unwrap();

    assert_eq!(forward.plan_count(), reverse.plan_count());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn recursive_calls_stop_at_the_configured_limit() {
    let recursive = plan(
        vec![],
        0,
        StructuredControlRegion::Call {
            target: id("recursive", FunctionPlanHandle::new),
            arguments: Box::new([]),
            results: Box::new([]),
            mandatory: true,
        },
    );
    let recursive = published_function(recursive, "recursive", &[], &[]);
    let resources = no_resources();

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &resources,
        &OneFunction(Arc::clone(&recursive)),
    )
    .with_recursion_limit(3)
    .run(recursive.plan.as_ref(), CancellationToken::new())
    .unwrap_err();

    assert_eq!(
        error,
        RunError::RecursionLimitExceeded { recursion_limit: 3 }
    );
}

#[test]
fn kernel_failure_releases_resources_without_retry() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("fail", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Err(KernelError::new("kernel failed"))
            }),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("fail", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.resources = Box::new([requirement("temporary")]);
    let resources = no_resources();
    let released = resources.released.clone();

    let error = RunExecutor::new(&kernels, &resources, &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, RunError::KernelFailed { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(released.load(Ordering::SeqCst), 1);
}

fn retry_policy(max_attempts: u32, backoff: Duration) -> RetryPolicy {
    RetryPolicy::new(NonZeroU32::new(max_attempts).unwrap(), backoff, backoff).unwrap()
}

fn retry_plan(kernel: &str, max_attempts: u32, backoff: Duration) -> ExecutionPlan {
    let mut planned = operation(kernel, &[], &[0]);
    planned.cache_policy = CachePolicy::PerRun;
    planned.retry = PlannedRetry {
        idempotent: true,
        policy: Some(retry_policy(max_attempts, backoff)),
    };
    let mut execution_plan = plan(
        vec![planned],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("retry_result"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    execution_plan
}

struct RetryProgressGate {
    release: Mutex<mpsc::Receiver<()>>,
    completed: mpsc::Sender<()>,
}

impl Kernel for RetryProgressGate {
    fn execute(
        &self,
        _: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        self.release.lock().unwrap().recv().unwrap();
        self.completed.send(()).unwrap();
        Ok(vec![Value::Integer(1).into()])
    }
}

fn retry_progress_plan(gates: usize, exclusive_tail: bool) -> ExecutionPlan {
    let mut retry = operation("retry_progress", &[], &[0]);
    retry.cache_policy = CachePolicy::PerRun;
    retry.retry = PlannedRetry {
        idempotent: true,
        policy: Some(retry_policy(2, Duration::from_secs(5))),
    };
    let mut operations = vec![retry];
    for index in 0..gates {
        let mut gate = operation("retry_progress_gate", &[], &[(index + 1) as u32]);
        gate.stable_id =
            OperationStableId::new(format!("test.retry.progress.gate.{index}")).unwrap();
        if exclusive_tail && index + 1 == gates {
            gate.workload = WorkloadClass::Exclusive;
        }
        operations.push(gate);
    }
    let mut execution_plan = plan(
        operations,
        (gates + 1) as u32,
        StructuredControlRegion::Sequence(
            (0..=gates)
                .map(|index| ControlStep::Operation(OperationIndex::new(index as u32)))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
    );
    if exclusive_tail && gates >= 2 {
        execution_plan.effect_dependencies = Box::new([EffectDependency {
            before: OperationIndex::new((gates - 1) as u32),
            after: OperationIndex::new(gates as u32),
        }]);
    }
    execution_plan
}

#[test]
fn retry_delayed_queue_drains_bounded_completions_during_long_backoff() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let (release_tx, release_rx) = mpsc::channel();
    let (completed_tx, completed_rx) = mpsc::channel();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_progress", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                if observed.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(KernelError::transient("delay"))
                } else {
                    Ok(vec![Value::Integer(0).into()])
                }
            }),
        )
        .unwrap();
    kernels
        .register(
            id("retry_progress_gate", KernelHandle::new),
            RetryProgressGate {
                release: Mutex::new(release_rx),
                completed: completed_tx,
            },
        )
        .unwrap();
    let cancellation = CancellationToken::new();
    let cancel_run = cancellation.clone();
    let release = Arc::new(Mutex::new(Some(release_tx)));
    let release_at_backoff = Arc::clone(&release);
    let execution_plan = retry_progress_plan(6, false);

    thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
                .with_scheduling_policy(parallel_policy(2, 1, 1))
                .with_test_checkpoint(Arc::new(move |checkpoint, _| {
                    if matches!(checkpoint, SchedulerCheckpoint::RetryBackoff { .. })
                        && let Some(release) = release_at_backoff.lock().unwrap().take()
                    {
                        for _ in 0..6 {
                            release.send(()).unwrap();
                        }
                    }
                }))
                .run(&execution_plan, cancellation)
        });
        for _ in 0..6 {
            completed_rx
                .recv_timeout(Duration::from_millis(500))
                .unwrap();
        }
        cancel_run.cancel();
        assert_eq!(run.join().unwrap(), Err(RunError::Cancelled));
    });
}

#[test]
fn retry_delayed_queue_allows_exclusive_effect_progress() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let (release_tx, release_rx) = mpsc::channel();
    let (completed_tx, completed_rx) = mpsc::channel();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_progress", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                if observed.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(KernelError::transient("delay"))
                } else {
                    Ok(vec![Value::Integer(0).into()])
                }
            }),
        )
        .unwrap();
    kernels
        .register(
            id("retry_progress_gate", KernelHandle::new),
            RetryProgressGate {
                release: Mutex::new(release_rx),
                completed: completed_tx,
            },
        )
        .unwrap();
    let cancellation = CancellationToken::new();
    let cancel_run = cancellation.clone();
    let release = Arc::new(Mutex::new(Some(release_tx)));
    let release_at_backoff = Arc::clone(&release);
    let execution_plan = retry_progress_plan(2, true);

    thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
                .with_scheduling_policy(parallel_policy(2, 1, 1))
                .with_test_checkpoint(Arc::new(move |checkpoint, _| {
                    if matches!(checkpoint, SchedulerCheckpoint::RetryBackoff { .. })
                        && let Some(release) = release_at_backoff.lock().unwrap().take()
                    {
                        release.send(()).unwrap();
                        release.send(()).unwrap();
                    }
                }))
                .run(&execution_plan, cancellation)
        });
        completed_rx
            .recv_timeout(Duration::from_millis(500))
            .unwrap();
        completed_rx
            .recv_timeout(Duration::from_millis(500))
            .unwrap();
        cancel_run.cancel();
        assert_eq!(run.join().unwrap(), Err(RunError::Cancelled));
    });
}

#[derive(Debug, Clone, Copy)]
enum AdmissionRejection {
    Cancellation,
    Deadline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AdmissionRollbackObservation {
    operation: OperationIndex,
    attempt: AttemptId,
    running_count: usize,
    tracked_running: usize,
    memo_owned: bool,
    frame_attempt: Option<AttemptId>,
}

struct CooperativeAdmissionPeer {
    calls: Arc<AtomicUsize>,
}

impl Kernel for CooperativeAdmissionPeer {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        context.wait_for(Duration::from_millis(100))?;
        Ok(vec![Value::Integer(1).into()])
    }
}

fn admission_rejection_deadline(rejection: AdmissionRejection) -> RunDeadline {
    match rejection {
        AdmissionRejection::Cancellation => RunDeadline::after(Duration::from_secs(5)),
        AdmissionRejection::Deadline => RunDeadline::after(Duration::from_millis(10)),
    }
}

fn reject_admission(rejection: AdmissionRejection, cancellation: &CancellationToken) {
    match rejection {
        AdmissionRejection::Cancellation => cancellation.cancel(),
        AdmissionRejection::Deadline => thread::sleep(Duration::from_millis(30)),
    }
}

fn assert_rejected_attempt_not_started(events: &RecordingRunEvents, operation: u32, attempt: u64) {
    assert!(!events.0.lock().unwrap().iter().any(|event| matches!(
        event.kind,
        RunEventKind::OperationStarted {
            operation_index,
            attempt_id,
            ..
        } if operation_index == operation && attempt_id == attempt
    )));
}

fn run_initial_admission_rejection(rejection: AdmissionRejection) {
    let peer_calls = Arc::new(AtomicUsize::new(0));
    let rejected_calls = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("admission_peer", KernelHandle::new),
            CooperativeAdmissionPeer {
                calls: Arc::clone(&peer_calls),
            },
        )
        .unwrap();
    let observed_rejected = Arc::clone(&rejected_calls);
    kernels
        .register(
            id("admission_rejected", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed_rejected.fetch_add(1, Ordering::SeqCst);
                Ok(vec![Value::Integer(2).into()])
            }),
        )
        .unwrap();
    let mut rejected = operation("admission_rejected", &[], &[1]);
    rejected.cache_policy = CachePolicy::PerRun;
    let execution_plan = plan(
        vec![operation("admission_peer", &[], &[0]), rejected],
        2,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    let rollback = Arc::new(Mutex::new(None));
    let observed_rollback = Arc::clone(&rollback);
    let events = RecordingRunEvents::default();

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_scheduling_policy(parallel_policy(2, 1, 1))
        .with_deadline(admission_rejection_deadline(rejection))
        .with_event_sink(&events)
        .with_test_checkpoint(Arc::new(move |checkpoint, cancellation| match checkpoint {
            SchedulerCheckpoint::AdmissionBookkept {
                operation, attempt, ..
            } if operation == OperationIndex::new(1) && attempt == AttemptId::initial() => {
                reject_admission(rejection, cancellation);
            }
            SchedulerCheckpoint::AdmissionRolledBack {
                operation,
                attempt,
                running_count,
                tracked_running,
                memo_owned,
                frame_attempt,
            } if operation == OperationIndex::new(1) => {
                *observed_rollback.lock().unwrap() = Some(AdmissionRollbackObservation {
                    operation,
                    attempt,
                    running_count,
                    tracked_running,
                    memo_owned,
                    frame_attempt,
                });
            }
            _ => {}
        }))
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(
        (rejection, error),
        (AdmissionRejection::Cancellation, RunError::Cancelled)
            | (
                AdmissionRejection::Deadline,
                RunError::DeadlineExceeded {
                    phase: RunPhase::QueueWait
                }
            )
    ));
    assert!(
        peer_calls.load(Ordering::SeqCst) <= 1,
        "cancellation may stop the admitted peer before kernel invocation"
    );
    assert_eq!(rejected_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        rollback.lock().unwrap().clone(),
        Some(AdmissionRollbackObservation {
            operation: OperationIndex::new(1),
            attempt: AttemptId::initial(),
            running_count: 1,
            tracked_running: 1,
            memo_owned: false,
            frame_attempt: None,
        })
    );
    assert_rejected_attempt_not_started(&events, 1, 1);
}

fn run_promoted_retry_admission_rejection(rejection: AdmissionRejection) {
    let retry_calls = Arc::new(AtomicUsize::new(0));
    let peer_calls = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    let observed_retry = Arc::clone(&retry_calls);
    kernels
        .register(
            id("retry_admission_rejected", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                if observed_retry.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(KernelError::transient("promote retry"))
                } else {
                    Ok(vec![Value::Integer(2).into()])
                }
            }),
        )
        .unwrap();
    kernels
        .register(
            id("retry_admission_peer", KernelHandle::new),
            CooperativeAdmissionPeer {
                calls: Arc::clone(&peer_calls),
            },
        )
        .unwrap();
    let mut retry = operation("retry_admission_rejected", &[], &[0]);
    retry.cache_policy = CachePolicy::PerRun;
    retry.retry = PlannedRetry {
        idempotent: true,
        policy: Some(retry_policy(2, Duration::ZERO)),
    };
    let execution_plan = plan(
        vec![retry, operation("retry_admission_peer", &[], &[1])],
        2,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    let rollback = Arc::new(Mutex::new(None));
    let observed_rollback = Arc::clone(&rollback);
    let events = RecordingRunEvents::default();

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_scheduling_policy(parallel_policy(2, 1, 1))
        .with_deadline(admission_rejection_deadline(rejection))
        .with_event_sink(&events)
        .with_test_checkpoint(Arc::new(move |checkpoint, cancellation| match checkpoint {
            SchedulerCheckpoint::AdmissionBookkept {
                operation, attempt, ..
            } if operation == OperationIndex::new(0) && attempt == AttemptId::new(2) => {
                reject_admission(rejection, cancellation);
            }
            SchedulerCheckpoint::AdmissionRolledBack {
                operation,
                attempt,
                running_count,
                tracked_running,
                memo_owned,
                frame_attempt,
            } if operation == OperationIndex::new(0) => {
                *observed_rollback.lock().unwrap() = Some(AdmissionRollbackObservation {
                    operation,
                    attempt,
                    running_count,
                    tracked_running,
                    memo_owned,
                    frame_attempt,
                });
            }
            _ => {}
        }))
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(
        (rejection, error),
        (AdmissionRejection::Cancellation, RunError::Cancelled)
            | (
                AdmissionRejection::Deadline,
                RunError::DeadlineExceeded {
                    phase: RunPhase::QueueWait
                }
            )
    ));
    assert_eq!(retry_calls.load(Ordering::SeqCst), 1);
    assert_eq!(peer_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        rollback.lock().unwrap().clone(),
        Some(AdmissionRollbackObservation {
            operation: OperationIndex::new(0),
            attempt: AttemptId::new(2),
            running_count: 1,
            tracked_running: 1,
            memo_owned: false,
            frame_attempt: Some(AttemptId::initial()),
        })
    );
    assert_rejected_attempt_not_started(&events, 0, 2);
}

#[test]
fn initial_admission_cancellation_rolls_back_before_queue_submission() {
    run_initial_admission_rejection(AdmissionRejection::Cancellation);
}

#[test]
fn initial_admission_deadline_rolls_back_before_queue_submission() {
    run_initial_admission_rejection(AdmissionRejection::Deadline);
}

#[test]
fn promoted_retry_admission_cancellation_rolls_back_before_queue_submission() {
    run_promoted_retry_admission_rejection(AdmissionRejection::Cancellation);
}

#[test]
fn promoted_retry_admission_deadline_rolls_back_before_queue_submission() {
    run_promoted_retry_admission_rejection(AdmissionRejection::Deadline);
}

#[test]
fn activation_allocator_exhaustion_is_typed_without_global_contamination() {
    let allocator = ActivationIdAllocator::for_test(NonZeroU64::new(u64::MAX).unwrap());
    let execution_plan = plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([])));
    let kernels = KernelRegistry::new();
    let resources = no_resources();
    let executor = RunExecutor::new(&kernels, &resources, &NoFunctions)
        .with_activation_allocator_for_test(&allocator);

    executor
        .run(&execution_plan, CancellationToken::new())
        .unwrap();
    assert_eq!(
        executor.run(&execution_plan, CancellationToken::new()),
        Err(RunError::ActivationIdExhausted)
    );
}

#[test]
fn retry_backoff_is_exponential_capped_and_overflow_safe() {
    let policy = RetryPolicy::new(
        NonZeroU32::new(10).unwrap(),
        Duration::from_millis(3),
        Duration::from_millis(10),
    )
    .unwrap();

    assert_eq!(
        super::scheduler::retry_backoff(policy, AttemptId::new(1)),
        Duration::from_millis(3)
    );
    assert_eq!(
        super::scheduler::retry_backoff(policy, AttemptId::new(2)),
        Duration::from_millis(6)
    );
    assert_eq!(
        super::scheduler::retry_backoff(policy, AttemptId::new(3)),
        Duration::from_millis(10)
    );
    assert_eq!(
        super::scheduler::retry_backoff(policy, AttemptId::new(u64::MAX)),
        Duration::from_millis(10)
    );
}

#[test]
fn retry_transient_failure_then_success_publishes_only_final_output() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_transient_success", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                if observed.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(KernelError::transient("try again"))
                } else {
                    Ok(vec![Value::Integer(42).into()])
                }
            }),
        )
        .unwrap();
    let events = RecordingRunEvents::default();
    let trace = RecordingTrace::default();
    let results = ResultStore::new();

    let execution_plan = retry_plan("retry_transient_success", 3, Duration::ZERO);
    execution_plan.validate().unwrap();
    let result = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_event_sink(&events)
        .with_trace_sink(&trace)
        .with_result_store(&results)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(result.values["result"], Value::Integer(42).into());
    let events = events.0.lock().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.kind, RunEventKind::OperationCompleted { .. }))
            .count(),
        1
    );
    drop(events);
    let spans = trace.0.lock().unwrap();
    let run = spans
        .iter()
        .find(|span| span.kind == SpanKind::Run)
        .unwrap();
    let attempts = spans
        .iter()
        .filter(|span| span.kind == SpanKind::OperationAttempt)
        .collect::<Vec<_>>();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].outcome, SpanOutcome::Retry);
    assert_eq!(attempts[1].outcome, SpanOutcome::Success);
    assert_eq!(attempts[0].attempt_id, Some(AttemptId::new(1)));
    assert_eq!(attempts[1].attempt_id, Some(AttemptId::new(2)));
    assert_eq!(attempts[0].operation_id, attempts[1].operation_id);
    assert_eq!(attempts[0].run_id, attempts[1].run_id);
    assert!(
        attempts
            .iter()
            .all(|span| span.parent_span_id == Some(run.span_id))
    );
    for kind in [
        SpanKind::ResourceAcquire,
        SpanKind::ResultPublication,
        SpanKind::Cleanup,
    ] {
        assert!(
            spans
                .iter()
                .any(|span| span.kind == kind && span.parent_span_id == Some(run.span_id))
        );
    }
}

#[test]
fn retry_permanent_error_never_retries() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_permanent", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Err(KernelError::new("permanent"))
            }),
        )
        .unwrap();

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(
            &retry_plan("retry_permanent", 3, Duration::ZERO),
            CancellationToken::new(),
        )
        .unwrap_err();

    assert!(matches!(error, RunError::KernelFailed { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn retry_max_attempts_includes_initial_attempt() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_exact_max", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Err(KernelError::transient("still transient"))
            }),
        )
        .unwrap();

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(
            &retry_plan("retry_exact_max", 3, Duration::ZERO),
            CancellationToken::new(),
        )
        .unwrap_err();

    assert!(matches!(error, RunError::KernelFailed { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[test]
fn retry_insufficient_deadline_returns_typed_deadline_without_next_attempt() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_deadline", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Err(KernelError::transient("retry later"))
            }),
        )
        .unwrap();

    let trace = RecordingTrace::default();
    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_trace_sink(&trace)
        .with_deadline(RunDeadline::after(Duration::from_millis(10)))
        .run(
            &retry_plan("retry_deadline", 3, Duration::from_millis(100)),
            CancellationToken::new(),
        )
        .unwrap_err();

    assert_eq!(
        error,
        RunError::DeadlineExceeded {
            phase: RunPhase::QueueWait,
        }
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let spans = trace.0.lock().unwrap();
    assert!(spans.iter().any(|span| {
        span.kind == SpanKind::OperationAttempt && span.outcome == SpanOutcome::Retry
    }));
    assert!(
        spans
            .iter()
            .any(|span| span.kind == SpanKind::Run && span.outcome == SpanOutcome::Timeout)
    );
    assert!(spans.iter().any(|span| matches!(
        (&span.kind, &span.outcome),
        (SpanKind::Cleanup, SpanOutcome::Cleanup { .. })
    )));
}

#[test]
fn retry_cancellation_during_backoff_wakes_promptly() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_cancel_backoff", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Err(KernelError::transient("retry later"))),
        )
        .unwrap();
    let cancellation = CancellationToken::new();
    let started = Instant::now();

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_test_checkpoint(Arc::new(|checkpoint, cancellation| {
            if matches!(checkpoint, SchedulerCheckpoint::RetryBackoff { .. }) {
                cancellation.cancel();
            }
        }))
        .run(
            &retry_plan("retry_cancel_backoff", 3, Duration::from_secs(5)),
            cancellation,
        )
        .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
    assert!(started.elapsed() < Duration::from_millis(200));
}

struct RetryIdentityKernel {
    calls: AtomicUsize,
    activations: Arc<Mutex<Vec<ActivationId>>>,
}

impl Kernel for RetryIdentityKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        self.activations.lock().unwrap().push(context.activation_id);
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            Err(KernelError::transient("retry with fresh identity"))
        } else {
            Ok(vec![Value::Integer(7).into()])
        }
    }
}

#[test]
fn retry_attempts_use_distinct_attempt_and_activation_with_stable_operation() {
    let activations = Arc::new(Mutex::new(Vec::new()));
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_identity", KernelHandle::new),
            RetryIdentityKernel {
                calls: AtomicUsize::new(0),
                activations: Arc::clone(&activations),
            },
        )
        .unwrap();
    let observed_attempts = Arc::clone(&attempts);
    let events = RecordingRunEvents::default();

    RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_event_sink(&events)
        .with_test_checkpoint(Arc::new(move |checkpoint, _| {
            if let SchedulerCheckpoint::AttemptPrepared {
                operation,
                activation,
                attempt,
            } = checkpoint
            {
                observed_attempts
                    .lock()
                    .unwrap()
                    .push((operation, activation, attempt));
            }
        }))
        .run(
            &retry_plan("retry_identity", 2, Duration::ZERO),
            CancellationToken::new(),
        )
        .unwrap();

    let activations = activations.lock().unwrap();
    assert_eq!(activations.len(), 2);
    assert_ne!(activations[0], activations[1]);
    let attempts = attempts.lock().unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].0, OperationIndex::new(0));
    assert_eq!(attempts[1].0, OperationIndex::new(0));
    assert_eq!(attempts[0].2, AttemptId::new(1));
    assert_eq!(attempts[1].2, AttemptId::new(2));
    assert_eq!(attempts[0].1, activations[0]);
    assert_eq!(attempts[1].1, activations[1]);
    let event_attempts = events
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event.kind {
            RunEventKind::OperationStarted { attempt_id, .. } => Some(attempt_id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(event_attempts, vec![1, 2]);
}

#[test]
fn retry_runtime_defense_rejects_malformed_side_effect_plan() {
    let mut unsafe_operation = adapter_operation(
        "test.unsafe.retry",
        0,
        1,
        OutputProduction::Streaming,
        InputConsumption::FullyMaterialized,
    );
    unsafe_operation.retry = PlannedRetry {
        idempotent: true,
        policy: Some(retry_policy(2, Duration::ZERO)),
    };
    let mut execution_plan = plan(
        vec![unsafe_operation],
        2,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.value_sources = Box::new([PlanValueSource::ExternalInput(
        ValueRef::new(0),
        OutputProduction::Streaming,
    )]);

    assert!(
        execution_plan
            .validate()
            .unwrap_err()
            .0
            .iter()
            .any(|error| {
                matches!(error, PlanValidationError::InvalidRetryPolicy { operation }
            if *operation == OperationIndex::new(0))
            })
    );
    let error = RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, RunError::InvalidPlan(_)));
}

#[test]
fn call_failure_releases_caller_and_callee_resources() {
    let mut callee = plan(
        vec![operation("missing", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    callee.resources = Box::new([requirement("callee")]);
    let mut caller = plan(
        vec![],
        0,
        StructuredControlRegion::Call {
            target: id("functions/callee", FunctionPlanHandle::new),
            arguments: Box::new([]),
            results: Box::new([]),
            mandatory: true,
        },
    );
    caller.resources = Box::new([requirement("caller")]);
    let resources = no_resources();
    let released = resources.released.clone();

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &resources,
        &OneFunction(published_function(callee, "functions/callee", &[], &[])),
    )
    .run(&caller, CancellationToken::new())
    .unwrap_err();

    assert!(matches!(error, RunError::KernelNotFound(_)));
    assert_eq!(released.load(Ordering::SeqCst), 2);
}

#[test]
fn bounded_channel_applies_backpressure() {
    let (sender, receiver) = bounded_stream_channel(1, CancellationToken::new()).unwrap();

    sender.try_send(Value::Integer(1)).unwrap();
    assert_eq!(
        sender.try_send(Value::Integer(2)),
        Err(StreamSendError::Full(Value::Integer(2)))
    );
    assert_eq!(receiver.recv().unwrap(), Value::Integer(1));
    sender.try_send(Value::Integer(2)).unwrap();
    assert_eq!(receiver.recv().unwrap(), Value::Integer(2));
}

fn relational_operation(subplan: u32, outputs: &[u32]) -> PlannedOperation {
    let mut operation = operation("relational", &[], outputs);
    operation.stable_id =
        OperationStableId::new(format!("test.operation.relational.{subplan}")).unwrap();
    operation.kernel = PlannedKernel::Relational(RelationalSubplanIndex::new(subplan));
    operation
}

fn relational_subplan(backend: &str, fragment: &str, _: Box<[()]>) -> RelationalSubplan {
    RelationalSubplan {
        backend: id(backend, RelationalBackendId::new),
        compiled_plan: CompiledRelationalPlan {
            fragment_order: Box::new([id(fragment, RelationalFragmentId::new)]),
            operators: Box::new([RelationalOperator::Input {
                name: fragment.into(),
            }]),
            fragment_roots: Box::new([crate::node_system::plan::RelationalFragmentRoot {
                fragment: id(fragment, RelationalFragmentId::new),
                operator: RelationalOperatorIndex::new(0),
            }]),
            roots: Box::new([RelationalOperatorIndex::new(0)]),
            pushdown_hints: Box::new([]),
        },
    }
}

struct RecordingRelationalBackend {
    executions: Arc<Mutex<Vec<Box<str>>>>,
}

impl RelationalBackend for RecordingRelationalBackend {
    fn execute(
        &self,
        context: &RelationalContext<'_>,
        plan: &CompiledRelationalPlan,
        operation_inputs: &[RuntimeValue],
    ) -> Result<RelationalExecution, RelationalError> {
        context
            .cancellation
            .check()
            .map_err(RelationalError::from)?;
        assert!(operation_inputs.is_empty());
        self.executions
            .lock()
            .unwrap()
            .push(plan.fragment_order[0].as_str().into());
        Ok(RelationalExecution {
            outputs: vec![Value::Integer(41).into()],
        })
    }
}

#[derive(Clone, Copy)]
enum TraceRelationalOutcome {
    Succeed,
    Fail,
    Cancel,
}

struct TraceRelationalBackend(TraceRelationalOutcome);

struct OrdinaryErrorAfterCancellationBackend;

impl RelationalBackend for OrdinaryErrorAfterCancellationBackend {
    fn execute(
        &self,
        context: &RelationalContext<'_>,
        _: &CompiledRelationalPlan,
        _: &[RuntimeValue],
    ) -> Result<RelationalExecution, RelationalError> {
        context.cancellation.cancel();
        Err(RelationalError::operator_invalid(
            "ordinary backend failure won the boundary",
        ))
    }
}

impl RelationalBackend for TraceRelationalBackend {
    fn execute(
        &self,
        context: &RelationalContext<'_>,
        _: &CompiledRelationalPlan,
        _: &[RuntimeValue],
    ) -> Result<RelationalExecution, RelationalError> {
        match self.0 {
            TraceRelationalOutcome::Succeed => Ok(RelationalExecution {
                outputs: vec![Value::Integer(41).into()],
            }),
            TraceRelationalOutcome::Fail => {
                Err(RelationalError::operator_invalid("backend failed"))
            }
            TraceRelationalOutcome::Cancel => {
                context.cancellation.cancel();
                Err(RelationalError::cancelled(
                    "relational execution was cancelled",
                ))
            }
        }
    }
}

fn run_relational_backend_trace(
    outcome: TraceRelationalOutcome,
) -> (Result<RunResult, RunError>, ExecutionPlan, Vec<TraceSpan>) {
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("trace-backend", RelationalBackendId::new),
            TraceRelationalBackend(outcome),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.relational_subplans = Box::new([relational_subplan(
        "trace-backend",
        "private-fragment",
        Box::new([]),
    )]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    let trace = RecordingTrace::default();

    let result = RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
        .with_relational_backends(&relational)
        .with_trace_sink(&trace)
        .run(&execution_plan, CancellationToken::new());
    let events = trace.0.into_inner().unwrap();
    (result, execution_plan, events)
}

struct OwnerThreadTrace {
    owner: thread::ThreadId,
    off_owner_calls: AtomicUsize,
    events: Mutex<Vec<TraceSpan>>,
}

impl OwnerThreadTrace {
    fn current() -> Self {
        Self {
            owner: thread::current().id(),
            off_owner_calls: AtomicUsize::new(0),
            events: Mutex::new(Vec::new()),
        }
    }
}

impl TraceSink for OwnerThreadTrace {
    fn start_span(&self, spec: SpanSpec) -> SpanGuard<'_> {
        SpanGuard::new(self, spec, &SYSTEM_TRACE_CLOCK)
    }

    fn complete_span(&self, span: TraceSpan) {
        if thread::current().id() != self.owner {
            self.off_owner_calls.fetch_add(1, Ordering::SeqCst);
        }
        self.events.lock().unwrap().push(span);
    }
}

#[test]
fn relational_cancellation_installed_before_ordinary_error_wins() {
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("ordinary-after-cancel", RelationalBackendId::new),
            OrdinaryErrorAfterCancellationBackend,
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.relational_subplans = Box::new([relational_subplan(
        "ordinary-after-cancel",
        "ordinary-fragment",
        Box::new([]),
    )]);

    let error = RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
        .with_relational_backends(&relational)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
}

struct SynchronizedSuccessBackend {
    started: mpsc::SyncSender<()>,
    release: Mutex<mpsc::Receiver<()>>,
}

impl RelationalBackend for SynchronizedSuccessBackend {
    fn execute(
        &self,
        _: &RelationalContext<'_>,
        _: &CompiledRelationalPlan,
        _: &[RuntimeValue],
    ) -> Result<RelationalExecution, RelationalError> {
        self.started.send(()).unwrap();
        self.release.lock().unwrap().recv().unwrap();
        Ok(RelationalExecution {
            outputs: vec![Value::Integer(7).into()],
        })
    }
}

fn synchronized_relational_plan(backend: &str) -> ExecutionPlan {
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.relational_subplans = Box::new([relational_subplan(
        backend,
        "synchronized-fragment",
        Box::new([]),
    )]);
    execution_plan
}

fn assert_deadline_worker_trace(spans: &[TraceSpan], attempt_outcome: SpanOutcome) {
    let run = spans
        .iter()
        .find(|span| span.kind == SpanKind::Run)
        .unwrap();
    let attempt = spans
        .iter()
        .filter(|span| span.kind == SpanKind::OperationAttempt)
        .collect::<Vec<_>>();
    let adapter = spans
        .iter()
        .filter(|span| span.kind == SpanKind::AdapterIo)
        .collect::<Vec<_>>();
    assert_eq!(attempt.len(), 1);
    assert_eq!(adapter.len(), 1);
    assert_eq!(attempt[0].parent_span_id, Some(run.span_id));
    assert_eq!(attempt[0].outcome, attempt_outcome);
    assert_eq!(adapter[0].parent_span_id, Some(attempt[0].span_id));
    assert_eq!(adapter[0].outcome, SpanOutcome::Success);
    assert_eq!(adapter[0].operation_id, attempt[0].operation_id);
    assert_eq!(adapter[0].activation_id, attempt[0].activation_id);
    assert_eq!(adapter[0].attempt_id, attempt[0].attempt_id);
    for kind in [
        SpanKind::ResourceAcquire,
        SpanKind::ResultPublication,
        SpanKind::Cleanup,
    ] {
        let phase = spans
            .iter()
            .filter(|span| span.kind == kind)
            .collect::<Vec<_>>();
        assert_eq!(phase.len(), 1, "expected exactly one {kind:?} span");
        assert_eq!(phase[0].parent_span_id, Some(run.span_id));
    }
    let ids = spans
        .iter()
        .map(|span| span.span_id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        ids.len(),
        spans.len(),
        "completed spans must be forwarded once"
    );
}

#[test]
fn deadline_before_envelope_receive_forwards_worker_spans_once() {
    let trace = RecordingTrace::default();
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("deadline-before-receive", RelationalBackendId::new),
            TraceRelationalBackend(TraceRelationalOutcome::Succeed),
        )
        .unwrap();
    let execution_plan = synchronized_relational_plan("deadline-before-receive");
    let (produced_tx, produced_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let release_rx = Arc::new(Mutex::new(release_rx));
    let checkpoint_release = Arc::clone(&release_rx);

    let result = thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
                .with_relational_backends(&relational)
                .with_trace_sink(&trace)
                .with_deadline(RunDeadline::after(Duration::from_millis(20)))
                .with_test_checkpoint(Arc::new(move |checkpoint, _| {
                    if checkpoint == SchedulerCheckpoint::WorkerOutcomeProduced {
                        produced_tx.send(()).unwrap();
                        checkpoint_release.lock().unwrap().recv().unwrap();
                    }
                }))
                .run(&execution_plan, CancellationToken::new())
        });
        produced_rx.recv().unwrap();
        thread::sleep(Duration::from_millis(40));
        release_tx.send(()).unwrap();
        run.join().unwrap()
    });

    assert_eq!(
        result,
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::Kernel,
        })
    );
    assert_deadline_worker_trace(&trace.0.lock().unwrap(), SpanOutcome::Success);
}

#[test]
fn completion_after_deadline_forwards_worker_spans_once() {
    let trace = RecordingTrace::default();
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("completion-after-deadline", RelationalBackendId::new),
            SynchronizedSuccessBackend {
                started: started_tx,
                release: Mutex::new(release_rx),
            },
        )
        .unwrap();
    let execution_plan = synchronized_relational_plan("completion-after-deadline");

    let result = thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
                .with_relational_backends(&relational)
                .with_trace_sink(&trace)
                .with_deadline(RunDeadline::after(Duration::from_millis(20)))
                .run(&execution_plan, CancellationToken::new())
        });
        started_rx.recv().unwrap();
        thread::sleep(Duration::from_millis(40));
        release_tx.send(()).unwrap();
        run.join().unwrap()
    });

    assert_eq!(
        result,
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::Kernel,
        })
    );
    assert_deadline_worker_trace(&trace.0.lock().unwrap(), SpanOutcome::Timeout);
}

#[test]
fn retryable_failure_after_deadline_keeps_retry_attempt_truth() {
    let trace = RecordingTrace::default();
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let release_rx = Arc::new(Mutex::new(release_rx));
    let kernel_release = Arc::clone(&release_rx);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("retry_truth", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                started_tx.send(()).unwrap();
                kernel_release.lock().unwrap().recv().unwrap();
                Err(KernelError::transient("retry truth"))
            }),
        )
        .unwrap();
    let execution_plan = retry_plan("retry_truth", 2, Duration::ZERO);

    let result = thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
                .with_trace_sink(&trace)
                .with_deadline(RunDeadline::after(Duration::from_millis(20)))
                .run(&execution_plan, CancellationToken::new())
        });
        started_rx.recv().unwrap();
        thread::sleep(Duration::from_millis(40));
        release_tx.send(()).unwrap();
        run.join().unwrap()
    });

    assert!(matches!(result, Err(RunError::DeadlineExceeded { .. })));
    let attempts = trace
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|span| span.kind == SpanKind::OperationAttempt)
        .map(|span| span.outcome.clone())
        .collect::<Vec<_>>();
    assert_eq!(attempts, [SpanOutcome::Retry]);
}

#[test]
fn success_completed_after_cancellation_rewrites_attempt_but_preserves_adapter_truth() {
    let trace = RecordingTrace::default();
    let cancellation = CancellationToken::new();
    let run_cancellation = cancellation.clone();
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("success_after_cancel", RelationalBackendId::new),
            SynchronizedSuccessBackend {
                started: started_tx,
                release: Mutex::new(release_rx),
            },
        )
        .unwrap();
    let execution_plan = synchronized_relational_plan("success_after_cancel");

    let result = thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
                .with_relational_backends(&relational)
                .with_trace_sink(&trace)
                .run(&execution_plan, run_cancellation)
        });
        started_rx.recv().unwrap();
        cancellation.cancel();
        release_tx.send(()).unwrap();
        run.join().unwrap()
    });

    assert_eq!(result, Err(RunError::Cancelled));
    let spans = trace.0.lock().unwrap();
    assert!(spans.iter().any(|span| {
        span.kind == SpanKind::OperationAttempt && span.outcome == SpanOutcome::Cancellation
    }));
    assert!(spans.iter().any(|span| {
        span.kind == SpanKind::AdapterIo && span.outcome == SpanOutcome::Cancellation
    }));
}

#[test]
fn success_completed_before_cancellation_keeps_attempt_truth_while_envelope_drains() {
    let trace = RecordingTrace::default();
    let cancellation = CancellationToken::new();
    let run_cancellation = cancellation.clone();
    let (produced_tx, produced_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let release_rx = Arc::new(Mutex::new(release_rx));
    let checkpoint_release = Arc::clone(&release_rx);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("success_before_cancel", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![])),
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("success_before_cancel", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    let result = thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
                .with_trace_sink(&trace)
                .with_test_checkpoint(Arc::new(move |checkpoint, _| {
                    if checkpoint == SchedulerCheckpoint::WorkerOutcomeProduced {
                        produced_tx.send(()).unwrap();
                        checkpoint_release.lock().unwrap().recv().unwrap();
                    }
                }))
                .run(&execution_plan, run_cancellation)
        });
        produced_rx.recv().unwrap();
        cancellation.cancel();
        release_tx.send(()).unwrap();
        run.join().unwrap()
    });

    assert_eq!(result, Err(RunError::Cancelled));
    let attempts = trace
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|span| span.kind == SpanKind::OperationAttempt)
        .map(|span| span.outcome.clone())
        .collect::<Vec<_>>();
    assert_eq!(attempts, [SpanOutcome::Success]);
}

#[test]
fn panic_attempt_truth_survives_deadline_and_cancellation() {
    for terminal in ["deadline", "cancellation"] {
        let trace = RecordingTrace::default();
        let cancellation = CancellationToken::new();
        let run_cancellation = cancellation.clone();
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let release_rx = Arc::new(Mutex::new(release_rx));
        let kernel_release = Arc::clone(&release_rx);
        let mut kernels = KernelRegistry::new();
        kernels
            .register(
                id("panic_terminal_truth", KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| {
                    started_tx.send(()).unwrap();
                    kernel_release.lock().unwrap().recv().unwrap();
                    panic!("panic terminal truth sentinel")
                }),
            )
            .unwrap();
        let execution_plan = plan(
            vec![operation("panic_terminal_truth", &[], &[])],
            0,
            StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(
                OperationIndex::new(0),
            )])),
        );

        let panic = thread::scope(|scope| {
            let run = scope.spawn(|| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let resources = no_resources();
                    let mut executor = RunExecutor::new(&kernels, &resources, &NoFunctions)
                        .with_trace_sink(&trace);
                    if terminal == "deadline" {
                        executor =
                            executor.with_deadline(RunDeadline::after(Duration::from_millis(20)));
                    }
                    let _ = executor.run(&execution_plan, run_cancellation);
                }))
            });
            started_rx.recv().unwrap();
            if terminal == "deadline" {
                thread::sleep(Duration::from_millis(40));
            } else {
                cancellation.cancel();
            }
            release_tx.send(()).unwrap();
            run.join().unwrap()
        });
        assert!(panic.is_err());
        assert!(trace.0.lock().unwrap().iter().any(|span| {
            span.kind == SpanKind::OperationAttempt && span.outcome == SpanOutcome::InternalAborted
        }));
    }
}

#[test]
fn peer_ordinary_error_does_not_rewrite_drained_success_attempt() {
    let trace = RecordingTrace::default();
    let entered = Arc::new(Barrier::new(3));
    let success_thread = Arc::new(Mutex::new(None));
    let (produced_tx, produced_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let release_rx = Arc::new(Mutex::new(release_rx));
    let mut kernels = KernelRegistry::new();
    let error_entered = Arc::clone(&entered);
    kernels
        .register(
            id("parallel0", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                error_entered.wait();
                thread::sleep(Duration::from_millis(20));
                Err(KernelError::new("peer ordinary error"))
            }),
        )
        .unwrap();
    let success_entered = Arc::clone(&entered);
    let worker_thread = Arc::clone(&success_thread);
    kernels
        .register(
            id("parallel1", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                success_entered.wait();
                *worker_thread.lock().unwrap() = Some(thread::current().id());
                Ok(vec![Value::Integer(1).into()])
            }),
        )
        .unwrap();
    let execution_plan = independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Cpu]);

    let checkpoint_thread = Arc::clone(&success_thread);
    let checkpoint_release = Arc::clone(&release_rx);
    let error = thread::scope(|scope| {
        let run = scope.spawn(|| {
            RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
                .with_trace_sink(&trace)
                .with_scheduling_policy(parallel_policy(2, 1, 1))
                .with_test_checkpoint(Arc::new(move |checkpoint, _| {
                    if checkpoint == SchedulerCheckpoint::WorkerOutcomeProduced
                        && checkpoint_thread.lock().unwrap().as_ref()
                            == Some(&thread::current().id())
                    {
                        produced_tx.send(()).unwrap();
                        checkpoint_release.lock().unwrap().recv().unwrap();
                    }
                }))
                .run(&execution_plan, CancellationToken::new())
        });
        entered.wait();
        produced_rx.recv().unwrap();
        thread::sleep(Duration::from_millis(40));
        release_tx.send(()).unwrap();
        run.join().unwrap().unwrap_err()
    });
    assert!(matches!(error, RunError::KernelFailed { .. }));
    let spans = trace.0.lock().unwrap();
    let success_operation = OperationStableId::new("test.operation.parallel1").unwrap();
    let attempts = spans
        .iter()
        .filter(|span| span.kind == SpanKind::OperationAttempt)
        .map(|span| (span.operation_id.clone(), span.outcome.clone()))
        .collect::<Vec<_>>();
    assert!(
        attempts.iter().any(|(operation, outcome)| {
            operation.as_ref() == Some(&success_operation) && *outcome == SpanOutcome::Success
        }),
        "attempts: {attempts:?}"
    );
}

#[test]
fn parallel_scheduler_workers_return_relational_trace_to_owner_thread() {
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("trace-owner", RelationalBackendId::new),
            TraceRelationalBackend(TraceRelationalOutcome::Succeed),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.relational_subplans = Box::new([relational_subplan(
        "trace-owner",
        "private-fragment",
        Box::new([]),
    )]);
    let trace = OwnerThreadTrace::current();

    RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
        .with_relational_backends(&relational)
        .with_trace_sink(&trace)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(trace.off_owner_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        trace
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|span| span.kind == SpanKind::AdapterIo)
            .count(),
        1
    );
}

fn assert_relational_backend_trace(
    execution_plan: &ExecutionPlan,
    spans: &[TraceSpan],
    terminal_outcome: SpanOutcome,
) {
    let spans = spans
        .iter()
        .filter(|span| span.kind == SpanKind::AdapterIo)
        .collect::<Vec<_>>();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].outcome, terminal_outcome);

    let correlation = &spans[0].correlation;
    assert_eq!(
        correlation.project_session_id,
        execution_plan.provenance.project_session_id
    );
    assert_eq!(correlation.graph_path, execution_plan.provenance.graph_path);
    assert_eq!(
        correlation.graph_revision,
        execution_plan.provenance.basis.graph_revision
    );
    assert_eq!(
        correlation.registry_fingerprint,
        execution_plan.provenance.basis.registry_fingerprint
    );
    assert_eq!(
        correlation.resource_versions,
        execution_plan.provenance.basis.resource_versions
    );
    assert_eq!(correlation.compile_id, execution_plan.provenance.compile_id);
    assert!(correlation.run_id.is_some());
    assert_eq!(
        correlation.node_id,
        Some(execution_plan.operations[0].source_node_id)
    );
    assert_eq!(
        correlation.node_type_id,
        Some(execution_plan.operations[0].source_node_type_id.clone())
    );
    assert_eq!(correlation.parent_call, None);
    assert_eq!(
        spans[0].operation_id.as_ref(),
        Some(&execution_plan.operations[0].stable_id)
    );
    assert!(spans[0].activation_id.is_some());
    assert_eq!(spans[0].attempt_id, Some(AttemptId::initial()));
}

#[test]
fn relational_backend_trace_records_success_with_full_operation_correlation() {
    let (result, execution_plan, events) =
        run_relational_backend_trace(TraceRelationalOutcome::Succeed);

    result.unwrap();
    assert_relational_backend_trace(&execution_plan, &events, SpanOutcome::Success);
}

#[test]
fn relational_backend_trace_records_failure_with_full_operation_correlation() {
    let (result, execution_plan, events) =
        run_relational_backend_trace(TraceRelationalOutcome::Fail);

    assert!(matches!(result, Err(RunError::RelationalFailed { .. })));
    assert_relational_backend_trace(&execution_plan, &events, SpanOutcome::Error);
}

#[test]
fn relational_backend_trace_records_cancellation_with_full_operation_correlation() {
    let (result, execution_plan, events) =
        run_relational_backend_trace(TraceRelationalOutcome::Cancel);

    assert_eq!(result, Err(RunError::Cancelled));
    assert_relational_backend_trace(&execution_plan, &events, SpanOutcome::Cancellation);
}

fn assert_production_source_cancellation(
    target: super::production_relational::ProductionRelationalCheckpoint,
    expected_checkpoints: &[super::production_relational::ProductionRelationalCheckpoint],
    expected_scan_limits: &[Option<usize>],
) {
    use polars::prelude::{Column, DataFrame};

    let resource = id("databases/main", ResourceId::new);
    let dataframe = DataFrame::new(2, vec![Column::new("value".into(), &[1_i64, 2])]).unwrap();
    let resource_versions = BTreeMap::from([(
        ResourceKey::new(resource.as_str()),
        ResourceVersion::new("1"),
    )]);
    let lease_observer = ProjectResourceLeaseObserver::default();
    let mut provider = ProjectResourceProvider::new(
        ProjectResourceSnapshot::new(
            ProjectSessionId::new("test-session"),
            resource_versions.clone(),
        )
        .with_database(resource.clone(), Arc::new(dataframe)),
    );
    provider.set_lease_observer(lease_observer.clone());
    let scan_limits = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let observed_for_hook = Arc::clone(&observed);
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("production", RelationalBackendId::new),
            ProductionRelationalBackend::recording_scan_limits(Arc::clone(&scan_limits))
                .with_test_checkpoint(Arc::new(move |checkpoint, cancellation| {
                    observed_for_hook.lock().unwrap().push(checkpoint);
                    if checkpoint == target {
                        cancellation.cancel();
                    }
                })),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.provenance.basis.resource_versions = resource_versions;
    execution_plan.resources = Box::new([CompiledResourceRequirement {
        resource: resource.clone(),
        kind: ResourceKind::DatabaseConnection,
        access: ResourceAccess::Shared,
        optional: false,
    }]);
    let source_fragment = id("source", RelationalFragmentId::new);
    execution_plan.relational_subplans = Box::new([RelationalSubplan {
        backend: id("production", RelationalBackendId::new),
        compiled_plan: CompiledRelationalPlan {
            fragment_order: Box::new([source_fragment.clone()]),
            operators: Box::new([RelationalOperator::Source {
                resource,
                relation: "main".into(),
            }]),
            fragment_roots: Box::new([RelationalFragmentRoot {
                fragment: source_fragment,
                operator: RelationalOperatorIndex::new(0),
            }]),
            roots: Box::new([RelationalOperatorIndex::new(0)]),
            pushdown_hints: Box::new([]),
        },
    }]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();

    let error = RunExecutor::new(&KernelRegistry::new(), &provider, &NoFunctions)
        .with_relational_backends(&relational)
        .with_event_sink(&events)
        .with_result_store(&results)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
    assert_eq!(observed.lock().unwrap().as_slice(), expected_checkpoints);
    assert_eq!(scan_limits.lock().unwrap().as_slice(), expected_scan_limits);
    assert_cancelled_without_completion(&events);
    assert_eq!(results.source_count(), 0);
    assert_eq!(lease_observer.acquired(), lease_observer.dropped());
    assert_eq!(lease_observer.active(), 0);
}

#[test]
fn cancellation_at_production_source_scan_stops_before_scan_and_publication() {
    use super::production_relational::ProductionRelationalCheckpoint;

    assert_production_source_cancellation(
        ProductionRelationalCheckpoint::SourceScan,
        &[
            ProductionRelationalCheckpoint::OperatorEvaluation,
            ProductionRelationalCheckpoint::SourceScan,
        ],
        &[],
    );
}

#[test]
fn cancellation_at_production_operator_evaluation_stops_before_dependencies_and_publication() {
    use super::production_relational::ProductionRelationalCheckpoint;

    assert_production_source_cancellation(
        ProductionRelationalCheckpoint::OperatorEvaluation,
        &[ProductionRelationalCheckpoint::OperatorEvaluation],
        &[],
    );
}

#[test]
fn cancellation_at_production_result_materialization_prevents_publication_and_completion() {
    use super::production_relational::ProductionRelationalCheckpoint;

    assert_production_source_cancellation(
        ProductionRelationalCheckpoint::ResultMaterialization,
        &[
            ProductionRelationalCheckpoint::OperatorEvaluation,
            ProductionRelationalCheckpoint::SourceScan,
            ProductionRelationalCheckpoint::ResultMaterialization,
        ],
        &[None],
    );
}

#[test]
fn cancellation_during_production_result_conversion_stops_without_publication_or_leaks() {
    use super::production_relational::ProductionRelationalCheckpoint;

    assert_production_source_cancellation(
        ProductionRelationalCheckpoint::ResultConversion,
        &[
            ProductionRelationalCheckpoint::OperatorEvaluation,
            ProductionRelationalCheckpoint::SourceScan,
            ProductionRelationalCheckpoint::ResultMaterialization,
            ProductionRelationalCheckpoint::ResultConversion,
        ],
        &[None],
    );
}

#[test]
fn invalid_pushdown_plan_is_rejected_before_relational_backend_execution() {
    let executions = Arc::new(Mutex::new(Vec::new()));
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("production", RelationalBackendId::new),
            RecordingRelationalBackend {
                executions: Arc::clone(&executions),
            },
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    let mut subplan = relational_subplan("production", "source", Box::new([]));
    subplan.compiled_plan.operators = Box::new([
        RelationalOperator::Source {
            resource: id("database.main", ResourceId::new),
            relation: "items".into(),
        },
        RelationalOperator::Filter {
            input: RelationalOperatorIndex::new(0),
            predicate: RelationalExpression::Literal(RelationalLiteral::Boolean(true)),
        },
        RelationalOperator::Limit {
            input: RelationalOperatorIndex::new(1),
            rows: 25,
        },
    ]);
    subplan.compiled_plan.pushdown_hints = Box::new([RelationalPushdownHint::Limit {
        source: RelationalOperatorIndex::new(0),
        rows: 25,
    }]);
    execution_plan.relational_subplans = Box::new([subplan]);

    let error = RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
        .with_relational_backends(&relational)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, RunError::InvalidPlan(_)));
    assert!(executions.lock().unwrap().is_empty());
}

#[test]
fn relational_operation_executes_compiled_subplan_by_index() {
    let executions = Arc::new(Mutex::new(Vec::new()));
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("single", RelationalBackendId::new),
            RecordingRelationalBackend {
                executions: executions.clone(),
            },
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.relational_subplans =
        Box::new([relational_subplan("single", "sales", Box::new([]))]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);

    let result = RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
        .with_relational_backends(&relational)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(*executions.lock().unwrap(), vec![Box::<str>::from("sales")]);
    assert_eq!(
        result.values["result"],
        RuntimeValue::from(Value::Integer(41))
    );
}

#[test]
fn failed_success_finalizer_publishes_no_result_or_completion() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("value", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(1).into()])),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("value", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "value".into(),
        output: stable_output("value"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();
    let finalizer = |_: &mut RunResult, _: &CancellationToken, _: Option<RunDeadline>| {
        Err(RunError::ResourceSnapshotMismatch(
            "authoritative commit failed".into(),
        ))
    };

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_event_sink(&events)
        .with_result_store(&results)
        .with_success_finalizer(&finalizer)
        .run(&execution_plan, CancellationToken::new())
        .expect_err("failed authoritative finalization must fail the run");

    assert!(matches!(error, RunError::ResourceSnapshotMismatch(_)));
    let recorded = events.0.lock().unwrap();
    assert!(recorded.iter().any(|event| matches!(
        event.kind,
        RunEventKind::RunErrored {
            outcome: RunErrorOutcome::Ordinary {
                code: OrdinaryRunErrorCode::ResourceSnapshotMismatch,
            },
        }
    )));
    assert!(
        recorded
            .iter()
            .all(|event| event.kind != RunEventKind::RunCompleted)
    );
    assert!(recorded.iter().all(|event| !matches!(
        event.kind,
        RunEventKind::ResultReady { .. } | RunEventKind::OutputReady { .. }
    )));
    assert_eq!(results.source_count(), 0);
}

fn publication_transaction_fixture() -> (KernelRegistry, ExecutionPlan) {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("transactional_value", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(1).into()])),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("transactional_value", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "value".into(),
        output: stable_output("transactional_value"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    (kernels, execution_plan)
}

fn seed_result_source(
    kernels: &KernelRegistry,
    execution_plan: &ExecutionPlan,
    results: &ResultStore,
) -> ResultSourceId {
    let events = RecordingRunEvents::default();
    RunExecutor::new(kernels, &no_resources(), &NoFunctions)
        .with_result_store(results)
        .with_event_sink(&events)
        .run(execution_plan, CancellationToken::new())
        .unwrap();
    events
        .0
        .lock()
        .unwrap()
        .iter()
        .find_map(|event| match event.kind {
            RunEventKind::ResultReady { source_id, .. } => Some(source_id),
            _ => None,
        })
        .expect("seed run publishes one result source")
}

#[test]
fn result_publication_error_preserves_capacity_eviction_candidate() {
    let trace = RecordingTrace::default();
    let (kernels, execution_plan) = publication_transaction_fixture();
    let results = ResultStore::with_capacity(1);
    let prior_source = seed_result_source(&kernels, &execution_plan, &results);
    let finalizer = |_: &mut RunResult, _: &CancellationToken, _: Option<RunDeadline>| {
        Err(RunError::ResourceSnapshotMismatch(
            "finalizer failed".into(),
        ))
    };

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_result_store(&results)
        .with_atomic_success_finalizer(&finalizer)
        .with_trace_sink(&trace)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, RunError::ResourceSnapshotMismatch(_)));
    assert_run_phase_coverage(
        &trace.0.lock().unwrap(),
        SpanOutcome::Success,
        SpanOutcome::Error,
    );
    assert_eq!(results.source_count(), 1);
    assert_eq!(results.artifact_count_for_test(), 1);
    assert!(results.descriptor(prior_source).is_some());
}

#[test]
fn successful_atomic_result_publication_is_invisible_and_non_evicting_until_final_gate() {
    let (kernels, execution_plan) = publication_transaction_fixture();
    let results = ResultStore::with_capacity(1);
    let prior_source = seed_result_source(&kernels, &execution_plan, &results);
    let finalizer = |_: &mut RunResult, _: &CancellationToken, _: Option<RunDeadline>| Ok(());

    RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_result_store(&results)
        .with_atomic_success_finalizer(&finalizer)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(results.source_count(), 1);
    assert_eq!(results.artifact_count_for_test(), 1);
    assert!(results.descriptor(prior_source).is_none());
}

#[test]
fn result_publication_deadline_preserves_capacity_eviction_candidate() {
    let (kernels, execution_plan) = publication_transaction_fixture();
    let results = ResultStore::with_capacity(1);
    let prior_source = seed_result_source(&kernels, &execution_plan, &results);
    let finalizer =
        |_: &mut RunResult, cancellation: &CancellationToken, deadline: Option<RunDeadline>| {
            thread::sleep(Duration::from_millis(20));
            deadline
                .expect("deadline configured")
                .check(cancellation, RunPhase::ResultPublication)
        };

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_result_store(&results)
        .with_atomic_success_finalizer(&finalizer)
        .with_deadline(RunDeadline::after(Duration::from_millis(5)))
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert_eq!(
        error,
        RunError::DeadlineExceeded {
            phase: RunPhase::ResultPublication,
        }
    );
    assert_eq!(results.source_count(), 1);
    assert_eq!(results.artifact_count_for_test(), 1);
    assert!(results.descriptor(prior_source).is_some());
}

#[test]
fn result_publication_finalizer_panic_preserves_prior_source_and_leaks_no_handle() {
    let (kernels, execution_plan) = publication_transaction_fixture();
    let results = ResultStore::with_capacity(1);
    let prior_source = seed_result_source(&kernels, &execution_plan, &results);
    let events = RecordingRunEvents::default();
    let finalizer = |_: &mut RunResult, _: &CancellationToken, _: Option<RunDeadline>| {
        panic!("finalizer panic sentinel")
    };

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
            .with_result_store(&results)
            .with_event_sink(&events)
            .with_atomic_success_finalizer(&finalizer)
            .run(&execution_plan, CancellationToken::new());
    }));

    assert!(panic.is_err());
    assert_eq!(results.source_count(), 1);
    assert_eq!(results.artifact_count_for_test(), 1);
    assert!(results.descriptor(prior_source).is_some());
    assert!(events.0.lock().unwrap().iter().all(|event| !matches!(
        event.kind,
        RunEventKind::ResultReady { .. }
            | RunEventKind::OutputReady { .. }
            | RunEventKind::RunCompleted
    )));
}

#[test]
fn cancellation_after_first_multi_result_source_is_staged_publishes_nothing() {
    use super::scheduler::SchedulerCheckpoint;

    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("pair", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| {
                Ok(vec![
                    RuntimeValue::from(Value::Integer(1)),
                    RuntimeValue::from(Value::Integer(2)),
                ])
            }),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("pair", &[], &[0, 1])],
        2,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([
        PlanResult {
            name: "first".into(),
            output: stable_output("first"),
            value: ValueRef::new(0),
        },
        PlanResult {
            name: "second".into(),
            output: stable_output("second"),
            value: ValueRef::new(1),
        },
    ]);
    execution_plan.publications = Box::new([
        PlannedPublication::GraphResult {
            name: "first".into(),
            output: stable_output("first"),
            value: ValueRef::new(0),
        },
        PlannedPublication::GraphResult {
            name: "second".into(),
            output: stable_output("second"),
            value: ValueRef::new(1),
        },
    ]);
    let events = RecordingRunEvents::default();
    let results = ResultStore::with_capacity(1);
    let old_run_id = RunId::new(90_001);
    let old_correlation =
        CorrelationContext::compile(&execution_plan.provenance).for_run(old_run_id, None);
    let old_descriptor = results.publish_snapshot(
        old_run_id,
        old_correlation,
        execution_plan.provenance.basis.clone(),
        "committed",
        ArtifactSnapshot::Value(Value::String("keep".into())),
    );
    let old_value = results.value(old_descriptor.source_id).unwrap();
    let staged = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&staged);

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_event_sink(&events)
        .with_result_store(&results)
        .with_test_checkpoint(Arc::new(move |checkpoint, cancellation| {
            if checkpoint == SchedulerCheckpoint::ResultSourceStaged
                && observed.fetch_add(1, Ordering::SeqCst) == 0
            {
                cancellation.cancel();
            }
        }))
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
    assert_eq!(staged.load(Ordering::SeqCst), 1);
    assert_cancelled_without_completion(&events);
    assert_eq!(results.source_count(), 1);
    assert_eq!(
        results.descriptor(old_descriptor.source_id),
        Some(old_descriptor.clone())
    );
    assert_eq!(results.value(old_descriptor.source_id), Some(old_value));
}

#[test]
fn cancellation_after_operation_output_preserves_an_older_committed_source() {
    let cancellation = CancellationToken::new();
    let kernel_cancellation = cancellation.clone();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("value", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(1).into()])),
        )
        .unwrap();
    kernels
        .register(
            id("cancel", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                kernel_cancellation.cancel();
                Ok(Vec::new())
            }),
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("value", &[], &[0]), operation("cancel", &[], &[])],
        1,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    let events = RecordingRunEvents::default();
    let results = ResultStore::with_capacity(1);
    let old_run_id = RunId::new(90_002);
    let old_correlation =
        CorrelationContext::compile(&execution_plan.provenance).for_run(old_run_id, None);
    let old_descriptor = results.publish_snapshot(
        old_run_id,
        old_correlation,
        execution_plan.provenance.basis.clone(),
        "committed",
        ArtifactSnapshot::Value(Value::String("keep".into())),
    );
    let old_value = results.value(old_descriptor.source_id).unwrap();

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_event_sink(&events)
        .with_result_store(&results)
        .run(&execution_plan, cancellation)
        .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
    assert_cancelled_without_completion(&events);
    assert!(
        events
            .0
            .lock()
            .unwrap()
            .iter()
            .all(|event| { serde_json::to_value(&event.kind).unwrap()["type"] != "valueReady" })
    );
    assert_eq!(results.source_count(), 1);
    assert_eq!(
        results.descriptor(old_descriptor.source_id),
        Some(old_descriptor.clone())
    );
    assert_eq!(results.value(old_descriptor.source_id), Some(old_value));
}

#[test]
fn cancellation_before_final_result_publication_cleans_results_without_completion() {
    use super::scheduler::SchedulerCheckpoint;

    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("final_publication", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(1).into()])),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("final_publication", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();
    let final_checkpoints = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&final_checkpoints);

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_event_sink(&events)
        .with_result_store(&results)
        .with_test_checkpoint(Arc::new(move |checkpoint, cancellation| {
            if checkpoint == SchedulerCheckpoint::FinalResultPublication {
                observed.fetch_add(1, Ordering::SeqCst);
                cancellation.cancel();
            }
        }))
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
    assert_eq!(final_checkpoints.load(Ordering::SeqCst), 1);
    assert_cancelled_without_completion(&events);
    assert_eq!(results.source_count(), 0);
}

#[test]
fn scheduler_executes_only_the_planned_materialization_adapter() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("adapter_source", KernelHandle::new),
            OwnedStreamKernel {
                values: vec![Value::Integer(7), Value::Integer(8)].into_boxed_slice(),
                executions: None,
            },
        )
        .unwrap();
    kernels
        .register(
            id("adapter_sink", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| {
                assert!(matches!(
                    inputs,
                    [RuntimeValue::Artifact(artifact)]
                        if artifact.kind() == ArtifactKind::Collected
                            && artifact.cursor().unwrap().collect::<Result<Vec<_>, _>>().unwrap()
                                == [Value::Integer(7), Value::Integer(8)]
                ));
                Ok(vec![Value::Integer(2).into()])
            }),
        )
        .unwrap();

    let mut source = operation("adapter_source", &[], &[0]);
    source.outputs[0].production = OutputProduction::Streaming;
    let mut sink = operation("adapter_sink", &[3], &[4]);
    sink.inputs[0].consumption = InputConsumption::FullyMaterialized;
    let adapter = PlannedOperation {
        stable_id: OperationStableId::new("test.operation.adapter.collect").unwrap(),
        source_node_id: NodeId::from_uuid(uuid::Uuid::nil()),
        source_node_type_id: NodeTypeId::new("yssbi.test.adapter.collect").unwrap(),
        kernel: PlannedKernel::Adapter(PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 1_000_000,
                max_bytes: 64 * 1024 * 1024,
            },
        }),
        inputs: Box::new([PlannedInput {
            value: ValueRef::new(1),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            consumption: InputConsumption::Streaming,
            bound_value: None,
        }]),
        outputs: Box::new([PlannedOutput {
            value: ValueRef::new(2),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            production: OutputProduction::FullyMaterialized,
        }]),
        params: id("adapter.none", CompiledParameterHandle::new),
        resource_dependencies: Box::new([]),
        cache_policy: CachePolicy::Disabled,
        semantics_version: ExecutionSemanticsVersion::from_bytes([9; 32]),
        workload: WorkloadClass::AdapterIo,
        retry: PlannedRetry::default(),
    };
    let mut execution_plan = plan(
        vec![source, sink, adapter],
        5,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(2)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    execution_plan.value_dependencies = Box::new([
        ValueDependency {
            source: ValueRef::new(0),
            destination: ValueRef::new(1),
        },
        ValueDependency {
            source: ValueRef::new(2),
            destination: ValueRef::new(3),
        },
    ]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(4),
    }]);
    publish_graph_results(&mut execution_plan);

    let result = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(
        result.values["result"],
        RuntimeValue::from(Value::Integer(2))
    );
}

#[test]
fn external_stream_fanout_before_branch_executes_once_and_delivers_complete_data() {
    for selected in [true, false] {
        let source_executions = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&source_executions);
        let mut kernels = KernelRegistry::new();
        kernels
            .register(
                id("external_stream_source", KernelHandle::new),
                OwnedStreamKernel {
                    values: vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)]
                        .into_boxed_slice(),
                    executions: Some(observed),
                },
            )
            .unwrap();
        kernels
            .register(
                id("branch_condition", KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| Ok(vec![Value::Bool(selected).into()])),
            )
            .unwrap();
        for name in ["then_stream_sink", "else_stream_sink"] {
            kernels
                .register(
                    id(name, KernelHandle::new),
                    FnKernel(|inputs: &[RuntimeValue]| {
                        let RuntimeValue::Artifact(artifact) = &inputs[0] else {
                            return Err(KernelError::new("expected materialized artifact"));
                        };
                        Ok(vec![
                            Value::Integer(artifact.cursor().unwrap().count() as i64).into(),
                        ])
                    }),
                )
                .unwrap();
        }

        let condition = operation("branch_condition", &[], &[1]);
        let shared = adapter_operation(
            "external.shared.collect",
            2,
            3,
            OutputProduction::Streaming,
            InputConsumption::FullyMaterialized,
        );
        let then_adapter = adapter_operation(
            "external.then.identity",
            4,
            5,
            OutputProduction::FullyMaterialized,
            InputConsumption::FullyMaterialized,
        );
        let else_adapter = adapter_operation(
            "external.else.identity",
            6,
            7,
            OutputProduction::FullyMaterialized,
            InputConsumption::FullyMaterialized,
        );
        let mut callee = plan(
            vec![
                condition,
                shared,
                then_adapter,
                else_adapter,
                operation("then_stream_sink", &[8], &[9]),
                operation("else_stream_sink", &[10], &[11]),
            ],
            13,
            StructuredControlRegion::Sequence(Box::new([
                ControlStep::Operation(OperationIndex::new(1)),
                ControlStep::Operation(OperationIndex::new(2)),
                ControlStep::Operation(OperationIndex::new(3)),
                ControlStep::Operation(OperationIndex::new(0)),
                ControlStep::Region(Box::new(StructuredControlRegion::If {
                    condition: ValueRef::new(1),
                    then_region: Box::new(StructuredControlRegion::Sequence(Box::new([
                        ControlStep::Operation(OperationIndex::new(4)),
                    ]))),
                    else_region: Box::new(StructuredControlRegion::Sequence(Box::new([
                        ControlStep::Operation(OperationIndex::new(5)),
                    ]))),
                    results: Box::new([BranchResultBinding {
                        destination: ValueRef::new(12),
                        then_source: ValueRef::new(9),
                        else_source: ValueRef::new(11),
                        production: Some(OutputProduction::FullyMaterialized),
                    }]),
                })),
            ])),
        );
        callee.value_sources = Box::new([
            PlanValueSource::ExternalInput(ValueRef::new(0), OutputProduction::Streaming),
            PlanValueSource::ControlProduced(
                ValueRef::new(12),
                OutputProduction::FullyMaterialized,
            ),
        ]);
        callee.value_dependencies = Box::new([
            ValueDependency {
                source: ValueRef::new(0),
                destination: ValueRef::new(2),
            },
            ValueDependency {
                source: ValueRef::new(3),
                destination: ValueRef::new(4),
            },
            ValueDependency {
                source: ValueRef::new(3),
                destination: ValueRef::new(6),
            },
            ValueDependency {
                source: ValueRef::new(5),
                destination: ValueRef::new(8),
            },
            ValueDependency {
                source: ValueRef::new(7),
                destination: ValueRef::new(10),
            },
        ]);

        let mut source = operation("external_stream_source", &[], &[0]);
        source.outputs[0].production = OutputProduction::Streaming;
        let mut caller = plan(
            vec![source],
            2,
            StructuredControlRegion::Sequence(Box::new([
                ControlStep::Operation(OperationIndex::new(0)),
                ControlStep::Region(Box::new(StructuredControlRegion::Call {
                    target: id("functions/external-branch", FunctionPlanHandle::new),
                    arguments: Box::new([CallArgumentBinding {
                        caller_source: ValueRef::new(0),
                        callee_destination: ValueRef::new(0),
                    }]),
                    results: Box::new([CallResultBinding {
                        callee_source: ValueRef::new(12),
                        caller_destination: ValueRef::new(1),
                        production: Some(OutputProduction::FullyMaterialized),
                    }]),
                    mandatory: true,
                })),
            ])),
        );
        caller.value_sources = Box::new([PlanValueSource::ControlProduced(
            ValueRef::new(1),
            OutputProduction::FullyMaterialized,
        )]);
        caller.results = Box::new([PlanResult {
            name: "count".into(),
            output: stable_output("count"),
            value: ValueRef::new(1),
        }]);
        publish_graph_results(&mut caller);

        let function = published_function(callee, "functions/external-branch", &[0], &[12]);
        let result = RunExecutor::new(&kernels, &no_resources(), &OneFunction(function))
            .run(&caller, CancellationToken::new())
            .unwrap();

        assert_eq!(result.values["count"], Value::Integer(3).into());
        assert_eq!(source_executions.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn shared_materialized_fanout_delivers_complete_data_to_same_and_different_consumers() {
    for different_contracts in [false, true] {
        let observed = Arc::new(Mutex::new(Vec::new()));
        let mut kernels = KernelRegistry::new();
        kernels
            .register(
                id("fanout_source", KernelHandle::new),
                OwnedStreamKernel {
                    values: vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)]
                        .into_boxed_slice(),
                    executions: None,
                },
            )
            .unwrap();
        for name in ["fanout_a", "fanout_b"] {
            let observed = Arc::clone(&observed);
            kernels
                .register(
                    id(name, KernelHandle::new),
                    FnKernel(move |inputs: &[RuntimeValue]| {
                        let count = match &inputs[0] {
                            RuntimeValue::Artifact(artifact) => artifact.cursor().unwrap().count(),
                            RuntimeValue::Stream(stream) => {
                                let mut count = 0;
                                while stream.recv().is_ok() {
                                    count += 1;
                                }
                                count
                            }
                            RuntimeValue::Scalar(_) => 1,
                        };
                        observed.lock().unwrap().push(count);
                        Ok(vec![Value::Integer(count as i64).into()])
                    }),
                )
                .unwrap();
        }
        let mut source = operation("fanout_source", &[], &[0]);
        source.outputs[0].production = OutputProduction::Streaming;
        let mut sink_a = operation("fanout_a", &[7], &[8]);
        sink_a.inputs[0].consumption = if different_contracts {
            InputConsumption::Streaming
        } else {
            InputConsumption::FullyMaterialized
        };
        let sink_b = operation("fanout_b", &[9], &[10]);
        let adapter_operation =
            |stable: &str,
             adapter: PlannedAdapter,
             input: u32,
             output: u32,
             consumption: InputConsumption,
             production: OutputProduction| PlannedOperation {
                stable_id: OperationStableId::new(stable).unwrap(),
                source_node_id: NodeId::from_uuid(uuid::Uuid::nil()),
                source_node_type_id: NodeTypeId::new("yssbi.test.fanout_adapter").unwrap(),
                kernel: PlannedKernel::Adapter(adapter),
                inputs: Box::new([PlannedInput {
                    value: ValueRef::new(input),
                    contract: crate::node_system::plan::PlannedValueContract::opaque(),
                    consumption,
                    bound_value: None,
                }]),
                outputs: Box::new([PlannedOutput {
                    value: ValueRef::new(output),
                    contract: crate::node_system::plan::PlannedValueContract::opaque(),
                    production,
                }]),
                params: id("adapter.fanout", CompiledParameterHandle::new),
                resource_dependencies: Box::new([]),
                cache_policy: CachePolicy::Disabled,
                semantics_version: ExecutionSemanticsVersion::from_bytes([7; 32]),
                workload: WorkloadClass::AdapterIo,
                retry: PlannedRetry::default(),
            };
        let shared = adapter_operation(
            "fanout.shared",
            PlannedAdapter::Collect {
                limits: MaterializationLimits {
                    max_values: 1_000_000,
                    max_bytes: 64 * 1024 * 1024,
                },
            },
            1,
            2,
            InputConsumption::Streaming,
            OutputProduction::FullyMaterialized,
        );
        let adapter_a = adapter_operation(
            "fanout.adapter.a",
            if different_contracts {
                PlannedAdapter::StreamBridge {
                    format: StreamFormat::Native,
                }
            } else {
                PlannedAdapter::Identity
            },
            3,
            4,
            InputConsumption::FullyMaterialized,
            if different_contracts {
                OutputProduction::Streaming
            } else {
                OutputProduction::FullyMaterialized
            },
        );
        let adapter_b = adapter_operation(
            "fanout.adapter.b",
            PlannedAdapter::Identity,
            5,
            6,
            InputConsumption::FullyMaterialized,
            OutputProduction::FullyMaterialized,
        );
        let mut execution_plan = plan(
            vec![source, sink_a, sink_b, shared, adapter_a, adapter_b],
            11,
            StructuredControlRegion::Sequence(Box::new([
                ControlStep::Operation(OperationIndex::new(0)),
                ControlStep::Operation(OperationIndex::new(3)),
                ControlStep::Operation(OperationIndex::new(4)),
                ControlStep::Operation(OperationIndex::new(1)),
                ControlStep::Operation(OperationIndex::new(5)),
                ControlStep::Operation(OperationIndex::new(2)),
            ])),
        );
        execution_plan.value_dependencies = Box::new([
            ValueDependency {
                source: ValueRef::new(0),
                destination: ValueRef::new(1),
            },
            ValueDependency {
                source: ValueRef::new(2),
                destination: ValueRef::new(3),
            },
            ValueDependency {
                source: ValueRef::new(2),
                destination: ValueRef::new(5),
            },
            ValueDependency {
                source: ValueRef::new(4),
                destination: ValueRef::new(7),
            },
            ValueDependency {
                source: ValueRef::new(6),
                destination: ValueRef::new(9),
            },
        ]);
        execution_plan.results = Box::new([
            PlanResult {
                name: "a".into(),
                output: stable_output("a"),
                value: ValueRef::new(8),
            },
            PlanResult {
                name: "b".into(),
                output: stable_output("b"),
                value: ValueRef::new(10),
            },
        ]);
        publish_graph_results(&mut execution_plan);

        RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
            .run(&execution_plan, CancellationToken::new())
            .unwrap();
        let mut counts = observed.lock().unwrap().clone();
        counts.sort();
        assert_eq!(counts, vec![3, 3]);
    }
}

#[test]
fn materialization_matrix_executes_all_fifteen_cells_with_declared_io_contracts() {
    let stream_owner = materialization_test_owner();
    #[derive(Clone, Copy)]
    enum Shape {
        Stream,
        Artifact(ArtifactKind),
    }

    let identity = PlannedAdapter::Identity;
    let stream_bridge = PlannedAdapter::StreamBridge {
        format: StreamFormat::Native,
    };
    let cases = [
        (
            OutputProduction::Streaming,
            InputConsumption::Streaming,
            identity.clone(),
            InputConsumption::Streaming,
            OutputProduction::Streaming,
            Shape::Stream,
        ),
        (
            OutputProduction::Streaming,
            InputConsumption::SinglePassBatches,
            PlannedAdapter::Buffer { capacity: 64 },
            InputConsumption::Streaming,
            OutputProduction::Batches,
            Shape::Artifact(ArtifactKind::Buffered),
        ),
        (
            OutputProduction::Streaming,
            InputConsumption::RewindableBatches,
            PlannedAdapter::Replay,
            InputConsumption::Streaming,
            OutputProduction::Batches,
            Shape::Artifact(ArtifactKind::Replayable),
        ),
        (
            OutputProduction::Streaming,
            InputConsumption::RandomAccess,
            PlannedAdapter::Spill {
                memory_limit_bytes: 64 * 1024 * 1024,
            },
            InputConsumption::Streaming,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Spilled),
        ),
        (
            OutputProduction::Streaming,
            InputConsumption::FullyMaterialized,
            PlannedAdapter::Collect {
                limits: MaterializationLimits {
                    max_values: 1_000_000,
                    max_bytes: 64 * 1024 * 1024,
                },
            },
            InputConsumption::Streaming,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Collected),
        ),
        (
            OutputProduction::Batches,
            InputConsumption::Streaming,
            stream_bridge.clone(),
            InputConsumption::SinglePassBatches,
            OutputProduction::Streaming,
            Shape::Stream,
        ),
        (
            OutputProduction::Batches,
            InputConsumption::SinglePassBatches,
            identity.clone(),
            InputConsumption::SinglePassBatches,
            OutputProduction::Batches,
            Shape::Artifact(ArtifactKind::Buffered),
        ),
        (
            OutputProduction::Batches,
            InputConsumption::RewindableBatches,
            PlannedAdapter::Replay,
            InputConsumption::SinglePassBatches,
            OutputProduction::Batches,
            Shape::Artifact(ArtifactKind::Replayable),
        ),
        (
            OutputProduction::Batches,
            InputConsumption::RandomAccess,
            PlannedAdapter::Spill {
                memory_limit_bytes: 64 * 1024 * 1024,
            },
            InputConsumption::SinglePassBatches,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Spilled),
        ),
        (
            OutputProduction::Batches,
            InputConsumption::FullyMaterialized,
            PlannedAdapter::Collect {
                limits: MaterializationLimits {
                    max_values: 1_000_000,
                    max_bytes: 64 * 1024 * 1024,
                },
            },
            InputConsumption::SinglePassBatches,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Collected),
        ),
        (
            OutputProduction::FullyMaterialized,
            InputConsumption::Streaming,
            stream_bridge,
            InputConsumption::FullyMaterialized,
            OutputProduction::Streaming,
            Shape::Stream,
        ),
        (
            OutputProduction::FullyMaterialized,
            InputConsumption::SinglePassBatches,
            identity.clone(),
            InputConsumption::FullyMaterialized,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Collected),
        ),
        (
            OutputProduction::FullyMaterialized,
            InputConsumption::RewindableBatches,
            identity.clone(),
            InputConsumption::FullyMaterialized,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Collected),
        ),
        (
            OutputProduction::FullyMaterialized,
            InputConsumption::RandomAccess,
            identity.clone(),
            InputConsumption::FullyMaterialized,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Collected),
        ),
        (
            OutputProduction::FullyMaterialized,
            InputConsumption::FullyMaterialized,
            identity,
            InputConsumption::FullyMaterialized,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Collected),
        ),
    ];

    for (production, consumption, adapter, adapter_consumption, adapter_production, shape) in cases
    {
        let planned = MaterializationAdapterPlan::for_contract(production, consumption);
        assert_eq!(planned.adapter, adapter);
        assert_eq!(planned.input_consumption, adapter_consumption);
        assert_eq!(planned.output_production, adapter_production);
        let input = match production {
            OutputProduction::Streaming => RuntimeValue::Stream(
                stream_owner
                    .stream_from_values([Value::Integer(7)])
                    .unwrap(),
            ),
            OutputProduction::Batches => RuntimeValue::Artifact(Artifact::new(
                ArtifactKind::Buffered,
                vec![Value::Integer(7)],
            )),
            OutputProduction::FullyMaterialized => RuntimeValue::Artifact(Artifact::new(
                ArtifactKind::Collected,
                vec![Value::Integer(7)],
            )),
        };
        let cancellation = CancellationToken::new();
        let output = execute_planned_adapter(
            &planned.adapter,
            input,
            stream_owner.as_ref(),
            &cancellation,
        )
        .unwrap();
        match (shape, output) {
            (Shape::Stream, RuntimeValue::Stream(_)) => {}
            (Shape::Artifact(expected), RuntimeValue::Artifact(actual)) => {
                assert_eq!(actual.kind(), expected);
                assert_eq!(
                    actual
                        .cursor()
                        .unwrap()
                        .collect::<Result<Vec<_>, _>>()
                        .unwrap(),
                    [Value::Integer(7)]
                );
            }
            _ => panic!("adapter runtime result does not match its declared production"),
        }
    }
}

fn per_run_memo_key(inputs: &[RuntimeValue], resource_revision: &str) -> OperationMemoKey {
    OperationMemoKey::from_inputs(
        OperationStableId::new("events/test::memoized-operation").unwrap(),
        inputs,
        BTreeMap::from([(
            ResourceKey::new("variables/relevant"),
            ResourceVersion::new(resource_revision),
        )]),
        ExecutionSemanticsVersion::from_bytes([7; 32]),
        DemandFingerprint::from_bytes([9; 32]),
    )
    .expect("materialized inputs are cacheable")
}

#[test]
fn per_run_memoization_demand_fingerprints_are_frame_specific_without_sentinels() {
    let mut root = plan(
        vec![operation("demand", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    root.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut root);

    assert_ne!(
        DemandFingerprint::for_root(&root, None),
        DemandFingerprint::for_root(&root, Some([0; 32]))
    );
    let mut different_publication = root.clone();
    different_publication.results[0].name = "other".into();
    different_publication.publications = Box::new([PlannedPublication::GraphResult {
        name: "other".into(),
        output: different_publication.results[0].output.clone(),
        value: ValueRef::new(0),
    }]);
    assert_ne!(
        DemandFingerprint::for_root(&root, None),
        DemandFingerprint::for_root(&different_publication, None)
    );

    let target = id("functions/callee", FunctionPlanHandle::new);
    let first_arguments = Box::new([CallArgumentBinding {
        caller_source: ValueRef::new(0),
        callee_destination: ValueRef::new(1),
    }]);
    let second_arguments = Box::new([CallArgumentBinding {
        caller_source: ValueRef::new(0),
        callee_destination: ValueRef::new(2),
    }]);
    let results = Box::new([CallResultBinding {
        callee_source: ValueRef::new(3),
        caller_destination: ValueRef::new(4),
        production: Some(OutputProduction::FullyMaterialized),
    }]);
    assert_ne!(
        DemandFingerprint::for_callee(&root, &target, &first_arguments[..], &results[..]),
        DemandFingerprint::for_callee(&root, &target, &second_arguments[..], &results[..])
    );
}

#[test]
fn per_run_memoization_same_key_produces_once() {
    let memo = RunMemoization::new();
    let key = per_run_memo_key(&[Value::Integer(7).into()], "1");
    let calls = AtomicUsize::new(0);

    for _ in 0..2 {
        let outputs = memo
            .get_or_produce(key.clone(), &CancellationToken::new(), || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(vec![Value::Integer(8).into()].into_boxed_slice())
            })
            .unwrap();
        assert_eq!(outputs.as_ref(), &[RuntimeValue::from(Value::Integer(8))]);
    }

    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn per_run_memoization_different_typed_inputs_produce_separately() {
    let memo = RunMemoization::new();
    let calls = AtomicUsize::new(0);

    for input in [
        RuntimeValue::from(Value::Integer(7)),
        RuntimeValue::from(Value::String("7".into())),
        RuntimeValue::Artifact(Artifact::new(ArtifactKind::Buffered, [Value::Integer(7)])),
        RuntimeValue::Artifact(Artifact::new(ArtifactKind::Buffered, [Value::Integer(7)])),
        RuntimeValue::Artifact(Artifact::new(ArtifactKind::Collected, [Value::Integer(7)])),
    ] {
        let key = per_run_memo_key(&[input], "1");
        memo.get_or_produce(key, &CancellationToken::new(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new([]))
        })
        .unwrap();
    }

    assert_eq!(calls.load(Ordering::SeqCst), 4);
}

#[test]
fn per_run_memoization_relevant_resource_revision_is_part_of_the_key() {
    let memo = RunMemoization::new();
    let calls = AtomicUsize::new(0);

    for revision in ["41", "42"] {
        memo.get_or_produce(
            per_run_memo_key(&[Value::Null.into()], revision),
            &CancellationToken::new(),
            || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(Box::new([]))
            },
        )
        .unwrap();
    }

    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn per_run_memoization_uses_only_operation_resource_versions() {
    let mut memoized = operation("memo_resource", &[], &[]);
    memoized.cache_policy = CachePolicy::PerRun;
    memoized.resource_dependencies = Box::new([ResourceKey::new("variables/relevant")]);
    let mut execution_plan = plan(
        vec![memoized],
        0,
        StructuredControlRegion::Sequence(Box::new([])),
    );
    execution_plan.provenance.basis.resource_versions = BTreeMap::from([
        (
            ResourceKey::new("variables/relevant"),
            ResourceVersion::new("1"),
        ),
        (
            ResourceKey::new("variables/unrelated"),
            ResourceVersion::new("1"),
        ),
    ]);
    let memo = RunMemoization::new();
    let calls = AtomicUsize::new(0);

    for (unrelated, relevant) in [("1", "1"), ("2", "1"), ("2", "2")] {
        execution_plan.provenance.basis.resource_versions.insert(
            ResourceKey::new("variables/unrelated"),
            ResourceVersion::new(unrelated),
        );
        execution_plan.provenance.basis.resource_versions.insert(
            ResourceKey::new("variables/relevant"),
            ResourceVersion::new(relevant),
        );
        let versions =
            super::scheduler::operation_resource_versions(&execution_plan, OperationIndex::new(0))
                .expect("declared relevant version exists");
        let key = OperationMemoKey::from_inputs(
            execution_plan.operations[0].stable_id.clone(),
            &[],
            versions,
            execution_plan.operations[0].semantics_version,
            DemandFingerprint::from_bytes([9; 32]),
        )
        .unwrap();
        memo.get_or_produce(key, &CancellationToken::new(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new([]))
        })
        .unwrap();
    }

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    execution_plan
        .provenance
        .basis
        .resource_versions
        .remove(&ResourceKey::new("variables/relevant"));
    assert!(
        super::scheduler::operation_resource_versions(&execution_plan, OperationIndex::new(0))
            .is_none()
    );
}

#[test]
fn per_run_memoization_concurrent_same_key_has_one_producer_and_waiter_cancel_isolated() {
    let memo = Arc::new(RunMemoization::new());
    let key = per_run_memo_key(&[Value::Integer(1).into()], "1");
    let producer_started = Arc::new(Barrier::new(2));
    let release_producer = Arc::new(Barrier::new(2));
    let calls = Arc::new(AtomicUsize::new(0));

    let producer = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let producer_started = Arc::clone(&producer_started);
        let release_producer = Arc::clone(&release_producer);
        let calls = Arc::clone(&calls);
        thread::spawn(move || {
            memo.get_or_produce(key, &CancellationToken::new(), || {
                calls.fetch_add(1, Ordering::SeqCst);
                producer_started.wait();
                release_producer.wait();
                Ok(vec![Value::Integer(2).into()].into_boxed_slice())
            })
        })
    };
    producer_started.wait();

    let cancelled = CancellationToken::new();
    let cancelled_waiter = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let cancelled = cancelled.clone();
        thread::spawn(move || memo.get_or_produce(key, &cancelled, || panic!("waiter produced")))
    };
    cancelled.cancel();

    let successful_waiter = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        thread::spawn(move || {
            memo.get_or_produce(key, &CancellationToken::new(), || panic!("waiter produced"))
        })
    };
    release_producer.wait();

    assert_eq!(cancelled_waiter.join().unwrap(), Err(RunError::Cancelled));
    assert_eq!(
        producer.join().unwrap().unwrap().as_ref(),
        &[RuntimeValue::from(Value::Integer(2))]
    );
    assert_eq!(
        successful_waiter.join().unwrap().unwrap().as_ref(),
        &[RuntimeValue::from(Value::Integer(2))]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn per_run_memoization_producer_panic_removes_flight_and_wakes_waiter() {
    let memo = Arc::new(RunMemoization::new());
    let key = per_run_memo_key(&[Value::Integer(1).into()], "1");
    let producer_started = Arc::new(Barrier::new(2));
    let waiter_registered = Arc::new(Barrier::new(2));
    let release_producer = Arc::new(Barrier::new(2));

    let producer = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let producer_started = Arc::clone(&producer_started);
        let release_producer = Arc::clone(&release_producer);
        thread::spawn(move || {
            memo.get_or_produce(key, &CancellationToken::new(), || {
                producer_started.wait();
                release_producer.wait();
                panic!("producer panic sentinel")
            })
        })
    };
    producer_started.wait();

    let waiter = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let waiter_registered = Arc::clone(&waiter_registered);
        thread::spawn(move || {
            memo.get_or_produce_with_commit_checkpoint(
                key,
                &CancellationToken::new(),
                || panic!("waiter produced"),
                |checkpoint| {
                    if checkpoint == MemoCommitCheckpoint::WaiterRegistered {
                        waiter_registered.wait();
                    }
                },
            )
        })
    };
    waiter_registered.wait();
    release_producer.wait();

    let expected = Err(RunError::InvalidPlan(
        "memoization producer panicked".into(),
    ));
    assert!(producer.join().is_err(), "producer panic must unwind");
    assert_eq!(waiter.join().unwrap(), expected);

    assert_eq!(
        memo.get_or_produce(key, &CancellationToken::new(), || Ok(Box::new([])))
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn per_run_memoization_producer_error_is_removed() {
    let memo = RunMemoization::new();
    let key = per_run_memo_key(&[Value::Null.into()], "1");
    let calls = AtomicUsize::new(0);

    let first = memo.get_or_produce(key.clone(), &CancellationToken::new(), || {
        calls.fetch_add(1, Ordering::SeqCst);
        Err(RunError::InvalidPlan("failed".into()))
    });
    let second = memo.get_or_produce(key, &CancellationToken::new(), || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new([]))
    });

    assert_eq!(first, Err(RunError::InvalidPlan("failed".into())));
    assert_eq!(second.unwrap().len(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn per_run_memoization_producer_cancellation_is_not_cached() {
    let memo = RunMemoization::new();
    let key = per_run_memo_key(&[Value::Null.into()], "1");
    let calls = AtomicUsize::new(0);

    let cancelled = CancellationToken::new();
    let producer_token = cancelled.clone();
    let first = memo.get_or_produce(key.clone(), &cancelled, || {
        calls.fetch_add(1, Ordering::SeqCst);
        producer_token.cancel();
        Ok(Box::new([]))
    });
    let second = memo.get_or_produce(key, &CancellationToken::new(), || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new([]))
    });

    assert_eq!(first, Err(RunError::Cancelled));
    assert_eq!(second.unwrap().len(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn per_run_memoization_cancellation_before_commit_does_not_cache() {
    let memo = Arc::new(RunMemoization::new());
    let key = per_run_memo_key(&[Value::Integer(1).into()], "1");
    let cancellation = CancellationToken::new();
    let at_commit = Arc::new(Barrier::new(2));
    let release_commit = Arc::new(Barrier::new(2));
    let calls = Arc::new(AtomicUsize::new(0));

    let producer = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let cancellation = cancellation.clone();
        let at_commit = Arc::clone(&at_commit);
        let release_commit = Arc::clone(&release_commit);
        let calls = Arc::clone(&calls);
        thread::spawn(move || {
            memo.get_or_produce_with_commit_checkpoint(
                key,
                &cancellation,
                || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(Box::new([]))
                },
                |checkpoint| {
                    if checkpoint == MemoCommitCheckpoint::BeforeCommit {
                        at_commit.wait();
                        release_commit.wait();
                    }
                },
            )
        })
    };
    at_commit.wait();
    cancellation.cancel();
    release_commit.wait();

    assert_eq!(producer.join().unwrap(), Err(RunError::Cancelled));
    memo.get_or_produce(key, &CancellationToken::new(), || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new([]))
    })
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn per_run_memoization_cancellation_after_commit_keeps_cache() {
    let memo = Arc::new(RunMemoization::new());
    let key = per_run_memo_key(&[Value::Integer(1).into()], "1");
    let cancellation = CancellationToken::new();
    let committed = Arc::new(Barrier::new(2));
    let release_producer = Arc::new(Barrier::new(2));
    let calls = Arc::new(AtomicUsize::new(0));

    let producer = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let cancellation = cancellation.clone();
        let committed = Arc::clone(&committed);
        let release_producer = Arc::clone(&release_producer);
        let calls = Arc::clone(&calls);
        thread::spawn(move || {
            memo.get_or_produce_with_commit_checkpoint(
                key,
                &cancellation,
                || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(Box::new([]))
                },
                |checkpoint| {
                    if checkpoint == MemoCommitCheckpoint::Committed {
                        committed.wait();
                        release_producer.wait();
                    }
                },
            )
        })
    };
    committed.wait();
    cancellation.cancel();
    release_producer.wait();

    assert_eq!(producer.join().unwrap().unwrap().len(), 0);
    memo.get_or_produce(key, &CancellationToken::new(), || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new([]))
    })
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn per_run_memoization_partial_stream_is_not_cacheable() {
    let memo = RunMemoization::new();
    let stream_owner = materialization_test_owner();
    let stream = RuntimeValue::Stream(
        stream_owner
            .stream_from_values([Value::Integer(1)])
            .unwrap(),
    );
    assert!(
        OperationMemoKey::from_inputs(
            OperationStableId::new("events/test::stream-operation").unwrap(),
            &[stream],
            BTreeMap::new(),
            ExecutionSemanticsVersion::from_bytes([7; 32]),
            DemandFingerprint::from_bytes([9; 32]),
        )
        .is_none()
    );

    let key = per_run_memo_key(&[Value::Integer(1).into()], "1");
    let calls = AtomicUsize::new(0);
    for _ in 0..2 {
        let result = memo.get_or_produce(key.clone(), &CancellationToken::new(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(vec![RuntimeValue::Stream(
                stream_owner
                    .stream_from_values([Value::Integer(2)])
                    .unwrap(),
            )]
            .into_boxed_slice())
        });
        assert!(matches!(
            result.unwrap().as_ref(),
            [RuntimeValue::Stream(_)]
        ));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn per_run_memoization_run_finalization_releases_entries() {
    let memo = RunMemoization::new();
    let key = per_run_memo_key(&[Value::Null.into()], "1");
    let calls = AtomicUsize::new(0);
    memo.get_or_produce(key.clone(), &CancellationToken::new(), || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new([]))
    })
    .unwrap();
    memo.finalize();
    assert_eq!(
        memo.get_or_produce(key, &CancellationToken::new(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new([]))
        }),
        Err(RunError::Cancelled)
    );

    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn per_run_memoization_finalize_wakes_waiter_and_prevents_late_commit() {
    let memo = Arc::new(RunMemoization::new());
    let key = per_run_memo_key(&[Value::Integer(1).into()], "1");
    let producer_started = Arc::new(Barrier::new(2));
    let waiter_registered = Arc::new(Barrier::new(2));
    let release_producer = Arc::new(Barrier::new(2));

    let producer = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let producer_started = Arc::clone(&producer_started);
        let release_producer = Arc::clone(&release_producer);
        thread::spawn(move || {
            memo.get_or_produce(key, &CancellationToken::new(), || {
                producer_started.wait();
                release_producer.wait();
                Ok(Box::new([]))
            })
        })
    };
    producer_started.wait();

    let (settled_tx, settled_rx) = mpsc::channel();
    let waiter = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let waiter_registered = Arc::clone(&waiter_registered);
        thread::spawn(move || {
            settled_tx
                .send(memo.get_or_produce_with_commit_checkpoint(
                    key,
                    &CancellationToken::new(),
                    || panic!("waiter produced"),
                    |checkpoint| {
                        if checkpoint == MemoCommitCheckpoint::WaiterRegistered {
                            waiter_registered.wait();
                        }
                    },
                ))
                .unwrap();
        })
    };
    waiter_registered.wait();
    memo.finalize();

    assert_eq!(
        settled_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        Err(RunError::Cancelled)
    );
    release_producer.wait();
    assert_eq!(producer.join().unwrap(), Err(RunError::Cancelled));
    waiter.join().unwrap();

    let late_calls = AtomicUsize::new(0);
    let late = memo.get_or_produce(key, &CancellationToken::new(), || {
        late_calls.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new([]))
    });
    assert_eq!(late, Err(RunError::Cancelled));
    assert_eq!(late_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn per_run_memoization_finalize_terminal_wins_over_late_producer_error() {
    let memo = Arc::new(RunMemoization::new());
    let key = per_run_memo_key(&[Value::Integer(1).into()], "1");
    let producer_started = Arc::new(Barrier::new(2));
    let release_error = Arc::new(Barrier::new(2));
    let waiter_registered = Arc::new(Barrier::new(2));

    let producer = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let producer_started = Arc::clone(&producer_started);
        let release_error = Arc::clone(&release_error);
        thread::spawn(move || {
            memo.get_or_produce(key, &CancellationToken::new(), || {
                producer_started.wait();
                release_error.wait();
                Err(RunError::InvalidPlan("late error".into()))
            })
        })
    };
    producer_started.wait();
    let (waiter_tx, waiter_rx) = mpsc::channel();
    let waiter = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let waiter_registered = Arc::clone(&waiter_registered);
        thread::spawn(move || {
            let result = memo.get_or_produce_with_commit_checkpoint(
                key,
                &CancellationToken::new(),
                || panic!("waiter produced"),
                |checkpoint| {
                    if checkpoint == MemoCommitCheckpoint::WaiterRegistered {
                        waiter_registered.wait();
                    }
                },
            );
            waiter_tx.send(result).unwrap();
        })
    };
    waiter_registered.wait();

    let terminal_set = Arc::new(Barrier::new(2));
    let release_finalize = Arc::new(Barrier::new(2));
    let finalizer = {
        let memo = Arc::clone(&memo);
        let terminal_set = Arc::clone(&terminal_set);
        let release_finalize = Arc::clone(&release_finalize);
        thread::spawn(move || {
            memo.finalize_with_checkpoint(|| {
                terminal_set.wait();
                release_finalize.wait();
            });
        })
    };
    terminal_set.wait();
    release_error.wait();
    assert!(waiter_rx.recv_timeout(Duration::from_millis(100)).is_err());
    release_finalize.wait();

    assert_eq!(producer.join().unwrap(), Err(RunError::Cancelled));
    assert_eq!(
        waiter_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        Err(RunError::Cancelled)
    );
    waiter.join().unwrap();
    finalizer.join().unwrap();
}

#[test]
fn per_run_memoization_finalize_terminal_wins_over_late_producer_panic() {
    let memo = Arc::new(RunMemoization::new());
    let key = per_run_memo_key(&[Value::Integer(1).into()], "1");
    let producer_started = Arc::new(Barrier::new(2));
    let release_panic = Arc::new(Barrier::new(2));
    let waiter_registered = Arc::new(Barrier::new(2));

    let producer = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let producer_started = Arc::clone(&producer_started);
        let release_panic = Arc::clone(&release_panic);
        thread::spawn(move || {
            memo.get_or_produce(key, &CancellationToken::new(), || {
                producer_started.wait();
                release_panic.wait();
                panic!("late panic")
            })
        })
    };
    producer_started.wait();
    let (waiter_tx, waiter_rx) = mpsc::channel();
    let waiter = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let waiter_registered = Arc::clone(&waiter_registered);
        thread::spawn(move || {
            let result = memo.get_or_produce_with_commit_checkpoint(
                key,
                &CancellationToken::new(),
                || panic!("waiter produced"),
                |checkpoint| {
                    if checkpoint == MemoCommitCheckpoint::WaiterRegistered {
                        waiter_registered.wait();
                    }
                },
            );
            waiter_tx.send(result).unwrap();
        })
    };
    waiter_registered.wait();

    let terminal_set = Arc::new(Barrier::new(2));
    let release_finalize = Arc::new(Barrier::new(2));
    let finalizer = {
        let memo = Arc::clone(&memo);
        let terminal_set = Arc::clone(&terminal_set);
        let release_finalize = Arc::clone(&release_finalize);
        thread::spawn(move || {
            memo.finalize_with_checkpoint(|| {
                terminal_set.wait();
                release_finalize.wait();
            });
        })
    };
    terminal_set.wait();
    release_panic.wait();
    assert!(waiter_rx.recv_timeout(Duration::from_millis(100)).is_err());
    release_finalize.wait();

    assert!(producer.join().is_err());
    assert_eq!(
        waiter_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        Err(RunError::Cancelled)
    );
    waiter.join().unwrap();
    finalizer.join().unwrap();
}

#[test]
fn per_run_memoization_finalize_owner_lock_rejects_late_lookup() {
    let memo = Arc::new(RunMemoization::new());
    let terminal_set = Arc::new(Barrier::new(2));
    let release_finalize = Arc::new(Barrier::new(2));
    let finalizer = {
        let memo = Arc::clone(&memo);
        let terminal_set = Arc::clone(&terminal_set);
        let release_finalize = Arc::clone(&release_finalize);
        thread::spawn(move || {
            memo.finalize_with_checkpoint(|| {
                terminal_set.wait();
                release_finalize.wait();
            });
        })
    };
    terminal_set.wait();

    let calls = Arc::new(AtomicUsize::new(0));
    let late = {
        let memo = Arc::clone(&memo);
        let calls = Arc::clone(&calls);
        thread::spawn(move || {
            memo.get_or_produce(
                per_run_memo_key(&[Value::Null.into()], "1"),
                &CancellationToken::new(),
                || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(Box::new([]))
                },
            )
        })
    };
    release_finalize.wait();

    finalizer.join().unwrap();
    assert_eq!(late.join().unwrap(), Err(RunError::Cancelled));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn per_run_memoization_new_run_is_isolated() {
    let key = per_run_memo_key(&[Value::Null.into()], "1");
    let calls = AtomicUsize::new(0);

    for _ in 0..2 {
        let memo = RunMemoization::new();
        memo.get_or_produce(key.clone(), &CancellationToken::new(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new([]))
        })
        .unwrap();
    }

    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn per_run_memoization_executor_runs_same_key_kernel_once_per_run() {
    let memoized_calls = Arc::new(AtomicUsize::new(0));
    let loop_calls = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("memo_initial", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(0).into()])),
        )
        .unwrap();
    let observed_memoized = Arc::clone(&memoized_calls);
    kernels
        .register(
            id("memo_value", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed_memoized.fetch_add(1, Ordering::SeqCst);
                Ok(vec![Value::Integer(41).into()])
            }),
        )
        .unwrap();
    let observed_loop = Arc::clone(&loop_calls);
    kernels
        .register(
            id("memo_loop", KernelHandle::new),
            FnKernel(move |inputs: &[RuntimeValue]| {
                observed_loop.fetch_add(1, Ordering::SeqCst);
                let RuntimeValue::Scalar(Value::Integer(value)) = &inputs[0] else {
                    return Err(KernelError::new("expected loop integer"));
                };
                let next = value + 1;
                Ok(vec![
                    Value::Integer(next).into(),
                    Value::Bool(next < 3).into(),
                ])
            }),
        )
        .unwrap();

    let mut memoized = operation("memo_value", &[], &[1]);
    memoized.cache_policy = CachePolicy::PerRun;
    let mut execution_plan = plan(
        vec![
            operation("memo_initial", &[], &[0]),
            memoized,
            operation("memo_loop", &[2], &[3, 4]),
        ],
        6,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(StructuredControlRegion::Loop {
                body: Box::new(StructuredControlRegion::Sequence(Box::new([
                    ControlStep::Operation(OperationIndex::new(1)),
                    ControlStep::Operation(OperationIndex::new(2)),
                ]))),
                carried: Box::new([LoopCarriedBinding {
                    body_input: ValueRef::new(2),
                    initial_source: ValueRef::new(0),
                    next_source: ValueRef::new(3),
                    result: ValueRef::new(5),
                    production: Some(OutputProduction::FullyMaterialized),
                }]),
                continue_condition: ValueRef::new(4),
                max_iterations: 4,
            })),
        ])),
    );
    execution_plan.value_sources = Box::new([
        PlanValueSource::ControlProduced(ValueRef::new(2), OutputProduction::FullyMaterialized),
        PlanValueSource::ControlProduced(ValueRef::new(5), OutputProduction::FullyMaterialized),
    ]);
    execution_plan.results = Box::new([PlanResult {
        name: "count".into(),
        output: stable_output("count"),
        value: ValueRef::new(5),
    }]);
    publish_graph_results(&mut execution_plan);

    execution_plan.validate().unwrap();
    let resources = no_resources();
    let executor = RunExecutor::new(&kernels, &resources, &NoFunctions);
    for expected_run_count in 1..=2 {
        let result = executor
            .run(&execution_plan, CancellationToken::new())
            .unwrap();
        assert_eq!(
            result.values["count"],
            RuntimeValue::from(Value::Integer(3))
        );
        assert_eq!(memoized_calls.load(Ordering::SeqCst), expected_run_count);
        assert_eq!(loop_calls.load(Ordering::SeqCst), expected_run_count * 3);
    }
}

#[test]
fn deadline_phase_codes_are_stable_and_cancellation_has_priority() {
    let phases = [
        (RunPhase::QueueWait, "\"queueWait\""),
        (RunPhase::Kernel, "\"kernel\""),
        (RunPhase::StreamSend, "\"streamSend\""),
        (RunPhase::StreamReceive, "\"streamReceive\""),
        (RunPhase::AdapterIo, "\"adapterIo\""),
        (RunPhase::ResultPublication, "\"resultPublication\""),
        (RunPhase::Cleanup, "\"cleanup\""),
    ];
    for (phase, wire) in phases {
        assert_eq!(serde_json::to_string(&phase).unwrap(), wire);
        assert_eq!(
            RunDeadline::after(Duration::ZERO).check(&CancellationToken::new(), phase),
            Err(RunError::DeadlineExceeded { phase })
        );
    }

    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert_eq!(
        RunDeadline::after(Duration::ZERO).check(&cancellation, RunPhase::Kernel),
        Err(RunError::Cancelled)
    );
}

#[test]
fn deadline_wakes_blocked_stream_send_and_receive_with_typed_phases() {
    let cancellation = CancellationToken::new();
    let deadline = RunDeadline::after(Duration::from_millis(20));
    let (sender, _receiver) =
        bounded_stream_channel_with_deadline(1, cancellation.clone(), Some(deadline)).unwrap();
    sender.send(1).unwrap();
    assert_eq!(sender.send(2), Err(StreamSendError::DeadlineExceeded(2)));

    let deadline = RunDeadline::after(Duration::from_millis(20));
    let (_sender, receiver) =
        bounded_stream_channel_with_deadline::<i32>(1, cancellation, Some(deadline)).unwrap();
    assert_eq!(receiver.recv(), Err(StreamReceiveError::DeadlineExceeded));
}

#[test]
fn deadline_late_kernel_completion_is_joined_without_commit_or_completion_event() {
    let events = RecordingRunEvents::default();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("deadline_late_kernel", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| {
                thread::sleep(Duration::from_millis(40));
                Ok(vec![Value::Integer(7).into()])
            }),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("deadline_late_kernel", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "late".into(),
        output: stable_output("late"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);

    let result = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_event_sink(&events)
        .with_deadline(RunDeadline::after(Duration::from_millis(10)))
        .run(&execution_plan, CancellationToken::new());

    assert_eq!(
        result,
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::Kernel,
        })
    );
    let events = events.0.lock().unwrap();
    assert!(!events.iter().any(|event| matches!(
        event.kind,
        RunEventKind::OperationCompleted { .. } | RunEventKind::ResultReady { .. }
    )));
    assert!(events.iter().any(|event| matches!(
        event.kind,
        RunEventKind::RunErrored {
            outcome: RunErrorOutcome::DeadlineExceeded {
                phase: RunPhase::Kernel,
            },
        }
    )));
}

#[test]
fn deadline_queue_wait_is_typed_and_late_workers_do_not_commit() {
    let events = RecordingRunEvents::default();
    let mut kernels = KernelRegistry::new();
    for name in ["parallel0", "parallel1"] {
        kernels
            .register(
                id(name, KernelHandle::new),
                FnKernel(|_: &[RuntimeValue]| {
                    thread::sleep(Duration::from_millis(30));
                    Ok(vec![Value::Integer(1).into()])
                }),
            )
            .unwrap();
    }
    let deadline = RunDeadline::after(Duration::from_millis(10));
    let result = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_event_sink(&events)
        .with_scheduling_policy(parallel_policy(1, 1, 1))
        .with_deadline(deadline)
        .run(
            &independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Cpu]),
            CancellationToken::new(),
        );

    assert_eq!(
        result,
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::QueueWait,
        })
    );
    assert!(
        !events
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|event| matches!(event.kind, RunEventKind::OperationCompleted { .. }))
    );
}

#[test]
fn deadline_adapter_io_uses_the_owner_deadline_without_a_local_timer() {
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::new_with_deadline(
        RunId::new(99),
        RunResourceBudgets::default(),
        cancellation.clone(),
        Some(RunDeadline::after(Duration::ZERO)),
    )
    .unwrap();

    assert_eq!(
        execute_planned_adapter(
            &PlannedAdapter::Identity,
            Value::Integer(1).into(),
            &owner,
            &cancellation,
        ),
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::AdapterIo,
        })
    );
    let _ = owner.cleanup();
}

#[test]
fn deadline_publication_suppresses_terminal_result_events() {
    let events = RecordingRunEvents::default();
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("deadline_publication", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(1).into()])),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("deadline_publication", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "publication".into(),
        output: stable_output("publication"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    let result = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_event_sink(&events)
        .with_deadline(RunDeadline::after(Duration::from_millis(20)))
        .with_test_checkpoint(Arc::new(|checkpoint, _| {
            if checkpoint == SchedulerCheckpoint::FinalResultPublication {
                thread::sleep(Duration::from_millis(30));
            }
        }))
        .run(&execution_plan, CancellationToken::new());

    assert_eq!(
        result,
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::ResultPublication,
        })
    );
    assert!(!events.0.lock().unwrap().iter().any(|event| matches!(
        event.kind,
        RunEventKind::RunCompleted
            | RunEventKind::ResultReady { .. }
            | RunEventKind::OutputReady { .. }
    )));
}

struct CleanupDeadlineKernel;

impl Kernel for CleanupDeadlineKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        context
            .resource_owner
            .register_cleanup_delay_for_test(Duration::from_millis(30));
        Ok(Vec::new())
    }
}

#[test]
fn deadline_cleanup_runs_to_completion_without_replacing_an_earlier_error() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("deadline_cleanup", KernelHandle::new),
            CleanupDeadlineKernel,
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("deadline_cleanup", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    assert_eq!(
        RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
            .with_deadline(RunDeadline::after(Duration::from_millis(10)))
            .run(&execution_plan, CancellationToken::new()),
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::Cleanup,
        })
    );
}

struct CooperativeDeadlineKernel;

struct PromptCancellationKernel {
    started: mpsc::SyncSender<()>,
}

impl Kernel for PromptCancellationKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        self.started.send(()).unwrap();
        context.wait_for(Duration::from_secs(5))?;
        Ok(Vec::new())
    }
}

impl Kernel for CooperativeDeadlineKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        assert_eq!(context.deadline, context.resource_owner.deadline());
        context.wait_for(Duration::from_secs(1))?;
        Ok(Vec::new())
    }
}

#[test]
fn cooperative_context_waits_use_cancellation_wake_primitive() {
    let kernel_source = include_str!("kernel.rs");
    let relational_source = include_str!("relational.rs");

    assert!(!kernel_source.contains("std::thread::sleep"));
    assert!(!relational_source.contains("std::thread::sleep"));
    assert!(kernel_source.contains("wait_timeout"));
    assert!(relational_source.contains("wait_timeout"));
}

#[test]
fn kernel_context_wait_wakes_promptly_on_cancellation() {
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("prompt_cancellation", KernelHandle::new),
            PromptCancellationKernel {
                started: started_tx,
            },
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("prompt_cancellation", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    let cancellation = CancellationToken::new();

    thread::scope(|scope| {
        let run_cancellation = cancellation.clone();
        let run = scope.spawn(|| {
            RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
                .run(&execution_plan, run_cancellation)
        });
        started_rx.recv().unwrap();
        let cancelled_at = std::time::Instant::now();
        cancellation.cancel();
        assert_eq!(run.join().unwrap(), Err(RunError::Cancelled));
        assert!(cancelled_at.elapsed() < Duration::from_millis(100));
    });
}

#[test]
fn deadline_is_propagated_into_cooperative_kernel_context() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("cooperative_deadline", KernelHandle::new),
            CooperativeDeadlineKernel,
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("cooperative_deadline", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    let started = std::time::Instant::now();

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_deadline(RunDeadline::after(Duration::from_millis(20)))
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert_eq!(
        error,
        RunError::DeadlineExceeded {
            phase: RunPhase::Kernel
        }
    );
    assert!(started.elapsed() < Duration::from_millis(200));
}

struct CooperativeDeadlineBackend;

impl RelationalBackend for CooperativeDeadlineBackend {
    fn execute(
        &self,
        context: &RelationalContext<'_>,
        _: &CompiledRelationalPlan,
        _: &[RuntimeValue],
    ) -> Result<RelationalExecution, RelationalError> {
        assert_eq!(context.deadline, context.resource_owner.deadline());
        context.wait_for(Duration::from_secs(1))?;
        Ok(RelationalExecution {
            outputs: Vec::new(),
        })
    }
}

#[test]
fn deadline_is_propagated_into_cooperative_relational_context() {
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("deadline-backend", RelationalBackendId::new),
            CooperativeDeadlineBackend,
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.relational_subplans = Box::new([relational_subplan(
        "deadline-backend",
        "deadline-fragment",
        Box::new([]),
    )]);
    let started = std::time::Instant::now();

    let error = RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
        .with_relational_backends(&relational)
        .with_deadline(RunDeadline::after(Duration::from_millis(20)))
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert_eq!(
        error,
        RunError::DeadlineExceeded {
            phase: RunPhase::Kernel
        }
    );
    assert!(started.elapsed() < Duration::from_millis(200));
}

#[test]
fn deadline_stream_fast_paths_never_mutate_after_expiry() {
    let cancellation = CancellationToken::new();
    let (sender, receiver) = bounded_stream_channel_with_deadline(
        2,
        cancellation.clone(),
        Some(RunDeadline::after(Duration::ZERO)),
    )
    .unwrap();
    assert_eq!(sender.send(1), Err(StreamSendError::DeadlineExceeded(1)));
    assert_eq!(
        sender.try_send(2),
        Err(StreamSendError::DeadlineExceeded(2))
    );
    assert_eq!(
        receiver.try_recv(),
        Err(StreamReceiveError::DeadlineExceeded)
    );

    let deadline = RunDeadline::after(Duration::from_millis(20));
    let (sender, receiver) =
        bounded_stream_channel_with_deadline(1, cancellation.clone(), Some(deadline)).unwrap();
    sender.send(3).unwrap();
    thread::sleep(Duration::from_millis(25));
    assert_eq!(receiver.recv(), Err(StreamReceiveError::DeadlineExceeded));
    cancellation.cancel();
    assert_eq!(receiver.try_recv(), Err(StreamReceiveError::Cancelled));
    assert_eq!(sender.try_send(4), Err(StreamSendError::Cancelled(4)));
}

#[test]
fn worker_outcome_timestamp_precedes_envelope_preparation_delay() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("timestamp_boundary", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Err(KernelError::new("boundary ordinary error"))),
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("timestamp_boundary", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_deadline(RunDeadline::after(Duration::from_millis(20)))
        .with_test_checkpoint(Arc::new(|checkpoint, _| {
            if checkpoint == SchedulerCheckpoint::WorkerOutcomeProduced {
                thread::sleep(Duration::from_millis(40));
            }
        }))
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(
        error,
        RunError::KernelFailed { message, .. }
            if message.as_ref() == "boundary ordinary error"
    ));
}

#[test]
fn worker_panic_timestamp_is_captured_at_unwind_boundary() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("panic_timestamp_boundary", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| panic!("worker panic timestamp sentinel")),
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("panic_timestamp_boundary", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
            .with_deadline(RunDeadline::after(Duration::from_millis(20)))
            .with_test_checkpoint(Arc::new(|checkpoint, _| {
                if checkpoint == SchedulerCheckpoint::WorkerOutcomeProduced {
                    thread::sleep(Duration::from_millis(40));
                }
            }))
            .run(&execution_plan, CancellationToken::new());
    }));

    assert!(panic.is_err());
}

#[test]
fn deadline_drain_preserves_ordinary_error_completed_before_expiry() {
    let (completed_tx, completed_rx) = mpsc::sync_channel(1);
    let completed_rx = Arc::new(Mutex::new(completed_rx));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("parallel0", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                completed_tx.send(()).unwrap();
                Err(KernelError::new("completed before deadline"))
            }),
        )
        .unwrap();
    kernels
        .register(
            id("parallel1", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(1).into()])),
        )
        .unwrap();
    let observed_completion = Arc::clone(&completed_rx);
    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_scheduling_policy(parallel_policy(1, 1, 1))
        .with_deadline(RunDeadline::after(Duration::from_millis(100)))
        .with_test_checkpoint(Arc::new(move |checkpoint, _| {
            if checkpoint == SchedulerCheckpoint::AdmissionBlocked(WorkloadClass::Cpu) {
                observed_completion.lock().unwrap().recv().unwrap();
                thread::sleep(Duration::from_millis(120));
            }
        }))
        .run(
            &independent_parallel_plan(&[WorkloadClass::Cpu, WorkloadClass::Cpu]),
            CancellationToken::new(),
        )
        .unwrap_err();

    assert!(
        matches!(error, RunError::KernelFailed { message, .. } if message.as_ref() == "completed before deadline")
    );
}

struct CancelAfterDeadlineKernel;

impl Kernel for CancelAfterDeadlineKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        thread::sleep(Duration::from_millis(25));
        context.cancellation.cancel();
        Err(KernelError::cancelled("cancel after scheduler deadline"))
    }
}

#[test]
fn cancellation_observed_while_draining_upgrades_deadline() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("cancel_after_deadline", KernelHandle::new),
            CancelAfterDeadlineKernel,
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("cancel_after_deadline", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    assert_eq!(
        RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
            .with_deadline(RunDeadline::after(Duration::from_millis(5)))
            .run(&execution_plan, CancellationToken::new()),
        Err(RunError::Cancelled),
    );
}

#[test]
fn deadline_result_store_commit_gate_publishes_nothing() {
    let results = ResultStore::new();
    let events = RecordingRunEvents::default();
    results.set_commit_checkpoint_for_test(Arc::new(|| {
        thread::sleep(Duration::from_millis(25));
    }));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("deadline_commit", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(1).into()])),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![operation("deadline_commit", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "commit".into(),
        output: stable_output("commit"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_result_store(&results)
        .with_event_sink(&events)
        .with_deadline(RunDeadline::after(Duration::from_millis(10)))
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert_eq!(
        error,
        RunError::DeadlineExceeded {
            phase: RunPhase::ResultPublication
        }
    );
    assert_eq!(results.source_count(), 0);
    assert!(!events.0.lock().unwrap().iter().any(|event| matches!(
        event.kind,
        RunEventKind::ResultReady { .. }
            | RunEventKind::OutputReady { .. }
            | RunEventKind::RunCompleted
    )));
}

#[test]
fn deadline_cleanup_drains_an_uncooperative_task_after_recording_timeout() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("deadline_cleanup_long", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(Vec::new())),
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("deadline_cleanup_long", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    let started = std::time::Instant::now();
    let finalizer = |_: &mut RunResult, _: &CancellationToken, _: Option<RunDeadline>| Ok(());
    let root = materialization_test_root("cleanup-deadline-uncooperative");
    let checkpoint = Arc::new(|checkpoint, _: &CancellationToken| {
        if checkpoint == SchedulerCheckpoint::FinalResultPublication {
            // The synchronized cleanup task is installed by the owner test API.
        }
    });
    let owner_delay = Duration::from_millis(250);
    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_deadline(RunDeadline::after(Duration::from_millis(10)))
        .with_success_finalizer(&finalizer)
        .with_test_spill_root(root.clone())
        .with_test_checkpoint(checkpoint)
        .with_cleanup_delay_for_test(owner_delay)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert_eq!(
        error,
        RunError::DeadlineExceeded {
            phase: RunPhase::Cleanup
        }
    );
    assert!(started.elapsed() >= owner_delay);
    assert!(
        !root.exists()
            || std::fs::read_dir(&root)
                .expect("cleanup spill root remains readable")
                .next()
                .is_none()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rust_error_outcomes_are_strict_by_construction() {
    assert_eq!(
        RunErrorOutcome::from(&RunError::DeadlineExceeded {
            phase: RunPhase::Kernel
        }),
        RunErrorOutcome::DeadlineExceeded {
            phase: RunPhase::Kernel
        },
    );
    assert_eq!(
        RunErrorOutcome::from(&RunError::KernelFailed {
            operation: OperationIndex::new(0),
            kind: KernelErrorKind::Permanent,
            message: "failed".into(),
        }),
        RunErrorOutcome::Ordinary {
            code: OrdinaryRunErrorCode::KernelFailed
        },
    );
}

#[test]
fn cancellation_wakes_blocked_stream_send_and_receive() {
    let token = CancellationToken::new();
    let (sender, _receiver) = bounded_stream_channel(1, token.clone()).unwrap();
    sender.send(1).unwrap();
    let blocked_sender = sender.clone();
    let send = thread::spawn(move || blocked_sender.send(2));
    thread::sleep(Duration::from_millis(20));
    token.cancel();
    assert_eq!(send.join().unwrap(), Err(StreamSendError::Cancelled(2)));

    let receive_token = CancellationToken::new();
    let (_sender, receiver) = bounded_stream_channel::<i32>(1, receive_token.clone()).unwrap();
    let receive = thread::spawn(move || receiver.recv());
    thread::sleep(Duration::from_millis(20));
    receive_token.cancel();
    assert_eq!(receive.join().unwrap(), Err(StreamReceiveError::Cancelled));
}

struct StreamingRelationalBackend {
    observed: Arc<Mutex<Option<StreamValue>>>,
    sender: Mutex<Option<BoundedStreamSender<Value>>>,
}

impl RelationalBackend for StreamingRelationalBackend {
    fn execute(
        &self,
        context: &RelationalContext<'_>,
        _: &CompiledRelationalPlan,
        _: &[RuntimeValue],
    ) -> Result<RelationalExecution, RelationalError> {
        let (sender, receiver) = bounded_stream_channel(1, context.cancellation.clone())
            .map_err(|_| RelationalError::operator_invalid("stream setup failed"))?;
        let stream = StreamValue::from_receiver(receiver);
        *self.sender.lock().unwrap() = Some(sender);
        *self.observed.lock().unwrap() = Some(stream.clone());
        Ok(RelationalExecution {
            outputs: vec![RuntimeValue::Stream(stream)],
        })
    }
}

#[test]
fn run_cleanup_closes_relational_streams() {
    let observed = Arc::new(Mutex::new(None));
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(
            id("single", RelationalBackendId::new),
            StreamingRelationalBackend {
                observed: observed.clone(),
                sender: Mutex::new(None),
            },
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.relational_subplans =
        Box::new([relational_subplan("single", "sales", Box::new([]))]);

    RunExecutor::new(&KernelRegistry::new(), &no_resources(), &NoFunctions)
        .with_relational_backends(&relational)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert!(observed.lock().unwrap().as_ref().unwrap().is_closed());
}

struct FailingRelationalBackend;

impl RelationalBackend for FailingRelationalBackend {
    fn execute(
        &self,
        _: &RelationalContext<'_>,
        _: &CompiledRelationalPlan,
        _: &[RuntimeValue],
    ) -> Result<RelationalExecution, RelationalError> {
        Err(RelationalError::operator_invalid(
            "relational execution failed",
        ))
    }
}

#[test]
fn relational_failure_releases_run_resources_and_backend_lease() {
    let resources = no_resources();
    let released_resources = resources.released.clone();
    let released_backends = Arc::new(AtomicUsize::new(0));
    let provider = TrackingRelationalProvider {
        released: released_backends.clone(),
    };
    let mut execution_plan = plan(
        vec![relational_operation(0, &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.resources = Box::new([requirement("temporary")]);
    execution_plan.relational_subplans =
        Box::new([relational_subplan("single", "sales", Box::new([]))]);

    let error = RunExecutor::new(&KernelRegistry::new(), &resources, &NoFunctions)
        .with_relational_backends(&provider)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, RunError::RelationalFailed { .. }));
    assert_eq!(released_resources.load(Ordering::SeqCst), 1);
    assert_eq!(released_backends.load(Ordering::SeqCst), 1);
}

struct TrackingRelationalProvider {
    released: Arc<AtomicUsize>,
}

struct TrackingRelationalLease {
    backend: FailingRelationalBackend,
    released: Arc<AtomicUsize>,
}

impl Drop for TrackingRelationalLease {
    fn drop(&mut self) {
        self.released.fetch_add(1, Ordering::SeqCst);
    }
}

impl RelationalBackendLease for TrackingRelationalLease {
    fn backend(&self) -> &dyn RelationalBackend {
        &self.backend
    }
}

impl RelationalBackendProvider for TrackingRelationalProvider {
    fn acquire(
        &self,
        _: &RelationalBackendId,
        _: &RunResourceSet,
        _: &CancellationToken,
    ) -> Result<Box<dyn RelationalBackendLease>, RelationalError> {
        Ok(Box::new(TrackingRelationalLease {
            backend: FailingRelationalBackend,
            released: self.released.clone(),
        }))
    }
}
