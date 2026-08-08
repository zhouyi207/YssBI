use super::*;
use crate::node_system::analysis::{
    CompilationBasis, CompileId, CompileProvenance, CorrelationContext, ProjectSessionId,
    ResourceKey, ResourceVersion, SpanEvent, SpanKind, SpanStatus, TraceSink, TraceValue,
};
use crate::node_system::document::{
    FunctionParameterId, GraphResourcePath, GraphRevision, NodeId, PortAddress,
};
use crate::node_system::plan::*;
use crate::node_system::protocol::{
    CachePolicy, InputConsumption, NodeTypeId, OutputProduction, PortKey, Value,
};
use crate::node_system::registry::RegistryFingerprint;
use std::any::Any;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex, mpsc};
use std::thread;
use std::time::Duration;

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
                consumption: InputConsumption::FullyMaterialized,
                bound_value: None,
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
        resource_dependencies: Box::new([]),
        cache_policy: CachePolicy::Disabled,
        semantics_version: ExecutionSemanticsVersion::from_bytes([1; 32]),
        workload: WorkloadClass::Cpu,
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
        value_sources: Box::new([]),
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
            parameters,
            result_productions: results
                .keys()
                .cloned()
                .map(|parameter| (parameter, OutputProduction::FullyMaterialized))
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

fn requirement(name: &str) -> CompiledResourceRequirement {
    CompiledResourceRequirement {
        resource: id(name, ResourceId::new),
        kind: ResourceKind::TemporaryStorage,
        access: ResourceAccess::Exclusive,
        optional: false,
    }
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
        PlanValueSource::ControlProduced(ValueRef::new(1), OutputProduction::FullyMaterialized),
        PlanValueSource::ControlProduced(ValueRef::new(4), OutputProduction::FullyMaterialized),
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
        RuntimeValue::from(Value::Integer(3))
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
            results: BTreeMap::new(),
            result_productions: BTreeMap::new(),
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
            result_productions: BTreeMap::new(),
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
) -> (Result<RunResult, RunError>, ExecutionPlan, Vec<SpanEvent>) {
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

fn assert_relational_backend_trace(
    execution_plan: &ExecutionPlan,
    events: &[SpanEvent],
    terminal_status: SpanStatus,
) {
    let spans = events
        .iter()
        .filter(|event| event.kind == SpanKind::RelationalBackend)
        .collect::<Vec<_>>();
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].status, SpanStatus::Started);
    assert_eq!(spans[1].status, terminal_status);
    assert_eq!(spans[0].correlation, spans[1].correlation);

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

    let expected_fields = BTreeMap::from([
        (
            Box::<str>::from("backendId"),
            TraceValue::Text("trace-backend".into()),
        ),
        (Box::<str>::from("subplanIndex"), TraceValue::Integer(0)),
    ]);
    assert_eq!(spans[0].fields, expected_fields);
    assert_eq!(spans[1].fields, expected_fields);
}

#[test]
fn relational_backend_trace_records_success_with_full_operation_correlation() {
    let (result, execution_plan, events) =
        run_relational_backend_trace(TraceRelationalOutcome::Succeed);

    result.unwrap();
    assert_relational_backend_trace(&execution_plan, &events, SpanStatus::Succeeded);
}

#[test]
fn relational_backend_trace_records_failure_with_full_operation_correlation() {
    let (result, execution_plan, events) =
        run_relational_backend_trace(TraceRelationalOutcome::Fail);

    assert!(matches!(result, Err(RunError::RelationalFailed { .. })));
    assert_relational_backend_trace(&execution_plan, &events, SpanStatus::Failed);
}

#[test]
fn relational_backend_trace_records_cancellation_with_full_operation_correlation() {
    let (result, execution_plan, events) =
        run_relational_backend_trace(TraceRelationalOutcome::Cancel);

    assert_eq!(result, Err(RunError::Cancelled));
    assert_relational_backend_trace(&execution_plan, &events, SpanStatus::Cancelled);
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
    let finalizer = |_: &mut RunResult, _: &CancellationToken| {
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
            code: RunErrorCode::ResourceSnapshotMismatch
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
            FnKernel(|_: &[RuntimeValue]| {
                Ok(vec![RuntimeValue::Stream(
                    StreamValue::from_values(
                        [Value::Integer(7), Value::Integer(8)],
                        CancellationToken::new(),
                    )
                    .unwrap(),
                )])
            }),
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
                            && artifact.values() == [Value::Integer(7), Value::Integer(8)]
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
            consumption: InputConsumption::Streaming,
            bound_value: None,
        }]),
        outputs: Box::new([PlannedOutput {
            value: ValueRef::new(2),
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
fn shared_materialized_fanout_delivers_complete_data_to_same_and_different_consumers() {
    for different_contracts in [false, true] {
        let observed = Arc::new(Mutex::new(Vec::new()));
        let mut kernels = KernelRegistry::new();
        kernels
            .register(
                id("fanout_source", KernelHandle::new),
                FnKernel(|_: &[RuntimeValue]| {
                    Ok(vec![RuntimeValue::Stream(
                        StreamValue::from_values(
                            [Value::Integer(1), Value::Integer(2), Value::Integer(3)],
                            CancellationToken::new(),
                        )
                        .unwrap(),
                    )])
                }),
            )
            .unwrap();
        for name in ["fanout_a", "fanout_b"] {
            let observed = Arc::clone(&observed);
            kernels
                .register(
                    id(name, KernelHandle::new),
                    FnKernel(move |inputs: &[RuntimeValue]| {
                        let count = match &inputs[0] {
                            RuntimeValue::Artifact(artifact) => artifact.values().len(),
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
                    consumption,
                    bound_value: None,
                }]),
                outputs: Box::new([PlannedOutput {
                    value: ValueRef::new(output),
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
                StreamValue::from_values([Value::Integer(7)], CancellationToken::new()).unwrap(),
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
        let output =
            execute_planned_adapter(&planned.adapter, input, &CancellationToken::new()).unwrap();
        match (shape, output) {
            (Shape::Stream, RuntimeValue::Stream(_)) => {}
            (Shape::Artifact(expected), RuntimeValue::Artifact(actual)) => {
                assert_eq!(actual.kind(), expected);
                assert_eq!(actual.values(), &[Value::Integer(7)]);
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
    let stream_token = CancellationToken::new();
    let stream = RuntimeValue::Stream(
        StreamValue::from_values([Value::Integer(1)], stream_token.clone()).unwrap(),
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
                StreamValue::from_values([Value::Integer(2)], stream_token.clone()).unwrap(),
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
