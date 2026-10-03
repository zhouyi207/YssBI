use super::fixture::{columns, connect, field, node, number};
use super::*;
use serde_json::{Value, json};

#[test]
fn cluster_and_adjusted_prediction_nodes_use_real_fits_and_canonical_parameters() {
    let f: Value = serde_json::from_str(include_str!(
        "../../../yss-sci/tests/fixtures/postestimation_reference.json"
    ))
    .unwrap();
    for family in ["cluster", "linear", "logit", "probit"] {
        let mut document = GraphDocument::default();
        let kind = if family == "cluster" {
            "yssbi.statistics.inference.cluster_robust".to_string()
        } else {
            format!("yssbi.statistics.{family}.fit")
        };
        let fit = node(&mut document, &kind, json!({}));
        let labels = f["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| format!("cluster {}", v.as_u64().unwrap()))
            .collect::<Vec<_>>();
        let sources = columns(
            &mut document,
            &[
                (
                    "response",
                    if family == "logit" || family == "probit" {
                        f["binary"].clone()
                    } else {
                        f["response"].clone()
                    },
                ),
                ("x1", f["predictors"][0].clone()),
                ("x2", f["predictors"][1].clone()),
                ("clusters", json!(labels)),
            ],
        );
        connect(
            &mut document,
            sources["response"],
            "series",
            fit,
            "response",
            None,
        );
        for (j, name) in ["x1", "x2"].into_iter().enumerate() {
            connect(
                &mut document,
                sources[name],
                "series",
                fit,
                "predictors",
                Some(j),
            );
        }
        if family == "cluster" {
            connect(
                &mut document,
                sources["clusters"],
                "series",
                fit,
                "clusters",
                None,
            );
            let report = execute(&document, &kind).unwrap();
            assert_eq!(number(field(&report, "degrees_of_freedom")), 19.);
            let RuntimeValue::List(coefficients) = field(&report, "coefficients") else {
                panic!("coefficients")
            };
            assert!(
                (number(field(&coefficients[1], "p_value"))
                    - f["cluster"]["p"][1].as_f64().unwrap())
                .abs()
                    < 1e-8
            );
        } else {
            let target_kind = "yssbi.statistics.postestimation.adjusted_predictions";
            let target = node(
                &mut document,
                target_kind,
                json!({"confidence_level":0.9,"evaluation":"average"}),
            );
            connect(&mut document, fit, "model", target, "model", None);
            let report = execute(&document, target_kind).unwrap();
            let expected = f["predictions"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| {
                    c["family"] == family && c["evaluation"] == "average" && c["override"] == false
                })
                .unwrap();
            assert!(
                (number(field(&report, "estimate")) - expected["estimate"].as_f64().unwrap()).abs()
                    < 1e-7
            );
            assert!(
                (number(field(&report, "standard_error")) - expected["se"].as_f64().unwrap()).abs()
                    < 1e-7
            );
        }
    }
}

#[test]
fn inference_tables_execute_with_labels_variants_and_more_than_512_rows() {
    yss_application::session::NodeComponents::builtins().unwrap();
    let f: Value = serde_json::from_str(include_str!(
        "../../../yss-sci/tests/fixtures/inference_reference.json"
    ))
    .unwrap();
    for method in [
        "inference.confidence_interval",
        "posthoc.multiple_comparisons",
    ] {
        let interval = method.starts_with("inference.");
        let kind = format!("yssbi.statistics.{method}");
        let mut document = GraphDocument::default();
        let target = node(
            &mut document,
            &kind,
            if interval {
                json!({"degrees_of_freedom":12.5})
            } else {
                json!({"confidence_level":0.9})
            },
        );
        let values = if interval {
            vec![
                (
                    "estimates",
                    json!((0..640).map(|i| i as f64 / 10.).collect::<Vec<_>>()),
                ),
                ("standard_errors", json!(vec![0.5; 640])),
            ]
        } else {
            let labels = f["groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(|g| ["A", "B", "C"][g.as_u64().unwrap() as usize])
                .collect::<Vec<_>>();
            vec![("response", f["y"].clone()), ("groups", json!(labels))]
        };
        for (key, source) in columns(&mut document, &values) {
            connect(&mut document, source, "series", target, &key, None);
        }
        let report = execute(&document, &kind).unwrap();
        if interval {
            assert_eq!(number(field(&report, "rows")), 640.);
        } else {
            let RuntimeValue::List(labels) = field(&report, "group_labels") else {
                panic!("labels")
            };
            assert_eq!(
                labels[0],
                RuntimeValue::from(TabularScalar::String("A".into()))
            );
            assert_eq!(number(field(&report, "comparisons")), 3.);
        }
        let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":1000}));
        connect(
            &mut document,
            target,
            if interval { "intervals" } else { "comparisons" },
            limit,
            "source",
            None,
        );
        let RuntimeValue::Relation(table) = execute(&document, "yssbi.dataframe.limit").unwrap()
        else {
            panic!("inference table")
        };
        let control = yss_relational_contract::RelationControl {
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(10),
            max_input_bytes: 4 * 1024 * 1024,
        };
        let page = table
            .page(if interval { 639 } else { 0 }, 10, &control)
            .unwrap();
        assert_eq!(page.data.columns().len(), if interval { 5 } else { 10 });
        let index = if interval { 4 } else { 7 };
        let actual = serde_json::to_value(page.data.columns()[index].values()).unwrap()[0]
            .as_f64()
            .unwrap();
        let expected = if interval {
            63.9 + 0.5 * f["t_q"].as_f64().unwrap()
        } else {
            f["cases"][1]["rows"][0]["adjusted_p"].as_f64().unwrap()
        };
        assert!((actual - expected).abs() < 1e-9);
        if !interval {
            document.nodes.get_mut(&target).unwrap().parameters = serde_json::from_value(
                json!({"equal_variances":false,"adjustment":"bonferroni","confidence_level":0.9}),
            )
            .unwrap();
            let RuntimeValue::Relation(table) =
                execute(&document, "yssbi.dataframe.limit").unwrap()
            else {
                panic!("Welch table")
            };
            let page = table.page(0, 10, &control).unwrap();
            let actual = serde_json::to_value(page.data.columns()[9].values()).unwrap()[0]
                .as_f64()
                .unwrap();
            assert!((actual - f["cases"][5]["rows"][0]["upper"].as_f64().unwrap()).abs() < 1e-9);
        }
    }
}
