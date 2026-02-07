use faer::prelude::*;

pub fn matmul(a: &Mat<f64>, b: &Mat<f64>) -> Mat<f64> {
    let mut c = Mat::zeros(a.nrows(), b.ncols());
    c.copy_from(&(a * b));
    c
}

pub fn solve_qr(a: &Mat<f64>, b: &Mat<f64>) -> Mat<f64> {
    a.qr().solve(b)
}
