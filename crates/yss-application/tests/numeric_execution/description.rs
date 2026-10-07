use super::fixture::{columns, connect, field, node, number};
use super::*;
use serde_json::json;

#[test]
fn description_frequencies_preserve_exact_decimal_codes() {
    use arrow::{
        array::Decimal128Array,
        datatypes::{DataType, Field, Schema},
        record_batch::RecordBatch,
    };
    use yss_data_contract::{ColumnSemantic, SemanticType, SemanticValue, ValueType};
    use yss_node_kernel::{KernelControl, KernelInvocation, KernelOutputSpec, KernelRegistry};

    let codes = ["90071992547409.93", "90071992547409.94"];
    let category_field = yss_database_arrow::with_column_semantic(
        Field::new("code", DataType::Decimal128(38, 2), false),
        &ColumnSemantic {
            kind: SemanticType::Categorical,
            values: codes
                .into_iter()
                .map(|value| SemanticValue {
                    value: value.into(),
                    label: format!("code {value}"),
                })
                .collect(),
            positive_value: None,
            numeric: None,
        },
    )
    .unwrap();
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![category_field])),
        vec![Arc::new(
            Decimal128Array::from(vec![
                9_007_199_254_740_993_i128,
                9_007_199_254_740_994,
                9_007_199_254_740_993,
            ])
            .with_precision_and_scale(38, 2)
            .unwrap(),
        )],
    )
    .unwrap();
    let relations: Arc<dyn yss_relational_contract::RelationFactory> =
        yss_database_runtime::dataset_query_engine().unwrap();
    let control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    let outputs = [KernelOutputSpec {
        data_type: ValueType::Struct("statistics.report".into()),
        fields: None,
    }];
    let mut invocation = KernelInvocation {
        relations: &relations,
        inputs: &[],
        input_keys: &["source"],
        parameters: Default::default(),
        outputs: &outputs,
        control: &control,
    };
    let inputs = [RuntimeValue::Relation(
        relations
            .clone()
            .materialize(batch, &invocation.relation_control())
            .unwrap(),
    )];
    invocation.inputs = &inputs;
    let output = KernelRegistry::default()
        .execute(
            &yss_node_kernel::KernelId::new("yssbi.statistics.describe".into()).unwrap(),
            &invocation,
        )
        .unwrap();
    let summary = field(field(&output[0], "columns"), "code");
    let categories = field(summary, "categories");
    for (position, code, frequency) in [("1", codes[0], 2.), ("2", codes[1], 1.)] {
        let category = field(categories, position);
        assert_eq!(
            field(category, "value"),
            &TabularScalar::String(code.into()).into()
        );
        assert_eq!(
            field(category, "label"),
            &TabularScalar::String(format!("code {code}").into()).into()
        );
        assert_eq!(number(field(category, "frequency")), frequency);
    }
}

#[test]
fn description_frequencies_keep_declared_ordinal_order_labels_and_exact_codes() {
    let mut document = GraphDocument::default();
    let high = 9_007_199_254_740_993_u64;
    let low = high - 1;
    let source = columns(&mut document, &[("codes", json!([high, low, null, high]))])["codes"];
    let labels = node(
        &mut document,
        "yssbi.dataframe.labels",
        json!({
            "target_type": "core.ordinal",
            "semantic_domain": {
                "values": [
                    {"value": high.to_string(), "label": "低"},
                    {"value": low.to_string(), "label": "高"}
                ]
            }
        }),
    );
    let describe = node(&mut document, "yssbi.statistics.describe", json!({}));
    connect(&mut document, source, "series", labels, "input", None);
    connect(&mut document, labels, "output", describe, "source", None);
    let result = execute(&document, "yssbi.statistics.describe").unwrap();
    let summary = field(field(&result, "columns"), "codes");
    assert_eq!(
        field(summary, "semantic"),
        &TabularScalar::String("Ordinal".into()).into()
    );
    assert_eq!(number(field(summary, "missing")), 1.);
    assert_eq!(number(field(summary, "unique")), 2.);
    let categories = field(summary, "categories");
    for (position, expected, label, frequency) in [("1", high, "低", 2.), ("2", low, "高", 1.)] {
        let category = field(categories, position);
        let RuntimeValue::Scalar(code) = field(category, "value") else {
            panic!("category code")
        };
        assert_eq!(
            code.compare(&TabularScalar::Unsigned(expected)),
            Some(std::cmp::Ordering::Equal)
        );
        assert_eq!(
            field(category, "label"),
            &TabularScalar::String(label.into()).into()
        );
        assert_eq!(number(field(category, "frequency")), frequency);
        assert!((number(field(category, "proportion")) - frequency / 3.).abs() < 1e-12);
    }
}
