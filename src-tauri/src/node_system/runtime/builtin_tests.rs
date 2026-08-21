mod control;
mod data_series;
mod relational;
mod resources;
mod scalar;
mod statistics;

use super::*;
use crate::node_system::ProjectSessionId;
use crate::node_system::analysis::{
    CompilationBasis, CompileId, CompileProvenance, SYSTEM_TRACE_CLOCK, SpanGuard, SpanKind,
    SpanOutcome, SpanSpec, TraceSink, TraceSpan,
};
use crate::node_system::compiler::{GraphCompiler, ResourceSnapshot};
use crate::node_system::document::{
    ConnectionId, DocumentConnection, DocumentNode, GraphDocument, GraphResourcePath,
    GraphRevision, NodeId, NodePosition, ParameterValues, PortAddress,
};
use crate::node_system::plan::*;
use crate::node_system::protocol::{
    CachePolicy, CanonicalDecimal, InputConsumption, NodeTypeId, OutputProduction, PortKey,
    TypeExpr, TypeId, Value,
};
use crate::node_system::registry::RegistryFingerprint;
use crate::project::StatisticalMissingValuePolicy;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

struct NoFunctions;

impl FunctionPlanProvider for NoFunctions {
    fn get_function(
        &self,
        _: &FunctionPlanHandle,
    ) -> Result<Option<std::sync::Arc<PublishedFunctionPlan>>, Box<str>> {
        Ok(None)
    }
}

struct NoResources;

#[derive(Default)]
struct RecordingPlotSink {
    publications: Mutex<Vec<(PlotKind, String)>>,
}

impl PlotSink for RecordingPlotSink {
    fn publish(&self, kind: PlotKind, payload: &str) -> Result<Box<str>, PlotPublishError> {
        self.publications
            .lock()
            .unwrap()
            .push((kind, payload.to_owned()));
        Ok("presentation:test".into())
    }
}

struct PlotResources {
    sink: Arc<RecordingPlotSink>,
}

impl ResourceProvider for PlotResources {
    fn acquire(
        &self,
        requirement: &CompiledResourceRequirement,
    ) -> Result<Box<dyn ResourceLease>, ResourceError> {
        assert_eq!(requirement.resource.as_str(), "yssbi.runtime.plot_sink");
        Ok(Box::new(PlotSinkResource::new(self.sink.clone())))
    }
}

impl ResourceProvider for NoResources {
    fn acquire(
        &self,
        _: &CompiledResourceRequirement,
    ) -> Result<Box<dyn ResourceLease>, ResourceError> {
        unreachable!("built-in scalar plans do not acquire resources")
    }
}

fn handle<T>(value: &str, constructor: impl FnOnce(Box<str>) -> Result<T, InvalidPlanId>) -> T {
    constructor(value.into()).unwrap()
}

fn decimal(value: &str) -> Value {
    Value::Decimal(CanonicalDecimal::new(value).unwrap())
}

