use super::fixture::{columns, connect, field, node, number};
use super::*;
use serde_json::json;
#[test]
fn moderation_nodes_keep_role_alignment_conditional_effects_and_every_prediction() {
    for advanced in [false, true] {
        let kind = if advanced {
            "yssbi.statistics.workflow.moderation_advanced"
        } else {
            "yssbi.statistics.workflow.moderation"
        };
        let mut document = GraphDocument::default();
        let x: Vec<_> = (0..640).map(|i| (i % 8) as f64 - 3.5).collect();
        let w: Vec<_> = (0..640).map(|i| ((i / 8) % 8) as f64 - 3.5).collect();
        let z: Vec<_> = (0..640).map(|i| ((i / 64) % 10) as f64 - 4.5).collect();
        let y: Vec<_> = (0..640)
            .map(|i| {
                2. + 0.3 * x[i]
                    + 0.2 * w[i]
                    + 0.5 * x[i] * w[i]
                    + 0.02 * ((i * 7 % 17) as f64 - 8.)
                    + if advanced {
                        0.4 * z[i] - 0.1 * x[i] * z[i]
                            + 0.06 * w[i] * z[i]
                            + 0.08 * x[i] * w[i] * z[i]
                    } else {
                        0.
                    }
            })
            .collect();
        let source = columns(
            &mut document,
            &[
                ("y", json!(y)),
                ("x", json!(x)),
                ("w", json!(w)),
                ("z", json!(z)),
            ],
        );
        let target = node(&mut document, kind, json!({"probe_sd":1}));
        for (column, input) in [("y", "y"), ("x", "x"), ("w", "moderator")] {
            connect(&mut document, source[column], "series", target, input, None);
        }
        if advanced {
            connect(
                &mut document,
                source["z"],
                "series",
                target,
                "second_moderator",
                None,
            );
        }
        let report = execute(&document, kind).unwrap();
        assert_eq!(number(field(&report, "observations")), 640.);
        let RuntimeValue::List(effects) = field(field(&report, "details"), "effects") else {
            panic!("effects")
        };
        assert_eq!(effects.len(), if advanced { 9 } else { 3 });
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
    }
}

#[test]
fn mediation_and_recursive_path_nodes_preserve_role_order_and_complete_equation_tables() {
    for stage in ["none", "first", "second", "recursive"] {
        let kind = match stage {
            "none" => "yssbi.statistics.workflow.mediation",
            "recursive" => "yssbi.statistics.sem.path",
            _ => "yssbi.statistics.workflow.moderated_mediation",
        };
        let mut document = GraphDocument::default();
        let x: Vec<_> = (0..640).map(|i| (i % 8) as f64 - 3.5).collect();
        let w: Vec<_> = (0..640).map(|i| ((i / 8) % 8) as f64 - 3.5).collect();
        let m: Vec<_> = (0..640)
            .map(|i| {
                2. + 0.6 * x[i] + 0.2 * w[i] + 0.15 * x[i] * w[i] + 0.3 * ((i * 7 % 17) as f64 - 8.)
            })
            .collect();
        let y: Vec<_> = (0..640)
            .map(|i| {
                1. + 0.2 * x[i] + 0.7 * m[i] + 0.15 * w[i] + 0.02 * ((i * 11 % 19) as f64 - 9.)
            })
            .collect();
        let source = columns(
            &mut document,
            &[
                ("x", json!(x)),
                ("m", json!(m)),
                ("w", json!(w)),
                ("y", json!(y)),
            ],
        );
        let params = match stage {
            "none" => json!({"replications":8,"seed":42}),
            "recursive" => json!({"equations":"x3 ~ x1 + x2; x2 ~ x1"}),
            _ => json!({"replications":8,"seed":42,"stage":stage,"probe_sd":1}),
        };
        let target = node(&mut document, kind, params);
        if stage == "recursive" {
            for (j, name) in ["x", "m", "y"].into_iter().enumerate() {
                connect(
                    &mut document,
                    source[name],
                    "series",
                    target,
                    "variables",
                    Some(j),
                );
            }
        } else {
            for (name, port) in [("y", "y"), ("x", "x"), ("m", "mediator")] {
                connect(&mut document, source[name], "series", target, port, None);
            }
            if stage != "none" {
                connect(
                    &mut document,
                    source["w"],
                    "series",
                    target,
                    "moderator",
                    None,
                );
            }
        }
        let report = execute(&document, kind).unwrap();
        assert_eq!(number(field(&report, "observations")), 640.);
        if stage != "recursive" {
            let RuntimeValue::List(effects) = field(field(&report, "details"), "effects") else {
                panic!("effects")
            };
            assert_eq!(effects.len(), if stage == "none" { 1 } else { 3 });
            assert!(matches!(
                field(field(&effects[0], "indirect"), "confidence_interval"),
                RuntimeValue::List(_)
            ));
        }
        let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":2000}));
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
        let page = table
            .page(if stage == "recursive" { 1279 } else { 639 }, 1, &c)
            .unwrap();
        assert_eq!(
            page.data.columns().len(),
            if stage == "recursive" { 5 } else { 7 }
        );
        let observation = usize::from(stage == "recursive");
        assert_eq!(
            page.data.columns()[observation].values(),
            &[TabularScalar::Float64(640_f64.try_into().unwrap())]
        );
        let values = |j: usize| {
            let TabularScalar::Float64(v) = page.data.columns()[j].values()[0] else {
                panic!("numeric")
            };
            v.as_f64()
        };
        assert!(
            (values(observation + 1) - values(observation + 2) - values(observation + 3)).abs()
                < 1e-8
        );
    }
}
