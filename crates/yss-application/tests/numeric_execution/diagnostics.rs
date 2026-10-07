use super::fixture::{columns, connect, field, node, number};
use super::*;
use serde_json::{Value, json};

#[test]
fn diagnostics_category_executes_model_unions_and_pages_observation_outputs() {
    yss_application::session::NodeComponents::builtins().unwrap();
    let f: Value = serde_json::from_str(include_str!(
        "../../../yss-sci/tests/fixtures/diagnostics_reference.json"
    ))
    .unwrap();
    let survival: Value = serde_json::from_str(include_str!(
        "../../../yss-sci/tests/fixtures/survival_category_reference.json"
    ))
    .unwrap();
    let catalog = yss_node_catalog::build_builtin_node_system().unwrap();
    for method in [
        "collinearity",
        "harman",
        "nri_idi",
        "ph",
        "residual",
        "cooks_distance",
        "aic",
        "bic",
        "lr",
        "score_lm",
        "nested_comparison",
    ] {
        let kind = format!("yssbi.statistics.diagnostic.{method}");
        eprintln!("diagnostic: {method}");
        for locale in ["en-US", "zh-CN"] {
            let localized = catalog.catalog.localize(&catalog.registry, locale);
            let item = localized
                .items
                .iter()
                .find(|item| item.node_type_id.as_ref() == kind)
                .unwrap();
            assert_eq!(item.category_id.as_ref(), "statistics.diagnostics");
            assert!(
                item.documentation
                    .as_ref()
                    .is_some_and(|d| d.len() > 200 && !d.contains("范围待确认")),
                "{method}, {locale}"
            );
        }
        let mut document = GraphDocument::default();
        let target = node(&mut document, &kind, json!({}));
        match method {
            "collinearity" | "harman" => {
                let vars = columns(
                    &mut document,
                    &[
                        ("x1", f["x"][0].clone()),
                        ("x2", f["x"][1].clone()),
                        ("x3", f["x"][2].clone()),
                    ],
                );
                for (i, source) in vars.values().enumerate() {
                    connect(
                        &mut document,
                        *source,
                        "series",
                        target,
                        "variables",
                        Some(i),
                    );
                }
                let result = execute(&document, &kind).unwrap();
                assert_eq!(number(field(&result, "observations")), 640.0);
            }
            "nri_idi" => {
                let vars = columns(
                    &mut document,
                    &[
                        ("outcome", json!([true, true, false, false])),
                        ("reference", json!([0.3, 0.6, 0.6, 0.4])),
                        ("new", json!([0.5, 0.6, 0.3, 0.4])),
                    ],
                );
                for (key, source) in vars {
                    connect(&mut document, source, "series", target, &key, None);
                }
                for params in [
                    json!({}),
                    json!({"nri_mode":"categorical", "risk_thresholds":[0.5]}),
                ] {
                    document.nodes.get_mut(&target).unwrap().parameters =
                        serde_json::from_value(params).unwrap();
                    let result = execute(&document, &kind).unwrap();
                    assert_eq!(number(field(&result, "nri")), 1.0);
                }
            }
            "ph" => {
                let d = &survival["data"];
                let vars = columns(
                    &mut document,
                    &[
                        ("time", d["time"].clone()),
                        (
                            "event",
                            json!(
                                d["event"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .map(|v| v == 1)
                                    .collect::<Vec<_>>()
                            ),
                        ),
                        ("x1", d["predictors"][0].clone()),
                        ("x2", d["predictors"][1].clone()),
                    ],
                );
                for key in ["time", "event"] {
                    connect(&mut document, vars[key], "series", target, key, None);
                }
                for (i, key) in ["x1", "x2"].iter().enumerate() {
                    connect(&mut document, vars[*key], "series", target, "x", Some(i));
                }
                let result = execute(&document, &kind).unwrap();
                assert!(number(field(field(&result, "global"), "p_value")) >= 0.0);
            }
            _ => {
                let vars = columns(
                    &mut document,
                    &[
                        ("y", f["y"].clone()),
                        ("binary_y", f["binary_y"].clone()),
                        ("weights", f["weights"].clone()),
                        ("x1", f["x"][0].clone()),
                        ("x2", f["x"][1].clone()),
                        ("x3", f["x"][2].clone()),
                    ],
                );
                let binary_supported = !matches!(method, "residual" | "cooks_distance");
                let mut restricted = None;
                let full = node(&mut document, "yssbi.statistics.linear.fit", json!({}));
                if matches!(method, "lr" | "score_lm" | "nested_comparison") {
                    let model = node(&mut document, "yssbi.statistics.linear.fit", json!({}));
                    connect(&mut document, vars["x1"], "series", model, "x", Some(0));
                    connect(&mut document, model, "model", target, "restricted", None);
                    restricted = Some(model);
                }
                for (i, key) in ["x1", "x2", "x3"].iter().enumerate() {
                    connect(&mut document, vars[*key], "series", full, "x", Some(i));
                }
                connect(
                    &mut document,
                    full,
                    "model",
                    target,
                    if restricted.is_some() {
                        "full"
                    } else {
                        "model"
                    },
                    None,
                );
                for family in if binary_supported {
                    &["ols", "wls", "logit", "probit"][..]
                } else {
                    &["ols", "wls"][..]
                } {
                    eprintln!("model: {family}");
                    let mut variant = document.clone();
                    for model in [Some(full), restricted].into_iter().flatten() {
                        let binary = matches!(*family, "logit" | "probit");
                        let fit_kind = if binary {
                            format!("yssbi.statistics.{family}.fit")
                        } else {
                            "yssbi.statistics.linear.fit".to_string()
                        };
                        let fitted = variant.nodes.get_mut(&model).unwrap();
                        fitted.node_type = fit_kind.parse().unwrap();
                        fitted.parameters = serde_json::from_value(if *family == "wls" {
                            json!({"method":"WLS"})
                        } else {
                            json!({})
                        })
                        .unwrap();
                        connect(
                            &mut variant,
                            vars[if binary { "binary_y" } else { "y" }],
                            "series",
                            model,
                            "y",
                            None,
                        );
                        if *family == "wls" {
                            connect(
                                &mut variant,
                                vars["weights"],
                                "series",
                                model,
                                "weights",
                                Some(0),
                            );
                        }
                    }
                    let result = execute(&variant, &kind)
                        .unwrap_or_else(|e| panic!("{method}, {family}: {e:?}"));
                    if matches!(method, "aic" | "bic") {
                        assert_eq!(number(field(&result, "observations")), 640.0);
                        let group = if matches!(*family, "logit" | "probit") {
                            "binary"
                        } else {
                            "gaussian"
                        };
                        let expected = f[group][*family]["criteria"][method].as_f64().unwrap();
                        assert!((number(field(&result, "value")) - expected).abs() < 1e-6);
                    } else if matches!(method, "lr" | "score_lm" | "nested_comparison") {
                        assert_eq!(number(field(&result, "restrictions")), 2.0);
                    } else {
                        // A downstream table node consumes the derived schema and keeps every row.
                        let limit =
                            node(&mut variant, "yssbi.dataframe.limit", json!({"rows":640}));
                        connect(&mut variant, target, "observations", limit, "source", None);
                        let RuntimeValue::Relation(table) =
                            execute(&variant, "yssbi.dataframe.limit").unwrap()
                        else {
                            panic!("pageable diagnostics")
                        };
                        let control = yss_relational_contract::RelationControl {
                            cancellation: Arc::new(AtomicBool::new(false)),
                            deadline: Instant::now() + Duration::from_secs(10),
                            max_input_bytes: 4 * 1024 * 1024,
                        };
                        let page = table.page(635, 10, &control).unwrap();
                        assert_eq!(page.row_count, 5);
                        assert!(!page.has_more);
                        assert_eq!(page.data.columns().len(), 8);
                        assert_eq!(
                            serde_json::to_value(page.data.columns()[0].values()).unwrap(),
                            json!([636.0, 637.0, 638.0, 639.0, 640.0])
                        );
                        let expected = f["gaussian"][*family]["cooks"][635].as_f64().unwrap();
                        let actual = serde_json::to_value(page.data.columns()[7].values()).unwrap()
                            [0]
                        .as_f64()
                        .unwrap();
                        assert!((actual - expected).abs() < 1e-8);
                    }
                }
            }
        }
    }
}
