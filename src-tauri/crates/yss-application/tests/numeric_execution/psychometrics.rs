use super::fixture::{columns, connect, field, node, number};
use super::*;
use serde_json::json;
#[test]
fn questionnaire_nodes_route_all_rows_and_pageable_item_scores() {
    for suffix in [
        "reliability",
        "validity",
        "content_validity",
        "item_analysis",
    ] {
        let mut document = GraphDocument::default();
        let kind = format!("yssbi.statistics.psychometrics.{suffix}");
        let target = node(&mut document, &kind, json!({}));
        let expert = suffix == "content_validity";
        let inputs = columns(
            &mut document,
            &[
                (
                    "first",
                    json!(
                        (0..640)
                            .map(|i| if expert {
                                3.
                            } else {
                                (i % 11) as f64 + ((i * 7) % 13) as f64 * 0.1
                            })
                            .collect::<Vec<_>>()
                    ),
                ),
                (
                    "second",
                    json!(
                        (0..640)
                            .map(|i| if expert {
                                4.
                            } else {
                                (i % 11) as f64 * 0.5 + ((i * 19) % 17) as f64 * 0.2
                            })
                            .collect::<Vec<_>>()
                    ),
                ),
            ],
        );
        for (j, key) in ["first", "second"].iter().enumerate() {
            connect(
                &mut document,
                inputs[*key],
                "series",
                target,
                "items",
                Some(j),
            );
        }
        let report = execute(&document, &kind).unwrap();
        assert_eq!(
            number(field(
                &report,
                if expert { "experts" } else { "observations" }
            )),
            640.
        );
        if expert {
            assert_eq!(number(field(&report, "scale_cvi_average")), 1.)
        }
        if suffix == "validity" {
            assert!((number(field(&report, "kmo")) - 0.5).abs() < 1e-10);
            continue;
        }
        let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":1000}));
        let (port, offset, width, first) = if suffix == "item_analysis" {
            ("scores", 639, 3, 640.)
        } else {
            ("item_statistics", 1, 5, 2.)
        };
        connect(&mut document, target, port, limit, "source", None);
        let RuntimeValue::Relation(table) = execute(&document, "yssbi.dataframe.limit").unwrap()
        else {
            panic!("table")
        };
        let c = yss_relational_contract::RelationControl {
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(10),
            max_input_bytes: 4 * 1024 * 1024,
        };
        let page = table.page(offset, 1, &c).unwrap();
        assert_eq!(page.data.columns().len(), width);
        assert_eq!(
            page.data.columns()[0].values(),
            &[TabularScalar::Float64(first.try_into().unwrap())]
        );
    }
}
