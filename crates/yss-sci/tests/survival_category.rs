use serde_json::Value;
use std::time::{Duration, Instant};
use yss_sci::survival::{cox, evaluation, nonparametric, parametric};
use yss_sci_contract::{execution::*, regression::models::IterationOptions, survival::*};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/survival_category_reference.json")).unwrap()
}
fn vector(v: &Value) -> Vec<f64> {
    serde_json::from_value(v.clone()).unwrap()
}
fn matrix(v: &Value) -> Vec<Vec<f64>> {
    serde_json::from_value(v.clone()).unwrap()
}
fn codes(v: &Value) -> Vec<usize> {
    serde_json::from_value(v.clone()).unwrap()
}
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(60),
    }
}

#[test]
fn survival_observation_failures_retain_data_and_shape_roles() {
    for (time, event, groups, violation) in [
        (
            vec![0., 2.],
            vec![1., 0.],
            vec![0, 0],
            ScientificInputViolation::DataOutOfRange,
        ),
        (
            vec![1., 2.],
            vec![1., 2.],
            vec![0, 0],
            ScientificInputViolation::DataOutOfRange,
        ),
        (
            vec![1., 2.],
            vec![1., f64::NAN],
            vec![0, 0],
            ScientificInputViolation::NonFiniteInput,
        ),
        (
            vec![1., 2.],
            vec![1., 0.],
            vec![0],
            ScientificInputViolation::ShapeMismatch,
        ),
    ] {
        assert_eq!(
            nonparametric::curves(&time, &event, &groups, CurveMethod::KaplanMeier, &control())
                .unwrap_err(),
            ScientificComputationError::InvalidInput { violation },
        );
    }
}

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol * (1.0 + b.abs()), "{a} != {b}");
}
fn compare(actual: &[f64], expected: &[f64], tol: f64) {
    assert_eq!(actual.len(), expected.len());
    for (&a, &b) in actual.iter().zip(expected) {
        close(a, b, tol);
    }
}
fn model(actual: &CoxResult, expected: &Value) {
    compare(
        &actual
            .coefficients
            .iter()
            .map(|c| c.estimate)
            .collect::<Vec<_>>(),
        &vector(&expected["coefficients"]),
        1e-7,
    );
    for (a, b) in actual
        .covariance
        .iter()
        .zip(matrix(&expected["covariance"]))
    {
        compare(a, &b, 1e-7);
    }
    close(
        actual.log_likelihood,
        expected["log_likelihood"].as_f64().unwrap(),
        1e-9,
    );
}

