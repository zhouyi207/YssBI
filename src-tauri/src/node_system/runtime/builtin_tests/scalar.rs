use super::*;

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

    assert_eq!(
        result.value_for_test("value_0").unwrap(),
        Value::Bool(true).into()
    );
    assert_eq!(
        result.value_for_test("value_1").unwrap(),
        Value::String("hello".into()).into()
    );
    assert_eq!(
        result.value_for_test("value_2").unwrap(),
        Value::Integer(42).into()
    );
    assert_eq!(
        result.value_for_test("value_3").unwrap(),
        decimal("1.25").into()
    );
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
        assert_eq!(
            result.value_for_test(key.as_str()).unwrap(),
            Value::Integer(expected).into()
        );
    }
    for (value, expected) in [(8, "10"), (9, "5"), (10, "18.75"), (11, "3")] {
        let key = format!("value_{value}");
        assert_eq!(
            result.value_for_test(key.as_str()).unwrap(),
            decimal(expected).into()
        );
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
        assert_eq!(
            result.value_for_test(key.as_str()).unwrap(),
            Value::Bool(expected).into()
        );
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
            result.value_for_test("value_2").unwrap(),
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
            ArtifactKind::Collected,
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
