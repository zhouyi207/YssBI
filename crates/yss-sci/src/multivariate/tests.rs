use super::*;
use std::time::{Duration, Instant};
use yss_sci_contract::{execution::*, multivariate::*};

fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn close(a: f64, b: f64, tolerance: f64) {
    assert!(
        (a - b).abs() <= tolerance * b.abs().max(1.0),
        "actual {a}, expected {b}"
    );
}
fn columns() -> Vec<Vec<f64>> {
    vec![
        vec![1., 2., 3., 4., 5., 6., 7., 8., 9., 10.],
        vec![2., 1., 5., 3., 7., 4., 8., 6., 9., 8.],
        vec![4., 3., 1., 6., 2., 8., 5., 9., 7., 10.],
    ]
}

#[test]
fn pca_matches_sample_covariance_and_reconstructs_original_units() {
    // NumPy 2.4.2 eigh(cov((X-mean)/sample_sd)) reference.
    let data = columns();
    let fit = pca(
        &data,
        PcaOptions {
            components: 3,
            standardize: true,
        },
        &control(),
    )
    .unwrap();
    for (a, b) in fit.report.eigenvalues.iter().zip([
        2.318542300134868,
        0.662365692459574,
        0.0190920074055593,
    ]) {
        close(*a, b, 1e-10);
    }
    close(
        fit.report.explained_variance_ratio[0],
        0.7728474333782891,
        1e-10,
    );
    close(fit.report.cumulative_variance_ratio[2], 1.0, 1e-10);
    for i in 0..10 {
        for (j, column) in data.iter().enumerate() {
            let value = (0..3)
                .map(|k| fit.coordinates[i][k] * fit.report.weights[j][k])
                .sum::<f64>()
                * fit.report.scales[j]
                + fit.report.means[j];
            close(value, column[i], 1e-10);
        }
    }
    let original = pca(
        &data,
        PcaOptions {
            components: 3,
            standardize: false,
        },
        &control(),
    )
    .unwrap();
    for (i, score) in original.coordinates.iter().enumerate() {
        for (j, column) in data.iter().enumerate() {
            close(
                (0..3)
                    .map(|k| score[k] * original.report.weights[j][k])
                    .sum::<f64>()
                    + original.report.means[j],
                column[i],
                1e-10,
            );
        }
    }
    let large = data
        .iter()
        .map(|column| column.iter().map(|x| x * 1e100).collect())
        .collect::<Vec<_>>();
    let scaled = pca(
        &large,
        PcaOptions {
            components: 3,
            standardize: false,
        },
        &control(),
    )
    .unwrap();
    for (a, b) in original
        .report
        .eigenvalues
        .iter()
        .zip(&scaled.report.eigenvalues)
    {
        close(*a, *b / 1e200, 1e-9);
    }
}

fn factor_columns() -> Vec<Vec<f64>> {
    let n = 64;
    let loadings = [
        [0.8, 0.1],
        [0.7, 0.2],
        [0.75, 0.05],
        [0.1, 0.8],
        [0.2, 0.7],
        [0.05, 0.75],
    ];
    let basis = |frequency: usize, row: usize| {
        ((n - 1) as f64 * 2.0 / n as f64).sqrt()
            * (2.0 * std::f64::consts::PI * frequency as f64 * row as f64 / n as f64).sin()
    };
    loadings
        .iter()
        .enumerate()
        .map(|(j, l)| {
            (0..n)
                .map(|i| {
                    l[0] * basis(1, i)
                        + l[1] * basis(2, i)
                        + (1.0_f64 - l[0] * l[0] - l[1] * l[1]).sqrt() * basis(j + 3, i)
                })
                .collect()
        })
        .collect()
}
#[test]
fn principal_axis_factor_communalities_and_diagnostics_match_reference() {
    // statsmodels 0.14.6 Factor(corr=R,n_factor=2,nobs=64,method='pa').
    let data = factor_columns();
    let fit = exploratory_factor(
        &data,
        FactorOptions {
            factors: 2,
            rotation: FactorRotation::None,
            max_iterations: 10000,
            tolerance: 1e-10,
        },
        &control(),
    )
    .unwrap();
    for (a, b) in fit
        .report
        .communalities
        .iter()
        .zip([0.65, 0.53, 0.565, 0.65, 0.53, 0.565])
    {
        close(*a, b, 1e-8);
    }
    close(fit.report.bartlett_chi_square, 120.36655701890909, 1e-9);
    assert_eq!(fit.report.bartlett_df, 15);
    assert!(fit.report.kmo.unwrap() > 0.5);
    assert_eq!(fit.coordinates.len(), 64);
    assert_eq!(fit.coordinates[0].len(), 2);
    assert!(
        exploratory_factor(
            &data,
            FactorOptions {
                factors: 2,
                max_iterations: 1,
                ..Default::default()
            },
            &control()
        )
        .is_err()
    );
}

