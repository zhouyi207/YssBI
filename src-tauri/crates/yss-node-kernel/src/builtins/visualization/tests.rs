use crate::*;
use std::{
    borrow::Cow,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use yss_data_contract::{
    ColumnSemantic, ConversionMetadata, SemanticType, SemanticValue, TabularScalar, ValueType,
};

fn number(value: f64) -> RuntimeValue {
    RuntimeValue::float64(value).unwrap()
}
fn integer(value: i64) -> RuntimeValue {
    TabularScalar::Integer(value).into()
}
fn text(value: &str) -> RuntimeValue {
    TabularScalar::String(value.into()).into()
}
fn flag(value: bool) -> RuntimeValue {
    TabularScalar::Bool(value).into()
}
fn series(values: &[f64]) -> RuntimeValue {
    RuntimeValue::List(values.iter().map(|v| number(*v)).collect())
}
fn words(values: &[&str]) -> RuntimeValue {
    RuntimeValue::List(values.iter().map(|v| text(v)).collect())
}
fn field<'a>(value: &'a RuntimeValue, key: &str) -> &'a RuntimeValue {
    let RuntimeValue::Record(fields) = value else {
        panic!("record")
    };
    &fields[key]
}
fn run(
    id: &str,
    inputs: &[(&str, RuntimeValue)],
    params: &[(&str, RuntimeValue)],
    outputs: &[ValueType],
) -> Result<Vec<RuntimeValue>, KernelError> {
    let control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    KernelRegistry::default().execute(
        &KernelId::new(id.into()).unwrap(),
        &KernelInvocation {
            relations: &crate::tests::relations(),
            inputs: &inputs.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>(),
            input_keys: &inputs.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
            parameters: params
                .iter()
                .map(|(key, value)| {
                    (
                        KernelParameterKey::new((*key).into()).unwrap(),
                        Cow::Borrowed(value),
                    )
                })
                .collect(),
            outputs: &outputs
                .iter()
                .map(|ty| KernelOutputSpec {
                    data_type: ty.clone(),
                    fields: None,
                })
                .collect::<Vec<_>>(),
            control: &control,
        },
    )
}
fn plot(
    name: &str,
    inputs: &[(&str, RuntimeValue)],
    parameters: &[(&str, RuntimeValue)],
) -> Result<Vec<RuntimeValue>, KernelError> {
    run(
        &format!("yssbi.plot.{name}.view"),
        inputs,
        parameters,
        &[ValueType::Struct("plot.data".into())],
    )
}

