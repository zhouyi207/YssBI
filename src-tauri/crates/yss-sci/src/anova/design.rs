use super::{Result, finite, invalid};
use yss_sci_contract::{anova::*, execution::*};
use yss_sci_linalg::{Mat, MatrixExt, Solve, matrix_rank};

pub(super) struct Term {
    pub name: String,
    pub components: u64,
    pub columns: Vec<usize>,
}
pub(super) struct Design {
    pub columns: Vec<Vec<f64>>,
    pub terms: Vec<Term>,
    pub covariate_means: Vec<f64>,
}

pub(super) fn masks(count: usize) -> Vec<usize> {
    let mut masks = (1..1usize << count).collect::<Vec<_>>();
    masks.sort_by_key(|mask| (mask.count_ones(), *mask));
    masks
}
pub(super) fn term_name(mask: usize, count: usize) -> String {
    (0..count)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| format!("factor{}", i + 1))
        .collect::<Vec<_>>()
        .join(":")
}
pub(super) fn validate_factors(
    n: usize,
    factors: &[Factor],
    max: usize,
    control: &ScientificExecutionControl,
) -> Result<()> {
    control.check()?;
    if n == 0 {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    if factors.is_empty() || factors.len() > max {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    for factor in factors {
        if factor.values.len() != n {
            return Err(invalid(ScientificInputViolation::ShapeMismatch));
        }
        if !(2..=MAX_ANOVA_LEVELS).contains(&factor.levels) {
            return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
        }
        let mut observed = vec![false; factor.levels];
        for (i, &value) in factor.values.iter().enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            if value >= factor.levels {
                return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
            }
            observed[value] = true;
        }
        if observed.contains(&false) {
            return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
        }
    }
    Ok(())
}

impl Design {
    pub fn new(
        n: usize,
        factors: &[Factor],
        covariates: &[Vec<f64>],
        model: FactorialModel,
        control: &ScientificExecutionControl,
    ) -> Result<Self> {
        validate_factors(n, factors, MAX_ANOVA_FACTORS, control)?;
        let width = design_columns(
            &factors.iter().map(|f| f.levels).collect::<Vec<_>>(),
            covariates.len(),
            model,
        )
        .ok_or_else(|| invalid(ScientificInputViolation::ParameterOutOfRange))?;
        if n <= width {
            return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
        }
        let mut columns = Vec::with_capacity(width);
        columns.push(vec![1.0; n]);
        let mut terms = Vec::new();
        let mut covariate_means = Vec::new();
        for (index, covariate) in covariates.iter().enumerate() {
            control.check()?;
            let (matrix, scale) = response_matrix(&[covariate.as_slice()], control)?;
            if matrix.nrows() != n {
                return Err(invalid(ScientificInputViolation::ShapeMismatch));
            }
            let mean = covariate
                .iter()
                .map(|x| x / scale[0] / n as f64)
                .sum::<f64>()
                * scale[0];
            covariate_means.push(finite(mean)?);
            let spread = (0..n).map(|i| matrix[(i, 0)].abs()).fold(0.0, f64::max);
            if spread == 0.0 {
                return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
            }
            terms.push(Term {
                name: format!("covariate{}", index + 1),
                components: 1 << (MAX_ANOVA_FACTORS + index),
                columns: vec![columns.len()],
            });
            columns.push((0..n).map(|i| matrix[(i, 0)] / spread).collect());
        }
        for mask in masks(factors.len()) {
            if model == FactorialModel::MainEffects && mask.count_ones() > 1 {
                continue;
            }
            control.check()?;
            let involved = (0..factors.len())
                .filter(|i| mask & (1 << i) != 0)
                .collect::<Vec<_>>();
            let count = involved
                .iter()
                .map(|&i| factors[i].levels - 1)
                .product::<usize>();
            let mut term = Term {
                name: term_name(mask, factors.len()),
                components: mask as u64,
                columns: Vec::with_capacity(count),
            };
            for code in 0..count {
                let mut remainder = code;
                let contrasts = involved
                    .iter()
                    .map(|&i| {
                        let contrast = remainder % (factors[i].levels - 1);
                        remainder /= factors[i].levels - 1;
                        (i, contrast)
                    })
                    .collect::<Vec<_>>();
                let mut column = Vec::with_capacity(n);
                for row in 0..n {
                    if row.is_multiple_of(1024) {
                        control.check()?;
                    }
                    column.push(
                        contrasts
                            .iter()
                            .map(|&(i, contrast)| {
                                let value = factors[i].values[row];
                                if value == contrast {
                                    1.0
                                } else if value == factors[i].levels - 1 {
                                    -1.0
                                } else {
                                    0.0
                                }
                            })
                            .product(),
                    );
                }
                term.columns.push(columns.len());
                columns.push(column);
            }
            terms.push(term);
        }
        Ok(Self {
            columns,
            terms,
            covariate_means,
        })
    }
}

/// Center each response after scaling, keeping residual SSCP in normalized units.
pub(super) fn response_matrix(
    responses: &[&[f64]],
    control: &ScientificExecutionControl,
) -> Result<(Mat<f64>, Vec<f64>)> {
    control.check()?;
    let n = responses.first().map_or(0, |column| column.len());
    if n == 0 {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    let mut matrix = Mat::zeros(n, responses.len());
    let mut scales = Vec::with_capacity(responses.len());
    for (j, response) in responses.iter().enumerate() {
        if response.len() != n {
            return Err(invalid(ScientificInputViolation::ShapeMismatch));
        }
        let mut scale = 0.0f64;
        for (i, &value) in response.iter().enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            if !value.is_finite() {
                return Err(invalid(ScientificInputViolation::NonFiniteInput));
            }
            scale = scale.max(value.abs());
        }
        if scale == 0.0 {
            scale = 1.0;
        }
        let anchor = response[0] / scale;
        let mean = response
            .iter()
            .map(|x| (x / scale - anchor) / n as f64)
            .sum::<f64>();
        for (i, value) in response.iter().enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            matrix[(i, j)] = (value / scale - anchor) - mean;
        }
        scales.push(scale);
    }
    Ok((matrix, scales))
}

