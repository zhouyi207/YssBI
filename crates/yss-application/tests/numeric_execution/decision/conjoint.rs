use super::*;
#[test]
fn conjoint_preserves_text_levels_and_all_prediction_rows() {
    let mut document = GraphDocument::default();
    let kind = "yssbi.statistics.decision.conjoint";
    let target = node(&mut document, kind, json!({}));
    let inputs = columns(
        &mut document,
        &[
            (
                "rating",
                json!(
                    (0..640)
                        .map(|i| 3. + 2. * (i % 2) as f64)
                        .collect::<Vec<_>>()
                ),
            ),
            (
                "brand",
                json!(
                    (0..640)
                        .map(|i| if i % 2 == 0 { "品牌 A" } else { "品牌 B" })
                        .collect::<Vec<_>>()
                ),
            ),
        ],
    );
    connect(
        &mut document,
        inputs["rating"],
        "series",
        target,
        "ratings",
        None,
    );
    connect(
        &mut document,
        inputs["brand"],
        "series",
        target,
        "factors",
        Some(0),
    );
    let report = execute(&document, kind).unwrap();
    assert_eq!(number(field(&report, "observations")), 640.);
    let RuntimeValue::List(labels) = field(&report, "level_labels") else {
        panic!("levels")
    };
    let RuntimeValue::List(levels) = &labels[0] else {
        panic!("levels")
    };
    // Serialized scalar labels retain their original values rather than dummy indices.
    let text = format!("{levels:?}");
    assert!(text.contains("品牌 A") && text.contains("品牌 B"));
    let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":1000}));
    connect(&mut document, target, "predictions", limit, "source", None);
    let RuntimeValue::Relation(table) = execute(&document, "yssbi.dataframe.limit").unwrap() else {
        panic!("predictions")
    };
    let c = yss_relational_contract::RelationControl {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(10),
        max_input_bytes: 4 * 1024 * 1024,
    };
    let page = table.page(639, 1, &c).unwrap();
    assert_eq!(page.data.columns().len(), 4);
    assert_eq!(
        page.data.columns()[0].values(),
        &[TabularScalar::Float64(640_f64.try_into().unwrap())]
    );
    let TabularScalar::Float64(prediction) = &page.data.columns()[2].values()[0] else {
        panic!("prediction")
    };
    assert!((prediction.as_f64() - 5.).abs() < 1e-10);
}
