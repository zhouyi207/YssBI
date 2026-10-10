use serde_json::Value;
use std::time::{Duration, Instant};
use yss_sci::meta::{diagnostics, effects, model, plots};
use yss_sci_contract::{execution::*, meta::*};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/meta_reference.json")).unwrap()
}
fn numbers(v: &Value) -> Vec<f64> {
    serde_json::from_value(v.clone()).unwrap()
}
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 2e-8 * (1. + b.abs()), "{a} != {b}");
}
fn options(case: &Value) -> MetaOptions {
    MetaOptions {
        estimator: match case["method"].as_str().unwrap() {
            "fixed" => MetaEstimator::Fixed,
            "dl" => MetaEstimator::DerSimonianLaird,
            "pm" => MetaEstimator::PauleMandel,
            _ => unreachable!(),
        },
        inference: if case["kh"] == true {
            MetaInference::KnappHartung
        } else {
            MetaInference::Wald
        },
        confidence_level: 0.9,
    }
}

#[test]
fn meta_effect_measures_match_independent_reference_and_transform_conventions() {
    let f = fixture();
    let d = &f["continuous"];
    let arms: Vec<Vec<f64>> = serde_json::from_value(d["inputs"].clone()).unwrap();
    let arm = |offset: usize| ArmSummary {
        mean: &arms[offset],
        sd: &arms[offset + 1],
        size: &arms[offset + 2],
    };
    for (measure, key) in [
        (EffectMeasure::MeanDifference, "md"),
        (EffectMeasure::HedgesG, "g"),
    ] {
        let result = effects::continuous(arm(0), arm(3), measure, 0.9, &control()).unwrap();
        for (i, row) in result.rows.iter().enumerate() {
            close(row.effect, d[key][i].as_f64().unwrap());
            close(
                row.variance,
                d[format!("{key}_variance")][i].as_f64().unwrap(),
            );
            close(
                row.upper - row.effect,
                1.6448536269514722 * row.standard_error,
            );
        }
    }
    let inputs: Vec<Vec<f64>> = serde_json::from_value(f["binary_inputs"].clone()).unwrap();
    for (measure, key) in [
        (EffectMeasure::LogOddsRatio, "log_odds_ratio"),
        (EffectMeasure::LogRiskRatio, "log_risk_ratio"),
        (EffectMeasure::RiskDifference, "risk_difference"),
    ] {
        let result = effects::binary(
            BinomialSummary {
                events: &inputs[0],
                total: &inputs[1],
            },
            BinomialSummary {
                events: &inputs[2],
                total: &inputs[3],
            },
            measure,
            0.5,
            0.95,
            &control(),
        )
        .unwrap();
        for (i, row) in result.rows.iter().enumerate() {
            close(row.effect, f["binary"][key]["effect"][i].as_f64().unwrap());
            close(
                row.variance,
                f["binary"][key]["variance"][i].as_f64().unwrap(),
            );
        }
    }
    let p = &f["proportion"];
    for (measure, key) in [
        (EffectMeasure::Proportion, "proportion"),
        (EffectMeasure::LogitProportion, "logit"),
        (EffectMeasure::ArcsineProportion, "arcsine"),
    ] {
        let result = effects::proportion(
            BinomialSummary {
                events: &numbers(&p["events"]),
                total: &numbers(&p["totals"]),
            },
            measure,
            0.5,
            0.95,
            &control(),
        )
        .unwrap();
        for (i, row) in result.rows.iter().enumerate() {
            close(row.effect, p[key][i].as_f64().unwrap());
        }
        if measure == EffectMeasure::LogitProportion {
            for (i, row) in result.rows.iter().enumerate() {
                close(row.variance, p["logit_variance"][i].as_f64().unwrap());
            }
        }
    }
    let mean = effects::mean(arm(0), 0.95, &control()).unwrap();
    close(mean.rows[0].effect, 6.2);
    close(mean.rows[0].variance, 1.44 / 20.);
    let constant_arm = ArmSummary {
        mean: &[5.],
        sd: &[0.],
        size: &[20.],
    };
    let varied_arm = ArmSummary {
        mean: &[4.],
        sd: &[1.],
        size: &[20.],
    };
    close(
        effects::continuous(
            constant_arm,
            varied_arm,
            EffectMeasure::MeanDifference,
            0.95,
            &control(),
        )
        .unwrap()
        .rows[0]
            .variance,
        0.05,
    );
    let r = effects::correlation(&[0.6, -0.3], &[23., 103.], 0.95, &control()).unwrap();
    close(r.rows[0].effect, 0.6_f64.atanh());
    close(r.rows[0].variance, 0.05);
    let log = effects::ratio(&[2.], &[1.], &[4.], 0.95, 0.95, &control()).unwrap();
    close(log.rows[0].effect, 2.0_f64.ln());
    close(log.rows[0].lower, 0.);
    close(log.rows[0].upper, 4.0_f64.ln());
}

