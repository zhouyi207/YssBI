//! Inequality measures for individual observations or population-weighted group means.
use yss_sci_contract::execution::{
    ScientificComputationError as Error, ScientificExecutionControl, ScientificInputViolation,
};

/// Theil T with natural logarithms. Weights are population counts or shares;
/// absent weights give each observation equal mass. Zero weights contribute nothing.
pub fn theil_t(
    values: &[f64],
    weights: Option<&[f64]>,
    control: &ScientificExecutionControl,
) -> Result<f64, Error> {
    control.check()?;
    let invalid = |violation| Error::InvalidInput { violation };
    if values.is_empty() {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    if weights.is_some_and(|weights| weights.len() != values.len()) {
        return Err(invalid(ScientificInputViolation::ShapeMismatch));
    }
    let weight = |i: usize| weights.map_or(1.0, |weights| weights[i]);
    let mut max_log_weight = f64::NEG_INFINITY;
    let mut max_log_income = f64::NEG_INFINITY;
    for (i, &value) in values.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        let weight = weight(i);
        if !value.is_finite() || !weight.is_finite() {
            return Err(invalid(ScientificInputViolation::NonFiniteInput));
        }
        if value < 0.0 || weight < 0.0 {
            return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
        }
        if weight > 0.0 {
            let log_weight = weight.ln();
            max_log_weight = max_log_weight.max(log_weight);
            if value > 0.0 {
                max_log_income = max_log_income.max(log_weight + value.ln());
            }
        }
    }
    if !max_log_income.is_finite() {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }

    // Normalize population and income separately in log space: both w*x and
    // their sums can overflow even though the dimensionless index is finite.
    let mut population = Sum::default();
    let mut income = Sum::default();
    for (i, &value) in values.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        let weight = weight(i);
        if weight > 0.0 {
            let log_weight = weight.ln();
            population.add((log_weight - max_log_weight).exp());
            if value > 0.0 {
                income.add((log_weight + value.ln() - max_log_income).exp());
            }
        }
    }
    let log_population = population.total.ln();
    let log_income = income.total.ln();
    let mut index = Sum::default();
    for (i, &value) in values.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        let weight = weight(i);
        if weight > 0.0 && value > 0.0 {
            let log_weight = weight.ln();
            let log_p = (log_weight - max_log_weight) - log_population;
            let log_q = (log_weight + value.ln() - max_log_income) - log_income;
            index.add(log_q.exp() * (log_q - log_p));
        }
    }
    control.check()?;
    if !index.total.is_finite() || index.total < -1e-12 {
        return Err(Error::ComputationFailed);
    }
    // The exact index is nonnegative; absorb roundoff at equality.
    Ok(index.total.max(0.0))
}

#[derive(Default)]
struct Sum {
    total: f64,
    correction: f64,
}

impl Sum {
    fn add(&mut self, value: f64) {
        let adjusted = value - self.correction;
        let next = self.total + adjusted;
        self.correction = (next - self.total) - adjusted;
        self.total = next;
    }
}

#[cfg(test)]
mod tests;
