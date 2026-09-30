use super::{
    Result,
    design::{Design, fit, response_matrix, term_indices},
    f_probability, factor_levels, finite, invalid, nonnegative,
};
use yss_sci_contract::{anova::*, execution::*};
use yss_sci_linalg::{Mat, MatrixExt, SymmetricEigen, matrix_rank};

/// Joint response tests share the categorical design and sums-of-squares policy.
pub fn manova(
    responses: &[Vec<f64>],
    factors: &[Factor],
    options: AnovaOptions,
    control: &ScientificExecutionControl,
) -> Result<ManovaResult> {
    control.check()?;
    if !(2..=MAX_MANOVA_RESPONSES).contains(&responses.len()) {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let n = responses[0].len();
    let design = Design::new(n, factors, &[], options.model, control)?;
    let (y, scales) = response_matrix(
        &responses.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        control,
    )?;
    let error = fit(
        &design,
        &y,
        &(0..design.columns.len()).collect::<Vec<_>>(),
        control,
    )?;
    let df_error = n - design.columns.len();
    if matrix_rank(error.as_ref())
        .map_err(|_| ScientificComputationError::ComputationFailed)?
        .0
        != responses.len()
    {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let lower = error
        .checked_cholesky()
        .map_err(|_| ScientificComputationError::ComputationFailed)?
        .lower();
    let mut inverse_lower = Mat::identity(responses.len(), responses.len());
    lower
        .as_ref()
        .solve_lower_triangular_in_place(inverse_lower.as_mut());
    let mut table = Vec::with_capacity(design.terms.len());
    for (index, term) in design.terms.iter().enumerate() {
        control.check()?;
        let (reduced, augmented) = term_indices(&design, index, options.sums_of_squares);
        let reduced_sscp = fit(&design, &y, &reduced, control)?;
        let augmented_sscp = if augmented.len() == design.columns.len() {
            error.clone()
        } else {
            fit(&design, &y, &augmented, control)?
        };
        let hypothesis = Mat::from_fn(responses.len(), responses.len(), |i, j| {
            reduced_sscp[(i, j)] - augmented_sscp[(i, j)]
        });
        let whitened = &inverse_lower * &hypothesis * inverse_lower.transpose();
        control.check()?;
        let eigen = SymmetricEigen::factor(whitened.as_ref())
            .map_err(|_| ScientificComputationError::ComputationFailed)?;
        let largest = eigen.values().iter().copied().fold(0.0, f64::max);
        let roots = eigen
            .values()
            .iter()
            .map(|&v| nonnegative(v, largest.max(1.0)))
            .collect::<Result<Vec<_>>>()?;
        table.push(ManovaTerm {
            term: term.name.clone(),
            hypothesis_df: term.columns.len(),
            hypothesis_sscp: original_sscp(&hypothesis, &scales)?,
            tests: tests(&roots, term.columns.len(), df_error)?,
        });
    }
    control.check()?;
    Ok(ManovaResult {
        method: "manova".into(),
        observations: n,
        responses: responses.len(),
        factors: factor_levels(factors),
        options,
        error_df: df_error,
        error_sscp: original_sscp(&error, &scales)?,
        table,
    })
}

fn original_sscp(matrix: &Mat<f64>, scales: &[f64]) -> Result<Vec<Vec<f64>>> {
    (0..matrix.nrows())
        .map(|i| {
            (0..matrix.ncols())
                .map(|j| finite((matrix[(i, j)] * scales[i]) * scales[j]))
                .collect()
        })
        .collect()
}

// F approximations follow the standard MANOVA definitions (SAS/STAT and statsmodels).
fn tests(roots: &[f64], hypothesis_df: usize, error_df: usize) -> Result<Vec<MultivariateTest>> {
    let p = roots.len() as f64;
    let q = hypothesis_df as f64;
    let v = error_df as f64;
    let s = p.min(q);
    let m = ((p - q).abs() - 1.0) / 2.0;
    let n = (v - p - 1.0) / 2.0;
    let log_lambda = -roots.iter().map(|root| root.ln_1p()).sum::<f64>();
    let lambda = log_lambda.exp();
    let pillai = roots.iter().map(|root| root / (1.0 + root)).sum::<f64>();
    let hotelling = finite(roots.iter().sum::<f64>())?;
    let roy = roots.iter().copied().fold(0.0, f64::max);
    let mut rows = Vec::with_capacity(4);
    let mut add = |name: &str, statistic: f64, f: f64, df1: f64, df2: f64| -> Result<()> {
        finite(statistic)?;
        let available = f.is_finite()
            && f >= 0.0
            && df1.is_finite()
            && df1 > 0.0
            && df2.is_finite()
            && df2 > 0.0;
        rows.push(MultivariateTest {
            test: name.into(),
            statistic,
            f_statistic: available.then_some(f),
            df_numerator: available.then_some(df1),
            df_denominator: available.then_some(df2),
            p_value: if available {
                Some(f_probability(f, df1, df2)?)
            } else {
                None
            },
        });
        Ok(())
    };
    let t = if p * p + q * q > 5.0 {
        ((p * p * q * q - 4.0) / (p * p + q * q - 5.0)).sqrt()
    } else {
        1.0
    };
    let df2 = (v - (p - q + 1.0) / 2.0) * t - (p * q - 2.0) / 2.0;
    add(
        "wilks_lambda",
        lambda,
        (-log_lambda / t).exp_m1() * df2 / (p * q),
        p * q,
        df2,
    )?;
    let df1 = s * (2.0 * m + s + 1.0);
    let df2 = s * (2.0 * n + s + 1.0);
    add(
        "pillai_trace",
        pillai,
        df2 / df1 * pillai / (s - pillai),
        df1,
        df2,
    )?;
    let (df1, df2, f) = if n > 0.0 {
        // Avoid division by n-1 at the well-defined limiting case n == 1.
        let denominator = (p + 2.0 * n) * (q + 2.0 * n) - 2.0 * (2.0 * n + 1.0) * (n - 1.0);
        let df2 = 4.0 + (p * q + 2.0) * 2.0 * (2.0 * n + 1.0) * (n - 1.0) / denominator;
        let c = (df2 - 2.0) / (2.0 * n);
        (p * q, df2, df2 / (p * q) * hotelling / c)
    } else {
        let df1 = s * (2.0 * m + s + 1.0);
        let df2 = s * (s * n + 1.0);
        (df1, df2, df2 / df1 / s * hotelling)
    };
    add("hotelling_lawley_trace", hotelling, f, df1, df2)?;
    let df1 = p.max(q);
    let df2 = v - df1 + q;
    add("roy_greatest_root", roy, df2 / df1 * roy, df1, df2)?;
    Ok(rows)
}
