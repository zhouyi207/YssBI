use super::fixture::{columns, connect, node};
use super::*;
use serde_json::json;
#[test]
fn value_labels_preserve_lazy_series_codes_nulls_and_ordered_meanings() {
    let mut document = GraphDocument::default();
    let codes: Vec<_> = (0..640)
        .map(|i| {
            if i % 17 == 0 {
                None
            } else if i % 2 == 0 {
                Some("001")
            } else {
                Some("002")
            }
        })
        .collect();
    let source = columns(&mut document, &[("codes", json!(codes))])["codes"];
    let kind = "yssbi.dataframe.labels";
    let target = node(
        &mut document,
        kind,
        json!({"target_type":"core.ordinal","semantic_domain":{"values":[{"value":"002","label":"低"},{"value":"001","label":"高"}]}}),
    );
    connect(&mut document, source, "series", target, "input", None);
    let output = execute(&document, kind).unwrap();
    let RuntimeValue::Series(series) = &output else {
        panic!("lazy series")
    };
    let metadata = yss_database_arrow::column_semantic(series.plan().field()).unwrap();
    assert_eq!(metadata.kind, yss_data_contract::SemanticType::Ordinal);
    assert_eq!(
        metadata
            .values
            .iter()
            .map(|v| (v.value.as_str(), v.label.as_str()))
            .collect::<Vec<_>>(),
        [("002", "低"), ("001", "高")]
    );
    let RuntimeValue::List(values) = observed_values(output) else {
        panic!("values")
    };
    assert_eq!(values.len(), 640);
    assert_eq!(values[0], RuntimeValue::Scalar(TabularScalar::Null));
    assert_eq!(
        values[638],
        RuntimeValue::Scalar(TabularScalar::String("001".into()))
    );
    assert_eq!(
        values[639],
        RuntimeValue::Scalar(TabularScalar::String("002".into()))
    );
    // Changing only labels updates the plan's metadata and keeps the exact codes.
    document.nodes.get_mut(&target).unwrap().parameters.insert(
        "semantic_domain".parse().unwrap(),
        json!({"values":[{"value":"002","label":"低"},{"value":"001","label":"新版高"}]}),
    );
    let RuntimeValue::Series(series) = execute(&document, kind).unwrap() else {
        panic!("lazy series")
    };
    assert_eq!(
        yss_database_arrow::column_semantic(series.plan().field())
            .unwrap()
            .values[1]
            .label,
        "新版高"
    );
    document.nodes.get_mut(&target).unwrap().parameters.insert(
        "semantic_domain".parse().unwrap(),
        json!({"values":[{"value":"001","label":"重复1"},{"value":"001","label":"重复2"}]}),
    );
    let resources = ResourceCatalogSnapshot::new(BTreeMap::new(), BTreeMap::new());
    let graph = GraphResourcePath::new("events/New Event.yssbi-event").unwrap();
    let analysis = analyze_document(&document, &graph, &resources);
    assert!(
        analysis
            .semantic_snapshot()
            .diagnostics()
            .iter()
            .any(|d| d.code.as_str() == "graph.parameter.invalid")
    );
}