#[test]
fn meta_pooling_regression_and_sensitivity_match_statsmodels_and_scipy() {
    let f = fixture();
    let (y, v) = (numbers(&f["y"]), numbers(&f["variances"]));
    for case in f["pooling"].as_array().unwrap() {
        let fit = model::fit(&y, &v, &[], options(case), &control()).unwrap();
        let summary = model::summary(&y, &v, &[], options(case), &control()).unwrap();
        assert_eq!(
            serde_json::to_value(&summary).unwrap(),
            serde_json::to_value(&fit.summary).unwrap()
        );
        let heterogeneity =
            model::heterogeneity(&y, &v, options(case).estimator, &control()).unwrap();
        close(heterogeneity.q, case["q"].as_f64().unwrap());
        close(heterogeneity.tau_squared, case["tau"].as_f64().unwrap());
        let c = &fit.summary.coefficients[0];
        close(c.estimate, case["estimate"].as_f64().unwrap());
        close(c.standard_error.unwrap(), case["se"].as_f64().unwrap());
        close(c.p_value.unwrap(), case["p"].as_f64().unwrap());
        for (a, b) in c
            .confidence_interval
            .unwrap()
            .iter()
            .zip(numbers(&case["ci"]))
        {
            close(*a, b);
        }
        close(
            fit.summary.heterogeneity.tau_squared,
            case["tau"].as_f64().unwrap(),
        );
        close(fit.summary.heterogeneity.q, case["q"].as_f64().unwrap());
        close(
            fit.summary.heterogeneity.i_squared_percent,
            case["i2"].as_f64().unwrap(),
        );
        for (row, b) in fit.studies.iter().zip(numbers(&case["weights"])) {
            close(row.weight, b);
        }
    }
    for case in f["regression"].as_array().unwrap() {
        let fit = model::fit(
            &y,
            &v,
            &[numbers(&f["moderator"])],
            options(case),
            &control(),
        )
        .unwrap();
        for (i, c) in fit.summary.coefficients.iter().enumerate() {
            close(c.estimate, case["beta"][i].as_f64().unwrap());
            for (j, a) in fit.summary.covariance[i].iter().enumerate() {
                close(*a, case["covariance"][i][j].as_f64().unwrap());
            }
        }
        close(
            fit.summary.heterogeneity.tau_squared,
            case["tau"].as_f64().unwrap(),
        );
        close(fit.summary.heterogeneity.q, case["q"].as_f64().unwrap());
        close(fit.summary.residual_q, case["residual_q"].as_f64().unwrap());
        for (row, b) in fit.studies.iter().zip(numbers(&case["fitted"])) {
            close(row.fitted, b);
        }
    }
    let opt = MetaOptions {
        confidence_level: 0.9,
        ..Default::default()
    };
    let (summary, rows) = diagnostics::sensitivity(&y, &v, opt, &control()).unwrap();
    assert_eq!(summary.alternative_models.len(), 3);
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.omitted_study, i + 1);
        close(
            row.estimate,
            f["omissions"][i]["estimate"].as_f64().unwrap(),
        );
        close(row.tau_squared, f["omissions"][i]["tau"].as_f64().unwrap());
    }
    let forest = plots::forest(&y, &v, opt, true, &control()).unwrap();
    close(
        forest.data.last().unwrap().value,
        summary.baseline.coefficients[0].estimate.exp(),
    );
    assert_eq!(forest.data.len(), y.len() + 1);
    let funnel = plots::funnel(&y, &v, opt.estimator, opt.confidence_level, &control()).unwrap();
    assert_eq!(funnel.plot.reference_lines.len(), 3);
    close(
        funnel.plot.reference_lines[1].start.x,
        summary.baseline.coefficients[0].estimate,
    );
    assert!(funnel.y_domain[0] > 0.);
    assert_eq!(funnel.y_domain[1], 0.);
}