fn compile_builtin_flow(
    nodes: &[(u128, &str)],
    connections: &[(u128, u128, &str, u128, &str)],
) -> crate::node_system::compiler::CompileResult {
    struct EmptyResources;
    impl ResourceSnapshot for EmptyResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            BTreeMap::new()
        }
    }

    let builtin = crate::node_system::catalog::build_builtin_node_system().unwrap();
    let mut graph = GraphDocument {
        revision: GraphRevision::INITIAL,
        nodes: BTreeMap::new(),
        port_bindings: BTreeMap::new(),
        connections: BTreeMap::new(),
        input_states: BTreeMap::new(),
    };
    for (id, node_type) in nodes {
        let id = NodeId::from_uuid(uuid::Uuid::from_u128(*id));
        let node_type = NodeTypeId::new(*node_type).unwrap();
        let parameters = builtin
            .registry
            .get(&node_type)
            .unwrap()
            .protocol()
            .parameters
            .parameters
            .iter()
            .filter_map(|parameter| {
                let value = match &parameter.default_value.as_ref()?.value {
                    Value::Null => serde_json::Value::Null,
                    Value::Bool(value) => serde_json::json!(value),
                    Value::Integer(value) => serde_json::json!(value),
                    Value::Unsigned(value) => serde_json::json!(value),
                    Value::Decimal(value) => serde_json::from_str(value.as_str()).unwrap(),
                    Value::String(value) => serde_json::json!(value),
                    Value::Bytes(_) | Value::List(_) | Value::Object(_) => return None,
                };
                Some((parameter.key.clone(), value))
            })
            .collect();
        graph.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type,
                position: NodePosition {
                    x: *id.as_uuid().as_bytes().last().unwrap() as f64,
                    y: 0.0,
                },
                parameters,
                user_label: None,
            },
        );
    }
    for (node, port, value_type, value) in [
        (0x1301, "mean", "core.float64", decimal("0")),
        (0x1301, "standard_deviation", "core.float64", decimal("1")),
        (0x1301, "sample_count", "core.int64", Value::Integer(8)),
    ] {
        if nodes.iter().any(|(id, _)| *id == node) {
            graph.input_states.insert(
                PortAddress::declared(
                    NodeId::from_uuid(uuid::Uuid::from_u128(node)),
                    PortKey::new(port).unwrap(),
                ),
                crate::node_system::document::InputState {
                    literal_override: Some(
                        serde_json::to_value(crate::node_system::protocol::TypedValue {
                            value_type: TypeExpr::Concrete(TypeId::new(value_type).unwrap()),
                            value,
                        })
                        .unwrap(),
                    ),
                },
            );
        }
    }
    for (id, source, source_port, target, target_port) in connections {
        let id = ConnectionId::from_uuid(uuid::Uuid::from_u128(*id));
        let target_node = NodeId::from_uuid(uuid::Uuid::from_u128(*target));
        let input = if *target_port == "predictors" {
            let address = PortAddress::instance(
                target_node,
                PortKey::new(*target_port).unwrap(),
                crate::node_system::document::PortInstanceId::from_uuid(id.as_uuid()),
            );
            graph.port_bindings.insert(
                address.clone(),
                crate::node_system::document::DynamicPortBinding::UserCreated {
                    order: crate::node_system::document::OrderKey("00000".into()),
                },
            );
            address
        } else {
            PortAddress::declared(target_node, PortKey::new(*target_port).unwrap())
        };
        graph.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(
                    NodeId::from_uuid(uuid::Uuid::from_u128(*source)),
                    PortKey::new(*source_port).unwrap(),
                ),
                input,
                order: None,
            },
        );
    }
    GraphCompiler::new(builtin.registry.as_ref(), &EmptyResources).compile(&graph)
}

fn data_series(
    element_type: DataSeriesElementType,
    values: impl Into<Box<[Value]>>,
) -> RuntimeValue {
    RuntimeValue::Artifact(
        DataSeriesBuilder::new(element_type)
            .values(values)
            .build(ArtifactKind::Collected)
            .unwrap(),
    )
}

fn named_data_series(
    element_type: DataSeriesElementType,
    name: &str,
    values: impl Into<Box<[Value]>>,
) -> RuntimeValue {
    RuntimeValue::Artifact(
        DataSeriesBuilder::new(element_type)
            .values(values)
            .name(name)
            .build(ArtifactKind::Collected)
            .unwrap(),
    )
}

