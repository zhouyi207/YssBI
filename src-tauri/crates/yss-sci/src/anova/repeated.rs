use super::{
    Result,
    design::{masks, term_name, validate_factors},
    f_probability, factor_levels, finite, invalid,
};
use yss_sci_contract::{anova::*, execution::*};

/// Complete factorial within-subject designs, one observation per subject/cell.
pub fn repeated_measures(
    response: &[f64],
    subjects: &[usize],
    factors: &[Factor],
    correction: SphericityCorrection,
    control: &ScientificExecutionControl,
) -> Result<RepeatedMeasuresResult> {
    control.check()?;
    validate_factors(response.len(), factors, MAX_REPEATED_FACTORS, control)?;
    if subjects.len() != response.len() {
        return Err(invalid(ScientificInputViolation::ShapeMismatch));
    }
    let cells = factors
        .iter()
        .try_fold(1usize, |n, f| n.checked_mul(f.levels))
        .filter(|&n| n <= MAX_REPEATED_CELLS)
        .ok_or_else(|| invalid(ScientificInputViolation::ParameterOutOfRange))?;
    let subject_count = subjects
        .iter()
        .copied()
        .max()
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| invalid(ScientificInputViolation::EmptyInput))?;
    if subject_count < 2 || subject_count.checked_mul(cells) != Some(response.len()) {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let mut scale = 0.0f64;
    for (i, &value) in response.iter().enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        if !value.is_finite() {
            return Err(invalid(ScientificInputViolation::NonFiniteInput));
        }
        scale = scale.max(value.abs());
    }
    if scale == 0.0 {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let mut grid = vec![0.0; response.len()];
    let mut seen = vec![false; response.len()];
    for (row, &value) in response.iter().enumerate() {
        if row.is_multiple_of(1024) {
            control.check()?;
        }
        let mut cell = 0;
        let mut stride = 1;
        for factor in factors {
            cell += factor.values[row] * stride;
            stride *= factor.levels;
        }
        let index = subjects[row] * cells + cell;
        if seen[index] {
            return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
        }
        seen[index] = true;
        grid[index] = value / scale;
    }
    if seen.contains(&false) {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    // Remove subject intercepts before contrasts, preserving paired differences.
    for values in grid.chunks_mut(cells) {
        control.check()?;
        let mean = values.iter().sum::<f64>() / cells as f64;
        for value in values {
            *value -= mean;
        }
    }
    let mut table = Vec::new();
    for mask in masks(factors.len()) {
        control.check()?;
        let df = factors
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, factor)| factor.levels - 1)
            .product::<usize>();
        let mut scores = vec![vec![0.0; subject_count]; df];
        for (contrast, contrast_scores) in scores.iter_mut().enumerate() {
            let mut code = contrast;
            let choices = factors
                .iter()
                .enumerate()
                .map(|(i, factor)| {
                    if mask & (1 << i) == 0 {
                        None
                    } else {
                        let choice = code % (factor.levels - 1);
                        code /= factor.levels - 1;
                        Some(choice)
                    }
                })
                .collect::<Vec<_>>();
            let weights = (0..cells)
                .map(|mut cell| {
                    factors
                        .iter()
                        .zip(&choices)
                        .map(|(factor, choice)| {
                            let level = cell % factor.levels;
                            cell /= factor.levels;
                            match choice {
                                None => 1.0 / (factor.levels as f64).sqrt(),
                                Some(j) => {
                                    let denominator = ((j + 1) * (j + 2)) as f64;
                                    if level <= *j {
                                        1.0 / denominator.sqrt()
                                    } else if level == j + 1 {
                                        -((*j + 1) as f64) / denominator.sqrt()
                                    } else {
                                        0.0
                                    }
                                }
                            }
                        })
                        .product::<f64>()
                })
                .collect::<Vec<_>>();
            for (subject, values) in grid.chunks(cells).enumerate() {
                if subject.is_multiple_of(128) {
                    control.check()?;
                }
                contrast_scores[subject] = values
                    .iter()
                    .zip(&weights)
                    .map(|(value, weight)| value * weight)
                    .sum();
            }
        }
        let means = scores
            .iter()
            .map(|values| {
                values
                    .iter()
                    .map(|value| value / subject_count as f64)
                    .sum::<f64>()
            })
            .collect::<Vec<_>>();
        let ss = means
            .iter()
            .map(|mean| mean * mean * subject_count as f64)
            .sum::<f64>();
        for (values, mean) in scores.iter_mut().zip(&means) {
            for value in values {
                *value -= mean;
            }
        }
        let error_ss = scores
            .iter()
            .flatten()
            .map(|value| value * value)
            .sum::<f64>();
        if error_ss <= 0.0 {
            return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
        }
        let error_df = (subject_count - 1) * df;
        let f = finite((ss / df as f64) / (error_ss / error_df as f64))?;
        let mut squared_trace = 0.0;
        for (i, left) in scores.iter().enumerate() {
            for (j, right) in scores.iter().enumerate().take(i + 1) {
                let mut cross = 0.0;
                for (subject, (left, right)) in left.iter().zip(right).enumerate() {
                    if subject.is_multiple_of(1024) {
                        control.check()?;
                    }
                    cross += left * right;
                }
                // Normalize before squaring to keep epsilon usable for small variances.
                squared_trace += (cross / error_ss).powi(2) * if i == j { 1.0 } else { 2.0 };
            }
        }
        let epsilon = finite(1.0 / (df as f64 * squared_trace))?.clamp(1.0 / df as f64, 1.0);
        let adjustment = if correction == SphericityCorrection::None {
            1.0
        } else {
            epsilon
        };
        let df_numerator = df as f64 * adjustment;
        let df_denominator = error_df as f64 * adjustment;
        let sum_squares = finite((ss * scale) * scale)?;
        let error_sum_squares = finite((error_ss * scale) * scale)?;
        table.push(RepeatedTerm {
            term: term_name(mask, factors.len()),
            sum_squares,
            error_sum_squares,
            df,
            error_df,
            mean_square: sum_squares / df as f64,
            error_mean_square: error_sum_squares / error_df as f64,
            f_statistic: f,
            p_value_uncorrected: f_probability(f, df as f64, error_df as f64)?,
            epsilon_greenhouse_geisser: epsilon,
            df_numerator,
            df_denominator,
            p_value: f_probability(f, df_numerator, df_denominator)?,
            partial_eta_squared: ss / (ss + error_ss),
        });
    }
    control.check()?;
    Ok(RepeatedMeasuresResult {
        method: "repeated_measures_anova".into(),
        observations: response.len(),
        subjects: subject_count,
        cells_per_subject: cells,
        factors: factor_levels(factors),
        correction,
        table,
    })
}
