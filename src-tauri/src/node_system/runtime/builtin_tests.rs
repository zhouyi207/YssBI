use super::*;
use crate::node_system::analysis::{
    CompilationBasis, CompileId, CompileProvenance, ProjectSessionId,
};
use crate::node_system::document::{GraphResourcePath, GraphRevision, NodeId};
use crate::node_system::plan::*;
use crate::node_system::protocol::{
    CanonicalDecimal, InputConsumption, NodeTypeId, OutputProduction, Value,
};
use crate::node_system::registry::RegistryFingerprint;
use std::collections::BTreeMap;

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

fn operation(kernel: &str, params: &str, inputs: &[u32], output: u32) -> PlannedOperation {
    PlannedOperation {
        source_node_id: NodeId::from_uuid(uuid::Uuid::new_v4()),
        source_node_type_id: NodeTypeId::new("yssbi.test.builtin").unwrap(),
        kernel: PlannedKernel::Native(handle(kernel, KernelHandle::new)),
        inputs: inputs
            .iter()
            .map(|value| PlannedInput {
                value: ValueRef::new(*value),
                consumption: InputConsumption::FullyMaterialized,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        outputs: Box::new([PlannedOutput {
            value: ValueRef::new(output),
            production: OutputProduction::FullyMaterialized,
        }]),
        params: handle(params, CompiledParameterHandle::new),
    }
}

fn plan(operations: Vec<PlannedOperation>, value_count: u32, results: &[u32]) -> ExecutionPlan {
    ExecutionPlan {
        provenance: CompileProvenance {
            project_session_id: ProjectSessionId::new("builtin-kernel-test"),
            graph_path: GraphResourcePath("events/builtin-kernel-test".into()),
            basis: CompilationBasis {
                graph_revision: GraphRevision::new(1),
                registry_fingerprint: RegistryFingerprint::from_bytes([7; 32]),
                resource_versions: BTreeMap::new(),
            },
            compile_id: CompileId::new(1),
        },
        value_count,
        value_sources: Box::new([]),
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
        results: results
            .iter()
            .map(|value| PlanResult {
                name: format!("value_{value}").into(),
                value: ValueRef::new(*value),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
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
            production: OutputProduction::FullyMaterialized,
        },
        PlannedOutput {
            value: ValueRef::new(6),
            production: OutputProduction::FullyMaterialized,
        },
        PlannedOutput {
            value: ValueRef::new(7),
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
    assert_eq!(
        result.values["value_6"],
        Value::List(vec![decimal("1"), decimal("2"), decimal("3"), decimal("4"),]).into()
    );
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
            production: OutputProduction::FullyMaterialized,
        },
        PlannedOutput {
            value: ValueRef::new(6),
            production: OutputProduction::FullyMaterialized,
        },
        PlannedOutput {
            value: ValueRef::new(7),
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
