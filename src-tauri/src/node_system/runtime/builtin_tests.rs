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
    CachePolicy, CanonicalDecimal, InputConsumption, NodeTypeId, OutputProduction, PortKey, Value,
};
use crate::node_system::registry::RegistryFingerprint;
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

fn assert_decimal_list_approx_eq(actual: &RuntimeValue, expected: &[f64]) {
    let RuntimeValue::Scalar(Value::List(actual)) = actual else {
        panic!("expected scalar decimal list, got {actual:?}");
    };
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
        result.values["value_3"],
        Value::List(vec![Value::Integer(1), Value::Integer(3)]).into()
    );
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
        (
            "int",
            decimal("7.9"),
            ConvertTarget::Int64,
            Value::Integer(7),
        ),
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
fn series_conversion_kernels_cover_every_legacy_conversion() {
    let cases = [
        (
            "yssbi.data_series.convert.string_to_categorical",
            Value::String("blue".into()),
            Value::String("blue".into()),
        ),
        (
            "yssbi.data_series.convert.string_to_float64",
            Value::String("2.5".into()),
            decimal("2.5"),
        ),
        (
            "yssbi.data_series.convert.string_to_int64",
            Value::String("2".into()),
            Value::Integer(2),
        ),
        (
            "yssbi.data_series.convert.int64_to_string",
            Value::Integer(2),
            Value::String("2".into()),
        ),
        (
            "yssbi.data_series.convert.float64_to_string",
            decimal("2.5"),
            Value::String("2.5".into()),
        ),
        (
            "yssbi.data_series.convert.int64_to_float64",
            Value::Integer(2),
            decimal("2"),
        ),
        (
            "yssbi.data_series.convert.float64_to_int64",
            decimal("2.9"),
            Value::Integer(2),
        ),
        (
            "yssbi.data_series.convert.int64_to_bool",
            Value::Integer(1),
            Value::Bool(true),
        ),
        (
            "yssbi.data_series.convert.float64_to_bool",
            decimal("0"),
            Value::Bool(false),
        ),
        (
            "yssbi.data_series.convert.categorical_to_string",
            Value::String("blue".into()),
            Value::String("blue".into()),
        ),
        (
            "yssbi.data_series.convert.int64_to_categorical",
            Value::Integer(2),
            Value::String("2".into()),
        ),
        (
            "yssbi.data_series.convert.categorical_to_int64",
            Value::String("2".into()),
            Value::Integer(2),
        ),
        (
            "yssbi.data_series.convert.float64_to_categorical",
            decimal("2.5"),
            Value::String("2.5".into()),
        ),
        (
            "yssbi.data_series.convert.categorical_to_float64",
            Value::String("2.5".into()),
            decimal("2.5"),
        ),
    ];
    let params = handle("series.convert", CompiledParameterHandle::new);
    for (kernel, input, expected) in cases {
        let output = execute_kernel_direct(
            kernel,
            &params,
            None,
            &[RuntimeValue::Artifact(Artifact::new(
                ArtifactKind::Replayable,
                vec![input, Value::Null],
            ))],
        )
        .unwrap();
        assert_eq!(
            output,
            vec![RuntimeValue::Artifact(Artifact::new(
                ArtifactKind::Replayable,
                vec![expected, Value::Null],
            ))],
            "{kernel}",
        );
    }

    let parse_error = execute_kernel_direct(
        "yssbi.data_series.convert.string_to_int64",
        &params,
        None,
        &[RuntimeValue::Artifact(Artifact::new(
            ArtifactKind::Replayable,
            vec![Value::String("bad".into())],
        ))],
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
        &[RuntimeValue::Artifact(Artifact::new(
            ArtifactKind::Replayable,
            vec![Value::Integer(1)],
        ))],
    )
    .unwrap_err();
    assert_eq!(
        source_type_error.message(),
        "DataSeries element 0: expected String, got Int64"
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
        "DataSeries conversion expects a fully materialized artifact"
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
