use super::fixture::{columns, connect, field, node, number};
use super::*;
use serde_json::json;
#[test]
fn quality_nodes_keep_full_chart_rows_and_route_subgroups_and_crossed_labels() {
    let mut chart = GraphDocument::default();
    let kind = "yssbi.statistics.plot.control_chart";
    let target = node(&mut chart, kind, json!({"chart_kind":"moving_range"}));
    let source = columns(
        &mut chart,
        &[(
            "value",
            json!(
                (0..640)
                    .map(|i| i as f64 * 0.1 + (i % 7) as f64)
                    .collect::<Vec<_>>()
            ),
        )],
    );
    connect(
        &mut chart,
        source["value"],
        "series",
        target,
        "measurements",
        None,
    );
    let plot = execute(&chart, kind).unwrap();
    let RuntimeValue::List(points) = field(&plot, "data") else {
        panic!("plot")
    };
    assert_eq!(points.len(), 639);
    assert_eq!(number(field(&points[638], "x")), 640.);
    let RuntimeValue::List(lines) = field(&plot, "referenceLines") else {
        panic!("limits")
    };
    assert_eq!(lines.len(), 3);
    let limit = node(&mut chart, "yssbi.dataframe.limit", json!({"rows":1000}));
    connect(&mut chart, target, "observations", limit, "source", None);
    let RuntimeValue::Relation(table) = execute(&chart, "yssbi.dataframe.limit").unwrap() else {
        panic!("observations")
    };
    let c = yss_relational_contract::RelationControl {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(10),
        max_input_bytes: 4 * 1024 * 1024,
    };
    let page = table.page(638, 1, &c).unwrap();
    assert_eq!(page.data.columns().len(), 6);
    assert_eq!(
        page.data.columns()[0].values(),
        &[TabularScalar::Float64(640_f64.try_into().unwrap())]
    );
    for (kind, params) in [
        (
            "yssbi.statistics.quality.process_capability",
            json!({"lower_limit":0,"upper_limit":10,"target":5}),
        ),
        (
            "yssbi.statistics.quality.measurement_system",
            json!({"include_interaction":false}),
        ),
    ] {
        let mut doc = GraphDocument::default();
        let target = node(&mut doc, kind, params);
        let source = columns(
            &mut doc,
            &[
                (
                    "value",
                    json!(
                        (0..640)
                            .map(|i| (i / 320) as f64 * 3.
                                + ((i / 160) % 2) as f64
                                + ((i % 160) % 7) as f64 * 0.1)
                            .collect::<Vec<_>>()
                    ),
                ),
                (
                    "part",
                    json!(
                        (0..640)
                            .map(|i| if i < 320 { "零件 A" } else { "零件 B" })
                            .collect::<Vec<_>>()
                    ),
                ),
                (
                    "operator",
                    json!(
                        (0..640)
                            .map(|i| if (i / 160) % 2 == 0 {
                                "操作者甲"
                            } else {
                                "操作者乙"
                            })
                            .collect::<Vec<_>>()
                    ),
                ),
            ],
        );
        connect(
            &mut doc,
            source["value"],
            "series",
            target,
            "measurements",
            None,
        );
        if kind.ends_with("process_capability") {
            connect(
                &mut doc,
                source["part"],
                "series",
                target,
                "subgroups",
                Some(0),
            );
        } else {
            connect(&mut doc, source["part"], "series", target, "parts", None);
            connect(
                &mut doc,
                source["operator"],
                "series",
                target,
                "operators",
                None,
            );
        }
        let report = execute(&doc, kind).unwrap();
        assert_eq!(number(field(&report, "observations")), 640.);
        if kind.ends_with("process_capability") {
            assert_eq!(number(field(&report, "subgroups")), 2.);
            assert!(number(field(&report, "cp")) > 0.);
        } else {
            assert_eq!(number(field(&report, "repetitions")), 160.);
            assert!(matches!(
                field(&report, "include_interaction"),
                RuntimeValue::Scalar(TabularScalar::Bool(false))
            ));
            assert!(format!("{:?}", field(&report, "part_labels")).contains("零件 A"));
        }
    }
}
