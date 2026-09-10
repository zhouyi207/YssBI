//! Dense f64 linear algebra over ndarray data, with a private numerical backend.
//!
//! Decompositions own their factors and can solve multiple right-hand sides.
//! Multiplication and solve dimensions are programmer preconditions (checked by
//! assertions); numerical decomposition failures use [`LinalgError`].

mod backend;
mod error;

pub use backend::{Cholesky, Eigen, Lu, Svd, SymmetricEigen};
pub use error::LinalgError;

use ndarray::{Array1, Array2, ArrayBase, ArrayView2, Data, Ix2};

/// A complex eigenvalue/component, independent of the backend's scalar types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComplexValue {
    pub re: f64,
    pub im: f64,
}

/// Matrix product; unlike ndarray's `*`, this is not elementwise multiplication.
pub trait MatMul<Rhs> {
    type Output;
    fn matmul(&self, rhs: &Rhs) -> Self::Output;
}

/// Solve using an existing factorization. The RHS may be a vector or a matrix.
pub trait Solve<Rhs> {
    type Output;
    fn solve(&self, rhs: &Rhs) -> Self::Output;
}

/// Forward substitution using the lower triangle, overwriting the RHS.
pub trait SolveLowerTriangular<Rhs> {
    fn solve_lower_triangular_in_place(&self, rhs: &mut Rhs);
}

pub trait MatrixExt {
    /// Uses only the lower triangle of a square positive definite matrix.
    fn cholesky(&self) -> Result<Cholesky, LinalgError>;
    /// LU with partial pivoting. An exactly zero pivot is rejected; no rank
    /// tolerance or conditioning threshold is applied.
    fn lu(&self) -> Result<Lu, LinalgError>;
    /// Full singular vectors; singular values are in descending order.
    fn svd(&self) -> Result<Svd, LinalgError>;
    fn singular_values(&self) -> Result<Vec<f64>, LinalgError>;
}

impl<S: Data<Elem = f64>> MatrixExt for ArrayBase<S, Ix2> {
    fn cholesky(&self) -> Result<Cholesky, LinalgError> {
        Cholesky::factor(self.view())
    }
    fn lu(&self) -> Result<Lu, LinalgError> {
        Lu::factor(self.view())
    }
    fn svd(&self) -> Result<Svd, LinalgError> {
        Svd::factor(self.view())
    }
    fn singular_values(&self) -> Result<Vec<f64>, LinalgError> {
        backend::singular_values(self.view())
    }
}

/// SVD rank with `max(rows, cols) * EPSILON * largest singular value` tolerance.
/// Empty matrices have rank zero and condition number one. A zero matrix or an
/// exactly zero smallest singular value yields infinite condition number.
/// A failed SVD is an explicit error.
pub fn matrix_rank(matrix: ArrayView2<'_, f64>) -> Result<(usize, f64), LinalgError> {
    let (rows, cols) = matrix.dim();
    if rows == 0 || cols == 0 {
        return Ok((0, 1.0));
    }
    // Rank/conditioning need only singular values. Full U would allocate rows² entries
    // for a tall design matrix even though no caller uses its singular vectors here.
    let values = matrix.singular_values()?;
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

// The operand/output types in these implementations are ndarray types only.
macro_rules! impl_operations {
    ($dim:ty, $output:ty, $multiply:ident, $solve:ident, $lower:ident) => {
        impl<S: Data<Elem = f64>, R: Data<Elem = f64>> MatMul<ArrayBase<R, $dim>>
            for ArrayBase<S, Ix2>
        {
            type Output = $output;
            fn matmul(&self, rhs: &ArrayBase<R, $dim>) -> Self::Output {
                backend::$multiply(self.view(), rhs.view())
            }
        }
        impl<R: Data<Elem = f64>> Solve<ArrayBase<R, $dim>> for Cholesky {
            type Output = $output;
            fn solve(&self, rhs: &ArrayBase<R, $dim>) -> Self::Output {
                self.$solve(rhs.view())
            }
        }
        impl<R: Data<Elem = f64>> Solve<ArrayBase<R, $dim>> for Lu {
            type Output = $output;
            fn solve(&self, rhs: &ArrayBase<R, $dim>) -> Self::Output {
                self.$solve(rhs.view())
            }
        }
        impl<S: Data<Elem = f64>, R: ndarray::DataMut<Elem = f64>>
            SolveLowerTriangular<ArrayBase<R, $dim>> for ArrayBase<S, Ix2>
        {
            fn solve_lower_triangular_in_place(&self, rhs: &mut ArrayBase<R, $dim>) {
                backend::$lower(self.view(), rhs.view_mut());
            }
        }
    };
}
impl_operations!(
    ndarray::Ix1,
    Array1<f64>,
    matvec,
    solve_vector,
    lower_vector
);
impl_operations!(Ix2, Array2<f64>, matmul, solve_matrix, lower_matrix);
