//! Linear IV GMM, Heckman two-step, normal–half-normal frontiers and SUR.
use super::common::*;
use crate::regression::models::{
    common::{normal_log_cdf, normal_log_cdf_density_ratio},
    glm,
};
use yss_sci_contract::causal::models::*;
use yss_sci_contract::regression::models::{GlmFamily, GlmLink, GlmOptions};
use yss_sci_linalg::matrix_rank;

pub fn gmm(
    y: &[f64],
    predictors: &[Vec<f64>],
    instruments: &[Vec<f64>],
    options: GmmOptions,
    control: &Control,
) -> Result<GmmResult> {
    validate(y, predictors, control)?;
    validate(y, instruments, control)?;
    let n = y.len();
    let design = Design::new(predictors, n, options.constant, true, true, control)?;
    let z_design = Design::new(instruments, n, options.constant, true, true, control)?;
    let x = &design.x;
    let z = &z_design.x;
    let p = x.ncols();
    let q = z.ncols();
    if q < p {
        return Err(parameter());
    }
    let mut response_scale = 0.0_f64;
    for (i, &value) in y.iter().enumerate() {
        if i.is_multiple_of(256) {
            control.check()?;
        }
        response_scale = response_scale.max(value.abs());
    }
    if response_scale == 0.0 {
        response_scale = 1.0;
    }
    let cross = Mat::from_fn(q, p, |j, k| {
        (0..n).map(|i| z[(i, j)] * x[(i, k)] / n as f64).sum()
    });
    let zy = Col::from_fn(q, |j| {
        (0..n)
            .map(|i| z[(i, j)] * (y[i] / response_scale) / n as f64)
            .sum()
    });
    let gram = Mat::from_fn(q, q, |j, k| {
        (0..n).map(|i| z[(i, j)] * z[(i, k)] / n as f64).sum()
    });
    let mut weight = inverse(&gram)?;
    let solve = |weight: &Mat<f64>| -> Result<(Vec<f64>, Mat<f64>)> {
        control.check()?;
        let a = cross.transpose() * weight.as_ref() * cross.as_ref();
        let bread = inverse(&a)?;
        let b = bread.as_ref() * cross.transpose() * weight.as_ref() * zy.as_ref();
        Ok((b.iter().copied().collect(), bread))
    };
    let score_cov = |residuals: &[f64]| -> Result<Mat<f64>> {
        let mut s = Mat::zeros(q, q);
        for i in 0..n {
            if i.is_multiple_of(256) {
                control.check()?;
            }
            for j in 0..q {
                for k in 0..q {
                    s[(j, k)] += z[(i, j)] * z[(i, k)] * residuals[i].powi(2) / n as f64;
                }
            }
        }
        Ok(s)
    };
    let (mut beta, mut bread) = solve(&weight)?;
    if options.two_step {
        let f = fitted(x, &beta);
        let residuals = y
            .iter()
            .zip(f)
            .map(|(y, f)| y / response_scale - f)
            .collect::<Vec<_>>();
        weight = inverse(&score_cov(&residuals)?)?;
        (beta, bread) = solve(&weight)?;
    }
    let mut predicted = fitted(x, &beta);
    let mut residuals = y
        .iter()
        .zip(&predicted)
        .map(|(y, f)| y / response_scale - f)
        .collect::<Vec<_>>();
    let a = bread.as_ref() * cross.transpose() * weight.as_ref();
    let mut cov = a.as_ref() * score_cov(&residuals)?.as_ref() * a.transpose();
    for j in 0..p {
        for k in 0..p {
            cov[(j, k)] /= n as f64;
        }
    }
    let g = Col::from_fn(q, |j| {
        (0..n).map(|i| z[(i, j)] * residuals[i] / n as f64).sum()
    });
    let hansen_j = if options.two_step && q > p {
        let wg = weight.as_ref() * g.as_ref();
        Some(chi_square(
            n as f64 * g.iter().zip(wg.iter()).map(|(g, w)| g * w).sum::<f64>(),
            q - p,
        )?)
    } else {
        None
    };
    let mut moments = vec![];
    if options.constant {
        moments.push(mean(&residuals));
    }
    moments.extend(instruments.iter().map(|z| {
        z.iter()
            .zip(&residuals)
            .map(|(z, e)| z * e / n as f64)
            .sum::<f64>()
    }));
    // Restore response and predictor units in one coordinate map. A separate
    // response-scale square or raw-design covariance can overflow unnecessarily.
    let coordinates = design.raw_jacobian(response_scale);
    for j in 0..p {
        control.check()?;
        for k in 0..p {
            finite(coordinates[(j, k)])?;
        }
    }
    let (beta, covariance) = transform(&beta, Some(cov), &coordinates);
    let covariance = covariance.expect("covariance");
    for j in 0..p {
        control.check()?;
        for k in 0..p {
            finite(covariance[(j, k)])?;
        }
    }
    for (i, value) in predicted
        .iter_mut()
        .chain(&mut residuals)
        .chain(&mut moments)
        .enumerate()
    {
        if i.is_multiple_of(256) {
            control.check()?;
        }
        *value = finite(*value * response_scale)?;
    }
    Ok(GmmResult {
        observations: n,
        instruments: q,
        steps: if options.two_step { 2 } else { 1 },
        coefficients: coefficient_table(
            &beta,
            names(predictors.len(), options.constant),
            Some(&covariance),
            None,
            0.95,
        )?,
        covariance: rows(&covariance),
        fitted: predicted,
        residuals,
        moments,
        hansen_j,
    })
}

