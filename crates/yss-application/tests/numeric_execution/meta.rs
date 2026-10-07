use super::fixture::{columns, connect, field, node, number};
use super::*;
use serde_json::{Value, json};

#[test]
fn meta_category_executes_every_node_and_reuses_study_tables() {
    yss_application::session::NodeComponents::builtins().unwrap();
    let f: Value = serde_json::from_str(include_str!(
        "../../../yss-sci/tests/fixtures/meta_reference.json"
    ))
    .unwrap();
    let catalog = yss_node_catalog::build_builtin_node_system().unwrap();
    for method in [
        "continuous",
        "binary",
        "single_proportion",
        "mean",
        "correlation",
        "or_hr",
        "combine_p",
        "inverse_variance",
        "fixed_effect",
        "random_effect",
        "cochran_q",
        "i_squared",
        "tau_squared",
        "regression",
        "egger",
        "begg",
        "leave_one_out",
        "sensitivity",
        "forest",
        "funnel",
    ] {
        let family = if matches!(method, "forest" | "funnel") {
            "plot"
        } else {
            "meta"
        };
        let kind = format!("yssbi.statistics.{family}.{method}");
        for locale in ["en-US", "zh-CN"] {
            let localized = catalog.catalog.localize(&catalog.registry, locale);
            let item = localized
                .items
                .iter()
                .find(|item| item.node_type_id.as_ref() == kind)
                .unwrap();
            assert_eq!(item.category_id.as_ref(), "statistics.meta");
            assert!(
                item.documentation
                    .as_ref()
                    .is_some_and(|d| d.len() > 200 && !d.contains("范围待确认"))
            );
        }
        let mut document = GraphDocument::default();
        let target = node(&mut document, &kind, json!({}));
        let pairs: Vec<(&str, Value)> = match method {
            "continuous" => [
                "treatment_mean",
                "treatment_sd",
                "treatment_n",
                "reference_mean",
                "reference_sd",
                "reference_n",
            ]
            .into_iter()
            .enumerate()
            .map(|(i, key)| (key, f["continuous"]["inputs"][i].clone()))
            .collect(),
            "binary" => [
                "treatment_events",
                "treatment_n",
                "reference_events",
                "reference_n",
            ]
            .into_iter()
            .enumerate()
            .map(|(i, key)| (key, f["binary_inputs"][i].clone()))
            .collect(),
            "single_proportion" => vec![
                ("events", f["proportion"]["events"].clone()),
                ("sample_size", f["proportion"]["totals"].clone()),
            ],
            "mean" => vec![
                ("mean", f["continuous"]["inputs"][0].clone()),
                ("sd", f["continuous"]["inputs"][1].clone()),
                ("sample_size", f["continuous"]["inputs"][2].clone()),
            ],
            "correlation" => vec![
                ("correlation", json!([0.2, 0.4, 0.6])),
                ("sample_size", json!([20, 30, 40])),
            ],
            "or_hr" => vec![
                ("ratio", json!([1.5, 2., 1.8])),
                ("lower", json!([1., 1.1, 1.2])),
                ("upper", json!([2.25, 3.63636363636, 2.7])),
            ],
            "combine_p" => vec![
                ("p_values", f["p_values"].clone()),
                ("weights", f["p_weights"].clone()),
            ],
            "regression" => vec![
                ("effects", f["y"].clone()),
                ("variances", f["variances"].clone()),
                ("moderators", f["moderator"].clone()),
            ],
            _ => vec![
                ("effects", f["y"].clone()),
                ("variances", f["variances"].clone()),
            ],
        };
        for (key, source) in columns(&mut document, &pairs) {
            connect(
                &mut document,
                source,
                "series",
                target,
                &key,
                matches!(key.as_str(), "moderators" | "weights").then_some(0),
            );
        }
        let result = execute(&document, &kind).unwrap_or_else(|e| panic!("{method}: {e:?}"));
        if method == "combine_p" {
            assert!(
                (number(field(&result, "p_value")) - f["fisher"][1].as_f64().unwrap()).abs()
                    < 1e-10
            );
            document.nodes.get_mut(&target).unwrap().parameters =
                serde_json::from_value(json!({"p_method":"stouffer"})).unwrap();
            let result = execute(&document, &kind).unwrap();
            assert!(
                (number(field(&result, "p_value")) - f["stouffer"][1].as_f64().unwrap()).abs()
                    < 1e-10
            );
        }
        let output = match method {
            "continuous" | "binary" | "single_proportion" | "mean" | "correlation" | "or_hr" => {
                Some(("effect", pairs[0].1.as_array().unwrap().len(), 6))
            }
            "inverse_variance" | "fixed_effect" | "random_effect" | "regression" => {
                Some(("effect", 8, 9))
            }
            "leave_one_out" | "sensitivity" => Some(("estimate", 8, 7)),
            _ => None,
        };
        if let Some((column, rows, width)) = output {
            let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":1000}));
            connect(&mut document, target, "studies", limit, "source", None);
            let RuntimeValue::Relation(table) =
                execute(&document, "yssbi.dataframe.limit").unwrap()
            else {
                panic!("study relation")
            };
            let page = table.page(0, 1000, &relation_control()).unwrap();
            assert_eq!(page.row_count, rows);
            assert_eq!(page.data.columns().len(), width);
            let select = node(
                &mut document,
                "yssbi.dataframe.series.select",
                json!({"column":column}),
            );
            connect(&mut document, limit, "result", select, "dataframe", None);
            // Verify derived columns resolve before execution, including their downstream type.
            let resources = ResourceCatalogSnapshot::new(BTreeMap::new(), BTreeMap::new());
            let analysis = analyze_document(
                &document,
                &GraphResourcePath::new("events/meta.yssbi-event").unwrap(),
                &resources,
            );
            assert!(
                analysis.semantic_snapshot().diagnostics().is_empty(),
                "{:?}",
                analysis.semantic_snapshot().diagnostics()
            );
        }
    }
}

