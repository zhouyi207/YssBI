use super::*;
use std::f64::consts::TAU;

struct Component {
    grouping: usize,
    term: usize,
    z: Vec<f64>,
    scale: f64,
    kernel: Mat<f64>,
}
struct Profile {
    objective: f64,
    beta: Vec<f64>,
    covariance: Mat<f64>,
    precision_residual: Col<f64>,
    sigma2: f64,
}
fn profile(
    y: &[f64],
    x: &Mat<f64>,
    components: &[Component],
    theta: &[f64],
    reml: bool,
    control: &Control,
) -> Result<Profile> {
    control.check()?;
    let n = y.len();
    let covariance = Mat::from_fn(n, n, |i, j| {
        f64::from(u8::from(i == j))
            + components
                .iter()
                .zip(theta)
                .map(|(c, t)| t * t * c.kernel[(i, j)])
                .sum::<f64>()
    });
    let factor = covariance.checked_cholesky().map_err(|_| failed())?;
    let vi_x = factor.solve(x);
    let vi_y = factor.solve(&Col::from_iter(y.iter().copied()));
    let information = x.transpose() * vi_x.as_ref();
    let information_factor = information.checked_cholesky().map_err(|_| failed())?;
    let covariance = information_factor.solve(&Mat::identity(x.ncols(), x.ncols()));
    let beta = information_factor.solve(&(x.transpose() * vi_y.as_ref()));
    let residual = Col::from_fn(n, |i| {
        y[i] - (0..x.ncols()).map(|j| x[(i, j)] * beta[j]).sum::<f64>()
    });
    let precision_residual = factor.solve(&residual);
    let df = if reml { n - x.ncols() } else { n };
    let sigma2 = finite(
        (0..n)
            .map(|i| residual[i] * precision_residual[i])
            .sum::<f64>()
            / df as f64,
    )?;
    if sigma2 <= 0.0 {
        return Err(failed());
    }
    let objective = 0.5
        * (df as f64 * (TAU.ln() + 1.0 + sigma2.ln())
            + log_determinant(&factor)?
            + if reml {
                log_determinant(&information_factor)?
            } else {
                0.0
            });
    control.check()?;
    Ok(Profile {
        objective: finite(objective)?,
        beta: beta.iter().copied().collect(),
        covariance: Mat::from_fn(x.ncols(), x.ncols(), |i, j| sigma2 * covariance[(i, j)]),
        precision_residual,
        sigma2,
    })
}