pub fn heckman(
    y: &[Option<f64>],
    selected: &[f64],
    predictors: &[Vec<f64>],
    selection_predictors: &[Vec<f64>],
    options: HeckmanOptions,
    control: &Control,
) -> Result<HeckmanResult> {
    let mut result = heckman_point(
        y,
        selected,
        predictors,
        selection_predictors,
        options,
        control,
    )?;
    let width = result.outcome_coefficients.len();
    let covariance = bootstrap_covariance(y.len(), width, options.bootstrap, control, |indices| {
        let sample = |v: &[f64]| indices.iter().map(|&i| v[i]).collect::<Vec<_>>();
        let xs = predictors.iter().map(|v| sample(v)).collect::<Vec<_>>();
        let zs = selection_predictors
            .iter()
            .map(|v| sample(v))
            .collect::<Vec<_>>();
        let ys = indices.iter().map(|&i| y[i]).collect::<Vec<_>>();
        let fit = heckman_point(&ys, &sample(selected), &xs, &zs, options, control)?;
        Ok(fit
            .outcome_coefficients
            .iter()
            .map(|c| c.estimate)
            .collect())
    })?;
    if let Some(covariance) = covariance {
        result.outcome_coefficients = coefficient_table(
            &result
                .outcome_coefficients
                .iter()
                .map(|c| c.estimate)
                .collect::<Vec<_>>(),
            result
                .outcome_coefficients
                .iter()
                .map(|c| c.term.clone())
                .collect(),
            Some(&covariance),
            None,
            0.95,
        )?;
        result.outcome_covariance = Some(rows(&covariance));
    }
    result.bootstrap_replications = options.bootstrap.replications;
    Ok(result)
}

