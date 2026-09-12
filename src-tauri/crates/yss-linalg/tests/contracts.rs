use faer::{Mat, MatRef, col, mat};
use yss_linalg::{Eigen, LinalgError, MatrixExt, Solve, Svd, SymmetricEigen, matrix_rank};

fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-11 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

fn near_matrix(actual: MatRef<'_, f64>, expected: MatRef<'_, f64>) {
    assert_eq!(actual.shape(), expected.shape());
    for row in 0..actual.nrows() {
        for col in 0..actual.ncols() {
            near(actual[(row, col)], expected[(row, col)]);
        }
    }
}

#[test]
fn checked_factors_accept_submatrices_and_reversed_views() {
    let mut storage = Mat::full(4, 4, 99.0);
    storage
        .submatrix_mut(1, 1, 2, 2)
        .copy_from(&mat![[4., 1.], [1., 3.]]);
    let matrix = storage.submatrix(1, 1, 2, 2).reverse_rows_and_cols();
    let factor = matrix.checked_cholesky().unwrap();
    let rhs_storage = col![9., 5.];
    let rhs = rhs_storage.reverse_rows();
    let result = factor.solve(&rhs);
    near(result[0], 1.);
    near(result[1], 2.);
    let mut rhs_matrix_storage = Mat::full(4, 4, 99.0);
    rhs_matrix_storage
        .submatrix_mut(1, 1, 2, 2)
        .copy_from(&mat![[5., 9.], [9., 14.]]);
    let rhs_matrix = rhs_matrix_storage.submatrix(1, 1, 2, 2);
    let solutions = factor.solve(&rhs_matrix);
    near_matrix(solutions.as_ref(), mat![[1., 2.], [2., 3.]].as_ref());
    assert_eq!(storage[(0, 0)], 99.);
    assert_eq!(rhs_matrix_storage[(3, 3)], 99.);
}

#[test]
fn decompositions_reuse_factors_for_vectors_and_multiple_rhs() {
    let matrix = mat![[4., 1.], [1., 3.]];
    let factor = matrix.checked_cholesky().unwrap();
    let rhs = col![6., 7.];
    let solution = factor.solve(&rhs);
    near(solution[0], 1.);
    near(solution[1], 2.);
    let inverse = factor.solve(&Mat::identity(2, 2));
    near_matrix((&matrix * &inverse).as_ref(), Mat::identity(2, 2).as_ref());
    let lower = factor.lower();
    near_matrix((&lower * lower.transpose()).as_ref(), matrix.as_ref());

    let pivoted = mat![[0., 2.], [1., 3.]];
    let lu = pivoted.checked_lu().unwrap();
    let x = lu.solve(&col![4., 7.]);
    near(x[0], 1.);
    near(x[1], 2.);
    assert!(matches!(
        mat![[1., 2.], [2., 1.]].checked_cholesky(),
        Err(LinalgError::NotPositiveDefinite)
    ));
    assert!(matches!(
        Mat::zeros(2, 3).checked_cholesky(),
        Err(LinalgError::NotSquare)
    ));
    assert!(matches!(
        mat![[1., 2.], [2., 4.]].checked_lu(),
        Err(LinalgError::Singular)
    ));
}

#[test]
fn svd_orientation_rank_and_empty_matrix_conventions_are_stable() {
    for matrix in [
        mat![[1., 2., 3.], [4., 5., 6.]],
        mat![[1., 4.], [2., 5.], [3., 6.]],
    ] {
        let svd = Svd::factor(matrix.as_ref()).unwrap();
        let values = svd.values();
        let count = values.nrows();
        let diagonal = Mat::from_fn(count, count, |i, j| if i == j { values[i] } else { 0.0 });
        let reconstructed = svd.left_vectors().subcols(0, count)
            * diagonal
            * svd.right_vectors().subcols(0, count).transpose();
        near_matrix(reconstructed.as_ref(), matrix.as_ref());
        assert!(values[0] >= values[1]);
    }
    assert_eq!(matrix_rank(Mat::zeros(0, 3).as_ref()).unwrap(), (0, 1.));
    assert_eq!(matrix_rank(Mat::zeros(3, 0).as_ref()).unwrap(), (0, 1.));
    assert_eq!(
        matrix_rank(Mat::zeros(2, 2).as_ref()).unwrap(),
        (0, f64::INFINITY)
    );
    let (rank, condition) = matrix_rank(mat![[1., 0.], [0., 1e-17]].as_ref()).unwrap();
    assert_eq!(rank, 1);
    near(condition, 1e17);
}

#[test]
fn rank_of_a_tall_design_does_not_require_square_singular_vectors() {
    let design = Mat::from_fn(100_000, 2, |row, column| {
        if column == 0 {
            1.0
        } else {
            row as f64 / 1000.0
        }
    });
    let (rank, condition) = matrix_rank(design.as_ref()).unwrap();
    assert_eq!(rank, 2);
    assert!(condition.is_finite() && condition > 100.0 && condition < 120.0);
}

#[test]
fn eigenvectors_match_real_and_complex_eigenvalues_without_assuming_order() {
    let symmetric = mat![[2., 1.], [1., 2.]];
    let decomposition = SymmetricEigen::factor(symmetric.as_ref()).unwrap();
    let values = decomposition.values();
    near(values[0], 1.);
    near(values[1], 3.);
    let vectors = decomposition.vectors();
    let diagonal = Mat::from_fn(2, 2, |i, j| if i == j { values[i] } else { 0.0 });
    near_matrix(
        (vectors * diagonal * vectors.transpose()).as_ref(),
        symmetric.as_ref(),
    );

    let rotation = mat![[0., -1.], [1., 0.]];
    let decomposition = Eigen::factor(rotation.as_ref()).unwrap();
    let vectors = decomposition.vectors();
    for (col, value) in decomposition.values().iter().enumerate() {
        near(value.re, 0.);
        near(value.im.abs(), 1.);
        for row in 0..2 {
            let component = vectors[(row, col)];
            let real: f64 = (0..2)
                .map(|j| rotation[(row, j)] * vectors[(j, col)].re)
                .sum();
            let imaginary: f64 = (0..2)
                .map(|j| rotation[(row, j)] * vectors[(j, col)].im)
                .sum();
            near(real, value.re * component.re - value.im * component.im);
            near(imaginary, value.re * component.im + value.im * component.re);
        }
    }
}
