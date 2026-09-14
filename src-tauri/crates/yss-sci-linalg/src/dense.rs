//! Owned dense arrays and borrowed views. Only this crate sees the faer representation.
use std::ops::{Index, IndexMut};

#[derive(Debug, Clone, PartialEq)]
pub struct Mat<T>(pub(crate) faer::Mat<T>);
#[derive(Debug, Clone, PartialEq)]
pub struct Col<T>(pub(crate) faer::Col<T>);
#[derive(Debug, Clone, PartialEq)]
pub struct Row<T>(pub(crate) faer::Row<T>);
#[derive(Debug)]
pub struct MatRef<'a, T>(pub(crate) faer::MatRef<'a, T>);
#[derive(Debug)]
pub struct ColRef<'a, T>(pub(crate) faer::ColRef<'a, T>);
#[derive(Debug)]
pub struct RowRef<'a, T>(pub(crate) faer::RowRef<'a, T>);
pub struct MatMut<'a, T>(pub(crate) faer::MatMut<'a, T>);
pub struct ColMut<'a, T>(pub(crate) faer::ColMut<'a, T>);
pub struct RowMut<'a, T>(pub(crate) faer::RowMut<'a, T>);
pub struct DiagRef<'a, T>(ColRef<'a, T>);

macro_rules! copy_view {
    ($name:ident) => {
        impl<T> Copy for $name<'_, T> {}
        impl<T> Clone for $name<'_, T> {
            fn clone(&self) -> Self {
                *self
            }
        }
    };
}
copy_view!(MatRef);
copy_view!(ColRef);
copy_view!(RowRef);

