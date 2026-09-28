//! Instrumental-variable design preparation and fit projection.
use super::iv2sls::{IV2SLS, IV2SLSConfig};
use super::ivliml::{IVLIML, IVLIMLConfig};
use crate::error::{computation_failed, invalid_input};
use yss_sci_contract::causal::iv::{InstrumentalVariableFit, InstrumentalVariableKind};
use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};
use yss_sci_linalg::{Col, Mat};

pub fn fit_instrumental_variables(
    kind: InstrumentalVariableKind,
    response: Vec<f64>,
    exogenous: Vec<f64>,
    endogenous: Vec<f64>,
    instruments: Vec<f64>,
) -> Result<InstrumentalVariableFit, SciError> {
    let observations = response.len();
    if [exogenous.len(), endogenous.len(), instruments.len()]
        .into_iter()
        .any(|len| len != observations)
    {
        return Err(invalid_input(
            SciOperationCode::InstrumentalVariables,
            SciInputViolation::ShapeMismatch,
        ));
    }
    let column = |values: Vec<f64>| Mat::from_fn(observations, 1, |row, _| values[row]);
    match kind {
        InstrumentalVariableKind::TwoStageLeastSquares => {
            let result = IV2SLS {
                endog: Col::from_iter(response),
                exog: column(exogenous),
                endog_reg: column(endogenous),
                instruments: column(instruments),
                config: IV2SLSConfig {
                    constant: true,
                    cov_type: "nonrobust".into(),
                    cov_params: None,
                    small: false,
                },
                endog_names: None,
                z_var_names: None,
            }
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))?;
            Ok(InstrumentalVariableFit {
                family: "iv_2sls",
                kappa: None,
                coefficients: result.betas.iter().copied().collect::<Vec<_>>(),
                standard_errors: result.stds.iter().copied().collect::<Vec<_>>(),
                p_values: result.pvalues.iter().copied().collect::<Vec<_>>(),
                r2: result.r2,
                adjusted_r2: result.r2_adjusted,
                first_stage_min_eigenvalue: result.first_stage_summary.min_eigenvalue,
            })
        }
        InstrumentalVariableKind::LimitedInformationMaximumLikelihood => {
            let result = IVLIML {
                endog: Col::from_iter(response),
                exog: column(exogenous),
                endog_reg: column(endogenous),
                instruments: column(instruments),
                config: IVLIMLConfig {
                    constant: true,
                    cov_type: "nonrobust".into(),
                    cov_params: None,
                    small: false,
                },
                endog_names: None,
                z_var_names: None,
            }
            .fit()
            .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))?;
            Ok(InstrumentalVariableFit {
                family: "iv_liml",
                coefficients: result.betas.iter().copied().collect::<Vec<_>>(),
                standard_errors: result.stds.iter().copied().collect::<Vec<_>>(),
                p_values: result.pvalues.iter().copied().collect::<Vec<_>>(),
                r2: result.r2,
                adjusted_r2: result.r2_adjusted,
                kappa: Some(result.kappa),
                first_stage_min_eigenvalue: result.first_stage_summary.min_eigenvalue,
            })
        }
    }
}
