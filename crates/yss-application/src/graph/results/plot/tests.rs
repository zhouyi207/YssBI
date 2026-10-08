use super::*;
use std::sync::Arc;
use yss_data_contract::ValueType;
use yss_graph_execution::{
    plan::{PlanGraphId, PlanOutputContract, PlanOutputRef, PlanPortAddress, PlanSourceIdentity},
    result::StoredResult,
};
use yss_node_kernel::RuntimeValue;
use yss_sci_contract::visualization as sci;

fn value(value: impl serde::Serialize) -> RuntimeValue {
    RuntimeValue::try_from(serde_json::to_value(value).unwrap()).unwrap()
}
fn xy() -> sci::XyPlot {
    sci::XyPlot {
        data: vec![
            sci::PlotPoint { x: 0., y: 0. },
            sci::PlotPoint { x: 1., y: 1. },
        ],
        x_label: "input".into(),
        y_label: "computed".into(),
        reference_lines: vec![sci::ReferenceLine {
            start: sci::PlotPoint { x: -1., y: 0. },
            end: sci::PlotPoint { x: 2., y: 1. },
        }],
        metadata: sci::PlotMetadata {
            observations: 12,
            displayed: 2,
            sampled: true,
        },
    }
}

#[test]
fn plot_query_preserves_complete_retained_results_and_session_identity() {
    use crate::session::{ApplicationSessionEpoch, ApplicationSessionSlot, NodeComponents};
    let candidate = crate::session::build_current_project_candidate(
        ApplicationSessionEpoch::INITIAL,
        Arc::new(yss_project::ProjectState::new()),
        [],
        &NodeComponents::builtins().unwrap(),
    )
    .unwrap();
    let app = ApplicationState::new(Arc::new(ApplicationSessionSlot::new(
        NodeComponents::builtins().unwrap(),
    )));
    app.install_candidate(candidate).unwrap();
    let captured = app.capture_session().unwrap();
    let mut plot = xy();
    plot.data = (0..5000)
        .map(|i| sci::PlotPoint {
            x: i as f64,
            y: i as f64 / 10.,
        })
        .collect();
    plot.metadata = sci::PlotMetadata {
        observations: 5000,
        displayed: 5000,
        sampled: false,
    };
    let mut raw = serde_json::to_value(&plot).unwrap();
    raw["yDomain"] = serde_json::json!([500., 0.]);
    raw["xFormat"] = serde_json::json!("date");
    let graph = PlanGraphId::from_existing("events/plot.yssbi-event".into());
    let stored = StoredResult::new(value(raw)).with_output_contract(PlanOutputContract {
        data_type: ValueType::Struct("plot.data".into()),
        schema: None,
        category: ResultCategory::PlotData(PlotDataKind::Line),
        source: PlanSourceIdentity::new(graph.clone(), None, None),
    });
    let result_id = captured.execution().publish_fixture_result(
        PlanOutputRef::new(graph.clone(), PlanPortAddress::from_existing("plot".into())),
        stored,
    );
    let reference = ResultReference {
        execution_session_id: captured.execution_session_id(),
        result_id,
    };
    let lease = uuid::Uuid::new_v4();
    app.retain_result(reference, lease, "plot-test", None)
        .unwrap();
    captured
        .execution()
        .invalidate_graph_results(graph.as_str());
    let Some(ResultPlotProjection::Cartesian(projected)) =
        app.query_result_plot(reference).unwrap()
    else {
        panic!("retained line");
    };
    assert_eq!(projected.series.data.len(), 5000);
    assert_eq!(
        projected.series.data[4999],
        PlotPoint { x: 4999., y: 499.9 }
    );
    assert_eq!(
        projected.series.x_format,
        crate::chart::PlotAxisFormat::Date
    );
    assert_eq!(projected.y_domain, Some([500., 0.]));
    assert_eq!(projected.metadata, Some(plot.metadata));
    assert_eq!(projected.reference_lines[0][0].x, -1.);
    let wrong_session = ResultReference {
        execution_session_id: yss_graph_execution::identity::ExecutionSessionId::new(
            uuid::Uuid::new_v4(),
        ),
        ..reference
    };
    assert!(matches!(
        app.query_result_plot(wrong_session),
        Err(ResultQueryApplicationError::SessionChanged)
    ));
    app.release_result_lease(lease, "plot-test").unwrap();
    assert!(app.query_result_plot(reference).unwrap().is_none());
}