fn data_series_values(value: &RuntimeValue) -> Vec<Value> {
    require_data_series(value)
        .unwrap()
        .cursor()
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn execute_variable_kernel(
    kernel: &str,
    variable: crate::variable::VariableInstance,
    inputs: &[RuntimeValue],
) -> (
    Result<Vec<RuntimeValue>, KernelError>,
    ProjectResourceSnapshot,
) {
    let resource = ResourceId::new(format!("variables/{}", variable.id)).unwrap();
    let snapshot = ProjectResourceSnapshot::new(
        ProjectSessionId::new("variable-series-test"),
        BTreeMap::from([(
            crate::node_system::analysis::ResourceKey::new(resource.as_str()),
            crate::node_system::analysis::ResourceVersion::new("1"),
        )]),
    )
    .with_variable(resource.clone(), Arc::new(variable));
    let provider = ProjectResourceProvider::new(snapshot.clone());
    let requirement = CompiledResourceRequirement {
        resource: resource.clone(),
        kind: ResourceKind::ExternalArtifact,
        access: if kernel == "yssbi.project.variable.set" {
            ResourceAccess::Exclusive
        } else {
            ResourceAccess::Shared
        },
        optional: false,
    };
    let resources = RunResourceSet::acquire(&[requirement], &provider).unwrap();
    let cancellation = CancellationToken::new();
    let resource_owner = RunResourceOwner::new(
        RunId::new(1),
        RunResourceBudgets::default(),
        cancellation.clone(),
    )
    .unwrap();
    let params = handle("variable-series", CompiledParameterHandle::new);
    let mut compiled = CompiledParameterStore::new();
    compiled
        .insert(params.clone(), BuiltinVariableParameters::new(resource))
        .unwrap();
    let source_graph_path = GraphResourcePath("events/builtin-test.yssbi-event".into());
    let context = KernelContext {
        run_id: RunId::new(1),
        frame_id: FrameId::next().unwrap(),
        activation_id: ActivationId::next().unwrap(),
        source_graph_path: &source_graph_path,
        source_node_id: NodeId::from_uuid(uuid::Uuid::nil()),
        run_output: &NOOP_RUN_OUTPUT_SINK,
        computation_settings: EffectiveComputationSettings::default(),
        params: &params,
        compiled_parameters: Some(&compiled),
        resources: &resources,
        resource_owner: &resource_owner,
        cancellation: &cancellation,
        deadline: None,
    };
    let result = build_builtin_kernel_registry()
        .get(&handle(kernel, KernelHandle::new))
        .unwrap()
        .execute(&context, inputs);
    (result, snapshot)
}

fn assert_decimal_list_approx_eq(actual: &RuntimeValue, expected: &[f64]) {
    let actual = data_series_values(actual);
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        let Value::Decimal(actual) = actual else {
            panic!("expected decimal list member, got {actual:?}");
        };
        let actual = actual.as_str().parse::<f64>().unwrap();
        let tolerance = 1e-12 * expected.abs().max(1.0);
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected} ± {tolerance}, got {actual}"
        );
    }
}

fn operation(kernel: &str, params: &str, inputs: &[u32], output: u32) -> PlannedOperation {
    let source_node_id = NodeId::from_uuid(uuid::Uuid::new_v4());
    PlannedOperation {
        stable_id: OperationStableId::new(format!("test.operation.{source_node_id}")).unwrap(),
        source_node_id,
        source_node_type_id: NodeTypeId::new("yssbi.test.builtin").unwrap(),
        kernel: PlannedKernel::Native(handle(kernel, KernelHandle::new)),
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
        outputs: Box::new([PlannedOutput {
            value: ValueRef::new(output),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            production: OutputProduction::FullyMaterialized,
            public_output: None,
            presentation: crate::node_system::plan::ResultPresentation::Inspector,
        }]),
        params: handle(params, CompiledParameterHandle::new),
        resource_dependencies: Box::new([]),
        cache_policy: CachePolicy::Disabled,
        semantics_version: ExecutionSemanticsVersion::from_bytes([1; 32]),
        workload: WorkloadClass::Cpu,
        retry: PlannedRetry::default(),
    }
}

fn effect_operation(kernel: &str, params: &str, inputs: &[u32]) -> PlannedOperation {
    let mut operation = operation(kernel, params, inputs, 0);
    operation.outputs = Box::new([]);
    operation
}

fn execute_kernel_direct(
    kernel: &str,
    params: &CompiledParameterHandle,
    compiled_parameters: Option<&CompiledParameterStore>,
    inputs: &[RuntimeValue],
) -> Result<Vec<RuntimeValue>, KernelError> {
    execute_kernel_direct_with_deadline(kernel, params, compiled_parameters, inputs, None)
}

