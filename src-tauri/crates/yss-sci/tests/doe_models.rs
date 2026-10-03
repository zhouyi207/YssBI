use std::time::{Duration, Instant};
use yss_sci::doe::{dose_response, response_surface};
use yss_sci_contract::{execution::*, regression::models::IterationOptions};
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: Default::default(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/doe_models_reference.json")).unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-6 * (1. + b.abs()), "{a} != {b}");
}

#[test]
fn response_surface_matches_ols_and_classifies_stationary_geometry() {
    let c = control();
    let x = vec![
        (0..640)
            .map(|i| 10. + 2. * (i % 8) as f64)
            .collect::<Vec<_>>(),
        (0..640)
            .map(|i| 100. + 10. * ((i / 8) % 8) as f64)
            .collect(),
    ];
    let y: Vec<_> = (0..640)
        .map(|i| {
            let (a, b) = ((x[0][i] - 17.) / 7., (x[1][i] - 135.) / 35.);
            50. + 4. * a - 2. * b - 3. * a * a - 4. * b * b
                + 1.2 * a * b
                + 0.03 * ((i * 7 % 17) as f64 - 8.)
        })
        .collect();
    let fit = response_surface(&y, &x, &c).unwrap();
    let f = fixture();
    let f = &f["surface"];
    for (j, coef) in fit.model.coefficients.iter().enumerate() {
        close(coef.estimate, f["coefficients"][j].as_f64().unwrap());
        close(coef.standard_error.unwrap(), f["se"][j].as_f64().unwrap());
    }
    close(
        fit.model.statistics.r_squared.unwrap(),
        f["r2"].as_f64().unwrap(),
    );
    close(
        fit.model.statistics.rss.unwrap(),
        f["rss"].as_f64().unwrap(),
    );
    close(fit.model.fitted[639], f["last_fitted"].as_f64().unwrap());
    assert_eq!(fit.geometry.classification, "maximum");
    let point = fit.geometry.stationary_point.unwrap();
    for (j, v) in point.coded_factors.iter().enumerate() {
        close(*v, f["stationary"][j].as_f64().unwrap());
    }
    close(point.predicted_response, f["prediction"].as_f64().unwrap());
    close(point.factors[0], 17. + 7. * point.coded_factors[0]);
    assert!(point.inside_factor_ranges);
    let minimum = response_surface(&y.iter().map(|v| -v).collect::<Vec<_>>(), &x, &c).unwrap();
    assert_eq!(minimum.geometry.classification, "minimum");
    let linear: Vec<_> = (0..640).map(|i| 3. + 2. * x[0][i] - x[1][i]).collect();
    let fit = response_surface(&linear, &x, &c).unwrap();
    assert_eq!(fit.geometry.classification, "degenerate");
    assert!(fit.geometry.stationary_point.is_none());
    assert!(response_surface(&y, &[x[0].clone(), x[0].clone()], &c).is_err());
}

#[test]
fn dose_curve_matches_independent_fit_includes_zero_and_preserves_dose_units() {
    let c = control();
    let options = IterationOptions {
        max_iterations: 500,
        tolerance: 1e-10,
    };
    let dose: Vec<_> = (0..640)
        .map(|i| {
            if i % 32 == 0 {
                0.
            } else {
                ((i % 32) as f64 / 4. - 4.).exp()
            }
        })
        .collect();
    let y: Vec<_> = (0..640)
        .map(|i| 1.2 + 8.4 / (1. + (dose[i] / 2.5).powf(1.4)) + 0.03 * ((i * 17 % 19) as f64 - 9.))
        .collect();
    let fit = dose_response(&y, &dose, options, &c).unwrap();
    let f = fixture();
    let f = &f["dose"];
    for (j, coef) in fit.model.coefficients.iter().enumerate() {
        close(coef.estimate, f["coefficients"][j].as_f64().unwrap());
        close(coef.standard_error.unwrap(), f["se"][j].as_f64().unwrap());
    }
    close(fit.parameters.ed50.estimate, f["ed50"].as_f64().unwrap());
    close(fit.parameters.hill.estimate, f["hill"].as_f64().unwrap());
    for (j, v) in fit
        .parameters
        .ed50
        .confidence_interval
        .unwrap()
        .iter()
        .enumerate()
    {
        close(*v, f["ed50_ci"][j].as_f64().unwrap());
    }
    close(fit.model.fitted[0], fit.model.coefficients[1].estimate);
    close(fit.model.fitted[639], f["last_fitted"].as_f64().unwrap());
    assert!(!fit.parameters.increasing);
    let increasing = dose_response(
        &y.iter().map(|v| -v).collect::<Vec<_>>(),
        &dose.iter().map(|v| v * 1000.).collect::<Vec<_>>(),
        options,
        &c,
    )
    .unwrap();
    assert!(increasing.parameters.increasing);
    close(
        increasing.parameters.ed50.estimate,
        fit.parameters.ed50.estimate * 1000.,
    );
    assert!(dose_response(&[2.; 640], &dose, options, &c).is_err());
    assert!(dose_response(&y, &vec![-1.; 640], options, &c).is_err());
    c.cancellation.cancel();
    assert!(matches!(
        dose_response(&y, &dose, options, &c),
        Err(ScientificComputationError::Cancelled)
    ));
}
