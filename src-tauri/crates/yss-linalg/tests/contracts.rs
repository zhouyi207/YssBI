use ndarray::{Array2, ShapeBuilder, array, s};
use yss_linalg::{
    Eigen, LinalgError, MatMul, MatrixExt, Solve, SolveLowerTriangular, SymmetricEigen, matrix_rank,
};

fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-11 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

#[test]
fn strided_views_preserve_logical_indices_and_exclusive_writes() {
    let data = Array2::from_shape_fn((4, 6).f(), |(row, col)| (row * 6 + col) as f64);
    let left = data.slice(s![..;-1, ..;2]);
    let right = array![[1., 2.], [3., 4.], [5., 6.]];
    let actual = left.matmul(&right);
    let expected = left.dot(&right);
    for (&a, &b) in actual.iter().zip(expected.iter()) {
        near(a, b);
    }
    let row = array![[1., 2., 3.]];
    let broadcast = row.broadcast((4, 3)).unwrap();
    assert_eq!(broadcast.matmul(&right), broadcast.dot(&right));

    let lower = array![[2., 0.], [1., 3.]];
    let mut storage = array![[99., 99., 99., 99.], [99., 99., 99., 99.]];
    {
        let mut rhs = storage.slice_mut(s![..;-1, ..;2]);
        rhs.assign(&array![[2., 4.], [7., 11.]]);
        lower.solve_lower_triangular_in_place(&mut rhs);
        for (&a, &b) in rhs.iter().zip(array![[1., 2.], [2., 3.]].iter()) {
            near(a, b);
        }
    }
    assert!(
        storage
            .column(1)
            .iter()
            .chain(storage.column(3).iter())
            .all(|&v| v == 99.)
    );
}

#[test]
fn decompositions_reuse_factors_for_vectors_and_multiple_rhs() {
    let matrix = array![[4., 1.], [1., 3.]];
    let factor = matrix.cholesky().unwrap();
    let rhs = array![6., 7.];
    let solution = factor.solve(&rhs);
    near(solution[0], 1.);
    near(solution[1], 2.);
    let inverse = factor.solve(&Array2::eye(2));
    let identity = matrix.matmul(&inverse);
    for ((row, col), &value) in identity.indexed_iter() {
        near(value, if row == col { 1. } else { 0. });
    }
    let lower = factor.lower();
    for (&a, &b) in lower.matmul(&lower.t()).iter().zip(matrix.iter()) {
        near(a, b);
    }
    let mut strided_rhs = array![7., 99., 6., 99.];
    lower.solve_lower_triangular_in_place(
        &mut strided_rhs.slice_mut(s![..;2]).slice_move(s![..;-1]),
    );
    near(strided_rhs[1], 99.);
    near(strided_rhs[3], 99.);

    let pivoted = array![[0., 2.], [1., 3.]];
    let lu = pivoted.lu().unwrap();
    let x = lu.solve(&array![4., 7.]);
    near(x[0], 1.);
    near(x[1], 2.);
    assert!(matches!(
        array![[1., 2.], [2., 1.]].cholesky(),
        Err(LinalgError::NotPositiveDefinite)
    ));
    assert!(matches!(
        Array2::zeros((2, 3)).cholesky(),
        Err(LinalgError::NotSquare)
    ));
    assert!(matches!(
        array![[1., 2.], [2., 4.]].lu(),
        Err(LinalgError::Singular)
    ));
}

#[test]
fn svd_orientation_rank_and_empty_matrix_conventions_are_stable() {
    for matrix in [
        array![[1., 2., 3.], [4., 5., 6.]],
        array![[1., 4.], [2., 5.], [3., 6.]],
    ] {
        let svd = matrix.svd().unwrap();
        let values = svd.values();
        let count = values.len();
        let reconstructed = svd
            .left_vectors()
            .slice(s![.., ..count])
            .matmul(&Array2::from_diag(&values))
            .matmul(&svd.right_vectors().slice(s![.., ..count]).t());
        for (&a, &b) in reconstructed.iter().zip(matrix.iter()) {
            near(a, b);
        }
        assert!(values[0] >= values[1]);
    }
    assert_eq!(matrix_rank(Array2::zeros((0, 3)).view()).unwrap(), (0, 1.));
    assert_eq!(matrix_rank(Array2::zeros((3, 0)).view()).unwrap(), (0, 1.));
    assert_eq!(
        matrix_rank(Array2::zeros((2, 2)).view()).unwrap(),
        (0, f64::INFINITY)
    );
    let (rank, condition) = matrix_rank(array![[1., 0.], [0., 1e-17]].view()).unwrap();
    assert_eq!(rank, 1);
    near(condition, 1e17);
}

#[test]
fn rank_of_a_tall_design_does_not_require_square_singular_vectors() {
    let design = Array2::from_shape_fn((100_000, 2), |(row, column)| {
        if column == 0 {
            1.0
        } else {
            row as f64 / 1000.0
        }
    });
    let (rank, condition) = matrix_rank(design.view()).unwrap();
    assert_eq!(rank, 2);
    assert!(condition.is_finite() && condition > 100.0 && condition < 120.0);
}

#[test]
fn eigenvectors_match_real_and_complex_eigenvalues_without_assuming_order() {
    let symmetric = array![[2., 1.], [1., 2.]];
    let decomposition = SymmetricEigen::factor(symmetric.view()).unwrap();
    let values = decomposition.values();
    near(values[0], 1.);
    near(values[1], 3.);
    let vectors = decomposition.vectors();
    let reconstructed = vectors
        .matmul(&Array2::from_diag(&values))
        .matmul(&vectors.t());
    for (&a, &b) in reconstructed.iter().zip(symmetric.iter()) {
        near(a, b);
    }

    let rotation = array![[0., -1.], [1., 0.]];
    let decomposition = Eigen::factor(rotation.view()).unwrap();
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
