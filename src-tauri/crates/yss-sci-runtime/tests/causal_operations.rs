use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};
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
    assert_eq!(report["family"], "panel_did_twfe");
    assert_eq!(report["observations"], 48);
    assert_eq!(report["entities"], 8);
    assert_eq!(report["timePeriods"], 6);
    assert!((report["coefficients"][1].as_f64().unwrap() - 1.75).abs() < 1e-10);

    treatment.pop();
    assert!(matches!(
        fit_did(response, vec![], entities, times, treatment),
        Err(SciError::InvalidInput {
            operation: SciOperationCode::Panel,
            violation: SciInputViolation::ShapeMismatch,
        })
    ));
}
