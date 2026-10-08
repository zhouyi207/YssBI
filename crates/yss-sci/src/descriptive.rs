//! Inequality measures for individual observations or population-weighted group means.
use std::collections::BTreeMap;
mod quantiles;
pub(crate) use quantiles::quantile_sorted;
use yss_sci_contract::descriptive::{DagumGroup, DagumPair, DagumResult, GiniResult};
use yss_sci_contract::execution::{
    ScientificComputationError as Error, ScientificExecutionControl, ScientificInputViolation,
};

/// Empirical Gini: sum of all absolute pairwise differences / (2 n² mean).
pub fn gini(values: &[f64], control: &ScientificExecutionControl) -> Result<GiniResult, Error> {
    let (sorted, scale) = normalized_values(values, control)?;
    let (mean, gini) = sorted_statistics(&sorted, control)?;
    Ok(GiniResult {
        gini: gini.ok_or(Error::ComputationFailed)?,
        mean: mean * scale,
        observations: values.len(),
    })
}

/// Dagum's within, net between and transvariation decomposition, with equal observation weights.
pub fn dagum_gini(
    values: &[f64],
    groups: &[usize],
    control: &ScientificExecutionControl,
) -> Result<DagumResult, Error> {
    control.check()?;
    if values.len() != groups.len() {
        return Err(Error::InvalidInput {
            violation: ScientificInputViolation::ShapeMismatch,
        });
    }
    let (sorted, scale) = normalized_values(values, control)?;
    let (mean, total_gini) = sorted_statistics(&sorted, control)?;
    let total_gini = total_gini.ok_or(Error::ComputationFailed)?;
    let mut samples = BTreeMap::<usize, Vec<f64>>::new();
    for (i, (&value, &group)) in values.iter().zip(groups).enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        samples.entry(group).or_default().push(value);
    }
    let n = values.len() as f64;
    let mut within = Sum::default();
    let mut group_rows = Vec::with_capacity(samples.len());
    let mut means = Vec::with_capacity(samples.len());
    let mut scales = Vec::with_capacity(samples.len());
    for (&group, sample) in &mut samples {
        control.check()?;
        sample.sort_unstable_by(f64::total_cmp);
        // Local scaling keeps a small positive subgroup meaningful beside a much larger one.
        let group_scale = sample.last().copied().unwrap_or(0.0);
        if group_scale > 0.0 {
            for (index, value) in sample.iter_mut().enumerate() {
                if index % 1024 == 0 {
                    control.check()?;
                }
                *value /= group_scale;
            }
        }
        let (group_mean, group_gini) = sorted_statistics(sample, control)?;
        let population_share = sample.len() as f64 / n;
        let income_share = population_share * (group_mean / mean) * (group_scale / scale);
        let within_contribution = population_share * income_share * group_gini.unwrap_or(0.0);
        within.add(within_contribution);
        means.push(group_mean);
        scales.push(group_scale);
        group_rows.push(DagumGroup {
            group,
            observations: sample.len(),
            mean: group_mean * group_scale,
            gini: group_gini,
            population_share,
            income_share,
            within_contribution,
        });
    }
    let samples = samples.into_values().collect::<Vec<_>>();
    let mut pairs = Vec::with_capacity(samples.len() * (samples.len() - 1) / 2);
    let mut between = Sum::default();
    let mut transvariation = Sum::default();
    for a in 0..samples.len() {
        for b in 0..a {
            let pair_scale = scales[a].max(scales[b]);
            let factors = if pair_scale > 0.0 {
                [scales[a] / pair_scale, scales[b] / pair_scale]
            } else {
                [0.0, 0.0]
            };
            let (forward, reverse) = cross_distances(&samples[a], &samples[b], factors, control)?;
            let distance = forward + reverse;
            let economic_distance = if distance > 0.0 {
                Some(unit_interval((forward - reverse).abs() / distance)?)
            } else {
                None
            };
            let pair_mean = means[a] * factors[0] + means[b] * factors[1];
            let pair_gini = if pair_mean > 0.0 {
                Some(unit_interval(distance / pair_mean)?)
            } else {
                None
            };
            let gross = group_rows[a].population_share
                * group_rows[b].population_share
                * (distance / mean)
                * (pair_scale / scale);
            let net = gross * economic_distance.unwrap_or(0.0);
            let overlap = (gross - net).max(0.0);
            between.add(net);
            transvariation.add(overlap);
            pairs.push(DagumPair {
                group_a: group_rows[a].group,
                group_b: group_rows[b].group,
                gini: pair_gini,
                economic_distance,
                between_contribution: net,
                transvariation_contribution: overlap,
            });
        }
    }
    control.check()?;
    if (within.total + between.total + transvariation.total - total_gini).abs() > 1e-10 {
        return Err(Error::ComputationFailed);
    }
    let share = |component| {
        if total_gini > 0.0 {
            Some(component / total_gini)
        } else {
            None
        }
    };
    Ok(DagumResult {
        gini: total_gini,
        mean: mean * scale,
        observations: values.len(),
        within: within.total,
        between: between.total,
        transvariation: transvariation.total,
        within_share: share(within.total),
        between_share: share(between.total),
        transvariation_share: share(transvariation.total),
        groups: group_rows,
        pairs,
    })
}

