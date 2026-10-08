use std::time::{Duration, Instant};
use yss_sci::hypothesis::categorical;
use yss_sci_contract::{
    execution::{ScientificCancellationToken, ScientificExecutionControl},
    hypothesis::CategoricalHypothesisTest,
};

#[test]
fn fisher_reports_sample_odds_ratio_and_retains_exact_p_when_ratio_is_infinite() {
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    };
    for (counts, expected, p_value) in [
        ([[8, 2], [1, 5]], Some(20.0), 0.034965034965034975),
        ([[4, 0], [0, 4]], None, 1.0 / 35.0),
        ([[0, 4], [3, 5]], Some(0.0), 0.4909090909090909),
    ] {
        let (mut row, mut column) = (Vec::new(), Vec::new());
        for (r, cells) in counts.iter().enumerate() {
            for (c, &count) in cells.iter().enumerate() {
                for _ in 0..count {
                    row.push(format!("r{r}").into_boxed_str());
                    column.push(format!("c{c}").into_boxed_str());
                }
            }
        }
        let report = categorical::run(
            CategoricalHypothesisTest::FisherExact { row, column },
            &control,
        )
        .unwrap();
        let serialized = serde_json::to_value(&report).unwrap();
        assert_eq!(report.statistic_name, "odds_ratio");
        assert_eq!(serialized["statistic"].as_f64(), expected);
        assert!(
            (report.p_value - p_value).abs() < 1e-12,
            "{counts:?}: {}",
            report.p_value
        );
    }
}

#[test]
fn count_tables_preserve_sorted_cells_for_repeated_long_labels() {
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    };
    let row_labels = ["a😀".repeat(1024), "z😀".repeat(1024)];
    let column_labels = ["a🥧".repeat(1024), "z🥧".repeat(1024)];
    let (mut row, mut column) = (Vec::new(), Vec::new());
    for (r, c, count) in [(1, 0, 11), (0, 1, 9), (1, 1, 3), (0, 0, 1)] {
        for _ in 0..count {
            row.push(row_labels[r].clone().into_boxed_str());
            column.push(column_labels[c].clone().into_boxed_str());
        }
    }
    let independence = categorical::run(
        CategoricalHypothesisTest::Independence {
            row: row.clone(),
            column: column.clone(),
        },
        &control,
    )
    .unwrap();
    assert!((independence.statistic.unwrap() - 384.0 / 35.0).abs() < 1e-12);
    assert_eq!(independence.sample_sizes, vec![24]);
    let fisher = categorical::run(
        CategoricalHypothesisTest::FisherExact { row, column },
        &control,
    )
    .unwrap();
    for (name, expected) in [("a", 1.0), ("b", 9.0), ("c", 11.0), ("d", 3.0)] {
        assert_eq!(fisher.details[name], expected);
    }
    assert_eq!(fisher.sample_sizes, vec![24]);
    assert!(fisher.p_value > 0.0 && fisher.p_value < 1.0);
}
