use yss_sci_contract::{SciError, SciOperationCode, execution::ScientificInputViolation};
use yss_sci_runtime::causal::did::fit_did;

#[test]
fn twfe_did_preserves_treatment_effect_and_report_contract() {
    let mut response = Vec::new();
    let mut entities = Vec::new();
    let mut times = Vec::new();
    let mut treatment = Vec::new();
    for entity in 0..8 {
        for time in 0..6 {
            let did = f64::from(entity < 3 && time >= 3);
            response.push(f64::from(entity) * 0.4 + f64::from(time) * 0.3 + 1.75 * did);
            entities.push(f64::from(entity));
            times.push(f64::from(time));
            treatment.push(did);
        }
    }

    let report = fit_did(
        response.clone(),
        vec![vec![1.0; response.len()]],
        entities.clone(),
        times.clone(),
        treatment.clone(),
    )
    .unwrap();
    assert_eq!(report.family, "panel_did_twfe");
    assert_eq!(report.statistics.observations, 48);
    assert_eq!(report.statistics.entities, 8);
    assert_eq!(report.statistics.time_periods, 6);
    assert!((report.coefficients[1] - 1.75).abs() < 1e-10);

    treatment.pop();
    assert!(matches!(
        fit_did(response, vec![], entities, times, treatment),
        Err(SciError::InvalidInput {
            operation: SciOperationCode::Panel,
            violation: ScientificInputViolation::ShapeMismatch,
        })
    ));
}

#[test]
fn iv_fixed_scale_unavailability_retains_the_nonrobust_reason() {
    use yss_sci_contract::causal::iv::{InstrumentalVariableKind, IvSummaryOptions};
    use yss_sci_contract::regression::{OlsCovariance, OlsOptions};
    use yss_sci_runtime::causal::iv::{fit_instrumental_variables, summary};
    let signal = |row: usize, bit: usize| if row & (1 << bit) == 0 { -1.0 } else { 1.0 };
    let endogenous = (0..16)
        .map(|row| 2.0 * signal(row, 0) + 0.5 * signal(row, 1))
        .collect::<Vec<_>>();
    let instruments = (0..2)
        .map(|bit| (0..16).map(|row| signal(row, bit)).collect())
        .collect::<Vec<Vec<f64>>>();
    let response = (0..16)
        .map(|row| 1.0 + 0.5 * endogenous[row] + 2.0 * signal(row, 2))
        .collect();
    let fit = fit_instrumental_variables(
        InstrumentalVariableKind::TwoStageLeastSquares,
        response,
        &[],
        &[endogenous],
        &instruments,
        OlsOptions {
            constant: true,
            covariance: OlsCovariance::FixedScale { scale: 1.0 },
        },
        false,
    )
    .unwrap();
    assert_eq!(fit.statistics.covariance_type, "fixed scale");
    let model = serde_json::to_value(&fit).unwrap();
    let fit = serde_json::from_value(model).unwrap();
    let report = summary(
        &fit,
        IvSummaryOptions {
            model_summary: false,
            coefficient_table: false,
            first_stage: false,
            overidentification: false,
            endogeneity: true,
        },
    )
    .unwrap();
    assert!(report["endogeneity"]["hausman"].is_null());
    assert!(report["endogeneity"]["endogenous"].is_null());
    assert_eq!(
        report["endogeneityUnavailable"],
        "insufficient_residual_variation_or_degrees_of_freedom"
    );
}

#[test]
fn iv_diagnostic_unavailability_uses_options_after_model_roundtrip() {
    use yss_sci_contract::causal::iv::{InstrumentalVariableKind, IvSummaryOptions};
    use yss_sci_contract::regression::{OlsCovariance, OlsOptions};
    use yss_sci_runtime::causal::iv::{fit_instrumental_variables, summary};
    let signal = |row: usize, bit: usize| if row & (1 << bit) == 0 { -1.0 } else { 1.0 };
    let endogenous = (0..16)
        .map(|row| 2.0 * signal(row, 0) + 0.5 * signal(row, 1) + signal(row, 2))
        .collect::<Vec<_>>();
    let instruments = (0..2)
        .map(|bit| (0..16).map(|row| signal(row, bit)).collect())
        .collect::<Vec<Vec<f64>>>();
    let response = (0..16)
        .map(|row| 1.0 + 0.5 * endogenous[row] + signal(row, 1) + 2.0 * signal(row, 3))
        .collect::<Vec<_>>();
    let mut reasons = Vec::new();
    for kind in [
        InstrumentalVariableKind::TwoStageLeastSquares,
        InstrumentalVariableKind::LimitedInformationMaximumLikelihood,
    ] {
        let fit = fit_instrumental_variables(
            kind,
            response.clone(),
            &[],
            std::slice::from_ref(&endogenous),
            &instruments,
            OlsOptions {
                constant: true,
                covariance: OlsCovariance::Hc1,
            },
            false,
        )
        .unwrap();
        let mut model = serde_json::to_value(&fit).unwrap();
        model["statistics"]["covarianceType"] = serde_json::json!("nonrobust");
        let fit = serde_json::from_value(model).unwrap();
        let liml = kind == InstrumentalVariableKind::LimitedInformationMaximumLikelihood;
        let report = summary(
            &fit,
            IvSummaryOptions {
                model_summary: false,
                coefficient_table: false,
                first_stage: false,
                overidentification: liml,
                endogeneity: !liml,
            },
        )
        .unwrap();
        let field = if liml {
            "overidentificationUnavailable"
        } else {
            "endogeneityUnavailable"
        };
        reasons.push((kind, report[field].clone()));
    }
    assert!(
        reasons
            .iter()
            .all(|(_, reason)| reason == "requires_nonrobust_covariance"),
        "{reasons:?}"
    );
}