fn normalized_values(
    values: &[f64],
    control: &ScientificExecutionControl,
) -> Result<(Vec<f64>, f64), Error> {
    control.check()?;
    if values.is_empty() {
        return Err(Error::InvalidInput {
            violation: ScientificInputViolation::EmptyInput,
        });
    }
    let mut scale = 0.0_f64;
    for (i, &value) in values.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        if !value.is_finite() {
            return Err(Error::InvalidInput {
                violation: ScientificInputViolation::NonFiniteInput,
            });
        }
        if value < 0.0 {
            return Err(Error::InvalidInput {
                violation: ScientificInputViolation::DataOutOfRange,
            });
        }
        scale = scale.max(value);
    }
    if scale == 0.0 {
        return Err(Error::InvalidInput {
            violation: ScientificInputViolation::DataOutOfRange,
        });
    }
    let mut sorted = Vec::new();
    sorted
        .try_reserve_exact(values.len())
        .map_err(|_| Error::ComputationFailed)?;
    for (i, &value) in values.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        sorted.push(value / scale);
    }
    sorted.sort_unstable_by(f64::total_cmp);
    control.check()?;
    Ok((sorted, scale))
}

fn sorted_statistics(
    sorted: &[f64],
    control: &ScientificExecutionControl,
) -> Result<(f64, Option<f64>), Error> {
    let n = sorted.len() as f64;
    let mut total = Sum::default();
    let mut differences = Sum::default();
    for (i, &value) in sorted.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        total.add(value);
        if i > 0 {
            let p = i as f64 / n;
            // Sum positive adjacent gaps instead of subtracting large weighted rank sums.
            differences.add(p * (1.0 - p) * (value - sorted[i - 1]));
        }
    }
    let mean = total.total / n;
    Ok((
        mean,
        if mean > 0.0 {
            Some(unit_interval(differences.total / mean)?)
        } else {
            None
        },
    ))
}

fn unit_interval(value: f64) -> Result<f64, Error> {
    if !value.is_finite() || !(-1e-12..=1.0 + 1e-12).contains(&value) {
        return Err(Error::ComputationFailed);
    }
    Ok(value.clamp(0.0, 1.0))
}

