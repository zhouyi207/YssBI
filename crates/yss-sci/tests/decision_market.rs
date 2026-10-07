use std::time::{Duration, Instant};
use yss_sci::decision::{pricing::price_sensitivity, reach::turf};
use yss_sci_contract::{decision::market::*, execution::*};
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn interval(value: &Option<PriceIntersection>) -> (f64, f64) {
    let value = value.as_ref().unwrap();
    (value.lower, value.upper)
}
#[test]
fn price_curves_preserve_plateaus_and_distinguish_both_bound_definitions() {
    let columns = [1., 3., 5., 7.]
        .into_iter()
        .map(|start| {
            (0..4)
                .map(|i| start + f64::from(i))
                .cycle()
                .take(800)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let original = price_sensitivity(&columns, PriceRangeDefinition::Original, &control()).unwrap();
    assert_eq!(original.rows.len(), 10);
    assert_eq!(original.summary.observations, 800);
    assert_eq!(interval(&original.summary.optimal), (4., 6.));
    assert_eq!(interval(&original.summary.indifference), (5., 5.));
    assert_eq!(interval(&original.summary.marginal_cheapness), (3., 3.));
    assert_eq!(interval(&original.summary.marginal_expensiveness), (7., 7.));
    assert_eq!(original.rows[4].cheap, 0.25);
    assert_eq!(original.rows[4].expensive, 0.25);
    let narrow = price_sensitivity(&columns, PriceRangeDefinition::Narrower, &control()).unwrap();
    assert_eq!(interval(&narrow.summary.marginal_cheapness), (4., 4.));
    assert_eq!(interval(&narrow.summary.marginal_expensiveness), (6., 6.));
    let interpolate = price_sensitivity(
        &[vec![1., 2.], vec![2., 4.], vec![4., 6.], vec![6., 10.]],
        PriceRangeDefinition::Original,
        &control(),
    )
    .unwrap();
    assert_eq!(
        interval(&interpolate.summary.marginal_cheapness),
        (1.5, 1.5)
    );
    let degenerate = price_sensitivity(
        &vec![vec![0.; 3]; 4],
        PriceRangeDefinition::Original,
        &control(),
    )
    .unwrap();
    assert!(degenerate.summary.optimal.is_none());
    assert!(
        price_sensitivity(
            &[vec![1.], vec![0.], vec![2.], vec![3.]],
            PriceRangeDefinition::Original,
            &control()
        )
        .is_err()
    );
    let c = control();
    c.cancellation.cancel();
    assert!(matches!(
        price_sensitivity(&columns, PriceRangeDefinition::Original, &c),
        Err(ScientificComputationError::Cancelled)
    ));
}
#[test]
fn turf_finds_the_exact_combination_where_greedy_selection_fails() {
    let columns = [
        vec![1., 1., 1., 1., 0., 0.],
        vec![1., 1., 0., 0., 1., 0.],
        vec![0., 0., 1., 1., 0., 1.],
    ]
    .into_iter()
    .map(|x| x.into_iter().cycle().take(642).collect::<Vec<_>>())
    .collect::<Vec<_>>();
    let result = turf(&columns, 2, &control()).unwrap();
    assert_eq!(result.selected_criteria, [2, 3]);
    assert_eq!(result.reach_count, 642);
    assert_eq!(result.combinations_evaluated, 3);
    assert_eq!(result.equally_optimal_combinations, 1);
    assert_eq!(result.exposures_per_reached, Some(1.));
    let tied = turf(&[vec![0.; 640], vec![0.; 640]], 1, &control()).unwrap();
    assert_eq!(tied.selected_criteria, [1]);
    assert_eq!(tied.equally_optimal_combinations, 2);
    assert!(tied.exposures_per_reached.is_none());
    // More than 64 options still work: bit packing is over observations, not option IDs.
    assert_eq!(
        turf(&vec![vec![1.]; 70], 70, &control())
            .unwrap()
            .total_exposures,
        70
    );
    assert!(turf(&columns, 4, &control()).is_err());
    assert!(turf(&[vec![2.]], 1, &control()).is_err());
    let c = control();
    c.cancellation.cancel();
    assert!(matches!(
        turf(&columns, 2, &c),
        Err(ScientificComputationError::Cancelled)
    ));
    let mut c = control();
    c.deadline = Instant::now() - Duration::from_secs(1);
    assert!(matches!(
        turf(&columns, 2, &c),
        Err(ScientificComputationError::DeadlineExceeded)
    ));
}
