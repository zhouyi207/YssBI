use super::*;
use crate::node_system::analysis::{
    CompilationBasis, CompileId, CompileProvenance, CorrelationContext, ProjectSessionId,
    ResourceKey, ResourceVersion, SpanEvent, SpanKind, SpanStatus, TraceSink,
};
use crate::node_system::document::{FunctionParameterId, GraphResourcePath, GraphRevision, NodeId};
use crate::node_system::plan::*;
use crate::node_system::protocol::{InputConsumption, NodeTypeId, OutputProduction, Value};
use crate::node_system::registry::RegistryFingerprint;
use std::any::Any;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn id<T>(value: &str, constructor: impl FnOnce(Box<str>) -> Result<T, InvalidPlanId>) -> T {
    constructor(value.into()).unwrap()
}

fn operation(kernel: &str, inputs: &[u32], outputs: &[u32]) -> PlannedOperation {
    PlannedOperation {
        source_node_id: NodeId::from_uuid(uuid::Uuid::nil()),
        source_node_type_id: NodeTypeId::new(format!("yssbi.test.{kernel}")).unwrap(),
        kernel: PlannedKernel::Native(id(kernel, KernelHandle::new)),
        inputs: inputs
            .iter()
            .map(|value| PlannedInput {
                value: ValueRef::new(*value),
                consumption: InputConsumption::FullyMaterialized,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        outputs: outputs
            .iter()
            .map(|value| PlannedOutput {
                value: ValueRef::new(*value),
                production: OutputProduction::FullyMaterialized,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        params: id("params", CompiledParameterHandle::new),
    }
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
            },
            compile_id: CompileId::new(1),
        },
        value_count,
        value_sources: Box::new([]),
        operations: operations.into_boxed_slice(),
        value_dependencies: Box::new([]),
        root_region,
        effect_dependencies: Box::new([]),
        relational_subplans: Box::new([]),
        resources: Box::new([]),
        results: Box::new([]),
    }
}

struct FnKernel<F>(F);

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
struct RecordingTrace(Mutex<Vec<SpanEvent>>);

impl TraceSink for RecordingTrace {
    fn record(&self, event: SpanEvent) {
        self.0.lock().unwrap().push(event);
    }
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
    assert!(
        events
            .iter()
            .all(|event| !matches!(event.kind, RunEventKind::ResultReady { .. }))
    );
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
    let parameters = parameters
        .iter()
        .enumerate()
        .map(|(index, value)| {
            (
                FunctionParameterId(format!("parameter-{index}").into()),
                ValueRef::new(*value),
            )
        })
        .collect();
    let results = results
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
            parameters,
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

fn requirement(name: &str) -> CompiledResourceRequirement {
    CompiledResourceRequirement {
        resource: id(name, ResourceId::new),
        kind: ResourceKind::TemporaryStorage,
        access: ResourceAccess::Exclusive,
        optional: false,
    }
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
        value: ValueRef::new(1),
    }]);

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
                }]),
            })),
        ])),
    );
    execution_plan.value_sources = Box::new([PlanValueSource::ControlProduced(ValueRef::new(3))]);

    RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(counts.lock().unwrap().get("then"), Some(&1));
    assert_eq!(counts.lock().unwrap().get("else"), None);
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
fn ordinary_kernel_error_is_not_hidden_by_simultaneous_token_cancellation() {
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

    assert!(matches!(
        error,
        RunError::KernelFailed { message, .. } if message.as_ref() == "ordinary failure"
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
    let released = Arc::new(AtomicUsize::new(0));
    let resources = TrackingResources {
        acquired: Arc::new(AtomicUsize::new(0)),
        released: released.clone(),
        fail_at: Some(2),
    };
    let mut execution_plan = plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([])));
    execution_plan.resources = Box::new([requirement("one"), requirement("two")]);

    let error = RunExecutor::new(&KernelRegistry::new(), &resources, &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert!(matches!(error, RunError::ResourceAcquire { .. }));
    assert_eq!(released.load(Ordering::SeqCst), 1);
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
        .filter(|event| event.kind == SpanKind::Cleanup && event.correlation.parent_call.is_none())
        .map(|event| event.status)
        .collect::<Vec<_>>();
    assert_eq!(
        cleanup,
        vec![
            SpanStatus::Succeeded,
            SpanStatus::Failed,
            SpanStatus::Cancelled
        ]
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
        .find(|event| {
            event.kind == SpanKind::Run
                && event.status == SpanStatus::Started
                && event.correlation.compile_id == CompileId::new(22)
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
    let mut execution_plan = plan(
        vec![
            operation("initial", &[], &[0]),
            operation("loop", &[1], &[2, 3]),
        ],
        5,
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
                }]),
                continue_condition: ValueRef::new(3),
                max_iterations: 4,
            })),
        ])),
    );
    execution_plan.value_sources = Box::new([
        PlanValueSource::ControlProduced(ValueRef::new(1)),
        PlanValueSource::ControlProduced(ValueRef::new(4)),
    ]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        value: ValueRef::new(4),
    }]);

    let result = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(
        result.values["result"],
        RuntimeValue::from(Value::Integer(3))
    );
    let activations = activations.lock().unwrap();
    assert_eq!(activations.len(), 3);
    assert!(activations.windows(2).all(|pair| pair[0] != pair[1]));
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
    callee.value_sources = Box::new([PlanValueSource::ExternalInput(ValueRef::new(1))]);
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
                }]),
            })),
        ])),
    );
    caller.value_sources = Box::new([PlanValueSource::ControlProduced(ValueRef::new(0))]);
    caller.results = Box::new([PlanResult {
        name: "answer".into(),
        value: ValueRef::new(0),
    }]);

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
            results: BTreeMap::new(),
        }),
    });
    let caller = plan(
        vec![],
        0,
        StructuredControlRegion::Call {
            target: id("functions/callee", FunctionPlanHandle::new),
            arguments: Box::new([]),
            results: Box::new([]),
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
    }

    for case in [
        InvalidCall::MissingArgument,
        InvalidCall::MissingResult,
        InvalidCall::DuplicateCalleeArgument,
        InvalidCall::DuplicateCalleeResult,
        InvalidCall::DuplicateCallerResult,
        InvalidCall::OutOfBoundsParameter,
        InvalidCall::UnsourcedResult,
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
        callee.value_sources = Box::new([PlanValueSource::ExternalInput(ValueRef::new(0))]);
        callee.resources = Box::new([CompiledResourceRequirement {
            resource: id("external/callee", ResourceId::new),
            kind: ResourceKind::ExternalArtifact,
            access: ResourceAccess::Shared,
            optional: false,
        }]);
        let mut published = published_function(callee, "functions/callee", &[0], &[2]);

        let standard_argument = CallArgumentBinding {
            caller_source: ValueRef::new(0),
            callee_destination: ValueRef::new(0),
        };
        let standard_result = CallResultBinding {
            callee_source: ValueRef::new(2),
            caller_destination: ValueRef::new(1),
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
                    }],
                )
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
                })),
            ])),
        );
        let mut destinations = BTreeMap::new();
        if !matches!(case, InvalidCall::MissingResult) {
            destinations.insert(
                ValueRef::new(1),
                PlanValueSource::ControlProduced(ValueRef::new(1)),
            );
        }
        if matches!(case, InvalidCall::DuplicateCalleeResult) {
            destinations.insert(
                ValueRef::new(3),
                PlanValueSource::ControlProduced(ValueRef::new(3)),
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
        PlanValueSource::ExternalInput(ValueRef::new(0)),
        PlanValueSource::ExternalInput(ValueRef::new(1)),
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
            results: BTreeMap::new(),
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
    operation.kernel = PlannedKernel::Relational(RelationalSubplanIndex::new(subplan));
    operation
}

fn relational_subplan(
    backend: &str,
    fragment: &str,
    bridges: Box<[PlannedMaterializationBridge]>,
) -> RelationalSubplan {
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
            bridge_inputs: Box::new([]),
            requested_fragment_outputs: Box::new([]),
            roots: Box::new([RelationalOperatorIndex::new(0)]),
            pushdown_hints: Box::new([]),
        },
        materialization_bridges: bridges,
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
        bridge_inputs: &[RelationalInput],
    ) -> Result<RelationalExecution, RelationalError> {
        context
            .cancellation
            .check()
            .map_err(RelationalError::from)?;
        assert!(operation_inputs.is_empty());
        assert!(bridge_inputs.is_empty());
        self.executions
            .lock()
            .unwrap()
            .push(plan.fragment_order[0].as_str().into());
        Ok(RelationalExecution {
            outputs: vec![Value::Integer(41).into()],
            fragment_outputs: BTreeMap::from([(
                plan.fragment_order[0].clone(),
                Value::Integer(41).into(),
            )]),
        })
    }
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
    let provider = ProjectResourceProvider::new(
        ProjectResourceSnapshot::new(
            ProjectSessionId::new("test-session"),
            resource_versions.clone(),
        )
        .with_database(resource.clone(), Arc::new(dataframe)),
    );
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
            bridge_inputs: Box::new([]),
            requested_fragment_outputs: Box::new([]),
            roots: Box::new([RelationalOperatorIndex::new(0)]),
            pushdown_hints: Box::new([]),
        },
        materialization_bridges: Box::new([]),
    }]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        value: ValueRef::new(0),
    }]);
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
        value: ValueRef::new(0),
    }]);

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

