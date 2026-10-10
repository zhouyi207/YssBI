use super::*;

struct Equations {
    bread: Mat<f64>,
    meat: Mat<f64>,
    score: Col<f64>,
    means: Vec<f64>,
    pearson: Vec<f64>,
}

fn equations(
    y: &[f64],
    x: &Mat<f64>,
    clusters: &[Vec<usize>],
    beta: &[f64],
    family: ResponseFamily,
    rho: f64,
    control: &Control,
) -> Result<Equations> {
    let p = x.ncols();
    let mut bread = Mat::zeros(p, p);
    let mut meat = Mat::zeros(p, p);
    let mut score = Col::zeros(p);
    let mut means = vec![0.0; y.len()];
    let mut pearson = vec![0.0; y.len()];
    let mut derivative = vec![0.0; y.len()];
    for i in 0..y.len() {
        control.check()?;
        let eta = (0..p).map(|j| x[(i, j)] * beta[j]).sum();
        let (mu, d, v) = mean_variance(eta, family, 0.0)?;
        means[i] = mu;
        pearson[i] = finite((y[i] - mu) / v.sqrt())?;
        derivative[i] = d / v.sqrt();
    }
    for cluster in clusters {
        control.check()?;
        let denominator = 1.0 + (cluster.len() - 1) as f64 * rho;
        if denominator <= 0.0 || rho >= 1.0 {
            return Err(parameter());
        }
        let adjustment = rho / denominator;
        let sum_r = cluster.iter().map(|&i| pearson[i]).sum::<f64>();
        let sums = (0..p)
            .map(|j| {
                cluster
                    .iter()
                    .map(|&i| x[(i, j)] * derivative[i])
                    .sum::<f64>()
            })
            .collect::<Vec<_>>();
        let mut u = vec![0.0; p];
        for j in 0..p {
            u[j] = (cluster
                .iter()
                .map(|&i| x[(i, j)] * derivative[i] * pearson[i])
                .sum::<f64>()
                - adjustment * sums[j] * sum_r)
                / (1.0 - rho);
            score[j] += u[j];
            for k in 0..p {
                bread[(j, k)] += (cluster
                    .iter()
                    .map(|&i| x[(i, j)] * x[(i, k)] * derivative[i].powi(2))
                    .sum::<f64>()
                    - adjustment * sums[j] * sums[k])
                    / (1.0 - rho);
            }
        }
        for j in 0..p {
            for k in 0..p {
                meat[(j, k)] += u[j] * u[k];
            }
        }
    }
    Ok(Equations {
        bread,
        meat,
        score,
        means,
        pearson,
    })
}

fn correlation(pearson: &[f64], clusters: &[Vec<usize>], p: usize) -> Result<f64> {
    let scale = pearson.iter().map(|v| v * v).sum::<f64>() / (pearson.len() - p) as f64;
    let mut pairs = 0usize;
    let mut cross = 0.0;
    for rows in clusters {
        pairs += rows.len() * (rows.len() - 1) / 2;
        cross += (rows.iter().map(|&i| pearson[i]).sum::<f64>().powi(2)
            - rows.iter().map(|&i| pearson[i].powi(2)).sum::<f64>())
            / 2.0;
    }
    if scale <= 0.0 || pairs <= p {
        return Err(parameter());
    }
    let rho = finite(cross / (scale * (pairs - p) as f64))?;
    let largest = clusters.iter().map(Vec::len).max().unwrap();
    if rho >= 1.0 || rho <= -1.0 / (largest - 1) as f64 {
        return Err(failed());
    }
    Ok(rho)
}

pub fn gee(
    y: &[f64],
    predictors: &[Vec<f64>],
    group: &Grouping,
    options: GeeOptions,
    control: &Control,
) -> Result<LongitudinalResult> {
    check_iteration(options.iteration)?;
    let design = prepare(y, predictors, options.constant, control)?;
    validate_response(y, options.family)?;
    if options.family == ResponseFamily::NegativeBinomial {
        return Err(parameter());
    }
    let clusters = rows(group, y.len(), control)?;
    let p = design.x.ncols();
    let mut beta = if options.family == ResponseFamily::Gaussian {
        least_squares(&design.x, y, None, control)?.0
    } else {
        let mut b = vec![0.0; p];
        if options.constant {
            let m = y.iter().sum::<f64>() / y.len() as f64;
            b[0] = if options.family == ResponseFamily::Binomial {
                (m / (1.0 - m)).ln()
            } else {
                m.ln()
            };
        }
        b
    };
    let mut rho = 0.0;
    let mut converged = None;
    for iteration in 0..options.iteration.max_iterations {
        let eq = equations(y, &design.x, &clusters, &beta, options.family, rho, control)?;
        let step = inverse(&eq.bread)?.as_ref() * eq.score.as_ref();
        // A bounded scoring step avoids overflowing the log link on the first iteration.
        let max_step = step.iter().copied().map(f64::abs).fold(0.0, f64::max);
        let damping = (5.0 / max_step).min(1.0);
        for j in 0..p {
            beta[j] = finite(beta[j] + damping * step[j])?;
        }
        let next = equations(y, &design.x, &clusters, &beta, options.family, rho, control)?;
        let new_rho = match options.correlation {
            WorkingCorrelation::Independence => 0.0,
            WorkingCorrelation::Exchangeable => correlation(&next.pearson, &clusters, p)?,
        };
        let delta = (new_rho - rho).abs().max(max_step);
        rho = new_rho;
        if delta <= options.iteration.tolerance {
            converged = Some(iteration + 1);
            break;
        }
    }
    let iterations = converged.ok_or_else(failed)?;
    let eq = equations(y, &design.x, &clusters, &beta, options.family, rho, control)?;
    let bread = inverse(&eq.bread)?;
    let covariance = bread.as_ref() * eq.meat.as_ref() * bread.as_ref();
    let (beta, covariance) = design.raw(&beta, Some(covariance));
    let covariance = covariance.unwrap();
    let scale = if options.family == ResponseFamily::Gaussian {
        eq.pearson.iter().map(|v| v * v).sum::<f64>() / (y.len() - p) as f64
    } else {
        1.0
    };
    control.check()?;
    Ok(LongitudinalResult {
        method: "gee".into(),
        family: options.family,
        observations: y.len(),
        group_counts: vec![group.levels],
        coefficients: coefficient_table(
            &beta,
            names(predictors.len(), options.constant),
            Some(&covariance),
            None,
            0.95,
        )?,
        coefficient_covariance: covariance_rows(&covariance),
        inference: "cluster_robust_wald_normal".into(),
        iterations,
        working_correlation: Some(options.correlation),
        correlation: Some(rho),
        scale: finite(scale)?,
        log_likelihood: None,
        aic: None,
        negative_binomial_alpha: None,
        variance_components: vec![],
        random_effects: vec![],
        residuals: y.iter().zip(&eq.means).map(|(y, m)| y - m).collect(),
        fixed_fitted: eq.means.clone(),
        fitted_values: eq.means,
    })
}
