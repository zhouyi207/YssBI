//! Reuse fitted model observations; diagnostics never refit the original model.
use super::{breusch_pagan as bp, im_test, leverage, reset, vif, weighted as wb, white};
use yss_sci_contract::diagnostics::residual::{
    ResidualDiagnostic as Test, ResidualDiagnosticResult as Output,
};
use yss_sci_contract::regression::linear::LinearRegressionResult;
use yss_sci_linalg::{Col, Mat};

pub fn diagnose(model: &LinearRegressionResult, test: Test) -> Result<Output, String> {
    let n = model.residuals.len();
    if n == 0
        || model.fitted.len() != n
        || model.design.is_empty()
        || model.design.iter().any(|v| v.len() != n)
        || model
            .weights
            .as_ref()
            .is_some_and(|w| w.len() != n || w.iter().any(|v| !v.is_finite() || *v <= 0.0))
    {
        return Err("invalid diagnostic input shape or weights".into());
    }
    if model.report.model_basic_info.model_type == "GLS" && !matches!(test, Test::Vif) {
        return Err("residual diagnostic requires OLS or WLS".into());
    }
    let x = Mat::from_fn(n, model.design.len(), |row, col| model.design[col][row]);
    let u = Col::from_iter(model.residuals.iter().copied());
    let fitted = Col::from_iter(model.fitted.iter().copied());
    let weights = model
        .weights
        .as_ref()
        .map(|w| Col::from_iter(w.iter().copied()));
    Ok(match test {
        Test::BreuschPagan { rhs, koenker } => {
            Output::BreuschPagan(match (rhs, koenker, weights.as_ref()) {
                (true, false, None) => bp::breusch_pagan_stata_rhs(&x, &u),
                (true, true, None) => bp::breusch_pagan_koenker_rhs(&x, &u),
                (false, false, None) => bp::breusch_pagan_stata(&u, &fitted),
                (false, true, None) => bp::breusch_pagan_koenker(&u, &fitted),
                (true, false, Some(w)) => wb::breusch_pagan_stata_rhs_weighted(&x, &u, w),
                (true, true, Some(w)) => wb::breusch_pagan_koenker_rhs_weighted(&x, &u, w),
                (false, false, Some(w)) => wb::breusch_pagan_stata_weighted(&u, &fitted, w),
                (false, true, Some(w)) => wb::breusch_pagan_koenker_weighted(&u, &fitted, w),
            }?)
        }
        Test::White => Output::White(match &weights {
            Some(w) => white::white_test_weighted(&x, &u, w),
            None => white::white_test(&x, &u),
        }?),
        Test::InformationMatrix => Output::InformationMatrix(match &weights {
            Some(w) => im_test::im_test_weighted(&x, &u, w),
            None => im_test::im_test(&x, &u),
        }?),
        Test::Reset { rhs } => {
            let y = Col::from_iter(
                model
                    .fitted
                    .iter()
                    .zip(&model.residuals)
                    .map(|(f, u)| f + u),
            );
            Output::Reset(if rhs {
                reset::reset_test_rhs(&y, &x, weights.as_ref())
            } else {
                reset::reset_test(&y, &x, &fitted, weights.as_ref())
            }?)
        }
        Test::Vif => Output::Vif(vif::vif_centered(&x, model.constant)?),
        Test::Leverage => {
            let x = if let Some(w) = weights {
                Mat::from_fn(n, x.ncols(), |i, j| x[(i, j)] * w[i].sqrt())
            } else {
                x
            };
            Output::Leverage(leverage::leverage(&x)?)
        }
    })
}

pub fn normality(
    values: &[f64],
) -> Result<yss_sci_contract::diagnostics::residual::NormalityTestResult, String> {
    if values.iter().any(|v| !v.is_finite()) {
        return Err("nonfinite residual".into());
    }
    super::normality::normality_tests(&Col::from_iter(values.iter().copied()))
}
