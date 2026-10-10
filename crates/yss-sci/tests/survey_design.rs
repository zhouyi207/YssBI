use std::time::{Duration, Instant};
use yss_sci::survey::{mean, regression, sampling_weights};
use yss_sci_contract::{
    execution::*,
    regression::models::{GlmFamily, IterationOptions},
    survey::*,
};
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: Default::default(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8 * (1. + b.abs()), "{a} != {b}");
}
struct Data {
    x: Vec<Vec<f64>>,
    w: Vec<f64>,
    h: Vec<usize>,
    g: Vec<usize>,
    y: Vec<f64>,
    binary: Vec<f64>,
    counts: Vec<f64>,
}
fn data() -> Data {
    let x: Vec<_> = (0..640).map(|i| (i % 16) as f64 / 5. - 1.5).collect();
    let z: Vec<_> = (0..640).map(|i| (i * 13 % 17) as f64 / 8. - 1.).collect();
    Data {
        y: (0..640)
            .map(|i| {
                4. + 0.5 * x[i] - 0.3 * z[i] + 0.4 * ((i / 8) % 7) as f64 - 1.2
                    + 0.03 * ((i * 11 % 19) as f64 - 9.)
            })
            .collect(),
        binary: (0..640)
            .map(|i| {
                f64::from(u8::from(
                    ((i * 37 % 101) as f64) < 45. + 3. * x[i] - 2. * z[i],
                ))
            })
            .collect(),
        counts: (0..640)
            .map(|i| (i * 7 % 6) as f64 + f64::from(u8::from(x[i] > 0.)))
            .collect(),
        w: (0..640).map(|i| 1. + (i % 7) as f64 * 0.2).collect(),
        h: (0..640).map(|i| i / 160).collect(),
        g: (0..640).map(|i| (i / 8) % 20).collect(),
        x: vec![x, z],
    }
}
fn design(d: &Data) -> SurveyDesign<'_> {
    SurveyDesign {
        weights: &d.w,
        strata: Some(&d.h),
        clusters: Some(&d.g),
        lonely_psu: LonelyPsu::Fail,
    }
}
fn reference() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/survey_reference.json")).unwrap()
}
#[test]
fn survey_means_and_proportions_match_stratum_psu_linearization_and_weight_scaling() {
    let d = data();
    let f = reference();
    let c = control();
    let w = sampling_weights(&d.w, false, &c).unwrap();
    close(w.summary.sum, f["weights"]["sum"].as_f64().unwrap());
    close(
        w.summary.kish_effective_n,
        f["weights"]["kish"].as_f64().unwrap(),
    );
    let probabilities: Vec<_> = d.w.iter().map(|w| 1. / w).collect();
    for (a, b) in sampling_weights(&probabilities, true, &c)
        .unwrap()
        .weights
        .iter()
        .zip(&d.w)
    {
        close(*a, *b);
    }
    for (name, strata, clusters) in [
        ("iid", None, None),
        ("stratified", Some(d.h.as_slice()), None),
        ("clustered", None, Some(d.g.as_slice())),
        ("nested", Some(d.h.as_slice()), Some(d.g.as_slice())),
    ] {
        let result = mean(
            &d.y,
            SurveyDesign {
                strata,
                clusters,
                ..design(&d)
            },
            false,
            &c,
        )
        .unwrap();
        close(result.estimate, f[name]["mean"].as_f64().unwrap());
        close(result.standard_error, f[name]["se"].as_f64().unwrap());
        assert_eq!(
            result.design.degrees_of_freedom,
            f[name]["df"].as_u64().unwrap() as usize
        );
        for j in 0..2 {
            close(
                result.confidence_interval[j],
                f[name]["ci"][j].as_f64().unwrap(),
            );
        }
    }
    let result = mean(&d.binary, design(&d), true, &c).unwrap();
    close(result.estimate, f["proportion"]["mean"].as_f64().unwrap());
    close(
        result.standard_error,
        f["proportion"]["se"].as_f64().unwrap(),
    );
    let changed: Vec<_> = d.w.iter().map(|w| w * 1e8).collect();
    let scaled = mean(
        &d.binary,
        SurveyDesign {
            weights: &changed,
            ..design(&d)
        },
        true,
        &c,
    )
    .unwrap();
    close(scaled.estimate, result.estimate);
    close(scaled.standard_error, result.standard_error);
}
#[test]
fn survey_regression_keeps_finite_design_inference_without_unused_unweighted_rss() {
    let scale = 1.2e154;
    let result = regression(
        &[-scale, scale, 0.0, 0.0],
        &[],
        SurveyDesign {
            weights: &[0.25, 0.25, 1.75, 1.75],
            strata: None,
            clusters: None,
            lonely_psu: LonelyPsu::Fail,
        },
        SurveyRegressionOptions {
            family: GlmFamily::Gaussian,
            constant: true,
            iteration: IterationOptions::default(),
        },
        &control(),
    )
    .unwrap();
    let coefficient = &result.model.coefficients[0];
    assert_eq!(coefficient.estimate, 0.0);
    // Four PSUs: bread=1/4; score variance=(4/3)*2*(scale/4)^2.
    assert!((coefficient.standard_error.unwrap() / scale - 1.0 / 96.0_f64.sqrt()).abs() < 1e-12);
    assert!(
        (result.model.covariance.as_ref().unwrap()[0][0] / scale / scale - 1.0 / 96.0).abs()
            < 1e-14
    );
    assert_eq!(result.diagnostics.coefficient_degrees_of_freedom, 3);
    assert_eq!(result.model.fitted, vec![0.0; 4]);
    assert!(result.model.statistics.rss.is_none());
    assert!(
        coefficient
            .confidence_interval
            .unwrap()
            .iter()
            .all(|v| v.is_finite())
    );
}

