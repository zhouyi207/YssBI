use super::common::*;
use statrs::distribution::FisherSnedecor;
use yss_sci_contract::execution::{
    ScientificComputationError as Error, ScientificExecutionControl as Control,
};
use yss_sci_contract::regression::models::*;
use yss_sci_linalg::{Mat, matrix_rank};

type ThresholdCandidate = (f64, f64, Vec<f64>, Mat<f64>, Mat<f64>, usize);

fn stage(label: usize, indices: &[usize], model: RegressionModelResult) -> RegressionStage {
    RegressionStage {
        label,
        predictors: indices.iter().map(|i| i + 1).collect(),
        observation_indices: vec![],
        delta_r_squared: None,
        change_f: None,
        change_df: None,
        change_p_value: None,
        model,
    }
}
fn choose(x: &[Vec<f64>], indices: &[usize]) -> Vec<Vec<f64>> {
    indices.iter().map(|&i| x[i].clone()).collect()
}
pub fn baseline(
    y: &[f64],
    x: &[Vec<f64>],
    constant: bool,
    control: &Control,
) -> Result<RegressionModelResult> {
    ols(y, x, constant, control)
}
pub fn hierarchical(
    y: &[f64],
    x: &[Vec<f64>],
    blocks: &[usize],
    constant: bool,
    control: &Control,
) -> Result<RegressionWorkflowResult> {
    validate(y, x, control)?;
    if blocks.is_empty()
        || blocks.contains(&0)
        || blocks.iter().try_fold(0usize, |n, &v| n.checked_add(v)) != Some(x.len())
    {
        return Err(parameter());
    }
    let mut stages: Vec<RegressionStage> = vec![];
    let mut end = 0;
    for (index, &size) in blocks.iter().enumerate() {
        control.check()?;
        end += size;
        let indices = (0..end).collect::<Vec<_>>();
        let model = ols(y, &x[..end], constant, control)?;
        let mut s = stage(index + 1, &indices, model);
        if let Some(previous) = stages.last() {
            let old = previous.model.statistics.rss.ok_or_else(failed)?;
            let new = s.model.statistics.rss.ok_or_else(failed)?;
            let df = s.model.statistics.df_residual.ok_or_else(failed)?;
            s.delta_r_squared = s
                .model
                .statistics
                .r_squared
                .zip(previous.model.statistics.r_squared)
                .map(|(a, b)| a - b);
            s.change_df = Some(size);
            if new > 0.0 {
                let f = ((old - new).max(0.0) / size as f64) / (new / df as f64);
                s.change_f = Some(finite(f)?);
                s.change_p_value = Some(crate::distribution::fisher_snedecor_sf(
                    &FisherSnedecor::new(size as f64, df as f64).map_err(|_| failed())?,
                    f,
                ));
            }
        }
        stages.push(s);
    }
    Ok(RegressionWorkflowResult {
        method: "hierarchical".into(),
        stages,
        selection_history: vec![],
        selected_predictors: (1..=x.len()).collect(),
    })
}
pub fn univariate_multivariable(
    y: &[f64],
    x: &[Vec<f64>],
    constant: bool,
    control: &Control,
) -> Result<RegressionWorkflowResult> {
    validate(y, x, control)?;
    if x.is_empty() {
        return Err(parameter());
    }
    let mut stages = vec![];
    for (i, column) in x.iter().enumerate() {
        control.check()?;
        let mut model = ols(y, std::slice::from_ref(column), constant, control)?;
        for c in &mut model.coefficients {
            if c.term == "x1" {
                c.term = format!("x{}", i + 1);
            }
        }
        stages.push(stage(i + 1, &[i], model));
    }
    stages.push(stage(
        x.len() + 1,
        &(0..x.len()).collect::<Vec<_>>(),
        ols(y, x, constant, control)?,
    ));
    Ok(RegressionWorkflowResult {
        method: "univariate_multivariable".into(),
        stages,
        selection_history: vec![],
        selected_predictors: (1..=x.len()).collect(),
    })
}
pub fn grouped(
    y: &[f64],
    x: &[Vec<f64>],
    groups: &[usize],
    constant: bool,
    control: &Control,
) -> Result<RegressionWorkflowResult> {
    validate(y, x, control)?;
    if groups.len() != y.len() {
        return Err(parameter());
    }
    if groups.iter().any(|&group| group >= y.len()) {
        return Err(parameter());
    }
    let count = groups.iter().copied().max().ok_or_else(parameter)? + 1;
    let mut grouped_rows = vec![Vec::new(); count];
    for (i, &group) in groups.iter().enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        grouped_rows[group].push(i);
    }
    let mut stages = vec![];
    for (group, rows) in grouped_rows.into_iter().enumerate() {
        control.check()?;
        if rows.is_empty() {
            return Err(parameter());
        }
        let response = rows.iter().map(|&i| y[i]).collect::<Vec<_>>();
        let predictors = x
            .iter()
            .map(|x| rows.iter().map(|&i| x[i]).collect())
            .collect::<Vec<_>>();
        let mut s = stage(
            group,
            &(0..x.len()).collect::<Vec<_>>(),
            ols(&response, &predictors, constant, control)?,
        );
        s.observation_indices = rows.iter().map(|i| i + 1).collect();
        stages.push(s);
    }
    Ok(RegressionWorkflowResult {
        method: "grouped".into(),
        stages,
        selection_history: vec![],
        selected_predictors: (1..=x.len()).collect(),
    })
}
fn criterion(model: &RegressionModelResult, kind: SelectionCriterion) -> Result<f64> {
    match kind {
        SelectionCriterion::Aic => model.statistics.aic,
        SelectionCriterion::Bic => model.statistics.bic,
    }
    .ok_or_else(parameter)
}
fn renamed(
    mut model: RegressionModelResult,
    indices: &[usize],
    constant: bool,
) -> RegressionModelResult {
    for (j, &i) in indices.iter().enumerate() {
        model.coefficients[j + usize::from(constant)].term = format!("x{}", i + 1);
    }
    model
}
pub fn stepwise(
    y: &[f64],
    x: &[Vec<f64>],
    constant: bool,
    direction: SelectionDirection,
    kind: SelectionCriterion,
    control: &Control,
) -> Result<RegressionWorkflowResult> {
    validate(y, x, control)?;
    if x.is_empty() {
        return Err(parameter());
    }
    let mut selected = if direction == SelectionDirection::Backward {
        (0..x.len()).collect::<Vec<_>>()
    } else {
        vec![]
    };
    if !constant && selected.is_empty() {
        let mut best = None;
        for (i, predictor) in x.iter().enumerate() {
            let fit = ols(y, std::slice::from_ref(predictor), false, control)?;
            let score = criterion(&fit, kind)?;
            if best.as_ref().is_none_or(|(_, s)| score < *s) {
                best = Some((i, score));
            }
        }
        selected.push(best.ok_or_else(failed)?.0);
    }
    let mut model = ols(y, &choose(x, &selected), constant, control)?;
    let mut score = criterion(&model, kind)?;
    let mut history = vec![SelectionStep {
        action: "start".into(),
        predictor: None,
        predictors: selected.iter().map(|i| i + 1).collect(),
        criterion_value: score,
    }];
    for _ in 0..x.len() * x.len() + 1 {
        control.check()?;
        let mut best: Option<(Vec<usize>, RegressionModelResult, f64, String, usize)> = None;
        let mut candidates = vec![];
        if direction != SelectionDirection::Backward {
            for i in 0..x.len() {
                if !selected.contains(&i) {
                    let mut next = selected.clone();
                    next.push(i);
                    next.sort_unstable();
                    candidates.push((next, "enter", i));
                }
            }
        }
        if direction != SelectionDirection::Forward && (constant || selected.len() > 1) {
            for &i in &selected {
                candidates.push((
                    selected.iter().copied().filter(|&j| j != i).collect(),
                    "remove",
                    i,
                ));
            }
        }
        for (next, action, i) in candidates {
            control.check()?;
            let candidate = match ols(y, &choose(x, &next), constant, control) {
                Ok(m) => m,
                Err(e @ (Error::Cancelled | Error::DeadlineExceeded)) => return Err(e),
                Err(_) => continue,
            };
            let value = criterion(&candidate, kind)?;
            if value < score - 1e-8 && best.as_ref().is_none_or(|b| value < b.2 - 1e-8) {
                best = Some((next, candidate, value, action.into(), i));
            }
        }
        let Some((next, fit, value, action, i)) = best else {
            let final_model = renamed(model, &selected, constant);
            return Ok(RegressionWorkflowResult {
                method: "stepwise".into(),
                stages: vec![stage(1, &selected, final_model)],
                selection_history: history,
                selected_predictors: selected.iter().map(|i| i + 1).collect(),
            });
        };
        selected = next;
        model = fit;
        score = value;
        history.push(SelectionStep {
            action,
            predictor: Some(i + 1),
            predictors: selected.iter().map(|i| i + 1).collect(),
            criterion_value: score,
        });
    }
    Err(failed())
}
pub fn threshold(
    y: &[f64],
    x: &[Vec<f64>],
    threshold_variable: &[f64],
    constant: bool,
    trimming: f64,
    max_candidates: usize,
    control: &Control,
) -> Result<RegressionModelResult> {
    validate(y, x, control)?;
    if threshold_variable.len() != y.len()
        || threshold_variable.iter().any(|v| !v.is_finite())
        || !trimming.is_finite()
        || !(0.05..=0.45).contains(&trimming)
        || !(1..=200).contains(&max_candidates)
    {
        return Err(parameter());
    }
    let design = Design::new(x, y.len(), constant, true, true, control)?;
    let p = design.x.ncols();
    let min = ((y.len() as f64 * trimming).ceil() as usize).max(p + 2);
    if y.len() < 2 * min {
        return Err(parameter());
    }
    let mut sorted = threshold_variable.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mut candidates = vec![];
    for i in min - 1..y.len() - min {
        if i % 1024 == 0 {
            control.check()?;
        }
        if sorted[i] != sorted[i + 1] {
            candidates.push(sorted[i]);
        }
    }
    if candidates.len() > max_candidates {
        let all = candidates;
        candidates = (0..max_candidates)
            .map(|i| {
                all[if max_candidates == 1 {
                    all.len() / 2
                } else {
                    i * (all.len() - 1) / (max_candidates - 1)
                }]
            })
            .collect();
    }
    let mut best: Option<ThresholdCandidate> = None;
    let mut evaluated = 0;
    for cutoff in candidates {
        control.check()?;
        let left = threshold_variable.iter().filter(|&&v| v <= cutoff).count();
        let matrix = Mat::from_fn(y.len(), 2 * p, |i, j| {
            if (threshold_variable[i] <= cutoff && j < p)
                || (threshold_variable[i] > cutoff && j >= p)
            {
                design.x[(i, j % p)]
            } else {
                0.0
            }
        });
        if matrix_rank(matrix.as_ref()).map_err(|_| failed())?.0 != 2 * p {
            continue;
        }
        let (beta, inv) = least_squares(&matrix, y, None, control)?;
        let pred = fitted(&matrix, &beta);
        let rss = y
            .iter()
            .zip(&pred)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>();
        evaluated += 1;
        if best.as_ref().is_none_or(|b| rss < b.0) {
            best = Some((rss, cutoff, beta, inv, matrix, left));
        }
    }
    let (rss, cutoff, beta, inv, matrix, left) = best.ok_or_else(parameter)?;
    let df = y.len() - 2 * p;
    let cov = Mat::from_fn(2 * p, 2 * p, |i, j| inv[(i, j)] * rss / df as f64);
    let j = design.raw_jacobian(1.0);
    let transform = Mat::from_fn(2 * p, 2 * p, |i, k| {
        if i / p == k / p {
            j[(i % p, k % p)]
        } else {
            0.0
        }
    });
    let (raw, cov) = super::common::transform(&beta, Some(cov), &transform);
    let terms = ["below", "above"]
        .iter()
        .flat_map(|prefix| {
            names(x.len(), constant)
                .into_iter()
                .map(move |name| format!("{prefix}.{name}"))
        })
        .collect();
    let mut r = result(
        "threshold",
        y,
        fitted(&matrix, &beta),
        raw,
        terms,
        cov,
        constant,
        Some(df),
        RegressionDetails::Threshold {
            threshold: cutoff,
            regime_counts: [left, y.len() - left],
            trimming,
            evaluated_candidates: evaluated,
        },
    )?;
    gaussian_likelihood(&mut r)?;
    control.check()?;
    Ok(r)
}