#[test]
fn meta_asymmetry_and_p_combination_match_reference_distributions() {
    let f = fixture();
    let (y, v) = (numbers(&f["y"]), numbers(&f["variances"]));
    let egger = diagnostics::egger(&y, &v, 0.95, &control()).unwrap();
    for (i, c) in egger.coefficients.iter().enumerate() {
        close(c.estimate, f["egger"]["beta"][i].as_f64().unwrap());
        close(
            c.standard_error.unwrap(),
            f["egger"]["se"][i].as_f64().unwrap(),
        );
        close(c.p_value.unwrap(), f["egger"]["p"][i].as_f64().unwrap());
    }
    let begg = diagnostics::begg(&y, &v, &control()).unwrap();
    close(begg.coefficient, f["begg"]["tau"].as_f64().unwrap());
    close(
        begg.inference.p_value.unwrap(),
        f["begg"]["p"].as_f64().unwrap(),
    );
    for (stouffer, key) in [(false, "fisher"), (true, "stouffer")] {
        let result = diagnostics::combine_p(
            &numbers(&f["p_values"]),
            Some(&numbers(&f["p_weights"])),
            stouffer,
            &control(),
        )
        .unwrap();
        close(result.statistic.unwrap(), f[key][0].as_f64().unwrap());
        close(result.p_value, f[key][1].as_f64().unwrap());
    }
    assert_eq!(
        diagnostics::combine_p(&[0., 0.2], None, false, &control())
            .unwrap()
            .p_value,
        0.
    );
    assert!(diagnostics::combine_p(&[0., 1.], None, true, &control()).is_err());
}

#[test]
fn meta_zero_heterogeneity_does_not_require_an_unrepresentable_moment_denominator() {
    let variance = 1e-308;
    for estimator in [
        MetaEstimator::Fixed,
        MetaEstimator::DerSimonianLaird,
        MetaEstimator::PauleMandel,
    ] {
        let result = model::fit(
            &[0.0; 4],
            &[variance; 4],
            &[],
            MetaOptions {
                estimator,
                ..Default::default()
            },
            &control(),
        )
        .unwrap();
        assert_eq!(result.summary.heterogeneity.q, 0.0);
        assert_eq!(result.summary.heterogeneity.tau_squared, 0.0);
        assert_eq!(result.summary.coefficients[0].estimate, 0.0);
        let se = result.summary.coefficients[0].standard_error.unwrap();
        assert!(se > 0.0);
        assert!((se / (variance / 4.0).sqrt() - 1.0).abs() < 1e-12);
        assert!(
            result
                .studies
                .iter()
                .all(|row| row.weight == 0.25 && row.fitted == 0.0)
        );
    }
}