#[test]
fn varimax_preserves_common_covariance_and_regression_score_axes() {
    let data = factor_columns();
    let options = FactorOptions {
        factors: 2,
        rotation: FactorRotation::None,
        max_iterations: 10000,
        tolerance: 1e-8,
    };
    let plain = exploratory_factor(&data, options, &control()).unwrap();
    let rotated = exploratory_factor(
        &data,
        FactorOptions {
            rotation: FactorRotation::Varimax,
            ..options
        },
        &control(),
    )
    .unwrap();
    for i in 0..6 {
        for j in 0..6 {
            let before = (0..2)
                .map(|k| plain.report.loadings[i][k] * plain.report.loadings[j][k])
                .sum::<f64>();
            let after = (0..2)
                .map(|k| rotated.report.loadings[i][k] * rotated.report.loadings[j][k])
                .sum::<f64>();
            close(before, after, 1e-8);
        }
    }
    let criterion = |loadings: &[Vec<f64>]| {
        (0..2)
            .map(|j| {
                loadings.iter().map(|row| row[j].powi(4)).sum::<f64>()
                    - loadings
                        .iter()
                        .map(|row| row[j].powi(2))
                        .sum::<f64>()
                        .powi(2)
                        / 6.0
            })
            .sum::<f64>()
    };
    assert!(criterion(&rotated.report.loadings) >= criterion(&plain.report.loadings) - 1e-10);
    for (a, b) in plain.coordinates.iter().zip(&rotated.coordinates) {
        close(
            a.iter().map(|x| x * x).sum(),
            b.iter().map(|x| x * x).sum(),
            1e-8,
        );
    }
}

#[test]
fn canonical_roots_scores_and_sequential_wilks_tests_match_reference() {
    let data = columns();
    let y = vec![
        data[2].clone(),
        vec![3.5, 2.2, 2.1, 6.8, 2.9, 8.8, 5.9, 9.6, 8.3, 11.6],
    ];
    let fit = canonical_correlation(&data[..2], &y, 2, &control()).unwrap();
    for (a, b) in fit
        .report
        .correlations
        .iter()
        .zip([0.9507103941048313, 0.7825009668110395])
    {
        close(*a, b, 1e-10);
    }
    close(fit.report.tests[0].wilks_lambda, 0.03727651031768807, 1e-10);
    close(
        fit.report.tests[0].chi_square.unwrap(),
        21.38104735587822,
        1e-9,
    );
    close(
        fit.report.tests[1].p_value.unwrap(),
        0.013074270346439916,
        1e-9,
    );
    for j in 0..2 {
        let vx = fit.x_scores.iter().map(|row| row[j] * row[j]).sum::<f64>() / 9.0;
        let vy = fit.y_scores.iter().map(|row| row[j] * row[j]).sum::<f64>() / 9.0;
        let cross = fit
            .x_scores
            .iter()
            .zip(&fit.y_scores)
            .map(|(x, y)| x[j] * y[j])
            .sum::<f64>()
            / 9.0;
        close(vx, 1.0, 1e-10);
        close(vy, 1.0, 1e-10);
        close(cross, fit.report.correlations[j], 1e-10);
    }
    let perfect = canonical_correlation(&data[..2], &data[..2], 1, &control()).unwrap();
    close(perfect.report.correlations[0], 1.0, 1e-10);
    assert!(perfect.report.tests[0].chi_square.is_none());
}

#[test]
fn correspondence_preserves_chi_square_geometry_and_independence() {
    let data = vec![vec![12., 3., 5.], vec![4., 15., 8.], vec![7., 6., 11.]];
    let fit = correspondence(&data, 2, &control()).unwrap();
    close(fit.report.eigenvalues[0], 0.18284414911983587, 1e-10);
    close(fit.report.eigenvalues[1], 0.034122772511940895, 1e-10);
    close(fit.report.chi_square, 15.404651435856145, 1e-10);
    for j in 0..2 {
        let weighted = fit
            .row_coordinates
            .iter()
            .zip(&fit.report.row_masses)
            .map(|(row, m)| row[j] * row[j] * m)
            .sum::<f64>();
        close(weighted, fit.report.eigenvalues[j], 1e-10);
    }
    let independent = correspondence(&[vec![1., 2.], vec![2., 4.]], 1, &control()).unwrap();
    close(independent.report.total_inertia, 0.0, 1e-10);
    assert!(independent.report.inertia_proportions.is_none());
    assert!(
        independent
            .row_coordinates
            .iter()
            .flatten()
            .all(|&v| v == 0.0)
    );
}

