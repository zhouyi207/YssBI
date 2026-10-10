use super::*;
use arrow_array::{ArrayRef, Float64Array, Int64Array, RecordBatch, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use yss_data_contract::{ColumnSemantic, SemanticType, SemanticValue};
use yss_database_engine::DataFusionRuntime;
use yss_relational_contract::{RelationControl, RelationFactory};

fn fixture() -> (Arc<dyn RelationFactory>, KernelControl) {
    (
        DataFusionRuntime::unbounded(32).unwrap(),
        KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        ),
    )
}

fn outputs(names: &[&str]) -> [KernelOutputSpec; 2] {
    [
        KernelOutputSpec {
            data_type: ValueType::Struct("statistics.report".into()),
            fields: None,
        },
        KernelOutputSpec {
            data_type: ValueType::DataFrame,
            fields: Some(
                names
                    .iter()
                    .map(|name| KernelField {
                        name: (*name).into(),
                        data_type: ValueType::number(),
                    })
                    .collect(),
            ),
        },
    ]
}

fn ordinal_factor(
    factory: &Arc<dyn RelationFactory>,
    control: &KernelControl,
    name: &str,
    label_bytes: usize,
    malformed: bool,
) -> RuntimeValue {
    let semantic = ColumnSemantic {
        values: ["0", "1"]
            .into_iter()
            .map(|value| SemanticValue {
                value: value.into(),
                label: "L".repeat(label_bytes),
            })
            .collect(),
        ..ColumnSemantic::new(SemanticType::Ordinal)
    };
    let mut field = yss_database_arrow::with_column_semantic(
        Field::new(name, DataType::Int64, false),
        &semantic,
    )
    .unwrap();
    if malformed {
        field
            .metadata_mut()
            .get_mut("yssbi.semantic")
            .unwrap()
            .push_str(" invalid");
    }
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![field])),
        vec![Arc::new(Int64Array::from(vec![0, 1, 0, 1]))],
    )
    .unwrap();
    let source = factory
        .clone()
        .materialize(
            batch,
            &RelationControl {
                cancellation: control.cancellation.clone(),
                deadline: control.deadline,
                max_input_bytes: usize::MAX,
            },
        )
        .unwrap();
    RuntimeValue::Series(source.select_series(name).unwrap())
}

fn range_with_control(
    factory: &Arc<dyn RelationFactory>,
    control: &KernelControl,
    factors: &[RuntimeValue],
) -> Result<Vec<RuntimeValue>, KernelError> {
    let mut inputs = vec![series(&[2., 4., 6., 8.])];
    inputs.extend_from_slice(factors);
    let keys = std::iter::once("y")
        .chain(std::iter::repeat_n("factors", factors.len()))
        .collect::<Vec<_>>();
    KernelRegistry::default().execute(
        &KernelId::new("yssbi.statistics.doe.range_analysis".into()).unwrap(),
        &KernelInvocation {
            relations: factory,
            inputs: &inputs,
            input_keys: &keys,
            parameters: [(
                KernelParameterKey::new("maximize".into()).unwrap(),
                Cow::Owned(flag(true)),
            )]
            .into(),
            outputs: &outputs(&["factor", "level", "observations", "total", "mean"]),
            control,
        },
    )
}

#[test]
fn typed_series_admits_semantic_metadata_before_decoding() {
    let (factory, mut control) = fixture();
    let factor = ordinal_factor(&factory, &control, "factor", 128 * 1024, true);
    control.max_input_bytes = 64 * 1024;
    let error = range_with_control(&factory, &control, std::slice::from_ref(&factor)).err();
    assert!(
        matches!(error, Some(KernelError::BudgetExceeded)),
        "{error:?}"
    );
    control.max_input_bytes = usize::MAX;
    assert!(matches!(
        range_with_control(&factory, &control, &[factor]),
        Err(KernelError::InvalidParameter)
    ));
}

#[test]
fn typed_series_carries_metadata_admission_between_independent_sources() {
    let (factory, mut control) = fixture();
    let factors = [
        ordinal_factor(&factory, &control, "factor_a", 4096, false),
        ordinal_factor(&factory, &control, "factor_b", 4096, false),
    ];
    control.max_input_bytes = 384 * 1024;
    for factor in &factors {
        range_with_control(&factory, &control, std::slice::from_ref(factor)).unwrap();
    }
    let error = range_with_control(&factory, &control, &factors).err();
    assert!(
        matches!(error, Some(KernelError::BudgetExceeded)),
        "{error:?}"
    );
    control.max_input_bytes = usize::MAX;
    let result = range_with_control(&factory, &control, &factors).unwrap();
    assert_eq!(
        field(&result[0], "factor_names").unwrap(),
        &RuntimeValue::List(vec![string("factor_a"), string("factor_b")].into()),
    );
}

