use super::common::*;
use yss_sci_contract::execution::ScientificExecutionControl as Control;
use yss_sci_contract::regression::models::*;
use yss_sci_linalg::Mat;

fn psi(u: f64, loss: RobustLoss, c: f64) -> (f64, f64, f64) {
    match loss {
        RobustLoss::Huber => {
            let v = u.clamp(-c, c);
            (
                v,
                if u.abs() <= c { 1.0 } else { 0.0 },
                if u == 0.0 { 1.0 } else { v / u },
            )
        }
        RobustLoss::Tukey => {
            if u.abs() >= c {
                (0.0, 0.0, 0.0)
            } else {
                let z = (u / c).powi(2);
                let w = (1.0 - z).powi(2);
                (u * w, (1.0 - z) * (1.0 - 5.0 * z), w)
            }
        }
    }
}
pub fn robust(
    y: &[f64],
    predictors: &[Vec<f64>],
    options: RobustOptions,
    control: &Control,
) -> Result<RegressionModelResult> {
    validate(y, predictors, control)?;
    check_iteration(options.iteration)?;
    if !options.tuning.is_finite() || options.tuning <= 0.0 {
        return Err(parameter());
    }
    let design = Design::new(predictors, y.len(), options.constant, true, true, control)?;
    let x = &design.x;
    let p = x.ncols();
    let n = y.len();
    let (mut beta, inv) = least_squares(x, y, None, control)?;
    let mut scale = 0.0;
    let mut converged = None;
    for iter in 1..=options.iteration.max_iterations {
        control.check()?;
        let pred = fitted(x, &beta);
        let residual = y.iter().zip(&pred).map(|(a, b)| a - b).collect::<Vec<_>>();
        scale = median(&residual.iter().map(|v| v.abs()).collect::<Vec<_>>()) / 0.6744897501960817;
        if scale <= f64::EPSILON * mean(&y.iter().map(|v| v.abs()).collect::<Vec<_>>()).max(1.0) {
            let max_response = y.iter().map(|v| v.abs()).fold(0.0, f64::max);
            if residual
                .iter()
                .any(|v| v.abs() > options.iteration.tolerance * (1.0 + max_response))
            {
                return Err(failed());
            }
            scale = 0.0;
            converged = Some(iter);
            break;
        }
        let weights = residual
            .iter()
            .map(|r| psi(r / scale, options.loss, options.tuning).2)
            .collect::<Vec<_>>();
        let (next, _) = least_squares(x, y, Some(&weights), control)?;
        let change = next
            .iter()
            .zip(&beta)
            .map(|(a, b)| (a - b).abs() / (1.0 + b.abs()))
            .fold(0.0, f64::max);
        beta = next;
        if change <= options.iteration.tolerance {
            converged = Some(iter);
            break;
        }
    }
    let iterations = converged.ok_or_else(failed)?;
    let pred = fitted(x, &beta);
    let residual = y.iter().zip(&pred).map(|(a, b)| a - b).collect::<Vec<_>>();
    if scale > 0.0 {
        scale = median(&residual.iter().map(|v| v.abs()).collect::<Vec<_>>()) / 0.6744897501960817;
    }
    let factor = if scale == 0.0 {
        0.0
    } else {
        let ps = residual
            .iter()
            .map(|r| psi(r / scale, options.loss, options.tuning))
            .collect::<Vec<_>>();
        let dm = ps.iter().map(|v| v.1 / n as f64).sum::<f64>();
        if dm <= 0.0 {
            return Err(failed());
        }
        let dv = ps
            .iter()
            .map(|v| (v.1 - dm).powi(2) / n as f64)
            .sum::<f64>();
        let k = 1.0 + (p as f64 / n as f64) * dv / (dm * dm);
        k * k * scale * scale * ps.iter().map(|v| v.0 * v.0).sum::<f64>()
            / (n - p) as f64
            / (dm * dm)
    };
    let cov = Mat::from_fn(p, p, |i, j| inv[(i, j)] * factor);
    let (raw, cov) = design.raw(&beta, Some(cov));
    let mut r = result(
        "robust",
        y,
        pred,
        raw,
        names(predictors.len(), options.constant),
        cov,
        options.constant,
        None,
        RegressionDetails::Robust {
            loss: options.loss,
            tuning: options.tuning,
            scale,
            covariance_method: "H1 normal approximation".into(),
        },
    )?;
    r.statistics.df_residual = Some(n - p);
    r.iterations = iterations;
    control.check()?;
    Ok(r)
}