fn execute_plot_kernel(
    kernel: &str,
    inputs: &[RuntimeValue],
) -> (
    Result<Vec<RuntimeValue>, KernelError>,
    Arc<RecordingPlotSink>,
) {
    let registry = build_builtin_kernel_registry();
    let sink = Arc::new(RecordingPlotSink::default());
    let provider = PlotResources { sink: sink.clone() };
    let requirement = CompiledResourceRequirement {
        resource: ResourceId::new("yssbi.runtime.plot_sink").unwrap(),
        kind: ResourceKind::ExternalArtifact,
        access: ResourceAccess::Shared,
        optional: false,
    };
    let resources = RunResourceSet::acquire(&[requirement], &provider).unwrap();
    let cancellation = CancellationToken::new();
    let resource_owner = RunResourceOwner::new(
        RunId::new(1),
        RunResourceBudgets::default(),
        cancellation.clone(),
    )
    .unwrap();
    let params = handle("plot.test", CompiledParameterHandle::new);
    let source_graph_path = GraphResourcePath("events/plot-test.yssbi-event".into());
    let context = KernelContext {
        run_id: RunId::new(1),
        frame_id: FrameId::next().unwrap(),
        activation_id: ActivationId::next().unwrap(),
        source_graph_path: &source_graph_path,
        source_node_id: NodeId::from_uuid(uuid::Uuid::nil()),
        run_output: &NOOP_RUN_OUTPUT_SINK,
        computation_settings: EffectiveComputationSettings::default(),
        params: &params,
        compiled_parameters: None,
        resources: &resources,
        resource_owner: &resource_owner,
        cancellation: &cancellation,
        deadline: None,
    };
    let result = registry
        .get(&handle(kernel, KernelHandle::new))
        .expect("production plot kernel handle")
        .execute(&context, inputs);
    (result, sink)
}

fn execute_kernel_direct_with_deadline(
    kernel: &str,
    params: &CompiledParameterHandle,
    compiled_parameters: Option<&CompiledParameterStore>,
    inputs: &[RuntimeValue],
    deadline: Option<RunDeadline>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let registry = build_builtin_kernel_registry();
    let resources = RunResourceSet::acquire(&[], &NoResources).unwrap();
    let cancellation = CancellationToken::new();
    let resource_owner = RunResourceOwner::new(
        RunId::new(1),
        RunResourceBudgets::default(),
        cancellation.clone(),
    )
    .unwrap();
    let source_graph_path = GraphResourcePath("events/kernel-test.yssbi-event".into());
    let context = KernelContext {
        run_id: RunId::new(1),
        frame_id: FrameId::next().unwrap(),
        activation_id: ActivationId::next().unwrap(),
        source_graph_path: &source_graph_path,
        source_node_id: NodeId::from_uuid(uuid::Uuid::nil()),
        run_output: &NOOP_RUN_OUTPUT_SINK,
        computation_settings: EffectiveComputationSettings::default(),
        params,
        compiled_parameters,
        resources: &resources,
        resource_owner: &resource_owner,
        cancellation: &cancellation,
        deadline,
    };
    registry
        .get(&handle(kernel, KernelHandle::new))
        .expect("production kernel handle")
        .execute(&context, inputs)
}