#[test]
fn prepared_numeric_columns_preserve_mixed_source_questionnaire_outputs() {
    let (factory, control) = fixture();
    let relation_control = RelationControl {
        cancellation: control.cancellation.clone(),
        deadline: control.deadline,
        max_input_bytes: control.max_input_bytes,
    };
    let source = factory
        .clone()
        .materialize(
            RecordBatch::try_from_iter([(
                "database_item",
                Arc::new(Float64Array::from(vec![1., 2., 3., 4.])) as ArrayRef,
            )])
            .unwrap(),
            &relation_control,
        )
        .unwrap();
    let result = KernelRegistry::default()
        .execute(
            &KernelId::new("yssbi.statistics.psychometrics.reliability".into()).unwrap(),
            &KernelInvocation {
                relations: &factory,
                inputs: &[
                    RuntimeValue::Series(source.select_series("database_item").unwrap()),
                    series(&[2., 4., 6., 8.]),
                ],
                input_keys: &["items", "items"],
                parameters: Default::default(),
                outputs: &outputs(&[
                    "item",
                    "mean",
                    "standard_deviation",
                    "corrected_item_total_correlation",
                    "alpha_if_deleted",
                ]),
                control: &control,
            },
        )
        .unwrap();
    let alpha =
        crate::builtins::numeric_input(Some(field(&result[0], "raw_alpha").unwrap())).unwrap();
    assert!((alpha - 8. / 9.).abs() < 1e-12);
    assert_eq!(
        field(&result[0], "item_names").unwrap(),
        &RuntimeValue::List(vec![string("database_item"), string("item2")].into()),
    );
    let RuntimeValue::Relation(table) = &result[1] else {
        panic!("questionnaire details must retain a relation");
    };
    let page = table.page(0, 10, &relation_control).unwrap();
    assert_eq!(
        page.data.columns()[1].values(),
        &[2.5_f64, 5.0].map(|v| TabularScalar::Float64(v.try_into().unwrap())),
    );
    assert_eq!(
        page.data.columns()[4].values(),
        &[TabularScalar::Null, TabularScalar::Null],
    );
    let heterogeneity = KernelRegistry::default()
        .execute(
            &KernelId::new("yssbi.statistics.meta.cochran_q".into()).unwrap(),
            &KernelInvocation {
                relations: &factory,
                inputs: &[
                    RuntimeValue::Series(source.select_series("database_item").unwrap()),
                    series(&[1., 1., 1., 1.]),
                ],
                input_keys: &["effects", "variances"],
                parameters: Default::default(),
                outputs: &[KernelOutputSpec {
                    data_type: ValueType::Struct("statistics.report".into()),
                    fields: None,
                }],
                control: &control,
            },
        )
        .unwrap();
    let q = crate::builtins::numeric_input(Some(field(&heterogeneity[0], "q").unwrap())).unwrap();
    assert!((q - 5.).abs() < 1e-12);
    assert_eq!(
        field(&heterogeneity[0], "degrees_of_freedom").unwrap(),
        &int(3)
    );
}

#[test]
fn numeric_only_adapters_reject_categorical_relation_codes() {
    let (factory, control) = fixture();
    let relation_control = RelationControl {
        cancellation: control.cancellation.clone(),
        deadline: control.deadline,
        max_input_bytes: control.max_input_bytes,
    };
    let field = yss_database_arrow::with_column_semantic(
        Field::new("coded_item", DataType::Int64, false),
        &ColumnSemantic::new(SemanticType::Categorical),
    )
    .unwrap();
    let source = factory
        .clone()
        .materialize(
            RecordBatch::try_new(
                Arc::new(Schema::new(vec![field])),
                vec![Arc::new(Int64Array::from(vec![1, 2, 3, 4]))],
            )
            .unwrap(),
            &relation_control,
        )
        .unwrap();
    let coded = source.select_series("coded_item").unwrap();
    assert_eq!(
        yss_database_arrow::column_semantic(coded.plan().field())
            .unwrap()
            .kind,
        SemanticType::Categorical,
    );
    for (id, keys, specifications, second) in [
        (
            "yssbi.statistics.psychometrics.reliability",
            ["items", "items"],
            outputs(&[
                "item",
                "mean",
                "standard_deviation",
                "corrected_item_total_correlation",
                "alpha_if_deleted",
            ])
            .to_vec(),
            series(&[2., 4., 6., 8.]),
        ),
        (
            "yssbi.statistics.meta.cochran_q",
            ["effects", "variances"],
            vec![KernelOutputSpec {
                data_type: ValueType::Struct("statistics.report".into()),
                fields: None,
            }],
            series(&[1., 1., 1., 1.]),
        ),
    ] {
        let result = KernelRegistry::default().execute(
            &KernelId::new(id.into()).unwrap(),
            &KernelInvocation {
                relations: &factory,
                inputs: &[RuntimeValue::Series(coded.clone()), second],
                input_keys: &keys,
                parameters: Default::default(),
                outputs: &specifications,
                control: &control,
            },
        );
        assert!(
            matches!(result, Err(KernelError::InvalidNumericInput)),
            "{id}: {result:?}",
        );
    }
}