#[test]
fn plot_projection_accepts_producer_contracts_and_rejects_malformed_geometry() {
    for kind in [
        PlotDataKind::Scatter,
        PlotDataKind::Line,
        PlotDataKind::Ecdf,
        PlotDataKind::Kde,
    ] {
        let ResultPlotProjection::Cartesian(result) = read::project(kind, &value(xy())).unwrap()
        else {
            panic!("Cartesian projection");
        };
        assert_eq!(result.kind, kind);
        assert_eq!(result.series.y_label.as_deref(), Some("computed"));
        assert_eq!(result.reference_lines.len(), 1);
        assert_eq!(result.metadata, Some(xy().metadata));
    }
    let ResultPlotProjection::Cartesian(probability) = read::project(
        PlotDataKind::PpQq,
        &value(sci::ProbabilityPlot {
            plot: xy(),
            mode: sci::ProbabilityPlotMode::Pp,
            reference_mean: 0.,
            reference_standard_deviation: 1.,
        }),
    )
    .unwrap() else {
        panic!("probability");
    };
    assert_eq!(
        probability.annotation,
        PlotAnnotation::Probability {
            pp: true,
            mean: 0.,
            standard_deviation: 1.
        }
    );
    let ResultPlotProjection::Cartesian(roc) = read::project(
        PlotDataKind::Roc,
        &value(sci::RocPlot {
            plot: xy(),
            auc: 0.75,
            positives: 4,
            negatives: 8,
        }),
    )
    .unwrap() else {
        panic!("ROC");
    };
    assert_eq!(
        roc.annotation,
        PlotAnnotation::Roc {
            auc: 0.75,
            positives: 4,
            negatives: 8
        }
    );
    let ResultPlotProjection::Cartesian(quadrant) = read::project(
        PlotDataKind::Quadrant,
        &value(sci::QuadrantPlot {
            plot: xy(),
            x_cut: 0.5,
            y_cut: 0.5,
            counts: [3, 2, 4, 3],
        }),
    )
    .unwrap() else {
        panic!("quadrant");
    };
    assert_eq!(
        quadrant.annotation,
        PlotAnnotation::Quadrant {
            counts: [3, 2, 4, 3]
        }
    );
    let ResultPlotProjection::Cartesian(bubble) = read::project(
        PlotDataKind::Bubble,
        &value(sci::BubblePlot {
            data: vec![
                sci::BubblePoint {
                    x: 2.,
                    y: 3.,
                    size: 0.,
                },
                sci::BubblePoint {
                    x: 4.,
                    y: 5.,
                    size: 16.,
                },
            ],
            metadata: xy().metadata,
        }),
    )
    .unwrap() else {
        panic!("bubble");
    };
    assert_eq!(bubble.point_sizes, Some(vec![0., 16.]));
    let ResultPlotProjection::Histogram(histogram) = read::project(
        PlotDataKind::Histogram,
        &value(sci::HistogramPlot {
            data: vec![
                sci::HistogramBin {
                    label: "same".into(),
                    lower: 0.,
                    upper: 1.,
                    count: 2,
                },
                sci::HistogramBin {
                    label: "same".into(),
                    lower: 1.,
                    upper: 2.,
                    count: 3,
                },
            ],
            x_label: "value".into(),
            y_label: "frequency".into(),
            observations: 5,
        }),
    )
    .unwrap() else {
        panic!("histogram");
    };
    assert_eq!(histogram.bins.len(), 2);
    assert_eq!(
        histogram
            .bins
            .iter()
            .map(|bin| bin.count)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!(histogram.observations, Some(5));
    for (key, bad) in [
        ("data", serde_json::json!([])),
        ("data", serde_json::json!([{ "x": 1, "y": null }])),
        ("data", serde_json::json!([{ "x": "1", "y": 2 }])),
        ("yDomain", serde_json::json!([1, 1])),
        ("yDomain", serde_json::json!([1, null])),
        (
            "referenceLines",
            serde_json::json!([{ "start": { "x": 0, "y": 1 } }]),
        ),
        (
            "metadata",
            serde_json::json!({ "observations": 12, "displayed": 0, "sampled": true }),
        ),
    ] {
        let mut raw = serde_json::to_value(xy()).unwrap();
        raw[key] = bad;
        assert!(
            read::project(PlotDataKind::Scatter, &value(raw)).is_err(),
            "{key}"
        );
    }
    assert!(
        read::project(
            PlotDataKind::Bubble,
            &value(serde_json::json!({"data": [{"x": 0, "y": 0, "size": -1}]}))
        )
        .is_err()
    );
    let mut bad_roc = serde_json::to_value(sci::RocPlot {
        plot: xy(),
        auc: 0.75,
        positives: 4,
        negatives: 8,
    })
    .unwrap();
    bad_roc["data"] = serde_json::json!([{ "x": 0, "y": 0 }, { "x": 1, "y": 0.9 }]);
    assert!(read::project(PlotDataKind::Roc, &value(bad_roc)).is_err());
}
