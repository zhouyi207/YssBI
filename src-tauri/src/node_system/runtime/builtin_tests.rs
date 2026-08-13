use super::*;
use crate::node_system::analysis::{
    CompilationBasis, CompileId, CompileProvenance, ProjectSessionId, SYSTEM_TRACE_CLOCK,
    SpanGuard, SpanKind, SpanOutcome, SpanSpec, TraceSink, TraceSpan,
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
            .build(ArtifactKind::Replayable)
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
            .build(ArtifactKind::Replayable)
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
    let context = KernelContext {
        run_id: RunId::new(1),
        frame_id: FrameId::next(),
        activation_id: ActivationId::next().unwrap(),
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
    let context = KernelContext {
        run_id: RunId::new(1),
        frame_id: FrameId::next(),
        activation_id: ActivationId::next().unwrap(),
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
    let context = KernelContext {
        run_id: RunId::new(1),
        frame_id: FrameId::next(),
        activation_id: ActivationId::next().unwrap(),
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

#[test]
fn variable_data_series_materializes_artifact_without_serialized_runtime_internals() {
    let id = crate::variable::VariableId::new();
    let variable = crate::variable::VariableInstance {
        id,
        name: "observations".into(),
        data_type: crate::graph::value::DataType::DataSeries(Box::new(
            crate::graph::value::DataType::Int64,
        )),
        data_value: crate::graph::value::DataValue::DataSeries(
            crate::graph::value::DataSeriesValue::new(crate::tabular::variable_handle(&id)),
        ),
        tabular: Some(
            crate::tabular::TabularSnapshot::from_json(r#"{"observations":[1,null,3]}"#).unwrap(),
        ),
        description: String::new(),
        scope: crate::variable::VariableScope::Global,
        tags: Vec::new(),
    };

    let (result, _) = execute_variable_kernel("yssbi.project.variable.get", variable.clone(), &[]);
    let output = result.unwrap();
    let artifact = require_data_series(&output[0]).unwrap();

    assert_eq!(
        artifact.data_series_metadata(),
        Some(&DataSeriesMetadata {
            element_type: DataSeriesElementType::Int64,
            length: 3,
            null_count: 1,
            name: Some("observations".into()),
            format: None,
        })
    );
    assert_eq!(
        serde_json::to_value(variable).unwrap()["dataValue"],
        serde_json::json!({"DataSeries": crate::tabular::variable_handle(&id)})
    );
}

#[test]
fn variable_data_series_set_serializes_payload_without_artifact_internals() {
    let id = crate::variable::VariableId::new();
    let variable = crate::variable::VariableInstance {
        id,
        name: "observations".into(),
        data_type: crate::graph::value::DataType::DataSeries(Box::new(
            crate::graph::value::DataType::Float64,
        )),
        data_value: crate::graph::value::DataValue::Null,
        tabular: None,
        description: String::new(),
        scope: crate::variable::VariableScope::Global,
        tags: Vec::new(),
    };
    let input = RuntimeValue::Artifact(
        DataSeriesBuilder::new(DataSeriesElementType::Float64)
            .values([decimal("1.5"), Value::Null, decimal("3.5")])
            .name("fitted")
            .format("number")
            .build(ArtifactKind::Replayable)
            .unwrap(),
    );

    let (result, snapshot) =
        execute_variable_kernel("yssbi.project.variable.set", variable, &[input]);
    result.unwrap();
    let effects = snapshot.variable_effects();

    assert_eq!(effects.len(), 1);
    let crate::graph::value::DataValue::DataSeries(value) = &effects[0].after else {
        panic!("DataSeries assignment must persist a serializable DataSeries value");
    };
    assert_eq!(
        value.element_type,
        Some(crate::graph::value::DataType::Float64)
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&value.id).unwrap(),
        serde_json::json!({"fitted": [1.5, null, 3.5]})
    );
    assert!(!value.id.contains("artifact"));
    assert!(!value.id.contains("Replayable"));
}

#[test]
fn data_series_variable_get_set_flows_into_statistics() {
    let id = crate::variable::VariableId::new();
    let empty = crate::variable::VariableInstance {
        id,
        name: "observations".into(),
        data_type: crate::graph::value::DataType::DataSeries(Box::new(
            crate::graph::value::DataType::Float64,
        )),
        data_value: crate::graph::value::DataValue::Null,
        tabular: None,
        description: String::new(),
        scope: crate::variable::VariableScope::Global,
        tags: Vec::new(),
    };
    let assigned = RuntimeValue::Artifact(
        DataSeriesBuilder::new(DataSeriesElementType::Float64)
            .values([decimal("1"), decimal("2"), decimal("4")])
            .name("observations")
            .format("number")
            .build(ArtifactKind::Replayable)
            .unwrap(),
    );
    let (set, snapshot) =
        execute_variable_kernel("yssbi.project.variable.set", empty.clone(), &[assigned]);
    set.unwrap();
    let after = snapshot.variable_effects().remove(0).after;
    let crate::graph::value::DataValue::DataSeries(series) = &after else {
        panic!("variable effect must persist a DataSeries payload");
    };
    let series_json = series.id.clone();
    let persisted = crate::variable::VariableInstance {
        data_value: after,
        tabular: Some(crate::tabular::TabularSnapshot::from_json(&series_json).unwrap()),
        ..empty
    };
    assert!(
        !serde_json::to_string(&persisted)
            .unwrap()
            .contains("Replayable")
    );

    let (get, _) = execute_variable_kernel("yssbi.project.variable.get", persisted, &[]);
    let values = get.unwrap();
    let metadata = require_data_series(&values[0])
        .unwrap()
        .data_series_metadata()
        .unwrap();
    assert_eq!(metadata.element_type, DataSeriesElementType::Float64);
    assert_eq!(metadata.length, 3);
    assert_eq!(metadata.name.as_deref(), Some("observations"));
    let fit = execute_ols_fit(int_series([1, 2, 3]), vec![values[0].clone()]).unwrap();
    assert_eq!(series_element_type(&fit[1]), DataSeriesElementType::Float64);
    assert_eq!(
        require_data_series(&fit[1])
            .unwrap()
            .data_series_metadata()
            .unwrap()
            .length,
        3
    );
}

#[test]
fn scatter_consumes_two_numeric_data_series_artifacts() {
    let x = named_data_series(
        DataSeriesElementType::Int64,
        "observed x",
        [Value::Integer(1), Value::Integer(2), Value::Integer(3)],
    );
    let y = named_data_series(
        DataSeriesElementType::Float64,
        "observed y",
        [decimal("1.5"), decimal("2.5"), decimal("3.5")],
    );

    let (result, sink) = execute_plot_kernel("yssbi.plot.scatter.view", &[x, y]);

    assert_eq!(
        result.unwrap(),
        vec![RuntimeValue::Scalar(Value::String(
            "presentation:test".into()
        ))]
    );
    let publications = sink.publications.lock().unwrap();
    assert_eq!(publications.len(), 1);
    assert_eq!(publications[0].0, PlotKind::Scatter);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&publications[0].1).unwrap()["data"],
        serde_json::json!([
            { "x": 1.0, "y": 1.5 },
            { "x": 2.0, "y": 2.5 },
            { "x": 3.0, "y": 3.5 }
        ])
    );
}

#[test]
fn plot_rejects_scalar_list_series_input() {
    let input = RuntimeValue::Scalar(Value::List(vec![Value::Integer(1)]));

    let (result, sink) = execute_plot_kernel("yssbi.plot.ecdf.view", &[input]);

    assert_eq!(
        result.unwrap_err().message(),
        "expected DataSeries Artifact, received scalar"
    );
    assert!(sink.publications.lock().unwrap().is_empty());
}

#[test]
fn plot_preserves_data_series_name_and_format_metadata() {
    let x = RuntimeValue::Artifact(
        DataSeriesBuilder::new(DataSeriesElementType::Int64)
            .name("calendar period")
            .format("0000")
            .values([Value::Integer(2024), Value::Integer(2025)])
            .build(ArtifactKind::Replayable)
            .unwrap(),
    );
    let y = RuntimeValue::Artifact(
        DataSeriesBuilder::new(DataSeriesElementType::Float64)
            .name("revenue")
            .format("$0.00")
            .values([decimal("10.5"), decimal("11.75")])
            .build(ArtifactKind::Replayable)
            .unwrap(),
    );

    let (result, sink) = execute_plot_kernel("yssbi.plot.line.view", &[x, y]);

    result.unwrap();
    let publications = sink.publications.lock().unwrap();
    let payload = serde_json::from_str::<serde_json::Value>(&publications[0].1).unwrap();
    assert_eq!(payload["xLabel"], "calendar period");
    assert_eq!(payload["yLabel"], "revenue");
    assert_eq!(payload["xFormat"], "0000");
    assert_eq!(payload["yFormat"], "$0.00");
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
    RunExecutor::new(&kernels, &NoResources, &NoFunctions)
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

#[test]
fn dataframe_filter_consumes_boolean_series_artifact() {
    let dataframe = RuntimeValue::Scalar(Value::Object(BTreeMap::from([
        (
            "id".into(),
            Value::List(vec![
                Value::Integer(10),
                Value::Integer(20),
                Value::Integer(30),
            ]),
        ),
        (
            "label".into(),
            Value::List(vec![
                Value::String("first".into()),
                Value::String("second".into()),
                Value::String("third".into()),
            ]),
        ),
    ])));
    let mask = data_series(
        DataSeriesElementType::Boolean,
        vec![Value::Bool(true), Value::Null, Value::Bool(false)],
    );

    let outputs = execute_dataframe_kernel("yssbi.dataframe.filter", &[dataframe, mask]).unwrap();

    assert_eq!(
        outputs,
        vec![RuntimeValue::Scalar(Value::Object(BTreeMap::from([
            ("id".into(), Value::List(vec![Value::Integer(10)])),
            (
                "label".into(),
                Value::List(vec![Value::String("first".into())]),
            ),
        ])))]
    );
}

#[test]
fn dataframe_length_returns_int64_scalar() {
    let input = data_series(
        DataSeriesElementType::Int64,
        vec![Value::Integer(1), Value::Null, Value::Integer(3)],
    );

    let outputs = execute_dataframe_kernel("yssbi.dataframe.series.length", &[input]).unwrap();

    assert_eq!(outputs, vec![RuntimeValue::Scalar(Value::Integer(3))]);
}

#[test]
fn dataframe_count_returns_non_null_int64_scalar() {
    let input = data_series(
        DataSeriesElementType::Int64,
        vec![Value::Integer(1), Value::Null, Value::Integer(3)],
    );

    let outputs = execute_dataframe_kernel("yssbi.dataframe.series.count", &[input]).unwrap();

    assert_eq!(outputs, vec![RuntimeValue::Scalar(Value::Integer(2))]);
}

#[test]
fn dataframe_checked_integer_sum_and_mean_returns_float64() {
    let sum = execute_dataframe_kernel(
        "yssbi.dataframe.series.sum",
        &[data_series(
            DataSeriesElementType::Int64,
            vec![Value::Integer(1), Value::Null, Value::Integer(3)],
        )],
    )
    .unwrap();
    assert_eq!(sum, vec![RuntimeValue::Scalar(Value::Integer(4))]);

    let overflow = execute_dataframe_kernel(
        "yssbi.dataframe.series.sum",
        &[data_series(
            DataSeriesElementType::Int64,
            vec![Value::Integer(i64::MAX), Value::Integer(1)],
        )],
    )
    .unwrap_err();
    assert_eq!(overflow.to_string(), "Int64 DataSeries sum overflow");

    let mean = execute_dataframe_kernel(
        "yssbi.dataframe.series.mean",
        &[data_series(
            DataSeriesElementType::Int64,
            vec![Value::Integer(1), Value::Null, Value::Integer(3)],
        )],
    )
    .unwrap();
    assert_eq!(mean, vec![RuntimeValue::Scalar(decimal("2"))]);
}

#[test]
fn dataframe_lag_preserves_exact_element_type_and_name() {
    let params = handle("dataframe-lag-test", CompiledParameterHandle::new);
    let mut compiled = CompiledParameterStore::new();
    compiled
        .insert(
            params.clone(),
            DataframeKernelParameters {
                order: Some(1),
                ..DataframeKernelParameters::default()
            },
        )
        .unwrap();
    let input = named_data_series(
        DataSeriesElementType::String,
        "category",
        vec![Value::String("a".into()), Value::String("b".into())],
    );

    let outputs = execute_kernel_direct(
        "yssbi.dataframe.timeseries.lag",
        &params,
        Some(&compiled),
        &[input],
    )
    .unwrap();
    let metadata = require_data_series(&outputs[0])
        .unwrap()
        .data_series_metadata()
        .unwrap();

    assert_eq!(metadata.element_type, DataSeriesElementType::String);
    assert_eq!(metadata.name.as_deref(), Some("category"));
    assert_eq!(
        data_series_values(&outputs[0]),
        vec![Value::Null, Value::String("a".into())]
    );
}

#[test]
fn dataframe_decompose_outputs_column_names_and_exact_types() {
    let dataframe = RuntimeValue::Scalar(Value::Object(BTreeMap::from([
        (
            "id".into(),
            Value::List(vec![Value::Integer(1), Value::Integer(2)]),
        ),
        (
            "label".into(),
            Value::List(vec![Value::String("a".into()), Value::Null]),
        ),
    ])));

    let outputs = execute_dataframe_kernel("yssbi.dataframe.decompose", &[dataframe]).unwrap();
    let metadata = outputs
        .iter()
        .map(|output| {
            require_data_series(output)
                .unwrap()
                .data_series_metadata()
                .unwrap()
        })
        .collect::<Vec<_>>();

    assert_eq!(metadata[0].name.as_deref(), Some("id"));
    assert_eq!(metadata[0].element_type, DataSeriesElementType::Int64);
    assert_eq!(metadata[1].name.as_deref(), Some("label"));
    assert_eq!(metadata[1].element_type, DataSeriesElementType::String);
    assert_eq!(metadata[1].null_count, 1);
}

#[test]
fn dataframe_decompose_emits_only_compiled_columns_in_output_order() {
    let params = handle("decompose-columns", CompiledParameterHandle::new);
    let mut compiled = CompiledParameterStore::new();
    compiled
        .insert(
            params.clone(),
            DataframeKernelParameters {
                columns: Some(vec!["third".into(), "first".into()].into_boxed_slice()),
                ..DataframeKernelParameters::default()
            },
        )
        .unwrap();
    let dataframe = RuntimeValue::Scalar(Value::Object(BTreeMap::from([
        ("first".into(), Value::List(vec![Value::Integer(1)])),
        ("second".into(), Value::List(vec![Value::Integer(2)])),
        ("third".into(), Value::List(vec![Value::Integer(3)])),
    ])));

    let outputs = execute_kernel_direct(
        "yssbi.dataframe.decompose",
        &params,
        Some(&compiled),
        &[dataframe],
    )
    .unwrap();
    let names = outputs
        .iter()
        .map(|output| {
            require_data_series(output)
                .unwrap()
                .data_series_metadata()
                .unwrap()
                .name
                .as_deref()
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();

    assert_eq!(names, ["third", "first"]);
}

#[test]
fn dataframe_decompose_accepts_collected_single_dataframe() {
    let dataframe = Value::Object(BTreeMap::from([(
        "value".into(),
        Value::List(vec![Value::Integer(1), Value::Integer(2)]),
    )]));
    let collected = RuntimeValue::Artifact(Artifact::new(ArtifactKind::Collected, [dataframe]));

    let outputs = execute_dataframe_kernel("yssbi.dataframe.decompose", &[collected]).unwrap();

    assert_eq!(outputs.len(), 1);
    assert_eq!(
        require_data_series(&outputs[0])
            .unwrap()
            .data_series_metadata()
            .unwrap()
            .name
            .as_deref(),
        Some("value")
    );
}

#[test]
fn dataframe_selected_series_flows_into_math_and_statistics() {
    let dataframe = RuntimeValue::Scalar(Value::Object(BTreeMap::from([
        (
            "count".into(),
            Value::List(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3),
            ]),
        ),
        (
            "amount".into(),
            Value::List(vec![decimal("1.5"), decimal("2.5"), decimal("4.5")]),
        ),
    ])));
    let decomposed = execute_dataframe_kernel("yssbi.dataframe.decompose", &[dataframe]).unwrap();
    let integers = decomposed
        .iter()
        .find(|value| series_element_type(value) == DataSeriesElementType::Int64)
        .unwrap()
        .clone();
    let floats = decomposed
        .iter()
        .find(|value| series_element_type(value) == DataSeriesElementType::Float64)
        .unwrap()
        .clone();

    validate_data_series_type_expr(
        require_data_series(&integers)
            .unwrap()
            .data_series_metadata()
            .unwrap(),
        &crate::node_system::protocol::data_series_type(TypeExpr::Concrete(
            TypeId::new("core.int64").unwrap(),
        )),
    )
    .unwrap();
    let converted = execute_kernel_direct(
        "yssbi.data_series.convert.int64_to_float64",
        &handle("decompose.convert", CompiledParameterHandle::new),
        None,
        &[integers],
    )
    .unwrap();
    let converted_metadata = require_data_series(&converted[0])
        .unwrap()
        .data_series_metadata()
        .unwrap();
    assert_eq!(
        converted_metadata.element_type,
        DataSeriesElementType::Float64
    );
    assert_eq!(converted_metadata.name.as_deref(), Some("count"));
    let fit = execute_ols_fit(converted[0].clone(), vec![floats.clone()]).unwrap();
    assert_eq!(series_element_type(&fit[1]), DataSeriesElementType::Float64);

    let math = execute_kernel_direct(
        "yssbi.numeric.series.add",
        &handle("decompose.math", CompiledParameterHandle::new),
        None,
        &[floats.clone(), Value::Integer(1).into()],
    )
    .unwrap();
    let math_metadata = require_data_series(&math[0])
        .unwrap()
        .data_series_metadata()
        .unwrap();
    assert_eq!(math_metadata.element_type, DataSeriesElementType::Float64);
    assert_eq!(math_metadata.name.as_deref(), Some("amount"));
    let (plot, sink) = execute_plot_kernel("yssbi.plot.line.view", &[floats, math[0].clone()]);
    plot.unwrap();
    assert_eq!(sink.publications.lock().unwrap().len(), 1);
}

#[test]
fn dataframe_combine_consumes_artifacts_and_rejects_scalar_lists() {
    let outputs = execute_dataframe_kernel(
        "yssbi.dataframe.combine",
        &[
            named_data_series(
                DataSeriesElementType::Int64,
                "id",
                vec![Value::Integer(1), Value::Integer(2)],
            ),
            named_data_series(
                DataSeriesElementType::String,
                "label",
                vec![Value::String("a".into()), Value::Null],
            ),
        ],
    )
    .unwrap();
    assert_eq!(
        outputs,
        vec![RuntimeValue::Scalar(Value::Object(BTreeMap::from([
            (
                "id".into(),
                Value::List(vec![Value::Integer(1), Value::Integer(2)]),
            ),
            (
                "label".into(),
                Value::List(vec![Value::String("a".into()), Value::Null]),
            ),
        ])))]
    );

    let error = execute_dataframe_kernel(
        "yssbi.dataframe.combine",
        &[RuntimeValue::Scalar(Value::List(vec![Value::Integer(1)]))],
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "expected DataSeries Artifact, received scalar"
    );
}

#[test]
fn dataframe_standardize_returns_float_artifact_and_propagates_nulls() {
    let input = data_series(
        DataSeriesElementType::Int64,
        vec![Value::Integer(1), Value::Null, Value::Integer(3)],
    );

    let outputs = execute_dataframe_kernel("yssbi.dataframe.series.standardize", &[input]).unwrap();
    let artifact = require_data_series(&outputs[0]).unwrap();
    let metadata = artifact.data_series_metadata().unwrap();

    assert_eq!(metadata.element_type, DataSeriesElementType::Float64);
    assert_eq!(metadata.length, 3);
    assert_eq!(metadata.null_count, 1);
    assert!(matches!(data_series_values(&outputs[0])[1], Value::Null));
}

#[test]
fn dataframe_numeric_and_string_comparison_kernels_propagate_nulls() {
    let numeric = execute_dataframe_kernel(
        "yssbi.dataframe.series.compare.equal",
        &[
            data_series(
                DataSeriesElementType::Float64,
                vec![decimal("1"), Value::Null, decimal("3")],
            ),
            RuntimeValue::Scalar(decimal("1.0000000000001")),
        ],
    )
    .unwrap();
    assert_eq!(
        data_series_values(&numeric[0]),
        vec![Value::Bool(true), Value::Null, Value::Bool(false)]
    );

    let string = execute_dataframe_kernel(
        "yssbi.dataframe.series.compare.string.equal",
        &[
            data_series(
                DataSeriesElementType::String,
                vec![Value::String("A".into()), Value::Null],
            ),
            RuntimeValue::Scalar(Value::String("a".into())),
        ],
    )
    .unwrap();
    assert_eq!(
        data_series_values(&string[0]),
        vec![Value::Bool(false), Value::Null]
    );
}

#[test]
fn dataframe_kernel_rejects_scalar_list_series_input() {
    let error = execute_dataframe_kernel(
        "yssbi.dataframe.series.length",
        &[RuntimeValue::Scalar(Value::List(vec![Value::Integer(1)]))],
    )
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "expected DataSeries Artifact, received scalar"
    );
}

#[test]
fn dataframe_integer_range_executes_through_production_registry() {
    let mut parameters = CompiledParameterStore::new();
    insert_constant(&mut parameters, "start", Value::Integer(1));
    insert_constant(&mut parameters, "end", Value::Integer(5));
    insert_constant(&mut parameters, "step", Value::Integer(2));
    parameters
        .insert(
            handle("range", CompiledParameterHandle::new),
            DataframeKernelParameters::default(),
        )
        .unwrap();
    let execution_plan = plan(
        vec![
            operation("yssbi.constant.int64", "start", &[], 0),
            operation("yssbi.constant.int64", "end", &[], 1),
            operation("yssbi.constant.int64", "step", &[], 2),
            operation("yssbi.dataframe.series.int_range", "range", &[0, 1, 2], 3),
        ],
        4,
        &[3],
    );

    let result = execute(&execution_plan, &parameters).unwrap();
    assert_eq!(
        data_series_values(&result.values["value_3"]),
        vec![Value::Integer(1), Value::Integer(3)]
    );
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

#[test]
fn statistics_summary_uses_compiled_data_series_input_indices() {
    let handle = handle("statistics.summary.indices", CompiledParameterHandle::new);
    let mut parameters = CompiledParameterStore::new();
    parameters
        .insert(
            handle.clone(),
            StatisticsKernelParameters {
                data_series_input_indices: Some(vec![1, 2].into_boxed_slice()),
                ..StatisticsKernelParameters::default()
            },
        )
        .unwrap();

    let outputs = execute_kernel_direct(
        "yssbi.statistics.ols.summary",
        &handle,
        Some(&parameters),
        &[
            RuntimeValue::Scalar(Value::Null),
            float_series([1.0, 2.0, 3.0]),
            float_series([1.0, 2.0, 4.0]),
        ],
    )
    .unwrap();

    assert_eq!(outputs.len(), 2);
}

#[test]
fn statistics_fit_consumes_artifacts_and_returns_float_series_artifacts() {
    let result =
        execute_ols_fit(int_series([1, 2, 3]), vec![float_series([1.0, 2.0, 4.0])]).unwrap();

    assert_eq!(
        series_element_type(&result[1]),
        DataSeriesElementType::Float64
    );
    assert_eq!(
        series_element_type(&result[2]),
        DataSeriesElementType::Float64
    );
}

#[test]
fn statistics_listwise_reports_removed_null_and_nan_rows() {
    let result = execute_ols_with_policy(
        response_with_null(),
        vec![predictor_with_nan()],
        StatisticalMissingValuePolicy::Listwise,
    )
    .unwrap();
    let model = scalar_object(&result[0]);
    let Value::Object(metadata) = &model["metadata"] else {
        panic!("missing observation metadata");
    };

    assert_eq!(metadata["originalObservationCount"], Value::Integer(4));
    assert_eq!(metadata["usedObservationCount"], Value::Integer(2));
    assert_eq!(metadata["droppedNullCount"], Value::Integer(1));
    assert_eq!(metadata["droppedNanCount"], Value::Integer(1));
}

#[test]
fn statistics_reject_reports_port_and_row() {
    let error = execute_ols_with_policy(
        int_series([1, 2, 3]),
        vec![data_series(
            DataSeriesElementType::Float64,
            vec![decimal("1"), Value::Null, decimal("3")],
        )],
        StatisticalMissingValuePolicy::Reject,
    )
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "statistics input 'predictors[0]' contains Null at row 1"
    );
}

#[test]
fn statistics_rejects_infinity_with_port_and_row() {
    let error = execute_ols_fit(
        int_series([1, 2, 3]),
        vec![float_series([1.0, f64::INFINITY, 3.0])],
    )
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "statistics input 'predictors[0]' contains positive infinity at row 1"
    );
}

#[test]
fn statistics_fit_rejects_scalar_list_graph_series_inputs() {
    let error = execute_ols_fit(
        RuntimeValue::Scalar(Value::List(vec![
            Value::Integer(1),
            Value::Integer(2),
            Value::Integer(3),
        ])),
        vec![float_series([1.0, 2.0, 3.0])],
    )
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "expected DataSeries Artifact, received scalar"
    );
}

