//! Binary-treatment matching, weighting and outcome-regression estimators.
use super::common::*;
use crate::regression::models::glm;
use yss_sci_contract::causal::models::*;
use yss_sci_contract::regression::models::{GlmFamily, GlmLink, GlmOptions};

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
            let mut matched = vec![0.0; n];
            let mut matched_counts = vec![0; n];
            let mut maximum = 0.0_f64;
            // Replacement is allowed; all exact nearest-distance ties receive equal weight.
            for i in 0..n {
                control.check()?;
                let mut best = f64::INFINITY;
                for j in 0..n {
                    if j.is_multiple_of(1024) {
                        control.check()?;
                    }
                    if treatment[i] == treatment[j] {
                        continue;
                    }
                    let distance = (e[i] - e[j]).abs();
                    if distance < best {
                        best = distance;
                        matched[i] = y[j];
                        matched_counts[i] = 1;
                    } else if distance == best {
                        matched_counts[i] += 1;
                        matched[i] += (y[j] - matched[i]) / matched_counts[i] as f64;
                    }
                }
                if best > options.caliper {
                    return Err(parameter());
                }
                maximum = maximum.max(best);
            }
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
