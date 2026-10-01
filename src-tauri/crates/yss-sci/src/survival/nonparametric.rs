use super::common::*;
use yss_sci_contract::survival::*;

pub fn curves(
    time: &[f64],
    event: &[f64],
    group: &[usize],
    method: CurveMethod,
    control: &Control,
) -> Result<CurveResult> {
    data(time, event, &[], control)?;
    let levels = groups(group, time.len())?;
    let mut curves = Vec::with_capacity(levels.len());
    for level in levels {
        control.check()?;
        let indices = (0..time.len())
            .filter(|&i| group[i] == level)
            .collect::<Vec<_>>();
        curves.push(curve(time, event, &indices, level, method, control)?);
    }
    Ok(CurveResult {
        method,
        confidence_level: 0.95,
        curves,
    })
}

pub(super) fn curve(
    time: &[f64],
    event: &[f64],
    indices: &[usize],
    group: usize,
    method: CurveMethod,
    control: &Control,
) -> Result<SurvivalCurve> {
    let mut ordered = indices.to_vec();
    ordered.sort_by(|&a, &b| time[a].total_cmp(&time[b]));
    control.check()?;
    let mut points = Vec::new();
    let (mut survival, mut hazard, mut greenwood, mut variance) = (1.0, 0.0, 0.0, 0.0);
    let (mut begin, mut total_events, mut median) = (0, 0, None);
    while begin < ordered.len() {
        control.check()?;
        let t = time[ordered[begin]];
        let mut end = begin;
        let mut events = 0;
        while end < ordered.len() && time[ordered[end]] == t {
            if end.is_multiple_of(1024) {
                control.check()?;
            }
            events += usize::from(event[ordered[end]] == 1.0);
            end += 1;
        }
        let risk = ordered.len() - begin;
        total_events += events;
        if events > 0 {
            survival *= 1.0 - events as f64 / risk as f64;
            hazard += events as f64 / risk as f64;
            variance += events as f64 / (risk as f64).powi(2);
            if events < risk {
                greenwood += events as f64 / (risk as f64 * (risk - events) as f64);
            }
        }
        let (estimate, se, ci, s) = match method {
            CurveMethod::KaplanMeier => {
                // Log-log intervals remain in [0,1], including terminal failures.
                let ci = if survival > 0.0 && survival < 1.0 {
                    let log_h = (-survival.ln()).ln();
                    let error = Z95 * greenwood.sqrt() / survival.ln().abs();
                    [
                        (-(log_h + error).exp()).exp(),
                        (-(log_h - error).exp()).exp(),
                    ]
                } else {
                    [survival; 2]
                };
                (survival, survival * greenwood.sqrt(), ci, survival)
            }
            CurveMethod::NelsonAalen => {
                let se = variance.sqrt();
                let ci = if hazard == 0.0 {
                    [0.0; 2]
                } else {
                    [
                        hazard * (-Z95 * se / hazard).exp(),
                        hazard * (Z95 * se / hazard).exp(),
                    ]
                };
                (hazard, se, ci, (-hazard).exp())
            }
        };
        if median.is_none() && s <= 0.5 {
            median = Some(t);
        }
        points.push(SurvivalPoint {
            time: t,
            at_risk: risk,
            events,
            censored: end - begin - events,
            survival: s,
            cumulative_hazard: hazard,
            estimate,
            standard_error: se,
            confidence_interval: ci,
        });
        begin = end;
    }
    Ok(SurvivalCurve {
        group,
        observations: indices.len(),
        events: total_events,
        median_survival: median,
        points,
    })
}

