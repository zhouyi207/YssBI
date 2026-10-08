use super::*;
use arrow_array::{ArrayRef, Float64Array, RecordBatch, UInt64Array};
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
}