fn exact_bridge_scheduler_fixture() -> (KernelRegistry, RelationalBackendRegistry, ExecutionPlan) {
    let backend = id("production", RelationalBackendId::new);
    let producer_decoy = id("producer-decoy", RelationalFragmentId::new);
    let producer_exact = id("producer-exact", RelationalFragmentId::new);
    let consumer = id("consumer", RelationalFragmentId::new);
    let decoy_bridge = PlannedMaterializationBridge {
        producer_fragment: producer_decoy.clone(),
        consumer_fragment: consumer.clone(),
        producer_subplan: RelationalSubplanIndex::new(0),
        consumer_subplan: RelationalSubplanIndex::new(1),
        bridge: MaterializationBridge::Collect,
    };
    let exact_bridge = PlannedMaterializationBridge {
        producer_fragment: producer_exact.clone(),
        consumer_fragment: consumer.clone(),
        producer_subplan: RelationalSubplanIndex::new(0),
        consumer_subplan: RelationalSubplanIndex::new(1),
        bridge: MaterializationBridge::Collect,
    };

    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("bridge_values", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| {
                Ok(vec![Value::Integer(13).into(), Value::Integer(41).into()])
            }),
        )
        .unwrap();
    let mut relational = RelationalBackendRegistry::new();
    relational
        .register(backend.clone(), ProductionRelationalBackend::default())
        .unwrap();

    let source = operation("bridge_values", &[], &[0, 1]);
    let mut producer_operation = relational_operation(0, &[2, 3]);
    producer_operation.inputs = Box::new([
        PlannedInput {
            value: ValueRef::new(0),
            consumption: InputConsumption::FullyMaterialized,
        },
        PlannedInput {
            value: ValueRef::new(1),
            consumption: InputConsumption::FullyMaterialized,
        },
    ]);
    let consumer_operation = relational_operation(1, &[4]);
    let mut execution_plan = plan(
        vec![source, producer_operation, consumer_operation],
        5,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
            ControlStep::Operation(OperationIndex::new(2)),
        ])),
    );
    execution_plan.relational_subplans = Box::new([
        RelationalSubplan {
            backend: backend.clone(),
            compiled_plan: CompiledRelationalPlan {
                fragment_order: Box::new([producer_decoy.clone(), producer_exact.clone()]),
                operators: Box::new([
                    RelationalOperator::Input {
                        name: "same-name".into(),
                    },
                    RelationalOperator::Input {
                        name: "same-name".into(),
                    },
                ]),
                fragment_roots: Box::new([
                    RelationalFragmentRoot {
                        fragment: producer_decoy.clone(),
                        operator: RelationalOperatorIndex::new(0),
                    },
                    RelationalFragmentRoot {
                        fragment: producer_exact.clone(),
                        operator: RelationalOperatorIndex::new(1),
                    },
                ]),
                bridge_inputs: Box::new([]),
                requested_fragment_outputs: Box::new([producer_decoy, producer_exact]),
                roots: Box::new([
                    RelationalOperatorIndex::new(0),
                    RelationalOperatorIndex::new(1),
                ]),
                pushdown_hints: Box::new([]),
            },
            materialization_bridges: Box::new([]),
        },
        RelationalSubplan {
            backend,
            compiled_plan: CompiledRelationalPlan {
                fragment_order: Box::new([consumer.clone()]),
                operators: Box::new([
                    RelationalOperator::Input {
                        name: "same-name".into(),
                    },
                    RelationalOperator::Input {
                        name: "same-name".into(),
                    },
                ]),
                fragment_roots: Box::new([RelationalFragmentRoot {
                    fragment: consumer,
                    operator: RelationalOperatorIndex::new(0),
                }]),
                bridge_inputs: Box::new([
                    RelationalBridgeInput {
                        operator: RelationalOperatorIndex::new(0),
                        bridge: exact_bridge.clone(),
                    },
                    RelationalBridgeInput {
                        operator: RelationalOperatorIndex::new(1),
                        bridge: decoy_bridge.clone(),
                    },
                ]),
                requested_fragment_outputs: Box::new([]),
                roots: Box::new([RelationalOperatorIndex::new(0)]),
                pushdown_hints: Box::new([]),
            },
            materialization_bridges: Box::new([decoy_bridge, exact_bridge]),
        },
    ]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        value: ValueRef::new(4),
    }]);
    (kernels, relational, execution_plan)
}

