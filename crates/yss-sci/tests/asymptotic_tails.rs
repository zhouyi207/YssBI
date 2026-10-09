use yss_sci::{diagnostics::residual::normality, regression::discrete::fit::fit_binary};
use yss_sci_contract::{
    MissingValuePolicy, StatisticalObservationMetadata,
    regression::{discrete::BinaryOptions, fit::BinaryRegressionLink},
};

#[test]
fn asymptotic_inference_retains_small_and_subnormal_probabilities() {
    let residuals: Vec<_> = (0..20).map(|i| f64::from(i == 0)).collect();
    let result = normality(&residuals).unwrap();
    // Chi-square(2) is exponential with rate 1/2.
    let mut reports = vec![(
        "Jarque-Bera",
        result.jarque_bera_p_value,
        (-result.jarque_bera_stat / 2.0).exp(),
    )];
    for (link, label, ordinary) in [
        (
            BinaryRegressionLink::Logit,
            "Logit",
            1.141_306_038_338_677_2e-20,
        ),
        (
            BinaryRegressionLink::Probit,
            "Probit",
            2.905_331_538_655_039e-26,
        ),
    ] {
        for successes in [20, 180] {
            let fit = fit_binary(
                link,
                (0..200).map(|i| f64::from(i < successes)).collect(),
                &[vec![1.0; 200]],
                BinaryOptions {
                    constant: false,
                    max_iterations: 100,
                    tolerance: 1e-12,
                },
                StatisticalObservationMetadata {
                    original_observation_count: 200,
                    used_observation_count: 200,
                    dropped_null_count: 0,
                    dropped_nan_count: 0,
                    missing_value_policy: MissingValuePolicy::Reject,
                },
            )
            .unwrap();
            reports.push((
                label,
                fit.statistics.coefficient_statistics().p_values[0],
                ordinary,
            ));
        }
    }
    // Closed Bernoulli MLE gives z=38.47953927894769. Independent erfc
    // rounds its two-sided probability to the smallest f64 subnormal.
    let fit = fit_binary(
        BinaryRegressionLink::Logit,
        (0..3408).map(|i| f64::from(i < 3067)).collect(),
        &[vec![1.0; 3408]],
        BinaryOptions {
            constant: false,
            max_iterations: 100,
            tolerance: 1e-12,
        },
        StatisticalObservationMetadata {
            original_observation_count: 3408,
            used_observation_count: 3408,
            dropped_null_count: 0,
            dropped_nan_count: 0,
            missing_value_policy: MissingValuePolicy::Reject,
        },
    )
    .unwrap();
    assert!(
        (fit.statistics.coefficient_statistics().statistic_values[0] - 38.479_539_278_947_69).abs()
            < 1e-9
    );
    reports.push((
        "Logit subnormal",
        fit.statistics.coefficient_statistics().p_values[0],
        f64::from_bits(1),
    ));
    let lost: Vec<_> = reports
        .into_iter()
        .filter(|(_, actual, expected)| {
            (actual / expected - 1.0).abs() >= 1e-8 || !actual.is_finite()
        })
        .collect();
    assert!(
        lost.is_empty(),
        "Representable asymptotic tails lost: {lost:?}"
    );
}
