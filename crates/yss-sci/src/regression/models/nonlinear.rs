use super::common::*;
use statrs::distribution::{ContinuousCDF, StudentsT};
use yss_math_expr::{BinaryOp, MathExpr, ParseOptions, UnaryOp, parse_expression};
use yss_sci_contract::execution::ScientificExecutionControl as Control;
use yss_sci_contract::regression::models::*;
use yss_sci_linalg::{Col, Mat, MatrixExt, Solve, matrix_rank};

fn evaluate(expr: &MathExpr, x: &[Vec<f64>], row: usize, b: &[f64]) -> Result<f64> {
    let value = match expr {
        MathExpr::Number(v) => *v,
        MathExpr::Symbol(s) => {
            let (predictor, number) = if let Some(number) = s.strip_prefix('x') {
                (true, number)
            } else if let Some(number) = s.strip_prefix('b') {
                (false, number)
            } else {
                return Err(parameter());
            };
            let i = number
                .parse::<usize>()
                .map_err(|_| parameter())?
                .checked_sub(1)
                .ok_or_else(parameter)?;
            if predictor {
                *x.get(i).and_then(|x| x.get(row)).ok_or_else(parameter)?
            } else {
                *b.get(i).ok_or_else(parameter)?
            }
        }
        MathExpr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => -evaluate(operand, x, row, b)?,
        MathExpr::Binary { op, left, right } => {
            let a = evaluate(left, x, row, b)?;
            let d = evaluate(right, x, row, b)?;
            match op {
                BinaryOp::Add => a + d,
                BinaryOp::Sub => a - d,
                BinaryOp::Mul => a * d,
                BinaryOp::Div => a / d,
                BinaryOp::Pow => a.powf(d),
            }
        }
        MathExpr::Call { name, args } => {
            let values = args
                .iter()
                .map(|a| evaluate(a, x, row, b))
                .collect::<Result<Vec<_>>>()?;
            match (name.as_str(), values.as_slice()) {
                ("exp", [a]) => a.exp(),
                ("ln", [a]) => a.ln(),
                ("sqrt", [a]) => a.sqrt(),
                ("abs", [a]) => a.abs(),
                ("sin", [a]) => a.sin(),
                ("cos", [a]) => a.cos(),
                ("min", [a, b]) => a.min(*b),
                ("max", [a, b]) => a.max(*b),
                _ => return Err(parameter()),
            }
        }
    };
    finite(value)
}
fn values(
    expr: &MathExpr,
    x: &[Vec<f64>],
    n: usize,
    b: &[f64],
    control: &Control,
) -> Result<Vec<f64>> {
    (0..n)
        .map(|i| {
            if i % 256 == 0 {
                control.check()?;
            }
            evaluate(expr, x, i, b)
        })
        .collect()
}
fn jacobian(
    expr: &MathExpr,
    x: &[Vec<f64>],
    n: usize,
    b: &[f64],
    control: &Control,
) -> Result<Mat<f64>> {
    let mut j = Mat::zeros(n, b.len());
    let mut beta = b.to_vec();
    for k in 0..b.len() {
        control.check()?;
        let h = 1e-6 * (1.0 + b[k].abs());
        beta[k] = b[k] + h;
        let a = values(expr, x, n, &beta, control)?;
        beta[k] = b[k] - h;
        let d = values(expr, x, n, &beta, control)?;
        beta[k] = b[k];
        for i in 0..n {
            j[(i, k)] = (a[i] - d[i]) / (2.0 * h);
        }
    }
    Ok(j)
}
// Formula, starts and both bound vectors stay explicit in this neutral numerical API.
#[allow(clippy::too_many_arguments)]
pub fn nonlinear_formula(
    y: &[f64],
    predictors: &[Vec<f64>],
    formula: &str,
    initial: &[f64],
    lower: &[f64],
    upper: &[f64],
    iteration: IterationOptions,
    control: &Control,
) -> Result<RegressionModelResult> {
    validate(y, predictors, control)?;
    check_iteration(iteration)?;
    let p = initial.len();
    if p == 0
        || y.len() <= p
        || initial.iter().any(|v| !v.is_finite())
        || (!lower.is_empty() && lower.len() != p)
        || (!upper.is_empty() && upper.len() != p)
        || lower.iter().chain(upper).any(|v| !v.is_finite())
    {
        return Err(parameter());
    }
    let lo = (0..p)
        .map(|i| lower.get(i).copied().unwrap_or(f64::NEG_INFINITY))
        .collect::<Vec<_>>();
    let hi = (0..p)
        .map(|i| upper.get(i).copied().unwrap_or(f64::INFINITY))
        .collect::<Vec<_>>();
    if (0..p).any(|i| lo[i] >= hi[i] || initial[i] < lo[i] || initial[i] > hi[i]) {
        return Err(parameter());
    }
    let symbols = (1..=predictors.len())
        .map(|i| format!("x{i}"))
        .chain((1..=p).map(|i| format!("b{i}")))
        .collect::<Vec<_>>();
    let expression =
        parse_expression(formula, ParseOptions::plain(&symbols)).map_err(|_| parameter())?;
    let mut beta = initial.to_vec();
    let mut pred = values(&expression, predictors, y.len(), &beta, control)?;
    let mut rss = y
        .iter()
        .zip(&pred)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>();
    finite(rss)?;
    let mut damping = 1e-3;
    let mut done = None;
    for iter in 1..=iteration.max_iterations {
        control.check()?;
        let j = jacobian(&expression, predictors, y.len(), &beta, control)?;
        let a = gram(&j, None, control)?;
        let gradient = Col::from_fn(p, |k| {
            (0..y.len())
                .map(|i| j[(i, k)] * (y[i] - pred[i]))
                .sum::<f64>()
        });
        let projected = (0..p)
            .map(|k| {
                if (beta[k] <= lo[k] && gradient[k] < 0.0)
                    || (beta[k] >= hi[k] && gradient[k] > 0.0)
                {
                    0.0
                } else {
                    gradient[k].abs() / (1.0 + rss.sqrt() * a[(k, k)].sqrt())
                }
            })
            .fold(0.0, f64::max);
        if projected <= iteration.tolerance {
            done = Some(iter);
            break;
        }
        let active = (0..p)
            .map(|k| {
                (beta[k] <= lo[k] && gradient[k] < 0.0) || (beta[k] >= hi[k] && gradient[k] > 0.0)
            })
            .collect::<Vec<_>>();
        let free_gradient = Col::from_fn(p, |k| if active[k] { 0.0 } else { gradient[k] });
        let mut accepted = None;
        for _ in 0..40 {
            control.check()?;
            let augmented = Mat::from_fn(p, p, |i, k| {
                if active[i] || active[k] {
                    if i == k { 1.0 } else { 0.0 }
                } else {
                    a[(i, k)]
                        + if i == k {
                            damping * a[(i, i)].max(1e-12)
                        } else {
                            0.0
                        }
                }
            });
            let step = augmented
                .checked_cholesky()
                .map_err(|_| failed())?
                .solve(&free_gradient);
            let trial = (0..p)
                .map(|i| (beta[i] + step[i]).clamp(lo[i], hi[i]))
                .collect::<Vec<_>>();
            match values(&expression, predictors, y.len(), &trial, control) {
                Ok(f) => {
                    let r = y.iter().zip(&f).map(|(a, b)| (a - b).powi(2)).sum::<f64>();
                    if r.is_finite() && r < rss {
                        accepted = Some((trial, f, r));
                        damping = (damping / 3.0).max(1e-15);
                        break;
                    }
                }
                Err(
                    e @ (yss_sci_contract::execution::ScientificComputationError::Cancelled
                    | yss_sci_contract::execution::ScientificComputationError::DeadlineExceeded),
                ) => return Err(e),
                _ => {}
            }
            damping *= 10.0;
        }
        let Some((next, f, r)) = accepted else {
            return Err(failed());
        };
        let change = next
            .iter()
            .zip(&beta)
            .map(|(a, b)| (a - b).abs() / (1.0 + b.abs()))
            .fold(0.0, f64::max);
        beta = next;
        pred = f;
        rss = r;
        if change <= iteration.tolerance {
            done = Some(iter);
            break;
        }
    }
    let iterations = done.ok_or_else(failed)?;
    let j = jacobian(&expression, predictors, y.len(), &beta, control)?;
    if matrix_rank(j.as_ref()).map_err(|_| failed())?.0 != p {
        return Err(parameter());
    }
    let bound = (0..p).any(|i| {
        beta[i] - lo[i] <= iteration.tolerance * (1.0 + beta[i].abs())
            || hi[i] - beta[i] <= iteration.tolerance * (1.0 + beta[i].abs())
    });
    let covariance = if bound {
        None
    } else {
        let inv = inverse(&gram(&j, None, control)?)?;
        Some(Mat::from_fn(p, p, |i, k| {
            inv[(i, k)] * rss / (y.len() - p) as f64
        }))
    };
    let mut r = result(
        "nonlinear",
        y,
        pred,
        beta,
        (1..=p).map(|i| format!("b{i}")).collect(),
        covariance,
        false,
        Some(y.len() - p),
        RegressionDetails::Nonlinear {
            formula: formula.into(),
            initial_values: initial.to_vec(),
            covariance_method: if bound {
                "unavailable at active parameter bounds".into()
            } else {
                "local Jacobian, t approximation".into()
            },
        },
    )?;
    // Nonlinear models have no conventional linear-model adjusted R-squared.
    r.statistics.r_squared = None;
    r.statistics.adjusted_r_squared = None;
    r.iterations = iterations;
    gaussian_likelihood(&mut r)?;
    control.check()?;
    Ok(r)
}
pub fn nonlinear(
    y: &[f64],
    x: &[f64],
    family: NonlinearFamily,
    initial: &[f64],
    iteration: IterationOptions,
    control: &Control,
) -> Result<RegressionModelResult> {
    control.check()?;
    let (formula, p) = match family {
        NonlinearFamily::Exponential => ("b1*exp(b2*x1)", 2),
        NonlinearFamily::Logistic => ("b1/(1+exp(-b2*(x1-b3)))", 3),
        NonlinearFamily::MichaelisMenten => ("b1*x1/(b2+x1)", 2),
        NonlinearFamily::Gompertz => ("b1*exp(-exp(-b2*(x1-b3)))", 3),
    };
    if x.len() != y.len() || x.is_empty() {
        return Err(parameter());
    }
    let automatic = if initial.is_empty() {
        let amplitude = y.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let scale = (x.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - x.iter().copied().fold(f64::INFINITY, f64::min))
        .max(1.0);
        match family {
            NonlinearFamily::Exponential => vec![mean(y), 0.0],
            NonlinearFamily::MichaelisMenten => vec![amplitude, mean(x).abs().max(1.0)],
            _ => vec![amplitude, 1.0 / scale, mean(x)],
        }
    } else {
        initial.to_vec()
    };
    if automatic.len() != p {
        return Err(parameter());
    }
    nonlinear_formula(
        y,
        &[x.to_vec()],
        formula,
        &automatic,
        &[],
        &[],
        iteration,
        control,
    )
}

