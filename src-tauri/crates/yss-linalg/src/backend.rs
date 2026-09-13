use crate::{Col, ColRef, ComplexValue, LinalgError, Mat, MatRef};
use faer::{Side, linalg::solvers::Solve};

fn square(matrix: MatRef<'_, f64>) -> Result<(), LinalgError> {
    if matrix.nrows() == matrix.ncols() {
        Ok(())
    } else {
        Err(LinalgError::NotSquare)
    }
}

pub struct Cholesky {
    factor: faer::linalg::solvers::Llt<f64>,
}

impl Cholesky {
    pub fn factor(matrix: MatRef<'_, f64>) -> Result<Self, LinalgError> {
        square(matrix)?;
        let factor = matrix
            .0
            .llt(Side::Lower)
            .map_err(|_| LinalgError::NotPositiveDefinite)?;
        Ok(Self { factor })
    }

    pub fn lower(&self) -> Mat<f64> {
        Mat(self.factor.L().to_owned())
    }

    pub(crate) fn solve_matrix(&self, rhs: MatRef<'_, f64>) -> Mat<f64> {
        assert_eq!(self.factor.L().nrows(), rhs.nrows(), "solve dimensions");
        Mat(self.factor.solve(rhs.0))
    }

    pub(crate) fn solve_vector(&self, rhs: ColRef<'_, f64>) -> Col<f64> {
        assert_eq!(self.factor.L().nrows(), rhs.nrows(), "solve dimensions");
        Col(self.factor.solve(rhs.0))
    }
}

pub struct Lu {
    factor: faer::linalg::solvers::PartialPivLu<f64>,
    size: usize,
}

impl Lu {
    pub fn factor(matrix: MatRef<'_, f64>) -> Result<Self, LinalgError> {
        square(matrix)?;
        let factor = matrix.0.partial_piv_lu();
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

    pub(crate) fn solve_matrix(&self, rhs: MatRef<'_, f64>) -> Mat<f64> {
        assert_eq!(self.size, rhs.nrows(), "solve dimensions");
        Mat(self.factor.solve(rhs.0))
    }

    pub(crate) fn solve_vector(&self, rhs: ColRef<'_, f64>) -> Col<f64> {
        assert_eq!(self.size, rhs.nrows(), "solve dimensions");
        Col(self.factor.solve(rhs.0))
    }
}

pub struct Svd {
    factor: faer::linalg::solvers::Svd<f64>,
}

impl Svd {
    pub fn factor(matrix: MatRef<'_, f64>) -> Result<Self, LinalgError> {
        let factor = matrix
            .0
            .svd()
            .map_err(|_| LinalgError::DecompositionFailed)?;
        Ok(Self { factor })
    }

    pub fn left_vectors(&self) -> MatRef<'_, f64> {
        MatRef(self.factor.U())
    }

    pub fn right_vectors(&self) -> MatRef<'_, f64> {
        MatRef(self.factor.V())
    }

    pub fn values(&self) -> ColRef<'_, f64> {
        ColRef(self.factor.S().column_vector())
    }
}

/// Symmetric eigenvalues are ascending; only the lower triangle is read.
pub struct SymmetricEigen {
    factor: faer::linalg::solvers::SelfAdjointEigen<f64>,
}

impl SymmetricEigen {
    pub fn factor(matrix: MatRef<'_, f64>) -> Result<Self, LinalgError> {
        square(matrix)?;
        let factor = faer::linalg::solvers::SelfAdjointEigen::new(matrix.0, Side::Lower)
            .map_err(|_| LinalgError::DecompositionFailed)?;
        Ok(Self { factor })
    }

    pub fn vectors(&self) -> MatRef<'_, f64> {
        MatRef(self.factor.U())
    }

    pub fn values(&self) -> ColRef<'_, f64> {
        ColRef(self.factor.S().column_vector())
    }
}

/// General complex eigenvalues are unordered; column i matches eigenvalue i.
pub struct Eigen {
    vectors: Mat<ComplexValue>,
    values: Col<ComplexValue>,
}

impl Eigen {
    pub fn factor(matrix: MatRef<'_, f64>) -> Result<Self, LinalgError> {
        square(matrix)?;
        let factor = faer::linalg::solvers::Eigen::new_from_real(matrix.0)
            .map_err(|_| LinalgError::DecompositionFailed)?;
        let vectors = factor.U();
        let values = factor.S().column_vector();
        Ok(Self {
            vectors: Mat::from_fn(matrix.nrows(), matrix.ncols(), |row, col| {
                let value = vectors[(row, col)];
                ComplexValue {
                    re: value.re,
                    im: value.im,
                }
            }),
            values: Col::from_fn(values.nrows(), |i| {
                let value = values[i];
                ComplexValue {
                    re: value.re,
                    im: value.im,
                }
            }),
        })
    }

    pub fn vectors(&self) -> MatRef<'_, ComplexValue> {
        self.vectors.as_ref()
    }

    pub fn values(&self) -> ColRef<'_, ComplexValue> {
        self.values.as_ref()
    }
}
