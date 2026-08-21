use super::*;

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
    assert!(matches!(outputs[0], RuntimeValue::Scalar(Value::Object(_))));
    let report = scalar_object(&outputs[1]);
    for key in [
        "title",
        "model_basic_info",
        "coefficients",
        "diagnostic_info",
    ] {
        assert!(report.contains_key(key), "missing report field '{key}'");
    }
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
fn ols_fit_produces_a_model_and_fitted_values() {
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
            public_output: None,
            presentation: crate::node_system::plan::ResultPresentation::Inspector,
        },
        PlannedOutput {
            value: ValueRef::new(6),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            production: OutputProduction::FullyMaterialized,
            public_output: None,
            presentation: crate::node_system::plan::ResultPresentation::Inspector,
        },
        PlannedOutput {
            value: ValueRef::new(7),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            production: OutputProduction::FullyMaterialized,
            public_output: None,
            presentation: crate::node_system::plan::ResultPresentation::Inspector,
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
        result.value_for_test("value_5").unwrap(),
        RuntimeValue::Scalar(Value::Object(_))
    ));
    assert_decimal_list_approx_eq(
        &result.value_for_test("value_6").unwrap(),
        &[1.0, 2.0, 3.0, 4.0],
    );
}

#[test]
fn logit_fit_rejects_non_binary_response_values() {
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
            public_output: None,
            presentation: crate::node_system::plan::ResultPresentation::Inspector,
        },
        PlannedOutput {
            value: ValueRef::new(6),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            production: OutputProduction::FullyMaterialized,
            public_output: None,
            presentation: crate::node_system::plan::ResultPresentation::Inspector,
        },
        PlannedOutput {
            value: ValueRef::new(7),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            production: OutputProduction::FullyMaterialized,
            public_output: None,
            presentation: crate::node_system::plan::ResultPresentation::Inspector,
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
