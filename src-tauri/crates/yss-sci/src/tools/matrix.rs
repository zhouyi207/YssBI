use faer::Mat;

/// Numeric rank via SVD. Safe for empty matrices (0 rows and/or 0 columns).
pub fn matrix_rank(mat: Mat<f64>) -> (usize, f64) {
    let n = mat.nrows();
    let m = mat.ncols();
    if n == 0 || m == 0 {
        return (0, 1.0);
    }

    let svd = match mat.svd() {
        Ok(s) => s,
        Err(_) => return (0, f64::INFINITY),
    };
    let s = svd.S().column_vector();
    if s.nrows() == 0 {
        return (0, 1.0);
    }

    let mut sigma_max = f64::NEG_INFINITY;
    let mut sigma_min = f64::INFINITY;
    for v in s.iter() {
        let v = *v;
        if v > sigma_max {
            sigma_max = v;
        }
        if v < sigma_min {
            sigma_min = v;
        }
    }
    if !sigma_max.is_finite() || sigma_max <= 0.0 {
        return (0, f64::INFINITY);
    }

    let cond_no = if sigma_min > 0.0 {
        sigma_max / sigma_min
    } else {
        f64::INFINITY
    };

    let tol = sigma_max * (n.max(m) as f64) * f64::EPSILON;
    (s.iter().filter(|&&v| v > tol).count(), cond_no)
}
