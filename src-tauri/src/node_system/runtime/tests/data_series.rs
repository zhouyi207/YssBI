use super::*;

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
fn data_series_artifact_preserves_metadata_through_spill() {
    let root = materialization_test_root("data-series-spill");
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
    let RuntimeValue::Artifact(artifact) = spilled else {
        panic!("spill must produce an artifact");
    };

    assert_eq!(artifact.kind(), ArtifactKind::Spilled);
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

fn numeric_test_tolerance() -> NumericTolerance {
    NumericTolerance {
        absolute: 1e-12,
        relative: 1e-9,
    }
}

fn numeric_view(
    element_type: DataSeriesElementType,
    values: impl Into<Box<[Value]>>,
) -> NumericSeriesView {
    let artifact = DataSeriesBuilder::new(element_type)
        .values(values)
        .build(ArtifactKind::Collected)
        .unwrap();
    numeric_series(&artifact, NullPolicy::Propagate).unwrap()
}

#[test]
fn approximate_equality_handles_special_values_and_exact_ints() {
    let tolerance = numeric_test_tolerance();

    assert!(approximately_equal(0.0, 5e-13, tolerance));
    assert!(approximately_equal(1e9, 1e9 + 0.5, tolerance));
    assert!(!approximately_equal(f64::NAN, f64::NAN, tolerance));
    assert!(approximately_equal(f64::INFINITY, f64::INFINITY, tolerance));
    assert!(!approximately_equal(
        f64::INFINITY,
        f64::NEG_INFINITY,
        tolerance
    ));
    assert!(approximately_equal(0.0, -0.0, tolerance));
    assert!(approximately_zero(-0.0, tolerance));
    assert!(approximately_zero(5e-13, tolerance));
    assert!(!approximately_zero(5e-10, tolerance));
    assert!(
        numeric_equal(
            NumericValue::Int64(9_007_199_254_740_993),
            NumericValue::Int64(9_007_199_254_740_993),
            tolerance
        )
        .unwrap()
    );
    assert!(
        numeric_equal(
            NumericValue::Int64(7),
            NumericValue::Float64(7.0),
            tolerance
        )
        .unwrap()
    );
}

#[test]
fn mixed_numeric_comparison_rejects_lossy_int64_conversion() {
    let error = numeric_equal(
        NumericValue::Int64(9_007_199_254_740_993),
        NumericValue::Float64(9_007_199_254_740_992.0),
        numeric_test_tolerance(),
    )
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "Int64 value 9007199254740993 cannot be represented exactly as Float64"
    );
}

#[test]
fn numeric_ordering_ignores_tolerance() {
    assert_eq!(
        numeric_ordering(
            NumericValue::Float64(1.0),
            NumericValue::Float64(1.0 + 5e-10)
        )
        .unwrap(),
        std::cmp::Ordering::Less
    );
    assert_eq!(
        numeric_ordering(NumericValue::Int64(3), NumericValue::Int64(2)).unwrap(),
        std::cmp::Ordering::Greater
    );
}

#[test]
fn listwise_rows_combines_all_model_inputs() {
    let inputs = [
        numeric_view(
            DataSeriesElementType::Int64,
            [
                Value::Integer(1),
                Value::Null,
                Value::Integer(3),
                Value::Integer(4),
            ],
        ),
        numeric_view(
            DataSeriesElementType::Float64,
            [
                decimal("10"),
                decimal("20"),
                Value::String("NaN".into()),
                decimal("40"),
            ],
        ),
        numeric_view(
            DataSeriesElementType::Float64,
            [
                decimal("100"),
                decimal("200"),
                decimal("300"),
                decimal("400"),
            ],
        ),
    ];

    let rows = prepare_numeric_rows(&inputs, StatisticalMissingValuePolicy::Listwise).unwrap();

    assert_eq!(rows.original_row_count(), 4);
    assert_eq!(rows.used_row_count(), 2);
    assert_eq!(rows.dropped_null_count(), 1);
    assert_eq!(rows.dropped_nan_count(), 1);
    assert_eq!(rows.columns()[0].as_ref(), [1.0, 4.0]);
    assert_eq!(rows.columns()[1].as_ref(), [10.0, 40.0]);
    assert_eq!(rows.columns()[2].as_ref(), [100.0, 400.0]);
}

#[test]
fn reject_missing_value_reports_input_and_row() {
    let null_inputs = [numeric_view(
        DataSeriesElementType::Int64,
        [Value::Integer(1), Value::Null],
    )];
    let null_error =
        prepare_numeric_rows(&null_inputs, StatisticalMissingValuePolicy::Reject).unwrap_err();
    assert_eq!(
        null_error.message(),
        "numeric input 0 contains Null at row 1"
    );

    let nan_inputs = [numeric_view(
        DataSeriesElementType::Float64,
        [decimal("1"), Value::String("NaN".into())],
    )];
    let nan_error =
        prepare_numeric_rows(&nan_inputs, StatisticalMissingValuePolicy::Reject).unwrap_err();
    assert_eq!(nan_error.message(), "numeric input 0 contains NaN at row 1");
}

#[test]
fn kernel_context_receives_an_immutable_effective_settings_snapshot() {
    struct SettingsKernel;

    impl Kernel for SettingsKernel {
        fn execute(
            &self,
            context: &KernelContext<'_>,
            _: &[RuntimeValue],
        ) -> Result<Vec<RuntimeValue>, KernelError> {
            let settings = context.computation_settings();
            assert_eq!(settings.numeric_tolerance.absolute, 0.25);
            assert_eq!(settings.numeric_tolerance.relative, 0.5);
            assert_eq!(
                settings.statistical_missing_value_policy,
                StatisticalMissingValuePolicy::Reject
            );
            Ok(Vec::new())
        }
    }

    let mut kernels = KernelRegistry::new();
    kernels
        .register(id("settings.snapshot", KernelHandle::new), SettingsKernel)
        .unwrap();
    let mut settings = crate::project::ProjectComputationSettings::default();
    settings.numeric.tolerance = NumericTolerance {
        absolute: 0.25,
        relative: 0.5,
    };
    settings.missing_values.statistics = StatisticalMissingValuePolicy::Reject;
    let resources = no_resources();
    let executor = RunExecutor::new(
        &kernels,
        &resources,
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_computation_settings_snapshot(&settings);
    settings.numeric.tolerance.absolute = 9.0;
    assert_eq!(settings.numeric.tolerance.absolute, 9.0);
    let execution_plan = plan(
        vec![operation("settings.snapshot", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );

    executor
        .run(&execution_plan, CancellationToken::new())
        .unwrap();
}

#[test]
fn numeric_row_preparation_rejects_infinity_and_mismatched_lengths() {
    let infinity_inputs = [numeric_view(
        DataSeriesElementType::Float64,
        [decimal("1"), Value::String("Infinity".into())],
    )];
    let infinity_error =
        prepare_numeric_rows(&infinity_inputs, StatisticalMissingValuePolicy::Listwise)
            .unwrap_err();
    assert_eq!(
        infinity_error.message(),
        "numeric input 0 contains positive infinity at row 1"
    );

    let mismatched_inputs = [
        numeric_view(DataSeriesElementType::Int64, [Value::Integer(1)]),
        numeric_view(
            DataSeriesElementType::Int64,
            [Value::Integer(1), Value::Integer(2)],
        ),
    ];
    let mismatch_error =
        prepare_numeric_rows(&mismatched_inputs, StatisticalMissingValuePolicy::Listwise)
            .unwrap_err();
    assert_eq!(
        mismatch_error.message(),
        "numeric input 1 has 2 rows; expected 1"
    );
}
