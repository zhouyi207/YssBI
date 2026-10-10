use std::time::{Duration, Instant};
use yss_sci::power::compute;
use yss_sci_contract::{execution::*, power::*};
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: Default::default(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-8, "{a} != {b}");
}

fn bounded_power(model: PowerModel, options: PowerOptions) -> PowerResult {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let control = ScientificExecutionControl {
            deadline: Instant::now() + Duration::from_millis(500),
            ..control()
        };
        let _ = send.send(compute(model, options, &control));
    });
    receive
        .recv_timeout(Duration::from_secs(2))
        .expect("scalar Power computation did not honor its deadline")
        .unwrap()
}

#[test]
fn noncentral_f_power_finishes_for_large_denominator_degrees() {
    let sample_size = 1_000_000_000_000_001;
    let result = bounded_power(
        PowerModel::Anova {
            groups: 3,
            effect_f: (2.0 / (3.0 * sample_size as f64)).sqrt(),
        },
        PowerOptions {
            alpha: 0.05,
            alternative: PowerAlternative::Greater,
            request: PowerRequest::Power { sample_size },
        },
    );
    // Independent finite-denominator integer-Beta/Poisson sum at lambda=2.
    assert!((result.power - 0.225_544_915_769_563_56).abs() < 1e-11);
}

#[test]
fn noncentral_f_power_retains_shift_with_large_balanced_degrees() {
    let sample_size = 1_000_000_000_000_001;
    let result = bounded_power(
        PowerModel::LinearRegression {
            predictors: 500_000_000_000_000,
            effect_f_squared: 2.0 / sample_size as f64,
        },
        PowerOptions {
            alpha: 0.05,
            alternative: PowerAlternative::Greater,
            request: PowerRequest::Power { sample_size },
        },
    );
    // Symmetric finite-Beta expansion; omitted noncentral terms are below 2e-15.
    let expected = 0.050_000_004_612_366_23;
    assert!(
        (result.power - expected).abs() < 1e-12,
        "{} != {expected}",
        result.power
    );
}

#[test]
fn noncentral_t_power_retains_large_degrees() {
    let sample_size = 1_000_000_000_000_001;
    let result = compute(
        PowerModel::TMean {
            standardized_effect: 2.0 / (sample_size as f64).sqrt(),
            design: MeanDesign::OneSample,
        },
        PowerOptions {
            alpha: 0.05,
            alternative: PowerAlternative::TwoSided,
            request: PowerRequest::Power { sample_size },
        },
        &control(),
    )
    .unwrap();
    // Independent 110-digit noncentral-t/Beta series at df=1e15 and delta=2.
    let expected = 0.516_005_273_976_174;
    assert!((result.power - expected).abs() < 2e-12);
}

#[test]
fn noncentral_t_power_retains_small_central_thresholds() {
    let sample_size = 1_000_000_000_000_001;
    let result = compute(
        PowerModel::TMean {
            standardized_effect: 2.0 / (sample_size as f64).sqrt(),
            design: MeanDesign::OneSample,
        },
        PowerOptions {
            alpha: 0.8,
            alternative: PowerAlternative::TwoSided,
            request: PowerRequest::Power { sample_size },
        },
        &control(),
    )
    .unwrap();
    // The old Beta argument rounded to one and falsely reported power one.
    let expected = 0.971_769_853_179_994_5;
    // Include the retained native Normal tail's error in the reference tolerance.
    assert!(
        (result.power - expected).abs() < 1e-11,
        "{} != {expected}",
        result.power
    );
}

#[test]
fn null_t_power_retains_alpha_without_allocating_large_sample_data() {
    // Tail, zero, central and reflected-central thresholds exercise distinct branches.
    for (design, alternative, alpha) in [
        (MeanDesign::OneSample, PowerAlternative::TwoSided, 0.05),
        (MeanDesign::Independent, PowerAlternative::Greater, 0.5),
        (MeanDesign::OneSample, PowerAlternative::TwoSided, 0.8),
        (MeanDesign::Independent, PowerAlternative::Less, 0.8),
    ] {
        let result = compute(
            PowerModel::TMean {
                standardized_effect: 0.0,
                design,
            },
            PowerOptions {
                alpha,
                alternative,
                request: PowerRequest::Power {
                    sample_size: 1_000_000_000_000_001,
                },
            },
            &control(),
        )
        .unwrap();
        // A central t test's rejection probability under its null is alpha.
        assert!(
            (result.power - alpha).abs() < 1e-12,
            "{} != {alpha}",
            result.power
        );
    }
}