#[test]
fn statistics_prediction_rejects_scalar_list_graph_series_inputs() {
    let (handle, parameters) = statistics_parameters(StatisticalMissingValuePolicy::Listwise);
    let model = RuntimeValue::Scalar(Value::Object(BTreeMap::from([
        ("family".into(), Value::String("ols".into())),
        (
            "coefficients".into(),
            Value::List(vec![decimal("0"), decimal("1")]),
        ),
    ])));
    let predictor = RuntimeValue::Scalar(Value::List(vec![decimal("1"), decimal("2")]));

    let error = execute_kernel_direct(
        "yssbi.statistics.linear.predict",
        &handle,
        Some(&parameters),
        &[model, predictor],
    )
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "expected DataSeries Artifact, received scalar"
    );
}

#[test]
fn statistics_prediction_flows_into_plot() {
    let fit = execute_ols_fit(int_series([1, 2, 3]), vec![float_series([1.0, 2.0, 3.0])]).unwrap();
    let (handle, parameters) = statistics_parameters(StatisticalMissingValuePolicy::Listwise);
    let prediction = execute_kernel_direct(
        "yssbi.statistics.linear.predict",
        &handle,
        Some(&parameters),
        &[fit[0].clone(), float_series([4.0, 5.0, 6.0])],
    )
    .unwrap();
    let metadata = require_data_series(&prediction[0])
        .unwrap()
        .data_series_metadata()
        .unwrap();
    assert_eq!(metadata.element_type, DataSeriesElementType::Float64);
    assert_eq!(metadata.length, 3);
    assert_eq!(metadata.name.as_deref(), Some("prediction"));
    validate_data_series_type_expr(
        metadata,
        &crate::node_system::protocol::numeric_data_series_type(),
    )
    .unwrap();

    let (plot, sink) = execute_plot_kernel(
        "yssbi.plot.scatter.view",
        &[float_series([4.0, 5.0, 6.0]), prediction[0].clone()],
    );
    plot.unwrap();
    assert_eq!(sink.publications.lock().unwrap().len(), 1);
}

