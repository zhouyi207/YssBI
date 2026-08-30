use super::*;

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
        &yss_graph_protocol::data_series_type(TypeExpr::Concrete(
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
        data_series_values(&result.value_for_test("value_3").unwrap()),
        vec![Value::Integer(1), Value::Integer(3)]
    );
}
