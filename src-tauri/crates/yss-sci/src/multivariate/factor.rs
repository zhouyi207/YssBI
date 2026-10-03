use super::common::*;
use yss_sci_contract::{execution::*, multivariate::*};
use yss_sci_linalg::{Mat, MatrixExt, Solve, Svd};

pub fn exploratory_factor(
    columns: &[Vec<f64>],
    options: FactorOptions,
    control: &ScientificExecutionControl,
) -> Result<OrdinationOutput<FactorReport>> {
    let prepared = prepare(columns, true, control)?;
    let n = prepared.matrix.nrows();
    let p = columns.len();
    let k = options.factors;
    check_components(k, p.saturating_sub(1))?;
    // A nonnegative factor-model degrees of freedom is required for identification.
    if p < 3
        || n <= p
        || (p - k) * (p - k) < p + k
        || options.max_iterations == 0
        || !options.tolerance.is_finite()
        || !(0.0..=0.1).contains(&options.tolerance)
        || options.tolerance == 0.0
    {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let corr = covariance(&prepared.matrix);
    full_rank(&corr, control)?;
    let factor = corr
        .checked_cholesky()
        .map_err(|_| invalid(ScientificInputViolation::ParameterOutOfRange))?;
    let inverse = factor.solve(&Mat::identity(p, p));
    let mut communalities = (0..p)
        .map(|i| (1.0 - 1.0 / inverse[(i, i)]).clamp(0.0, 1.0))
        .collect::<Vec<_>>();
    let mut loadings = Mat::zeros(p, k);
    let mut iterations = 0;
    let mut converged = false;
    for iteration in 1..=options.max_iterations {
        control.check()?;
        let reduced = Mat::from_fn(p, p, |i, j| {
            if i == j {
                communalities[i]
            } else {
                corr[(i, j)]
            }
        });
        let eigen = spectrum(&reduced, false, control)?;
        if eigen.values[..k].iter().any(|&v| v < -1e-10) {
            return Err(failed());
        }
        loadings = Mat::from_fn(p, k, |i, j| {
            eigen.vectors[(i, j)] * eigen.values[j].max(0.0).sqrt()
        });
        let updated = (0..p)
            .map(|i| (0..k).map(|j| loadings[(i, j)].powi(2)).sum::<f64>())
            .collect::<Vec<_>>();
        if updated.iter().any(|&v| !v.is_finite() || v >= 1.0) {
            return Err(failed());
        }
        let change = communalities
            .iter()
            .zip(&updated)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        communalities = updated;
        iterations = iteration;
        if change < options.tolerance {
            converged = true;
            break;
        }
    }
    if !converged {
        return Err(failed());
    }
    let rotation_iterations = if options.rotation == FactorRotation::Varimax && k > 1 {
        varimax(
            &mut loadings,
            options.max_iterations,
            options.tolerance,
            control,
        )?
    } else {
        0
    };
    orient(&mut loadings);
    let communalities = (0..p)
        .map(|i| (0..k).map(|j| loadings[(i, j)].powi(2)).sum::<f64>())
        .collect::<Vec<_>>();
    let coefficients = &inverse * &loadings;
    let scores = &prepared.matrix * &coefficients;
    let variance_proportions = (0..k)
        .map(|j| (0..p).map(|i| loadings[(i, j)].powi(2)).sum::<f64>() / p as f64)
        .collect();
    let lower = factor.lower();
    let log_determinant = (0..p).map(|i| 2.0 * lower[(i, i)].ln()).sum::<f64>();
    let adequacy = super::adequacy::assess(&corr, &inverse, n, log_determinant, control)?;
    let report = FactorReport {
        method: "principal_axis_factor".into(),
        observations: n,
        variables: p,
        factors: k,
        rotation: options.rotation,
        iterations,
        rotation_iterations,
        means: prepared.means,
        scales: prepared.scales,
        loadings: rows(&loadings, 1.0, control)?,
        uniquenesses: communalities.iter().map(|v| 1.0 - v).collect(),
        communalities,
        variance_proportions,
        kmo: adequacy.kmo,
        bartlett_chi_square: adequacy.bartlett_chi_square,
        bartlett_df: adequacy.bartlett_df,
        bartlett_p_value: adequacy.bartlett_p_value,
        score_method: "regression".into(),
    };
    Ok(OrdinationOutput {
        report,
        coordinates: rows(&scores, 1.0, control)?,
    })
}

fn varimax(
    loadings: &mut Mat<f64>,
    max_iterations: usize,
    tolerance: f64,
    control: &ScientificExecutionControl,
) -> Result<usize> {
    let p = loadings.nrows();
    let k = loadings.ncols();
    let original = loadings.clone();
    let mut rotation = Mat::identity(k, k);
    let mut previous = 0.0;
    for iteration in 1..=max_iterations {
        control.check()?;
        let current = &original * &rotation;
        let column_squares = (0..k)
            .map(|j| (0..p).map(|i| current[(i, j)].powi(2)).sum::<f64>())
            .collect::<Vec<_>>();
        let adjusted = Mat::from_fn(p, k, |i, j| {
            current[(i, j)].powi(3) - current[(i, j)] * column_squares[j] / p as f64
        });
        let target = original.transpose() * &adjusted;
        let svd = Svd::factor(target.as_ref()).map_err(|_| failed())?;
        rotation = svd.left_vectors() * svd.right_vectors().transpose();
        let objective = svd.values().iter().sum::<f64>();
        if objective == 0.0
            || (iteration > 1
                && (objective - previous).abs()
                    <= tolerance * objective.abs().max(f64::MIN_POSITIVE))
        {
            *loadings = &original * &rotation;
            return Ok(iteration);
        }
        previous = objective;
    }
    Err(failed())
}