#[test]
fn scheduler_publishes_and_consumes_exact_relational_fragment_bridges() {
    let (kernels, relational, execution_plan) = exact_bridge_scheduler_fixture();
    let result = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_relational_backends(&relational)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    let RuntimeValue::Artifact(value) = &result.values["result"] else {
        panic!("consumer output must be a materialized bridge artifact");
    };
    assert_eq!(value.kind(), ArtifactKind::Collected);
    assert_eq!(value.values(), &[Value::Integer(41)]);
}

#[test]
fn cancellation_at_exact_bridge_materialization_cleans_results_without_completion() {
    use super::scheduler::SchedulerCheckpoint;

    let (kernels, relational, execution_plan) = exact_bridge_scheduler_fixture();
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let observed_for_hook = Arc::clone(&observed);

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_relational_backends(&relational)
        .with_event_sink(&events)
        .with_result_store(&results)
        .with_test_checkpoint(Arc::new(move |checkpoint, cancellation| {
            observed_for_hook.lock().unwrap().push(checkpoint);
            if checkpoint == SchedulerCheckpoint::BridgeMaterialization {
                cancellation.cancel();
            }
        }))
        .run(&execution_plan, CancellationToken::new())
        .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[SchedulerCheckpoint::BridgeMaterialization]
    );
    assert_cancelled_without_completion(&events);
    assert_eq!(results.source_count(), 0);
}

