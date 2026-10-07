//! Grade memberships use one criterion per row and one grade per column.
use super::data::*;
use yss_sci_contract::decision::fuzzy::*;
pub fn evaluate(
    columns: &[Vec<f64>],
    weights: &[f64],
    grade_scores: &[f64],
    operator: FuzzyOperator,
    control: &Control,
) -> Result<FuzzyEvaluation> {
    let (n, grades) = dimensions(columns, &vec![false; columns.len()], control)?;
    if !grade_scores.is_empty()
        && (grade_scores.len() != grades || grade_scores.iter().any(|v| !v.is_finite()))
    {
        return Err(parameter());
    }
    let weights = supplied_weights(weights, n)?;
    let mut aggregate = vec![0_f64; grades];
    let mut row = vec![0.; grades];
    for i in 0..n {
        control.check()?;
        for j in 0..grades {
            row[j] = columns[j][i];
        }
        let membership = normalized_weights(&row)?;
        for (j, b) in aggregate.iter_mut().enumerate() {
            let a = match operator {
                FuzzyOperator::ProductSum | FuzzyOperator::ProductMax => weights[i] * membership[j],
                FuzzyOperator::MinMax | FuzzyOperator::MinSum => weights[i].min(membership[j]),
            };
            *b = match operator {
                FuzzyOperator::ProductSum => *b + a,
                FuzzyOperator::MinSum => (*b + a).min(1.),
                FuzzyOperator::MinMax | FuzzyOperator::ProductMax => b.max(a),
            };
        }
    }
    let memberships = normalized_weights(&aggregate)?;
    let max = memberships.iter().copied().fold(0., f64::max);
    let dominant_grades = memberships
        .iter()
        .enumerate()
        .filter_map(|(i, &v)| ((v - max).abs() <= 64. * f64::EPSILON * max).then_some(i + 1))
        .collect();
    let score = if grade_scores.is_empty() {
        None
    } else {
        Some(finite(
            memberships
                .iter()
                .zip(grade_scores)
                .map(|(p, x)| p * x)
                .sum(),
        )?)
    };
    Ok(FuzzyEvaluation {
        criteria: n,
        grades,
        operator,
        weights,
        raw_memberships: aggregate,
        memberships,
        dominant_grades,
        score,
    })
}
