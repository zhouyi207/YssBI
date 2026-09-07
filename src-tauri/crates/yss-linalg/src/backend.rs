//! The only module that knows the concrete numerical library and memory layout.

use crate::{ComplexValue, LinalgError};
use faer::{MatRef, Side, linalg::solvers::Solve};
use ndarray::{Array1, Array2, ArrayView1, ArrayView2, ArrayViewMut1, ArrayViewMut2, ShapeBuilder};

fn matrix_view(array: ArrayView2<'_, f64>) -> MatRef<'_, f64> {
    let (rows, cols) = array.dim();
    let strides = array.strides();
    // ndarray guarantees aligned, initialized elements for every logical index,
    // including transposed, sliced, broadcast and negative-stride shared views.
    // The result borrows that storage for exactly the input view's lifetime.
    unsafe { MatRef::from_raw_parts(array.as_ptr(), rows, cols, strides[0], strides[1]) }
}

fn vector_view(array: ArrayView1<'_, f64>) -> faer::ColRef<'_, f64> {
    // Same borrowing invariant as matrix_view; no mutable alias is constructed.
    unsafe { faer::ColRef::from_raw_parts(array.as_ptr(), array.len(), array.strides()[0]) }
}

fn matrix_output(matrix: MatRef<'_, f64>) -> Array2<f64> {
    Array2::from_shape_fn((matrix.nrows(), matrix.ncols()).f(), |(row, col)| {
        matrix[(row, col)]
    })
}

fn vector_output(vector: faer::ColRef<'_, f64>) -> Array1<f64> {
    Array1::from_iter(vector.iter().copied())
}

fn square(matrix: ArrayView2<'_, f64>) -> Result<(), LinalgError> {
    if matrix.nrows() == matrix.ncols() {
        Ok(())
    } else {
        Err(LinalgError::NotSquare)
    }
}

pub(crate) fn matmul(left: ArrayView2<'_, f64>, right: ArrayView2<'_, f64>) -> Array2<f64> {
    assert_eq!(left.ncols(), right.nrows(), "matrix product dimensions");
    matrix_output((matrix_view(left) * matrix_view(right)).as_ref())
}

pub(crate) fn matvec(left: ArrayView2<'_, f64>, right: ArrayView1<'_, f64>) -> Array1<f64> {
    assert_eq!(
        left.ncols(),
        right.len(),
        "matrix-vector product dimensions"
    );
    vector_output((matrix_view(left) * vector_view(right)).as_ref())
}

pub struct Cholesky {
    factor: faer::linalg::solvers::Llt<f64>,
}

impl Cholesky {
    pub(crate) fn factor(matrix: ArrayView2<'_, f64>) -> Result<Self, LinalgError> {
        square(matrix)?;
        let factor = matrix_view(matrix)
            .llt(Side::Lower)
            .map_err(|_| LinalgError::NotPositiveDefinite)?;
        Ok(Self { factor })
    }

    pub fn lower(&self) -> Array2<f64> {
        matrix_output(self.factor.L())
    }

    pub(crate) fn solve_matrix(&self, rhs: ArrayView2<'_, f64>) -> Array2<f64> {
        assert_eq!(self.factor.L().nrows(), rhs.nrows(), "solve dimensions");
        matrix_output(self.factor.solve(matrix_view(rhs)).as_ref())
    }

    pub(crate) fn solve_vector(&self, rhs: ArrayView1<'_, f64>) -> Array1<f64> {
        assert_eq!(self.factor.L().nrows(), rhs.len(), "solve dimensions");
        vector_output(self.factor.solve(vector_view(rhs)).as_ref())
    }
}

pub struct Lu {
    factor: faer::linalg::solvers::PartialPivLu<f64>,
    size: usize,
}

impl Lu {
    pub(crate) fn factor(matrix: ArrayView2<'_, f64>) -> Result<Self, LinalgError> {
        square(matrix)?;
        let factor = matrix_view(matrix).partial_piv_lu();
        if factor
            .U()
            .diagonal()
            .column_vector()
            .iter()
            .any(|&value| value == 0.0)
        {
            return Err(LinalgError::Singular);
        }
        Ok(Self {
            factor,
            size: matrix.nrows(),
        })
    }

    pub(crate) fn solve_matrix(&self, rhs: ArrayView2<'_, f64>) -> Array2<f64> {
        assert_eq!(self.size, rhs.nrows(), "solve dimensions");
        matrix_output(self.factor.solve(matrix_view(rhs)).as_ref())
    }

    pub(crate) fn solve_vector(&self, rhs: ArrayView1<'_, f64>) -> Array1<f64> {
        assert_eq!(self.size, rhs.len(), "solve dimensions");
        vector_output(self.factor.solve(vector_view(rhs)).as_ref())
    }
}

pub struct Svd {
    factor: faer::linalg::solvers::Svd<f64>,
}

impl Svd {
    pub(crate) fn factor(matrix: ArrayView2<'_, f64>) -> Result<Self, LinalgError> {
        let factor = matrix_view(matrix)
            .svd()
            .map_err(|_| LinalgError::DecompositionFailed)?;
        Ok(Self { factor })
    }
    pub fn left_vectors(&self) -> ArrayView2<'_, f64> {
        factor_matrix_view(self.factor.U())
    }
    pub fn right_vectors(&self) -> ArrayView2<'_, f64> {
        factor_matrix_view(self.factor.V())
    }
    pub fn values(&self) -> ArrayView1<'_, f64> {
        let values = self.factor.S().column_vector();
        let stride =
            usize::try_from(values.row_stride()).expect("owned factor has nonnegative strides");
        // The shared array borrows initialized values owned by self; no copy of
        // the (potentially large) singular vector matrices is needed for rank.
        unsafe { ArrayView1::from_shape_ptr(values.nrows().strides(stride), values.as_ptr()) }
    }
}

