//! Dense matrix/vector types and project numerical conventions.
//!
//! Opaque owned and borrowed types keep faer inside this crate. Arithmetic,
//! checked factors, numerical errors and rank conventions share that boundary.

mod backend;
mod dense;
mod error;

pub use backend::{Cholesky, Eigen, Lu, Svd, SymmetricEigen};
pub use dense::{
    Col, ColMut, ColRef, DiagRef, LowerTriangularRhs, Mat, MatMut, MatRef, Row, RowMut, RowRef,
    Scale,
};
pub use error::LinalgError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComplexValue {
    pub re: f64,
    pub im: f64,
}

/// Solve with a checked factorization and an owned or borrowed RHS.
pub trait Solve<Rhs> {
    type Output;
    fn solve(&self, rhs: &Rhs) -> Self::Output;
}

pub trait MatrixExt {
    /// Read only the lower triangle of a square positive definite matrix.
    fn checked_cholesky(&self) -> Result<Cholesky, LinalgError>;
    /// Reject an exactly zero pivot, without a rank or conditioning threshold.
    fn checked_lu(&self) -> Result<Lu, LinalgError>;
}

impl MatrixExt for Mat<f64> {
    fn checked_cholesky(&self) -> Result<Cholesky, LinalgError> {
        Cholesky::factor(self.as_ref())
    }

    fn checked_lu(&self) -> Result<Lu, LinalgError> {
        Lu::factor(self.as_ref())
    }
}

impl MatrixExt for MatRef<'_, f64> {
    fn checked_cholesky(&self) -> Result<Cholesky, LinalgError> {
        Cholesky::factor(*self)
    }

    fn checked_lu(&self) -> Result<Lu, LinalgError> {
        Lu::factor(*self)
    }
}

/// SVD rank with max(rows, cols) * EPSILON * largest singular value tolerance.
/// Empty matrices have rank zero and condition number one; zero matrices have
/// infinite condition number. A failed SVD is an explicit error.
pub fn matrix_rank(matrix: MatRef<'_, f64>) -> Result<(usize, f64), LinalgError> {
    let (rows, cols) = (matrix.nrows(), matrix.ncols());
    if rows == 0 || cols == 0 {
        return Ok((0, 1.0));
    }
    let values = matrix
        .singular_values()
        .map_err(|_| LinalgError::DecompositionFailed)?;
    let largest = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let smallest = values.iter().copied().fold(f64::INFINITY, f64::min);
    if !largest.is_finite() || largest <= 0.0 {
        return Ok((0, f64::INFINITY));
    }
    let condition = if smallest > 0.0 {
        largest / smallest
    } else {
        f64::INFINITY
    };
    let tolerance = largest * rows.max(cols) as f64 * f64::EPSILON;
    Ok((
        values.iter().filter(|&&value| value > tolerance).count(),
        condition,
    ))
}

macro_rules! impl_solve {
    ($factor:ty) => {
        impl Solve<Mat<f64>> for $factor {
            type Output = Mat<f64>;
            fn solve(&self, rhs: &Mat<f64>) -> Self::Output {
                self.solve_matrix(rhs.as_ref())
            }
        }
        impl Solve<MatRef<'_, f64>> for $factor {
            type Output = Mat<f64>;
            fn solve(&self, rhs: &MatRef<'_, f64>) -> Self::Output {
                self.solve_matrix(*rhs)
            }
        }
        impl Solve<Col<f64>> for $factor {
            type Output = Col<f64>;
            fn solve(&self, rhs: &Col<f64>) -> Self::Output {
                self.solve_vector(rhs.as_ref())
            }
        }
        impl Solve<ColRef<'_, f64>> for $factor {
            type Output = Col<f64>;
            fn solve(&self, rhs: &ColRef<'_, f64>) -> Self::Output {
                self.solve_vector(*rhs)
            }
        }
    };
}
impl_solve!(Cholesky);
impl_solve!(Lu);