pub fn curve(
    y: &[f64],
    x: &[f64],
    family: CurveFamily,
    degree: usize,
    control: &Control,
) -> Result<RegressionModelResult> {
    validate(y, &[x.to_vec()], control)?;
    let mut response = y.to_vec();
    let predictors = match family {
        CurveFamily::Polynomial => {
            if !(1..=8).contains(&degree) {
                return Err(parameter());
            }
            (1..=degree)
                .map(|j| x.iter().map(|v| v.powi(j as i32)).collect())
                .collect::<Vec<_>>()
        }
        CurveFamily::Logarithmic => {
            if x.iter().any(|&v| v <= 0.0) {
                return Err(parameter());
            }
            vec![x.iter().map(|v| v.ln()).collect()]
        }
        CurveFamily::Inverse => {
            if x.contains(&0.0) {
                return Err(parameter());
            }
            vec![x.iter().map(|v| 1.0 / v).collect()]
        }
        CurveFamily::Exponential | CurveFamily::Power => {
            if y.iter().any(|&v| v <= 0.0)
                || (family == CurveFamily::Power && x.iter().any(|&v| v <= 0.0))
            {
                return Err(parameter());
            }
            response = y.iter().map(|v| v.ln()).collect();
            vec![
                x.iter()
                    .map(|&v| {
                        if family == CurveFamily::Power {
                            v.ln()
                        } else {
                            v
                        }
                    })
                    .collect(),
            ]
        }
    };
    let fit = ols(&response, &predictors, true, control)?;
    let mut beta = fit
        .coefficients
        .iter()
        .map(|c| c.estimate)
        .collect::<Vec<_>>();
    let log = matches!(family, CurveFamily::Exponential | CurveFamily::Power);
    let covariance = fit.covariance.map(|c| {
        Mat::from_fn(beta.len(), beta.len(), |i, j| {
            c[i][j]
                * if log && i == 0 { beta[0].exp() } else { 1.0 }
                * if log && j == 0 { beta[0].exp() } else { 1.0 }
        })
    });
    let predicted = if log {
        beta[0] = beta[0].exp();
        fit.fitted.iter().map(|v| v.exp()).collect()
    } else {
        fit.fitted
    };
    let mut terms = names(predictors.len(), true);
    if log {
        terms[0] = "amplitude".into();
    }
    let mut r = result(
        "curve",
        y,
        predicted,
        beta,
        terms,
        covariance,
        true,
        fit.statistics.df_residual,
        RegressionDetails::Curve {
            family,
            degree: (family == CurveFamily::Polynomial).then_some(degree),
            response_scale: if log {
                "least squares on log(response); fitted conditional median".into()
            } else {
                "least squares on response".into()
            },
        },
    )?;
    if log {
        let coefficient = &mut r.coefficients[0];
        coefficient.statistic = None;
        coefficient.p_value = None;
        let df = fit.statistics.df_residual.ok_or_else(failed)?;
        let critical = StudentsT::new(0.0, 1.0, df as f64)
            .map_err(|_| failed())?
            .inverse_cdf(0.975);
        coefficient.confidence_interval = coefficient
            .standard_error
            .map(|se| -> Result<[f64; 2]> {
                let log_estimate = fit.coefficients[0].estimate;
                let log_se = se / coefficient.estimate;
                let lower = (log_estimate - critical * log_se).exp();
                let upper = (log_estimate + critical * log_se).exp();
                Ok([finite(lower)?, finite(upper)?])
            })
            .transpose()?;
        if let Some(ll) = fit.statistics.log_likelihood {
            let adjusted = ll - y.iter().map(|v| v.ln()).sum::<f64>();
            let size = r.coefficients.len() + 1;
            likelihood_statistics(&mut r, adjusted, size);
        }
    } else {
        gaussian_likelihood(&mut r)?;
    }
    control.check()?;
    Ok(r)
}
fn deming_moments(xm: f64, ym: f64, a: f64, d: f64, c: f64, ratio: f64) -> Result<Vec<f64>> {
    if a <= 0.0 || d <= 0.0 || c.abs() <= f64::EPSILON * (a * d).sqrt() {
        return Err(parameter());
    }
    let diff = d - ratio * a;
    let root = diff.hypot(2.0 * ratio.sqrt() * c);
    let slope = if diff >= 0.0 {
        (diff + root) / (2.0 * c)
    } else {
        2.0 * ratio * c / (root - diff)
    };
    Ok(vec![finite(ym - slope * xm)?, finite(slope)?])
}
fn deming_estimate(x: &[f64], y: &[f64], ratio: f64) -> Result<Vec<f64>> {
    let xm = mean(x);
    let ym = mean(y);
    let scale = x
        .iter()
        .zip(y)
        .map(|(a, b)| (a - xm).abs().max((b - ym).abs()))
        .fold(0.0, f64::max);
    if scale <= 0.0 || !scale.is_finite() {
        return Err(parameter());
    }
    let a = x.iter().map(|v| ((v - xm) / scale).powi(2)).sum::<f64>();
    let d = y.iter().map(|v| ((v - ym) / scale).powi(2)).sum::<f64>();
    let c = x
        .iter()
        .zip(y)
        .map(|(x, y)| ((x - xm) / scale) * ((y - ym) / scale))
        .sum::<f64>();
    deming_moments(xm, ym, a, d, c, ratio)
}
pub fn deming(
    y: &[f64],
    x: &[f64],
    ratio: f64,
    control: &Control,
) -> Result<RegressionModelResult> {
    validate(y, &[x.to_vec()], control)?;
    if y.len() < 4 || !ratio.is_finite() || ratio <= 0.0 {
        return Err(parameter());
    }
    let beta = deming_estimate(x, y, ratio)?;
    let mut jack = Vec::with_capacity(y.len());
    let xm = mean(x);
    let ym = mean(y);
    let scale = x
        .iter()
        .zip(y)
        .map(|(a, b)| (a - xm).abs().max((b - ym).abs()))
        .fold(0.0, f64::max);
    let sxx = x.iter().map(|v| ((v - xm) / scale).powi(2)).sum::<f64>();
    let syy = y.iter().map(|v| ((v - ym) / scale).powi(2)).sum::<f64>();
    let sxy = x
        .iter()
        .zip(y)
        .map(|(a, b)| ((a - xm) / scale) * ((b - ym) / scale))
        .sum::<f64>();
    let adjustment = y.len() as f64 / (y.len() - 1) as f64;
    for i in 0..y.len() {
        if i % 1024 == 0 {
            control.check()?;
        }
        let dx = (x[i] - xm) / scale;
        let dy = (y[i] - ym) / scale;
        jack.push(deming_moments(
            xm - (x[i] - xm) / (y.len() - 1) as f64,
            ym - (y[i] - ym) / (y.len() - 1) as f64,
            sxx - adjustment * dx * dx,
            syy - adjustment * dy * dy,
            sxy - adjustment * dx * dy,
            ratio,
        )?);
    }
    let jm = (0..2)
        .map(|j| jack.iter().map(|b| b[j] / y.len() as f64).sum::<f64>())
        .collect::<Vec<_>>();
    let cov = Mat::from_fn(2, 2, |i, j| {
        jack.iter()
            .map(|b| (b[i] - jm[i]) * (b[j] - jm[j]))
            .sum::<f64>()
            * (y.len() - 1) as f64
            / y.len() as f64
    });
    let pred = x.iter().map(|x| beta[0] + beta[1] * x).collect();
    result(
        "deming",
        y,
        pred,
        beta,
        names(1, true),
        Some(cov),
        true,
        None,
        RegressionDetails::Deming {
            variance_ratio: ratio,
            covariance_method: "delete-one jackknife, normal approximation".into(),
        },
    )
}

