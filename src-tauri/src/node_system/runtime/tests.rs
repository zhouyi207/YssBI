use super::*;
use crate::node_system::analysis::{
    CompilationBasis, CompileId, CompileProvenance, ProjectSessionId, SpanEvent, SpanKind,
    SpanStatus, TraceSink,
};
use crate::node_system::document::{GraphResourcePath, GraphRevision, NodeId};
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

struct NoFunctions;

impl FunctionPlanProvider for NoFunctions {
    fn get_plan(&self, _: &FunctionPlanHandle) -> Result<Option<Arc<ExecutionPlan>>, Box<str>> {
        Ok(None)
    }
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
    let execution_plan = plan(
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
    struct OneFunction(Arc<ExecutionPlan>);
    impl FunctionPlanProvider for OneFunction {
        fn get_plan(&self, _: &FunctionPlanHandle) -> Result<Option<Arc<ExecutionPlan>>, Box<str>> {
            Ok(Some(self.0.clone()))
        }
    }

    let mut callee = plan(vec![], 0, StructuredControlRegion::Sequence(Box::new([])));
    callee.provenance.compile_id = CompileId::new(22);
    callee.provenance.graph_path = GraphResourcePath("functions/callee".into());
    let mut caller = plan(
        vec![],
        0,
        StructuredControlRegion::Call {
            target: id("callee", FunctionPlanHandle::new),
            arguments: Box::new([]),
            results: Box::new([]),
        },
    );
    caller.provenance.compile_id = CompileId::new(11);
    let trace = RecordingTrace::default();

    RunExecutor::new(
        &KernelRegistry::new(),
        &no_resources(),
        &OneFunction(Arc::new(callee)),
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
    struct OneFunction(Arc<ExecutionPlan>);
    impl FunctionPlanProvider for OneFunction {
        fn get_plan(&self, _: &FunctionPlanHandle) -> Result<Option<Arc<ExecutionPlan>>, Box<str>> {
            Ok(Some(self.0.clone()))
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
                target: id("callee", FunctionPlanHandle::new),
                arguments: Box::new([]),
                results: Box::new([]),
            })),
        ])),
    );

    RunExecutor::new(&kernels, &no_resources(), &OneFunction(callee))
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
fn recursive_calls_stop_at_the_configured_limit() {
    struct RecursiveFunction(Arc<ExecutionPlan>);
    impl FunctionPlanProvider for RecursiveFunction {
        fn get_plan(&self, _: &FunctionPlanHandle) -> Result<Option<Arc<ExecutionPlan>>, Box<str>> {
            Ok(Some(self.0.clone()))
        }
    }
    let recursive = Arc::new(plan(
        vec![],
        0,
        StructuredControlRegion::Call {
            target: id("recursive", FunctionPlanHandle::new),
            arguments: Box::new([]),
            results: Box::new([]),
        },
    ));
    let resources = no_resources();

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &resources,
        &RecursiveFunction(recursive.clone()),
    )
    .with_recursion_limit(3)
    .run(&recursive, CancellationToken::new())
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
    struct OneFunction(Arc<ExecutionPlan>);
    impl FunctionPlanProvider for OneFunction {
        fn get_plan(&self, _: &FunctionPlanHandle) -> Result<Option<Arc<ExecutionPlan>>, Box<str>> {
            Ok(Some(self.0.clone()))
        }
    }
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
            target: id("callee", FunctionPlanHandle::new),
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
        &OneFunction(Arc::new(callee)),
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
