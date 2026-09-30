use super::common::*;
use rand::{SeedableRng, rngs::StdRng, seq::SliceRandom};
use yss_sci_contract::{execution::*, multivariate::*};
use yss_sci_linalg::{Mat, MatrixExt, Solve, Svd, matrix_rank};

pub fn pca(
    columns: &[Vec<f64>],
    options: PcaOptions,
    control: &ScientificExecutionControl,
) -> Result<OrdinationOutput<PcaReport>> {
    let prepared = prepare(columns, options.standardize, control)?;
    let n = prepared.matrix.nrows();
    let p = columns.len();
    check_components(options.components, p.min(n - 1))?;
    let eigen = spectrum(&covariance(&prepared.matrix), true, control)?;
    let total = eigen.values.iter().sum::<f64>();
    if total <= 0.0 {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let weights = eigen.vectors.subcols(0, options.components).to_owned();
    let scores = &prepared.matrix * &weights;
    let proportions = eigen.values.iter().map(|x| x / total).collect::<Vec<_>>();
    let mut cumulative = 0.0;
    let cumulative_variance_ratio = proportions
        .iter()
        .map(|x| {
            cumulative += x;
            cumulative
        })
        .collect();
    let report = PcaReport {
        method: "pca".into(),
        observations: n,
        variables: p,
        components: options.components,
        standardized: options.standardize,
        rank: eigen.rank,
        means: prepared.means,
        scales: prepared.scales,
        weights: rows(&weights, 1.0, control)?,
        eigenvalues: eigen
            .values
            .iter()
            .map(|&x| rescale_variance(x, prepared.global_scale))
            .collect::<Result<_>>()?,
        retained_variance_ratio: proportions[..options.components].iter().sum(),
        explained_variance_ratio: proportions,
        cumulative_variance_ratio,
    };
    Ok(OrdinationOutput {
        report,
        coordinates: rows(&scores, prepared.global_scale, control)?,
    })
}

/// Simple CA of nonnegative weights; rows and columns are table categories.
pub fn correspondence(
    columns: &[Vec<f64>],
    components: usize,
    control: &ScientificExecutionControl,
) -> Result<CorrespondenceOutput> {
    let r = validate(columns, MAX_CORRESPONDENCE_CATEGORIES, control)?;
    let c = columns.len();
    if r > MAX_CORRESPONDENCE_CATEGORIES || c < 2 {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    check_components(components, (r - 1).min(c - 1))?;
    if columns.iter().flatten().any(|&x| x < 0.0) {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let scale = columns.iter().flatten().copied().fold(0.0, f64::max);
    if scale == 0.0 {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let total = columns.iter().flatten().map(|x| x / scale).sum::<f64>();
    let total_weight = finite(total * scale)?;
    let probabilities = Mat::from_fn(r, c, |i, j| columns[j][i] / scale / total);
    let rm = (0..r)
        .map(|i| (0..c).map(|j| probabilities[(i, j)]).sum::<f64>())
        .collect::<Vec<_>>();
    let cm = (0..c)
        .map(|j| (0..r).map(|i| probabilities[(i, j)]).sum::<f64>())
        .collect::<Vec<_>>();
    if rm.iter().chain(&cm).any(|&x| x <= 0.0) {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let standardized = Mat::from_fn(r, c, |i, j| {
        (probabilities[(i, j)] - rm[i] * cm[j]) / (rm[i] * cm[j]).sqrt()
    });
    control.check()?;
    let svd = Svd::factor(standardized.as_ref()).map_err(|_| failed())?;
    control.check()?;
    let axis_count = (r - 1).min(c - 1);
    let mut eigenvalues = svd
        .values()
        .iter()
        .take(axis_count)
        .map(|x| x * x)
        .collect::<Vec<_>>();
    let mut inertia = eigenvalues.iter().sum::<f64>();
    if inertia <= 64.0 * f64::EPSILON.powi(2) * r.max(c) as f64 {
        inertia = 0.0;
        eigenvalues.fill(0.0);
    }
    let mut row_coordinates = Mat::from_fn(r, components, |i, j| {
        svd.left_vectors()[(i, j)] * eigenvalues[j].sqrt() / rm[i].sqrt()
    });
    let mut column_coordinates = Mat::from_fn(c, components, |i, j| {
        svd.right_vectors()[(i, j)] * eigenvalues[j].sqrt() / cm[i].sqrt()
    });
    for j in 0..components {
        let pivot = (0..r)
            .max_by(|&a, &b| {
                row_coordinates[(a, j)]
                    .abs()
                    .total_cmp(&row_coordinates[(b, j)].abs())
            })
            .unwrap();
        if row_coordinates[(pivot, j)] < 0.0 {
            for i in 0..r {
                row_coordinates[(i, j)] *= -1.0;
            }
            for i in 0..c {
                column_coordinates[(i, j)] *= -1.0;
            }
        }
    }
    let ratios = if inertia > 0.0 {
        Some(eigenvalues.iter().map(|x| x / inertia).collect::<Vec<_>>())
    } else {
        None
    };
    let report = CorrespondenceReport {
        method: "simple_correspondence".into(),
        rows: r,
        columns: c,
        components,
        total_weight,
        total_inertia: inertia,
        chi_square: finite(total_weight * inertia)?,
        row_masses: rm,
        column_masses: cm,
        eigenvalues,
        retained_inertia_proportion: ratios
            .as_ref()
            .map(|values| values[..components].iter().sum()),
        inertia_proportions: ratios,
    };
    Ok(CorrespondenceOutput {
        report,
        row_coordinates: rows(&row_coordinates, 1.0, control)?,
        column_coordinates: rows(&column_coordinates, 1.0, control)?,
    })
}

/// RDA is centered multivariate least squares followed by PCA of fitted responses.
pub fn rda(
    responses: &[Vec<f64>],
    constraints: &[Vec<f64>],
    options: RdaOptions,
    control: &ScientificExecutionControl,
) -> Result<OrdinationOutput<RdaReport>> {
    let y = prepare(responses, options.standardize, control)?;
    let x = prepare(constraints, true, control)?;
    let n = y.matrix.nrows();
    let p = responses.len();
    let r = constraints.len();
    if x.matrix.nrows() != n {
        return Err(invalid(ScientificInputViolation::ShapeMismatch));
    }
    if n <= r + 1 || options.permutations > 9999 {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    check_components(options.components, p.min(r))?;
    if matrix_rank(x.matrix.as_ref()).map_err(|_| failed())?.0 != r {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let cross = x.matrix.transpose() * &x.matrix;
    let factor = cross.checked_cholesky().map_err(|_| failed())?;
    let beta = factor.solve(&(x.matrix.transpose() * &y.matrix));
    let fitted = &x.matrix * &beta;
    let residual = Mat::from_fn(n, p, |i, j| y.matrix[(i, j)] - fitted[(i, j)]);
    let fitted_spectrum = spectrum(&covariance(&fitted), true, control)?;
    let residual_spectrum = spectrum(&covariance(&residual), true, control)?;
    let total = (0..n)
        .map(|i| (0..p).map(|j| y.matrix[(i, j)].powi(2)).sum::<f64>())
        .sum::<f64>();
    let explained = (0..n)
        .map(|i| (0..p).map(|j| fitted[(i, j)].powi(2)).sum::<f64>())
        .sum::<f64>();
    let error = (0..n)
        .map(|i| (0..p).map(|j| residual[(i, j)].powi(2)).sum::<f64>())
        .sum::<f64>();
    if total <= 0.0 {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let f_statistic = if error > total * (n.max(p) as f64 * f64::EPSILON).powi(2) {
        Some(finite(
            (explained / r as f64) / (error / (n - r - 1) as f64),
        )?)
    } else {
        None
    };
    let mut permutation_p_value = None;
    if let Some(observed) = f_statistic.filter(|_| options.permutations > 0) {
        let mut rng = StdRng::seed_from_u64(options.seed);
        let mut order = (0..n).collect::<Vec<_>>();
        let mut exceedances = 0;
        for _ in 0..options.permutations {
            control.check()?;
            order.shuffle(&mut rng);
            let permuted = Mat::from_fn(n, p, |i, j| y.matrix[(order[i], j)]);
            let coefficients = factor.solve(&(x.matrix.transpose() * &permuted));
            let projected = &x.matrix * &coefficients;
            let mut ss_fitted = 0.0;
            let mut ss_residual = 0.0;
            for i in 0..n {
                if i.is_multiple_of(1024) {
                    control.check()?;
                }
                for j in 0..p {
                    ss_fitted += projected[(i, j)].powi(2);
                    ss_residual += (permuted[(i, j)] - projected[(i, j)]).powi(2);
                }
            }
            if ss_residual == 0.0
                || (ss_fitted / r as f64) / (ss_residual / (n - r - 1) as f64)
                    >= observed * (1.0 - 1e-12)
            {
                exceedances += 1;
            }
        }
        permutation_p_value = Some((exceedances + 1) as f64 / (options.permutations + 1) as f64);
    }
    let weights = fitted_spectrum
        .vectors
        .subcols(0, options.components)
        .to_owned();
    let coordinates = &fitted * &weights;
    let r_squared = (explained / total).clamp(0.0, 1.0);
    let report = RdaReport {
        method: "rda".into(),
        observations: n,
        response_variables: p,
        constraints: r,
        components: options.components,
        standardized: options.standardize,
        means: y.means,
        scales: y.scales,
        response_weights: rows(&weights, 1.0, control)?,
        total_inertia: rescale_variance(total / (n - 1) as f64, y.global_scale)?,
        constrained_inertia: rescale_variance(explained / (n - 1) as f64, y.global_scale)?,
        residual_inertia: rescale_variance(error / (n - 1) as f64, y.global_scale)?,
        constrained_eigenvalues: fitted_spectrum
            .values
            .iter()
            .map(|&v| rescale_variance(v, y.global_scale))
            .collect::<Result<_>>()?,
        residual_eigenvalues: residual_spectrum
            .values
            .iter()
            .map(|&v| rescale_variance(v, y.global_scale))
            .collect::<Result<_>>()?,
        r_squared,
        adjusted_r_squared: finite(1.0 - (1.0 - r_squared) * (n - 1) as f64 / (n - r - 1) as f64)?,
        f_statistic,
        df_numerator: r,
        df_denominator: n - r - 1,
        permutation_p_value,
        permutations: if permutation_p_value.is_some() {
            options.permutations
        } else {
            0
        },
        seed: options.seed,
    };
    Ok(OrdinationOutput {
        report,
        coordinates: rows(&coordinates, y.global_scale, control)?,
    })
}

pub fn mds(
    columns: &[Vec<f64>],
    options: MdsOptions,
    control: &ScientificExecutionControl,
) -> Result<OrdinationOutput<MdsReport>> {
    let n = validate(columns, MAX_MDS_OBSERVATIONS, control)?;
    if n > MAX_MDS_OBSERVATIONS {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    check_components(options.components, n - 1)?;
    let (mut distances, scale) = match options.input {
        MdsInput::Observations => {
            let prepared = prepare(columns, options.standardize, control)?;
            let mut distances = Mat::zeros(n, n);
            for i in 0..n {
                control.check()?;
                for j in 0..i {
                    let value = (0..columns.len())
                        .map(|k| (prepared.matrix[(i, k)] - prepared.matrix[(j, k)]).powi(2))
                        .sum::<f64>()
                        .sqrt();
                    distances[(i, j)] = value;
                    distances[(j, i)] = value;
                }
            }
            (distances, prepared.global_scale)
        }
        MdsInput::DissimilarityMatrix => {
            if columns.len() != n || options.standardize {
                return Err(invalid(ScientificInputViolation::ShapeMismatch));
            }
            let scale = columns.iter().flatten().copied().fold(0.0, f64::max);
            if scale <= 0.0 || columns.iter().flatten().any(|&v| v < 0.0) {
                return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
            }
            let distances = Mat::from_fn(n, n, |i, j| columns[j][i] / scale);
            for i in 0..n {
                control.check()?;
                if distances[(i, i)].abs() > 1e-12 {
                    return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
                }
                for j in 0..i {
                    if (distances[(i, j)] - distances[(j, i)]).abs() > 1e-12 {
                        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
                    }
                }
            }
            (
                Mat::from_fn(n, n, |i, j| {
                    if i == j {
                        0.0
                    } else {
                        (distances[(i, j)] + distances[(j, i)]) / 2.0
                    }
                }),
                scale,
            )
        }
    };
    // A second common scale bounds distance squares even with many raw variables.
    let max_distance = (0..n)
        .map(|i| (0..n).map(|j| distances[(i, j)]).fold(0.0, f64::max))
        .fold(0.0, f64::max);
    if max_distance == 0.0 {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let scale = finite(scale * max_distance)?;
    for i in 0..n {
        for j in 0..n {
            distances[(i, j)] /= max_distance;
        }
    }
    let means = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| distances[(i, j)].powi(2) / n as f64)
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    let overall = means.iter().sum::<f64>() / n as f64;
    let gram = Mat::from_fn(n, n, |i, j| {
        -0.5 * (distances[(i, j)].powi(2) - means[i] - means[j] + overall)
    });
    let eigen = spectrum(&gram, false, control)?;
    let positive = eigen.values.iter().filter(|&&v| v > 0.0).sum::<f64>();
    let negative = -eigen.values.iter().filter(|&&v| v < 0.0).sum::<f64>();
    if positive <= 0.0 {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let coordinates = Mat::from_fn(n, options.components, |i, j| {
        eigen.vectors[(i, j)] * eigen.values[j].max(0.0).sqrt()
    });
    let mut residual = 0.0;
    let mut original = 0.0;
    for i in 0..n {
        control.check()?;
        for j in 0..i {
            let estimated = (0..options.components)
                .map(|k| (coordinates[(i, k)] - coordinates[(j, k)]).powi(2))
                .sum::<f64>()
                .sqrt();
            residual += (estimated - distances[(i, j)]).powi(2);
            original += distances[(i, j)].powi(2);
        }
    }
    let retained = eigen.values[..options.components]
        .iter()
        .map(|v| v.max(0.0))
        .sum::<f64>();
    let report = MdsReport {
        method: "classical_mds".into(),
        observations: n,
        components: options.components,
        input: options.input,
        standardized: options.standardize,
        positive_rank: eigen.rank,
        negative_eigenvalues: eigen.values.iter().filter(|&&v| v < 0.0).count(),
        selected_eigenvalues: eigen.values[..options.components]
            .iter()
            .map(|&v| rescale_variance(v.max(0.0), scale))
            .collect::<Result<_>>()?,
        positive_inertia: rescale_variance(positive, scale)?,
        negative_inertia: rescale_variance(negative, scale)?,
        goodness_of_fit_positive: retained / positive,
        goodness_of_fit_absolute: retained / (positive + negative),
        stress: finite((residual / original).sqrt())?,
    };
    Ok(OrdinationOutput {
        report,
        coordinates: rows(&coordinates, scale, control)?,
    })
}
