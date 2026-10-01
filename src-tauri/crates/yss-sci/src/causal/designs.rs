//! Sharp local-linear RDD, treatment-effect heterogeneity and synthetic controls.
use super::common::*;
use yss_sci_contract::causal::models::*;

pub fn rdd(
    y: &[f64],
    running: &[f64],
    options: RddOptions,
    control: &Control,
) -> Result<RddResult> {
    validate(y, &[running.to_vec()], control)?;
    if !options.cutoff.is_finite() || !options.bandwidth.is_finite() || options.bandwidth <= 0.0 {
        return Err(parameter());
    }
    let indices = (0..y.len())
        .filter(|&i| {
            let distance = (running[i] - options.cutoff).abs();
            if options.triangular {
                distance < options.bandwidth
            } else {
                distance <= options.bandwidth
            }
        })
        .collect::<Vec<_>>();
    let x = indices
        .iter()
        .map(|&i| running[i] - options.cutoff)
        .collect::<Vec<_>>();
    let treatment = x
        .iter()
        .map(|&v| f64::from(u8::from(v >= 0.0)))
        .collect::<Vec<_>>();
    let right = binary(&treatment)?;
    let response = indices.iter().map(|&i| y[i]).collect::<Vec<_>>();
    let interaction = x
        .iter()
        .zip(&treatment)
        .map(|(x, d)| x * d)
        .collect::<Vec<_>>();
    let weights = x
        .iter()
        .map(|v| {
            if options.triangular {
                1.0 - v.abs() / options.bandwidth
            } else {
                1.0
            }
        })
        .collect::<Vec<_>>();
    let design = Design::new(
        &[treatment, x, interaction],
        indices.len(),
        true,
        true,
        true,
        control,
    )?;
    let (beta, bread) = least_squares(&design.x, &response, Some(&weights), control)?;
    let fitted = fitted(&design.x, &beta);
    let residuals = response
        .iter()
        .zip(&fitted)
        .map(|(y, f)| y - f)
        .collect::<Vec<_>>();
    let covariance = hc3(&design.x, &residuals, Some(&weights), &bread, control)?;
    let (beta, covariance) = design.raw(&beta, Some(covariance));
    let covariance = covariance.expect("covariance");
    Ok(RddResult {
        cutoff: options.cutoff,
        bandwidth: options.bandwidth,
        kernel: if options.triangular {
            "triangular"
        } else {
            "uniform"
        }
        .into(),
        left_observations: indices.len() - right,
        right_observations: right,
        coefficients: coefficient_table(
            &beta,
            [
                "intercept_left",
                "discontinuity",
                "slope_left",
                "slope_change",
            ]
            .map(String::from)
            .to_vec(),
            Some(&covariance),
            None,
        )?,
        covariance: rows(&covariance),
        rows: indices.iter().map(|i| i + 1).collect(),
        fitted,
        residuals,
    })
}

pub fn heterogeneity(
    y: &[f64],
    treatment: &[f64],
    groups: &[usize],
    predictors: &[Vec<f64>],
    control: &Control,
) -> Result<HeterogeneityResult> {
    validate(y, predictors, control)?;
    validate(y, &[treatment.to_vec()], control)?;
    binary(treatment)?;
    if groups.len() != y.len() {
        return Err(parameter());
    }
    let levels = groups
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let count = levels.len();
    if count < 2 || levels.iter().copied().ne(0..count) {
        return Err(parameter());
    }
    for group in 0..count {
        control.check()?;
        if [0.0, 1.0].iter().any(|&d| {
            !groups
                .iter()
                .zip(treatment)
                .any(|(&g, &t)| g == group && t == d)
        }) {
            return Err(parameter());
        }
    }
    let mut xs = predictors.to_vec();
    let d_index = xs.len() + 1;
    xs.push(treatment.to_vec());
    for level in 1..count {
        xs.push(
            groups
                .iter()
                .map(|&g| f64::from(u8::from(g == level)))
                .collect(),
        );
    }
    let interaction_start = xs.len() + 1;
    for level in 1..count {
        xs.push(
            groups
                .iter()
                .zip(treatment)
                .map(|(&g, &d)| if g == level { d } else { 0.0 })
                .collect(),
        );
    }
    let design = Design::new(&xs, y.len(), true, true, true, control)?;
    let (beta, bread) = least_squares(&design.x, y, None, control)?;
    let predicted = fitted(&design.x, &beta);
    let residuals = y
        .iter()
        .zip(predicted)
        .map(|(y, f)| y - f)
        .collect::<Vec<_>>();
    let covariance = hc3(&design.x, &residuals, None, &bread, control)?;
    let (beta, covariance) = design.raw(&beta, Some(covariance));
    let covariance = covariance.expect("covariance");
    let differences = Col::from_fn(count - 1, |i| beta[interaction_start + i]);
    let restricted = Mat::from_fn(count - 1, count - 1, |i, j| {
        covariance[(interaction_start + i, interaction_start + j)]
    });
    let w = inverse(&restricted)?.as_ref() * differences.as_ref();
    let statistic = differences.iter().zip(w.iter()).map(|(a, b)| a * b).sum();
    let effects = (0..count)
        .map(|g| {
            beta[d_index]
                + if g == 0 {
                    0.0
                } else {
                    beta[interaction_start + g - 1]
                }
        })
        .collect::<Vec<_>>();
    let effect_cov = Mat::from_fn(count, count, |g, h| {
        covariance[(d_index, d_index)]
            + if g > 0 {
                covariance[(interaction_start + g - 1, d_index)]
            } else {
                0.0
            }
            + if h > 0 {
                covariance[(d_index, interaction_start + h - 1)]
            } else {
                0.0
            }
            + if g > 0 && h > 0 {
                covariance[(interaction_start + g - 1, interaction_start + h - 1)]
            } else {
                0.0
            }
    });
    let mut terms = names(predictors.len(), true);
    terms.push("treatment".into());
    terms.extend((1..count).map(|g| format!("group{}", g + 1)));
    terms.extend((1..count).map(|g| format!("treatment:group{}", g + 1)));
    Ok(HeterogeneityResult {
        observations: y.len(),
        groups: (0..count).collect(),
        group_effects: coefficient_table(
            &effects,
            (1..=count).map(|g| format!("group{g}")).collect(),
            Some(&effect_cov),
            None,
        )?,
        equality_test: chi_square(statistic, count - 1)?,
        coefficients: coefficient_table(&beta, terms, Some(&covariance), None)?,
        covariance: rows(&covariance),
    })
}

