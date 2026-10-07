use super::common::*;
use yss_sci_contract::survival::*;

struct RiskData<'a> {
    start: &'a [f64],
    stop: &'a [f64],
    event: &'a [f64],
    strata: Vec<(usize, Vec<usize>, Vec<f64>)>,
    x: Mat<f64>,
    ties: CoxTies,
    control: &'a Control,
}
impl RiskData<'_> {
    fn evaluate(&self, beta: &[f64]) -> Result<(f64, Vec<f64>, Mat<f64>)> {
        self.evaluate_time(beta, None)
    }

    fn evaluate_time(
        &self,
        beta: &[f64],
        only_time: Option<f64>,
    ) -> Result<(f64, Vec<f64>, Mat<f64>)> {
        let p = beta.len();
        let eta = fitted(&self.x, beta);
        let mut log_likelihood = 0.0;
        let mut score = vec![0.0; p];
        let mut information = Mat::zeros(p, p);
        for (_, indices, times) in &self.strata {
            for &t in times {
                if only_time.is_some_and(|selected| selected != t) {
                    continue;
                }
                self.control.check()?;
                let deaths = indices
                    .iter()
                    .copied()
                    .filter(|&i| self.stop[i] == t && self.event[i] == 1.0)
                    .collect::<Vec<_>>();
                let risk = indices
                    .iter()
                    .copied()
                    .filter(|&i| self.start[i] < t && t <= self.stop[i])
                    .collect::<Vec<_>>();
                let shift = risk
                    .iter()
                    .map(|&i| eta[i])
                    .fold(f64::NEG_INFINITY, f64::max);
                let (mut sum, mut death_sum) = (0.0, 0.0);
                let (mut first, mut death_first) = (vec![0.0; p], vec![0.0; p]);
                let (mut second, mut death_second) = (Mat::zeros(p, p), Mat::zeros(p, p));
                for &i in &risk {
                    self.control.check()?;
                    let w = finite((eta[i] - shift).exp())?;
                    let is_death = self.stop[i] == t && self.event[i] == 1.0;
                    sum += w;
                    if is_death {
                        death_sum += w;
                    }
                    for j in 0..p {
                        first[j] += w * self.x[(i, j)];
                        if is_death {
                            death_first[j] += w * self.x[(i, j)];
                        }
                        for k in 0..p {
                            let v = w * self.x[(i, j)] * self.x[(i, k)];
                            second[(j, k)] += v;
                            if is_death {
                                death_second[(j, k)] += v;
                            }
                        }
                    }
                }
                for &i in &deaths {
                    log_likelihood += eta[i] - shift;
                    for (j, s) in score.iter_mut().enumerate() {
                        *s += self.x[(i, j)];
                    }
                }
                for l in 0..deaths.len() {
                    let fraction = if self.ties == CoxTies::Efron {
                        l as f64 / deaths.len() as f64
                    } else {
                        0.0
                    };
                    let denominator = finite(sum - fraction * death_sum)?;
                    if denominator <= 0.0 {
                        return Err(failed());
                    }
                    log_likelihood -= denominator.ln();
                    for j in 0..p {
                        let mean = (first[j] - fraction * death_first[j]) / denominator;
                        score[j] -= mean;
                        for k in 0..p {
                            information[(j, k)] +=
                                (second[(j, k)] - fraction * death_second[(j, k)]) / denominator
                                    - mean * (first[k] - fraction * death_first[k]) / denominator;
                        }
                    }
                }
            }
        }
        Ok((finite(log_likelihood)?, score, information))
    }
}

