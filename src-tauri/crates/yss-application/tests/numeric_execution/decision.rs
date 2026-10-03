use super::fixture::{columns, connect, field, node, number};
use super::*;
use serde_json::{Value, json};
#[path = "decision/conjoint.rs"]
mod conjoint;
#[path = "decision/experts.rs"]
mod experts;
#[path = "decision/matrices.rs"]
mod matrices;

#[test]
fn market_nodes_route_parameters_and_price_curve_schemas() {
    let mut document = GraphDocument::default();
    let kind = "yssbi.statistics.decision.turf";
    let target = node(&mut document, kind, json!({"combination_size":2}));
    let inputs = columns(
        &mut document,
        &[
            ("a", json!([1, 1, 1, 1, 0, 0])),
            ("b", json!([1, 1, 0, 0, 1, 0])),
            ("c", json!([0, 0, 1, 1, 0, 1])),
        ],
    );
    for (j, key) in ["a", "b", "c"].into_iter().enumerate() {
        connect(
            &mut document,
            inputs[key],
            "series",
            target,
            "criteria",
            Some(j),
        );
    }
    let report = execute(&document, kind).unwrap();
    assert_eq!(number(field(&report, "reach_percent")), 100.);
    let RuntimeValue::List(selected) = field(&report, "selected_criteria") else {
        panic!("selection")
    };
    assert_eq!(selected.iter().map(number).collect::<Vec<_>>(), [2., 3.]);

    for (definition, lower) in [("original", 3.), ("narrower", 4.)] {
        let mut document = GraphDocument::default();
        let kind = "yssbi.statistics.decision.psm";
        let target = node(&mut document, kind, json!({"range_definition":definition}));
        let data = ["too_cheap", "cheap", "expensive", "too_expensive"]
            .into_iter()
            .enumerate()
            .map(|(j, key)| {
                (
                    key,
                    json!(
                        (0..4)
                            .map(|i| 1 + 2 * j + i)
                            .cycle()
                            .take(640)
                            .collect::<Vec<_>>()
                    ),
                )
            })
            .collect::<Vec<_>>();
        let inputs = columns(&mut document, &data);
        for (key, _) in data {
            connect(&mut document, inputs[key], "series", target, key, None);
        }
        let report = execute(&document, kind).unwrap();
        assert_eq!(number(field(&report, "observations")), 640.);
        assert_eq!(
            number(field(field(&report, "marginal_cheapness"), "lower")),
            lower
        );
        let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":1000}));
        connect(&mut document, target, "curves", limit, "source", None);
        let RuntimeValue::Relation(table) = execute(&document, "yssbi.dataframe.limit").unwrap()
        else {
            panic!("price curves")
        };
        let control = yss_relational_contract::RelationControl {
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(10),
            max_input_bytes: 4 * 1024 * 1024,
        };
        let page = table.page(9, 4, &control).unwrap();
        assert_eq!(page.data.columns().len(), 7);
        assert_eq!(
            page.data.columns()[0].values(),
            &[TabularScalar::Float64(10_f64.try_into().unwrap())]
        );
    }
}

#[test]
fn preference_nodes_execute_full_samples_and_page_customer_scores() {
    for (method, data) in [
        (
            "nps",
            vec![(
                "ratings",
                json!(
                    [0., 7., 9., 10.]
                        .into_iter()
                        .cycle()
                        .take(640)
                        .collect::<Vec<_>>()
                ),
            )],
        ),
        (
            "kano",
            vec![
                (
                    "functional",
                    json!(
                        [1., 1., 2., 2.]
                            .into_iter()
                            .cycle()
                            .take(640)
                            .collect::<Vec<_>>()
                    ),
                ),
                (
                    "dysfunctional",
                    json!(
                        [2., 5., 5., 2.]
                            .into_iter()
                            .cycle()
                            .take(640)
                            .collect::<Vec<_>>()
                    ),
                ),
            ],
        ),
        (
            "rfm",
            ["recency", "frequency", "monetary"]
                .into_iter()
                .map(|key| (key, json!((0..640).collect::<Vec<_>>())))
                .collect(),
        ),
    ] {
        let mut document = GraphDocument::default();
        let kind = format!("yssbi.statistics.decision.{method}");
        let target = node(&mut document, &kind, json!({}));
        let sources = columns(&mut document, &data);
        for &(key, _) in &data {
            connect(&mut document, sources[key], "series", target, key, None);
        }
        let report = execute(&document, &kind).unwrap();
        assert_eq!(number(field(&report, "observations")), 640.);
        match method {
            "nps" => assert_eq!(number(field(&report, "net_promoter_score")), 25.),
            "kano" => {
                assert_eq!(number(field(&report, "better")), 0.5);
                assert_eq!(number(field(&report, "worse")), -0.5);
            }
            _ => {
                let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":1000}));
                connect(&mut document, target, "scores", limit, "source", None);
                let RuntimeValue::Relation(table) =
                    execute(&document, "yssbi.dataframe.limit").unwrap()
                else {
                    panic!("RFM table")
                };
                let control = yss_relational_contract::RelationControl {
                    cancellation: Arc::new(AtomicBool::new(false)),
                    deadline: Instant::now() + Duration::from_secs(10),
                    max_input_bytes: 4 * 1024 * 1024,
                };
                let page = table.page(639, 5, &control).unwrap();
                assert_eq!(
                    page.data.columns()[0].values(),
                    &[TabularScalar::Float64(640_f64.try_into().unwrap())]
                );
                assert_eq!(
                    page.data.columns()[4].values(),
                    &[TabularScalar::Float64(11_f64.try_into().unwrap())]
                );
            }
        }
    }
}