fn models() -> Vec<(&'static str, PowerModel, PowerAlternative)> {
    use PowerAlternative::*;
    use PowerModel::*;
    vec![
        (
            "normal",
            NormalMean {
                standardized_effect: 0.5,
            },
            TwoSided,
        ),
        (
            "t_one",
            TMean {
                standardized_effect: 0.5,
                design: MeanDesign::OneSample,
            },
            TwoSided,
        ),
        (
            "t_two",
            TMean {
                standardized_effect: 0.5,
                design: MeanDesign::Independent,
            },
            TwoSided,
        ),
        (
            "t_one",
            TMean {
                standardized_effect: 0.5,
                design: MeanDesign::Paired,
            },
            TwoSided,
        ),
        (
            "variance",
            Variance {
                variance_ratio: 1.5,
            },
            TwoSided,
        ),
        (
            "proportion",
            Proportion {
                null_proportion: 0.5,
                proportion: 0.6,
            },
            TwoSided,
        ),
        (
            "proportion_difference",
            ProportionDifference {
                proportion1: 0.6,
                proportion2: 0.4,
            },
            TwoSided,
        ),
        (
            "correlation",
            Correlation {
                null_correlation: 0.,
                correlation: 0.3,
            },
            TwoSided,
        ),
        (
            "anova",
            Anova {
                groups: 3,
                effect_f: 0.25,
            },
            Greater,
        ),
        (
            "linear",
            LinearRegression {
                predictors: 3,
                effect_f_squared: 0.15,
            },
            Greater,
        ),
        (
            "poisson",
            PoissonRate {
                baseline_rate: 1.,
                rate_ratio: 1.5,
                exposure: 1.,
            },
            TwoSided,
        ),
        (
            "logistic",
            Logistic {
                baseline_probability: 0.2,
                odds_ratio: 1.5,
            },
            TwoSided,
        ),
        (
            "survival",
            Survival {
                hazard_ratio: 0.7,
                event_fraction: 0.5,
                predictor_variance: 0.25,
            },
            TwoSided,
        ),
        (
            "cluster",
            ClusterRandomized {
                standardized_effect: 0.5,
                cluster_size: 20,
                intraclass_correlation: 0.05,
            },
            TwoSided,
        ),
        (
            "noninferiority",
            Noninferiority {
                standardized_difference: 0.,
                margin: 0.3,
                higher_is_better: true,
            },
            Greater,
        ),
        (
            "equivalence",
            Equivalence {
                standardized_difference: 0.05,
                margin: 0.3,
            },
            TwoSided,
        ),
    ]
}
#[test]
fn power_and_minimum_integer_sample_sizes_match_independent_references() {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/power_reference.json")).unwrap();
    let c = control();
    for (name, model, alternative) in models() {
        let options = PowerOptions {
            alpha: 0.05,
            alternative,
            request: PowerRequest::Power { sample_size: 100 },
        };
        let result = compute(model, options, &c).unwrap();
        close(result.power, f[name]["power"].as_f64().unwrap());
        let result = compute(
            model,
            PowerOptions {
                request: PowerRequest::SampleSize { target_power: 0.8 },
                ..options
            },
            &c,
        )
        .unwrap();
        assert_eq!(
            result.sample_size,
            f[name]["required"].as_u64().unwrap() as usize,
            "{name}"
        );
        close(result.power, f[name]["achieved"].as_f64().unwrap());
        assert!(result.power >= 0.8);
        let previous = compute(
            model,
            PowerOptions {
                request: PowerRequest::Power {
                    sample_size: result.sample_size - 1,
                },
                ..options
            },
            &c,
        )
        .unwrap();
        assert!(previous.power < 0.8);
    }
}
#[test]
fn power_directions_null_effects_invalid_targets_and_control_are_explicit() {
    let c = control();
    let options = PowerOptions {
        alpha: 0.05,
        alternative: PowerAlternative::Greater,
        request: PowerRequest::Power { sample_size: 640 },
    };
    let null = compute(
        PowerModel::TMean {
            standardized_effect: 0.,
            design: MeanDesign::Independent,
        },
        options,
        &c,
    )
    .unwrap();
    close(null.power, 0.05);
    let lower = compute(
        PowerModel::TMean {
            standardized_effect: -0.15,
            design: MeanDesign::Independent,
        },
        PowerOptions {
            alternative: PowerAlternative::Less,
            ..options
        },
        &c,
    )
    .unwrap();
    let upper = compute(
        PowerModel::TMean {
            standardized_effect: 0.15,
            design: MeanDesign::Independent,
        },
        options,
        &c,
    )
    .unwrap();
    close(lower.power, upper.power);
    assert_eq!(upper.total_observations, 1280);
    assert!(
        compute(
            PowerModel::NormalMean {
                standardized_effect: 0.
            },
            PowerOptions {
                request: PowerRequest::SampleSize { target_power: 0.8 },
                ..options
            },
            &c
        )
        .is_err()
    );
    assert!(
        compute(
            PowerModel::Equivalence {
                standardized_difference: 0.6,
                margin: 0.3
            },
            PowerOptions {
                alternative: PowerAlternative::TwoSided,
                request: PowerRequest::SampleSize { target_power: 0.8 },
                ..options
            },
            &c
        )
        .is_err()
    );
    c.cancellation.cancel();
    assert!(matches!(
        compute(
            PowerModel::NormalMean {
                standardized_effect: 0.5
            },
            options,
            &c
        ),
        Err(ScientificComputationError::Cancelled)
    ));
}
