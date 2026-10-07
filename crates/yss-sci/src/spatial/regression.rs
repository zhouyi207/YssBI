use super::*;
use crate::regression::models::common::{
    Design, check_iteration, coefficient_table, fitted, hessian, inverse, least_squares, minimize,
    names, validate,
};
use yss_sci_contract::spatial::*;
use yss_sci_linalg::{Col, ComplexValue, Eigen, Mat, MatrixExt, Solve};

struct Likelihood<'a> {
    y: &'a [f64],
    wy: Vec<f64>,
    wwy: Vec<f64>,
    x: &'a Mat<f64>,
    wx: Mat<f64>,
    eigenvalues: Vec<ComplexValue>,
    method: SpatialMethod,
    bound: f64,
    blocks: usize,
    control: &'a Control,
}
impl Likelihood<'_> {
    fn parameters(&self, eta: &[f64]) -> (f64, f64) {
        let rho = if self.method.lag_y() {
            self.bound * eta[0].tanh()
        } else {
            0.0
        };
        let lambda = if self.method.lag_error() {
            self.bound * eta[usize::from(self.method.lag_y())].tanh()
        } else {
            0.0
        };
        (rho, lambda)
    }
    fn logdet(&self, rho: f64, lambda: f64) -> Result<f64> {
        let mut sum = 0.0;
        for v in &self.eigenvalues {
            sum += (1.0 - rho * v.re).hypot(rho * v.im).ln()
                + (1.0 - lambda * v.re).hypot(lambda * v.im).ln();
        }
        finite(self.blocks as f64 * sum)
    }
    fn transformed(&self, rho: f64, lambda: f64) -> (Vec<f64>, Mat<f64>) {
        let y = (0..self.y.len())
            .map(|i| self.y[i] - (rho + lambda) * self.wy[i] + rho * lambda * self.wwy[i])
            .collect();
        let x = Mat::from_fn(self.x.nrows(), self.x.ncols(), |i, j| {
            self.x[(i, j)] - lambda * self.wx[(i, j)]
        });
        (y, x)
    }
    fn profile(&self, eta: &[f64]) -> Result<(f64, Vec<f64>, f64)> {
        self.control.check()?;
        let (rho, lambda) = self.parameters(eta);
        let (y, x) = self.transformed(rho, lambda);
        let (b, _) = least_squares(&x, &y, None, self.control)?;
        let prediction = fitted(&x, &b);
        let variance = y
            .iter()
            .zip(prediction)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            / y.len() as f64;
        if !variance.is_finite() || variance <= 0.0 {
            return Err(failed());
        }
        let value =
            0.5 * y.len() as f64 * ((2.0 * std::f64::consts::PI).ln() + 1.0 + variance.ln())
                - self.logdet(rho, lambda)?;
        Ok((finite(value)?, b, variance))
    }
    fn full(&self, theta: &[f64]) -> Result<f64> {
        self.control.check()?;
        let p = self.x.ncols();
        let (rho, lambda) = self.parameters(&theta[p..]);
        let (y, x) = self.transformed(rho, lambda);
        let prediction = fitted(&x, &theta[..p]);
        let log_variance = theta[theta.len() - 1];
        let ss = y
            .iter()
            .zip(prediction)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>();
        finite(
            0.5 * (y.len() as f64 * ((2.0 * std::f64::consts::PI).ln() + log_variance)
                + ss * (-log_variance).exp())
                - self.logdet(rho, lambda)?,
        )
    }
}

