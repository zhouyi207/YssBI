//! Binary-treatment matching, weighting and outcome-regression estimators.
use super::common::*;
use crate::regression::models::glm;
use yss_sci_contract::causal::models::*;
use yss_sci_contract::regression::models::{GlmFamily, GlmLink, GlmOptions};

#[derive(Clone, Copy, Default)]
struct OutcomeAverage {
    mean: f64,
    count: usize,
}

impl OutcomeAverage {
    fn combine(self, other: Self) -> Self {
        if self.count == 0 {
            return other;
        }
        if other.count == 0 {
            return self;
        }
        let count = self.count + other.count;
        Self {
            mean: if self.mean == other.mean {
                self.mean
            } else {
                self.mean * (self.count as f64 / count as f64)
                    + other.mean * (other.count as f64 / count as f64)
            },
            count,
        }
    }
}

fn matched_outcomes(
    y: &[f64],
    treatment: &[f64],
    scores: &[f64],
    caliper: f64,
    control: &Control,
) -> Result<(Vec<f64>, Vec<usize>, f64)> {
    let n = y.len();
    let mut cohorts = [Vec::new(), Vec::new()];
    for (i, &level) in treatment.iter().enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        cohorts[level as usize].push(i);
    }
    for indices in &mut cohorts {
        control.check()?;
        indices.sort_unstable_by(|&i, &j| scores[i].total_cmp(&scores[j]).then(i.cmp(&j)));
        control.check()?;
    }

    let mut matched = vec![0.0; n];
    let mut counts = vec![0; n];
    let mut maximum = 0.0_f64;
    for level in 0..2 {
        let candidates = &cohorts[1 - level];
        let offset = candidates.len();
        // A range-mean tree pools all ties without rescanning their outcomes or
        // subtracting prefix sums, which can lose small local outcome means.
        let mut averages = vec![OutcomeAverage::default(); 2 * offset];
        for (i, &row) in candidates.iter().enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            averages[offset + i] = OutcomeAverage {
                mean: y[row],
                count: 1,
            };
        }
        for i in (1..offset).rev() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            averages[i] = averages[2 * i].combine(averages[2 * i + 1]);
        }

        let mut previous: Option<(f64, OutcomeAverage, f64)> = None;
        for &row in &cohorts[level] {
            control.check()?;
            let score = scores[row];
            let (average, distance) = if let Some((previous_score, average, distance)) = previous
                && previous_score == score
            {
                (average, distance)
            } else {
                let split = candidates.partition_point(|&i| scores[i] < score);
                let distance = split
                    .checked_sub(1)
                    .map(|i| (score - scores[candidates[i]]).abs())
                    .into_iter()
                    .chain(candidates.get(split).map(|&i| (score - scores[i]).abs()))
                    .fold(f64::INFINITY, f64::min);
                if distance > caliper {
                    return Err(parameter());
                }
                // Subtraction can round several distinct scores to the same
                // nearest distance; retain the full interval on both sides.
                let first =
                    candidates[..split].partition_point(|&i| (score - scores[i]).abs() > distance);
                let last = split
                    + candidates[split..]
                        .partition_point(|&i| (score - scores[i]).abs() <= distance);
                let (mut left, mut right) = (offset + first, offset + last);
                let mut average = OutcomeAverage::default();
                while left < right {
                    if left % 2 == 1 {
                        average = average.combine(averages[left]);
                        left += 1;
                    }
                    if right % 2 == 1 {
                        right -= 1;
                        average = average.combine(averages[right]);
                    }
                    left /= 2;
                    right /= 2;
                }
                finite(average.mean)?;
                previous = Some((score, average, distance));
                (average, distance)
            };
            matched[row] = average.mean;
            counts[row] = average.count;
            maximum = maximum.max(distance);
        }
    }
    Ok((matched, counts, maximum))
}

