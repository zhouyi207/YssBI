use super::*;
use yss_sci_contract::survival::{NomogramAxis, NomogramPlot, NomogramTick};

#[test]
fn statistical_projections_preserve_producer_geometry_and_nullable_inference() {
    let correlation = sci::CorrelationPlot {
        labels: vec!["duplicate".into(), "duplicate".into()],
        matrix: vec![vec![Some(1.), None], vec![None, None]],
        p_matrix: vec![vec![None, None], vec![None, None]],
        observations: 123,
    };
    assert_eq!(
        read::project(PlotDataKind::Correlation, &value(&correlation)).unwrap(),
        ResultPlotProjection::Correlation(correlation)
    );
    let correlogram = sci::CorrelogramPlot {
        acf: vec![sci::CorrelogramPoint {
            lag: 1,
            value: -0.2,
            q_stat: Some(4.1),
            p_value: Some(0.043),
        }],
        pacf: vec![sci::CorrelogramPoint {
            lag: 1,
            value: -0.2,
            q_stat: None,
            p_value: None,
        }],
        ci_half_width: 0.19,
        n: 123,
    };
    assert_eq!(
        read::project(PlotDataKind::Correlogram, &value(&correlogram)).unwrap(),
        ResultPlotProjection::Correlogram(correlogram)
    );
    let heatmap = sci::HeatmapPlot {
        x_labels: vec!["same".into(), "same".into()],
        y_labels: vec!["1".into()],
        matrix: vec![vec![-f64::MAX, f64::MAX]],
        metadata: sci::PlotMetadata {
            observations: 4000,
            displayed: 1,
            sampled: true,
        },
    };
    assert_eq!(
        read::project(PlotDataKind::Heatmap, &value(&heatmap)).unwrap(),
        ResultPlotProjection::Heatmap(heatmap)
    );
    let distribution = sci::DistributionPlot {
        groups: vec![sci::DistributionGroup {
            label: "group".into(),
            observations: 10000,
            lower_whisker: 0.,
            q1: 1.,
            median: 2.,
            q3: 3.,
            upper_whisker: 4.,
            outliers: vec![9.],
            outlier_count: 500,
            density: vec![
                sci::PlotPoint { x: 0., y: 0. },
                sci::PlotPoint { x: 4., y: 0.25 },
            ],
        }],
    };
    for (kind, violin) in [(PlotDataKind::Boxplot, false), (PlotDataKind::Violin, true)] {
        assert_eq!(
            read::project(kind, &value(&distribution)).unwrap(),
            ResultPlotProjection::Distribution {
                plot: distribution.clone(),
                violin
            }
        );
    }
    let interval = sci::IntervalPlot {
        data: vec![sci::IntervalPoint {
            x: 1.,
            y: -2.,
            lower: -3.,
            upper: -1.,
        }],
        metadata: xy().metadata,
    };
    assert_eq!(
        read::project(PlotDataKind::Errorbar, &value(&interval)).unwrap(),
        ResultPlotProjection::Interval(interval)
    );
    let coefficients = sci::CoefficientPlot {
        data: (0..205)
            .map(|i| sci::CoefficientPoint {
                label: format!("coefficient {i}"),
                value: i as f64,
                lower: i as f64 - 1.,
                upper: i as f64 + 1.,
            })
            .collect(),
        confidence_level: 0.99,
    };
    assert_eq!(
        read::project(PlotDataKind::Coefficient, &value(&coefficients)).unwrap(),
        ResultPlotProjection::Coefficient(coefficients)
    );
    let nomogram = NomogramPlot {
        axes: (0..4)
            .map(|i| NomogramAxis {
                label: format!("axis {i}"),
                ticks: vec![
                    NomogramTick {
                        position: 0.9,
                        label: "end".into(),
                    },
                    NomogramTick {
                        position: 0.1,
                        label: "start".into(),
                    },
                ],
            })
            .collect(),
        horizon: 365.,
        maximum_total_points: 150.,
        points_per_log_hazard: 100.,
        baseline_cumulative_hazard: 0.12,
    };
    assert_eq!(
        read::project(PlotDataKind::Nomogram, &value(&nomogram)).unwrap(),
        ResultPlotProjection::Nomogram(nomogram)
    );
}

#[test]
fn statistical_projections_reject_shape_and_inference_contract_violations() {
    let correlation = serde_json::json!({"labels":["a","b"],"matrix":[[1,null],[null,1]],"pMatrix":[[null,null],[null,null]],"observations":10});
    for (key, bad) in [
        ("matrix", serde_json::json!([[1, 0]])),
        ("pMatrix", serde_json::json!([[null, null], [null, 2]])),
        ("observations", serde_json::json!(0)),
    ] {
        let mut raw = correlation.clone();
        raw[key] = bad;
        assert!(
            read::project(PlotDataKind::Correlation, &value(raw)).is_err(),
            "{key}"
        );
    }
    let group = serde_json::json!({"label":"a","observations":10,"lowerWhisker":0,"q1":1,"median":2,"q3":3,"upperWhisker":4,"outliers":[9],"outlierCount":1,"density":[{"x":0,"y":0},{"x":4,"y":0.2}]});
    for (key, bad) in [
        ("median", serde_json::json!(5)),
        ("outlierCount", serde_json::json!(0)),
        ("density", serde_json::json!([{"x":0,"y":0},{"x":4,"y":-1}])),
    ] {
        let mut raw = group.clone();
        raw[key] = bad;
        assert!(
            read::project(
                PlotDataKind::Violin,
                &value(serde_json::json!({"groups":[raw]}))
            )
            .is_err(),
            "{key}"
        );
    }
    for (kind, raw) in [
        (
            PlotDataKind::Heatmap,
            serde_json::json!({"xLabels":["a"],"yLabels":["1","2"],"matrix":[[1]],"metadata":{"observations":2,"displayed":2,"sampled":false}}),
        ),
        (
            PlotDataKind::Correlogram,
            serde_json::json!({"acf":[{"lag":1,"value":0.2,"qStat":null,"pValue":null}],"pacf":[{"lag":1,"value":0.2,"qStat":null,"pValue":null}],"n":10,"ciHalfWidth":0.5}),
        ),
        (
            PlotDataKind::Errorbar,
            serde_json::json!({"data":[{"x":1,"y":2,"lower":3,"upper":4}],"metadata":{"observations":2,"displayed":2,"sampled":false}}),
        ),
        (
            PlotDataKind::Coefficient,
            serde_json::json!({"data":[{"label":"a","value":0,"lower":1,"upper":2}],"confidenceLevel":0.95}),
        ),
        (
            PlotDataKind::Nomogram,
            serde_json::json!({"axes":(0..4).map(|_|serde_json::json!({"label":"axis","ticks":[{"label":"value","position":1.1}]})).collect::<Vec<_>>(),"horizon":1,"maximumTotalPoints":100,"pointsPerLogHazard":10,"baselineCumulativeHazard":0.1}),
        ),
    ] {
        assert!(read::project(kind, &value(raw)).is_err(), "{kind:?}");
    }
}