#[test]
fn curves_logrank_and_competing_risks_handle_censoring_and_ties() {
    let f = fixture();
    let d = &f["nonparametric"];
    let time = vector(&d["time"]);
    let event = vector(&d["event"]);
    let r = nonparametric::curves(
        &time,
        &event,
        &vec![0; time.len()],
        CurveMethod::KaplanMeier,
        &control(),
    )
    .unwrap();
    let points = &r.curves[0].points;
    for (j, t) in vector(&d["km_times"]).iter().enumerate() {
        let p = points.iter().find(|p| p.time == *t).unwrap();
        close(p.survival, vector(&d["km"])[j], 1e-12);
        close(p.standard_error, vector(&d["km_se"])[j], 1e-12);
        assert!(p.confidence_interval[0] <= p.survival && p.confidence_interval[1] >= p.survival);
    }
    let p = points.iter().find(|p| p.time == 2.0).unwrap();
    assert_eq!((p.at_risk, p.events, p.censored), (7, 1, 1));
    let na = nonparametric::curves(
        &time,
        &event,
        &vec![0; time.len()],
        CurveMethod::NelsonAalen,
        &control(),
    )
    .unwrap();
    close(
        na.curves[0].points[1].estimate,
        1.0 / 8.0 + 1.0 / 7.0,
        1e-12,
    );
    close(
        na.curves[0].points[1].standard_error,
        (1.0_f64 / 64.0 + 1.0 / 49.0).sqrt(),
        1e-12,
    );
    let groups = codes(&d["group"])
        .into_iter()
        .map(|g| usize::MAX - 1 + g)
        .collect::<Vec<_>>();
    let lr = nonparametric::logrank(&time, &event, &groups, &control()).unwrap();
    assert_eq!(lr.groups, vec![usize::MAX - 1, usize::MAX]);
    close(lr.test.statistic, d["logrank"][0].as_f64().unwrap(), 1e-12);
    close(lr.test.p_value, d["logrank"][1].as_f64().unwrap(), 1e-12);
    let grouped = nonparametric::curves(
        &[3.0, 2.0, 1.0, 4.0],
        &[1.0, 1.0, 0.0, 0.0],
        &[usize::MAX, usize::MAX - 1, usize::MAX, usize::MAX - 1],
        CurveMethod::KaplanMeier,
        &control(),
    )
    .unwrap();
    assert_eq!(
        grouped.curves.iter().map(|c| c.group).collect::<Vec<_>>(),
        vec![usize::MAX - 1, usize::MAX]
    );
    for c in &grouped.curves {
        assert_eq!((c.observations, c.events), (2, 1));
    }
    compare(
        &grouped.curves[0]
            .points
            .iter()
            .map(|p| p.survival)
            .collect::<Vec<_>>(),
        &[0.5, 0.5],
        1e-12,
    );
    compare(
        &grouped.curves[1]
            .points
            .iter()
            .map(|p| p.survival)
            .collect::<Vec<_>>(),
        &[1.0, 0.0],
        1e-12,
    );
    let cif = nonparametric::competing_risks(&time, &[5, 9, 0, 5, 0, 9, 5, 0], &control()).unwrap();
    assert_eq!(cif.causes, vec![5, 9]);
    for (j, t) in vector(&d["cif_times"]).iter().enumerate() {
        let p = cif.points.iter().find(|p| p.time == *t).unwrap();
        for k in 0..2 {
            close(
                p.cumulative_incidence[k],
                d["cif"][k][j].as_f64().unwrap(),
                1e-12,
            );
        }
        close(
            p.survival + p.cumulative_incidence.iter().sum::<f64>(),
            1.0,
            1e-12,
        );
    }
    let censored = nonparametric::curves(
        &[1.0, 2.0],
        &[0.0, 0.0],
        &[7, 7],
        CurveMethod::KaplanMeier,
        &control(),
    )
    .unwrap();
    assert_eq!(censored.curves[0].median_survival, None);
    assert_eq!(censored.curves[0].points[1].confidence_interval, [1.0, 1.0]);
}

#[test]
fn cox_and_counting_process_risk_sets_match_phreg() {
    let f = fixture();
    let d = &f["data"];
    let time = vector(&d["time"]);
    let event = vector(&d["event"]);
    let x = matrix(&d["predictors"]);
    for (ties, key) in [(CoxTies::Efron, "efron"), (CoxTies::Breslow, "breslow")] {
        let r = cox::fit(
            &time,
            &event,
            &x,
            CoxOptions {
                ties,
                ..Default::default()
            },
            &control(),
        )
        .unwrap();
        model(&r, &f["cox"][key]);
        if ties == CoxTies::Efron {
            compare(
                &r.baselines[0]
                    .points
                    .iter()
                    .map(|p| p.cumulative_hazard)
                    .collect::<Vec<_>>(),
                &vector(&f["cox"][key]["baseline"]),
                1e-7,
            );
            compare(
                &cox::event_probabilities(&r, 2.0).unwrap(),
                &vector(&f["cox"][key]["risk"]),
                1e-7,
            );
        }
    }
    let td = &f["time_dependent"];
    let r = cox::time_dependent(
        &vector(&td["start"]),
        &vector(&td["stop"]),
        &vector(&td["event"]),
        &codes(&td["subjects"])
            .into_iter()
            .map(|subject| usize::MAX - subject)
            .collect::<Vec<_>>(),
        &matrix(&td["predictors"]),
        CoxOptions::default(),
        &control(),
    )
    .unwrap();
    model(&r, &td["reference"]);
    // Splitting every subject at exactly another subject's failure time must not double-count risk.
    let mut start = vec![];
    let mut stop = vec![];
    let mut e = vec![];
    let mut ids = vec![];
    let mut xs = vec![vec![], vec![]];
    for i in 0..time.len() {
        let intervals = if time[i] > 1.0 {
            vec![(0.0, 1.0, 0.0), (1.0, time[i], event[i])]
        } else {
            vec![(0.0, time[i], event[i])]
        };
        for (a, b, c) in intervals {
            start.push(a);
            stop.push(b);
            e.push(c);
            ids.push(i);
            for j in 0..2 {
                xs[j].push(x[j][i]);
            }
        }
    }
    let split = cox::time_dependent(
        &start,
        &stop,
        &e,
        &ids,
        &xs,
        CoxOptions::default(),
        &control(),
    )
    .unwrap();
    model(&split, &f["cox"]["efron"]);
}