#[test]
fn discriminant_models_predict_unseen_rows_with_class_specific_or_pooled_covariance() {
    let x = vec![
        vec![0., 0.8, 1.4, -0.3, 4., 4.6, 5.1, 3.7],
        vec![1., 1.3, 0.2, -0.7, 4., 5.3, 3.8, 4.7],
    ];
    let groups = [0, 0, 0, 0, 1, 1, 1, 1];
    let new = vec![vec![1., 4.5, 2.8], vec![1., 4.5, 1.7]];
    for method in [DiscriminantMethod::Linear, DiscriminantMethod::Quadratic] {
        let result = discriminant(
            &x,
            &groups,
            2,
            Some(&new),
            DiscriminantOptions {
                method,
                ..Default::default()
            },
            &control(),
        )
        .unwrap();
        assert_eq!(result.predictions, vec![0, 1, 0]);
        close(result.report.training_accuracy, 1.0, 1e-10);
        assert_eq!(
            result.report.training_confusion,
            vec![vec![4, 0], vec![0, 4]]
        );
        let single = discriminant(
            &x,
            &groups,
            2,
            Some(&[vec![4.5], vec![4.5]]),
            DiscriminantOptions {
                method,
                ..Default::default()
            },
            &control(),
        )
        .unwrap();
        assert_eq!(single.predictions, vec![1]);
    }
    let singular = vec![x[0].clone(), x[0].clone()];
    assert!(
        discriminant(
            &singular,
            &groups,
            2,
            None,
            DiscriminantOptions::default(),
            &control()
        )
        .is_err()
    );
    assert!(
        discriminant(
            &singular,
            &groups,
            2,
            None,
            DiscriminantOptions {
                shrinkage: 0.2,
                ..Default::default()
            },
            &control()
        )
        .is_ok()
    );
}

#[test]
fn rda_variance_partition_and_permutation_seed_match_their_contracts() {
    let y = vec![
        vec![3., 2., 4., 6., 5., 8., 9., 7.],
        vec![1., 3., 2., 5., 3., 7., 5., 8.],
    ];
    let x = vec![(0..8).map(|i| i as f64).collect()];
    let options = RdaOptions {
        components: 1,
        standardize: false,
        permutations: 39,
        seed: 73,
    };
    let result = rda(&y, &x, options, &control()).unwrap();
    close(result.report.total_inertia, 11.928571428571429, 1e-10);
    close(result.report.constrained_inertia, 9.064625850340144, 1e-10);
    close(result.report.residual_inertia, 2.863945578231292, 1e-10);
    close(result.report.r_squared, 0.7599087539207307, 1e-10);
    close(result.report.adjusted_r_squared, 0.7198935462408524, 1e-10);
    close(
        result.report.f_statistic.unwrap(),
        18.990498812351564,
        1e-10,
    );
    let repeated = rda(&y, &x, options, &control()).unwrap();
    assert_eq!(
        result.report.permutation_p_value,
        repeated.report.permutation_p_value
    );
    assert!((0.025..=1.0).contains(&result.report.permutation_p_value.unwrap()));
    let disabled = rda(
        &y,
        &x,
        RdaOptions {
            permutations: 0,
            ..options
        },
        &control(),
    )
    .unwrap();
    assert!(disabled.report.permutation_p_value.is_none());
    let perfect = rda(
        &x,
        &x,
        RdaOptions {
            permutations: 0,
            ..options
        },
        &control(),
    )
    .unwrap();
    close(perfect.report.r_squared, 1.0, 1e-10);
    assert!(perfect.report.f_statistic.is_none());
}

#[test]
fn classical_mds_recovers_euclidean_distances_and_reports_non_euclidean_inertia() {
    let points = vec![vec![0., 1., 0., 2.], vec![0., 0., 2., 2.]];
    let fit = mds(&points, MdsOptions::default(), &control()).unwrap();
    close(fit.report.stress, 0.0, 1e-10);
    assert_eq!(fit.report.positive_rank, 2);
    let mut distances = vec![vec![0.; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            let original = points
                .iter()
                .map(|column| (column[i] - column[j]).powi(2))
                .sum::<f64>()
                .sqrt();
            let estimated = (0..2)
                .map(|k| (fit.coordinates[i][k] - fit.coordinates[j][k]).powi(2))
                .sum::<f64>()
                .sqrt();
            close(original, estimated, 1e-10);
            distances[j][i] = original;
        }
    }
    let from_distances = mds(
        &distances,
        MdsOptions {
            input: MdsInput::DissimilarityMatrix,
            ..Default::default()
        },
        &control(),
    )
    .unwrap();
    close(from_distances.report.stress, 0.0, 1e-10);
    let non_euclidean = mds(
        &[vec![0., 1., 1.], vec![1., 0., 3.], vec![1., 3., 0.]],
        MdsOptions {
            components: 2,
            input: MdsInput::DissimilarityMatrix,
            standardize: false,
        },
        &control(),
    )
    .unwrap();
    assert_eq!(non_euclidean.report.negative_eigenvalues, 1);
    assert!(non_euclidean.report.negative_inertia > 0.0);
    assert!(non_euclidean.report.stress > 0.0);
}

