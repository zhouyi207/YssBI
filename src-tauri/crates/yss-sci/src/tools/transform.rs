use faer::prelude::*;
use ndarray::{ArrayView, ArrayViewMut, Ix1, Ix2, ShapeBuilder};

pub trait IntoFaer {
    type Faer;
    fn into_faer(self) -> Self::Faer;
}

pub trait IntoFaerCol {
    type Faer;
    fn into_faer_col(self) -> Self::Faer;
}

pub trait IntoNdarray {
    type Ndarray;
    fn into_ndarray(self) -> Self::Ndarray;
}

impl<'a, T> IntoFaer for ArrayView<'a, T, Ix2> {
    type Faer = MatRef<'a, T>;

    #[track_caller]
    fn into_faer(self) -> Self::Faer {
        let nrows = self.nrows();
        let ncols = self.ncols();
        let strides: [isize; 2] = self.strides().try_into().unwrap();
        let ptr = self.as_ptr();
        unsafe { faer::MatRef::from_raw_parts(ptr, nrows, ncols, strides[0], strides[1]) }
    }
}

impl<'a, T> IntoFaerCol for ArrayView<'a, T, Ix1> {
    type Faer = ColRef<'a, T>;

    #[track_caller]
    fn into_faer_col(self) -> Self::Faer {
        let nrows = self.len();
        let row_stride: [isize; 1] = self.strides().try_into().unwrap();
        let ptr = self.as_ptr();
        unsafe { faer::ColRef::from_raw_parts(ptr, nrows, row_stride[0]) }
    }
}

impl<'a, T> IntoFaer for ArrayViewMut<'a, T, Ix2> {
    type Faer = MatMut<'a, T>;

    #[track_caller]
    fn into_faer(self) -> Self::Faer {
        let nrows = self.nrows();
        let ncols = self.ncols();
        let strides: [isize; 2] = self.strides().try_into().unwrap();
        let ptr = { self }.as_mut_ptr();
        unsafe { faer::MatMut::from_raw_parts_mut(ptr, nrows, ncols, strides[0], strides[1]) }
    }
}

impl<'a, T> IntoNdarray for MatRef<'a, T> {
    type Ndarray = ArrayView<'a, T, Ix2>;

    #[track_caller]
    fn into_ndarray(self) -> Self::Ndarray {
        let nrows = self.nrows();
        let ncols = self.ncols();
        let row_stride: usize = self.row_stride().try_into().unwrap();
        let col_stride: usize = self.col_stride().try_into().unwrap();
        let ptr = self.as_ptr();
        unsafe {
            ArrayView::<'_, T, Ix2>::from_shape_ptr(
                (nrows, ncols).strides((row_stride, col_stride)),
                ptr,
            )
        }
    }
}

impl<'a, T> IntoNdarray for MatMut<'a, T> {
    type Ndarray = ArrayViewMut<'a, T, Ix2>;

    #[track_caller]
    fn into_ndarray(self) -> Self::Ndarray {
        let nrows = self.nrows();
        let ncols = self.ncols();
        let row_stride: usize = self.row_stride().try_into().unwrap();
        let col_stride: usize = self.col_stride().try_into().unwrap();
        let ptr = self.as_ptr_mut();
        unsafe {
            ArrayViewMut::<'_, T, Ix2>::from_shape_ptr(
                (nrows, ncols).strides((row_stride, col_stride)),
                ptr,
            )
        }
    }
}

impl<'a, T> IntoNdarray for ColRef<'a, T> {
    type Ndarray = ArrayView<'a, T, Ix1>;

    #[track_caller]
    fn into_ndarray(self) -> Self::Ndarray {
        let nrows = self.nrows();
        let row_stride: usize = self.row_stride().try_into().unwrap();
        let ptr = self.as_ptr();
        unsafe { ArrayView::<'_, T, Ix1>::from_shape_ptr(nrows.strides(row_stride), ptr) }
    }
}

impl<'a, T> IntoNdarray for ColMut<'a, T> {
    type Ndarray = ArrayViewMut<'a, T, Ix1>;

    #[track_caller]
    fn into_ndarray(self) -> Self::Ndarray {
        let nrows = self.nrows();
        let row_stride: usize = self.row_stride().try_into().unwrap();
        let ptr = self.as_ptr_mut();
        unsafe { ArrayViewMut::<'_, T, Ix1>::from_shape_ptr(nrows.strides(row_stride), ptr) }
    }
}