#[test]
fn all_parametric_likelihoods_and_full_information_match_scipy() {
    let f = fixture();
    let d = &f["data"];
    for (distribution, key) in [
        (AftDistribution::Exponential, "exponential"),
        (AftDistribution::Weibull, "weibull"),
        (AftDistribution::Lognormal, "lognormal"),
        (AftDistribution::Loglogistic, "loglogistic"),
    ] {
        let r = parametric::fit(
            &vector(&d["time"]),
            &vector(&d["event"]),
            &matrix(&d["predictors"]),
            AftOptions {
                distribution,
                iteration: IterationOptions::default(),
                horizon: 2.0,
            },
            &control(),
        )
        .unwrap();
        let e = &f["aft"][key];
        compare(
            &r.coefficients
                .iter()
                .map(|c| c.estimate)
                .collect::<Vec<_>>(),
            &vector(&e["coefficients"]),
            2e-5,
        );
        for (a, b) in r.covariance.iter().zip(matrix(&e["covariance"])) {
            compare(a, &b, 2e-5);
        }
        close(r.scale, e["scale"].as_f64().unwrap(), 2e-5);
        close(
            r.log_likelihood,
            e["log_likelihood"].as_f64().unwrap(),
            1e-8,
        );
        compare(&r.median_survival, &vector(&e["median"]), 2e-5);
        compare(&r.event_probabilities, &vector(&e["risk"]), 2e-5);
    }
    let exp = parametric::fit(
        &[1.0, 2.0, 3.0, 4.0],
        &[1.0, 0.0, 1.0, 0.0],
        &[],
        AftOptions {
            distribution: AftDistribution::Exponential,
            iteration: IterationOptions::default(),
            horizon: 1.0,
        },
        &control(),
    )
    .unwrap();
    close(exp.coefficients[0].estimate, 5.0_f64.ln(), 1e-6);
    close(
        exp.coefficients[0].standard_error.unwrap(),
        (0.5_f64).sqrt(),
        1e-5,
    );
}

#[test]
fn subgroup_uses_stratified_baselines_and_joint_contrast_covariance() {
    let f = fixture();
    let d = &f["data"];
    let r = cox::subgroup(
        &vector(&d["time"]),
        &vector(&d["event"]),
        &vector(&d["treatment"]),
        &codes(&d["group"])
            .into_iter()
            .map(|g| usize::MAX - 1 + g)
            .collect::<Vec<_>>(),
        &[matrix(&d["predictors"])[0].clone()],
        CoxOptions::default(),
        &control(),
    )
    .unwrap();
    model(&r.model, &f["subgroup"]);
    assert_eq!(r.groups, vec![usize::MAX - 1, usize::MAX]);
    close(
        r.equality_test.statistic,
        f["subgroup"]["statistic"].as_f64().unwrap(),
        1e-7,
    );
    assert_eq!(r.model.baselines.len(), 2);
    assert_eq!(r.equality_test.degrees_of_freedom, 1);
}

