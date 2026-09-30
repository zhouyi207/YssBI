use super::common::*;
use statrs::distribution::{ChiSquared, ContinuousCDF};
use yss_sci_contract::{execution::*, multivariate::*};
use yss_sci_linalg::{Mat, MatrixExt, Svd};

pub fn canonical_correlation(
    x_columns: &[Vec<f64>],
    y_columns: &[Vec<f64>],
    components: usize,
    control: &ScientificExecutionControl,
) -> Result<CanonicalOutput> {
    control.check()?;
    if x_columns
        .len()
        .checked_add(y_columns.len())
        .is_none_or(|n| n > MAX_CANONICAL_VARIABLES)
    {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let x = prepare(x_columns, true, control)?;
    let y = prepare(y_columns, true, control)?;
    let n = x.matrix.nrows();
    let p = x_columns.len();
    let q = y_columns.len();
    if y.matrix.nrows() != n {
        return Err(invalid(ScientificInputViolation::ShapeMismatch));
    }
    if n <= p + q {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    check_components(components, p.min(q))?;
    full_rank(&x.matrix, control)?;
    full_rank(&y.matrix, control)?;
    let rx = covariance(&x.matrix);
    let ry = covariance(&y.matrix);
    let cross = x.matrix.transpose() * &y.matrix;
    let cross = Mat::from_fn(p, q, |i, j| cross[(i, j)] / (n - 1) as f64);
    let lx = rx
        .checked_cholesky()
        .map_err(|_| invalid(ScientificInputViolation::ParameterOutOfRange))?
        .lower();
    let ly = ry
        .checked_cholesky()
        .map_err(|_| invalid(ScientificInputViolation::ParameterOutOfRange))?
        .lower();
    let mut ix = Mat::identity(p, p);
    let mut iy = Mat::identity(q, q);
    lx.as_ref().solve_lower_triangular_in_place(ix.as_mut());
    ly.as_ref().solve_lower_triangular_in_place(iy.as_mut());
    let whitened = &ix * &cross * iy.transpose();
    control.check()?;
    let svd = Svd::factor(whitened.as_ref()).map_err(|_| failed())?;
    control.check()?;
    let correlations = svd
        .values()
        .iter()
        .take(p.min(q))
        .map(|&v| {
            if !v.is_finite() || !(0.0..=1.0 + 1e-10).contains(&v) {
                Err(failed())
            } else {
                Ok(if v >= 1.0 - 64.0 * p.max(q) as f64 * f64::EPSILON {
                    1.0
                } else {
                    v.clamp(0.0, 1.0)
                })
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let mut wx = ix.transpose() * svd.left_vectors().subcols(0, components);
    let mut wy = iy.transpose() * svd.right_vectors().subcols(0, components);
    for j in 0..components {
        let pivot = (0..p)
            .max_by(|&a, &b| wx[(a, j)].abs().total_cmp(&wx[(b, j)].abs()))
            .unwrap();
        if wx[(pivot, j)] < 0.0 {
            for i in 0..p {
                wx[(i, j)] *= -1.0;
            }
            for i in 0..q {
                wy[(i, j)] *= -1.0;
            }
        }
    }
    let x_scores = &x.matrix * &wx;
    let y_scores = &y.matrix * &wy;
    let x_loadings = &rx * &wx;
    let y_loadings = &ry * &wy;
    let correction = (n - 1) as f64 - (p + q + 1) as f64 / 2.0;
    let mut tests = Vec::new();
    for first in 0..correlations.len() {
        let log_lambda = correlations[first..]
            .iter()
            .map(|r| (-r * r).ln_1p())
            .sum::<f64>();
        let df = (p - first) * (q - first);
        let chi_square = if log_lambda.is_finite() {
            Some(finite(-correction * log_lambda)?)
        } else {
            None
        };
        let p_value = chi_square
            .map(|statistic| {
                ChiSquared::new(df as f64)
                    .map_err(|_| failed())
                    .map(|distribution| distribution.sf(statistic))
            })
            .transpose()?;
        tests.push(CanonicalTest {
            first_axis: first + 1,
            wilks_lambda: log_lambda.exp(),
            chi_square,
            df,
            p_value,
        });
    }
    let report = CanonicalReport {
        method: "canonical_correlation".into(),
        observations: n,
        x_variables: p,
        y_variables: q,
        components,
        correlations,
        x_means: x.means,
        y_means: y.means,
        x_scales: x.scales,
        y_scales: y.scales,
        x_weights: rows(&wx, 1.0, control)?,
        y_weights: rows(&wy, 1.0, control)?,
        x_loadings: rows(&x_loadings, 1.0, control)?,
        y_loadings: rows(&y_loadings, 1.0, control)?,
        tests,
    };
    Ok(CanonicalOutput {
        report,
        x_scores: rows(&x_scores, 1.0, control)?,
        y_scores: rows(&y_scores, 1.0, control)?,
    })
}
