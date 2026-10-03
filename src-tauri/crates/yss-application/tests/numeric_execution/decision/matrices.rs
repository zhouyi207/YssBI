use super::*;
#[test]
fn matrix_nodes_preserve_orientation_and_fixed_relation_outputs() {
    let f: Value = serde_json::from_str(include_str!(
        "../../../../yss-sci/tests/fixtures/decision_matrices_reference.json"
    ))
    .unwrap();
    let control = yss_relational_contract::RelationControl {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(10),
        max_input_bytes: 4 * 1024 * 1024,
    };
    for method in ["ahp", "fahp", "dematel", "ism"] {
        let mut document = GraphDocument::default();
        let kind = format!("yssbi.statistics.decision.{method}");
        let target = node(&mut document, &kind, json!({}));
        let values = if method == "ism" {
            json!([[0, 0, 0], [1, 0, 0], [0, 1, 0]])
        } else {
            f[method]["columns"].clone()
        };
        let names = (0..values.as_array().unwrap().len())
            .map(|i| format!("criterion{}", i + 1))
            .collect::<Vec<_>>();
        let data = names
            .iter()
            .zip(values.as_array().unwrap())
            .map(|(name, col)| (name.as_str(), col.clone()))
            .collect::<Vec<_>>();
        let inputs = columns(&mut document, &data);
        for (i, (name, _)) in data.iter().enumerate() {
            connect(
                &mut document,
                inputs[*name],
                "series",
                target,
                "criteria",
                Some(i),
            );
        }
        let report = execute(&document, &kind).unwrap();
        assert_eq!(number(field(&report, "criteria")), names.len() as f64);
        if matches!(method, "ahp" | "fahp") {
            let RuntimeValue::List(weights) = field(&report, "weights") else {
                panic!("weights")
            };
            for (i, w) in weights.iter().enumerate() {
                assert!((number(w) - f[method]["weights"][i].as_f64().unwrap()).abs() < 1e-10);
            }
        } else {
            let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":1000}));
            connect(&mut document, target, "relations", limit, "source", None);
            let RuntimeValue::Relation(table) =
                execute(&document, "yssbi.dataframe.limit").unwrap()
            else {
                panic!("matrix")
            };
            let page = table.page(2, 1, &control).unwrap();
            if method == "ism" {
                assert_eq!(
                    page.data.columns()[2].values(),
                    &[TabularScalar::Float64(1_f64.try_into().unwrap())]
                );
            } else {
                let cell = serde_json::to_value(&page.data.columns()[3].values()[0])
                    .unwrap()
                    .as_f64()
                    .unwrap();
                assert!((cell - f[method]["total"][0][2].as_f64().unwrap()).abs() < 1e-10);
            }
        }
    }
    let mut document = GraphDocument::default();
    let kind = "yssbi.statistics.decision.fuzzy_evaluation";
    let target = node(&mut document, kind, json!({}));
    let inputs = columns(
        &mut document,
        &[("grade1", json!([2, 6])), ("grade2", json!([8, 4]))],
    );
    for (i, name) in ["grade1", "grade2"].into_iter().enumerate() {
        connect(
            &mut document,
            inputs[name],
            "series",
            target,
            "memberships",
            Some(i),
        );
    }
    let weights = columns(&mut document, &[("weights", json!([1, 3]))]);
    connect(
        &mut document,
        weights["weights"],
        "series",
        target,
        "criterion_weights",
        Some(0),
    );
    let scores = columns(&mut document, &[("scores", json!([1, 5]))]);
    connect(
        &mut document,
        scores["scores"],
        "series",
        target,
        "grade_scores",
        Some(0),
    );
    let report = execute(&document, kind).unwrap();
    assert!((number(field(&report, "score")) - 3.).abs() < 1e-12);
}
