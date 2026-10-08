use std::time::{Duration, Instant};
use yss_sci::hypothesis::categorical;
use yss_sci_contract::{
    execution::{ScientificCancellationToken, ScientificExecutionControl},
    hypothesis::CategoricalHypothesisTest,
};

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
    assert!((independence.statistic - 384.0 / 35.0).abs() < 1e-12);
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
