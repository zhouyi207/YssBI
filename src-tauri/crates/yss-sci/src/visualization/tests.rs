use super::*;
use std::time::{Duration, Instant};
use yss_sci_contract::execution::ScientificCancellationToken;
use yss_sci_contract::visualization::*;

fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).into()).collect()
}

#[test]
fn histogram_counts_include_the_maximum_and_constant_samples() {
    let plot = histogram(&[0., 1., 2., 3., 4.], 2, &control()).unwrap();
    assert_eq!(
        plot.data.iter().map(|bin| bin.count).collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!(plot.data[0].lower, 0.);
    assert_eq!(plot.data[1].upper, 4.);
    let constant = histogram(&[7., 7., 7.], 0, &control()).unwrap();
    assert_eq!(constant.data.iter().map(|bin| bin.count).sum::<usize>(), 3);
    assert!(
        constant
            .data
            .iter()
            .all(|bin| bin.lower.is_finite() && bin.upper > bin.lower)
    );
}

#[test]
fn ecdf_accumulates_tied_mass_and_reaches_one() {
    let plot = ecdf(&[3., 1., 1., 2.], &control()).unwrap();
    assert_eq!(
        plot.data,
        vec![
            PlotPoint { x: 1., y: 0.5 },
            PlotPoint { x: 2., y: 0.75 },
            PlotPoint { x: 3., y: 1. }
        ]
    );
    assert!(!plot.metadata.sampled);
    assert_eq!(plot.metadata.observations, 4);
}

#[test]
fn gaussian_density_has_finite_grid_and_approximately_unit_mass() {
    let plot = kde(&[-2., -1., 0., 1., 2.], 512, &control()).unwrap();
    let area = plot
        .data
        .windows(2)
        .map(|p| (p[1].x - p[0].x) * (p[1].y + p[0].y) / 2.)
        .sum::<f64>();
    assert!((0.94..=1.001).contains(&area), "{area}");
    assert!(
        plot.data
            .iter()
            .all(|point| point.x.is_finite() && point.y.is_finite() && point.y >= 0.)
    );
    assert!(!plot.metadata.sampled);
}

#[test]
fn distributions_use_linear_quartiles_tukey_whiskers_and_independent_samples() {
    let groups = vec![vec![0., 1., 2., 3., 4., 100.], vec![7., 8., 9.]];
    let result = box_violin(&strings(&["A", "B"]), &groups, false, &control()).unwrap();
    let first = &result.groups[0];
    assert_eq!((first.q1, first.median, first.q3), (1.25, 2.5, 3.75));
    assert_eq!((first.lower_whisker, first.upper_whisker), (0., 4.));
    assert_eq!(first.outliers, vec![100.]);
    assert_eq!(result.groups[1].observations, 3);
    assert_eq!(
        box_violin(&strings(&["B"]), &groups[1..], true, &control())
            .unwrap()
            .groups[0]
            .density
            .len(),
        128
    );
}

#[test]
fn normal_probability_modes_follow_hazen_positions_without_endpoint_infinities() {
    let q = 0.674_489_750_196_081_7;
    let qq = probability(&[-q, q], ProbabilityPlotMode::Qq, false, 0., 1., &control()).unwrap();
    assert!((qq.plot.data[0].x + q).abs() < 1e-10);
    assert!((qq.plot.data[1].x - q).abs() < 1e-10);
    let pp = probability(&[-q, q], ProbabilityPlotMode::Pp, false, 0., 1., &control()).unwrap();
    assert!((pp.plot.data[0].x - 0.25).abs() < 1e-10);
    assert_eq!(pp.plot.data[1].y, 0.75);
    assert!(probability(&[3., 3.], ProbabilityPlotMode::Qq, true, 0., 1., &control()).is_err());
}

#[test]
fn roc_ties_receive_half_credit_and_auc_uses_every_observation() {
    let result = roc(
        &[false, true, false, true],
        &[0.1, 0.4, 0.4, 0.8],
        &control(),
    )
    .unwrap();
    assert_eq!(result.auc, 0.875);
    assert_eq!(result.plot.data.first(), Some(&PlotPoint { x: 0., y: 0. }));
    assert_eq!(result.plot.data.last(), Some(&PlotPoint { x: 1., y: 1. }));
    assert_eq!(
        roc(&[true, false], &[0.5, 0.5], &control()).unwrap().auc,
        0.5
    );
    assert!(roc(&[true, true], &[0.1, 0.2], &control()).is_err());
}

#[test]
fn point_sampling_preserves_endpoints_and_quadrant_counts_use_the_population() {
    let x = (0..10_000).map(|i| i as f64).collect::<Vec<_>>();
    let y = x.clone();
    let result = quadrant(&x, &y, 5000., 5000., &control()).unwrap();
    assert_eq!(result.counts, [5000, 0, 5000, 0]);
    assert_eq!(result.plot.data.len(), MAX_PLOT_POINTS);
    assert_eq!(result.plot.data.first().unwrap().x, 0.);
    assert_eq!(result.plot.data.last().unwrap().x, 9999.);
    assert!(result.plot.metadata.sampled);
}

#[test]
fn categorical_plots_count_terms_and_cumulative_shares_from_full_samples() {
    let values = strings(&["b", "a", "b", "c", "a", "a"]);
    let cloud = word_cloud(&values, 2, &control()).unwrap();
    assert_eq!(cloud.unique_words, 3);
    assert_eq!(
        cloud.words[0],
        WordCount {
            label: "a".into(),
            count: 3
        }
    );
    let plot = pareto(&values, &control()).unwrap();
    assert_eq!(
        plot.data.iter().map(|row| row.count).collect::<Vec<_>>(),
        vec![3, 2, 1]
    );
    assert_eq!(plot.data.last().unwrap().cumulative, 1.);
    let combo = combination(
        &strings(&["A", "B"]),
        &[2., 4.],
        &[1., 3.],
        true,
        &control(),
    )
    .unwrap();
    assert_eq!(combo.bars, vec![2., 4.]);
    assert!(combo.dual_axis);
}

#[test]
fn intervals_use_t_quantiles_and_validate_order_and_bubble_sizes() {
    let result = coefficients(&strings(&["x"]), &[2.], &[0.5], 5., 0.95, &control()).unwrap();
    assert!((result.data[0].upper - 3.285_290_918_).abs() < 1e-8);
    assert!((result.data[0].lower - 0.714_709_082_).abs() < 1e-8);
    assert!(error_bars(&[1.], &[2.], &[3.], &[4.], &control()).is_err());
    assert!(bubble(&[1.], &[2.], &[-1.], &control()).is_err());
    assert_eq!(
        bubble(&[1., 2.], &[2., 3.], &[0., 4.], &control())
            .unwrap()
            .data[1]
            .size,
        4.
    );
}

#[test]
fn matrix_plots_retain_column_orientation_and_undefined_constant_correlations() {
    let columns = vec![
        vec![1., 2., 3., 4.],
        vec![2., 4., 6., 8.],
        vec![1., 1., 1., 1.],
    ];
    let labels = strings(&["A", "B", "C"]);
    let result = correlation(&labels, &columns, &control()).unwrap();
    assert!((result.matrix[0][1].unwrap() - 1.).abs() < 1e-12);
    assert_eq!(result.matrix[2][2], None);
    assert_eq!(result.matrix[0][2], None);
    let heat = heatmap(&labels, &columns, &control()).unwrap();
    assert_eq!(heat.matrix[1], vec![2., 4., 1.]);
    let acf = correlogram(&[1., 2., 1., 4., 2., 6., 3., 7.], 20, &control()).unwrap();
    assert_eq!(acf.acf.len(), 3);
    assert_eq!(acf.pacf.len(), 3);
    assert!(
        acf.acf
            .iter()
            .all(|bar| bar.q_stat.unwrap() >= 0. && (0.0..=1.0).contains(&bar.p_value.unwrap()))
    );
}

#[test]
fn plot_computation_rejects_shape_nonfinite_and_cancelled_inputs() {
    assert!(xy(&[1., 2.], &[3.], false, &control()).is_err());
    assert!(histogram(&[f64::NAN], 2, &control()).is_err());
    let cancelled = control();
    cancelled.cancellation.cancel();
    assert_eq!(
        kde(&[0., 1.], 256, &cancelled),
        Err(ScientificComputationError::Cancelled)
    );
    assert_eq!(
        xy(&[0.], &[1.], false, &cancelled),
        Err(ScientificComputationError::Cancelled)
    );
}

#[test]
fn scale_limits_plot_data_keeps_all_categories_and_coefficients_for_paging() {
    let labels = (0..2200).map(|i| format!("term{i}")).collect::<Vec<_>>();
    let pareto = pareto(&labels, &control()).unwrap();
    assert_eq!(pareto.data.len(), 2200);
    assert_eq!(pareto.data.last().unwrap().cumulative, 1.0);
    let values = (0..2200).map(|i| i as f64).collect::<Vec<_>>();
    let coefficients =
        coefficients(&labels, &values, &vec![0.5; 2200], 50.0, 0.95, &control()).unwrap();
    assert_eq!(coefficients.data.len(), 2200);
    assert_eq!(coefficients.data[2199].value, 2199.0);
    assert!(coefficients.data[2199].lower < 2199.0);
}