fn plan(operations: Vec<PlannedOperation>, value_count: u32, results: &[u32]) -> ExecutionPlan {
    let results = results
        .iter()
        .map(|value| PlanResult {
            name: format!("value_{value}").into(),
            output: GraphOutputRef {
                graph_path: GraphResourcePath("events/builtin-kernel-test".into()),
                port: PortAddress::declared(
                    NodeId::from_uuid(uuid::Uuid::nil()),
                    PortKey::new(format!("value_{value}")).unwrap(),
                ),
            },
            value: ValueRef::new(*value),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let publications = results
        .iter()
        .map(|result| PlannedPublication::GraphResult {
            name: result.name.clone(),
            output: result.output.clone(),
            value: result.value,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    ExecutionPlan {
        provenance: CompileProvenance {
            project_session_id: ProjectSessionId::new("builtin-kernel-test"),
            graph_path: GraphResourcePath("events/builtin-kernel-test".into()),
            basis: CompilationBasis {
                graph_revision: GraphRevision::new(1),
                registry_fingerprint: RegistryFingerprint::from_bytes([7; 32]),
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
        value_dependencies: Box::new([]),
        root_region: StructuredControlRegion::Sequence(
            (0..operations.len())
                .map(|index| ControlStep::Operation(OperationIndex::new(index as u32)))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        operations: operations.into_boxed_slice(),
        effect_dependencies: Box::new([]),
        relational_subplans: Box::new([]),
        resources: Box::new([]),
        results,
        publications,
    }
}

fn execute(
    plan: &ExecutionPlan,
    parameters: &CompiledParameterStore,
) -> Result<RunResult, RunError> {
    let kernels = build_builtin_kernel_registry();
    RunExecutor::new(
        &kernels,
        &NoResources,
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_compiled_parameters(parameters)
    .run(plan, CancellationToken::new())
}

fn insert_constant(parameters: &mut CompiledParameterStore, name: &str, value: Value) {
    parameters
        .insert(
            handle(name, CompiledParameterHandle::new),
            BuiltinConstantParameters::new(value),
        )
        .unwrap();
}

fn execute_dataframe_kernel(
    kernel: &str,
    inputs: &[RuntimeValue],
) -> Result<Vec<RuntimeValue>, KernelError> {
    let params = handle("dataframe-test", CompiledParameterHandle::new);
    let mut compiled = CompiledParameterStore::new();
    compiled
        .insert(params.clone(), DataframeKernelParameters::default())
        .unwrap();
    execute_kernel_direct(kernel, &params, Some(&compiled), inputs)
}

fn statistics_parameters(
    policy: StatisticalMissingValuePolicy,
) -> (CompiledParameterHandle, CompiledParameterStore) {
    let handle = handle("statistics.test", CompiledParameterHandle::new);
    let mut parameters = CompiledParameterStore::new();
    parameters
        .insert(
            handle.clone(),
            StatisticsKernelParameters {
                missing_value_policy: policy,
                ..StatisticsKernelParameters::default()
            },
        )
        .unwrap();
    (handle, parameters)
}

fn execute_ols_fit(
    response: RuntimeValue,
    predictors: Vec<RuntimeValue>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    execute_ols_with_policy(
        response,
        predictors,
        StatisticalMissingValuePolicy::Listwise,
    )
}

fn execute_ols_with_policy(
    response: RuntimeValue,
    predictors: Vec<RuntimeValue>,
    policy: StatisticalMissingValuePolicy,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let (handle, parameters) = statistics_parameters(policy);
    let mut inputs = vec![response];
    inputs.extend(predictors);
    execute_kernel_direct(
        "yssbi.statistics.ols.fit",
        &handle,
        Some(&parameters),
        &inputs,
    )
}

fn int_series(values: impl IntoIterator<Item = i64>) -> RuntimeValue {
    data_series(
        DataSeriesElementType::Int64,
        values.into_iter().map(Value::Integer).collect::<Vec<_>>(),
    )
}

fn float_series(values: impl IntoIterator<Item = f64>) -> RuntimeValue {
    data_series(
        DataSeriesElementType::Float64,
        values
            .into_iter()
            .map(|value| {
                if value.is_nan() {
                    Value::String("NaN".into())
                } else if value == f64::INFINITY {
                    Value::String("Infinity".into())
                } else if value == f64::NEG_INFINITY {
                    Value::String("-Infinity".into())
                } else {
                    decimal(&value.to_string())
                }
            })
            .collect::<Vec<_>>(),
    )
}

fn response_with_null() -> RuntimeValue {
    data_series(
        DataSeriesElementType::Int64,
        vec![
            Value::Integer(1),
            Value::Null,
            Value::Integer(3),
            Value::Integer(4),
        ],
    )
}

fn predictor_with_nan() -> RuntimeValue {
    data_series(
        DataSeriesElementType::Float64,
        vec![
            decimal("1"),
            decimal("2"),
            Value::String("NaN".into()),
            decimal("4"),
        ],
    )
}

fn series_element_type(value: &RuntimeValue) -> DataSeriesElementType {
    require_data_series(value)
        .unwrap()
        .data_series_metadata()
        .unwrap()
        .element_type
}

fn scalar_object(value: &RuntimeValue) -> &BTreeMap<Box<str>, Value> {
    let RuntimeValue::Scalar(Value::Object(value)) = value else {
        panic!("expected scalar object, got {value:?}");
    };
    value
}