#[test]
fn cancellation_during_exact_bridge_stream_collection_is_cancelled() {
    use super::scheduler::SchedulerCheckpoint;

    let cancellation = CancellationToken::new();
    let stream = StreamValue::from_values([Value::Integer(41)], cancellation.clone()).unwrap();
    let (mut kernels, relational, mut execution_plan) = exact_bridge_scheduler_fixture();
    kernels
        .register(
            id("bridge_stream", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                Ok(vec![
                    Value::Integer(13).into(),
                    RuntimeValue::Stream(stream.clone()),
                ])
            }),
        )
        .unwrap();
    execution_plan.operations[0].kernel =
        PlannedKernel::Native(id("bridge_stream", KernelHandle::new));
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();
    let collected = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&collected);

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_relational_backends(&relational)
        .with_event_sink(&events)
        .with_result_store(&results)
        .with_test_checkpoint(Arc::new(move |checkpoint, cancellation| {
            if checkpoint == SchedulerCheckpoint::BridgeCollectionStarted {
                observed.fetch_add(1, Ordering::SeqCst);
                cancellation.cancel();
            }
        }))
        .run(&execution_plan, cancellation)
        .unwrap_err();

    assert_eq!(error, RunError::Cancelled);
    assert_eq!(collected.load(Ordering::SeqCst), 1);
    assert_cancelled_without_completion(&events);
    assert_eq!(results.source_count(), 0);
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
            value: ValueRef::new(0),
        },
        PlanResult {
            name: "second".into(),
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
fn cancellation_after_value_ready_preserves_an_older_committed_source() {
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
            .all(|event| !matches!(event.kind, RunEventKind::ValueReady { .. }))
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

    let (kernels, relational, execution_plan) = exact_bridge_scheduler_fixture();
    let events = RecordingRunEvents::default();
    let results = ResultStore::new();
    let final_checkpoints = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&final_checkpoints);

    let error = RunExecutor::new(&kernels, &no_resources(), &NoFunctions)
        .with_relational_backends(&relational)
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
fn materialization_bridges_preserve_their_explicit_semantics() {
    let token = CancellationToken::new();
    let values = || RuntimeValue::from(Value::Integer(7));

    let stream = materialize_bridge(MaterializationBridge::Stream, values(), &token).unwrap();
    assert!(matches!(stream, RuntimeValue::Stream(_)));

    for (bridge, expected) in [
        (MaterializationBridge::Buffer, ArtifactKind::Buffered),
        (MaterializationBridge::Collect, ArtifactKind::Collected),
        (MaterializationBridge::Spill, ArtifactKind::Spilled),
        (MaterializationBridge::Replay, ArtifactKind::Replayable),
    ] {
        let RuntimeValue::Artifact(artifact) =
            materialize_bridge(bridge, values(), &token).unwrap()
        else {
            panic!("bridge must produce an artifact");
        };
        assert_eq!(artifact.kind(), expected);
        assert_eq!(artifact.values(), &[Value::Integer(7)]);
    }

    let producer = id("producer", RelationalFragmentId::new);
    let decoy_producer = id("decoy-producer", RelationalFragmentId::new);
    let consumer = id("consumer", RelationalFragmentId::new);
    let expected_bridge = PlannedMaterializationBridge {
        producer_fragment: producer,
        consumer_fragment: consumer.clone(),
        producer_subplan: RelationalSubplanIndex::new(0),
        consumer_subplan: RelationalSubplanIndex::new(1),
        bridge: MaterializationBridge::Collect,
    };
    let decoy_bridge = PlannedMaterializationBridge {
        producer_fragment: decoy_producer,
        consumer_fragment: consumer.clone(),
        producer_subplan: RelationalSubplanIndex::new(2),
        consumer_subplan: RelationalSubplanIndex::new(1),
        bridge: MaterializationBridge::Collect,
    };
    let plan = CompiledRelationalPlan {
        fragment_order: Box::new([consumer.clone()]),
        operators: Box::new([RelationalOperator::Input {
            name: "ambiguous-display-name".into(),
        }]),
        fragment_roots: Box::new([RelationalFragmentRoot {
            fragment: consumer,
            operator: RelationalOperatorIndex::new(0),
        }]),
        bridge_inputs: Box::new([RelationalBridgeInput {
            operator: RelationalOperatorIndex::new(0),
            bridge: expected_bridge.clone(),
        }]),
        requested_fragment_outputs: Box::new([]),
        roots: Box::new([RelationalOperatorIndex::new(0)]),
        pushdown_hints: Box::new([]),
    };
    let resources = RunResourceSet::acquire(&[], &no_resources()).unwrap();
    let context = RelationalContext {
        run_id: RunId::new(1),
        resources: &resources,
        cancellation: &token,
    };
    let execution = ProductionRelationalBackend::default()
        .execute(
            &context,
            &plan,
            &[RuntimeValue::from(Value::Integer(99))],
            &[
                RelationalInput {
                    bridge: decoy_bridge,
                    value: RuntimeValue::from(Value::Integer(13)),
                },
                RelationalInput {
                    bridge: expected_bridge,
                    value: RuntimeValue::from(Value::Integer(41)),
                },
            ],
        )
        .unwrap();

    assert_eq!(
        execution.outputs,
        vec![RuntimeValue::from(Value::Integer(41))]
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
        plan: &CompiledRelationalPlan,
        _: &[RuntimeValue],
        _: &[RelationalInput],
    ) -> Result<RelationalExecution, RelationalError> {
        let (sender, receiver) = bounded_stream_channel(1, context.cancellation.clone())
            .map_err(|error| RelationalError::new(error.to_string()))?;
        let stream = StreamValue::from_receiver(receiver);
        *self.sender.lock().unwrap() = Some(sender);
        *self.observed.lock().unwrap() = Some(stream.clone());
        Ok(RelationalExecution {
            outputs: vec![RuntimeValue::Stream(stream.clone())],
            fragment_outputs: BTreeMap::from([(
                plan.fragment_order[0].clone(),
                RuntimeValue::Stream(stream),
            )]),
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
        _: &[RelationalInput],
    ) -> Result<RelationalExecution, RelationalError> {
        Err(RelationalError::new("relational execution failed"))
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