fn relation_control() -> yss_relational_contract::RelationControl {
    yss_relational_contract::RelationControl {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(10),
        max_input_bytes: 4 * 1024 * 1024,
    }
}

#[test]
fn meta_effect_table_feeds_pooling_and_preserves_more_than_512_studies() {
    let mut document = GraphDocument::default();
    let prep = node(&mut document, "yssbi.statistics.meta.mean", json!({}));
    let means = (0..640)
        .map(|i| 1. + (i % 9) as f64 / 10.)
        .collect::<Vec<_>>();
    for (key, source) in columns(
        &mut document,
        &[
            ("mean", json!(means)),
            ("sd", json!(vec![2.; 640])),
            ("sample_size", json!(vec![50; 640])),
        ],
    ) {
        connect(&mut document, source, "series", prep, &key, None);
    }
    let pooled = node(
        &mut document,
        "yssbi.statistics.meta.fixed_effect",
        json!({}),
    );
    for (column, input) in [("effect", "effects"), ("variance", "variances")] {
        let select = node(
            &mut document,
            "yssbi.dataframe.series.select",
            json!({"column":column}),
        );
        connect(&mut document, prep, "studies", select, "dataframe", None);
        connect(&mut document, select, "series", pooled, input, None);
    }
    let result = execute(&document, "yssbi.statistics.meta.fixed_effect").unwrap();
    assert_eq!(number(field(&result, "studies")), 640.);
    let RuntimeValue::List(coefficients) = field(&result, "coefficients") else {
        panic!("coefficients")
    };
    assert!(
        (number(field(&coefficients[0], "estimate")) - means.iter().sum::<f64>() / 640.).abs()
            < 1e-9
    );
    let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":1000}));
    connect(&mut document, pooled, "studies", limit, "source", None);
    let RuntimeValue::Relation(table) = execute(&document, "yssbi.dataframe.limit").unwrap() else {
        panic!("pooled study relation")
    };
    let page = table.page(635, 10, &relation_control()).unwrap();
    assert_eq!(page.row_count, 5);
    assert!(!page.has_more);
    assert_eq!(
        serde_json::to_value(page.data.columns()[0].values()).unwrap(),
        json!([636., 637., 638., 639., 640.])
    );
}
