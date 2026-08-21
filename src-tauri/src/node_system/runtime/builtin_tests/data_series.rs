use super::*;

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
        ArtifactKind::Collected,
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
        ArtifactKind::Collected,
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
fn series_conversion_kernels_convert_supported_types_and_preserve_nulls() {
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