#[test]
fn meta_positive_heterogeneity_keeps_representable_tau_when_raw_moment_overflows() {
    let variance: f64 = 1e-308;
    let amplitude = variance.sqrt();
    let y = [-amplitude, amplitude, -amplitude, amplitude];
    for estimator in [MetaEstimator::DerSimonianLaird, MetaEstimator::PauleMandel] {
        let result = model::summary(
            &y,
            &[variance; 4],
            &[],
            MetaOptions {
                estimator,
                ..Default::default()
            },
            &control(),
        )
        .unwrap();
        // Equal-variance pooling: Q=4, df=3, and DL/PM both give tau²=v/3.
        assert_eq!(result.heterogeneity.q, 4.0);
        let tau = result.heterogeneity.tau_squared;
        assert!(tau > 0.0 && tau.is_finite());
        assert!((tau / (variance / 3.0) - 1.0).abs() < 1e-9);
        assert!((result.residual_q - 3.0).abs() < 1e-9);
    }
}

#[test]
fn meta_funnel_uses_its_finite_center_without_unused_coefficient_inference() {
    let y = [1e160; 4];
    let v = [1e-300; 4];
    let options = MetaOptions {
        estimator: MetaEstimator::Fixed,
        ..Default::default()
    };
    let plot = plots::funnel(
        &y,
        &v,
        options.estimator,
        options.confidence_level,
        &control(),
    )
    .unwrap();
    let heterogeneity = model::heterogeneity(&y, &v, options.estimator, &control()).unwrap();
    assert_eq!(heterogeneity.q, 0.0);
    assert_eq!(heterogeneity.degrees_of_freedom, y.len() - 1);
    assert_eq!(heterogeneity.p_value, 1.0);
    assert_eq!(plot.plot.data.len(), y.len());
    assert!(plot.plot.reference_lines.iter().all(|line| {
        line.start.x == y[0]
            && line.end.x == y[0]
            && line.start.y.is_finite()
            && line.end.y.is_finite()
    }));
    assert_eq!(plot.y_domain[1], 0.0);
    assert!(plot.y_domain[0] > 0.0 && plot.y_domain[0].is_finite());
    assert!(matches!(
        plots::funnel(&y, &v, options.estimator, 0.0, &control()),
        Err(ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::ParameterOutOfRange
        })
    ));
}

#[test]
fn meta_rejects_invalid_study_contracts_and_obeys_control_without_row_caps() {
    let c = control();
    let opt = MetaOptions::default();
    for (y, v) in [
        (&[1., 2.][..], &[1.][..]),
        (&[1., 2.], &[0., 1.]),
        (&[1., f64::NAN], &[1., 2.]),
    ] {
        assert!(model::fit(y, v, &[], opt, &c).is_err());
    }
    assert!(model::fit(&[1., 2., 3.], &[1., 1., 1.], &[vec![2., 2., 2.]], opt, &c).is_err());
    assert!(diagnostics::leave_one_out(&[1., 2.], &[1., 1.], opt, &c).is_err());
    assert!(effects::correlation(&[1.], &[25.], 0.95, &c).is_err());
    assert!(
        effects::mean(
            ArmSummary {
                mean: &[2.],
                sd: &[1.],
                size: &[2.5]
            },
            0.95,
            &c
        )
        .is_err()
    );
    let y = (0..640)
        .map(|i| 0.1 + (i % 7) as f64 / 10.)
        .collect::<Vec<_>>();
    let fit = model::fit(&y, &vec![0.2; 640], &[], opt, &c).unwrap();
    assert_eq!(fit.summary.studies, 640);
    close(
        fit.summary.coefficients[0].estimate,
        y.iter().sum::<f64>() / 640.,
    );
    c.cancellation.cancel();
    assert!(matches!(
        model::fit(&y, &vec![0.2; 640], &[], opt, &c),
        Err(ScientificComputationError::Cancelled)
    ));
    let expired = ScientificExecutionControl {
        deadline: Instant::now(),
        ..control()
    };
    assert!(matches!(
        diagnostics::combine_p(&[0.1, 0.2], None, false, &expired),
        Err(ScientificComputationError::DeadlineExceeded)
    ));
}
