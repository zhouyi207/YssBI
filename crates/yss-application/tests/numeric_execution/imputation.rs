use super::fixture::{columns, connect, node};
use super::*;
use serde_json::json;

#[test]
fn single_imputation_node_preserves_alignment_and_executes_every_method_above_512_rows() {
    for (method, expected) in [
        ("mean", 7. / 3.),
        ("median", 2.),
        ("mode", 1.),
        ("constant", 9.),
    ] {
        let mut document = GraphDocument::default();
        let kind = "yssbi.dataframe.impute.single";
        let target = node(
            &mut document,
            kind,
            json!({"imputation_method":method,"fill_value":9.}),
        );
        let sources = columns(
            &mut document,
            &[(
                "x",
                json!(
                    (0..640)
                        .map(|i| [None, Some(1), Some(2), Some(4)][i % 4])
                        .collect::<Vec<_>>()
                ),
            )],
        );
        connect(
            &mut document,
            sources["x"],
            "series",
            target,
            "series",
            None,
        );
        let RuntimeValue::Series(series) = execute(&document, kind).unwrap() else {
            panic!("series")
        };
        let control = yss_relational_contract::RelationControl {
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(10),
            max_input_bytes: 4 * 1024 * 1024,
        };
        let table = series.as_relation().unwrap();
        let page = table.page(636, 10, &control).unwrap();
        let values = serde_json::to_value(page.data.columns()[0].values()).unwrap();
        assert_eq!(values.as_array().unwrap().len(), 4);
        assert!((values[0].as_f64().unwrap() - expected).abs() < 1e-12);
        assert_eq!(values[3].as_f64().unwrap(), 4.);
    }
}
