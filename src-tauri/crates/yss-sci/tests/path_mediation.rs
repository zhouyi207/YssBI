use std::time::{Duration, Instant};
use yss_sci::path::{mediation, parse_equations, recursive_path};
use yss_sci_contract::{execution::*, path::*};
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: Default::default(),
        deadline: Instant::now() + Duration::from_secs(60),
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8 * (1. + b.abs()), "{a} != {b}");
}
fn options(stage: MediatedStage) -> MediationOptions {
    MediationOptions {
        moderated_stage: stage,
        probe_sd: 1.,
        replications: 0,
        seed: 42,
    }
}
fn data(stage: MediatedStage) -> (Vec<f64>, Vec<Vec<f64>>) {
    let x: Vec<_> = (0..640).map(|i| (i % 8) as f64 - 3.5).collect();
    let w: Vec<_> = (0..640).map(|i| ((i / 8) % 8) as f64 - 3.5).collect();
    let c: Vec<_> = (0..640).map(|i| (i * 19 % 23) as f64 / 11. - 1.).collect();
    let m: Vec<_> = (0..640)
        .map(|i| {
            2. + 0.6 * x[i]
                + 0.2 * w[i]
                + 0.3 * c[i]
                + 0.3 * ((i * 7 % 17) as f64 - 8.)
                + if stage == MediatedStage::First {
                    0.15 * x[i] * w[i]
                } else {
                    0.
                }
        })
        .collect();
    let mean = m.iter().sum::<f64>() / 640.;
    let y = (0..640)
        .map(|i| {
            1. + 0.25 * x[i]
                + 0.7 * (m[i] - mean)
                + 0.2 * w[i]
                + 0.3 * c[i]
                + 0.2 * ((i * 11 % 19) as f64 - 9.)
                + if stage == MediatedStage::Second {
                    0.08 * (m[i] - mean) * w[i]
                } else {
                    0.
                }
        })
        .collect();
    let mut columns = vec![x, m];
    if stage != MediatedStage::None {
        columns.push(w.iter().map(|v| v + 10.).collect());
    }
    columns.push(c);
    (y, columns)
}
#[test]
fn mediation_stages_match_independent_equations_effects_and_reproducible_bootstrap() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/path_reference.json")).unwrap();
    for (stage, key) in [
        (MediatedStage::None, "none"),
        (MediatedStage::First, "first"),
        (MediatedStage::Second, "second"),
    ] {
        let (y, x) = data(stage);
        let fit = mediation(&y, &x, options(stage), &control()).unwrap();
        let f = &reference[key];
        for (model, name) in [(&fit.mediator, "mediator"), (&fit.outcome, "outcome")] {
            for (j, c) in model.coefficients.iter().enumerate() {
                close(c.estimate, f[name][j].as_f64().unwrap());
                close(
                    c.standard_error.unwrap(),
                    f[format!("{name}_se")][j].as_f64().unwrap(),
                );
            }
        }
        for (j, e) in fit.diagnostics.effects.iter().enumerate() {
            close(e.indirect.estimate, f["indirect"][j].as_f64().unwrap());
            close(e.total.estimate, f["total"][j].as_f64().unwrap());
            assert!(e.indirect.confidence_interval.is_none());
        }
        close(fit.mediator.fitted[639], f["last_m"].as_f64().unwrap());
        close(fit.outcome.fitted[639], f["last_y"].as_f64().unwrap());
        if let Some(index) = fit.diagnostics.moderated_mediation_index {
            close(index.estimate, f["index"].as_f64().unwrap());
        }
        let o = MediationOptions {
            replications: 64,
            ..options(stage)
        };
        let first = mediation(&y, &x, o, &control()).unwrap();
        let second = mediation(&y, &x, o, &control()).unwrap();
        assert_eq!(
            serde_json::to_value(&first.diagnostics).unwrap(),
            serde_json::to_value(&second.diagnostics).unwrap()
        );
        for e in &first.diagnostics.effects {
            assert!(e.indirect.standard_error.unwrap() > 0.);
            let [lo, hi] = e.indirect.confidence_interval.unwrap();
            assert!(lo < hi);
        }
    }
}
#[test]
fn mediation_handles_extrapolation_rank_and_cooperative_control() {
    let (y, mut x) = data(MediatedStage::First);
    let o = MediationOptions {
        probe_sd: 100.,
        ..options(MediatedStage::First)
    };
    let c = control();
    let result = mediation(&y, &x, o, &c).unwrap();
    assert!(!result.diagnostics.effects[0].in_observed_range);
    assert!(result.diagnostics.effects[1].in_observed_range);
    x[1] = x[0].clone();
    assert!(mediation(&y, &x, o, &c).is_err());
    c.cancellation.cancel();
    assert!(matches!(
        mediation(&y, &x, o, &c),
        Err(ScientificComputationError::Cancelled)
    ));
}
#[test]
fn recursive_effects_match_matrix_inverse_and_reject_ambiguous_or_cyclic_equations() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/path_reference.json")).unwrap();
    let f = &reference["recursive"];
    let (y, x) = data(MediatedStage::None);
    let z = (0..640)
        .map(|i| -0.4 * x[0][i] + 0.2 * x[1][i] + 0.5 * y[i] + 0.15 * ((i * 13 % 29) as f64 - 14.))
        .collect();
    let columns = vec![x[0].clone(), x[1].clone(), y, z, x[2].clone()];
    let c = control();
    let equations =
        parse_equations("x4 ~ x1 + x2 + x3; x3 ~ x1 + x2 + x5\nx2 ~ x1 + x5", 5, &c).unwrap();
    let fit = recursive_path(&columns, &equations, &c).unwrap();
    assert_eq!(fit.effects.len(), 20);
    for (j, e) in fit.equations.iter().enumerate() {
        for (k, coef) in e.model.coefficients.iter().enumerate() {
            close(coef.estimate, f["fits"][j][k].as_f64().unwrap());
        }
    }
    for e in fit.effects {
        close(e.direct, f["direct"][e.target][e.source].as_f64().unwrap());
        close(e.total, f["total"][e.target][e.source].as_f64().unwrap());
        close(
            e.standardized_total.unwrap(),
            f["standardized"][e.target][e.source].as_f64().unwrap(),
        );
        close(e.indirect + e.direct, e.total);
    }
    for invalid in [
        "x1~x2;x2~x1",
        "x2~x1+x1",
        "x2~x1;x2~x3",
        "x2~x6",
        "x2~x2",
        "x2~x1*0.5",
    ] {
        assert!(parse_equations(invalid, 5, &c).is_err(), "{invalid}");
    }
    c.cancellation.cancel();
    assert!(matches!(
        recursive_path(&columns, &equations, &c),
        Err(ScientificComputationError::Cancelled)
    ));
}
