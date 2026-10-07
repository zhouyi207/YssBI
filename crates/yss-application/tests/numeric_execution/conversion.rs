use super::fixture::{columns, connect, node};
use super::*;
use serde_json::json;

#[test]
fn categorical_conversion_discovers_all_series_values_and_keeps_row_alignment() {
    let mut document = GraphDocument::default();
    let codes: Vec<_> = (0..20_000)
        .map(|i| match i {
            19_999 => Some("001"),
            19_998 => Some(""),
            i if i % 17 == 0 => None,
            _ => Some("002"),
        })
        .collect();
    let source = columns(&mut document, &[("codes", json!(codes))])["codes"];
    const KIND: &str = "yssbi.value.to_categorical";
    let target = node(&mut document, KIND, json!({}));
    connect(&mut document, source, "series", target, "input", None);
    let output = execute(&document, KIND).unwrap();
    let RuntimeValue::Series(series) = &output else {
        panic!("lazy series")
    };
    let metadata = yss_database_arrow::column_semantic(series.plan().field()).unwrap();
    assert_eq!(metadata.kind, yss_data_contract::SemanticType::Categorical);
    assert_eq!(
        metadata
            .values
            .iter()
            .map(|value| (value.value.as_str(), value.label.as_str()))
            .collect::<Vec<_>>(),
        [("", ""), ("001", "001"), ("002", "002")],
    );
    let page = series
        .as_relation()
        .unwrap()
        .page(
            0,
            codes.len(),
            &yss_relational_contract::RelationControl {
                cancellation: Arc::new(AtomicBool::new(false)),
                deadline: Instant::now() + Duration::from_secs(10),
                max_input_bytes: 16 * 1024 * 1024,
            },
        )
        .unwrap();
    let values = page.data.columns()[0].values();
    let expected: Vec<_> = codes
        .into_iter()
        .map(|value| {
            value.map_or(TabularScalar::Null, |value| {
                TabularScalar::String(value.into())
            })
        })
        .collect();
    assert_eq!(values.len(), expected.len());
    for (row, (actual, expected)) in values.iter().zip(&expected).enumerate() {
        assert_eq!(actual, expected, "page row {row}");
    }
    let RuntimeValue::List(streamed) = observed_values(output) else {
        panic!("streamed values")
    };
    assert_eq!(streamed.len(), expected.len());
    for (row, (actual, expected)) in streamed.iter().zip(expected).enumerate() {
        assert_eq!(*actual, RuntimeValue::Scalar(expected), "stream row {row}");
    }
    assert!(document.nodes[&target].parameters.is_empty());
}
