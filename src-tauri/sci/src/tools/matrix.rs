use faer::Mat;

pub fn matrix_rank(mat: Mat<f64>) -> (usize, f64) {
    let svd = mat.svd().unwrap();
    let s = svd.S().column_vector();
    let sigma_max = s.max().unwrap();
    let sigma_min = s.min().unwrap();
    let tol = sigma_max * (mat.nrows().max(mat.ncols()) as f64) * f64::EPSILON;

    (
        s.iter().filter(|&&v| v > tol).count(),
        sigma_max / sigma_min,
    )
}
