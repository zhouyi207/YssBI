use std::time::{Duration, Instant};
use yss_sci::path::moderation;
use yss_sci_contract::{execution::*, path::ModerationOptions};
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: Default::default(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8 * (1. + b.abs()), "{a} != {b}");
}
fn data(advanced: bool) -> (Vec<f64>, Vec<Vec<f64>>) {
    let x: Vec<_> = (0..640).map(|i| (i % 8) as f64 - 3.5).collect();
    let w: Vec<_> = (0..640).map(|i| 10. + ((i / 8) % 8) as f64).collect();
    let z: Vec<_> = (0..640).map(|i| 20. + ((i / 64) % 10) as f64).collect();
    let cov: Vec<_> = (0..640).map(|i| (i * 19 % 23) as f64 / 11. - 1.).collect();
    let y = (0..640)
        .map(|i| {
            let (a, b, d) = (x[i], w[i] - 13.5, z[i] - 24.5);
            1. + 0.3 * a
                + 0.2 * b
                + 0.5 * a * b
                + 0.3 * cov[i]
                + 0.05 * ((i * 7 % 17) as f64 - 8.)
                + if advanced {
                    0.4 * d - 0.12 * a * d + 0.06 * b * d + 0.08 * a * b * d
                } else {
                    0.
                }
        })
        .collect();
    (
        y,
        if advanced {
            vec![x, w, z, cov]
        } else {
            vec![x, w, cov]
        },
    )
}
#[test]
fn two_and_three_way_moderation_match_ols_contrasts_and_johnson_neyman_roots() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/moderation_reference.json")).unwrap();
    for advanced in [false, true] {
        let (y, x) = data(advanced);
        let f = &fixture[if advanced { "advanced" } else { "simple" }];
        let fit = moderation(
            &y,
            &x,
            ModerationOptions {
                second_moderator: advanced,
                probe_sd: 1.,
            },
            &control(),
        )
        .unwrap();
        for (j, b) in fit.model.coefficients.iter().enumerate() {
            close(b.estimate, f["coefficients"][j].as_f64().unwrap());
        }
        for (j, slope) in fit.diagnostics.effects.iter().enumerate() {
            let r = &f["slopes"][j];
            close(slope.moderator, r["w"].as_f64().unwrap());
            close(slope.effect.estimate, r["estimate"].as_f64().unwrap());
            close(
                slope.effect.standard_error.unwrap(),
                r["se"].as_f64().unwrap(),
            );
            close(slope.effect.p_value.unwrap(), r["p"].as_f64().unwrap());
        }
        for (j, region) in fit.diagnostics.johnson_neyman.iter().enumerate() {
            let r = f["boundaries"][j].as_array().unwrap();
            assert_eq!(region.boundaries.len(), r.len());
            for (a, b) in region.boundaries.iter().zip(r) {
                close(*a, b.as_f64().unwrap());
            }
            assert!(!region.significant_ranges.is_empty());
        }
        close(fit.model.fitted[639], f["last_fitted"].as_f64().unwrap());
    }
}
#[test]
fn moderation_rejects_unidentified_designs_marks_extrapolation_and_observes_control() {
    let (y, mut x) = data(false);
    let c = control();
    let options = ModerationOptions {
        second_moderator: false,
        probe_sd: 100.,
    };
    let fit = moderation(&y, &x, options, &c).unwrap();
    assert!(!fit.diagnostics.effects[0].in_observed_ranges);
    assert!(fit.diagnostics.effects[1].in_observed_ranges);
    assert_eq!(fit.diagnostics.johnson_neyman[0].observed_range, [10., 17.]);
    let mut changed_units = x.clone();
    for w in &mut changed_units[1] {
        *w *= 1e9;
    }
    let scaled = moderation(&y, &changed_units, options, &c).unwrap();
    assert_eq!(
        scaled.diagnostics.johnson_neyman[0].boundaries.len(),
        fit.diagnostics.johnson_neyman[0].boundaries.len()
    );
    for (a, b) in scaled.diagnostics.johnson_neyman[0]
        .boundaries
        .iter()
        .zip(&fit.diagnostics.johnson_neyman[0].boundaries)
    {
        close(a / 1e9, *b);
    }
    x[1].fill(1.);
    assert!(moderation(&y, &x, options, &c).is_err());
    c.cancellation.cancel();
    assert!(matches!(
        moderation(&y, &x, options, &c),
        Err(ScientificComputationError::Cancelled)
    ));
}