/// Efficient score test for beta(t) = beta + gamma * g(t) in a static Cox model.
/// Event contributions reuse exactly the fit's risk-set and tied-death likelihood.
pub fn proportional_hazards(
    time: &[f64],
    event: &[f64],
    predictors: &[Vec<f64>],
    options: CoxOptions,
    transform: PhTimeTransform,
    control: &Control,
) -> Result<ProportionalHazardsResult> {
    let model = fit(time, event, predictors, options, control)?;
    let n = time.len();
    let p = predictors.len();
    let design = Design::new(predictors, n, true, true, false, control)?;
    let x = Mat::from_fn(n, p, |i, j| design.x[(i, j + 1)]);
    let beta = model
        .coefficients
        .iter()
        .zip(&design.scales)
        .map(|(b, s)| b.estimate * s)
        .collect::<Vec<_>>();
    let mut times = time
        .iter()
        .zip(event)
        .filter_map(|(&t, &e)| (e == 1.0).then_some(t))
        .collect::<Vec<_>>();
    times.sort_by(f64::total_cmp);
    times.dedup();
    if times.len() < 2 {
        return Err(parameter());
    }
    let mut sorted = time.to_vec();
    sorted.sort_by(f64::total_cmp);
    let g = |t: f64| match transform {
        PhTimeTransform::Identity => t,
        PhTimeTransform::Log => t.ln(),
        PhTimeTransform::Rank => {
            let lo = sorted.partition_point(|v| *v < t);
            let hi = sorted.partition_point(|v| *v <= t);
            (lo as f64 + hi as f64 + 1.0) / 2.0
        }
    };
    let mean = time
        .iter()
        .zip(event)
        .filter(|(_, e)| **e == 1.0)
        .map(|(&t, _)| g(t) / model.events as f64)
        .sum::<f64>();
    let scale = times
        .iter()
        .map(|&t| (g(t) - mean).abs())
        .fold(0.0, f64::max);
    if scale <= 0.0 || !scale.is_finite() {
        return Err(parameter());
    }
    let start = vec![0.0; n];
    let risk = RiskData {
        start: &start,
        stop: time,
        event,
        strata: vec![(0, (0..n).collect(), times.clone())],
        x,
        ties: options.ties,
        control,
    };
    let mut score = vec![0.0; p];
    let mut base_score = vec![0.0; p];
    let mut i00 = Mat::zeros(p, p);
    let mut i01 = Mat::zeros(p, p);
    let mut i11 = Mat::zeros(p, p);
    for &t in &times {
        control.check()?;
        let (_, s, information) = risk.evaluate_time(&beta, Some(t))?;
        let gt = (g(t) - mean) / scale;
        for j in 0..p {
            base_score[j] += s[j];
            score[j] += gt * s[j];
            for k in 0..p {
                i00[(j, k)] += information[(j, k)];
                i01[(j, k)] += gt * information[(j, k)];
                i11[(j, k)] += gt * gt * information[(j, k)];
            }
        }
    }
    let correction = &i01 * inverse(&i00)?;
    let efficient = &i11 - &correction * &i01;
    let score = (0..p)
        .map(|j| {
            score[j]
                - (0..p)
                    .map(|k| correction[(j, k)] * base_score[k])
                    .sum::<f64>()
        })
        .collect::<Vec<_>>();
    let inverse = inverse(&efficient)?;
    let global = (0..p)
        .map(|j| score[j] * (0..p).map(|k| inverse[(j, k)] * score[k]).sum::<f64>())
        .sum::<f64>();
    let terms = (0..p)
        .map(|j| {
            if efficient[(j, j)] <= 0.0 {
                return Err(failed());
            }
            Ok(PhTermTest {
                term: model.coefficients[j].term.clone(),
                test: chi_square(score[j].powi(2) / efficient[(j, j)], 1)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    control.check()?;
    Ok(ProportionalHazardsResult {
        observations: n,
        events: model.events,
        ties: options.ties,
        time_transform: transform,
        log_likelihood: model.log_likelihood,
        coefficients: model.coefficients,
        terms,
        global: chi_square(global, p)?,
    })
}

pub fn fit(
    time: &[f64],
    event: &[f64],
    predictors: &[Vec<f64>],
    options: CoxOptions,
    control: &Control,
) -> Result<CoxResult> {
    fit_stratified(
        &vec![0.0; time.len()],
        time,
        event,
        predictors,
        &vec![0; time.len()],
        options,
        control,
    )
}

fn fit_stratified(
    start: &[f64],
    time: &[f64],
    event: &[f64],
    predictors: &[Vec<f64>],
    strata: &[usize],
    options: CoxOptions,
    control: &Control,
) -> Result<CoxResult> {
    data(time, event, predictors, control)?;
    check_iteration(options.iteration)?;
    if predictors.is_empty()
        || start.len() != time.len()
        || start
            .iter()
            .zip(time)
            .any(|(&s, &t)| !s.is_finite() || s < 0.0 || s >= t)
    {
        return Err(parameter());
    }
    let levels = groups(strata, time.len())?;
    let events = event.iter().filter(|&&v| v == 1.0).count();
    if events == 0 {
        return Err(parameter());
    }
    let design = Design::new(predictors, time.len(), true, true, false, control)?;
    let p = predictors.len();
    let x = Mat::from_fn(time.len(), p, |i, j| design.x[(i, j + 1)]);
    let strata = levels
        .into_iter()
        .map(|g| {
            let indices = (0..time.len())
                .filter(|&i| strata[i] == g)
                .collect::<Vec<_>>();
            let mut times = indices
                .iter()
                .filter(|&&i| event[i] == 1.0)
                .map(|&i| time[i])
                .collect::<Vec<_>>();
            times.sort_by(f64::total_cmp);
            times.dedup();
            (g, indices, times)
        })
        .collect::<Vec<_>>();
    let risk = RiskData {
        start,
        stop: time,
        event,
        strata,
        x,
        ties: options.ties,
        control,
    };
    let mut beta = vec![0.0; p];
    let mut iterations = 0;
    let mut converged = false;
    for iteration in 0..options.iteration.max_iterations {
        control.check()?;
        let (ll, score, info) = risk.evaluate(&beta)?;
        let inv = inverse(&info)?;
        let step = (0..p)
            .map(|j| (0..p).map(|k| inv[(j, k)] * score[k]).sum::<f64>())
            .collect::<Vec<_>>();
        let norm = step.iter().map(|v| v.abs()).fold(0.0, f64::max);
        let scale = beta.iter().map(|v| v.abs()).fold(1.0, f64::max);
        if finite(norm)? <= options.iteration.tolerance * scale {
            converged = true;
            break;
        }
        let improvement = score.iter().zip(&step).map(|(s, d)| s * d).sum::<f64>();
        let mut fraction = 1.0;
        let mut accepted = None;
        while fraction >= f64::EPSILON.sqrt() {
            control.check()?;
            let next = beta
                .iter()
                .zip(&step)
                .map(|(b, d)| b + fraction * d)
                .collect::<Vec<_>>();
            let next_ll = risk.evaluate(&next)?.0;
            if next_ll >= ll + 1e-4 * fraction * improvement - 1e-12 * (1.0 + ll.abs()) {
                accepted = Some(next);
                break;
            }
            fraction *= 0.5;
        }
        beta = accepted.ok_or_else(failed)?;
        iterations = iteration + 1;
    }
    if !converged {
        return Err(failed());
    }
    let (log_likelihood, _, info) = risk.evaluate(&beta)?;
    let inv = inverse(&info)?;
    let covariance = Mat::from_fn(p, p, |j, k| {
        inv[(j, k)] / (design.scales[j] * design.scales[k])
    });
    let raw = beta
        .iter()
        .zip(&design.scales)
        .map(|(b, s)| b / s)
        .collect::<Vec<_>>();
    let coefficients = coefficient_table(&raw, names(p, false), Some(&covariance), None)?;
    let linear_predictors = fitted(&risk.x, &beta);
    let mut baselines = Vec::new();
    for (stratum, indices, times) in &risk.strata {
        let mut cumulative_hazard = 0.0;
        let mut points = Vec::new();
        for &t in times {
            control.check()?;
            let at_risk = indices
                .iter()
                .copied()
                .filter(|&i| start[i] < t && t <= time[i])
                .collect::<Vec<_>>();
            let shift = at_risk
                .iter()
                .map(|&i| linear_predictors[i])
                .fold(f64::NEG_INFINITY, f64::max);
            let sum = at_risk
                .iter()
                .map(|&i| (linear_predictors[i] - shift).exp())
                .sum::<f64>();
            let deaths = indices
                .iter()
                .filter(|&&i| time[i] == t && event[i] == 1.0)
                .count();
            cumulative_hazard =
                finite(cumulative_hazard + ((deaths as f64).ln() - shift - sum.ln()).exp())?;
            points.push(BaselinePoint {
                time: t,
                cumulative_hazard,
                survival: (-cumulative_hazard).exp(),
            });
        }
        baselines.push(CoxBaseline {
            stratum: *stratum,
            maximum_followup: indices.iter().map(|&i| time[i]).fold(0.0, f64::max),
            points,
        });
    }
    Ok(CoxResult {
        observations: time.len(),
        events,
        ties: options.ties,
        iterations,
        log_likelihood,
        hazard_ratios: ratios(&coefficients)?,
        coefficients,
        covariance: rows(&covariance),
        predictor_means: design.means,
        predictor_ranges: predictors
            .iter()
            .map(|x| {
                [
                    x.iter().copied().fold(f64::INFINITY, f64::min),
                    x.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                ]
            })
            .collect(),
        linear_predictors,
        baselines,
    })
}

/// Single terminal event per subject; adjacent intervals are (start, stop].
pub fn time_dependent(
    start: &[f64],
    stop: &[f64],
    event: &[f64],
    subjects: &[usize],
    predictors: &[Vec<f64>],
    options: CoxOptions,
    control: &Control,
) -> Result<CoxResult> {
    data(stop, event, predictors, control)?;
    if start.len() != stop.len() {
        return Err(parameter());
    }
    for subject in groups(subjects, stop.len())? {
        control.check()?;
        let mut indices = (0..stop.len())
            .filter(|&i| subjects[i] == subject)
            .collect::<Vec<_>>();
        indices.sort_by(|&a, &b| start[a].total_cmp(&start[b]));
        for (j, &i) in indices.iter().enumerate() {
            if (j + 1 < indices.len() && (event[i] == 1.0 || stop[i] > start[indices[j + 1]]))
                || !start[i].is_finite()
            {
                return Err(parameter());
            }
        }
    }
    fit_stratified(
        start,
        stop,
        event,
        predictors,
        &vec![0; stop.len()],
        options,
        control,
    )
}

pub(super) fn baseline_at(model: &CoxResult, horizon: f64) -> Result<f64> {
    positive_horizon(horizon)?;
    if model.baselines.len() != 1 {
        return Err(parameter());
    }
    let baseline = &model.baselines[0];
    if horizon > baseline.maximum_followup {
        return Err(parameter());
    }
    Ok(baseline
        .points
        .iter()
        .rev()
        .find(|p| p.time <= horizon)
        .map_or(0.0, |p| p.cumulative_hazard))
}
pub fn event_probabilities(model: &CoxResult, horizon: f64) -> Result<Vec<f64>> {
    let hazard = baseline_at(model, horizon)?;
    model
        .linear_predictors
        .iter()
        .map(|&eta| {
            if hazard == 0.0 {
                Ok(0.0)
            } else {
                finite(-(-(hazard.ln() + eta).exp()).exp_m1())
            }
        })
        .collect()
}

/// Stratum-specific baseline hazards and treatment slopes, shared adjustment slopes.
pub fn subgroup(
    time: &[f64],
    event: &[f64],
    treatment: &[f64],
    group: &[usize],
    predictors: &[Vec<f64>],
    options: CoxOptions,
    control: &Control,
) -> Result<SubgroupResult> {
    data(time, event, predictors, control)?;
    if treatment.len() != time.len() || treatment.iter().any(|&v| v != 0.0 && v != 1.0) {
        return Err(parameter());
    }
    let levels = groups(group, time.len())?;
    let k = levels.len();
    if k < 2 {
        return Err(parameter());
    }
    let mut xs = Vec::new();
    let mut group_observations = Vec::new();
    let mut group_events = Vec::new();
    for &g in &levels {
        control.check()?;
        let indices = (0..time.len())
            .filter(|&i| group[i] == g)
            .collect::<Vec<_>>();
        let treated = indices.iter().filter(|&&i| treatment[i] == 1.0).count();
        if treated == 0 || treated == indices.len() {
            return Err(parameter());
        }
        group_observations.push(indices.len());
        group_events.push(indices.iter().filter(|&&i| event[i] == 1.0).count());
        xs.push(
            (0..time.len())
                .map(|i| if group[i] == g { treatment[i] } else { 0.0 })
                .collect(),
        );
    }
    xs.extend_from_slice(predictors);
    let mut model = fit_stratified(
        &vec![0.0; time.len()],
        time,
        event,
        &xs,
        group,
        options,
        control,
    )?;
    for j in 0..k {
        let name = format!("treatment:group{}", j + 1);
        model.coefficients[j].term.clone_from(&name);
        model.hazard_ratios[j].term = name;
    }
    let v = Mat::from_fn(k - 1, k - 1, |j, l| {
        model.covariance[j + 1][l + 1] - model.covariance[j + 1][0] - model.covariance[0][l + 1]
            + model.covariance[0][0]
    });
    let inv = inverse(&v)?;
    let delta = (1..k)
        .map(|j| model.coefficients[j].estimate - model.coefficients[0].estimate)
        .collect::<Vec<_>>();
    let statistic = (0..k - 1)
        .map(|j| delta[j] * (0..k - 1).map(|l| inv[(j, l)] * delta[l]).sum::<f64>())
        .sum::<f64>();
    Ok(SubgroupResult {
        groups: levels,
        group_observations,
        group_events,
        treatment_hazard_ratios: model.hazard_ratios[..k].to_vec(),
        equality_test: chi_square(statistic.max(0.0), k - 1)?,
        model,
    })
}
