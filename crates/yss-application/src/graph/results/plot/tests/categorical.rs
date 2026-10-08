use super::*;

#[test]
fn categorical_projections_preserve_complete_counts_and_aligned_series() {
    let pareto = sci::ParetoPlot {
        data: (0..205)
            .map(|i| sci::ParetoCategory {
                label: format!("category {i}"),
                count: 2,
                cumulative: (i + 1) as f64 / 205.,
            })
            .collect(),
        observations: 410,
    };
    assert_eq!(
        read::project(PlotDataKind::Pareto, &value(&pareto)).unwrap(),
        ResultPlotProjection::Pareto(pareto)
    );
    let cloud = sci::WordCloudPlot {
        words: vec![
            sci::WordCount {
                label: "统计 📈".into(),
                count: usize::MAX / 2,
            },
            sci::WordCount {
                label: "  data  ".into(),
                count: 3,
            },
        ],
        observations: usize::MAX,
        unique_words: 7,
    };
    assert_eq!(
        read::project(PlotDataKind::Wordcloud, &value(&cloud)).unwrap(),
        ResultPlotProjection::WordCloud(cloud)
    );
    for dual_axis in [false, true] {
        let combination = sci::CombinationPlot {
            labels: vec!["same".into(), "same".into(), "".into()],
            bars: vec![-f64::MAX, 0., f64::MAX],
            line: vec![1., -1., 0.],
            dual_axis,
            metadata: sci::PlotMetadata {
                observations: 12,
                displayed: 3,
                sampled: true,
            },
        };
        assert_eq!(
            read::project(PlotDataKind::Combination, &value(&combination)).unwrap(),
            ResultPlotProjection::Combination(combination)
        );
    }
}

#[test]
fn categorical_projections_reject_invalid_counts_cumulative_and_series_shapes() {
    let pareto = serde_json::json!({
        "data":[{"label":"a","count":3,"cumulative":0.6},{"label":"b","count":2,"cumulative":1.}],
        "observations":5,
    });
    for bad in [
        serde_json::json!([]),
        serde_json::json!([{"label":"a","count":5,"cumulative":0.9}]),
        serde_json::json!([{"label":"a","count":3,"cumulative":1.},{"label":"b","count":1,"cumulative":0.9},{"label":"c","count":1,"cumulative":1.}]),
        serde_json::json!([{"label":"a","count":3,"cumulative":0.},{"label":"b","count":2,"cumulative":1.}]),
        serde_json::json!([{"label":"a","count":usize::MAX,"cumulative":0.5},{"label":"b","count":6,"cumulative":1.}]),
    ] {
        let mut raw = pareto.clone();
        raw["data"] = bad;
        assert!(read::project(PlotDataKind::Pareto, &value(raw)).is_err());
    }
    let cloud = serde_json::json!({"words":[{"label":"a","count":3},{"label":"b","count":2}],"uniqueWords":2,"observations":6});
    for (key, bad) in [
        ("uniqueWords", serde_json::json!(1)),
        ("observations", serde_json::json!(4)),
        ("words", serde_json::json!([])),
        ("words", serde_json::json!([{"label":" \t","count":1}])),
        ("words", serde_json::json!([{"label":"a","count":0}])),
        ("words", serde_json::json!([{"label":"a","count":1.5}])),
        (
            "words",
            serde_json::json!([{"label":"a","count":usize::MAX},{"label":"b","count":7}]),
        ),
    ] {
        let mut raw = cloud.clone();
        raw[key] = bad;
        assert!(
            read::project(PlotDataKind::Wordcloud, &value(raw)).is_err(),
            "{key}"
        );
    }
    let combination = serde_json::json!({"labels":["a","b"],"bars":[1.,2.],"line":[3.,4.],"dualAxis":true,"metadata":{"observations":2,"displayed":2,"sampled":false}});
    for (key, bad) in [
        ("labels", serde_json::json!([])),
        ("bars", serde_json::json!([1.])),
        ("line", serde_json::json!([3., null])),
        ("dualAxis", serde_json::json!("true")),
        (
            "metadata",
            serde_json::json!({"observations":1,"displayed":2,"sampled":false}),
        ),
        (
            "metadata",
            serde_json::json!({"observations":2,"displayed":1,"sampled":false}),
        ),
    ] {
        let mut raw = combination.clone();
        raw[key] = bad;
        assert!(
            read::project(PlotDataKind::Combination, &value(raw)).is_err(),
            "{key}"
        );
    }
}