#[test]
fn multivariate_designs_reject_undefined_shapes_nonfinite_values_and_unsupported_dimensions() {
    let data = columns();
    assert!(
        pca(
            &[vec![1.; 10], data[1].clone()],
            PcaOptions::default(),
            &control()
        )
        .is_err()
    );
    assert!(
        pca(
            &data,
            PcaOptions {
                components: 4,
                ..Default::default()
            },
            &control()
        )
        .is_err()
    );
    let mut nonfinite = data.clone();
    nonfinite[0][0] = f64::INFINITY;
    assert!(pca(&nonfinite, PcaOptions::default(), &control()).is_err());
    assert!(canonical_correlation(&data[..2], &[data[0][..8].to_vec()], 1, &control()).is_err());
    assert!(
        exploratory_factor(
            &data,
            FactorOptions {
                factors: 2,
                ..Default::default()
            },
            &control()
        )
        .is_err()
    );
    assert!(correspondence(&[vec![1., 2.], vec![0., 0.]], 1, &control()).is_err());
    assert!(
        mds(
            &[vec![0., 1.], vec![2., 0.]],
            MdsOptions {
                components: 1,
                input: MdsInput::DissimilarityMatrix,
                standardize: false
            },
            &control()
        )
        .is_err()
    );
    assert!(
        rda(
            &data,
            &[data[0].clone(), data[0].clone()],
            RdaOptions {
                permutations: 0,
                ..Default::default()
            },
            &control()
        )
        .is_err()
    );
}

#[test]
fn all_multivariate_entries_preserve_cancellation_and_deadline_errors() {
    let data = columns();
    let cancelled = control();
    cancelled.cancellation.cancel();
    let expired = ScientificExecutionControl {
        deadline: Instant::now(),
        ..control()
    };
    for (control, expected) in [
        (cancelled, ScientificComputationError::Cancelled),
        (expired, ScientificComputationError::DeadlineExceeded),
    ] {
        assert_eq!(
            pca(&data, PcaOptions::default(), &control).unwrap_err(),
            expected
        );
        assert_eq!(
            exploratory_factor(&data, FactorOptions::default(), &control).unwrap_err(),
            expected
        );
        assert_eq!(
            canonical_correlation(&data[..1], &data[1..], 1, &control).unwrap_err(),
            expected
        );
        assert_eq!(correspondence(&data, 1, &control).unwrap_err(), expected);
        assert_eq!(
            discriminant(
                &data,
                &[0; 10],
                2,
                None,
                DiscriminantOptions::default(),
                &control
            )
            .unwrap_err(),
            expected
        );
        assert_eq!(
            rda(&data, &data[..1], RdaOptions::default(), &control).unwrap_err(),
            expected
        );
        assert_eq!(
            mds(&data, MdsOptions::default(), &control).unwrap_err(),
            expected
        );
    }
}

#[test]
fn scale_limits_multivariate_retains_wide_axes_and_large_distance_geometry() {
    let n = 64usize;
    let columns = (1..=20)
        .map(|j| {
            (0..n)
                .map(|i| {
                    if (i & j).count_ones().is_multiple_of(2) {
                        1.0
                    } else {
                        -1.0
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let fit = pca(
        &columns,
        PcaOptions {
            components: 20,
            standardize: true,
        },
        &control(),
    )
    .unwrap();
    assert_eq!(fit.report.rank, 20);
    assert_eq!(fit.coordinates[0].len(), 20);
    close(fit.report.retained_variance_ratio, 1.0, 1e-10);
    let x = (0..600).map(|i| i as f64 / 10.0).collect::<Vec<_>>();
    let result = mds(
        &[x],
        MdsOptions {
            components: 1,
            ..Default::default()
        },
        &control(),
    )
    .unwrap();
    assert_eq!(result.coordinates.len(), 600);
    close(
        (result.coordinates[0][0] - result.coordinates[599][0]).abs(),
        59.9,
        1e-8,
    );
    assert!(result.report.stress < 1e-8);
}
