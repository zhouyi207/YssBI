use super::common::*;
use yss_sci_contract::execution::ScientificExecutionControl as Control;
use yss_sci_contract::regression::models::*;
use yss_sci_linalg::{Col, Mat, MatrixExt, Solve};

pub fn penalized(
    y: &[f64],
    predictors: &[Vec<f64>],
    options: PenalizedOptions,
    control: &Control,
) -> Result<RegressionModelResult> {
    validate(y, predictors, control)?;
    check_iteration(options.iteration)?;
    if predictors.is_empty() || !options.lambda.is_finite() || options.lambda < 0.0 {
        return Err(parameter());
    }
    let design = Design::new(
        predictors,
        y.len(),
        options.constant,
        options.standardize,
        false,
        control,
    )?;
    let x = &design.x;
    let p = x.ncols();
    let n = y.len() as f64;
    let xtx = gram(x, None, control)?;
    let (beta, iterations, edf) = match options.penalty {
        Penalty::Ridge => {
            let a = Mat::from_fn(p, p, |i, j| {
                xtx[(i, j)] / n
                    + if i == j && !(options.constant && i == 0) {
                        options.lambda
                    } else {
                        0.0
                    }
            });
            let inv = inverse(&a)?;
            let rhs = Col::from_fn(p, |j| (0..y.len()).map(|i| x[(i, j)] * y[i] / n).sum());
            let b = inv.as_ref() * rhs.as_ref();
            let edf = (0..p)
                .map(|i| (0..p).map(|j| inv[(i, j)] * xtx[(j, i)] / n).sum::<f64>())
                .sum();
            (b.iter().copied().collect::<Vec<_>>(), 1, edf)
        }
        Penalty::Lasso => {
            let mut beta = vec![0.0; p];
            let mut residual = y.to_vec();
            let mut done = None;
            for iter in 1..=options.iteration.max_iterations {
                control.check()?;
                for j in 0..p {
                    let norm = xtx[(j, j)] / n;
                    if norm == 0.0 {
                        continue;
                    }
                    let rho = (0..y.len())
                        .map(|i| x[(i, j)] * (residual[i] + x[(i, j)] * beta[j]) / n)
                        .sum::<f64>();
                    let next = if options.constant && j == 0 {
                        rho / norm
                    } else {
                        rho.signum() * (rho.abs() - options.lambda).max(0.0) / norm
                    };
                    let diff = beta[j] - next;
                    for i in 0..y.len() {
                        if i % 1024 == 0 {
                            control.check()?;
                        }
                        residual[i] += x[(i, j)] * diff;
                    }
                    beta[j] = next;
                }
                let violation = (0..p)
                    .map(|j| {
                        let g = (0..y.len())
                            .map(|i| x[(i, j)] * residual[i] / n)
                            .sum::<f64>();
                        if options.constant && j == 0 {
                            g.abs()
                        } else if beta[j] != 0.0 {
                            (g - options.lambda * beta[j].signum()).abs()
                        } else {
                            (g.abs() - options.lambda).max(0.0)
                        }
                    })
                    .fold(0.0, f64::max);
                if violation <= options.iteration.tolerance * (1.0 + options.lambda) {
                    done = Some(iter);
                    break;
                }
            }
            let iterations = done.ok_or_else(failed)?;
            let edf = (usize::from(options.constant)
                + beta
                    .iter()
                    .skip(usize::from(options.constant))
                    .filter(|&&v| v != 0.0)
                    .count()) as f64;
            (beta, iterations, edf)
        }
    };
    let predicted = fitted(x, &beta);
    let rss = y
        .iter()
        .zip(&predicted)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>();
    let penalty = beta
        .iter()
        .enumerate()
        .filter(|(j, _)| !(options.constant && *j == 0))
        .map(|(_, b)| match options.penalty {
            Penalty::Ridge => b * b / 2.0,
            Penalty::Lasso => b.abs(),
        })
        .sum::<f64>();
    let objective = finite(rss / (2.0 * n) + options.lambda * penalty)?;
    let (raw, _) = design.raw(&beta, None);
    let mut r = result(
        match options.penalty {
            Penalty::Ridge => "ridge",
            Penalty::Lasso => "lasso",
        },
        y,
        predicted,
        raw,
        names(predictors.len(), options.constant),
        None,
        options.constant,
        None,
        RegressionDetails::Penalized {
            penalty: options.penalty,
            lambda: options.lambda,
            standardized: options.standardize,
            objective,
            effective_df: edf,
        },
    )?;
    r.iterations = iterations;
    control.check()?;
    Ok(r)
}

