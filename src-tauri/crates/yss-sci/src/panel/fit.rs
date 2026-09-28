//! Panel design preparation and fit projection.
use super::fit_panel_fe_twoway;
use crate::error::{computation_failed, invalid_input};
use crate::regression::design::design_matrix;
use yss_sci_contract::panel::PanelFit;
use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};
use yss_sci_linalg::Col;

pub fn fit_panel(
    response: Vec<f64>,
    predictors: Vec<Vec<f64>>,
    entity: Vec<f64>,
    time: Vec<f64>,
) -> Result<PanelFit, SciError> {
    let observations = response.len();
    if entity.len() != observations || time.len() != observations {
        return Err(invalid_input(
            SciOperationCode::Panel,
            SciInputViolation::ShapeMismatch,
        ));
    }
    let exog = design_matrix(&predictors, observations, false, SciOperationCode::Panel)?;
    let ids = |values: Vec<f64>| -> Vec<usize> {
        let mut levels = Vec::<f64>::new();
        values
            .into_iter()
            .map(|value| {
                levels
                    .iter()
                    .position(|level| *level == value)
                    .unwrap_or_else(|| {
                        levels.push(value);
                        levels.len() - 1
                    })
            })
            .collect()
    };
    let result = fit_panel_fe_twoway(
        &Col::from_iter(response),
        &exog,
        &ids(entity),
        &ids(time),
        true,
        "cluster",
        None,
    )
    .map_err(|_| computation_failed(SciOperationCode::Panel))?;
    Ok(PanelFit {
        family: "panel_fe_twoway",
        coefficients: result.betas.iter().copied().collect::<Vec<_>>(),
        standard_errors: result.stds.iter().copied().collect::<Vec<_>>(),
        p_values: result.pvalues.iter().copied().collect::<Vec<_>>(),
        r2: result.r2,
        adjusted_r2: result.r2_adjusted,
        observations: result.num_observation,
        entities: result.num_entities,
        time_periods: result.num_time_periods,
    })
}
