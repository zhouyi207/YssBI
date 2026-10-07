pub(super) use crate::regression::models::common::{Result, finite, parameter, validate};
pub(super) use yss_sci_contract::{decision::*, execution::ScientificExecutionControl as Control};

pub(super) fn dimensions(
    columns: &[Vec<f64>],
    costs: &[bool],
    control: &Control,
) -> Result<(usize, usize)> {
    let n = columns.first().map_or(0, Vec::len);
    if n == 0 || columns.is_empty() || columns.len() != costs.len() {
        return Err(parameter());
    }
    validate(&columns[0], columns, control)?;
    Ok((n, columns.len()))
}
pub(super) fn square(columns: &[Vec<f64>], control: &Control) -> Result<usize> {
    let (n, p) = dimensions(columns, &vec![false; columns.len()], control)?;
    if n != p {
        return Err(
            yss_sci_contract::execution::ScientificComputationError::InvalidInput {
                violation: yss_sci_contract::execution::ScientificInputViolation::ShapeMismatch,
            },
        );
    }
    Ok(n)
}
pub(super) fn normalize(
    columns: &[Vec<f64>],
    method: Normalization,
    control: &Control,
) -> Result<Vec<Vec<f64>>> {
    columns
        .iter()
        .map(|x| {
            control.check()?;
            let scale = x.iter().map(|x| x.abs()).fold(0., f64::max);
            let scaled = x
                .iter()
                .map(|x| if scale > 0. { x / scale } else { 0. })
                .collect::<Vec<_>>();
            let low = scaled.iter().copied().fold(f64::INFINITY, f64::min);
            let high = scaled.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let norm = scaled.iter().map(|x| x * x).sum::<f64>().sqrt();
            x.iter()
                .zip(scaled)
                .map(|(&raw, z)| {
                    finite(match method {
                        Normalization::None => raw,
                        Normalization::MinMax => {
                            if high > low {
                                (z - low) / (high - low)
                            } else {
                                0.
                            }
                        }
                        Normalization::Vector => {
                            if norm > 0. {
                                z / norm
                            } else {
                                0.
                            }
                        }
                    })
                })
                .collect()
        })
        .collect()
}
pub(super) fn normalized_weights(weights: &[f64]) -> Result<Vec<f64>> {
    if weights.is_empty() || weights.iter().any(|v| !v.is_finite() || *v < 0.) {
        return Err(parameter());
    }
    let scale = weights.iter().copied().fold(0., f64::max);
    if scale <= 0. {
        return Err(parameter());
    }
    let total = weights.iter().map(|w| w / scale).sum::<f64>();
    Ok(weights.iter().map(|w| (w / scale) / total).collect())
}
pub(super) fn moments(x: &[f64]) -> (f64, f64) {
    let mean = x.iter().map(|v| v / x.len() as f64).sum::<f64>();
    let sd = (x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (x.len() - 1) as f64).sqrt();
    (mean, sd)
}

pub(super) fn supplied_weights(values: &[f64], criteria: usize) -> Result<Vec<f64>> {
    if values.is_empty() {
        Ok(vec![1. / criteria as f64; criteria])
    } else if values.len() == criteria {
        normalized_weights(values)
    } else {
        Err(parameter())
    }
}
/// Unit-interval utilities, with constant min–max columns consistently neutralized.
pub(super) fn utilities(
    columns: &[Vec<f64>],
    costs: &[bool],
    rescale: bool,
    control: &Control,
) -> Result<Vec<Vec<f64>>> {
    let mut result = normalize(
        columns,
        if rescale {
            Normalization::MinMax
        } else {
            Normalization::None
        },
        control,
    )?;
    for (j, x) in result.iter_mut().enumerate() {
        control.check()?;
        if x.iter().any(|v| !(0.0..=1.0).contains(v)) {
            return Err(parameter());
        }
        let constant = rescale && x.iter().all(|v| *v == 0.);
        if costs[j] && !constant {
            for v in x {
                *v = 1. - *v;
            }
        }
    }
    Ok(result)
}