pub fn pls(
    y: &[f64],
    predictors: &[Vec<f64>],
    components: usize,
    standardize: bool,
    control: &Control,
) -> Result<RegressionModelResult> {
    validate(y, predictors, control)?;
    if components == 0 || components > predictors.len() || components >= y.len() {
        return Err(parameter());
    }
    let design = Design::new(predictors, y.len(), true, standardize, false, control)?;
    let p = predictors.len();
    let n = y.len();
    let mut x = Mat::from_fn(n, p, |i, j| design.x[(i, j + 1)]);
    let ym = mean(y);
    let mut target = y.iter().map(|v| v - ym).collect::<Vec<_>>();
    if yss_sci_linalg::matrix_rank(x.as_ref())
        .map_err(|_| failed())?
        .0
        < components
    {
        return Err(parameter());
    }
    let mut weights = Vec::new();
    let mut loadings = Vec::new();
    let mut qs = Vec::new();
    for _ in 0..components {
        control.check()?;
        let mut w = (0..p)
            .map(|j| (0..n).map(|i| x[(i, j)] * target[i]).sum::<f64>())
            .collect::<Vec<_>>();
        let norm = dot(&w, &w).sqrt();
        if !norm.is_finite() || norm <= f64::EPSILON {
            return Err(parameter());
        }
        for v in &mut w {
            *v /= norm;
        }
        let sign = w
            .iter()
            .max_by(|a, b| a.abs().total_cmp(&b.abs()))
            .copied()
            .unwrap_or(1.0)
            .signum();
        for v in &mut w {
            *v *= sign;
        }
        let t = fitted(&x, &w);
        let tt = dot(&t, &t);
        if tt <= f64::EPSILON || !tt.is_finite() {
            return Err(parameter());
        }
        let loading = (0..p)
            .map(|j| (0..n).map(|i| x[(i, j)] * t[i] / tt).sum::<f64>())
            .collect::<Vec<_>>();
        let q = dot(&t, &target) / tt;
        for i in 0..n {
            if i % 1024 == 0 {
                control.check()?;
            }
            target[i] -= t[i] * q;
            for j in 0..p {
                x[(i, j)] -= t[i] * loading[j];
            }
        }
        weights.push(w);
        loadings.push(loading);
        qs.push(q);
    }
    let w = Mat::from_fn(p, components, |i, j| weights[j][i]);
    let ptw = Mat::from_fn(components, components, |i, j| {
        dot(&loadings[i], &weights[j])
    });
    let a = ptw
        .checked_lu()
        .map_err(|_| failed())?
        .solve(&Col::from_iter(qs.iter().copied()));
    let b = w.as_ref() * a.as_ref();
    let mut beta = vec![ym];
    beta.extend(b.iter().copied());
    let pred = fitted(&design.x, &beta);
    let (raw, _) = design.raw(&beta, None);
    let mut r = result(
        "pls",
        y,
        pred,
        raw,
        names(p, true),
        None,
        true,
        None,
        RegressionDetails::Pls {
            components,
            standardized: standardize,
            x_weights: weights,
            x_loadings: loadings,
            y_loadings: qs,
        },
    )?;
    r.iterations = components;
    control.check()?;
    Ok(r)
}
