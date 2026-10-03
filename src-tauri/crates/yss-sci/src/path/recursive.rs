//! Observed continuous-variable recursive path equations and effect decomposition.
use super::*;
use crate::regression::models::common::ols;
use std::collections::VecDeque;

/// Explicit x1..xp column references; one equation per line or semicolon.
pub fn parse_equations(
    text: &str,
    variables: usize,
    control: &Control,
) -> Result<Vec<PathEquation>> {
    let variable = |token: &str| -> Result<usize> {
        let digits = token.trim().strip_prefix('x').ok_or_else(parameter)?;
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(parameter());
        }
        let index = digits.parse::<usize>().map_err(|_| parameter())?;
        if index == 0 || index > variables {
            return Err(parameter());
        }
        Ok(index - 1)
    };
    let mut equations = Vec::new();
    for statement in text
        .split([';', '\n'])
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        control.check()?;
        let (lhs, rhs) = statement.split_once('~').ok_or_else(parameter)?;
        equations.push(PathEquation {
            response: variable(lhs)?,
            predictors: rhs.split('+').map(variable).collect::<Result<_>>()?,
        });
    }
    order(variables, &equations, control)?;
    Ok(equations)
}

pub fn recursive_path(
    columns: &[Vec<f64>],
    equations: &[PathEquation],
    control: &Control,
) -> Result<RecursivePathResult> {
    control.check()?;
    if columns.len() < 2 {
        return Err(parameter());
    }
    validate(&columns[0], &columns[1..], control)?;
    let ordered = order(columns.len(), equations, control)?;
    let mut fits = Vec::with_capacity(equations.len());
    let mut direct = vec![vec![0.; columns.len()]; columns.len()];
    for equation in equations {
        control.check()?;
        let x: Vec<_> = equation
            .predictors
            .iter()
            .map(|&j| columns[j].clone())
            .collect();
        let mut model = ols(&columns[equation.response], &x, true, control)?;
        model.method = "recursive_path_ols".into();
        for (j, &predictor) in equation.predictors.iter().enumerate() {
            model.coefficients[j + 1].term = format!("x{}", predictor + 1);
            direct[equation.response][predictor] = model.coefficients[j + 1].estimate;
        }
        fits.push(PathEquationFit {
            equation: equation.clone(),
            model,
        });
    }
    let centered = super::preparation::center_columns(columns, control)?;
    let sd: Vec<_> = centered
        .values
        .iter()
        .map(|x| super::preparation::sample_sd(x).ok())
        .collect();
    let mut effects = Vec::new();
    for source in 0..columns.len() {
        control.check()?;
        let mut total = vec![0.; columns.len()];
        total[source] = 1.;
        for &target in &ordered {
            if target != source {
                total[target] =
                    finite(direct[target].iter().zip(&total).map(|(a, b)| a * b).sum())?;
            }
        }
        for target in 0..columns.len() {
            if target == source {
                continue;
            }
            effects.push(PathDecomposition {
                source,
                target,
                direct: direct[target][source],
                indirect: finite(total[target] - direct[target][source])?,
                total: total[target],
                standardized_total: sd[source]
                    .zip(sd[target])
                    .map(|(s, t)| finite(total[target] * s / t))
                    .transpose()?,
            });
        }
    }
    Ok(RecursivePathResult {
        observations: columns[0].len(),
        equations: fits,
        effects,
    })
}

fn order(variables: usize, equations: &[PathEquation], control: &Control) -> Result<Vec<usize>> {
    if variables < 2 || equations.is_empty() {
        return Err(parameter());
    }
    let mut responses = vec![false; variables];
    let mut children = vec![Vec::new(); variables];
    let mut indegrees = vec![0; variables];
    for e in equations {
        control.check()?;
        if e.response >= variables || e.predictors.is_empty() || responses[e.response] {
            return Err(parameter());
        }
        responses[e.response] = true;
        let mut seen = vec![false; variables];
        for &p in &e.predictors {
            if p >= variables || p == e.response || seen[p] {
                return Err(parameter());
            }
            seen[p] = true;
            indegrees[e.response] += 1;
            children[p].push(e.response);
        }
    }
    let mut ready: VecDeque<_> = indegrees
        .iter()
        .enumerate()
        .filter_map(|(i, &d)| (d == 0).then_some(i))
        .collect();
    let mut order = Vec::with_capacity(variables);
    while let Some(p) = ready.pop_front() {
        control.check()?;
        order.push(p);
        for &c in &children[p] {
            indegrees[c] -= 1;
            if indegrees[c] == 0 {
                ready.push_back(c);
            }
        }
    }
    if order.len() != variables {
        return Err(parameter());
    }
    Ok(order)
}