fn heckman_point(
    y: &[Option<f64>],
    selected: &[f64],
    predictors: &[Vec<f64>],
    selection_predictors: &[Vec<f64>],
    options: HeckmanOptions,
    control: &Control,
) -> Result<HeckmanResult> {
    validate(selected, predictors, control)?;
    validate(selected, selection_predictors, control)?;
    let count = binary(selected)?;
    if y.len() != selected.len() || selection_predictors.is_empty() {
        return Err(parameter());
    }
    let n = y.len();
    {
        let base = Design::new(predictors, n, true, true, true, control)?;
        let all = predictors
            .iter()
            .chain(selection_predictors)
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        let full = Design::new(&all, n, true, true, false, control)?;
        let (rank, _) = matrix_rank(full.x.as_ref()).map_err(|_| failed())?;
        if rank <= base.x.ncols() {
            return Err(parameter());
        }
    }
    let selection = glm(
        selected,
        selection_predictors,
        GlmOptions {
            constant: true,
            family: GlmFamily::Binomial,
            link: GlmLink::Probit,
            fractional: false,
            iteration: options.iteration,
        },
        control,
    )?;
    let indices = (0..n).filter(|&i| selected[i] == 1.0).collect::<Vec<_>>();
    let response = indices
        .iter()
        .map(|&i| y[i].filter(|v| v.is_finite()).ok_or_else(parameter))
        .collect::<Result<Vec<_>>>()?;
    let eta = (0..n)
        .map(|i| {
            selection.coefficients[0].estimate
                + selection_predictors
                    .iter()
                    .enumerate()
                    .map(|(j, x)| selection.coefficients[j + 1].estimate * x[i])
                    .sum::<f64>()
        })
        .collect::<Vec<_>>();
    let mills = indices
        .iter()
        .map(|&i| {
            (-0.5 * eta[i] * eta[i]
                - 0.5 * (2.0 * std::f64::consts::PI).ln()
                - normal_log_cdf(eta[i]))
            .exp()
        })
        .collect::<Vec<_>>();
    let design = {
        let predictor_rows = predictors
            .iter()
            .map(|v| indices.iter().map(|&i| v[i]).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let columns = predictor_rows
            .iter()
            .map(Vec::as_slice)
            .chain(std::iter::once(mills.as_slice()))
            .collect::<Vec<_>>();
        Design::new(&columns, count, true, true, true, control)?
    };
    let (beta, _) = least_squares(&design.x, &response, None, control)?;
    let predicted = fitted(&design.x, &beta);
    let residuals = response
        .iter()
        .zip(&predicted)
        .map(|(y, f)| y - f)
        .collect::<Vec<_>>();
    let (beta, _) = design.raw(&beta, None);
    let lambda = *beta.last().expect("Mills coefficient");
    let mut scale = finite(lambda.abs())?;
    for (i, &residual) in residuals.iter().enumerate() {
        if i.is_multiple_of(256) {
            control.check()?;
        }
        scale = scale.max(finite(residual.abs())?);
    }
    if scale == 0.0 {
        return Err(failed());
    }
    let mut residual_variance = 0.0;
    let mut delta_mean = 0.0;
    for (row, (&index, &mills)) in indices.iter().zip(&mills).enumerate() {
        if row.is_multiple_of(256) {
            control.check()?;
        }
        residual_variance += (residuals[row] / scale).powi(2) / count as f64;
        delta_mean += mills * (mills + eta[index]) / count as f64;
    }
    // Report sigma directly in response units; its raw variance need not be
    // representable. Both normalized components stay bounded before squaring.
    let variance = residual_variance + (lambda / scale).powi(2) * delta_mean;
    let sigma = finite(variance.sqrt() * scale)?;
    if sigma <= 0.0 {
        return Err(failed());
    }
    let rho = finite(lambda / sigma)?;
    // A two-step moment estimate outside the bivariate-normal parameter space is
    // reported as a failure, rather than silently clipping the correlation.
    if rho.abs() >= 1.0 {
        return Err(failed());
    }
    let mut terms = names(predictors.len(), true);
    terms.push("inverse_mills".into());
    Ok(HeckmanResult {
        observations: n,
        selected_observations: count,
        selection_coefficients: selection.coefficients,
        outcome_coefficients: coefficient_table(&beta, terms, None, None, 0.95)?,
        outcome_covariance: None,
        selection_probabilities: selection.fitted,
        inverse_mills: mills,
        selected_rows: indices.iter().map(|i| i + 1).collect(),
        fitted_selected: predicted,
        residuals_selected: residuals,
        sigma,
        rho,
        bootstrap_replications: 0,
    })
}

pub fn frontier(
    y: &[f64],
    predictors: &[Vec<f64>],
    options: FrontierOptions,
    control: &Control,
) -> Result<FrontierResult> {
    validate(y, predictors, control)?;
    check_iteration(options.iteration)?;
    let design = Design::new(predictors, y.len(), options.constant, true, true, control)?;
    let x = &design.x;
    let p = x.ncols();
    if y.len() <= p + 2 {
        return Err(parameter());
    }
    let mut response_scale = 0.0_f64;
    for (i, value) in y.iter().enumerate() {
        if i.is_multiple_of(512) {
            control.check()?;
        }
        response_scale = response_scale.max(value.abs());
    }
    if response_scale == 0.0 {
        response_scale = 1.0;
    }
    let response = y
        .iter()
        .map(|value| value / response_scale)
        .collect::<Vec<_>>();
    let (ols, _) = least_squares(x, &response, None, control)?;
    let sd = response
        .iter()
        .zip(fitted(x, &ols))
        .map(|(value, fitted)| (value - fitted).powi(2) / response.len() as f64)
        .sum::<f64>()
        .sqrt();
    if !sd.is_finite() || sd <= 0.0 {
        return Err(parameter());
    }
    let sign = if options.cost { -1.0 } else { 1.0 };
    let objective = |b: &[f64]| -> Result<f64> {
        control.check()?;
        let su = b[p].exp();
        let sv = b[p + 1].exp();
        let sigma = su.hypot(sv);
        if su <= 0.0 || sv <= 0.0 || !sigma.is_finite() {
            return Err(failed());
        }
        let mut loss = 0.0;
        for (i, &y) in response.iter().enumerate() {
            if i.is_multiple_of(512) {
                control.check()?;
            }
            let fitted = (0..p).map(|j| x[(i, j)] * b[j]).sum::<f64>();
            let e = (y - fitted) / sigma;
            loss += sigma.ln() + 0.5 * e * e + 0.5 * (2.0 * std::f64::consts::PI).ln()
                - 2.0_f64.ln()
                - normal_log_cdf(-sign * e * su / sv);
        }
        finite(loss / y.len() as f64)
    };
    let mut initial = ols;
    if options.constant {
        initial[0] += sign * sd * (2.0 / std::f64::consts::PI).sqrt();
    }
    initial.extend([sd.ln(), (sd * 0.7).ln()]);
    let optimum = minimize(&objective, initial, options.iteration, control)?;
    let su = optimum.beta[p].exp();
    let sv = optimum.beta[p + 1].exp();
    // A near-zero component has nonregular inference and no identified interior
    // information matrix. Do not turn that boundary into ordinary Wald results.
    if su / sv < 1e-5 || sv / su < 1e-5 {
        return Err(failed());
    }
    let information = hessian(&objective, &optimum.beta, control)?;
    let inv = inverse(&information)?;
    let cov = Mat::from_fn(p, p, |i, j| inv[(i, j)] / y.len() as f64);
    let mut predicted = fitted(x, &optimum.beta[..p]);
    let mut residuals = response
        .iter()
        .zip(&predicted)
        .map(|(y, f)| y - f)
        .collect::<Vec<_>>();
    let sigma = su.hypot(sv);
    let conditional_sd = su * (sv / sigma);
    let raw_conditional_sd = finite(conditional_sd * response_scale)?;
    let mut inefficiency = Vec::with_capacity(y.len());
    let mut efficiency = Vec::with_capacity(y.len());
    for (i, e) in residuals.iter_mut().enumerate() {
        if i.is_multiple_of(512) {
            control.check()?;
        }
        let mu = -sign * *e * (su / sigma).powi(2);
        let z = mu / conditional_sd;
        let log_ratio = normal_log_cdf_density_ratio(z);
        let mills = (-log_ratio).exp();
        inefficiency.push(finite((mu + conditional_sd * mills) * response_scale)?);
        // Completing the square expresses E[exp(-u)] as a ratio of CDF/density
        // ratios, without subtracting quadratic terms in raw response units.
        efficiency.push(finite(
            (normal_log_cdf_density_ratio(z - raw_conditional_sd) - log_ratio).exp(),
        )?);
        predicted[i] = finite(predicted[i] * response_scale)?;
        *e = finite(*e * response_scale)?;
    }
    let coordinates = design.raw_jacobian(response_scale);
    let (beta, cov) = transform(&optimum.beta[..p], Some(cov), &coordinates);
    let cov = cov.expect("covariance");
    for i in 0..p {
        control.check()?;
        finite(beta[i])?;
        for j in 0..p {
            finite(cov[(i, j)])?;
        }
    }
    Ok(FrontierResult {
        observations: y.len(),
        cost: options.cost,
        coefficients: coefficient_table(
            &beta,
            names(predictors.len(), options.constant),
            Some(&cov),
            None,
            0.95,
        )?,
        covariance: rows(&cov),
        sigma_u: finite(su * response_scale)?,
        sigma_v: finite(sv * response_scale)?,
        log_likelihood: finite(-(optimum.value + response_scale.ln()) * y.len() as f64)?,
        iterations: optimum.iterations,
        frontier: predicted,
        residuals,
        conditional_inefficiency: inefficiency,
        efficiency,
    })
}

pub fn sur(
    responses: &[Vec<f64>],
    predictors: &[Vec<f64>],
    equation_predictors: &[Vec<usize>],
    constant: bool,
    control: &Control,
) -> Result<SurResult> {
    control.check()?;
    let m = responses.len();
    if m < 2 || equation_predictors.len() != m {
        return Err(parameter());
    }
    let n = responses[0].len();
    validate(&responses[0], responses, control)?;
    validate(&responses[0], predictors, control)?;
    let mut designs = Vec::with_capacity(m);
    let mut errors = Vec::with_capacity(m);
    let mut response_scales = Vec::with_capacity(m);
    let mut offsets = vec![0usize];
    for (i, indices) in equation_predictors.iter().enumerate() {
        control.check()?;
        if indices.iter().any(|&j| j >= predictors.len())
            || indices
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != indices.len()
        {
            return Err(parameter());
        }
        let xs = indices
            .iter()
            .map(|&j| predictors[j].as_slice())
            .collect::<Vec<_>>();
        let design = Design::new(&xs, n, constant, true, true, control)?;
        let mut scale = 0.0_f64;
        for (row, value) in responses[i].iter().enumerate() {
            if row.is_multiple_of(256) {
                control.check()?;
            }
            scale = scale.max(value.abs());
        }
        if scale == 0.0 {
            scale = 1.0;
        }
        let mut residuals = responses[i]
            .iter()
            .map(|value| value / scale)
            .collect::<Vec<_>>();
        let (beta, _) = least_squares(&design.x, &residuals, None, control)?;
        for (row, (residual, fitted)) in residuals
            .iter_mut()
            .zip(fitted(&design.x, &beta))
            .enumerate()
        {
            if row.is_multiple_of(256) {
                control.check()?;
            }
            *residual = finite(*residual - fitted)?;
        }
        errors.push(residuals);
        response_scales.push(scale);
        offsets.push(
            offsets[i]
                .checked_add(design.x.ncols())
                .ok_or_else(parameter)?,
        );
        designs.push(design);
    }
    let mut sigma = Mat::zeros(m, m);
    for a in 0..m {
        for b in 0..=a {
            let mut covariance = 0.0;
            for (row, (ea, eb)) in errors[a].iter().zip(&errors[b]).enumerate() {
                if row.is_multiple_of(256) {
                    control.check()?;
                }
                covariance += ea * eb / n as f64;
            }
            sigma[(a, b)] = finite(covariance)?;
            sigma[(b, a)] = covariance;
        }
    }
    drop(errors);
    let width = offsets[m];
    // Per-equation response coordinates keep the residual precision finite;
    // restoring units before this solve can overflow even for finite output.
    let (beta, covariance) = {
        let precision = inverse(&sigma)?;
        let mut gram = Mat::zeros(width, width);
        let mut rhs = Col::zeros(width);
        for a in 0..m {
            for b in 0..m {
                let xa = &designs[a].x;
                let xb = &designs[b].x;
                for row in 0..n {
                    if row.is_multiple_of(256) {
                        control.check()?;
                    }
                    let response = responses[b][row] / response_scales[b];
                    for j in 0..xa.ncols() {
                        let weighted = precision[(a, b)] * xa[(row, j)];
                        rhs[offsets[a] + j] += weighted * response;
                        for k in 0..xb.ncols() {
                            gram[(offsets[a] + j, offsets[b] + k)] += weighted * xb[(row, k)];
                        }
                    }
                }
            }
        }
        let covariance = inverse(&gram)?;
        let beta = covariance.as_ref() * rhs.as_ref();
        (beta.iter().copied().collect::<Vec<_>>(), covariance)
    };
    let mut jacobian = Mat::zeros(width, width);
    let mut terms = Vec::with_capacity(width);
    for i in 0..m {
        control.check()?;
        let start = offsets[i];
        let p = offsets[i + 1] - start;
        let j = designs[i].raw_jacobian(response_scales[i]);
        for a in 0..p {
            for b in 0..p {
                jacobian[(start + a, start + b)] = finite(j[(a, b)])?;
            }
        }
        if constant {
            terms.push("intercept".into());
        }
        terms.extend(equation_predictors[i].iter().map(|j| format!("x{}", j + 1)));
    }
    let (raw_beta, covariance) = transform(&beta, Some(covariance), &jacobian);
    let covariance = covariance.expect("SUR coefficient covariance");
    for i in 0..width {
        control.check()?;
        finite(raw_beta[i])?;
        for j in 0..width {
            finite(covariance[(i, j)])?;
        }
    }
    let mut coefficients =
        coefficient_table(&raw_beta, terms, Some(&covariance), None, 0.95)?.into_iter();
    let mut equations = Vec::with_capacity(m);
    for i in 0..m {
        control.check()?;
        let start = offsets[i];
        let end = offsets[i + 1];
        let mut predicted = fitted(&designs[i].x, &beta[start..end]);
        let mut residuals = Vec::with_capacity(n);
        for (row, (value, response)) in predicted.iter_mut().zip(&responses[i]).enumerate() {
            if row.is_multiple_of(256) {
                control.check()?;
            }
            *value = finite(*value * response_scales[i])?;
            residuals.push(finite(response - *value)?);
        }
        equations.push(SurEquation {
            predictors: equation_predictors[i].iter().map(|j| j + 1).collect(),
            coefficients: coefficients.by_ref().take(end - start).collect(),
            fitted: predicted,
            residuals,
        });
    }
    for a in 0..m {
        control.check()?;
        for b in 0..=a {
            let covariance = finite(
                sigma[(a, b)]
                    * response_scales[a].max(response_scales[b])
                    * response_scales[a].min(response_scales[b]),
            )?;
            sigma[(a, b)] = covariance;
            sigma[(b, a)] = covariance;
        }
    }
    control.check()?;
    Ok(SurResult {
        observations: n,
        equations,
        error_covariance: rows(&sigma),
        coefficient_covariance: rows(&covariance),
    })
}