fn spatial_inverse(w: &[Vec<f64>], rho: f64, control: &Control) -> Result<Mat<f64>> {
    control.check()?;
    let n = w.len();
    let a = Mat::from_fn(n, n, |i, j| f64::from(u8::from(i == j)) - rho * w[i][j]);
    let inv = a
        .checked_lu()
        .map_err(|_| failed())?
        .solve(&Mat::identity(n, n));
    control.check()?;
    Ok(inv)
}
fn reduced(inverse: &Mat<f64>, xb: &[f64], control: &Control) -> Result<Vec<f64>> {
    let mut result = Vec::with_capacity(xb.len());
    for block in xb.chunks_exact(inverse.nrows()) {
        control.check()?;
        let b = inverse.as_ref() * Col::from_iter(block.iter().copied()).as_ref();
        result.extend(b.iter().map(|v| finite(*v)).collect::<Result<Vec<_>>>()?);
    }
    Ok(result)
}
fn impacts(
    w: &[Vec<f64>],
    inv: &Mat<f64>,
    beta: &[f64],
    method: SpatialMethod,
    constant: bool,
    count: usize,
    control: &Control,
) -> Result<Vec<SpatialImpact>> {
    let mut impacts = Vec::with_capacity(count);
    let offset = usize::from(constant);
    for k in 0..count {
        control.check()?;
        let b = beta[offset + k];
        let theta = if method.lag_x() {
            beta[offset + count + k]
        } else {
            0.0
        };
        let derivative = Mat::from_fn(
            w.len(),
            w.len(),
            |i, j| if i == j { b } else { theta * w[i][j] },
        );
        let effect = inv.as_ref() * derivative.as_ref();
        let direct = finite((0..w.len()).map(|i| effect[(i, i)] / w.len() as f64).sum())?;
        let total = finite(
            (0..w.len())
                .map(|i| {
                    (0..w.len())
                        .map(|j| effect[(i, j)] / w.len() as f64)
                        .sum::<f64>()
                })
                .sum(),
        )?;
        impacts.push(SpatialImpact {
            term: format!("x{}", k + 1),
            direct,
            indirect: finite(total - direct)?,
            total,
        });
    }
    Ok(impacts)
}