pub fn estimate(
    y: &[f64],
    treatment: &[f64],
    predictors: &[Vec<f64>],
    options: TreatmentOptions,
    control: &Control,
) -> Result<TreatmentResult> {
    let mut result = point(y, treatment, predictors, options, control)?;
    if options.method == TreatmentMethod::Matching && options.bootstrap.replications != 0 {
        return Err(parameter());
    }
    let covariance = bootstrap_covariance(y.len(), 2, options.bootstrap, control, |indices| {
        let sample = |values: &[f64]| indices.iter().map(|&i| values[i]).collect::<Vec<_>>();
        let xs = predictors.iter().map(|v| sample(v)).collect::<Vec<_>>();
        let fitted = point(&sample(y), &sample(treatment), &xs, options, control)?;
        Ok(vec![fitted.ate.estimate, fitted.att.estimate])
    })?;
    if let Some(covariance) = covariance {
        let estimates = coefficient_table(
            &[result.ate.estimate, result.att.estimate],
            vec!["ATE".into(), "ATT".into()],
            Some(&covariance),
            None,
        )?;
        let convert = |index: usize| {
            let c = &estimates[index];
            TreatmentEffect {
                estimate: c.estimate,
                standard_error: c.standard_error,
                statistic: c.statistic,
                p_value: c.p_value,
                confidence_interval: c.confidence_interval,
            }
        };
        result.ate = convert(0);
        result.att = convert(1);
        result.inference = "iid_pairs_bootstrap_normal".into();
        result.bootstrap_replications = options.bootstrap.replications;
    }
    control.check()?;
    Ok(result)
}

