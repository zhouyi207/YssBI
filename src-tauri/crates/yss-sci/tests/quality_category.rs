use std::time::{Duration, Instant};
use yss_sci::quality::*;
use yss_sci_contract::{execution::*, quality::*};
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/quality_reference.json")).unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * (1. + b.abs()), "{a} != {b}")
}
#[test]
fn process_limits_and_capability_match_reference_and_preserve_all_signals() {
    let x = [49.6, 47.6, 49.9, 51.3, 47.8, 51.2, 52.6, 52.4, 53.6, 52.1];
    let c = control();
    let f = fixture();
    let f = &f["capability"];
    let reference = |k: &str| f[k].as_f64().unwrap();
    let limits = CapabilityLimits {
        lower: 45.,
        upper: 56.,
        target: 50.5,
    };
    let fit = process_capability(&x, None, limits, &c).unwrap();
    for (a, k) in [
        (fit.mean, "mean"),
        (fit.overall_standard_deviation, "overall"),
        (fit.within_standard_deviation, "within"),
        (fit.cp.unwrap(), "cp"),
        (fit.cpk.unwrap(), "cpk"),
        (fit.pp.unwrap(), "pp"),
        (fit.ppk.unwrap(), "ppk"),
        (fit.cpm.unwrap(), "cpm"),
        (fit.within_normal_ppm.unwrap(), "within_ppm"),
        (fit.overall_normal_ppm.unwrap(), "overall_ppm"),
    ] {
        close(a, reference(k));
    }
    let grouped =
        process_capability(&x, Some(&[0, 0, 0, 0, 0, 1, 1, 1, 1, 1]), limits, &c).unwrap();
    close(grouped.within_standard_deviation, reference("pooled"));
    assert_eq!(grouped.subgroups, Some(2));
    let plot = control_chart(&x, ControlChartKind::Individuals, &c).unwrap();
    close(
        plot.summary.lower,
        reference("mean") - 3. * reference("within"),
    );
    close(
        plot.summary.upper,
        reference("mean") + 3. * reference("within"),
    );
    assert_eq!(plot.summary.outside_count, 0);
    assert_eq!(plot.plot.reference_lines.len(), 3);
    let plot = control_chart(&x, ControlChartKind::MovingRange, &c).unwrap();
    close(plot.summary.upper, 3.267 * reference("mr"));
    assert_eq!(plot.rows[0].observation, 2);
    assert_eq!(plot.rows.len(), 9);
    let mut large = vec![50.; 640];
    large[639] = 100.;
    let plot = control_chart(&large, ControlChartKind::Individuals, &c).unwrap();
    assert_eq!(plot.plot.data.len(), 640);
    assert!(!plot.plot.metadata.sampled && plot.rows[639].outside);
    let zero = process_capability(&[50.; 640], None, limits, &c).unwrap();
    assert!(zero.cp.is_none() && zero.pp.is_none() && zero.within_normal_ppm.is_none());
    close(zero.cpm.unwrap(), 11. / 3.);
    assert!(process_capability(&x, Some(&[0, 1, 1, 1, 1, 1, 1, 1, 1, 1]), limits, &c).is_err());
}
#[test]
fn crossed_random_effects_match_statsmodels_sums_of_squares_and_correct_denominators() {
    let parts: Vec<_> = (0..720).map(|i| i / 72).collect();
    let operators: Vec<_> = (0..720).map(|i| (i / 24) % 3).collect();
    let y: Vec<_> = (0..720)
        .map(|i| {
            10. + 0.7 * parts[i] as f64
                + 0.25 * operators[i] as f64
                + 0.12 * (parts[i] * operators[i]) as f64
                + (((i * 17) % 29) as f64 - 14.) * 0.03
        })
        .collect();
    let f = fixture();
    for (interaction, key) in [(true, "gage_interaction"), (false, "gage_pooled")] {
        let fit = measurement_system(&y, &parts, &operators, interaction, &control()).unwrap();
        assert_eq!(
            (fit.observations, fit.parts, fit.operators, fit.repetitions),
            (720, 10, 3, 24)
        );
        let reference = &f[key];
        for (j, row) in fit.anova.iter().enumerate() {
            close(row.sum_squares, reference["ss"][j].as_f64().unwrap());
            close(
                row.degrees_of_freedom as f64,
                reference["df"][j].as_f64().unwrap(),
            );
            if j + 1 < fit.anova.len() {
                close(
                    row.f_statistic.unwrap(),
                    reference["f"][j].as_f64().unwrap(),
                );
                close(row.p_value.unwrap(), reference["p"][j].as_f64().unwrap());
            }
        }
        for (j, row) in fit.components.iter().enumerate() {
            close(row.variance, reference["components"][j].as_f64().unwrap());
            close(row.study_variation, 6. * row.standard_deviation);
        }
        close(
            fit.total_sum_squares,
            fit.anova.iter().map(|r| r.sum_squares).sum(),
        );
    }
}
#[test]
fn crossed_studies_reject_missing_cells_and_retain_zero_variation_without_nan() {
    let parts = [0, 0, 0, 0, 1, 1, 1, 1];
    let operators = [0, 0, 1, 1, 0, 0, 1, 1];
    let c = control();
    let zero = measurement_system(&[3.; 8], &parts, &operators, true, &c).unwrap();
    assert!(zero.components.iter().all(|r| r.variance == 0.
        && r.contribution_percent.is_none()
        && r.study_variation_percent.is_none()));
    assert!(
        zero.anova
            .iter()
            .all(|r| r.f_statistic.is_none() && r.p_value.is_none())
    );
    let varied = measurement_system(
        &[0., 1., 0., 1., 0., 1., 0., 1.],
        &parts,
        &operators,
        true,
        &c,
    )
    .unwrap();
    assert!(
        varied
            .negative_components_truncated
            .contains(&"interaction")
    );
    assert!(measurement_system(&[3.; 8], &parts, &[0, 0, 0, 1, 0, 0, 1, 1], true, &c).is_err());
    assert!(measurement_system(&[3.; 8], &[0; 8], &operators, true, &c).is_err());
    c.cancellation.cancel();
    assert!(matches!(
        measurement_system(&[3.; 8], &parts, &operators, true, &c),
        Err(ScientificComputationError::Cancelled)
    ));
    assert!(matches!(
        control_chart(&[1., 2.], ControlChartKind::Individuals, &c),
        Err(ScientificComputationError::Cancelled)
    ));
}
