use super::common::*;
use yss_sci_contract::{execution::*, multivariate::*};
use yss_sci_linalg::{Mat, MatrixExt, Solve};

pub fn discriminant(
    columns: &[Vec<f64>],
    groups: &[usize],
    class_count: usize,
    prediction_columns: Option<&[Vec<f64>]>,
    options: DiscriminantOptions,
    control: &ScientificExecutionControl,
) -> Result<DiscriminantOutput> {
    let prepared = prepare(columns, true, control)?;
    let n = prepared.matrix.nrows();
    let p = columns.len();
    if groups.len() != n {
        return Err(invalid(ScientificInputViolation::ShapeMismatch));
    }
    if !(2..=MAX_DISCRIMINANT_CLASSES).contains(&class_count)
        || !options.shrinkage.is_finite()
        || !(0.0..=1.0).contains(&options.shrinkage)
    {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let mut counts = vec![0usize; class_count];
    let mut means = Mat::zeros(class_count, p);
    for (i, &group) in groups.iter().enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        if group >= class_count {
            return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
        }
        counts[group] += 1;
        for j in 0..p {
            means[(group, j)] += prepared.matrix[(i, j)];
        }
    }
    if counts.iter().any(|&count| count < 2) {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    for g in 0..class_count {
        for j in 0..p {
            means[(g, j)] /= counts[g] as f64;
        }
    }
    let mut covariances = (0..if options.method == DiscriminantMethod::Linear {
        1
    } else {
        class_count
    })
        .map(|_| Mat::zeros(p, p))
        .collect::<Vec<_>>();
    for (row, &group) in groups.iter().enumerate() {
        if row.is_multiple_of(1024) {
            control.check()?;
        }
        let index = if options.method == DiscriminantMethod::Linear {
            0
        } else {
            group
        };
        for i in 0..p {
            for j in 0..p {
                covariances[index][(i, j)] += (prepared.matrix[(row, i)] - means[(group, i)])
                    * (prepared.matrix[(row, j)] - means[(group, j)]);
            }
        }
    }
    let mut inverses = Vec::new();
    let mut log_determinants = Vec::new();
    for (index, covariance) in covariances.iter_mut().enumerate() {
        control.check()?;
        let df = if options.method == DiscriminantMethod::Linear {
            n - class_count
        } else {
            counts[index] - 1
        };
        for i in 0..p {
            for j in 0..p {
                covariance[(i, j)] /= df as f64;
            }
        }
        let target = (0..p).map(|i| covariance[(i, i)]).sum::<f64>() / p as f64;
        for i in 0..p {
            for j in 0..p {
                covariance[(i, j)] = covariance[(i, j)] * (1.0 - options.shrinkage)
                    + if i == j {
                        options.shrinkage * target
                    } else {
                        0.0
                    };
            }
        }
        full_rank(covariance, control)?;
        let factor = covariance
            .checked_cholesky()
            .map_err(|_| invalid(ScientificInputViolation::ParameterOutOfRange))?;
        let lower = factor.lower();
        log_determinants.push(finite((0..p).map(|i| 2.0 * lower[(i, i)].ln()).sum())?);
        inverses.push(factor.solve(&Mat::identity(p, p)));
    }
    let priors = counts
        .iter()
        .map(|&count| {
            if options.priors == ClassPriors::Empirical {
                count as f64 / n as f64
            } else {
                1.0 / class_count as f64
            }
        })
        .collect::<Vec<_>>();
    let classify = |data: &Mat<f64>| -> Result<Vec<usize>> {
        let mut output = Vec::with_capacity(data.nrows());
        for row in 0..data.nrows() {
            if row.is_multiple_of(1024) {
                control.check()?;
            }
            let mut best = 0;
            let mut best_score = f64::NEG_INFINITY;
            for g in 0..class_count {
                let index = if options.method == DiscriminantMethod::Linear {
                    0
                } else {
                    g
                };
                let mut distance = 0.0;
                for i in 0..p {
                    for j in 0..p {
                        distance += (data[(row, i)] - means[(g, i)])
                            * inverses[index][(i, j)]
                            * (data[(row, j)] - means[(g, j)]);
                    }
                }
                let score = finite(-0.5 * (distance + log_determinants[index]) + priors[g].ln())?;
                if score > best_score {
                    best_score = score;
                    best = g;
                }
            }
            output.push(best);
        }
        Ok(output)
    };
    let training = classify(&prepared.matrix)?;
    let mut confusion = vec![vec![0usize; class_count]; class_count];
    let mut correct = 0;
    for (&actual, &predicted) in groups.iter().zip(&training) {
        confusion[actual][predicted] += 1;
        correct += usize::from(actual == predicted);
    }
    let predictions = if let Some(columns) = prediction_columns {
        if columns.len() != p {
            return Err(invalid(ScientificInputViolation::ShapeMismatch));
        }
        let count = validate(columns, MAX_MULTIVARIATE_VARIABLES, control)?;
        let mut data = Mat::zeros(count, p);
        for i in 0..count {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            for j in 0..p {
                data[(i, j)] = finite((columns[j][i] - prepared.means[j]) / prepared.scales[j])?;
            }
        }
        classify(&data)?
    } else {
        training
    };
    let class_means = (0..class_count)
        .map(|g| {
            (0..p)
                .map(|j| finite(means[(g, j)] * prepared.scales[j] + prepared.means[j]))
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<Vec<_>>>()?;
    let report = DiscriminantReport {
        method: options.method,
        observations: n,
        prediction_observations: predictions.len(),
        variables: p,
        classes: (0..class_count).collect(),
        class_counts: counts,
        class_priors: priors,
        class_means,
        shrinkage: options.shrinkage,
        training_accuracy: correct as f64 / n as f64,
        training_confusion: confusion,
    };
    Ok(DiscriminantOutput {
        report,
        predictions,
    })
}
