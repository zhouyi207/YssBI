mod cancellation;
mod control_flow;
mod data_series;
mod deadlines;
mod function_calls;
mod materialization;
mod memoization;
mod relational;
mod resources;
mod results;
mod retry;
mod scheduler;
mod streams;

use super::scheduler::SchedulerCheckpoint;
use super::*;
use crate::graph_document::{
    FunctionParameterId, GraphResourcePath, GraphRevision, NodeId, PortAddress,
};
use crate::node_system::ProjectSessionId;
use crate::node_system::analysis::{
    CompilationBasis, CompileId, CompileProvenance, ResourceKey, ResourceVersion,
};
use crate::node_system::plan::*;
use crate::node_system::protocol::{
    CachePolicy, CanonicalDecimal, InputConsumption, NodeTypeId, OutputProduction, PortKey,
    RetryPolicy, TypeExpr, TypeId, Value,
};
use crate::node_system::registry::RegistryFingerprint;
use crate::project::{NumericTolerance, StatisticalMissingValuePolicy};
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Condvar, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

fn id<T>(value: &str, constructor: impl FnOnce(Box<str>) -> Result<T, InvalidPlanId>) -> T {
    constructor(value.into()).unwrap()
}

fn stable_output(port_key: &str) -> GraphOutputRef {
    GraphOutputRef {
        graph_path: GraphResourcePath::new("events/test").unwrap(),
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
                public_output: None,
                presentation: crate::node_system::plan::ResultPresentation::Inspector,
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
        kernel: PlannedKernel::Adapter(
            contract
                .adapter
                .expect("adapter operation helper requires a conversion"),
        ),
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
            public_output: None,
            presentation: crate::node_system::plan::ResultPresentation::Inspector,
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
            graph_path: GraphResourcePath::new("events/test").unwrap(),
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
struct RecordingRunEvents(Mutex<Vec<RunEvent>>);

impl RunEventSink for RecordingRunEvents {
    fn record(&self, event: RunEvent) {
        self.0.lock().unwrap().push(event);
    }
}

fn assert_cancelled_without_completion(events: &RecordingRunEvents) -> RunId {
    let events = events.0.lock().unwrap();
    let run_id = events
        .iter()
        .find(|event| event.kind == RunEventKind::RunStarted)
        .map(|event| event.run.run_id)
        .expect("RunStarted carries the active run ID");
    let final_lifecycle = events.iter().rev().find(|event| {
        matches!(
            event.kind,
            RunEventKind::RunStarted
                | RunEventKind::RunCompleted
                | RunEventKind::RunErrored { .. }
                | RunEventKind::RunCancelled
        )
    });
    assert_eq!(
        final_lifecycle.map(|event| &event.kind),
        Some(&RunEventKind::RunCancelled)
    );
    assert!(
        events
            .iter()
            .all(|event| event.kind != RunEventKind::RunCompleted)
    );
    run_id
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
    plan.provenance.graph_path = GraphResourcePath::new(target).unwrap();
    let provenance = plan.provenance.clone();
    let parameters: BTreeMap<FunctionParameterId, ValueRef> = parameters
        .iter()
        .enumerate()
        .map(|(index, value)| {
            (
                FunctionParameterId::new(format!("parameter-{index}")),
                ValueRef::new(*value),
            )
        })
        .collect();
    let results: BTreeMap<FunctionParameterId, ValueRef> = results
        .iter()
        .enumerate()
        .map(|(index, value)| {
            (
                FunctionParameterId::new(format!("result-{index}")),
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

fn requirement(name: &str) -> CompiledResourceRequirement {
    CompiledResourceRequirement {
        resource: id(name, ResourceId::new),
        kind: ResourceKind::TemporaryStorage,
        access: ResourceAccess::Exclusive,
        optional: false,
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
    let output = stable_output("retry_result");
    planned.outputs[0].public_output = Some(output.clone());
    let mut execution_plan = plan(
        vec![planned],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output,
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    execution_plan
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
