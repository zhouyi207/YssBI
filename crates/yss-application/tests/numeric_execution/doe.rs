use super::fixture::{columns, connect, field, node, number};
use super::*;
use serde_json::json;
#[test]
fn designed_response_models_retain_all_observed_and_fitted_rows() {
    for surface in [true, false] {
        let kind = if surface {
            "yssbi.statistics.doe.response_surface"
        } else {
            "yssbi.statistics.doe.dose_response"
        };
        let x: Vec<_> = (0..640)
            .map(|i| {
                if surface {
                    (i % 16) as f64 - 7.5
                } else if i % 32 == 0 {
                    0.
                } else {
                    ((i % 32) as f64 / 4. - 4.).exp()
                }
            })
            .collect();
        let y: Vec<_> = x
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                if surface {
                    5. + 2. * v - 0.3 * v * v + 0.03 * ((i * 7 % 17) as f64 - 8.)
                } else {
                    1.2 + 8.4 / (1. + (v / 2.5).powf(1.4)) + 0.03 * ((i * 17 % 19) as f64 - 9.)
                }
            })
            .collect();
        let mut document = GraphDocument::default();
        let target = node(
            &mut document,
            kind,
            if surface {
                json!({})
            } else {
                json!({"max_iterations":500,"tolerance":0.0000001})
            },
        );
        let inputs = columns(&mut document, &[("y", json!(y)), ("x", json!(x))]);
        connect(&mut document, inputs["y"], "series", target, "y", None);
        connect(
            &mut document,
            inputs["x"],
            "series",
            target,
            if surface { "factors" } else { "dose" },
            if surface { Some(0) } else { None },
        );
        let report = execute(&document, kind).unwrap();
        assert_eq!(number(field(&report, "observations")), 640.);
        let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":1000}));
        connect(&mut document, target, "observations", limit, "source", None);
        let RuntimeValue::Relation(table) = execute(&document, "yssbi.dataframe.limit").unwrap()
        else {
            panic!("observations")
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
        let get = |j: usize| match &page.data.columns()[j].values()[0] {
            TabularScalar::Float64(v) => v.as_f64(),
            _ => panic!("number"),
        };
        assert!((get(1) - get(2) - get(3)).abs() < 1e-10);
    }
}
#[test]
fn design_generators_resolve_wide_schemas_and_page_every_run() {
    for (kind, parameters, runs, factors) in [
        (
            "yssbi.statistics.doe.family",
            json!({"factors":10,"levels":2}),
            1024,
            10,
        ),
        (
            "yssbi.statistics.doe.orthogonal",
            json!({"factors":20,"levels":2}),
            32,
            20,
        ),
        (
            "yssbi.statistics.doe.uniform_design",
            json!({"factors":20,"runs":640,"candidates":1,"seed":17}),
            640,
            20,
        ),
    ] {
        let mut document = GraphDocument::default();
        let target = node(&mut document, kind, parameters);
        let report = execute(&document, kind).unwrap();
        assert_eq!(number(field(&report, "runs")), runs as f64);
        let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":2000}));
        connect(&mut document, target, "design", limit, "source", None);
        let RuntimeValue::Relation(table) = execute(&document, "yssbi.dataframe.limit").unwrap()
        else {
            panic!("design relation")
        };
        let c = yss_relational_contract::RelationControl {
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(10),
            max_input_bytes: 4 * 1024 * 1024,
        };
        let page = table.page(runs - 1, 1, &c).unwrap();
        assert_eq!(page.data.columns().len(), factors + 1);
        assert_eq!(
            page.data.columns()[0].values(),
            &[TabularScalar::Float64((runs as f64).try_into().unwrap())]
        );
    }
}

#[test]
fn multivariate_coordinate_schema_accepts_more_than_sixteen_components() {
    let mut document = GraphDocument::default();
    let target = node(
        &mut document,
        "yssbi.statistics.multivariate.pca",
        json!({"components":20,"standardize":true}),
    );
    let data: Vec<_> = (0..20)
        .map(|j| {
            (
                format!("x{j}"),
                json!(
                    (0..640)
                        .map(|i| (((i + 1) * (j + 1)) as f64 * 0.017).sin())
                        .collect::<Vec<_>>()
                ),
            )
        })
        .collect();
    let inputs = columns(
        &mut document,
        &data
            .iter()
            .map(|(k, v)| (k.as_str(), v.clone()))
            .collect::<Vec<_>>(),
    );
    for (j, (key, _)) in data.iter().enumerate() {
        connect(
            &mut document,
            inputs[key],
            "series",
            target,
            "variables",
            Some(j),
        );
    }
    let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":1000}));
    connect(&mut document, target, "scores", limit, "source", None);
    let RuntimeValue::Relation(table) = execute(&document, "yssbi.dataframe.limit").unwrap() else {
        panic!("scores")
    };
    let c = yss_relational_contract::RelationControl {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(10),
        max_input_bytes: 4 * 1024 * 1024,
    };
    let page = table.page(639, 1, &c).unwrap();
    assert_eq!(page.data.columns().len(), 20);
    assert_eq!(page.data.columns()[19].values().len(), 1);
}
#[test]
fn range_analysis_pages_all_factor_levels_and_retains_exact_labels() {
    let mut document = GraphDocument::default();
    let kind = "yssbi.statistics.doe.range_analysis";
    let target = node(&mut document, kind, json!({"maximize":false}));
    let inputs = columns(
        &mut document,
        &[
            ("y", json!((0..640).map(|i| i * 2).collect::<Vec<_>>())),
            (
                "batch",
                json!(
                    (0..640)
                        .map(|i| if i % 2 == 0 { "A" } else { "B" })
                        .collect::<Vec<_>>()
                ),
            ),
            (
                "condition",
                json!((0..640).map(|i| format!("条件{i}")).collect::<Vec<_>>()),
            ),
        ],
    );
    connect(&mut document, inputs["y"], "series", target, "y", None);
    connect(
        &mut document,
        inputs["batch"],
        "series",
        target,
        "factors",
        Some(0),
    );
    connect(
        &mut document,
        inputs["condition"],
        "series",
        target,
        "factors",
        Some(1),
    );
    let report = execute(&document, kind).unwrap();
    assert_eq!(number(field(&report, "observations")), 640.);
    assert!(matches!(
        field(&report, "pairwise_orthogonal"),
        RuntimeValue::Scalar(TabularScalar::Bool(false))
    ));
    assert!(format!("{:?}", field(&report, "level_labels")).contains("条件639"));
    let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":1000}));
    connect(&mut document, target, "levels", limit, "source", None);
    let RuntimeValue::Relation(table) = execute(&document, "yssbi.dataframe.limit").unwrap() else {
        panic!("levels")
    };
    let c = yss_relational_contract::RelationControl {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(10),
        max_input_bytes: 4 * 1024 * 1024,
    };
    let page = table.page(641, 1, &c).unwrap();
    assert_eq!(page.data.columns().len(), 5);
    assert_eq!(
        page.data.columns()[1].values(),
        &[TabularScalar::Float64(640_f64.try_into().unwrap())]
    );
    assert_eq!(
        page.data.columns()[4].values(),
        &[TabularScalar::Float64(1278_f64.try_into().unwrap())]
    );
}