fn simplex(values: &[f64]) -> Vec<f64> {
    let mut ordered = values.to_vec();
    ordered.sort_by(|a, b| b.total_cmp(a));
    let mut sum = 0.0;
    let mut threshold = 0.0;
    for (i, &value) in ordered.iter().enumerate() {
        sum += value;
        let candidate = (sum - 1.0) / (i + 1) as f64;
        if value > candidate {
            threshold = candidate;
        }
    }
    values.iter().map(|v| (v - threshold).max(0.0)).collect()
}

pub fn synthetic_control(
    y: &[f64],
    donors: &[Vec<f64>],
    options: SyntheticControlOptions,
    control: &Control,
) -> Result<SyntheticControlResult> {
    validate(y, donors, control)?;
    check_iteration(options.iteration)?;
    let pre = options.pre_periods;
    if pre == 0 || pre >= y.len() || donors.is_empty() {
        return Err(parameter());
    }
    let k = donors.len();
    let center = mean(&y[..pre]);
    let scale = std::iter::once(y)
        .chain(donors.iter().map(Vec::as_slice))
        .flat_map(|x| x[..pre].iter().map(|v| (v - center).abs()))
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let x = Mat::from_fn(pre, k, |i, j| (donors[j][i] - center) / scale);
    let target = y[..pre]
        .iter()
        .map(|v| (v - center) / scale)
        .collect::<Vec<_>>();
    let gram = Mat::from_fn(k, k, |j, l| {
        (0..pre)
            .map(|i| x[(i, j)] * x[(i, l)] / pre as f64)
            .sum::<f64>()
    });
    let rhs = (0..k)
        .map(|j| {
            (0..pre)
                .map(|i| x[(i, j)] * target[i] / pre as f64)
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    let lipschitz = 2.0 * (0..k).map(|j| gram[(j, j)]).sum::<f64>();
    let mut weights = vec![1.0 / k as f64; k];
    let mut iterations = 0;
    let mut converged = lipschitz == 0.0;
    for step in 0..options.iteration.max_iterations {
        control.check()?;
        if converged {
            break;
        }
        let gradient = (0..k)
            .map(|j| 2.0 * ((0..k).map(|l| gram[(j, l)] * weights[l]).sum::<f64>() - rhs[j]))
            .collect::<Vec<_>>();
        let gap = gradient
            .iter()
            .zip(&weights)
            .map(|(g, w)| g * w)
            .sum::<f64>()
            - gradient.iter().copied().fold(f64::INFINITY, f64::min);
        if !gap.is_finite() {
            return Err(failed());
        }
        if gap <= options.iteration.tolerance {
            converged = true;
            break;
        }
        weights = simplex(
            &weights
                .iter()
                .zip(gradient)
                .map(|(w, g)| w - g / lipschitz)
                .collect::<Vec<_>>(),
        );
        iterations = step + 1;
    }
    if !converged {
        return Err(failed());
    }
    let synthetic = (0..y.len())
        .map(|i| {
            donors
                .iter()
                .zip(&weights)
                .map(|(x, w)| x[i] * w)
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    let gaps = y
        .iter()
        .zip(&synthetic)
        .map(|(y, s)| y - s)
        .collect::<Vec<_>>();
    let rmspe = |g: &[f64]| finite((g.iter().map(|v| v * v / g.len() as f64).sum::<f64>()).sqrt());
    control.check()?;
    Ok(SyntheticControlResult {
        pre_periods: pre,
        post_periods: y.len() - pre,
        donor_weights: weights,
        post_effect: finite(mean(&gaps[pre..]))?,
        pre_rmspe: rmspe(&gaps[..pre])?,
        post_rmspe: rmspe(&gaps[pre..])?,
        synthetic,
        gaps,
        iterations,
    })
}