fn point(
    y: &[f64],
    treatment: &[f64],
    predictors: &[Vec<f64>],
    options: TreatmentOptions,
    control: &Control,
) -> Result<TreatmentResult> {
    validate(y, predictors, control)?;
    validate(y, &[treatment.to_vec()], control)?;
    let n = y.len();
    let treated = binary(treatment)?;
    let scores = if options.method != TreatmentMethod::RegressionAdjustment {
        if !options.overlap.is_finite() || options.overlap <= 0.0 || options.overlap >= 0.5 {
            return Err(parameter());
        }
        let model = glm(
            treatment,
            predictors,
            GlmOptions {
                constant: true,
                family: GlmFamily::Binomial,
                link: GlmLink::Logit,
                fractional: false,
                iteration: options.iteration,
            },
            control,
        )?;
        if model
            .fitted
            .iter()
            .any(|&e| e < options.overlap || e > 1.0 - options.overlap)
        {
            return Err(parameter());
        }
        Some(model.fitted)
    } else {
        None
    };
    let outcomes = if matches!(
        options.method,
        TreatmentMethod::RegressionAdjustment | TreatmentMethod::Aipw
    ) {
        let mut predictions = [vec![], vec![]];
        for (level, prediction) in predictions.iter_mut().enumerate() {
            let indices = (0..n)
                .filter(|&i| treatment[i] == level as f64)
                .collect::<Vec<_>>();
            let response = indices.iter().map(|&i| y[i]).collect::<Vec<_>>();
            let xs = predictors
                .iter()
                .map(|x| indices.iter().map(|&i| x[i]).collect::<Vec<_>>())
                .collect::<Vec<_>>();
            let design = Design::new(&xs, indices.len(), true, true, true, control)?;
            let (beta, _) = least_squares(&design.x, &response, None, control)?;
            let (raw, _) = design.raw(&beta, None);
            *prediction = (0..n)
                .map(|i| {
                    raw[0]
                        + predictors
                            .iter()
                            .enumerate()
                            .map(|(j, x)| raw[j + 1] * x[i])
                            .sum::<f64>()
                })
                .collect();
        }
        Some(predictions)
    } else {
        None
    };
    let mut matches = None;
    let mut counts = None;
    let mut maximum_distance = None;
    let (ate, att) = match options.method {
        TreatmentMethod::Matching => {
            if !options.caliper.is_finite() || options.caliper <= 0.0 || options.caliper > 1.0 {
                return Err(parameter());
            }
            let e = scores.as_ref().expect("propensity");
            let (matched, matched_counts, maximum) =
                matched_outcomes(y, treatment, e, options.caliper, control)?;
            let effects = (0..n)
                .map(|i| {
                    if treatment[i] == 1.0 {
                        y[i] - matched[i]
                    } else {
                        matched[i] - y[i]
                    }
                })
                .collect::<Vec<_>>();
            let att = (0..n)
                .filter(|&i| treatment[i] == 1.0)
                .map(|i| effects[i] / treated as f64)
                .sum();
            matches = Some(matched);
            counts = Some(matched_counts);
            maximum_distance = Some(maximum);
            (mean(&effects), att)
        }
        TreatmentMethod::Ipw => {
            let e = scores.as_ref().expect("propensity");
            let mut weighted_y = [0.0; 3];
            let mut weight = [0.0; 3];
            for i in 0..n {
                if i.is_multiple_of(1024) {
                    control.check()?;
                }
                let w = [
                    treatment[i] / e[i],
                    (1.0 - treatment[i]) / (1.0 - e[i]),
                    (1.0 - treatment[i]) * e[i] / (1.0 - e[i]),
                ];
                for j in 0..3 {
                    weighted_y[j] += w[j] * y[i];
                    weight[j] += w[j];
                }
            }
            let observed_treated = (0..n)
                .map(|i| treatment[i] * y[i] / treated as f64)
                .sum::<f64>();
            (
                weighted_y[0] / weight[0] - weighted_y[1] / weight[1],
                observed_treated - weighted_y[2] / weight[2],
            )
        }
        TreatmentMethod::RegressionAdjustment | TreatmentMethod::Aipw => {
            let m = outcomes.as_ref().expect("outcome regressions");
            let mut ate = 0.0;
            let mut att = 0.0;
            for i in 0..n {
                if i.is_multiple_of(1024) {
                    control.check()?;
                }
                let d = treatment[i];
                let difference = m[1][i] - m[0][i];
                if let Some(e) = &scores {
                    ate += (difference + d * (y[i] - m[1][i]) / e[i]
                        - (1.0 - d) * (y[i] - m[0][i]) / (1.0 - e[i]))
                        / n as f64;
                    att += (d * (y[i] - m[0][i])
                        - (1.0 - d) * e[i] * (y[i] - m[0][i]) / (1.0 - e[i]))
                        / treated as f64;
                } else {
                    ate += difference / n as f64;
                    att += d * difference / treated as f64;
                }
            }
            (ate, att)
        }
    };
    let effect = |estimate| -> Result<TreatmentEffect> {
        Ok(TreatmentEffect {
            estimate: finite(estimate)?,
            standard_error: None,
            statistic: None,
            p_value: None,
            confidence_interval: None,
        })
    };
    Ok(TreatmentResult {
        method: options.method,
        observations: n,
        treated,
        controls: n - treated,
        ate: effect(ate)?,
        att: effect(att)?,
        inference: "unavailable".into(),
        bootstrap_replications: 0,
        propensity_scores: scores,
        potential_outcomes: outcomes,
        matched_outcomes: matches,
        match_counts: counts,
        maximum_match_distance: maximum_distance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use yss_sci_contract::execution::{ScientificCancellationToken, ScientificComputationError};

    fn control() -> Control {
        Control {
            cancellation: ScientificCancellationToken::new(),
            deadline: Instant::now() + Duration::from_secs(60),
        }
    }

    #[test]
    fn matching_keeps_all_nearest_ties_across_score_groups() {
        let y = [10.0, 2.0, 4.0, 12.0, 6.0];
        let treatment = [1.0, 0.0, 0.0, 0.0, 1.0];
        let scores = [0.5, 0.25, 0.25, 0.75, 0.125];
        let expected = [6.0, 6.0, 6.0, 10.0, 3.0];
        let expected_counts = [3, 1, 1, 1, 2];
        for order in [[0, 1, 2, 3, 4], [4, 0, 3, 2, 1]] {
            let sample = |values: &[f64]| order.map(|i| values[i]);
            let (matched, counts, maximum) = matched_outcomes(
                &sample(&y),
                &sample(&treatment),
                &sample(&scores),
                0.25,
                &control(),
            )
            .unwrap();
            for (row, &index) in order.iter().enumerate() {
                assert!((matched[row] - expected[index]).abs() < 1e-12);
                assert_eq!(counts[row], expected_counts[index]);
            }
            assert_eq!(maximum, 0.25);
        }
    }

    #[test]
    fn matching_keeps_rounded_distance_ties_and_caliper_boundary() {
        let y = [4.0, 10.0, 16.0, 20.0, 30.0];
        let treatment = [0.0, 0.0, 0.0, 1.0, 1.0];
        let scores = [1e-20, 2e-20, 3e-20, 0.5, 0.75];
        let (matched, counts, maximum) =
            matched_outcomes(&y, &treatment, &scores, 0.75, &control()).unwrap();
        assert_eq!(matched, [20.0, 20.0, 20.0, 10.0, 10.0]);
        assert_eq!(counts, [1, 1, 1, 3, 3]);
        assert_eq!(maximum, 0.75);
        assert_eq!(
            matched_outcomes(&y, &treatment, &scores, 0.75_f64.next_down(), &control())
                .unwrap_err(),
            parameter()
        );
        let cancelled = control();
        cancelled.cancellation.cancel();
        let expired = Control {
            deadline: Instant::now(),
            ..control()
        };
        for (control, expected) in [
            (cancelled, ScientificComputationError::Cancelled),
            (expired, ScientificComputationError::DeadlineExceeded),
        ] {
            assert_eq!(
                matched_outcomes(&y, &treatment, &scores, 0.75, &control).unwrap_err(),
                expected
            );
        }
    }
}