fn fit_blocks(
    method: SpatialMethod,
    y: &[f64],
    predictors: &[Vec<f64>],
    w: &[Vec<f64>],
    options: SpatialOptions,
    control: &Control,
) -> Result<SpatialRegressionResult> {
    validate(y, predictors, control)?;
    let maximum = weights::validate(w, control)?;
    if !y.len().is_multiple_of(w.len()) {
        return Err(invalid(Violation::ShapeMismatch));
    }
    if predictors.is_empty() {
        return Err(parameter());
    }
    check_iteration(options.iteration)?;
    let bound = finite(1.0 / maximum)?;
    let blocks = y.len() / w.len();
    let mut x = predictors.to_vec();
    if method.lag_x() {
        for column in predictors {
            x.push(lag(w, column, control)?);
        }
    }
    let design = Design::new(&x, y.len(), options.constant, true, true, control)?;
    let p = design.x.ncols();
    let s = usize::from(method.lag_y()) + usize::from(method.lag_error());
    if y.len() <= p + s {
        return Err(parameter());
    }
    let wy = lag(w, y, control)?;
    let mut wx = Mat::zeros(y.len(), p);
    for j in 0..p {
        let v = lag(
            w,
            &(0..y.len()).map(|i| design.x[(i, j)]).collect::<Vec<_>>(),
            control,
        )?;
        for (i, v) in v.into_iter().enumerate() {
            wx[(i, j)] = v;
        }
    }
    let eigenvalues = if s > 0 {
        let matrix = Mat::from_fn(w.len(), w.len(), |i, j| w[i][j]);
        let eigen = Eigen::factor(matrix.as_ref()).map_err(|_| failed())?;
        control.check()?;
        eigen.values().iter().copied().collect()
    } else {
        vec![]
    };
    let likelihood = Likelihood {
        y,
        wwy: lag(w, &wy, control)?,
        wy,
        x: &design.x,
        wx,
        eigenvalues,
        method,
        bound,
        blocks,
        control,
    };
    let mut terms = names(predictors.len(), options.constant);
    if method.lag_x() {
        terms.extend((1..=predictors.len()).map(|i| format!("W:x{i}")));
    }
    let (beta, covariance, rho, lambda, sigma_squared, nll, iterations) = if s == 0 {
        let (b, inv) = least_squares(&design.x, y, None, control)?;
        let ss = y
            .iter()
            .zip(fitted(&design.x, &b))
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>();
        if !ss.is_finite() || ss <= 0.0 {
            return Err(failed());
        }
        let variance = ss / (y.len() - p) as f64;
        let cov = Mat::from_fn(p, p, |i, j| inv[(i, j)] * variance);
        let (beta, covariance) = design.raw(&b, Some(cov));
        let nll = 0.5
            * y.len() as f64
            * ((2.0 * std::f64::consts::PI).ln() + 1.0 + (ss / y.len() as f64).ln());
        (
            beta,
            covariance.ok_or_else(failed)?,
            0.0,
            0.0,
            variance,
            nll,
            1,
        )
    } else {
        // Profile out beta and variance. Multiple starts matter for SAC's likelihood ridge.
        let objective =
            |eta: &[f64]| -> Result<f64> { Ok(likelihood.profile(eta)?.0 / y.len() as f64) };
        let mut best = None;
        for a in [-0.55_f64, 0.0, 0.55] {
            for b in if s == 2 {
                &[-0.55_f64, 0.0, 0.55][..]
            } else {
                &[0.0][..]
            } {
                control.check()?;
                let initial = if s == 2 {
                    vec![a.atanh(), b.atanh()]
                } else {
                    vec![a.atanh()]
                };
                match minimize(&objective, initial, options.iteration, control) {
                    Ok(candidate)
                        if best.as_ref().is_none_or(
                            |old: &crate::regression::models::common::Minimum| {
                                candidate.value < old.value
                            },
                        ) =>
                    {
                        best = Some(candidate)
                    }
                    Err(yss_sci_contract::execution::ScientificComputationError::Cancelled) => {
                        control.check()?;
                        return Err(failed());
                    }
                    Err(
                        yss_sci_contract::execution::ScientificComputationError::DeadlineExceeded,
                    ) => {
                        control.check()?;
                        return Err(failed());
                    }
                    _ => {}
                }
            }
        }
        let best = best.ok_or_else(failed)?;
        if best.beta.iter().any(|v| v.tanh().abs() >= 0.9999) {
            return Err(failed());
        }
        let (nll, b, variance) = likelihood.profile(&best.beta)?;
        let (rho, lambda) = likelihood.parameters(&best.beta);
        let mut theta = b.clone();
        theta.extend_from_slice(&best.beta);
        theta.push(variance.ln());
        let h = hessian(&|theta| likelihood.full(theta), &theta, control)?;
        let inv = inverse(&h)?;
        let raw_j = design.raw_jacobian();
        let jacobian = Mat::from_fn(p + s, p + s + 1, |i, j| {
            if i < p && j < p {
                raw_j[(i, j)]
            } else if i >= p && i == j {
                bound * (1.0 - best.beta[i - p].tanh().powi(2))
            } else {
                0.0
            }
        });
        let cov = jacobian.as_ref() * inv.as_ref() * jacobian.transpose();
        let (mut beta, _) = design.raw(&b, None);
        if method.lag_y() {
            beta.push(rho);
            terms.push("rho".into());
        }
        if method.lag_error() {
            beta.push(lambda);
            terms.push("lambda".into());
        }
        (beta, cov, rho, lambda, variance, nll, best.iterations)
    };
    let coefficients = coefficient_table(
        &beta,
        terms,
        Some(&covariance),
        if s == 0 { Some(y.len() - p) } else { None },
    )?;
    let xb = (0..y.len())
        .map(|i| {
            let offset = usize::from(options.constant);
            let value = if options.constant { beta[0] } else { 0.0 };
            finite(
                value
                    + x.iter()
                        .enumerate()
                        .map(|(j, x)| x[i] * beta[offset + j])
                        .sum::<f64>(),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let fitted = xb
        .iter()
        .zip(&likelihood.wy)
        .map(|(a, b)| finite(a + rho * b))
        .collect::<Result<Vec<_>>>()?;
    let residuals = y
        .iter()
        .zip(&fitted)
        .map(|(a, b)| finite(a - b))
        .collect::<Result<Vec<_>>>()?;
    let wu = lag(w, &residuals, control)?;
    let innovations = residuals
        .iter()
        .zip(wu)
        .map(|(a, b)| finite(a - lambda * b))
        .collect::<Result<Vec<_>>>()?;
    let inv = spatial_inverse(w, rho, control)?;
    let reduced_fitted = reduced(&inv, &xb, control)?;
    let impacts = impacts(
        w,
        &inv,
        &beta,
        method,
        options.constant,
        predictors.len(),
        control,
    )?;
    let innovation_moran_i = innovations
        .chunks_exact(w.len())
        .map(|e| moran::descriptive(e, w, control))
        .collect::<Result<Vec<_>>>()?;
    let parameters = (p + s + 1) as f64;
    Ok(SpatialRegressionResult {
        method,
        observations: y.len(),
        estimation_observations: y.len(),
        constant: options.constant,
        periods: blocks,
        coefficients,
        covariance: (0..p + s)
            .map(|i| (0..p + s).map(|j| finite(covariance[(i, j)])).collect())
            .collect::<Result<_>>()?,
        rho: method.lag_y().then_some(rho),
        lambda: method.lag_error().then_some(lambda),
        spatial_parameter_bound: bound,
        sigma_squared: finite(sigma_squared)?,
        log_likelihood: finite(-nll)?,
        aic: finite(2.0 * nll + 2.0 * parameters)?,
        bic: finite(2.0 * nll + (y.len() as f64).ln() * parameters)?,
        df_residual: y.len() - p - s,
        iterations,
        fitted,
        residuals,
        innovations,
        reduced_fitted,
        impacts,
        innovation_moran_i,
        unit_effects: vec![],
    })
}

pub fn fit(
    method: SpatialMethod,
    y: &[f64],
    predictors: &[Vec<f64>],
    w: &[Vec<f64>],
    options: SpatialOptions,
    control: &Control,
) -> Result<SpatialRegressionResult> {
    if y.len() != w.len() {
        return Err(invalid(Violation::ShapeMismatch));
    }
    fit_blocks(method, y, predictors, w, options, control)
}

/// Balanced entity FE panel in period-major weights order. Orthonormal Helmert
/// time contrasts eliminate entity effects; the likelihood has N*(T-1) observations.
pub fn panel(
    method: SpatialMethod,
    y: &[f64],
    predictors: &[Vec<f64>],
    w: &[Vec<f64>],
    iteration: yss_sci_contract::regression::models::IterationOptions,
    control: &Control,
) -> Result<SpatialRegressionResult> {
    validate(y, predictors, control)?;
    weights::validate(w, control)?;
    let n = w.len();
    if !y.len().is_multiple_of(n) {
        return Err(invalid(Violation::ShapeMismatch));
    }
    let periods = y.len() / n;
    if periods < 2 || !matches!(method, SpatialMethod::Slm | SpatialMethod::Sem) {
        return Err(parameter());
    }
    let contrast = |v: &[f64]| -> Result<Vec<f64>> {
        let mut sum = vec![0.0; n];
        let mut out = Vec::with_capacity(n * (periods - 1));
        for k in 0..periods - 1 {
            control.check()?;
            let scale = (((k + 1) as f64) * ((k + 2) as f64)).sqrt();
            for i in 0..n {
                sum[i] += v[k * n + i];
                out.push(finite(
                    (sum[i] - (k + 1) as f64 * v[(k + 1) * n + i]) / scale,
                )?);
            }
        }
        Ok(out)
    };
    let cy = contrast(y)?;
    let cx = predictors
        .iter()
        .map(|x| contrast(x))
        .collect::<Result<Vec<_>>>()?;
    let mut model = fit_blocks(
        method,
        &cy,
        &cx,
        w,
        SpatialOptions {
            constant: false,
            iteration,
        },
        control,
    )?;
    let rho = model.rho.unwrap_or(0.0);
    let lambda = model.lambda.unwrap_or(0.0);
    let wy = lag(w, y, control)?;
    let mut xb = (0..y.len())
        .map(|i| {
            predictors
                .iter()
                .zip(&model.coefficients)
                .map(|(x, b)| x[i] * b.estimate)
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    let mut effects = vec![0.0; n];
    for i in 0..y.len() {
        effects[i % n] += (y[i] - rho * wy[i] - xb[i]) / periods as f64;
    }
    for i in 0..y.len() {
        xb[i] = finite(xb[i] + effects[i % n])?;
    }
    model.fitted = xb
        .iter()
        .zip(wy)
        .map(|(a, b)| finite(a + rho * b))
        .collect::<Result<_>>()?;
    model.residuals = y
        .iter()
        .zip(&model.fitted)
        .map(|(a, b)| finite(a - b))
        .collect::<Result<_>>()?;
    model.innovations = model
        .residuals
        .iter()
        .zip(lag(w, &model.residuals, control)?)
        .map(|(a, b)| finite(a - lambda * b))
        .collect::<Result<_>>()?;
    model.reduced_fitted = reduced(&spatial_inverse(w, rho, control)?, &xb, control)?;
    model.innovation_moran_i = model
        .innovations
        .chunks_exact(n)
        .map(|e| moran::descriptive(e, w, control))
        .collect::<Result<_>>()?;
    model.observations = y.len();
    model.periods = periods;
    model.unit_effects = effects.into_iter().map(finite).collect::<Result<_>>()?;
    Ok(model)
}