#[test]
fn survey_regressions_match_weighted_glm_and_design_covariance() {
    let d = data();
    let f = reference();
    let c = control();
    for (name, y, family) in [
        ("gaussian", &d.y, GlmFamily::Gaussian),
        ("binomial", &d.binary, GlmFamily::Binomial),
        ("poisson", &d.counts, GlmFamily::Poisson),
    ] {
        let options = SurveyRegressionOptions {
            family,
            constant: true,
            iteration: IterationOptions::default(),
        };
        let fit = regression(y, &d.x, design(&d), options, &c).unwrap();
        assert_eq!(fit.diagnostics.coefficient_degrees_of_freedom, 74);
        for (j, coefficient) in fit.model.coefficients.iter().enumerate() {
            close(coefficient.estimate, f[name]["beta"][j].as_f64().unwrap());
            close(
                coefficient.standard_error.unwrap(),
                f[name]["se"][j].as_f64().unwrap(),
            );
            close(
                coefficient.p_value.unwrap(),
                f[name]["p"][j].as_f64().unwrap(),
            );
            for k in 0..3 {
                close(
                    fit.model.covariance.as_ref().unwrap()[j][k],
                    f[name]["covariance"][j][k].as_f64().unwrap(),
                );
            }
        }
        close(
            fit.model.fitted[639],
            f[name]["last_fitted"].as_f64().unwrap(),
        );
        assert!(fit.model.statistics.aic.is_none());
        let changed: Vec<_> = d.w.iter().map(|w| w * 37.).collect();
        let scaled = regression(
            y,
            &d.x,
            SurveyDesign {
                weights: &changed,
                ..design(&d)
            },
            options,
            &c,
        )
        .unwrap();
        for (a, b) in fit
            .model
            .coefficients
            .iter()
            .zip(&scaled.model.coefficients)
        {
            close(a.estimate, b.estimate);
            close(a.standard_error.unwrap(), b.standard_error.unwrap());
        }
    }
}
#[test]
fn survey_rejects_unsupported_designs_and_respects_explicit_certainty_and_control() {
    let d = data();
    let c = control();
    let mut h = d.h.clone();
    h[0] = 10;
    let lonely = SurveyDesign {
        strata: Some(&h),
        ..design(&d)
    };
    assert!(mean(&d.y, lonely, false, &c).is_err());
    assert_eq!(
        mean(
            &d.y,
            SurveyDesign {
                lonely_psu: LonelyPsu::Certainty,
                ..lonely
            },
            false,
            &c
        )
        .unwrap()
        .design
        .certainty_strata,
        1
    );
    let boundary = mean(&vec![1.; 640], design(&d), true, &c).unwrap();
    assert!(boundary.boundary_proportion);
    assert_eq!(boundary.standard_error, 0.);
    assert!(sampling_weights(&[0., 1.], false, &c).is_err());
    assert!(sampling_weights(&[0.4, 1.01], true, &c).is_err());
    assert!(mean(&d.y, design(&d), true, &c).is_err());
    c.cancellation.cancel();
    assert!(matches!(
        mean(&d.y, design(&d), false, &c),
        Err(ScientificComputationError::Cancelled)
    ));
}