impl<T> Mat<T> {
    pub fn full(rows: usize, cols: usize, value: T) -> Self
    where
        T: Clone,
    {
        Self::from_fn(rows, cols, |_, _| value.clone())
    }
    pub fn from_fn(rows: usize, cols: usize, f: impl FnMut(usize, usize) -> T) -> Self {
        Self(faer::Mat::from_fn(rows, cols, f))
    }
    pub fn nrows(&self) -> usize {
        self.0.nrows()
    }
    pub fn ncols(&self) -> usize {
        self.0.ncols()
    }
    pub fn as_ref(&self) -> MatRef<'_, T> {
        MatRef(self.0.as_ref())
    }
    pub fn as_mut(&mut self) -> MatMut<'_, T> {
        MatMut(self.0.as_mut())
    }
    pub fn transpose(&self) -> MatRef<'_, T> {
        self.as_ref().transpose()
    }
    pub fn col(&self, index: usize) -> ColRef<'_, T> {
        self.as_ref().col(index)
    }
    pub fn row(&self, index: usize) -> RowRef<'_, T> {
        self.as_ref().row(index)
    }
    pub fn col_iter(&self) -> impl Iterator<Item = ColRef<'_, T>> {
        self.0.col_iter().map(ColRef)
    }
    pub fn row_iter(&self) -> impl Iterator<Item = RowRef<'_, T>> {
        self.0.row_iter().map(RowRef)
    }
    pub fn row_iter_mut(&mut self) -> impl Iterator<Item = RowMut<'_, T>> {
        self.0.row_iter_mut().map(RowMut)
    }
    pub fn diagonal(&self) -> DiagRef<'_, T> {
        self.as_ref().diagonal()
    }
    pub fn submatrix(&self, row: usize, col: usize, rows: usize, cols: usize) -> MatRef<'_, T> {
        self.as_ref().submatrix(row, col, rows, cols)
    }
    pub fn subrows(&self, start: usize, rows: usize) -> MatRef<'_, T> {
        self.as_ref().subrows(start, rows)
    }
    pub fn subcols(&self, start: usize, cols: usize) -> MatRef<'_, T> {
        self.as_ref().subcols(start, cols)
    }
    pub fn submatrix_mut(
        &mut self,
        row: usize,
        col: usize,
        rows: usize,
        cols: usize,
    ) -> MatMut<'_, T> {
        MatMut(self.0.submatrix_mut(row, col, rows, cols))
    }
}
impl Mat<f64> {
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self(faer::Mat::zeros(rows, cols))
    }
    pub fn identity(rows: usize, cols: usize) -> Self {
        Self(faer::Mat::identity(rows, cols))
    }
    pub fn singular_values(&self) -> Result<Vec<f64>, crate::LinalgError> {
        self.as_ref().singular_values()
    }
}
impl<'a, T> MatRef<'a, T> {
    pub fn shape(self) -> (usize, usize) {
        (self.nrows(), self.ncols())
    }
    pub fn from_row_major_slice(values: &'a [T], rows: usize, cols: usize) -> Self {
        Self(faer::MatRef::from_row_major_slice(values, rows, cols))
    }
    pub fn from_column_major_slice(values: &'a [T], rows: usize, cols: usize) -> Self {
        Self(faer::MatRef::from_column_major_slice(values, rows, cols))
    }
    pub fn as_ref(&self) -> Self {
        *self
    }
    pub fn nrows(self) -> usize {
        self.0.nrows()
    }
    pub fn ncols(self) -> usize {
        self.0.ncols()
    }
    pub fn transpose(self) -> Self {
        Self(self.0.transpose())
    }
    pub fn col(self, index: usize) -> ColRef<'a, T> {
        ColRef(self.0.col(index))
    }
    pub fn row(self, index: usize) -> RowRef<'a, T> {
        RowRef(self.0.row(index))
    }
    pub fn col_iter(self) -> impl Iterator<Item = ColRef<'a, T>> {
        self.0.col_iter().map(ColRef)
    }
    pub fn row_iter(self) -> impl Iterator<Item = RowRef<'a, T>> {
        self.0.row_iter().map(RowRef)
    }
    pub fn diagonal(self) -> DiagRef<'a, T> {
        DiagRef(ColRef(self.0.diagonal().column_vector()))
    }
    pub fn submatrix(self, row: usize, col: usize, rows: usize, cols: usize) -> Self {
        Self(self.0.submatrix(row, col, rows, cols))
    }
    pub fn subrows(self, start: usize, rows: usize) -> Self {
        Self(self.0.subrows(start, rows))
    }
    pub fn subcols(self, start: usize, cols: usize) -> Self {
        Self(self.0.subcols(start, cols))
    }
    pub fn reverse_rows_and_cols(self) -> Self {
        Self(self.0.reverse_rows_and_cols())
    }
    pub fn to_owned(self) -> Mat<T>
    where
        T: Clone,
    {
        Mat::from_fn(self.nrows(), self.ncols(), |row, col| {
            self[(row, col)].clone()
        })
    }
}
impl MatRef<'_, f64> {
    pub fn singular_values(self) -> Result<Vec<f64>, crate::LinalgError> {
        self.0
            .singular_values()
            .map_err(|_| crate::LinalgError::DecompositionFailed)
    }
    pub fn solve_lower_triangular_in_place<R: LowerTriangularRhs>(self, rhs: R) {
        rhs.solve_lower(self);
    }
}
impl MatMut<'_, f64> {
    pub fn copy_from(&mut self, source: &Mat<f64>) {
        self.0.copy_from(source.0.as_ref());
    }
}
impl<T> Col<T> {
    pub fn full(rows: usize, value: T) -> Self
    where
        T: Clone,
    {
        Self::from_fn(rows, |_| value.clone())
    }
    pub fn map<U>(&self, mut f: impl FnMut(&T) -> U) -> Col<U> {
        Col::from_fn(self.nrows(), |i| f(&self[i]))
    }
    pub fn from_fn(rows: usize, f: impl FnMut(usize) -> T) -> Self {
        Self(faer::Col::from_fn(rows, f))
    }
    pub fn nrows(&self) -> usize {
        self.0.nrows()
    }
    pub fn as_ref(&self) -> ColRef<'_, T> {
        ColRef(self.0.as_ref())
    }
    pub fn as_mut(&mut self) -> ColMut<'_, T> {
        ColMut(self.0.as_mut())
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        self.0.iter()
    }
    pub fn transpose(&self) -> RowRef<'_, T> {
        self.as_ref().transpose()
    }
    pub fn subrows(&self, start: usize, rows: usize) -> ColRef<'_, T> {
        self.as_ref().subrows(start, rows)
    }
    pub fn reverse_rows(&self) -> ColRef<'_, T> {
        self.as_ref().reverse_rows()
    }
}
impl Col<f64> {
    pub fn zeros(rows: usize) -> Self {
        Self(faer::Col::zeros(rows))
    }
}
impl<T> FromIterator<T> for Col<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(faer::Col::from_iter(iter))
    }
}
impl<'a, T> ColRef<'a, T> {
    pub fn map<U>(self, mut f: impl FnMut(&T) -> U) -> Col<U> {
        Col::from_fn(self.nrows(), |i| f(&self[i]))
    }
    pub fn from_slice(values: &'a [T]) -> Self {
        Self(faer::ColRef::from_slice(values))
    }
    pub fn as_ref(&self) -> Self {
        *self
    }
    pub fn nrows(self) -> usize {
        self.0.nrows()
    }
    pub fn iter(self) -> impl DoubleEndedIterator<Item = &'a T> + ExactSizeIterator {
        self.0.iter()
    }
    pub fn transpose(self) -> RowRef<'a, T> {
        RowRef(self.0.transpose())
    }
    pub fn subrows(self, start: usize, rows: usize) -> Self {
        Self(self.0.subrows(start, rows))
    }
    pub fn reverse_rows(self) -> Self {
        Self(self.0.reverse_rows())
    }
    pub fn to_owned(self) -> Col<T>
    where
        T: Clone,
    {
        Col::from_fn(self.nrows(), |row| self[row].clone())
    }
}
impl<'a, T> RowRef<'a, T> {
    pub fn as_ref(&self) -> Self {
        *self
    }
    pub fn ncols(self) -> usize {
        self.0.ncols()
    }
    pub fn iter(self) -> impl DoubleEndedIterator<Item = &'a T> + ExactSizeIterator {
        self.0.iter()
    }
    pub fn transpose(self) -> ColRef<'a, T> {
        ColRef(self.0.transpose())
    }
}
impl<T> Row<T> {
    pub fn as_ref(&self) -> RowRef<'_, T> {
        RowRef(self.0.as_ref())
    }
    pub fn transpose(&self) -> ColRef<'_, T> {
        self.as_ref().transpose()
    }
}
impl<'a, T> DiagRef<'a, T> {
    pub fn column_vector(self) -> ColRef<'a, T> {
        self.0
    }
}