#[test]
fn calibration_decision_curves_and_nomogram_use_the_same_horizon() {
    let time = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let event = [1.0; 6];
    let risk = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
    for (predicted, violation) in [
        (vec![0.2], ScientificInputViolation::ShapeMismatch),
        (vec![f64::NAN; 6], ScientificInputViolation::NonFiniteInput),
        (vec![1.2; 6], ScientificInputViolation::DataOutOfRange),
    ] {
        assert_eq!(
            evaluation::calibration(&time, &event, &predicted, 2.0, 2, &control()).unwrap_err(),
            ScientificComputationError::InvalidInput { violation },
        );
    }
    assert_eq!(
        evaluation::calibration(&time, &event, &risk, 2.0, 1, &control()).unwrap_err(),
        ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::ParameterOutOfRange,
        },
    );
    let r = evaluation::calibration(&time, &event, &risk, 2.0, 2, &control()).unwrap();
    assert_eq!(r.bins.len(), 2);
    close(r.bins[0].observed_risk, 2.0 / 3.0, 1e-12);
    close(r.bins[1].observed_risk, 0.0, 1e-12);
    let tied = evaluation::calibration(&time, &event, &[0.2; 6], 2.0, 3, &control()).unwrap();
    assert_eq!(tied.bins.len(), 1);
    let d = evaluation::decision_curve(
        &time,
        &event,
        &risk,
        DecisionOptions {
            horizon: 2.0,
            minimum_threshold: 0.3,
            maximum_threshold: 0.7,
            points: 3,
        },
        &control(),
    )
    .unwrap();
    close(d.estimates[0].model, -(4.0 / 6.0) * 0.3 / 0.7, 1e-12);
    close(
        d.estimates[0].treat_all,
        1.0 / 3.0 - (2.0 / 3.0) * 0.3 / 0.7,
        1e-12,
    );
    close(d.estimates[2].model, 0.0, 1e-12);
    let f = fixture();
    let data = &f["data"];
    let model = cox::fit(
        &vector(&data["time"]),
        &vector(&data["event"]),
        &matrix(&data["predictors"]),
        CoxOptions::default(),
        &control(),
    )
    .unwrap();
    let plot = evaluation::nomogram(&model, 2.0, 5, &control()).unwrap();
    assert_eq!(plot.axes.len(), model.coefficients.len() + 3);
    let origin = model
        .coefficients
        .iter()
        .enumerate()
        .map(|(j, c)| {
            let [lo, hi] = model.predictor_ranges[j];
            (c.estimate * (lo - model.predictor_means[j]))
                .min(c.estimate * (hi - model.predictor_means[j]))
        })
        .sum::<f64>();
    let label = &plot.axes.last().unwrap().ticks[0].label;
    close(
        label.parse().unwrap(),
        (-plot.baseline_cumulative_hazard * origin.exp()).exp(),
        5e-5,
    );
    for (horizon, ticks) in [(100.0, 5), (2.0, 1), (0.1, 5)] {
        assert_eq!(
            evaluation::nomogram(&model, horizon, ticks, &control()).unwrap_err(),
            ScientificComputationError::InvalidInput {
                violation: ScientificInputViolation::ParameterOutOfRange,
            },
        );
    }
    let mut malformed = model.clone();
    malformed.predictor_means.clear();
    assert_eq!(
        evaluation::nomogram(&malformed, 2.0, 5, &control()).unwrap_err(),
        ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::ShapeMismatch,
        },
    );
    let mut malformed = model.clone();
    malformed.coefficients[0].estimate = f64::NAN;
    assert_eq!(
        evaluation::nomogram(&malformed, 2.0, 5, &control()).unwrap_err(),
        ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::NonFiniteInput,
        },
    );
    let mut malformed = model.clone();
    malformed.predictor_ranges[0] = [2.0, 1.0];
    assert_eq!(
        evaluation::nomogram(&malformed, 2.0, 5, &control()).unwrap_err(),
        ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::DataOutOfRange,
        },
    );
    let mut malformed = model;
    malformed.baselines.clear();
    assert_eq!(
        cox::event_probabilities(&malformed, 2.0).unwrap_err(),
        ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::ShapeMismatch,
        },
    );
}

