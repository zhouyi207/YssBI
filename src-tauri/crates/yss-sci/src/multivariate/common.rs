use yss_sci_contract::{execution::*, multivariate::MAX_MULTIVARIATE_VARIABLES};
use yss_sci_linalg::{Mat, SymmetricEigen, matrix_rank};

pub(super) type Result<T> = std::result::Result<T, ScientificComputationError>;
pub(super) fn invalid(violation: ScientificInputViolation) -> ScientificComputationError {
    ScientificComputationError::InvalidInput { violation }
}
pub(super) fn failed() -> ScientificComputationError {
    ScientificComputationError::ComputationFailed
}
pub(super) fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(failed())
    }
}
pub(super) fn check_components(k: usize, bound: usize) -> Result<()> {
    if k == 0 || k > bound || k > MAX_MULTIVARIATE_VARIABLES {
        Err(invalid(ScientificInputViolation::ParameterOutOfRange))
    } else {
        Ok(())
    }
}
pub(super) fn validate(
    columns: &[Vec<f64>],
    max: usize,
    control: &ScientificExecutionControl,
) -> Result<usize> {
    control.check()?;
    if columns.is_empty() || columns.len() > max {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let n = columns[0].len();
    if n == 0 {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    for column in columns {
        if column.len() != n {
            return Err(invalid(ScientificInputViolation::ShapeMismatch));
        }
        for (i, value) in column.iter().enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            if !value.is_finite() {
                return Err(invalid(ScientificInputViolation::NonFiniteInput));
            }
        }
    }
    Ok(n)
}
pub(super) struct Prepared {
    pub matrix: Mat<f64>,
    pub means: Vec<f64>,
    pub scales: Vec<f64>,
    pub global_scale: f64,
}
/// Stable centering and either sample-SD scaling or a common numerical scale.
pub(super) fn prepare(
    columns: &[Vec<f64>],
    standardize: bool,
    control: &ScientificExecutionControl,
) -> Result<Prepared> {
    let n = validate(columns, MAX_MULTIVARIATE_VARIABLES, control)?;
    if n < 2 {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let mut matrix = Mat::zeros(n, columns.len());
    let mut means = Vec::new();
    let mut scales = Vec::new();
    let global_scale = if standardize {
        1.0
    } else {
        columns
            .iter()
            .flatten()
            .map(|x| x.abs())
            .fold(0.0, f64::max)
            .max(f64::MIN_POSITIVE)
    };
    for (j, column) in columns.iter().enumerate() {
        control.check()?;
        let scale = column
            .iter()
            .map(|x| x.abs())
            .fold(0.0, f64::max)
            .max(f64::MIN_POSITIVE);
        let anchor = column[0] / scale;
        let offset = column
            .iter()
            .map(|x| (x / scale - anchor) / n as f64)
            .sum::<f64>();
        means.push(finite((anchor + offset) * scale)?);
        let variance = column
            .iter()
            .map(|x| ((x / scale - anchor) - offset).powi(2))
            .sum::<f64>()
            / (n - 1) as f64;
        let sd = variance.sqrt();
        if standardize && sd == 0.0 {
            return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
        }
        let original_sd = if standardize {
            finite(sd * scale)?
        } else {
            1.0
        };
        if original_sd <= 0.0 {
            return Err(failed());
        }
        scales.push(original_sd);
        for (i, value) in column.iter().enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            let centered = (value / scale - anchor) - offset;
            matrix[(i, j)] = if standardize {
                centered / sd
            } else {
                centered * (scale / global_scale)
            };
        }
    }
    Ok(Prepared {
        matrix,
        means,
        scales,
        global_scale,
    })
}
pub(super) fn covariance(matrix: &Mat<f64>) -> Mat<f64> {
    let cross = matrix.transpose() * matrix;
    Mat::from_fn(cross.nrows(), cross.ncols(), |i, j| {
        cross[(i, j)] / (matrix.nrows() - 1) as f64
    })
}
pub(super) fn full_rank(matrix: &Mat<f64>, control: &ScientificExecutionControl) -> Result<()> {
    control.check()?;
    let rank = matrix_rank(matrix.as_ref()).map_err(|_| failed())?.0;
    control.check()?;
    if rank != matrix.nrows().min(matrix.ncols()) {
        Err(invalid(ScientificInputViolation::ParameterOutOfRange))
    } else {
        Ok(())
    }
}
pub(super) struct Spectrum {
    pub values: Vec<f64>,
    pub vectors: Mat<f64>,
    pub rank: usize,
}
pub(super) fn spectrum(
    matrix: &Mat<f64>,
    positive_semidefinite: bool,
    control: &ScientificExecutionControl,
) -> Result<Spectrum> {
    control.check()?;
    for value in (0..matrix.nrows()).flat_map(|i| (0..matrix.ncols()).map(move |j| matrix[(i, j)]))
    {
        finite(value)?;
    }
    let eigen = SymmetricEigen::factor(matrix.as_ref()).map_err(|_| failed())?;
    control.check()?;
    let largest = eigen.values().iter().map(|x| x.abs()).fold(0.0, f64::max);
    let tolerance = largest * matrix.nrows() as f64 * f64::EPSILON * 64.0;
    let mut values = eigen.values().iter().rev().copied().collect::<Vec<_>>();
    if positive_semidefinite && values.iter().any(|&x| x < -tolerance) {
        return Err(failed());
    }
    for value in &mut values {
        if value.abs() <= tolerance {
            *value = 0.0;
        }
        finite(*value)?;
    }
    let mut vectors = Mat::from_fn(matrix.nrows(), matrix.ncols(), |i, j| {
        eigen.vectors()[(i, matrix.ncols() - 1 - j)]
    });
    orient(&mut vectors);
    let rank = values.iter().filter(|&&x| x > tolerance).count();
    Ok(Spectrum {
        values,
        vectors,
        rank,
    })
}
pub(super) fn orient(matrix: &mut Mat<f64>) {
    for j in 0..matrix.ncols() {
        let pivot = (0..matrix.nrows())
            .max_by(|&a, &b| matrix[(a, j)].abs().total_cmp(&matrix[(b, j)].abs()));
        if pivot.is_some_and(|i| matrix[(i, j)] < 0.0) {
            for i in 0..matrix.nrows() {
                matrix[(i, j)] = -matrix[(i, j)];
            }
        }
    }
}
pub(super) fn rows(
    matrix: &Mat<f64>,
    scale: f64,
    control: &ScientificExecutionControl,
) -> Result<Vec<Vec<f64>>> {
    let mut output = Vec::with_capacity(matrix.nrows());
    for i in 0..matrix.nrows() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        output.push(
            (0..matrix.ncols())
                .map(|j| finite(matrix[(i, j)] * scale))
                .collect::<Result<_>>()?,
        );
    }
    Ok(output)
}
pub(super) fn rescale_variance(value: f64, scale: f64) -> Result<f64> {
    finite((value * scale) * scale)
}