fn factor_matrix_view(matrix: MatRef<'_, f64>) -> ArrayView2<'_, f64> {
    let row_stride =
        usize::try_from(matrix.row_stride()).expect("owned factor has nonnegative strides");
    let col_stride =
        usize::try_from(matrix.col_stride()).expect("owned factor has nonnegative strides");
    // Only called on shared, initialized matrices owned by a decomposition.
    // The returned view cannot outlive that decomposition or mutate its factors.
    unsafe {
        ArrayView2::from_shape_ptr(
            (matrix.nrows(), matrix.ncols()).strides((row_stride, col_stride)),
            matrix.as_ptr(),
        )
    }
}

pub(crate) fn singular_values(matrix: ArrayView2<'_, f64>) -> Result<Vec<f64>, LinalgError> {
    matrix_view(matrix)
        .singular_values()
        .map_err(|_| LinalgError::DecompositionFailed)
}

/// Symmetric eigendecomposition from the lower triangle, eigenvalues ascending.
pub struct SymmetricEigen {
    vectors: Array2<f64>,
    values: Array1<f64>,
}

impl SymmetricEigen {
    pub fn factor(matrix: ArrayView2<'_, f64>) -> Result<Self, LinalgError> {
        square(matrix)?;
        let factor = faer::linalg::solvers::SelfAdjointEigen::new(matrix_view(matrix), Side::Lower)
            .map_err(|_| LinalgError::DecompositionFailed)?;
        Ok(Self {
            vectors: matrix_output(factor.U()),
            values: vector_output(factor.S().column_vector()),
        })
    }
    pub fn vectors(&self) -> ArrayView2<'_, f64> {
        self.vectors.view()
    }
    pub fn values(&self) -> ArrayView1<'_, f64> {
        self.values.view()
    }
}

/// General real eigendecomposition. Ordering is unspecified; column i matches
/// eigenvalue i. Eigenvectors retain the backend's normalization and phase.
pub struct Eigen {
    vectors: Array2<ComplexValue>,
    values: Array1<ComplexValue>,
}

impl Eigen {
    pub fn factor(matrix: ArrayView2<'_, f64>) -> Result<Self, LinalgError> {
        square(matrix)?;
        let factor = faer::linalg::solvers::Eigen::new_from_real(matrix_view(matrix))
            .map_err(|_| LinalgError::DecompositionFailed)?;
        let vectors = factor.U();
        Ok(Self {
            vectors: Array2::from_shape_fn(matrix.dim().f(), |(row, col)| {
                let value = vectors[(row, col)];
                ComplexValue {
                    re: value.re,
                    im: value.im,
                }
            }),
            values: Array1::from_iter(factor.S().column_vector().iter().map(|value| {
                ComplexValue {
                    re: value.re,
                    im: value.im,
                }
            })),
        })
    }
    pub fn vectors(&self) -> ArrayView2<'_, ComplexValue> {
        self.vectors.view()
    }
    pub fn values(&self) -> ArrayView1<'_, ComplexValue> {
        self.values.view()
    }
}

pub(crate) fn lower_matrix(lower: ArrayView2<'_, f64>, mut rhs: ArrayViewMut2<'_, f64>) {
    assert_eq!(
        lower.nrows(),
        lower.ncols(),
        "triangular matrix must be square"
    );
    assert_eq!(lower.nrows(), rhs.nrows(), "triangular solve dimensions");
    let (rows, cols) = rhs.dim();
    let strides = [rhs.strides()[0], rhs.strides()[1]];
    // A mutable ndarray view guarantees exclusive, nonoverlapping logical elements.
    let rhs_view = unsafe {
        faer::MatMut::from_raw_parts_mut(rhs.as_mut_ptr(), rows, cols, strides[0], strides[1])
    };
    matrix_view(lower).solve_lower_triangular_in_place(rhs_view);
}

pub(crate) fn lower_vector(lower: ArrayView2<'_, f64>, mut rhs: ArrayViewMut1<'_, f64>) {
    assert_eq!(
        lower.nrows(),
        lower.ncols(),
        "triangular matrix must be square"
    );
    assert_eq!(lower.nrows(), rhs.len(), "triangular solve dimensions");
    let stride = rhs.strides()[0];
    // Same exclusive borrowing invariant as lower_matrix.
    let rhs_view = unsafe { faer::ColMut::from_raw_parts_mut(rhs.as_mut_ptr(), rhs.len(), stride) };
    matrix_view(lower).solve_lower_triangular_in_place(rhs_view);
}