macro_rules! index_matrix {
    ($name:ty) => {
        impl<T> Index<(usize, usize)> for $name {
            type Output = T;
            fn index(&self, index: (usize, usize)) -> &T {
                &self.0[index]
            }
        }
    };
}
index_matrix!(Mat<T>);
index_matrix!(MatRef<'_, T>);
index_matrix!(MatMut<'_, T>);
impl<T> IndexMut<(usize, usize)> for Mat<T> {
    fn index_mut(&mut self, index: (usize, usize)) -> &mut T {
        &mut self.0[index]
    }
}
impl<T> IndexMut<(usize, usize)> for MatMut<'_, T> {
    fn index_mut(&mut self, index: (usize, usize)) -> &mut T {
        &mut self.0[index]
    }
}
macro_rules! index_vector {
    ($name:ty) => {
        impl<T> Index<usize> for $name {
            type Output = T;
            fn index(&self, index: usize) -> &T {
                &self.0[index]
            }
        }
    };
}
index_vector!(Col<T>);
index_vector!(ColRef<'_, T>);
index_vector!(ColMut<'_, T>);
index_vector!(RowRef<'_, T>);
index_vector!(Row<T>);
impl<T> IndexMut<usize> for Col<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.0[index]
    }
}
impl<T> IndexMut<usize> for ColMut<'_, T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.0[index]
    }
}