pub fn linear_mixed(
    y: &[f64],
    predictors: &[Vec<f64>],
    groups: &[Grouping],
    random_predictors: &[Vec<f64>],
    options: MixedOptions,
    control: &Control,
) -> Result<LongitudinalResult> {
    check_iteration(options.iteration)?;
    let design = prepare(y, predictors, options.constant, control)?;
    if groups.is_empty() || (!random_predictors.is_empty() && groups.len() != 1) {
        return Err(parameter());
    }
    validate(y, random_predictors, control)?;
    let clusters = groups
        .iter()
        .map(|g| rows(g, y.len(), control))
        .collect::<Result<Vec<_>>>()?;
    if options.nested {
        for pair in groups.windows(2) {
            let mut parents = vec![None; pair[0].levels];
            for (&child, &parent) in pair[0].codes.iter().zip(&pair[1].codes) {
                if parents[child].is_some_and(|g| g != parent) {
                    return Err(parameter());
                }
                parents[child] = Some(parent);
            }
            if pair[0].levels <= pair[1].levels {
                return Err(parameter());
            }
        }
    }
    let mut components = Vec::new();
    for (g, group) in groups.iter().enumerate() {
        for term in 0..=if g == 0 { random_predictors.len() } else { 0 } {
            let (z, scale) = if term == 0 {
                (vec![1.0; y.len()], 1.0)
            } else {
                let values = &random_predictors[term - 1];
                let scale = values.iter().copied().map(f64::abs).fold(0.0, f64::max);
                if scale == 0.0 {
                    return Err(parameter());
                }
                // Scaling without centering preserves the declared zero intercept/slope covariance.
                (values.iter().map(|v| v / scale).collect(), scale)
            };
            let kernel = Mat::from_fn(y.len(), y.len(), |i, j| {
                if group.codes[i] == group.codes[j] {
                    z[i] * z[j]
                } else {
                    0.0
                }
            });
            components.push(Component {
                grouping: g,
                term,
                z,
                scale,
                kernel,
            });
        }
    }
    // Distinguish identifiable covariance components, including residual variance.
    let covariance_design_rows = y
        .len()
        .checked_add(1)
        .and_then(|next| y.len().checked_mul(next))
        .map(|n| n / 2)
        .ok_or_else(parameter)?;
    let covariance_design = Mat::from_fn(covariance_design_rows, components.len() + 1, |row, k| {
        let i = (((8 * row + 1) as f64).sqrt() as usize - 1) / 2;
        let j = row - i * (i + 1) / 2;
        if k == components.len() {
            f64::from(u8::from(i == j))
        } else {
            components[k].kernel[(i, j)]
        }
    });
    if yss_sci_linalg::matrix_rank(covariance_design.as_ref())
        .map_err(|_| failed())?
        .0
        != components.len() + 1
    {
        return Err(parameter());
    }
    drop(covariance_design);
    let reml = options.estimation == MixedEstimation::Reml;
    let objective = |theta: &[f64]| {
        profile(y, &design.x, &components, theta, reml, control)
            .map(|f| f.objective / y.len() as f64)
    };
    let minimum = minimize(
        &objective,
        vec![1.0; components.len()],
        options.iteration,
        control,
    )?;
    let fit = profile(y, &design.x, &components, &minimum.beta, reml, control)?;
    let fixed_fitted = fitted(&design.x, &fit.beta);
    let mut fitted_values = fixed_fitted.clone();
    let mut random_effects = Vec::new();
    let mut variance_components = Vec::new();
    for (c, theta) in components.iter().zip(&minimum.beta) {
        let ratio = theta * theta;
        variance_components.push(VarianceComponent {
            grouping: c.grouping + 1,
            term: c.term,
            variance: finite(fit.sigma2 * ratio / c.scale.powi(2))?,
            boundary: ratio <= 1e-8,
        });
        for (level, rows) in clusters[c.grouping].iter().enumerate() {
            let b = ratio
                * rows
                    .iter()
                    .map(|&i| c.z[i] * fit.precision_residual[i])
                    .sum::<f64>();
            for &i in rows {
                fitted_values[i] += c.z[i] * b;
            }
            random_effects.push(RandomEffect {
                grouping: c.grouping + 1,
                level: level + 1,
                term: c.term,
                estimate: finite(b / c.scale)?,
            });
        }
    }
    let (beta, covariance) = design.raw(&fit.beta, Some(fit.covariance));
    let covariance = covariance.unwrap();
    // Restricted likelihood depends on the fixed-design units; undo standardization.
    let log_likelihood = -fit.objective
        - if reml {
            design.scales.iter().map(|s| s.ln()).sum::<f64>()
        } else {
            0.0
        };
    control.check()?;
    Ok(LongitudinalResult {
        method: if reml { "lmm_reml" } else { "lmm_ml" }.into(),
        family: ResponseFamily::Gaussian,
        observations: y.len(),
        group_counts: groups.iter().map(|g| g.levels).collect(),
        coefficients: coefficient_table(
            &beta,
            names(predictors.len(), options.constant),
            Some(&covariance),
            None,
        )?,
        coefficient_covariance: covariance_rows(&covariance),
        inference: "approximate_wald_normal".into(),
        iterations: minimum.iterations,
        working_correlation: None,
        correlation: None,
        scale: fit.sigma2,
        log_likelihood: Some(log_likelihood),
        aic: if reml {
            None
        } else {
            Some(-2.0 * log_likelihood + 2.0 * (beta.len() + components.len() + 1) as f64)
        },
        negative_binomial_alpha: None,
        variance_components,
        random_effects,
        fixed_fitted,
        residuals: y.iter().zip(&fitted_values).map(|(y, m)| y - m).collect(),
        fitted_values,
    })
}