#[test]
fn distribution_plots_compute_from_all_rows() {
    for values in [vec![3.; 640], (0..640).map(|i| i as f64).collect()] {
        let histogram = plot(
            "histogram",
            &[("values", series(&values))],
            &[("bins", integer(0))],
        )
        .unwrap();
        assert_eq!(field(&histogram[0], "observations"), &integer(640));
        let RuntimeValue::List(bins) = field(&histogram[0], "data") else {
            panic!("histogram")
        };
        let count: i64 = bins
            .iter()
            .map(|b| match field(b, "count") {
                RuntimeValue::Scalar(TabularScalar::Integer(v)) => *v,
                _ => panic!("count"),
            })
            .sum();
        assert_eq!(count, 640);
        let ecdf = plot("ecdf", &[("values", series(&values))], &[]).unwrap();
        assert_eq!(
            field(field(&ecdf[0], "metadata"), "observations"),
            &integer(640)
        );
        let RuntimeValue::List(points) = field(&ecdf[0], "data") else {
            panic!("ecdf")
        };
        assert_eq!(field(points.last().unwrap(), "y"), &number(1.));
        let boxplot = plot("boxplot", &[("series", series(&values))], &[]).unwrap();
        let RuntimeValue::List(groups) = field(&boxplot[0], "groups") else {
            panic!("boxplot")
        };
        assert_eq!(field(&groups[0], "observations"), &integer(640));
        assert_eq!(
            field(&groups[0], "median"),
            &number(if values[0] == values[639] { 3. } else { 319.5 })
        );
    }
}
#[test]
fn every_visualization_kernel_executes_its_declared_input_layout_and_plot_carrier() {
    let x = series(&[1., 2., 3., 4., 5., 6., 7., 8.]);
    let y = series(&[2., 2.9, 4.2, 3.8, 5.1, 6.2, 5.9, 7.3]);
    let categories = words(&["A", "B", "A", "C", "A", "B", "C", "B"]);
    let pair = vec![("x", x.clone()), ("y", y.clone())];
    let values = vec![("values", x.clone())];
    let group = vec![("series", x.clone()), ("series", y.clone())];
    let cases = vec![
        ("scatter", pair.clone(), vec![]),
        ("line", pair.clone(), vec![]),
        ("ecdf", values.clone(), vec![]),
        ("kde", values.clone(), vec![("grid_points", integer(256))]),
        ("histogram", values.clone(), vec![("bins", integer(0))]),
        ("correlation", group.clone(), vec![]),
        (
            "correlogram",
            values.clone(),
            vec![("maximum_lag", integer(20))],
        ),
        ("boxplot", group.clone(), vec![]),
        ("violin", group.clone(), vec![]),
        ("heatmap", group, vec![]),
        (
            "wordcloud",
            vec![("words", categories.clone())],
            vec![("max_words", integer(100))],
        ),
        ("pareto", vec![("categories", categories.clone())], vec![]),
        (
            "combination",
            vec![
                ("categories", categories),
                ("bars", x.clone()),
                ("line", y.clone()),
            ],
            vec![("dual_axis", flag(true))],
        ),
        (
            "quadrant",
            pair.clone(),
            vec![("x_cut", number(4.)), ("y_cut", number(4.))],
        ),
        (
            "bubble",
            vec![("x", x.clone()), ("y", y.clone()), ("size", x.clone())],
            vec![],
        ),
        (
            "errorbar",
            vec![
                ("x", x.clone()),
                ("y", x.clone()),
                ("lower", series(&[0., 1., 2., 3., 4., 5., 6., 7.])),
                ("upper", series(&[2., 3., 4., 5., 6., 7., 8., 9.])),
            ],
            vec![],
        ),
        (
            "pp_qq",
            values,
            vec![
                ("mode", text("qq")),
                ("estimate_parameters", flag(true)),
                ("reference_mean", number(0.)),
                ("reference_standard_deviation", number(1.)),
            ],
        ),
        (
            "roc",
            vec![
                (
                    "labels",
                    RuntimeValue::List(
                        [false, true, false, true, false, true, false, true]
                            .map(flag)
                            .into(),
                    ),
                ),
                ("scores", x.clone()),
            ],
            vec![],
        ),
    ];
    for (name, inputs, parameters) in cases {
        let output =
            plot(name, &inputs, &parameters).unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert!(
            matches!(&output[0], RuntimeValue::Record(fields) if !fields.is_empty()),
            "{name}"
        );
    }
    let fit = run(
        "yssbi.statistics.linear.fit",
        &[("y", y), ("x", x)],
        &[
            ("method", text("OLS")),
            ("constant", flag(true)),
            ("covariance", text("nonrobust")),
        ],
        &[
            ValueType::Struct("statistics.model.linear".into()),
            ValueType::DataSeries(Box::new(ValueType::number())),
            ValueType::DataSeries(Box::new(ValueType::number())),
        ],
    )
    .unwrap();
    let coefficient = plot(
        "coefficient",
        &[("model", fit[0].clone())],
        &[
            ("confidence_level", number(0.95)),
            ("include_intercept", flag(false)),
        ],
    )
    .unwrap();
    let RuntimeValue::List(data) = field(&coefficient[0], "data") else {
        panic!("coefficient points")
    };
    assert_eq!(data.len(), 1);
}

#[test]
fn roc_respects_declared_positive_binary_values_and_plot_inputs_reject_missing_rows() {
    let mut meaning = ColumnSemantic::new(SemanticType::Binary);
    meaning.values = vec![
        SemanticValue {
            value: "false".into(),
            label: "Positive".into(),
        },
        SemanticValue {
            value: "true".into(),
            label: "Negative".into(),
        },
    ];
    meaning.positive_value = Some("false".into());
    let labels = RuntimeValue::List([false, false, true, true].map(flag).into())
        .with_metadata(ConversionMetadata {
            semantic: meaning,
            temporal: None,
            dummy_base_level: None,
        })
        .unwrap();
    let output = plot(
        "roc",
        &[
            ("labels", labels),
            ("scores", series(&[0.1, 0.2, 0.8, 0.9])),
        ],
        &[],
    )
    .unwrap();
    assert_eq!(
        super::numeric_input(Some(field(&output[0], "auc"))).unwrap(),
        0.
    );
    assert!(matches!(
        plot(
            "scatter",
            &[("x", series(&[1., 2.])), ("y", series(&[3.]))],
            &[]
        ),
        Err(KernelError::ShapeMismatch)
    ));
    let missing = RuntimeValue::List(vec![number(1.), TabularScalar::Null.into()].into());
    assert!(matches!(
        plot("histogram", &[("values", missing)], &[("bins", integer(2))]),
        Err(KernelError::InvalidNumericInput)
    ));
}