pub fn quantile(
    y: &[f64],
    predictors: &[Vec<f64>],
    constant: bool,
    tau: f64,
    iteration: IterationOptions,
    control: &Control,
) -> Result<RegressionModelResult> {
    validate(y, predictors, control)?;
    check_iteration(iteration)?;
    if !tau.is_finite() || tau <= 0.0 || tau >= 1.0 {
        return Err(parameter());
    }
    let design = Design::new(predictors, y.len(), constant, true, true, control)?;
    let x = &design.x;
    let (mut beta, inv) = least_squares(x, y, None, control)?;
    let response_mean = mean(y);
    let floor = 1e-8
        * (y.iter()
            .map(|v| (v - response_mean).abs())
            .fold(0.0, f64::max))
        .max(1.0);
    let mut done = None;
    for iter in 1..=iteration.max_iterations {
        control.check()?;
        let pred = fitted(x, &beta);
        let weights = y
            .iter()
            .zip(&pred)
            .map(|(a, b)| {
                let r = a - b;
                1.0 / (r.abs().max(floor) * if r < 0.0 { tau } else { 1.0 - tau })
            })
            .collect::<Vec<_>>();
        let (next, _) = least_squares(x, y, Some(&weights), control)?;
        let change = next
            .iter()
            .zip(&beta)
            .map(|(a, b)| (a - b).abs() / (1.0 + b.abs()))
            .fold(0.0, f64::max);
        beta = next;
        if change <= iteration.tolerance {
            done = Some(iter);
            break;
        }
    }
    let iterations = done.ok_or_else(failed)?;
    let pred = fitted(x, &beta);
    let residual = y.iter().zip(&pred).map(|(a, b)| a - b).collect::<Vec<_>>();
    let loss = residual
        .iter()
        .map(|&r| r * if r >= 0.0 { tau } else { tau - 1.0 })
        .sum::<f64>();
    let rm = mean(&residual);
    let sd = (residual.iter().map(|v| (v - rm).powi(2)).sum::<f64>() / (y.len() - 1) as f64).sqrt();
    let mut sorted = residual.clone();
    sorted.sort_by(f64::total_cmp);
    let q = |prob: f64| {
        let t = (sorted.len() - 1) as f64 * prob;
        let i = t.floor() as usize;
        sorted[i] * (1.0 - t.fract()) + sorted[(i + 1).min(sorted.len() - 1)] * t.fract()
    };
    let bandwidth = 0.9 * sd.min((q(0.75) - q(0.25)) / 1.34) * (y.len() as f64).powf(-0.2);
    let cov = if bandwidth > floor && bandwidth.is_finite() {
        let density = residual
            .iter()
            .map(|r| {
                (-0.5 * (r / bandwidth).powi(2)).exp()
                    / (2.0 * std::f64::consts::PI).sqrt()
                    / bandwidth
                    / y.len() as f64
            })
            .sum::<f64>();
        (density > 0.0).then(|| {
            Mat::from_fn(x.ncols(), x.ncols(), |i, j| {
                inv[(i, j)] * tau * (1.0 - tau) / (density * density)
            })
        })
    } else {
        None
    };
    let (raw, cov) = design.raw(&beta, cov);
    let mut r = result(
        "quantile",
        y,
        pred,
        raw,
        names(predictors.len(), constant),
        cov,
        constant,
        None,
        RegressionDetails::Quantile {
            quantile: tau,
            check_loss: finite(loss)?,
            covariance_method: "iid Gaussian-kernel density, Silverman bandwidth".into(),
        },
    )?;
    r.iterations = iterations;
    r.statistics.df_residual = Some(y.len() - x.ncols());
    control.check()?;
    Ok(r)
}
