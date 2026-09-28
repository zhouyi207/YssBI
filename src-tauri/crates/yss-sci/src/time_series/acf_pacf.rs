//! ACF (Autocorrelation) and PACF (Partial Autocorrelation)
//!
//! 参考 Stata ac / pac 命令：样本自相关与偏自相关，用于残差诊断。

use yss_sci_contract::execution::{
    ScientificComputationError, ScientificExecutionControl, ScientificInputViolation,
};
use yss_sci_contract::time_series::acf_pacf::AcfPacfResult;

/// Compute ACF once and reuse it for PACF, with cooperative execution checks.
/// The numerical lag bound is n - 1; product budgets belong to callers.
pub fn compute_acf_pacf(
    values: &[f64],
    max_lag: usize,
    control: &ScientificExecutionControl,
) -> Result<AcfPacfResult, ScientificComputationError> {
    control.check()?;
    if values.len() < 2 {
        return Err(ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::EmptyInput,
        });
    }
    if max_lag == 0 {
        return Err(ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::ParameterOutOfRange,
        });
    }
    for (index, value) in values.iter().enumerate() {
        checkpoint(Some(control), index)?;
        if !value.is_finite() {
            return Err(ScientificComputationError::InvalidInput {
                violation: ScientificInputViolation::NonFiniteInput,
            });
        }
    }
    control.check()?;
    let acf = acf_values(values, max_lag, Some(control))?;
    let pacf = pacf_from_acf(&acf, Some(control))?;
    control.check()?;
    Ok(AcfPacfResult {
        acf,
        pacf,
        n: values.len(),
    })
}

/// 计算样本自相关 ACF
///
/// 返回 lag 0..=max_lag，其中 lag 0 恒为 1.0。
/// 公式: ρ̂(k) = Σ(y_t - ȳ)(y_{t-k} - ȳ) / Σ(y_t - ȳ)²
pub fn acf(x: &[f64], max_lag: usize) -> Vec<f64> {
    acf_values(x, max_lag, None).expect("uncontrolled ACF cannot be interrupted")
}

fn checkpoint(
    control: Option<&ScientificExecutionControl>,
    index: usize,
) -> Result<(), ScientificComputationError> {
    if index.is_multiple_of(1024)
        && let Some(control) = control
    {
        control.check()?;
    }
    Ok(())
}

fn acf_values(
    x: &[f64],
    max_lag: usize,
    control: Option<&ScientificExecutionControl>,
) -> Result<Vec<f64>, ScientificComputationError> {
    checkpoint(control, 0)?;
    let n = x.len();
    if n < 2 || max_lag == 0 {
        return Ok(vec![1.0]);
    }
    let mut sum = 0.0;
    for (index, value) in x.iter().enumerate() {
        checkpoint(control, index)?;
        sum += value;
    }
    let mean = sum / n as f64;
    let mut var = 0.0;
    for (index, value) in x.iter().enumerate() {
        checkpoint(control, index)?;
        var += (value - mean).powi(2);
    }
    if var <= 0.0 {
        return Ok(vec![1.0]);
    }
    let max_lag = max_lag.min(n - 1);
    let mut acf_vals = Vec::with_capacity(max_lag + 1);
    acf_vals.push(1.0); // lag 0
    for k in 1..=max_lag {
        checkpoint(control, 0)?;
        let mut sum = 0.0;
        for t in k..n {
            checkpoint(control, t - k)?;
            sum += (x[t] - mean) * (x[t - k] - mean);
        }
        acf_vals.push(sum / var);
    }
    Ok(acf_vals)
}

/// 计算偏自相关 PACF（Durbin-Levinson 递推）
///
/// 返回 lag 1..=max_lag（PACF 无 lag 0）。
/// 与 Stata pac 一致，使用 Yule-Walker / Durbin-Levinson。
pub fn pacf(x: &[f64], max_lag: usize) -> Vec<f64> {
    pacf_from_acf(&acf(x, max_lag), None).expect("uncontrolled PACF cannot be interrupted")
}

fn pacf_from_acf(
    acf_vals: &[f64],
    control: Option<&ScientificExecutionControl>,
) -> Result<Vec<f64>, ScientificComputationError> {
    checkpoint(control, 0)?;
    if acf_vals.len() <= 1 {
        return Ok(vec![]);
    }
    let rho = &acf_vals[1..]; // ρ_1, ρ_2, ...
    let n_lag = rho.len();
    let mut pacf_out = Vec::with_capacity(n_lag);
    let mut phi_prev = vec![rho[0]]; // φ_{1,1} = ρ_1
    pacf_out.push(rho[0]);
    for k in 2..=n_lag {
        checkpoint(control, 0)?;
        let mut num = rho[k - 1];
        let mut den = 1.0;
        for j in 1..k {
            checkpoint(control, j - 1)?;
            num -= phi_prev[j - 1] * rho[k - 1 - j];
            den -= phi_prev[j - 1] * rho[j - 1];
        }
        let phi_kk = if den.abs() < 1e-15 { 0.0 } else { num / den };
        pacf_out.push(phi_kk);
        let mut phi_curr = Vec::with_capacity(k);
        for j in 1..k {
            checkpoint(control, j - 1)?;
            phi_curr.push(phi_prev[j - 1] - phi_kk * phi_prev[k - 1 - j]);
        }
        phi_curr.push(phi_kk);
        phi_prev = phi_curr;
    }
    Ok(pacf_out)
}

#[cfg(test)]
mod tests;
