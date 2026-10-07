use super::common::*;
use statrs::function::gamma::ln_gamma;
use yss_sci_contract::execution::ScientificExecutionControl as Control;
use yss_sci_contract::regression::models::*;
use yss_sci_linalg::{Mat, MatrixExt};

type FirthEvaluation = (Vec<f64>, Vec<f64>, Mat<f64>, f64);

pub(super) fn logistic(z: f64) -> f64 {
    if z >= 0.0 {
        1.0 / (1.0 + (-z).exp())
    } else {
        let e = z.exp();
        e / (1.0 + e)
    }
}
fn softplus(z: f64) -> f64 {
    z.max(0.0) + (-z.abs()).exp().ln_1p()
}
fn logadd(a: f64, b: f64) -> f64 {
    let m = a.max(b);
    if m == f64::NEG_INFINITY {
        m
    } else {
        m + ((a - m).exp() + (b - m).exp()).ln()
    }
}
pub fn firth_logit(
    y: &[f64],
    predictors: &[Vec<f64>],
    constant: bool,
    iteration: IterationOptions,
    control: &Control,
) -> Result<RegressionModelResult> {
    validate(y, predictors, control)?;
    check_iteration(iteration)?;
    if y.iter().any(|&v| v != 0.0 && v != 1.0) || !y.contains(&0.0) || !y.contains(&1.0) {
        return Err(parameter());
    }
    let design = Design::new(predictors, y.len(), constant, true, true, control)?;
    let x = &design.x;
    let p = x.ncols();
    let mut beta = vec![0.0; p];
    let evaluate = |b: &[f64]| -> Result<FirthEvaluation> {
        let eta = fitted(x, b);
        let mu = eta.iter().map(|&e| logistic(e)).collect::<Vec<_>>();
        let weights = mu.iter().map(|m| m * (1.0 - m)).collect::<Vec<_>>();
        let fisher = gram(x, Some(&weights), control)?;
        let inv = inverse(&fisher)?;
        let lower = fisher.checked_cholesky().map_err(|_| failed())?.lower();
        let logdet = (0..p).map(|i| 2.0 * lower[(i, i)].ln()).sum::<f64>();
        let ll = eta
            .iter()
            .zip(y)
            .map(|(e, y)| y * e - softplus(*e))
            .sum::<f64>();
        Ok((mu, weights, inv, finite(ll + 0.5 * logdet)?))
    };
    let mut current = evaluate(&beta)?;
    let mut done = None;
    for iter in 1..=iteration.max_iterations {
        control.check()?;
        let mut score = vec![0.0; p];
        for i in 0..y.len() {
            if i % 1024 == 0 {
                control.check()?;
            }
            let leverage = current.1[i]
                * (0..p)
                    .map(|j| {
                        (0..p)
                            .map(|k| x[(i, j)] * current.2[(j, k)] * x[(i, k)])
                            .sum::<f64>()
                    })
                    .sum::<f64>();
            let adjustment = y[i] - current.0[i] + leverage * (0.5 - current.0[i]);
            for j in 0..p {
                score[j] += x[(i, j)] * adjustment;
            }
        }
        let increment = (0..p)
            .map(|i| (0..p).map(|j| current.2[(i, j)] * score[j]).sum::<f64>())
            .collect::<Vec<_>>();
        if increment.iter().fold(0.0_f64, |m, v| m.max(v.abs())) <= iteration.tolerance {
            done = Some(iter);
            break;
        }
        let mut step = 1.0;
        let mut next = None;
        for _ in 0..40 {
            let b = beta
                .iter()
                .zip(&increment)
                .map(|(a, d)| a + step * d)
                .collect::<Vec<_>>();
            match evaluate(&b) {
                Ok(v) if v.3 >= current.3 => {
                    next = Some((b, v));
                    break;
                }
                Err(
                    e @ (yss_sci_contract::execution::ScientificComputationError::Cancelled
                    | yss_sci_contract::execution::ScientificComputationError::DeadlineExceeded),
                ) => return Err(e),
                _ => step *= 0.5,
            }
        }
        let (b, v) = next.ok_or_else(failed)?;
        beta = b;
        current = v;
    }
    let iterations = done.ok_or_else(failed)?;
    let (raw, cov) = design.raw(&beta, Some(current.2));
    let mut r = result(
        "firth_logit",
        y,
        current.0,
        raw,
        names(predictors.len(), constant),
        cov,
        constant,
        None,
        RegressionDetails::Firth {
            penalized_log_likelihood: current.3 + design.scales.iter().map(|s| s.ln()).sum::<f64>(),
            covariance_method: "unpenalized Fisher information, Wald normal approximation".into(),
        },
    )?;
    r.statistics.log_likelihood = Some(
        fitted(x, &beta)
            .iter()
            .zip(y)
            .map(|(e, y)| y * e - softplus(*e))
            .sum(),
    );
    r.statistics.r_squared = None;
    r.statistics.adjusted_r_squared = None;
    r.iterations = iterations;
    control.check()?;
    Ok(r)
}