#[test]
fn statistics_fit_executes_instead_of_returning_an_adapter_error() {
    let mut parameters = CompiledParameterStore::new();
    insert_constant(&mut parameters, "start", Value::Integer(1));
    insert_constant(&mut parameters, "end", Value::Integer(5));
    insert_constant(&mut parameters, "step", Value::Integer(1));
    for name in ["x", "y"] {
        parameters
            .insert(
                handle(name, CompiledParameterHandle::new),
                DataframeKernelParameters::default(),
            )
            .unwrap();
    }
    parameters
        .insert(
            handle("fit", CompiledParameterHandle::new),
            StatisticsKernelParameters::default(),
        )
        .unwrap();
    let mut fit = operation("yssbi.statistics.ols.fit", "fit", &[3, 4], 5);
    fit.outputs = Box::new([
        PlannedOutput {
            value: ValueRef::new(5),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            production: OutputProduction::FullyMaterialized,
        },
        PlannedOutput {
            value: ValueRef::new(6),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            production: OutputProduction::FullyMaterialized,
        },
        PlannedOutput {
            value: ValueRef::new(7),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            production: OutputProduction::FullyMaterialized,
        },
    ]);
    let execution_plan = plan(
        vec![
            operation("yssbi.constant.int64", "start", &[], 0),
            operation("yssbi.constant.int64", "end", &[], 1),
            operation("yssbi.constant.int64", "step", &[], 2),
            operation("yssbi.dataframe.series.int_range", "x", &[0, 1, 2], 3),
            operation("yssbi.dataframe.series.int_range", "y", &[0, 1, 2], 4),
            fit,
        ],
        8,
        &[5, 6, 7],
    );

    let result = execute(&execution_plan, &parameters).unwrap();
    assert!(matches!(
        result.values["value_5"],
        RuntimeValue::Scalar(Value::Object(_))
    ));
    assert_decimal_list_approx_eq(&result.values["value_6"], &[1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn logit_fit_uses_the_real_binary_response_implementation() {
    let mut parameters = CompiledParameterStore::new();
    insert_constant(&mut parameters, "start", Value::Integer(0));
    insert_constant(&mut parameters, "end", Value::Integer(4));
    insert_constant(&mut parameters, "step", Value::Integer(1));
    for name in ["x", "invalid-y"] {
        parameters
            .insert(
                handle(name, CompiledParameterHandle::new),
                DataframeKernelParameters::default(),
            )
            .unwrap();
    }
    parameters
        .insert(
            handle("fit", CompiledParameterHandle::new),
            StatisticsKernelParameters::default(),
        )
        .unwrap();
    let mut fit = operation("yssbi.statistics.logit.fit", "fit", &[4, 3], 5);
    fit.outputs = Box::new([
        PlannedOutput {
            value: ValueRef::new(5),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            production: OutputProduction::FullyMaterialized,
        },
        PlannedOutput {
            value: ValueRef::new(6),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            production: OutputProduction::FullyMaterialized,
        },
        PlannedOutput {
            value: ValueRef::new(7),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            production: OutputProduction::FullyMaterialized,
        },
    ]);
    let execution_plan = plan(
        vec![
            operation("yssbi.constant.int64", "start", &[], 0),
            operation("yssbi.constant.int64", "end", &[], 1),
            operation("yssbi.constant.int64", "step", &[], 2),
            operation("yssbi.dataframe.series.int_range", "x", &[0, 1, 2], 3),
            operation(
                "yssbi.dataframe.series.int_range",
                "invalid-y",
                &[0, 1, 2],
                4,
            ),
            fit,
        ],
        8,
        &[5],
    );

    let error = execute(&execution_plan, &parameters).unwrap_err();
    assert!(
        error.to_string().contains("endog must be 0/1"),
        "unexpected error: {error}"
    );
}

#[test]
fn builtin_registry_contains_every_catalog_kernel_handle() {
    let registry = build_builtin_kernel_registry();
    let expected = [
        "yssbi.constant.bool",
        "yssbi.constant.string",
        "yssbi.constant.int64",
        "yssbi.constant.float64",
        "yssbi.numeric.add.int64",
        "yssbi.numeric.subtract.int64",
        "yssbi.numeric.multiply.int64",
        "yssbi.numeric.divide.int64",
        "yssbi.numeric.add.float64",
        "yssbi.numeric.subtract.float64",
        "yssbi.numeric.multiply.float64",
        "yssbi.numeric.divide.float64",
        "yssbi.compare.equal",
        "yssbi.compare.not_equal",
        "yssbi.compare.less",
        "yssbi.compare.less_equal",
        "yssbi.compare.greater",
        "yssbi.compare.greater_equal",
        "yssbi.logic.and",
        "yssbi.logic.or",
        "yssbi.logic.not",
    ];

    for kernel in expected {
        assert!(
            registry.get(&handle(kernel, KernelHandle::new)).is_some(),
            "{kernel}"
        );
    }
}

#[test]
fn constant_kernels_resolve_compiled_parameters_by_plan_handle() {
    let mut parameters = CompiledParameterStore::new();
    insert_constant(&mut parameters, "constant.bool", Value::Bool(true));
    insert_constant(
        &mut parameters,
        "constant.string",
        Value::String("hello".into()),
    );
    insert_constant(&mut parameters, "constant.int64", Value::Integer(42));
    insert_constant(&mut parameters, "constant.float64", decimal("1.25"));
    let execution_plan = plan(
        vec![
            operation("yssbi.constant.bool", "constant.bool", &[], 0),
            operation("yssbi.constant.string", "constant.string", &[], 1),
            operation("yssbi.constant.int64", "constant.int64", &[], 2),
            operation("yssbi.constant.float64", "constant.float64", &[], 3),
        ],
        4,
        &[0, 1, 2, 3],
    );

    let result = execute(&execution_plan, &parameters).unwrap();

    assert_eq!(result.values["value_0"], Value::Bool(true).into());
    assert_eq!(
        result.values["value_1"],
        Value::String("hello".into()).into()
    );
    assert_eq!(result.values["value_2"], Value::Integer(42).into());
    assert_eq!(result.values["value_3"], decimal("1.25").into());
}

#[test]
fn numeric_kernels_execute_int64_and_float64_operations() {
    let mut parameters = CompiledParameterStore::new();
    for (name, value) in [
        ("int.left", Value::Integer(12)),
        ("int.right", Value::Integer(3)),
        ("float.left", decimal("7.5")),
        ("float.right", decimal("2.5")),
    ] {
        insert_constant(&mut parameters, name, value);
    }
    let execution_plan = plan(
        vec![
            operation("yssbi.constant.int64", "int.left", &[], 0),
            operation("yssbi.constant.int64", "int.right", &[], 1),
            operation("yssbi.constant.float64", "float.left", &[], 2),
            operation("yssbi.constant.float64", "float.right", &[], 3),
            operation("yssbi.numeric.add.int64", "unused.0", &[0, 1], 4),
            operation("yssbi.numeric.subtract.int64", "unused.1", &[0, 1], 5),
            operation("yssbi.numeric.multiply.int64", "unused.2", &[0, 1], 6),
            operation("yssbi.numeric.divide.int64", "unused.3", &[0, 1], 7),
            operation("yssbi.numeric.add.float64", "unused.4", &[2, 3], 8),
            operation("yssbi.numeric.subtract.float64", "unused.5", &[2, 3], 9),
            operation("yssbi.numeric.multiply.float64", "unused.6", &[2, 3], 10),
            operation("yssbi.numeric.divide.float64", "unused.7", &[2, 3], 11),
        ],
        12,
        &[4, 5, 6, 7, 8, 9, 10, 11],
    );

    let result = execute(&execution_plan, &parameters).unwrap();

    for (value, expected) in [(4, 15), (5, 9), (6, 36), (7, 4)] {
        let key = format!("value_{value}");
        assert_eq!(result.values[key.as_str()], Value::Integer(expected).into());
    }
    for (value, expected) in [(8, "10"), (9, "5"), (10, "18.75"), (11, "3")] {
        let key = format!("value_{value}");
        assert_eq!(result.values[key.as_str()], decimal(expected).into());
    }
}

#[test]
fn compare_and_logic_kernels_execute_through_the_run_scheduler() {
    let mut parameters = CompiledParameterStore::new();
    for (name, value) in [
        ("float.two", decimal("2")),
        ("float.three", decimal("3")),
        ("bool.true", Value::Bool(true)),
        ("bool.false", Value::Bool(false)),
    ] {
        insert_constant(&mut parameters, name, value);
    }
    let execution_plan = plan(
        vec![
            operation("yssbi.constant.float64", "float.two", &[], 0),
            operation("yssbi.constant.float64", "float.three", &[], 1),
            operation("yssbi.constant.bool", "bool.true", &[], 2),
            operation("yssbi.constant.bool", "bool.false", &[], 3),
            operation("yssbi.compare.equal", "unused.0", &[0, 1], 4),
            operation("yssbi.compare.not_equal", "unused.1", &[0, 1], 5),
            operation("yssbi.compare.less", "unused.2", &[0, 1], 6),
            operation("yssbi.compare.less_equal", "unused.3", &[0, 1], 7),
            operation("yssbi.compare.greater", "unused.4", &[0, 1], 8),
            operation("yssbi.compare.greater_equal", "unused.5", &[0, 1], 9),
            operation("yssbi.logic.and", "unused.6", &[2, 3], 10),
            operation("yssbi.logic.or", "unused.7", &[2, 3], 11),
            operation("yssbi.logic.not", "unused.8", &[3], 12),
        ],
        13,
        &[4, 5, 6, 7, 8, 9, 10, 11, 12],
    );

    let result = execute(&execution_plan, &parameters).unwrap();
    let expected = [false, true, true, true, false, false, false, true, true];
    for (value, expected) in (4_u32..=12).zip(expected) {
        let key = format!("value_{value}");
        assert_eq!(result.values[key.as_str()], Value::Bool(expected).into());
    }
}

#[test]
fn equal_kernel_covers_bool_int_string_and_float() {
    for (label, left, right, expected) in [
        ("bool", Value::Bool(true), Value::Bool(true), true),
        ("int", Value::Integer(7), Value::Integer(7), true),
        (
            "string",
            Value::String("same".into()),
            Value::String("different".into()),
            false,
        ),
        ("float", decimal("1.25"), decimal("1.25"), true),
    ] {
        let mut parameters = CompiledParameterStore::new();
        insert_constant(&mut parameters, &format!("{label}.left"), left);
        insert_constant(&mut parameters, &format!("{label}.right"), right);
        let execution_plan = plan(
            vec![
                operation("yssbi.constant.bool", &format!("{label}.left"), &[], 0),
                operation("yssbi.constant.bool", &format!("{label}.right"), &[], 1),
                operation("yssbi.compare.equal", "unused.equal", &[0, 1], 2),
            ],
            3,
            &[2],
        );
        let constant_kind = match label {
            "bool" => "yssbi.constant.bool",
            "int" => "yssbi.constant.int64",
            "string" => "yssbi.constant.string",
            "float" => "yssbi.constant.float64",
            _ => unreachable!(),
        };
        let mut operations = execution_plan.operations.into_vec();
        operations[0].kernel = PlannedKernel::Native(handle(constant_kind, KernelHandle::new));
        operations[1].kernel = PlannedKernel::Native(handle(constant_kind, KernelHandle::new));
        let execution_plan = plan(operations, 3, &[2]);
        let result = execute(&execution_plan, &parameters).unwrap();
        assert_eq!(
            result.values["value_2"],
            Value::Bool(expected).into(),
            "{label}"
        );
    }
}

#[test]
fn scalar_convert_kernel_covers_supported_targets_and_errors() {
    for (label, input, target, expected) in [
        (
            "bool",
            Value::String("yes".into()),
            ConvertTarget::Bool,
            Value::Bool(true),
        ),
        ("int", decimal("7"), ConvertTarget::Int64, Value::Integer(7)),
        (
            "float",
            Value::Integer(5),
            ConvertTarget::Float64,
            decimal("5"),
        ),
        (
            "string",
            Value::Bool(true),
            ConvertTarget::String,
            Value::String("true".into()),
        ),
    ] {
        let mut parameters = CompiledParameterStore::new();
        let params = handle(&format!("convert.{label}"), CompiledParameterHandle::new);
        parameters
            .insert(params.clone(), ConvertParameters { target })
            .unwrap();
        let output = execute_kernel_direct(
            "yssbi.value.convert",
            &params,
            Some(&parameters),
            &[input.into()],
        )
        .unwrap();
        assert_eq!(output, vec![expected.into()], "{label}");
    }

    let mut parameters = CompiledParameterStore::new();
    let params = handle("convert.error", CompiledParameterHandle::new);
    parameters
        .insert(
            params.clone(),
            ConvertParameters {
                target: ConvertTarget::Int64,
            },
        )
        .unwrap();
    let error = execute_kernel_direct(
        "yssbi.value.convert",
        &params,
        Some(&parameters),
        &[Value::String("not-an-int".into()).into()],
    )
    .unwrap_err();
    assert_eq!(error.message(), "cannot parse 'not-an-int' as Int64");

    for (target, input, expected) in [
        (
            ConvertTarget::Bool,
            Value::String("maybe".into()),
            "cannot parse 'maybe' as Boolean",
        ),
        (
            ConvertTarget::Float64,
            Value::String("not-a-float".into()),
            "cannot parse 'not-a-float' as Float64",
        ),
        (
            ConvertTarget::String,
            Value::List(vec![]),
            "cannot convert List to String",
        ),
    ] {
        let mut parameters = CompiledParameterStore::new();
        let params = handle("convert.target-error", CompiledParameterHandle::new);
        parameters
            .insert(params.clone(), ConvertParameters { target })
            .unwrap();
        let error = execute_kernel_direct(
            "yssbi.value.convert",
            &params,
            Some(&parameters),
            &[input.into()],
        )
        .unwrap_err();
        assert_eq!(error.message(), expected);
    }

    let error = execute_kernel_direct(
        "yssbi.value.convert",
        &params,
        Some(&parameters),
        &[RuntimeValue::Artifact(Artifact::new(
            ArtifactKind::Replayable,
            vec![Value::Integer(1)],
        ))],
    )
    .unwrap_err();
    assert_eq!(error.message(), "value conversion expects a scalar input");
}

#[test]
fn scalar_float_to_int_accepts_exact_f64_boundary_and_rejects_two_to_the_63rd() {
    let mut parameters = CompiledParameterStore::new();
    let params = handle("convert.int64.boundary", CompiledParameterHandle::new);
    parameters
        .insert(
            params.clone(),
            ConvertParameters {
                target: ConvertTarget::Int64,
            },
        )
        .unwrap();

    let accepted = execute_kernel_direct(
        "yssbi.value.convert",
        &params,
        Some(&parameters),
        &[decimal("9223372036854774784").into()],
    )
    .unwrap();
    assert_eq!(
        accepted,
        vec![Value::Integer(9_223_372_036_854_774_784).into()]
    );

    let error = execute_kernel_direct(
        "yssbi.value.convert",
        &params,
        Some(&parameters),
        &[decimal("9223372036854775808").into()],
    )
    .unwrap_err();
    assert_eq!(error.message(), "Float64 value is outside the Int64 range");
}

#[test]
fn distribution_series_flows_into_statistics_without_scalar_encoding() {
    let compiled = compile_builtin_flow(
        &[
            (0x1300, "yssbi.project.event.begin"),
            (0x1301, "yssbi.distribution.normal.sample"),
            (0x1302, "yssbi.statistics.adf.test"),
        ],
        &[
            (0x1310, 0x1300, "then", 0x1302, "enter"),
            (0x1311, 0x1301, "samples", 0x1302, "series"),
        ],
    );
    assert!(
        compiled.semantic.is_some() && compiled.analysis.diagnostics.is_empty(),
        "cross-family type compilation failed: outcome={:?}, diagnostics={:?}",
        compiled.outcome,
        compiled.analysis.diagnostics
    );

    let distribution = execute_kernel_direct(
        "yssbi.distribution.normal.sample",
        &handle("distribution.cross-family", CompiledParameterHandle::new),
        None,
        &[
            decimal("0").into(),
            decimal("1").into(),
            Value::Integer(8).into(),
        ],
    )
    .unwrap();
    let samples = distribution[0].clone();
    let sample_metadata = require_data_series(&samples)
        .unwrap()
        .data_series_metadata()
        .unwrap();
    assert_eq!(sample_metadata.element_type, DataSeriesElementType::Float64);
    assert_eq!(sample_metadata.length, 8);
    assert_eq!(sample_metadata.name.as_deref(), Some("normal"));

    let fit = execute_ols_fit(samples.clone(), vec![samples]).unwrap();
    for (value, name) in [(&fit[1], "fitted"), (&fit[2], "residuals")] {
        let metadata = require_data_series(value)
            .unwrap()
            .data_series_metadata()
            .unwrap();
        assert_eq!(metadata.element_type, DataSeriesElementType::Float64);
        assert_eq!(metadata.length, 8);
        assert_eq!(metadata.name.as_deref(), Some(name));
    }
}

#[test]
fn continuous_distribution_returns_float_data_series_artifact() {
    let params = handle("distribution.normal", CompiledParameterHandle::new);
    let output = execute_kernel_direct(
        "yssbi.distribution.normal.sample",
        &params,
        None,
        &[
            decimal("0").into(),
            decimal("1").into(),
            Value::Integer(4).into(),
        ],
    )
    .unwrap();

    let artifact = require_data_series(&output[0]).unwrap();
    let metadata = artifact.data_series_metadata().unwrap();
    assert_eq!(metadata.element_type, DataSeriesElementType::Float64);
    assert_eq!(metadata.length, 4);
    assert_eq!(metadata.name.as_deref(), Some("normal"));
    assert_eq!(metadata.format.as_deref(), Some("number"));
}

#[test]
fn discrete_distribution_returns_int_data_series_artifact() {
    let params = handle("distribution.bernoulli", CompiledParameterHandle::new);
    let output = execute_kernel_direct(
        "yssbi.distribution.bernoulli.sample",
        &params,
        None,
        &[decimal("0.5").into(), Value::Integer(4).into()],
    )
    .unwrap();

    let metadata = require_data_series(&output[0])
        .unwrap()
        .data_series_metadata()
        .unwrap();
    assert_eq!(metadata.element_type, DataSeriesElementType::Int64);
    assert_eq!(metadata.length, 4);
}

#[test]
fn distribution_integer_parameters_are_strict_and_sample_count_is_positive() {
    let params = handle("distribution.strict-integers", CompiledParameterHandle::new);
    for inputs in [
        vec![
            decimal("0").into(),
            decimal("1").into(),
            decimal("4").into(),
        ],
        vec![
            decimal("0").into(),
            decimal("1").into(),
            Value::Integer(0).into(),
        ],
    ] {
        assert!(
            execute_kernel_direct("yssbi.distribution.normal.sample", &params, None, &inputs,)
                .is_err()
        );
    }
}

#[test]
fn series_conversion_rejects_scalar_list_and_preserves_nulls() {
    let params = handle("series.convert.contract", CompiledParameterHandle::new);
    let source = data_series(
        DataSeriesElementType::Int64,
        [Value::Integer(1), Value::Null, Value::Integer(3)],
    );
    let converted = execute_kernel_direct(
        "yssbi.data_series.convert.int64_to_float64",
        &params,
        None,
        &[source],
    )
    .unwrap();
    let metadata = require_data_series(&converted[0])
        .unwrap()
        .data_series_metadata()
        .unwrap();
    assert_eq!(metadata.element_type, DataSeriesElementType::Float64);
    assert_eq!(metadata.null_count, 1);
    assert_eq!(
        data_series_values(&converted[0]),
        vec![decimal("1"), Value::Null, decimal("3")]
    );

    assert!(
        execute_kernel_direct(
            "yssbi.data_series.convert.int64_to_float64",
            &params,
            None,
            &[Value::List(vec![]).into()],
        )
        .is_err()
    );
}

#[test]
fn series_conversion_rejects_sequence_artifact_payload() {
    let params = handle("series.convert.payload", CompiledParameterHandle::new);
    let sequence = RuntimeValue::Artifact(Artifact::new(
        ArtifactKind::Replayable,
        vec![Value::Integer(1)],
    ));
    let error = execute_kernel_direct(
        "yssbi.data_series.convert.int64_to_float64",
        &params,
        None,
        &[sequence],
    )
    .unwrap_err();

    assert_eq!(
        error.message(),
        "expected DataSeries Artifact, received sequence Artifact"
    );
}

#[test]
fn series_float_to_int_rejects_fractional_and_out_of_range_values() {
    let params = handle("series.convert.lossless", CompiledParameterHandle::new);
    for value in ["2.5", "9223372036854775808"] {
        let error = execute_kernel_direct(
            "yssbi.data_series.convert.float64_to_int64",
            &params,
            None,
            &[data_series(
                DataSeriesElementType::Float64,
                [decimal(value)],
            )],
        )
        .unwrap_err();
        assert!(error.message().contains("DataSeries element 0"));
    }
}

#[test]
fn series_math_preserves_nulls_and_applies_numeric_promotion() {
    let params = handle("series.math.promotion", CompiledParameterHandle::new);
    let integers = data_series(
        DataSeriesElementType::Int64,
        [Value::Integer(2), Value::Null, Value::Integer(6)],
    );
    let added = execute_kernel_direct(
        "yssbi.numeric.series.add",
        &params,
        None,
        &[integers.clone(), Value::Integer(3).into()],
    )
    .unwrap();
    assert_eq!(
        require_data_series(&added[0])
            .unwrap()
            .data_series_metadata()
            .unwrap()
            .element_type,
        DataSeriesElementType::Int64
    );
    assert_eq!(
        data_series_values(&added[0]),
        vec![Value::Integer(5), Value::Null, Value::Integer(9)]
    );

    let divided = execute_kernel_direct(
        "yssbi.numeric.series.divide",
        &params,
        None,
        &[integers, Value::Integer(2).into()],
    )
    .unwrap();
    assert_eq!(
        require_data_series(&divided[0])
            .unwrap()
            .data_series_metadata()
            .unwrap()
            .element_type,
        DataSeriesElementType::Float64
    );
    assert_eq!(
        data_series_values(&divided[0]),
        vec![decimal("1"), Value::Null, decimal("3")]
    );
}

#[test]
fn series_math_requires_at_least_one_series_operand() {
    let params = handle("series.math.series-required", CompiledParameterHandle::new);
    let error = execute_kernel_direct(
        "yssbi.numeric.series.add",
        &params,
        None,
        &[Value::Integer(1).into(), Value::Integer(2).into()],
    )
    .unwrap_err();

    assert!(error.message().contains("at least one DataSeries operand"));
}

#[test]
fn series_math_rejects_sequence_artifact_operand() {
    let params = handle("series.math.payload", CompiledParameterHandle::new);
    let sequence = RuntimeValue::Artifact(Artifact::new(
        ArtifactKind::Replayable,
        vec![Value::Integer(1)],
    ));
    let error = execute_kernel_direct(
        "yssbi.numeric.series.add",
        &params,
        None,
        &[sequence, Value::Integer(1).into()],
    )
    .unwrap_err();

    assert!(error.message().contains("sequence Artifact"));
}

#[test]
fn series_int_math_rejects_overflow() {
    let params = handle("series.math.overflow", CompiledParameterHandle::new);
    let error = execute_kernel_direct(
        "yssbi.numeric.series.add",
        &params,
        None,
        &[
            data_series(DataSeriesElementType::Int64, [Value::Integer(i64::MAX)]),
            Value::Integer(1).into(),
        ],
    )
    .unwrap_err();

    assert!(error.message().contains("overflow"));
}

#[test]
fn series_conversion_kernels_cover_every_legacy_conversion() {
    let cases = [
        (
            "yssbi.data_series.convert.string_to_categorical",
            DataSeriesElementType::String,
            Value::String("blue".into()),
            DataSeriesElementType::Categorical,
            Value::String("blue".into()),
        ),
        (
            "yssbi.data_series.convert.string_to_float64",
            DataSeriesElementType::String,
            Value::String("2.5".into()),
            DataSeriesElementType::Float64,
            decimal("2.5"),
        ),
        (
            "yssbi.data_series.convert.string_to_int64",
            DataSeriesElementType::String,
            Value::String("2".into()),
            DataSeriesElementType::Int64,
            Value::Integer(2),
        ),
        (
            "yssbi.data_series.convert.int64_to_string",
            DataSeriesElementType::Int64,
            Value::Integer(2),
            DataSeriesElementType::String,
            Value::String("2".into()),
        ),
        (
            "yssbi.data_series.convert.float64_to_string",
            DataSeriesElementType::Float64,
            decimal("2.5"),
            DataSeriesElementType::String,
            Value::String("2.5".into()),
        ),
        (
            "yssbi.data_series.convert.int64_to_float64",
            DataSeriesElementType::Int64,
            Value::Integer(2),
            DataSeriesElementType::Float64,
            decimal("2"),
        ),
        (
            "yssbi.data_series.convert.float64_to_int64",
            DataSeriesElementType::Float64,
            decimal("2"),
            DataSeriesElementType::Int64,
            Value::Integer(2),
        ),
        (
            "yssbi.data_series.convert.int64_to_bool",
            DataSeriesElementType::Int64,
            Value::Integer(1),
            DataSeriesElementType::Boolean,
            Value::Bool(true),
        ),
        (
            "yssbi.data_series.convert.float64_to_bool",
            DataSeriesElementType::Float64,
            decimal("0"),
            DataSeriesElementType::Boolean,
            Value::Bool(false),
        ),
        (
            "yssbi.data_series.convert.categorical_to_string",
            DataSeriesElementType::Categorical,
            Value::String("blue".into()),
            DataSeriesElementType::String,
            Value::String("blue".into()),
        ),
        (
            "yssbi.data_series.convert.int64_to_categorical",
            DataSeriesElementType::Int64,
            Value::Integer(2),
            DataSeriesElementType::Categorical,
            Value::String("2".into()),
        ),
        (
            "yssbi.data_series.convert.categorical_to_int64",
            DataSeriesElementType::Categorical,
            Value::String("2".into()),
            DataSeriesElementType::Int64,
            Value::Integer(2),
        ),
        (
            "yssbi.data_series.convert.float64_to_categorical",
            DataSeriesElementType::Float64,
            decimal("2.5"),
            DataSeriesElementType::Categorical,
            Value::String("2.5".into()),
        ),
        (
            "yssbi.data_series.convert.categorical_to_float64",
            DataSeriesElementType::Categorical,
            Value::String("2.5".into()),
            DataSeriesElementType::Float64,
            decimal("2.5"),
        ),
    ];
    let params = handle("series.convert", CompiledParameterHandle::new);
    for (kernel, source_type, input, target_type, expected) in cases {
        let output = execute_kernel_direct(
            kernel,
            &params,
            None,
            &[data_series(source_type, [input, Value::Null])],
        )
        .unwrap();
        let artifact = require_data_series(&output[0]).unwrap();
        assert_eq!(
            artifact.data_series_metadata().unwrap().element_type,
            target_type
        );
        assert_eq!(
            data_series_values(&output[0]),
            vec![expected, Value::Null],
            "{kernel}"
        );
    }

    let parse_error = execute_kernel_direct(
        "yssbi.data_series.convert.string_to_int64",
        &params,
        None,
        &[data_series(
            DataSeriesElementType::String,
            [Value::String("bad".into())],
        )],
    )
    .unwrap_err();
    assert_eq!(
        parse_error.message(),
        "DataSeries element 0: cannot parse 'bad' as Int64"
    );

    let source_type_error = execute_kernel_direct(
        "yssbi.data_series.convert.string_to_float64",
        &params,
        None,
        &[data_series(
            DataSeriesElementType::Int64,
            [Value::Integer(1)],
        )],
    )
    .unwrap_err();
    assert_eq!(
        source_type_error.message(),
        "expected String DataSeries, received Int64"
    );

    let materialization_error = execute_kernel_direct(
        "yssbi.data_series.convert.int64_to_string",
        &params,
        None,
        &[Value::Integer(1).into()],
    )
    .unwrap_err();
    assert_eq!(
        materialization_error.message(),
        "expected DataSeries Artifact, received scalar"
    );
}

#[test]
fn unary_math_kernels_execute_each_legacy_operation() {
    let params = handle("unary", CompiledParameterHandle::new);
    for (kernel, input, expected) in [
        ("yssbi.numeric.ln", "1", "0"),
        ("yssbi.numeric.log2", "8", "3"),
        ("yssbi.numeric.log10", "100", "2"),
        ("yssbi.numeric.exp", "0", "1"),
        ("yssbi.numeric.sqrt", "9", "3"),
        ("yssbi.numeric.square", "4", "16"),
    ] {
        let output =
            execute_kernel_direct(kernel, &params, None, &[decimal(input).into()]).unwrap();
        assert_eq!(output, vec![decimal(expected).into()], "{kernel}");
    }
}

#[test]
fn do_sleep_print_and_view_leaf_kernels_preserve_contracts() {
    let params = handle("effects", CompiledParameterHandle::new);
    assert!(
        execute_kernel_direct("yssbi.control.do", &params, None, &[])
            .unwrap()
            .is_empty()
    );
    assert!(
        execute_kernel_direct("yssbi.control.sleep", &params, None, &[decimal("0").into()])
            .unwrap()
            .is_empty()
    );
    let sleep_error = execute_kernel_direct(
        "yssbi.control.sleep",
        &params,
        None,
        &[decimal("-0.01").into()],
    )
    .unwrap_err();
    assert_eq!(
        sleep_error.message(),
        "Sleep duration must be between zero and sixty seconds"
    );
    let started = std::time::Instant::now();
    let deadline_error = execute_kernel_direct_with_deadline(
        "yssbi.control.sleep",
        &params,
        None,
        &[decimal("1").into()],
        Some(RunDeadline::after(std::time::Duration::from_millis(10))),
    )
    .unwrap_err();
    assert_eq!(deadline_error.kind(), KernelErrorKind::DeadlineExceeded);
    assert!(started.elapsed() < std::time::Duration::from_millis(200));
    crate::log::clear_test_logs();
    assert!(
        execute_kernel_direct(
            "yssbi.debug.print",
            &params,
            None,
            &[Value::String("fine".into()).into()],
        )
        .unwrap()
        .is_empty()
    );
    let print_log = crate::log::take_test_logs()
        .into_iter()
        .find(|log| log.message == "fine")
        .expect("Print emits an application-visible execution log");
    assert_eq!(print_log.level, crate::log::LogLevel::Info);
    assert_eq!(print_log.log_type, crate::log::LogType::Execution);
    assert!(
        print_log
            .source
            .as_deref()
            .is_some_and(|source| { source.starts_with("yssbi.debug.print activation=") })
    );
    let print_error = execute_kernel_direct(
        "yssbi.debug.print",
        &params,
        None,
        &[Value::Integer(1).into()],
    )
    .unwrap_err();
    assert_eq!(
        print_error.message(),
        "Print message must be a String scalar"
    );
    let viewed = execute_kernel_direct(
        "yssbi.debug.view",
        &params,
        None,
        &[Value::Integer(9).into()],
    )
    .unwrap();
    assert_eq!(
        viewed,
        vec![RuntimeValue::Artifact(Artifact::new(
            ArtifactKind::Replayable,
            vec![Value::Integer(9)],
        ))]
    );

    let mut parameters = CompiledParameterStore::new();
    for (name, message) in [("first", "First"), ("second", "Second"), ("third", "Third")] {
        insert_constant(&mut parameters, name, Value::String(message.into()));
    }
    let chain = plan(
        vec![
            operation("yssbi.constant.string", "first", &[], 0),
            operation("yssbi.constant.string", "second", &[], 1),
            operation("yssbi.constant.string", "third", &[], 2),
            effect_operation("yssbi.debug.print", "unused.print.1", &[0]),
            effect_operation("yssbi.control.do", "unused.do", &[]),
            effect_operation("yssbi.debug.print", "unused.print.2", &[1]),
            effect_operation("yssbi.debug.print", "unused.print.3", &[2]),
        ],
        3,
        &[],
    );
    execute(&chain, &parameters).unwrap();
}

#[test]
fn print_observer_and_trace_preserve_exact_first_second_third_order() {
    #[derive(Default)]
    struct Events(Mutex<Vec<RunEvent>>);
    impl RunEventSink for Events {
        fn record(&self, event: RunEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    #[derive(Default)]
    struct Trace(Mutex<Vec<TraceSpan>>);
    impl TraceSink for Trace {
        fn start_span(&self, spec: SpanSpec) -> SpanGuard<'_> {
            SpanGuard::new(self, spec, &SYSTEM_TRACE_CLOCK)
        }

        fn complete_span(&self, span: TraceSpan) {
            self.0.lock().unwrap().push(span);
        }
    }

    let mut parameters = CompiledParameterStore::new();
    for (name, message) in [("first", "First"), ("second", "Second"), ("third", "Third")] {
        insert_constant(&mut parameters, name, Value::String(message.into()));
    }
    let mut operations = vec![
        operation("yssbi.constant.string", "first", &[], 0),
        operation("yssbi.constant.string", "second", &[], 1),
        operation("yssbi.constant.string", "third", &[], 2),
        effect_operation("yssbi.debug.print", "unused.print.1", &[0]),
        effect_operation("yssbi.control.do", "unused.do", &[]),
        effect_operation("yssbi.debug.print", "unused.print.2", &[1]),
        effect_operation("yssbi.debug.print", "unused.print.3", &[2]),
    ];
    for (index, node) in [(3, 101_u128), (5, 102), (6, 103)] {
        operations[index].source_node_id = NodeId::from_uuid(uuid::Uuid::from_u128(node));
        operations[index].source_node_type_id = NodeTypeId::new("yssbi.debug.print").unwrap();
    }
    let mut execution_plan = plan(operations, 3, &[]);
    execution_plan.effect_dependencies = Box::new([
        EffectDependency {
            before: OperationIndex::new(3),
            after: OperationIndex::new(5),
        },
        EffectDependency {
            before: OperationIndex::new(5),
            after: OperationIndex::new(6),
        },
    ]);
    let events = Events::default();
    let trace = Trace::default();
    let kernels = build_builtin_kernel_registry();

    RunExecutor::new(&kernels, &NoResources, &NoFunctions)
        .with_compiled_parameters(&parameters)
        .with_event_sink(&events)
        .with_trace_sink(&trace)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    let label = |node_id: NodeId| match node_id.as_uuid().as_u128() {
        101 => Some("First"),
        102 => Some("Second"),
        103 => Some("Third"),
        _ => None,
    };
    let event_order = events
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|event| matches!(event.kind, RunEventKind::OperationCompleted { .. }))
        .filter_map(|event| event.correlation.node_id.and_then(label))
        .collect::<Vec<_>>();
    let trace_order = trace
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|span| {
            span.kind == SpanKind::OperationAttempt && span.outcome == SpanOutcome::Success
        })
        .filter_map(|event| event.correlation.node_id.and_then(label))
        .collect::<Vec<_>>();
    assert_eq!(event_order, ["First", "Second", "Third"]);
    assert_eq!(trace_order, ["First", "Second", "Third"]);
}

#[test]
fn real_graph_connection_overrides_print_protocol_default_at_runtime() {
    struct Resources;
    impl ResourceSnapshot for Resources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            BTreeMap::new()
        }
    }
    struct CapturePrint(Arc<Mutex<Vec<Value>>>);
    impl Kernel for CapturePrint {
        fn execute(
            &self,
            _: &KernelContext<'_>,
            inputs: &[RuntimeValue],
        ) -> Result<Vec<RuntimeValue>, KernelError> {
            let [RuntimeValue::Scalar(value)] = inputs else {
                return Err(KernelError::new("expected one scalar print input"));
            };
            self.0.lock().unwrap().push(value.clone());
            Ok(Vec::new())
        }
    }

    let system = crate::node_system::catalog::build_builtin_node_system().unwrap();
    let registry = Arc::unwrap_or_clone(system.registry);
    let constant_id = NodeId::from_uuid(uuid::Uuid::from_u128(201));
    let print_id = NodeId::from_uuid(uuid::Uuid::from_u128(202));
    let mut constant_parameters = ParameterValues::new();
    constant_parameters.insert(
        crate::node_system::protocol::ParameterKey::new("value").unwrap(),
        serde_json::json!("Connected message"),
    );
    let mut graph = GraphDocument::default();
    graph.nodes.insert(
        constant_id,
        DocumentNode {
            id: constant_id,
            node_type: NodeTypeId::new("yssbi.constant.string").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: constant_parameters,
            user_label: None,
        },
    );
    graph.nodes.insert(
        print_id,
        DocumentNode {
            id: print_id,
            node_type: NodeTypeId::new("yssbi.debug.print").unwrap(),
            position: NodePosition { x: 1.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    graph.connections.insert(
        ConnectionId::from_uuid(uuid::Uuid::from_u128(203)),
        DocumentConnection {
            id: ConnectionId::from_uuid(uuid::Uuid::from_u128(203)),
            output: PortAddress::declared(constant_id, PortKey::new("value").unwrap()),
            input: PortAddress::declared(print_id, PortKey::new("message").unwrap()),
            order: None,
        },
    );

    let compiled = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let mut execution_plan = compiled
        .plan
        .unwrap_or_else(|| panic!("print diagnostics: {:?}", compiled.analysis.diagnostics));
    let constant_index = execution_plan
        .operations
        .iter()
        .position(|operation| operation.source_node_id == constant_id)
        .unwrap();
    let print_index = execution_plan
        .operations
        .iter()
        .position(|operation| operation.source_node_id == print_id)
        .unwrap();
    assert_eq!(execution_plan.operations[print_index].inputs.len(), 1);
    assert_eq!(
        execution_plan.operations[print_index].inputs[0].bound_value,
        None
    );
    let print_input = execution_plan.operations[print_index].inputs[0].value;
    let mut reachable =
        BTreeSet::from([execution_plan.operations[constant_index].outputs[0].value]);
    loop {
        let previous_len = reachable.len();
        for dependency in &execution_plan.value_dependencies {
            if reachable.contains(&dependency.source) {
                reachable.insert(dependency.destination);
            }
        }
        for operation in &execution_plan.operations {
            if matches!(operation.kernel, PlannedKernel::Adapter(_))
                && operation
                    .inputs
                    .iter()
                    .any(|input| reachable.contains(&input.value))
            {
                reachable.extend(operation.outputs.iter().map(|output| output.value));
            }
        }
        if reachable.len() == previous_len {
            break;
        }
    }
    assert!(reachable.contains(&print_input));

    let capture_handle = handle("test.capture.print", KernelHandle::new);
    execution_plan.operations[print_index].kernel = PlannedKernel::Native(capture_handle.clone());
    let captured = Arc::new(Mutex::new(Vec::new()));
    let mut kernels = build_builtin_kernel_registry();
    kernels
        .register(capture_handle, CapturePrint(Arc::clone(&captured)))
        .unwrap();
    let mut parameters = CompiledParameterStore::new();
    parameters
        .insert(
            execution_plan.operations[constant_index].params.clone(),
            BuiltinConstantParameters::new(Value::String("Connected message".into())),
        )
        .unwrap();

    RunExecutor::new(&kernels, &NoResources, &NoFunctions)
        .with_compiled_parameters(&parameters)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(
        captured.lock().unwrap().as_slice(),
        [Value::String("Connected message".into())]
    );
}

#[test]
fn print_protocol_has_default_and_ordered_chain_contract() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::protocol::{EffectSemantics, ParameterKey, PortKey, Purity};

    let system = build_builtin_node_system().unwrap();
    let print = system
        .registry
        .get(&NodeTypeId::new("yssbi.debug.print").unwrap())
        .unwrap();
    assert_eq!(print.protocol().execution.effects, EffectSemantics::Ordered);
    assert_eq!(print.protocol().execution.purity, Purity::Effectful);
    let message = print
        .protocol()
        .interface
        .ports
        .iter()
        .find(|port| port.key == PortKey::new("message").unwrap())
        .unwrap();
    assert_eq!(
        message
            .input_binding
            .as_ref()
            .and_then(|binding| binding.default_value.as_ref())
            .map(|value| &value.value),
        Some(&Value::String("Hello, World!".into()))
    );
    let _ = ParameterKey::new("unused").unwrap();

    let mut default_print = effect_operation("yssbi.debug.print", "unused.default", &[0]);
    default_print.inputs[0].bound_value = Some(Value::String("Hello, World!".into()));
    execute(
        &plan(vec![default_print], 1, &[]),
        &CompiledParameterStore::new(),
    )
    .unwrap();
}

#[test]
fn builtin_kernels_report_division_by_zero_and_type_errors() {
    let mut parameters = CompiledParameterStore::new();
    insert_constant(&mut parameters, "int.one", Value::Integer(1));
    insert_constant(&mut parameters, "int.zero", Value::Integer(0));
    insert_constant(&mut parameters, "bool.true", Value::Bool(true));
    let divide_by_zero = plan(
        vec![
            operation("yssbi.constant.int64", "int.one", &[], 0),
            operation("yssbi.constant.int64", "int.zero", &[], 1),
            operation("yssbi.numeric.divide.int64", "unused.0", &[0, 1], 2),
        ],
        3,
        &[2],
    );
    let wrong_type = plan(
        vec![
            operation("yssbi.constant.bool", "bool.true", &[], 0),
            operation("yssbi.constant.int64", "int.one", &[], 1),
            operation("yssbi.numeric.add.int64", "unused.1", &[0, 1], 2),
        ],
        3,
        &[2],
    );

    let zero_error = execute(&divide_by_zero, &parameters).unwrap_err();
    let type_error = execute(&wrong_type, &parameters).unwrap_err();

    assert!(
        matches!(zero_error, RunError::KernelFailed { ref message, .. } if message.contains("division by zero"))
    );
    assert!(
        matches!(type_error, RunError::KernelFailed { ref message, .. } if message.contains("expected int64"))
    );
}
