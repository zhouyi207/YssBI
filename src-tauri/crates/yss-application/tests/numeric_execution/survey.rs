use super::fixture::{columns, connect, field, node, number};
use super::*;
use serde_json::json;

fn source(document: &mut GraphDocument) -> BTreeMap<String, NodeId> {
    let x: Vec<_> = (0..640).map(|i| (i % 16) as f64 / 5. - 1.5).collect();
    let z: Vec<_> = (0..640).map(|i| (i * 13 % 17) as f64 / 8. - 1.).collect();
    let y: Vec<_> = (0..640)
        .map(|i| {
            4. + 0.5 * x[i] - 0.3 * z[i] + 0.4 * ((i / 8) % 7) as f64 - 1.2
                + 0.03 * ((i * 11 % 19) as f64 - 9.)
        })
        .collect();
    let binary: Vec<_> = (0..640)
        .map(|i| u8::from(((i * 37 % 101) as f64) < 45. + 3. * x[i] - 2. * z[i]))
        .collect();
    let counts: Vec<_> = (0..640)
        .map(|i| (i * 7 % 6) + usize::from(x[i] > 0.))
        .collect();
    let probability: Vec<_> = (0..640).map(|i| 1. / (1. + (i % 7) as f64 * 0.2)).collect();
    let strata: Vec<_> = (0..640).map(|i| format!("stratum{}", i / 160)).collect();
    let psu: Vec<_> = (0..640).map(|i| format!("psu{}", (i / 8) % 20)).collect();
    columns(
        document,
        &[
            ("x", json!(x)),
            ("z", json!(z)),
            ("y", json!(y)),
            ("binary", json!(binary)),
            ("counts", json!(counts)),
            ("probability", json!(probability)),
            ("strata", json!(strata)),
            ("psu", json!(psu)),
        ],
    )
}
fn weights(document: &mut GraphDocument, source: &BTreeMap<String, NodeId>) -> NodeId {
    let weights = node(
        document,
        "yssbi.statistics.survey.weights",
        json!({"input_kind":"inclusion_probabilities"}),
    );
    connect(
        document,
        source["probability"],
        "series",
        weights,
        "values",
        None,
    );
    weights
}
#[test]
fn survey_weights_and_mean_nodes_preserve_source_alignment_and_nested_psu_identity() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../../yss-sci/tests/fixtures/survey_reference.json"
    ))
    .unwrap();
    for method in ["mean_proportion", "stratified", "clustered"] {
        let kind = format!("yssbi.statistics.survey.{method}");
        let mut document = GraphDocument::default();
        let source = source(&mut document);
        let weights = weights(&mut document, &source);
        let report = execute(&document, "yssbi.statistics.survey.weights").unwrap();
        assert_eq!(number(field(&report, "observations")), 640.);
        let target = node(
            &mut document,
            &kind,
            json!({"statistic":"mean","lonely_psu":"fail"}),
        );
        connect(
            &mut document,
            source["y"],
            "series",
            target,
            "response",
            None,
        );
        connect(&mut document, weights, "weights", target, "weights", None);
        connect(
            &mut document,
            source["strata"],
            "series",
            target,
            "strata",
            if method == "stratified" {
                None
            } else {
                Some(0)
            },
        );
        connect(
            &mut document,
            source["psu"],
            "series",
            target,
            "clusters",
            if method == "clustered" { None } else { Some(0) },
        );
        let report = execute(&document, &kind).unwrap();
        let design = field(&report, "design");
        assert_eq!(number(field(design, "primary_sampling_units")), 80.);
        assert_eq!(number(field(design, "degrees_of_freedom")), 76.);
        assert!(
            (number(field(&report, "estimate")) - reference["nested"]["mean"].as_f64().unwrap())
                .abs()
                < 1e-8
        );
        assert!(
            (number(field(&report, "standard_error"))
                - reference["nested"]["se"].as_f64().unwrap())
            .abs()
                < 1e-8
        );
    }
}
#[test]
fn survey_regressions_report_only_predictor_names_and_keep_every_fitted_row() {
    for (method, response) in [
        ("linear_regression", "y"),
        ("logistic", "binary"),
        ("poisson", "counts"),
    ] {
        let kind = format!("yssbi.statistics.survey.{method}");
        let mut document = GraphDocument::default();
        let source = source(&mut document);
        let weights = weights(&mut document, &source);
        let target = node(
            &mut document,
            &kind,
            json!({"lonely_psu":"fail","constant":true,"max_iterations":500,"tolerance":0.0000001}),
        );
        connect(
            &mut document,
            source[response],
            "series",
            target,
            "response",
            None,
        );
        connect(&mut document, weights, "weights", target, "weights", None);
        for (name, port) in [("strata", "strata"), ("psu", "clusters")] {
            connect(&mut document, source[name], "series", target, port, Some(0));
        }
        for (j, name) in ["x", "z"].into_iter().enumerate() {
            connect(
                &mut document,
                source[name],
                "series",
                target,
                "predictors",
                Some(j),
            );
        }
        let report = execute(&document, &kind).unwrap();
        let RuntimeValue::List(names) = field(&report, "factor_names") else {
            panic!("names")
        };
        assert_eq!(names.len(), 2);
        assert_eq!(
            number(field(
                field(&report, "details"),
                "coefficient_degrees_of_freedom"
            )),
            74.
        );
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
        assert_eq!(
            page.data.columns()[0].values(),
            &[TabularScalar::Float64(640_f64.try_into().unwrap())]
        );
        let get = |j: usize| {
            let TabularScalar::Float64(v) = page.data.columns()[j].values()[0] else {
                panic!("number")
            };
            v.as_f64()
        };
        assert!((get(1) - get(2) - get(3)).abs() < 1e-8);
    }
}