fn nb_log_pmf(y: f64, eta: f64, log_alpha: f64) -> Result<f64> {
    if !(-12.0..=12.0).contains(&log_alpha) {
        return Err(failed());
    }
    let r = (-log_alpha).exp();
    let z = eta + log_alpha;
    finite(
        ln_gamma(y + r) - ln_gamma(r) - ln_gamma(y + 1.0) - r * softplus(z) + y * (z - softplus(z)),
    )
}
fn ordinal_cuts(b: &[f64], p: usize, k: usize) -> Vec<f64> {
    let mut cuts = vec![b[p]];
    for j in 1..k - 1 {
        cuts.push(cuts[j - 1] + b[p + j].exp());
    }
    cuts
}
fn conditional_loglike(
    y: &[f64],
    eta: &[f64],
    groups: &[Vec<usize>],
    control: &Control,
) -> Result<f64> {
    let mut ll = 0.0;
    for rows in groups {
        control.check()?;
        let m = rows.iter().filter(|&&i| y[i] == 1.0).count();
        if m == 0 || m == rows.len() {
            continue;
        }
        // Complement conditioning reduces the dynamic-programming degree when cases dominate.
        let complement = m > rows.len() / 2;
        let degree = if complement { rows.len() - m } else { m };
        let mut dp = vec![f64::NEG_INFINITY; degree + 1];
        dp[0] = 0.0;
        for (count, &i) in rows.iter().enumerate() {
            if count % 128 == 0 {
                control.check()?;
            }
            let e = if complement { -eta[i] } else { eta[i] };
            for j in (1..=degree.min(count + 1)).rev() {
                dp[j] = logadd(dp[j], dp[j - 1] + e);
            }
        }
        let observed = rows
            .iter()
            .filter(|&&i| if complement { y[i] == 0.0 } else { y[i] == 1.0 })
            .map(|&i| if complement { -eta[i] } else { eta[i] })
            .sum::<f64>();
        ll += observed - dp[degree];
    }
    finite(ll)
}
pub fn likelihood(
    y: &[f64],
    predictors: &[Vec<f64>],
    inflation: &[Vec<f64>],
    groups: Option<&[usize]>,
    options: LikelihoodOptions,
    control: &Control,
) -> Result<RegressionModelResult> {
    validate(y, predictors, control)?;
    validate(y, inflation, control)?;
    check_iteration(options.iteration)?;
    let categorical = matches!(
        options.method,
        LikelihoodMethod::MultinomialLogit | LikelihoodMethod::OrdinalLogit
    );
    let conditional = options.method == LikelihoodMethod::ConditionalLogit;
    let constant = options.constant
        && !matches!(
            options.method,
            LikelihoodMethod::OrdinalLogit | LikelihoodMethod::ConditionalLogit
        );
    let design = Design::new(predictors, y.len(), constant, true, true, control)?;
    let x = &design.x;
    let p = x.ncols();
    let n = y.len();
    let mut categories = 0;
    let mut group_rows = vec![];
    let mut informative = 0;
    let mut dropped = 0;
    match options.method {
        LikelihoodMethod::NegativeBinomial
        | LikelihoodMethod::ZeroInflatedPoisson
        | LikelihoodMethod::ZeroInflatedNegativeBinomial => {
            if y.iter().any(|&v| v < 0.0 || v.fract() != 0.0) || mean(y) == 0.0 {
                return Err(parameter());
            }
        }
        LikelihoodMethod::Beta => {
            if y.iter().any(|&v| v <= 0.0 || v >= 1.0) {
                return Err(parameter());
            }
        }
        LikelihoodMethod::Tobit => {
            if !options.lower.is_finite()
                || options
                    .upper
                    .is_some_and(|u| !u.is_finite() || u <= options.lower)
                || y.iter()
                    .any(|&v| v < options.lower || options.upper.is_some_and(|u| v > u))
                || !y
                    .iter()
                    .any(|&v| v > options.lower && options.upper.is_none_or(|u| v < u))
            {
                return Err(parameter());
            }
        }
        LikelihoodMethod::MultinomialLogit | LikelihoodMethod::OrdinalLogit => {
            if y.iter().any(|&v| v < 0.0 || v.fract() != 0.0) {
                return Err(parameter());
            }
            let maximum = y.iter().copied().fold(0.0, f64::max);
            if maximum >= n as f64 {
                return Err(parameter());
            }
            categories = maximum as usize + 1;
            if categories < 2 || (0..categories).any(|c| !y.contains(&(c as f64))) {
                return Err(parameter());
            }
        }
        LikelihoodMethod::ConditionalLogit => {
            if y.iter().any(|&v| v != 0.0 && v != 1.0) {
                return Err(parameter());
            }
            let g = groups.ok_or_else(parameter)?;
            if g.len() != n {
                return Err(parameter());
            }
            if g.iter().any(|&group| group >= n) {
                return Err(parameter());
            }
            let count = g.iter().copied().max().ok_or_else(parameter)? + 1;
            group_rows = vec![vec![]; count];
            for (i, &j) in g.iter().enumerate() {
                group_rows[j].push(i);
            }
            let mut rows = vec![];
            for group in &group_rows {
                let m = group.iter().filter(|&&i| y[i] == 1.0).count();
                if m > 0 && m < group.len() {
                    informative += 1;
                    rows.extend(group);
                } else {
                    dropped += group.len();
                }
            }
            if informative == 0 || rows.len() <= p {
                return Err(parameter());
            }
            let centered = Mat::from_fn(rows.len(), p, |i, j| {
                let group = &group_rows[g[rows[i]]];
                x[(rows[i], j)]
                    - group
                        .iter()
                        .map(|&r| x[(r, j)] / group.len() as f64)
                        .sum::<f64>()
            });
            if yss_sci_linalg::matrix_rank(centered.as_ref())
                .map_err(|_| failed())?
                .0
                != p
            {
                return Err(parameter());
            }
        }
    }
    let zi = matches!(
        options.method,
        LikelihoodMethod::ZeroInflatedPoisson | LikelihoodMethod::ZeroInflatedNegativeBinomial
    );
    let inflate = if zi {
        Some(Design::new(inflation, n, true, true, true, control)?)
    } else {
        None
    };
    let z = inflate.as_ref().map(|d| &d.x);
    let zp = z.map_or(0, |z| z.ncols());
    let extra = usize::from(matches!(
        options.method,
        LikelihoodMethod::NegativeBinomial
            | LikelihoodMethod::ZeroInflatedNegativeBinomial
            | LikelihoodMethod::Tobit
            | LikelihoodMethod::Beta
    ));
    let size = if options.method == LikelihoodMethod::MultinomialLogit {
        (categories - 1).checked_mul(p).ok_or_else(parameter)?
    } else if options.method == LikelihoodMethod::OrdinalLogit {
        p.checked_add(categories - 1).ok_or_else(parameter)?
    } else {
        p.checked_add(zp)
            .and_then(|p| p.checked_add(extra))
            .ok_or_else(parameter)?
    };
    if n <= size {
        return Err(parameter());
    }
    let mut initial = vec![0.0; size];
    let ym = mean(y);
    if constant {
        initial[0] = match options.method {
            LikelihoodMethod::Beta => (ym / (1.0 - ym)).ln(),
            LikelihoodMethod::Tobit => ym,
            LikelihoodMethod::MultinomialLogit => 0.0,
            _ => ym.ln(),
        };
    }
    if zi {
        initial[p] = -1.0;
    }
    if options.method == LikelihoodMethod::Tobit {
        let (b, _) = least_squares(x, y, None, control)?;
        initial[..p].copy_from_slice(&b);
        let residual = y
            .iter()
            .zip(fitted(x, &b))
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>();
        initial[size - 1] = (residual / n as f64).sqrt().max(0.1).ln();
    }
    if options.method == LikelihoodMethod::Beta {
        let variance = y.iter().map(|v| (v - ym).powi(2)).sum::<f64>() / n as f64;
        initial[size - 1] = (ym * (1.0 - ym) / variance - 1.0).clamp(2.0, 100.0).ln();
    }
    if options.method == LikelihoodMethod::MultinomialLogit && constant {
        let c0 = y.iter().filter(|&&v| v == 0.0).count() as f64;
        for c in 1..categories {
            initial[(c - 1) * p] = (y.iter().filter(|&&v| v == c as f64).count() as f64 / c0).ln();
        }
    }
    if options.method == LikelihoodMethod::OrdinalLogit {
        let mut prev = 0.0;
        for c in 0..categories - 1 {
            let prob = y.iter().filter(|&&v| v <= c as f64).count() as f64 / n as f64;
            let cut = (prob / (1.0 - prob)).ln();
            initial[p + c] = if c == 0 { cut } else { (cut - prev).ln() };
            prev = cut;
        }
    }
    let loglike = |b: &[f64]| -> Result<f64> {
        control.check()?;
        if categorical && options.method == LikelihoodMethod::MultinomialLogit {
            let mut ll = 0.0;
            for i in 0..n {
                if i % 256 == 0 {
                    control.check()?;
                }
                let mut eta = vec![0.0; categories];
                for c in 1..categories {
                    eta[c] = (0..p).map(|j| x[(i, j)] * b[(c - 1) * p + j]).sum();
                }
                let m = eta.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                ll += eta[y[i] as usize] - m - eta.iter().map(|e| (e - m).exp()).sum::<f64>().ln();
            }
            return finite(ll);
        }
        let eta = fitted(x, &b[..p]);
        if conditional {
            return conditional_loglike(y, &eta, &group_rows, control);
        }
        let cuts = if options.method == LikelihoodMethod::OrdinalLogit {
            ordinal_cuts(b, p, categories)
        } else {
            vec![]
        };
        let inflation_eta = z.map(|z| fitted(z, &b[p..p + zp]));
        let mut ll = 0.0;
        for i in 0..n {
            if i % 256 == 0 {
                control.check()?;
            }
            let e = eta[i];
            let v = y[i];
            let value = match options.method {
                LikelihoodMethod::NegativeBinomial => nb_log_pmf(v, e, b[size - 1])?,
                LikelihoodMethod::ZeroInflatedPoisson
                | LikelihoodMethod::ZeroInflatedNegativeBinomial => {
                    let gamma = inflation_eta.as_ref().expect("ZI design")[i];
                    let logpi = -softplus(-gamma);
                    let log1mpi = -softplus(gamma);
                    let count = if options.method == LikelihoodMethod::ZeroInflatedPoisson {
                        v * e - e.exp() - ln_gamma(v + 1.0)
                    } else {
                        nb_log_pmf(v, e, b[size - 1])?
                    };
                    if v == 0.0 {
                        logadd(logpi, log1mpi + count)
                    } else {
                        log1mpi + count
                    }
                }
                LikelihoodMethod::Tobit => {
                    let sigma = b[size - 1].exp();
                    if v == options.lower {
                        normal_log_cdf((options.lower - e) / sigma)
                    } else if options.upper == Some(v) {
                        normal_log_cdf((e - v) / sigma)
                    } else {
                        -0.5 * ((v - e) / sigma).powi(2)
                            - b[size - 1]
                            - 0.5 * (2.0 * std::f64::consts::PI).ln()
                    }
                }
                LikelihoodMethod::Beta => {
                    let mu = logistic(e);
                    let phi = b[size - 1].exp();
                    let a = mu * phi;
                    let d = (1.0 - mu) * phi;
                    if a <= 0.0 || d <= 0.0 {
                        return Err(failed());
                    }
                    ln_gamma(phi) - ln_gamma(a) - ln_gamma(d)
                        + (a - 1.0) * v.ln()
                        + (d - 1.0) * (-v).ln_1p()
                }
                LikelihoodMethod::OrdinalLogit => {
                    let c = v as usize;
                    let prob = if c == 0 {
                        logistic(cuts[0] - e)
                    } else if c == categories - 1 {
                        logistic(e - cuts[c - 1])
                    } else {
                        let high = cuts[c] - e;
                        let low = cuts[c - 1] - e;
                        logistic(high) * logistic(-low) * (-(-(high - low)).exp_m1())
                    };
                    prob.ln()
                }
                _ => unreachable!(),
            };
            ll += value;
        }
        finite(ll)
    };
    let objective = |b: &[f64]| loglike(b).map(|ll| -ll / n as f64);
    let minimum = minimize(&objective, initial, options.iteration, control)?;
    let b = &minimum.beta;
    let info = hessian(&objective, b, control)?;
    let info = Mat::from_fn(size, size, |i, j| info[(i, j)] * n as f64);
    let covariance = inverse(&info)?;
    let mut jacobian = Mat::identity(size, size);
    let mut terms = names(predictors.len(), constant);
    let mut output_beta = b.clone();
    if options.method == LikelihoodMethod::MultinomialLogit {
        terms.clear();
        let j = design.raw_jacobian();
        for c in 0..categories - 1 {
            for i in 0..p {
                for k in 0..p {
                    jacobian[(c * p + i, c * p + k)] = j[(i, k)];
                }
            }
            terms.extend(
                names(predictors.len(), constant)
                    .iter()
                    .map(|s| format!("class[{}].{s}", c + 1)),
            );
        }
        output_beta = transform(b, None, &jacobian).0;
    } else if options.method == LikelihoodMethod::OrdinalLogit {
        for i in 0..p {
            jacobian[(i, i)] = 1.0 / design.scales[i];
            output_beta[i] = b[i] / design.scales[i];
        }
        let cuts = ordinal_cuts(b, p, categories);
        for c in 0..categories - 1 {
            output_beta[p + c] = cuts[c];
            terms.push(format!("cut[{}|{}]", c, c + 1));
            for j in 0..categories - 1 {
                jacobian[(p + c, p + j)] = if j == 0 {
                    1.0
                } else if j <= c {
                    b[p + j].exp()
                } else {
                    0.0
                };
            }
        }
    } else {
        let j = design.raw_jacobian();
        for i in 0..p {
            for k in 0..p {
                jacobian[(i, k)] = j[(i, k)];
            }
        }
        let raw = design.raw(&b[..p], None).0;
        output_beta[..p].copy_from_slice(&raw);
        if let Some(inflate) = &inflate {
            let j = inflate.raw_jacobian();
            for i in 0..zp {
                for k in 0..zp {
                    jacobian[(p + i, p + k)] = j[(i, k)];
                }
            }
            let raw = inflate.raw(&b[p..p + zp], None).0;
            output_beta[p..p + zp].copy_from_slice(&raw);
            terms.extend(
                names(inflation.len(), true)
                    .iter()
                    .map(|s| format!("inflation.{s}")),
            );
        }
        if extra > 0 {
            output_beta[size - 1] = b[size - 1].exp();
            jacobian[(size - 1, size - 1)] = output_beta[size - 1];
            terms.push(
                match options.method {
                    LikelihoodMethod::Tobit => "sigma",
                    LikelihoodMethod::Beta => "precision",
                    _ => "alpha",
                }
                .into(),
            );
        }
    }
    let covariance = jacobian.as_ref() * covariance.as_ref() * jacobian.transpose();
    let mut probabilities = vec![];
    let mut fitted_categories = vec![];
    let predicted = if categorical {
        for i in 0..n {
            if i % 256 == 0 {
                control.check()?;
            }
            let probs = if options.method == LikelihoodMethod::MultinomialLogit {
                let mut eta = vec![0.0; categories];
                for c in 1..categories {
                    eta[c] = (0..p).map(|j| x[(i, j)] * b[(c - 1) * p + j]).sum();
                }
                let m = eta.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let sum = eta.iter().map(|e| (e - m).exp()).sum::<f64>();
                eta.into_iter()
                    .map(|e| (e - m).exp() / sum)
                    .collect::<Vec<_>>()
            } else {
                let eta = (0..p).map(|j| x[(i, j)] * b[j]).sum::<f64>();
                let cuts = ordinal_cuts(b, p, categories);
                let cumulative = std::iter::once(0.0)
                    .chain(cuts.iter().map(|c| logistic(c - eta)))
                    .chain(std::iter::once(1.0))
                    .collect::<Vec<_>>();
                cumulative.windows(2).map(|w| w[1] - w[0]).collect()
            };
            let best = probs
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .expect("categories")
                .0;
            fitted_categories.push(best);
            probabilities.push(probs);
        }
        vec![0.0; n]
    } else {
        let eta = fitted(x, &b[..p]);
        eta.iter()
            .enumerate()
            .map(|(i, &e)| match options.method {
                LikelihoodMethod::Beta => logistic(e),
                LikelihoodMethod::Tobit => e,
                LikelihoodMethod::ConditionalLogit => 0.0,
                _ => {
                    e.exp()
                        * if zi {
                            logistic(-inflation_eta_for_row(z, b, p, zp, i))
                        } else {
                            1.0
                        }
                }
            })
            .collect()
    };
    let details = if conditional {
        RegressionDetails::Conditional {
            total_groups: group_rows.iter().filter(|rows| !rows.is_empty()).count(),
            informative_groups: informative,
            dropped_observations: dropped,
            covariance_method: "observed conditional information".into(),
        }
    } else {
        RegressionDetails::Likelihood {
            method: options.method,
            covariance_method: "observed likelihood information".into(),
        }
    };
    let mut r = result(
        match options.method {
            LikelihoodMethod::NegativeBinomial => "negative_binomial",
            LikelihoodMethod::ZeroInflatedPoisson => "zero_inflated_poisson",
            LikelihoodMethod::ZeroInflatedNegativeBinomial => "zero_inflated_negative_binomial",
            LikelihoodMethod::Tobit => "tobit",
            LikelihoodMethod::Beta => "beta",
            LikelihoodMethod::MultinomialLogit => "multinomial_logit",
            LikelihoodMethod::OrdinalLogit => "ordinal_logit",
            LikelihoodMethod::ConditionalLogit => "conditional_logit",
        },
        y,
        predicted,
        output_beta,
        terms,
        Some(covariance),
        constant,
        None,
        details,
    )?;
    r.iterations = minimum.iterations;
    r.categories = (0..categories).collect();
    r.fitted_categories = fitted_categories;
    r.probabilities = probabilities;
    if extra > 0 {
        // A zero dispersion/scale lies on the parameter-space boundary, not a regular Wald null.
        let coefficient = r
            .coefficients
            .last_mut()
            .expect("positive nuisance parameter");
        coefficient.statistic = None;
        coefficient.p_value = None;
        coefficient.confidence_interval = coefficient
            .standard_error
            .map(|se| -> Result<[f64; 2]> {
                let log_se = se / coefficient.estimate;
                let z = 1.959963984540054;
                let lower = (b[size - 1] - z * log_se).exp();
                let upper = (b[size - 1] + z * log_se).exp();
                Ok([finite(lower)?, finite(upper)?])
            })
            .transpose()?;
    }
    r.statistics.r_squared = None;
    r.statistics.adjusted_r_squared = None;
    if categorical || conditional {
        r.fitted.clear();
        r.residuals.clear();
        r.statistics.rss = None;
        r.statistics.rmse = None;
    }
    if conditional {
        r.observations -= dropped;
    }
    likelihood_statistics(&mut r, -minimum.value * n as f64, size);
    control.check()?;
    Ok(r)
}
fn inflation_eta_for_row(z: Option<&Mat<f64>>, b: &[f64], p: usize, zp: usize, i: usize) -> f64 {
    let z = z.expect("inflation");
    (0..zp).map(|j| z[(i, j)] * b[p + j]).sum()
}