pub fn restricted_cubic_spline(
    y: &[f64],
    x: &[f64],
    knots: &[f64],
    control: &Control,
) -> Result<RegressionModelResult> {
    validate(y, &[x.to_vec()], control)?;
    if !(3..=8).contains(&knots.len())
        || knots.iter().any(|v| !v.is_finite())
        || knots.windows(2).any(|w| w[0] >= w[1])
    {
        return Err(parameter());
    }
    let k = knots.len();
    let last = knots[k - 1];
    let penultimate = knots[k - 2];
    let scale = (last - knots[0]).powi(2);
    if !scale.is_finite() || scale <= 0.0 {
        return Err(parameter());
    }
    let mut basis = vec![x.to_vec()];
    for &t in &knots[..k - 2] {
        let mut column = Vec::with_capacity(x.len());
        for (i, &v) in x.iter().enumerate() {
            if i % 1024 == 0 {
                control.check()?;
            }
            column.push(finite(
                ((v - t).max(0.0).powi(3)
                    - (v - penultimate).max(0.0).powi(3) * (last - t) / (last - penultimate)
                    + (v - last).max(0.0).powi(3) * (penultimate - t) / (last - penultimate))
                    / scale,
            )?);
        }
        basis.push(column);
    }
    let mut r = ols(y, &basis, true, control)?;
    r.method = "restricted_cubic_spline".into();
    r.details = RegressionDetails::RestrictedCubicSpline {
        knots: knots.to_vec(),
        basis_names: (1..=basis.len()).map(|i| format!("basis{i}")).collect(),
        basis,
    };
    control.check()?;
    Ok(r)
}
pub fn automatic_spline_knots(x: &[f64], count: usize, control: &Control) -> Result<Vec<f64>> {
    control.check()?;
    if x.is_empty() || !(3..=8).contains(&count) || x.iter().any(|v| !v.is_finite()) {
        return Err(parameter());
    }
    let mut sorted = x.to_vec();
    sorted.sort_by(f64::total_cmp);
    control.check()?;
    let bound = if count == 3 { 0.1 } else { 0.05 };
    let knots = (0..count)
        .map(|i| {
            let probability = bound + (1.0 - 2.0 * bound) * i as f64 / (count - 1) as f64;
            let position = (x.len() - 1) as f64 * probability;
            let lo = position.floor() as usize;
            let fraction = position.fract();
            sorted[lo] * (1.0 - fraction) + sorted[(lo + 1).min(x.len() - 1)] * fraction
        })
        .collect::<Vec<_>>();
    if knots.windows(2).any(|w| w[0] >= w[1]) {
        return Err(parameter());
    }
    Ok(knots)
}
