use super::fixture::{field, node, number};
use super::*;
use serde_json::json;
#[test]
fn all_power_nodes_execute_default_designs_and_minimum_integer_sample_size_planning() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../../yss-sci/tests/fixtures/power_reference.json"
    ))
    .unwrap();
    for (method, key) in [
        ("principles", "normal"),
        ("mean_difference", "t_two"),
        ("paired", "t_one"),
        ("variance", "variance"),
        ("proportion", "proportion"),
        ("proportion_difference", "proportion_difference"),
        ("correlation", "correlation"),
        ("anova", "anova"),
        ("linear_regression", "linear"),
        ("generalized_model", "poisson"),
        ("logistic", "logistic"),
        ("cox", "survival"),
        ("logrank", "survival"),
        ("cluster_randomized", "cluster"),
        ("noninferiority", "noninferiority"),
        ("equivalence", "equivalence"),
    ] {
        let kind = format!("yssbi.statistics.power.{method}");
        for sample_size in [false, true] {
            let mut document = GraphDocument::default();
            let mut params = json!({"solve_for":if sample_size {"sample_size"}else{"power"}});
            if method == "equivalence" {
                params["difference"] = json!(0.05);
            }
            node(&mut document, &kind, params);
            let report = execute(&document, &kind).unwrap();
            let expected = reference[key][if sample_size { "achieved" } else { "power" }]
                .as_f64()
                .unwrap();
            assert!(
                (number(field(&report, "power")) - expected).abs() < 2e-8,
                "{method}"
            );
            let n = if sample_size {
                reference[key]["required"].as_u64().unwrap() as f64
            } else {
                100.
            };
            assert_eq!(number(field(&report, "sample_size")), n, "{method}");
            assert!(
                (number(field(&report, "power")) + number(field(&report, "type_ii_error")) - 1.)
                    .abs()
                    < 1e-12
            );
        }
    }
}