/// Full-rank least squares reuses the repository's checked factorization policy.
pub(super) fn fit(
    design: &Design,
    y: &Mat<f64>,
    columns: &[usize],
    control: &ScientificExecutionControl,
) -> Result<Mat<f64>> {
    control.check()?;
    let x = Mat::from_fn(y.nrows(), columns.len(), |i, j| {
        design.columns[columns[j]][i]
    });
    let (rank, _) =
        matrix_rank(x.as_ref()).map_err(|_| ScientificComputationError::ComputationFailed)?;
    if rank != columns.len() {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    control.check()?;
    let cross = x.transpose() * &x;
    let rhs = x.transpose() * y;
    let beta = cross
        .checked_cholesky()
        .map_err(|_| ScientificComputationError::ComputationFailed)?
        .solve(&rhs);
    let fitted = &x * &beta;
    let mut residual = Mat::zeros(y.nrows(), y.ncols());
    for i in 0..y.nrows() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        for j in 0..y.ncols() {
            residual[(i, j)] = finite(y[(i, j)] - fitted[(i, j)])?;
        }
    }
    control.check()?;
    Ok(residual.transpose() * &residual)
}

pub(super) fn term_indices(
    design: &Design,
    index: usize,
    kind: SumsOfSquares,
) -> (Vec<usize>, Vec<usize>) {
    let mut reduced = vec![0];
    let mut augmented = vec![0];
    let term = &design.terms[index];
    for (i, other) in design.terms.iter().enumerate() {
        let include = match kind {
            SumsOfSquares::TypeI => i <= index,
            SumsOfSquares::TypeII => {
                i == index || other.components & term.components != term.components
            }
            SumsOfSquares::TypeIII => true,
        };
        if include {
            augmented.extend(&other.columns);
            if i != index {
                reduced.extend(&other.columns);
            }
        }
    }
    (reduced, augmented)
}