#[test]
fn prepared_group_columns_preserve_wide_labels_and_doe_detail_rows() {
    let (factory, control) = fixture();
    let relation_control = RelationControl {
        cancellation: control.cancellation.clone(),
        deadline: control.deadline,
        max_input_bytes: control.max_input_bytes,
    };
    let labels = [u64::MAX, u64::MAX - 1];
    let source = factory
        .clone()
        .materialize(
            RecordBatch::try_from_iter([(
                "database_factor",
                Arc::new(UInt64Array::from(vec![
                    labels[0], labels[1], labels[0], labels[1],
                ])) as ArrayRef,
            )])
            .unwrap(),
            &relation_control,
        )
        .unwrap();
    let result = KernelRegistry::default()
        .execute(
            &KernelId::new("yssbi.statistics.doe.range_analysis".into()).unwrap(),
            &KernelInvocation {
                relations: &factory,
                inputs: &[
                    series(&[2., 4., 6., 8.]),
                    RuntimeValue::Series(source.select_series("database_factor").unwrap()),
                ],
                input_keys: &["y", "factors"],
                parameters: [(
                    KernelParameterKey::new("maximize".into()).unwrap(),
                    Cow::Owned(flag(true)),
                )]
                .into(),
                outputs: &outputs(&["factor", "level", "observations", "total", "mean"]),
                control: &control,
            },
        )
        .unwrap();
    assert_eq!(
        field(&result[0], "level_labels").unwrap(),
        &RuntimeValue::List(
            vec![RuntimeValue::List(
                labels
                    .into_iter()
                    .map(|v| TabularScalar::Unsigned(v).into())
                    .collect(),
            )]
            .into(),
        ),
    );
    assert_eq!(
        field(&result[0], "factor_names").unwrap(),
        &RuntimeValue::List(vec![string("database_factor")].into()),
    );
    let RuntimeValue::Relation(table) = &result[1] else {
        panic!("DOE level details must retain a relation");
    };
    let page = table.page(0, 10, &relation_control).unwrap();
    assert_eq!(
        page.data.columns()[2].values(),
        &[2.0_f64, 2.0].map(|v| TabularScalar::Float64(v.try_into().unwrap())),
    );
    assert_eq!(
        page.data.columns()[4].values(),
        &[4.0_f64, 6.0].map(|v| TabularScalar::Float64(v.try_into().unwrap())),
    );
    let conjoint = KernelRegistry::default()
        .execute(
            &KernelId::new("yssbi.statistics.decision.conjoint".into()).unwrap(),
            &KernelInvocation {
                relations: &factory,
                inputs: &[
                    series(&[2., 4., 6., 8.]),
                    RuntimeValue::Series(source.select_series("database_factor").unwrap()),
                ],
                input_keys: &["ratings", "factors"],
                parameters: Default::default(),
                outputs: &outputs(&["observation", "observed", "fitted", "residual"]),
                control: &control,
            },
        )
        .unwrap();
    assert_eq!(
        field(&conjoint[0], "level_labels").unwrap(),
        field(&result[0], "level_labels").unwrap(),
    );
    assert_eq!(
        field(&conjoint[0], "factor_names").unwrap(),
        field(&result[0], "factor_names").unwrap(),
    );
    let RuntimeValue::Relation(table) = &conjoint[1] else {
        panic!("conjoint fitted observations must retain a relation");
    };
    let page = table.page(0, 10, &relation_control).unwrap();
    let fitted = page.data.columns()[2].values();
    assert_eq!(fitted.len(), 4);
    for (actual, expected) in fitted.iter().zip([4., 6., 4., 6.]) {
        let TabularScalar::Float64(actual) = actual else {
            panic!("fitted observations must be numeric");
        };
        assert!((actual.as_f64() - expected).abs() < 1e-12);
    }
}
