use super::*;

fn long_run_variance(
    residuals: &[f64],
    bandwidth: usize,
    response_scale: f64,
    control: &Control,
) -> Result<(f64, f64, f64)> {
    if bandwidth >= residuals.len() {
        return Err(parameter());
    }
    let n = residuals.len() as f64;
    let mut gamma0 = 0.0;
    for (i, value) in residuals.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        gamma0 += value * value / n;
    }
    let mut variance = gamma0;
    for lag in 1..=bandwidth {
        control.check()?;
        let mut cov = 0.0;
        for i in lag..residuals.len() {
            if i % 1024 == 0 {
                control.check()?;
            }
            cov += residuals[i] * residuals[i - lag] / n;
        }
        variance += 2.0 * (1.0 - lag as f64 / (bandwidth + 1) as f64) * cov;
    }
    finite(variance)?;
    if variance <= 0.0 {
        return Err(parameter());
    }
    // Statistics use normalized moments; only the reported long-run variance
    // returns to physical units, without first squaring the response scale.
    let raw_variance = finite((variance * response_scale) * response_scale)?;
    if raw_variance <= 0.0 {
        return Err(parameter());
    }
    Ok((gamma0, variance, raw_variance))
}

pub fn phillips_perron(
    y: &[f64],
    bandwidth: usize,
    deterministic: Deterministic,
    control: &Control,
) -> Result<StationarityResult> {
    validate(y, &[], control)?;
    if y.len() < 4 {
        return Err(parameter());
    }
    let (response, response_scale) = normalized_series(y, control)?;
    let y = response.as_slice();
    let n = y.len() - 1;
    let constant = deterministic != Deterministic::None;
    let trend = (deterministic == Deterministic::Trend)
        .then(|| (1..=n).map(|i| i as f64).collect::<Vec<_>>());
    let mut predictors = vec![&y[..n]];
    if let Some(trend) = &trend {
        predictors.push(trend.as_slice());
    }
    let design = Design::new(&predictors, n, constant, true, true, control)?;
    let (beta, inverse) = least_squares(&design.x, &y[1..], None, control)?;
    let mut residuals = fitted(&design.x, &beta);
    for (i, (residual, value)) in residuals.iter_mut().zip(&y[1..]).enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        *residual = value - *residual;
    }
    let (gamma0, lambda2, raw_lambda2) =
        long_run_variance(&residuals, bandwidth, response_scale, control)?;
    let s2 = gamma0 * n as f64 / (n - beta.len()) as f64;
    let j = usize::from(constant);
    let rho = beta[j] / design.scales[0];
    let se = finite((inverse[(j, j)] * s2).sqrt() / design.scales[0])?;
    if se <= 0.0 || s2 <= 0.0 {
        return Err(parameter());
    }
    let statistic = finite(
        (gamma0 / lambda2).sqrt() * (rho - 1.0) / se
            - 0.5 * (lambda2 - gamma0) / lambda2.sqrt() * (n as f64 * se / s2.sqrt()),
    )?;
    let p_value = super::super::mackinnon::p_value(statistic, deterministic, 1);
    Ok(StationarityResult {
        method: "phillips_perron_tau".into(),
        observations: y.len(),
        effective_observations: n,
        deterministic,
        bandwidth,
        statistic,
        p_value,
        p_value_kind: "mackinnon_approximation".into(),
        critical_values: vec![],
        long_run_variance: raw_lambda2,
    })
}

pub fn kpss(
    y: &[f64],
    bandwidth: usize,
    deterministic: Deterministic,
    control: &Control,
) -> Result<StationarityResult> {
    validate(y, &[], control)?;
    if y.len() < 3 || deterministic == Deterministic::None {
        return Err(parameter());
    }
    let (response, response_scale) = normalized_series(y, control)?;
    let y = response.as_slice();
    let x: Vec<Vec<f64>> = if deterministic == Deterministic::Trend {
        vec![(1..=y.len()).map(|i| i as f64).collect()]
    } else {
        vec![]
    };
    let design = Design::new(&x, y.len(), true, true, true, control)?;
    let (beta, _) = least_squares(&design.x, y, None, control)?;
    let mut residuals = fitted(&design.x, &beta);
    for (i, (residual, value)) in residuals.iter_mut().zip(y).enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        *residual = value - *residual;
    }
    let (_, variance, raw_variance) =
        long_run_variance(&residuals, bandwidth, response_scale, control)?;
    let mut sum = 0.0;
    let mut eta = 0.0;
    for (i, e) in residuals.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        sum += e / y.len() as f64;
        eta += sum * sum;
    }
    let statistic = finite(eta / variance)?;
    let critical = if deterministic == Deterministic::Trend {
        [0.119, 0.146, 0.176, 0.216]
    } else {
        [0.347, 0.463, 0.574, 0.739]
    };
    let probabilities = [0.10, 0.05, 0.025, 0.01];
    let (p_value, kind) = if statistic < critical[0] {
        (0.10, "greater_than")
    } else if statistic > critical[3] {
        (0.01, "less_than")
    } else {
        let j = (1..4)
            .find(|&i| statistic <= critical[i])
            .ok_or_else(failed)?;
        (
            probabilities[j - 1]
                + (probabilities[j] - probabilities[j - 1]) * (statistic - critical[j - 1])
                    / (critical[j] - critical[j - 1]),
            "interpolated",
        )
    };
    Ok(StationarityResult {
        method: "kpss".into(),
        observations: y.len(),
        effective_observations: y.len(),
        deterministic,
        bandwidth,
        statistic,
        p_value,
        p_value_kind: kind.into(),
        critical_values: probabilities
            .iter()
            .zip(critical)
            .map(|(p, v)| estimate(format!("alpha_{p}"), v))
            .collect(),
        long_run_variance: raw_variance,
    })
}
