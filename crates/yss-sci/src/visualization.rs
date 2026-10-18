//! Controlled plot-data calculations over neutral slices and model facts.
mod categorical;
mod distribution;
mod matrix;
mod points;

pub use categorical::{combination, pareto, word_cloud};
pub use distribution::{box_violin, ecdf, histogram, kde};
pub use matrix::{correlation, correlogram, heatmap};
pub use points::{bubble, coefficients, error_bars, probability, quadrant, roc, xy};

use yss_sci_contract::execution::{
    ScientificComputationError, ScientificExecutionControl, ScientificInputViolation,
};
use yss_sci_contract::visualization::{MAX_PLOT_POINTS, PlotMetadata};

type Result<T> = std::result::Result<T, ScientificComputationError>;

fn invalid(violation: ScientificInputViolation) -> ScientificComputationError {
    ScientificComputationError::InvalidInput { violation }
}

fn validate(values: &[f64], control: &ScientificExecutionControl) -> Result<()> {
    control.check()?;
    if values.is_empty() {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    for (index, value) in values.iter().enumerate() {
        if index.is_multiple_of(1024) {
            control.check()?;
        }
        if !value.is_finite() {
            return Err(invalid(ScientificInputViolation::NonFiniteInput));
        }
    }
    Ok(())
}

fn aligned(columns: &[&[f64]], control: &ScientificExecutionControl) -> Result<usize> {
    let first = columns
        .first()
        .ok_or_else(|| invalid(ScientificInputViolation::EmptyInput))?;
    for values in columns {
        if values.len() != first.len() {
            return Err(invalid(ScientificInputViolation::ShapeMismatch));
        }
        validate(values, control)?;
    }
    Ok(first.len())
}

fn sample_indices(length: usize, limit: usize) -> Vec<usize> {
    if length <= limit {
        return (0..length).collect();
    }
    (0..limit)
        .map(|index| index * (length - 1) / (limit - 1))
        .collect()
}

fn metadata(length: usize, displayed: usize) -> PlotMetadata {
    PlotMetadata {
        observations: length,
        displayed,
        sampled: displayed < length,
    }
}

fn sample_metadata(length: usize) -> PlotMetadata {
    metadata(length, length.min(MAX_PLOT_POINTS))
}

fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ScientificComputationError::ComputationFailed)
    }
}

#[cfg(test)]
mod tests;