/// A mutable matrix or vector accepted by the triangular solve.
pub trait LowerTriangularRhs {
    fn solve_lower(self, matrix: MatRef<'_, f64>);
}
impl LowerTriangularRhs for MatMut<'_, f64> {
    fn solve_lower(self, matrix: MatRef<'_, f64>) {
        matrix.0.solve_lower_triangular_in_place(self.0);
    }
}
impl LowerTriangularRhs for ColMut<'_, f64> {
    fn solve_lower(self, matrix: MatRef<'_, f64>) {
        matrix.0.solve_lower_triangular_in_place(self.0);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Scale(pub f64);

macro_rules! binary_op {
    ($trait:ident, $method:ident, $op:tt, $left:ty, $right:ty, $output:ident) => {
        impl std::ops::$trait<$right> for $left {
            type Output = $output<f64>;
            fn $method(self, rhs: $right) -> Self::Output { $output(self.as_ref().0 $op rhs.as_ref().0) }
        }
    };
}
macro_rules! matrix_rhs {
    ($trait:ident, $method:ident, $op:tt, $left:ty) => {
        binary_op!($trait, $method, $op, $left, Mat<f64>, Mat);
        binary_op!($trait, $method, $op, $left, &Mat<f64>, Mat);
        binary_op!($trait, $method, $op, $left, MatRef<'_, f64>, Mat);
        binary_op!($trait, $method, $op, $left, &MatRef<'_, f64>, Mat);
    };
}
macro_rules! matrix_binary {
    ($trait:ident, $method:ident, $op:tt) => {
        matrix_rhs!($trait, $method, $op, Mat<f64>);
        matrix_rhs!($trait, $method, $op, &Mat<f64>);
        matrix_rhs!($trait, $method, $op, MatRef<'_, f64>);
        matrix_rhs!($trait, $method, $op, &MatRef<'_, f64>);
    };
}
matrix_binary!(Mul, mul, *);
matrix_binary!(Add, add, +);
matrix_binary!(Sub, sub, -);
macro_rules! column_rhs {
    ($trait:ident, $method:ident, $op:tt, $left:ty) => {
        binary_op!($trait, $method, $op, $left, Col<f64>, Col);
        binary_op!($trait, $method, $op, $left, &Col<f64>, Col);
        binary_op!($trait, $method, $op, $left, ColRef<'_, f64>, Col);
        binary_op!($trait, $method, $op, $left, &ColRef<'_, f64>, Col);
    };
}
column_rhs!(Mul, mul, *, Mat<f64>);
column_rhs!(Mul, mul, *, &Mat<f64>);
column_rhs!(Mul, mul, *, MatRef<'_, f64>);
column_rhs!(Mul, mul, *, &MatRef<'_, f64>);
macro_rules! column_binary {
    ($trait:ident, $method:ident, $op:tt) => {
        column_rhs!($trait, $method, $op, Col<f64>);
        column_rhs!($trait, $method, $op, &Col<f64>);
        column_rhs!($trait, $method, $op, ColRef<'_, f64>);
        column_rhs!($trait, $method, $op, &ColRef<'_, f64>);
    };
}
column_binary!(Add, add, +);
column_binary!(Sub, sub, -);

macro_rules! scale_ops {
    ($operand:ty, $output:ident) => {
        impl std::ops::Mul<Scale> for $operand {
            type Output = $output<f64>;
            fn mul(self, rhs: Scale) -> Self::Output {
                $output(self.as_ref().0 * faer::Scale(rhs.0))
            }
        }
        impl std::ops::Div<Scale> for $operand {
            type Output = $output<f64>;
            fn div(self, rhs: Scale) -> Self::Output {
                $output(self.as_ref().0 / faer::Scale(rhs.0))
            }
        }
        impl std::ops::Mul<$operand> for Scale {
            type Output = $output<f64>;
            fn mul(self, rhs: $operand) -> Self::Output {
                $output(faer::Scale(self.0) * rhs.as_ref().0)
            }
        }
        impl std::ops::Neg for $operand {
            type Output = $output<f64>;
            fn neg(self) -> Self::Output {
                $output(-self.as_ref().0)
            }
        }
        impl std::ops::Mul<f64> for $operand {
            type Output = $output<f64>;
            fn mul(self, rhs: f64) -> Self::Output {
                self * Scale(rhs)
            }
        }
        impl std::ops::Mul<$operand> for f64 {
            type Output = $output<f64>;
            fn mul(self, rhs: $operand) -> Self::Output {
                Scale(self) * rhs
            }
        }
        impl std::ops::Div<f64> for $operand {
            type Output = $output<f64>;
            fn div(self, rhs: f64) -> Self::Output {
                self / Scale(rhs)
            }
        }
    };
}
scale_ops!(Mat<f64>, Mat);
scale_ops!(&Mat<f64>, Mat);
scale_ops!(MatRef<'_, f64>, Mat);
scale_ops!(&MatRef<'_, f64>, Mat);
scale_ops!(Col<f64>, Col);
scale_ops!(&Col<f64>, Col);
scale_ops!(ColRef<'_, f64>, Col);
scale_ops!(&ColRef<'_, f64>, Col);
impl std::ops::MulAssign<Scale> for RowMut<'_, f64> {
    fn mul_assign(&mut self, rhs: Scale) {
        self.0 *= faer::Scale(rhs.0);
    }
}
impl std::ops::AddAssign<Mat<f64>> for Mat<f64> {
    fn add_assign(&mut self, rhs: Mat<f64>) {
        self.0 += rhs.0;
    }
}

macro_rules! row_operations {
    ($row:ty) => {
        binary_op!(Mul, mul, *, $row, Mat<f64>, Row);
        binary_op!(Mul, mul, *, $row, &Mat<f64>, Row);
        binary_op!(Mul, mul, *, $row, MatRef<'_, f64>, Row);
        binary_op!(Mul, mul, *, $row, &MatRef<'_, f64>, Row);
        impl std::ops::Mul<Col<f64>> for $row { type Output = f64; fn mul(self, rhs: Col<f64>) -> f64 { self.as_ref().0 * rhs.as_ref().0 } }
        impl std::ops::Mul<&Col<f64>> for $row { type Output = f64; fn mul(self, rhs: &Col<f64>) -> f64 { self.as_ref().0 * rhs.as_ref().0 } }
        impl std::ops::Mul<ColRef<'_, f64>> for $row { type Output = f64; fn mul(self, rhs: ColRef<'_, f64>) -> f64 { self.as_ref().0 * rhs.as_ref().0 } }
        impl std::ops::Mul<&ColRef<'_, f64>> for $row { type Output = f64; fn mul(self, rhs: &ColRef<'_, f64>) -> f64 { self.as_ref().0 * rhs.as_ref().0 } }
    };
}
row_operations!(Row<f64>);
row_operations!(&Row<f64>);
row_operations!(RowRef<'_, f64>);
row_operations!(&RowRef<'_, f64>);

#[macro_export]
macro_rules! mat {
    ($([$($value:expr),* $(,)?]),* $(,)?) => {{
        let rows = [$([$($value),*]),*];
        $crate::Mat::from_fn(rows.len(), rows.first().map_or(0, |row| row.len()), |r, c| rows[r][c].clone())
    }};
}
#[macro_export]
macro_rules! col {
    ($($value:expr),* $(,)?) => { $crate::Col::from_iter([$($value),*]) };
}