#[test]
fn decision_system_nodes_preserve_compromise_sets_and_nullable_table_cells() {
    for method in ["vikor", "coupling_coordination", "obstacle_degree"] {
        let mut document = GraphDocument::default();
        let kind = format!("yssbi.statistics.decision.{method}");
        let target = node(
            &mut document,
            &kind,
            if method == "obstacle_degree" {
                json!({"rescale":false})
            } else {
                json!({})
            },
        );
        let sources = columns(
            &mut document,
            &[("a", json!([0., 0.5, 1.])), ("b", json!([0., 0.5, 1.]))],
        );
        for (j, name) in ["a", "b"].into_iter().enumerate() {
            connect(
                &mut document,
                sources[name],
                "series",
                target,
                "criteria",
                Some(j),
            );
        }
        let report = execute(&document, &kind).unwrap();
        if method == "vikor" {
            let RuntimeValue::List(set) = field(&report, "compromise_alternatives") else {
                panic!("compromise")
            };
            assert_eq!(set.len(), 1);
            assert_eq!(number(&set[0]), 3.);
        } else {
            assert_eq!(number(field(&report, "undefined_rows")), 1.);
        }
        let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":1000}));
        connect(&mut document, target, "scores", limit, "source", None);
        let RuntimeValue::Relation(table) = execute(&document, "yssbi.dataframe.limit").unwrap()
        else {
            panic!("system table")
        };
        let control = yss_relational_contract::RelationControl {
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(10),
            max_input_bytes: 4 * 1024 * 1024,
        };
        let page = table.page(0, 10, &control).unwrap();
        if method == "coupling_coordination" {
            assert_eq!(page.data.columns()[1].values()[0], TabularScalar::Null);
        }
        if method == "obstacle_degree" {
            assert_eq!(page.data.columns()[4].values()[4], TabularScalar::Null);
        }
    }
}

#[test]
fn decision_nodes_execute_all_rankings_and_reuse_generated_weights_on_independent_domains() {
    yss_application::session::NodeComponents::builtins().unwrap();
    let f: Value = serde_json::from_str(include_str!(
        "../../../yss-sci/tests/fixtures/decision_reference.json"
    ))
    .unwrap();
    let data = f["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|col| {
            col.as_array()
                .unwrap()
                .iter()
                .cycle()
                .take(640)
                .cloned()
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let control = yss_relational_contract::RelationControl {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(10),
        max_input_bytes: 4 * 1024 * 1024,
    };
    for case in f["cases"].as_array().unwrap() {
        let method = case["method"].as_str().unwrap();
        let kind = format!(
            "yssbi.statistics.{}.{}",
            if method == "entropy_topsis" {
                "workflow"
            } else {
                "decision"
            },
            method
        );
        let mut parameters = json!({"cost_criteria":[2]});
        let explicit = case["weighting"] == "explicit";
        if explicit {
            parameters["weight_method"] = json!("explicit");
        }
        let mut document = GraphDocument::default();
        let target = node(&mut document, &kind, parameters);
        let inputs = columns(
            &mut document,
            &[
                ("criterion1", json!(data[0])),
                ("criterion2", json!(data[1])),
                ("criterion3", json!(data[2])),
            ],
        );
        for (j, key) in ["criterion1", "criterion2", "criterion3"]
            .into_iter()
            .enumerate()
        {
            connect(
                &mut document,
                inputs[key],
                "series",
                target,
                "criteria",
                Some(j),
            );
        }
        if explicit {
            let weights = columns(&mut document, &[("weights", json!([0.5, 0.3, 0.2]))]);
            connect(
                &mut document,
                weights["weights"],
                "series",
                target,
                "criterion_weights",
                Some(0),
            );
        }
        let report = execute(&document, &kind).unwrap();
        assert_eq!(number(field(&report, "observations")), 640.);
        let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":1000}));
        connect(&mut document, target, "scores", limit, "source", None);
        let RuntimeValue::Relation(scores) = execute(&document, "yssbi.dataframe.limit").unwrap()
        else {
            panic!("scores relation")
        };
        let page = scores.page(632, 10, &control).unwrap();
        let values = serde_json::to_value(page.data.columns()[1].values()).unwrap();
        for (j, score) in values.as_array().unwrap().iter().enumerate() {
            assert!(
                (score.as_f64().unwrap() - case["scores_640_tail"][j].as_f64().unwrap()).abs()
                    < 1e-9,
                "{method}"
            );
        }
        if method == "entropy_weight" {
            let consumer_kind = "yssbi.statistics.decision.topsis";
            let consumer = node(
                &mut document,
                consumer_kind,
                json!({"weight_method":"explicit","cost_criteria":[2]}),
            );
            for (j, key) in ["criterion1", "criterion2", "criterion3"]
                .into_iter()
                .enumerate()
            {
                connect(
                    &mut document,
                    inputs[key],
                    "series",
                    consumer,
                    "criteria",
                    Some(j),
                );
            }
            connect(
                &mut document,
                target,
                "weights",
                consumer,
                "criterion_weights",
                Some(0),
            );
            let report = execute(&document, consumer_kind).unwrap();
            let RuntimeValue::List(actual) = field(field(&report, "weighting"), "weights") else {
                panic!("weight vector")
            };
            for (j, value) in actual.iter().enumerate() {
                assert!(
                    (number(value) - f["weights"]["entropy"][j].as_f64().unwrap()).abs() < 1e-10
                );
            }
        }
    }
}