fn cross_distances(
    a: &[f64],
    b: &[f64],
    factors: [f64; 2],
    control: &ScientificExecutionControl,
) -> Result<(f64, f64), Error> {
    let (mut i, mut j, mut previous) = (0usize, 0usize, 0.0);
    let mut next_check = 0;
    let mut forward = Sum::default();
    let mut reverse = Sum::default();
    while i < a.len() || j < b.len() {
        if i + j >= next_check {
            control.check()?;
            next_check = i + j + 1024;
        }
        let x = a.get(i).map(|x| x * factors[0]);
        let y = b.get(j).map(|y| y * factors[1]);
        let next = match (x, y) {
            (Some(x), Some(y)) => x.min(y),
            (Some(x), None) | (None, Some(x)) => x,
            (None, None) => unreachable!(),
        };
        let p = i as f64 / a.len() as f64;
        let q = j as f64 / b.len() as f64;
        // Integrate the two directed gaps separately, avoiding cancellation in group means.
        forward.add(p * (1.0 - q) * (next - previous));
        reverse.add(q * (1.0 - p) * (next - previous));
        if x == Some(next) {
            i += 1;
        }
        if y == Some(next) {
            j += 1;
        }
        previous = next;
    }
    control.check()?;
    Ok((forward.total, reverse.total))
}

/// Theil T with natural logarithms. Weights are population counts or shares;
/// absent weights give each observation equal mass. Zero weights contribute nothing.
pub fn theil_t(
    values: &[f64],
    weights: Option<&[f64]>,
    control: &ScientificExecutionControl,
) -> Result<f64, Error> {
    control.check()?;
    let invalid = |violation| Error::InvalidInput { violation };
    if values.is_empty() {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    if weights.is_some_and(|weights| weights.len() != values.len()) {
        return Err(invalid(ScientificInputViolation::ShapeMismatch));
    }
    let weight = |i: usize| weights.map_or(1.0, |weights| weights[i]);
    let mut max_log_weight = f64::NEG_INFINITY;
    let mut max_log_income = f64::NEG_INFINITY;
    for (i, &value) in values.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        let weight = weight(i);
        if !value.is_finite() || !weight.is_finite() {
            return Err(invalid(ScientificInputViolation::NonFiniteInput));
        }
        if value < 0.0 || weight < 0.0 {
            return Err(invalid(ScientificInputViolation::DataOutOfRange));
        }
        if weight > 0.0 {
            let log_weight = weight.ln();
            max_log_weight = max_log_weight.max(log_weight);
            if value > 0.0 {
                max_log_income = max_log_income.max(log_weight + value.ln());
            }
        }
    }
    if !max_log_income.is_finite() {
        return Err(invalid(ScientificInputViolation::DataOutOfRange));
    }

    // Normalize population and income separately in log space: both w*x and
    // their sums can overflow even though the dimensionless index is finite.
    let mut population = Sum::default();
    let mut income = Sum::default();
    for (i, &value) in values.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        let weight = weight(i);
        if weight > 0.0 {
            let log_weight = weight.ln();
            population.add((log_weight - max_log_weight).exp());
            if value > 0.0 {
                income.add((log_weight + value.ln() - max_log_income).exp());
            }
        }
    }
    let log_population = population.total.ln();
    let log_income = income.total.ln();
    let mut index = Sum::default();
    for (i, &value) in values.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        let weight = weight(i);
        if weight > 0.0 && value > 0.0 {
            let log_weight = weight.ln();
            let log_p = (log_weight - max_log_weight) - log_population;
            let log_q = (log_weight + value.ln() - max_log_income) - log_income;
            index.add(log_q.exp() * (log_q - log_p));
        }
    }
    control.check()?;
    if !index.total.is_finite() || index.total < -1e-12 {
        return Err(Error::ComputationFailed);
    }
    // The exact index is nonnegative; absorb roundoff at equality.
    Ok(index.total.max(0.0))
}

#[derive(Default)]
struct Sum {
    total: f64,
    correction: f64,
}

impl Sum {
    fn add(&mut self, value: f64) {
        let adjusted = value - self.correction;
        let next = self.total + adjusted;
        self.correction = (next - self.total) - adjusted;
        self.total = next;
    }
}

#[cfg(test)]
mod tests;