#[test]
fn invalid_censoring_intervals_identification_and_execution_controls_are_rejected() {
    let c = control();
    assert!(nonparametric::logrank(&[1.0, 2.0], &[0.0, 0.0], &[0, 1], &c).is_err());
    assert_eq!(
        cox::fit(
            &[1.0, 2.0, 3.0],
            &[1.0; 3],
            &[vec![1.0; 3]],
            CoxOptions::default(),
            &c
        )
        .unwrap_err(),
        ScientificComputationError::ComputationFailed,
    );
    for (start, event, subjects, violation) in [
        (
            vec![0.0],
            vec![0.0, 1.0],
            vec![7, 7],
            ScientificInputViolation::ShapeMismatch,
        ),
        (
            vec![0.0, 1.0],
            vec![0.0, 1.0],
            vec![7],
            ScientificInputViolation::ShapeMismatch,
        ),
        (
            vec![0.0, f64::NAN],
            vec![0.0, 1.0],
            vec![7, 7],
            ScientificInputViolation::NonFiniteInput,
        ),
        (
            vec![0.0, 0.5],
            vec![0.0, 1.0],
            vec![7, 7],
            ScientificInputViolation::DataOutOfRange,
        ),
        (
            vec![0.0, 1.0],
            vec![1.0, 1.0],
            vec![7, 7],
            ScientificInputViolation::DataOutOfRange,
        ),
        (
            vec![0.0, 2.0],
            vec![0.0, 1.0],
            vec![7, 8],
            ScientificInputViolation::DataOutOfRange,
        ),
    ] {
        assert_eq!(
            cox::time_dependent(
                &start,
                &[1.0, 2.0],
                &event,
                &subjects,
                &[vec![0.0, 1.0]],
                CoxOptions::default(),
                &c,
            )
            .unwrap_err(),
            ScientificComputationError::InvalidInput { violation }
        );
    }
    for (treatment, violation) in [
        (vec![0.0], ScientificInputViolation::ShapeMismatch),
        (
            vec![0.0, f64::NAN],
            ScientificInputViolation::NonFiniteInput,
        ),
        (vec![0.0, 2.0], ScientificInputViolation::DataOutOfRange),
    ] {
        assert_eq!(
            cox::subgroup(
                &[1.0, 2.0],
                &[1.0, 0.0],
                &treatment,
                &[0, 1],
                &[],
                CoxOptions::default(),
                &c
            )
            .unwrap_err(),
            ScientificComputationError::InvalidInput { violation },
        );
    }
    let data_error = ScientificComputationError::InvalidInput {
        violation: ScientificInputViolation::DataOutOfRange,
    };
    assert_eq!(
        nonparametric::logrank(&[1.0, 2.0], &[1.0, 0.0], &[0, 0], &c).unwrap_err(),
        data_error
    );
    assert_eq!(
        nonparametric::competing_risks(&[1.0, 2.0], &[0, 0], &c).unwrap_err(),
        data_error
    );
    assert_eq!(
        cox::fit(
            &[1.0, 2.0],
            &[0.0, 0.0],
            &[vec![0.0, 1.0]],
            CoxOptions::default(),
            &c
        )
        .unwrap_err(),
        data_error
    );
    assert_eq!(
        parametric::fit(
            &[1.0, 2.0],
            &[0.0, 0.0],
            &[],
            AftOptions {
                distribution: AftDistribution::Exponential,
                horizon: 1.0,
                iteration: IterationOptions::default()
            },
            &c
        )
        .unwrap_err(),
        data_error
    );
    assert_eq!(
        cox::proportional_hazards(
            &[1.0; 3],
            &[1.0; 3],
            &[vec![0.0, 1.0, 2.0]],
            CoxOptions::default(),
            PhTimeTransform::Identity,
            &c
        )
        .unwrap_err(),
        data_error
    );
    assert_eq!(
        cox::fit(&[1.0, 2.0], &[1.0, 0.0], &[], CoxOptions::default(), &c).unwrap_err(),
        ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::ShapeMismatch
        }
    );
    assert_eq!(
        evaluation::calibration(&[1.0, 2.0], &[0.0, 0.0], &[0.2, 0.3], 3.0, 2, &c).unwrap_err(),
        ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::ParameterOutOfRange
        }
    );
    c.cancellation.cancel();
    assert_eq!(
        nonparametric::curves(&[1.0], &[1.0], &[0], CurveMethod::KaplanMeier, &c).unwrap_err(),
        ScientificComputationError::Cancelled
    );
    let expired = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() - Duration::from_secs(1),
    };
    assert_eq!(
        nonparametric::competing_risks(&[1.0], &[1], &expired).unwrap_err(),
        ScientificComputationError::DeadlineExceeded
    );
}