pub fn logrank(
    time: &[f64],
    event: &[f64],
    group: &[usize],
    control: &Control,
) -> Result<LogrankResult> {
    data(time, event, &[], control)?;
    let levels = groups(group, time.len())?;
    let k = levels.len();
    if k < 2 {
        return Err(parameter());
    }
    let codes = group
        .iter()
        .map(|g| levels.binary_search(g).expect("known group"))
        .collect::<Vec<_>>();
    let mut risk = vec![0usize; k];
    for &g in &codes {
        risk[g] += 1;
    }
    let mut observed = vec![0; k];
    let mut expected = vec![0.0; k];
    let mut covariance = Mat::zeros(k, k);
    let mut ordered = (0..time.len()).collect::<Vec<_>>();
    ordered.sort_by(|&a, &b| time[a].total_cmp(&time[b]));
    let mut begin = 0;
    while begin < ordered.len() {
        control.check()?;
        let mut end = begin;
        let mut d = vec![0usize; k];
        let mut removed = vec![0usize; k];
        while end < ordered.len() && time[ordered[end]] == time[ordered[begin]] {
            let i = ordered[end];
            d[codes[i]] += usize::from(event[i] == 1.0);
            removed[codes[i]] += 1;
            end += 1;
        }
        let n = (ordered.len() - begin) as f64;
        let events = d.iter().sum::<usize>() as f64;
        for j in 0..k {
            observed[j] += d[j];
            let pj = risk[j] as f64 / n;
            expected[j] += events * pj;
            if n > 1.0 {
                for l in 0..k {
                    let indicator = if j == l { 1.0 } else { 0.0 };
                    covariance[(j, l)] +=
                        events * (n - events) / (n - 1.0) * pj * (indicator - risk[l] as f64 / n);
                }
            }
        }
        for j in 0..k {
            risk[j] -= removed[j];
        }
        begin = end;
    }
    let inv = inverse(&Mat::from_fn(k - 1, k - 1, |j, l| covariance[(j, l)]))?;
    let score = (0..k - 1)
        .map(|j| observed[j] as f64 - expected[j])
        .collect::<Vec<_>>();
    let statistic = (0..k - 1)
        .map(|j| score[j] * (0..k - 1).map(|l| inv[(j, l)] * score[l]).sum::<f64>())
        .sum::<f64>();
    Ok(LogrankResult {
        groups: levels,
        observed,
        expected,
        covariance: rows(&covariance),
        test: chi_square(statistic.max(0.0), k - 1)?,
    })
}

pub fn competing_risks(
    time: &[f64],
    status: &[usize],
    control: &Control,
) -> Result<CompetingRisksResult> {
    let event = status
        .iter()
        .map(|&s| if s == 0 { 0.0 } else { 1.0 })
        .collect::<Vec<_>>();
    data(time, &event, &[], control)?;
    let causes = status
        .iter()
        .copied()
        .filter(|&s| s > 0)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if causes.is_empty() {
        return Err(parameter());
    }
    let mut ordered = (0..time.len()).collect::<Vec<_>>();
    ordered.sort_by(|&a, &b| time[a].total_cmp(&time[b]));
    let mut points = Vec::new();
    let mut incidence = vec![0.0; causes.len()];
    let (mut begin, mut survival) = (0, 1.0);
    while begin < ordered.len() {
        control.check()?;
        let mut end = begin;
        let mut events = vec![0; causes.len()];
        let mut censored = 0;
        while end < ordered.len() && time[ordered[end]] == time[ordered[begin]] {
            let s = status[ordered[end]];
            if s == 0 {
                censored += 1;
            } else {
                events[causes.binary_search(&s).expect("known cause")] += 1;
            }
            end += 1;
        }
        let risk = ordered.len() - begin;
        for j in 0..causes.len() {
            incidence[j] += survival * events[j] as f64 / risk as f64;
        }
        survival *= 1.0 - events.iter().sum::<usize>() as f64 / risk as f64;
        points.push(IncidencePoint {
            time: time[ordered[begin]],
            at_risk: risk,
            events,
            censored,
            survival,
            cumulative_incidence: incidence.clone(),
        });
        begin = end;
    }
    Ok(CompetingRisksResult {
        observations: time.len(),
        causes,
        points,
    })
}

pub(super) fn risk_at(
    time: &[f64],
    event: &[f64],
    indices: &[usize],
    horizon: f64,
    control: &Control,
) -> Result<(f64, [f64; 2])> {
    if indices.is_empty() {
        return Err(parameter());
    }
    let curve = curve(time, event, indices, 0, CurveMethod::KaplanMeier, control)?;
    let last = curve.points.iter().rev().find(|p| p.time <= horizon);
    let (s, ci) = last.map_or((1.0, [1.0; 2]), |p| (p.survival, p.confidence_interval));
    if curve.points.last().is_none_or(|p| p.time < horizon) && s > 0.0 {
        return Err(parameter());
    }
    Ok((1.0 - s, [1.0 - ci[1], 1.0 - ci[0]]))
}
