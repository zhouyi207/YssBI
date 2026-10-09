//! Paired correlation and inter-rater agreement with cooperative execution control.
mod agreement;
mod correlation;
mod ridit;
pub use agreement::{bland_altman, icc, kappa, kendall_w, rwg};
pub use correlation::{kendall, partial, pearson, spearman};
pub use ridit::ridit;

use statrs::distribution::{ContinuousCDF, Normal, StudentsT};
use yss_sci_contract::association::{ConfidenceInterval, TestInference};
use yss_sci_contract::execution::{
    ScientificComputationError as Error, ScientificExecutionControl as Control,
    ScientificInputViolation as Violation,
};
use yss_sci_contract::hypothesis::Alternative;

fn invalid(violation: Violation) -> Error {
    Error::InvalidInput { violation }
}
fn finite(value: f64) -> Result<f64, Error> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::ComputationFailed)
    }
}
fn bounded(value: f64, low: f64, high: f64) -> Result<f64, Error> {
    let value = finite(value)?;
    if value < low - 1e-12 || value > high + 1e-12 {
        return Err(Error::ComputationFailed);
    }
    Ok(value.clamp(low, high))
}
fn checkpoint(control: &Control, index: usize) -> Result<(), Error> {
    if index.is_multiple_of(1024) {
        control.check()?;
    }
    Ok(())
}
fn validate(values: &[f64], minimum: usize, control: &Control) -> Result<(), Error> {
    control.check()?;
    if values.len() < minimum {
        return Err(Error::InvalidInput {
            violation: Violation::EmptyInput,
        });
    }
    for (i, &value) in values.iter().enumerate() {
        checkpoint(control, i)?;
        if !value.is_finite() {
            return Err(Error::InvalidInput {
                violation: Violation::NonFiniteInput,
            });
        }
    }
    Ok(())
}
fn paired(x: &[f64], y: &[f64], minimum: usize, control: &Control) -> Result<(), Error> {
    control.check()?;
    if x.len() != y.len() {
        return Err(Error::InvalidInput {
            violation: Violation::ShapeMismatch,
        });
    }
    validate(x, minimum, control)?;
    validate(y, minimum, control)
}
fn matrix(columns: &[Vec<f64>], minimum_columns: usize, control: &Control) -> Result<usize, Error> {
    control.check()?;
    if columns.len() < minimum_columns {
        return Err(invalid(Violation::ShapeMismatch));
    }
    let rows = columns[0].len();
    for column in columns {
        if column.len() != rows {
            return Err(Error::InvalidInput {
                violation: Violation::ShapeMismatch,
            });
        }
        validate(column, 2, control)?;
    }
    Ok(rows)
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
fn sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut sum = Sum::default();
    for value in values {
        sum.add(value);
    }
    sum.total
}
fn centered(values: &[f64], control: &Control) -> Result<(Vec<f64>, f64, f64), Error> {
    validate(values, 2, control)?;
    let scale = values.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);
    let scale = if scale == 0.0 { 1.0 } else { scale };
    let mean = sum(values.iter().map(|x| x / scale)) / values.len() as f64;
    let mut output = Vec::with_capacity(values.len());
    for (i, &value) in values.iter().enumerate() {
        checkpoint(control, i)?;
        output.push(value / scale - mean);
    }
    Ok((output, scale, mean))
}
fn unit_vector(values: &[f64], control: &Control) -> Result<Vec<f64>, Error> {
    let (mut values, _, _) = centered(values, control)?;
    let norm = finite(sum(values.iter().map(|x| x * x)).sqrt())?;
    if norm <= 0.0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    for value in &mut values {
        *value /= norm;
    }
    control.check()?;
    Ok(values)
}
fn coefficient(x: &[f64], y: &[f64], control: &Control) -> Result<f64, Error> {
    let x = unit_vector(x, control)?;
    let y = unit_vector(y, control)?;
    bounded(sum(x.iter().zip(&y).map(|(x, y)| x * y)), -1.0, 1.0)
}
fn level(value: f64) -> Result<(), Error> {
    if value.is_finite() && value > 0.0 && value < 1.0 {
        Ok(())
    } else {
        Err(invalid(Violation::ParameterOutOfRange))
    }
}
fn alternative(value: Alternative) -> &'static str {
    match value {
        Alternative::TwoSided => "two_sided",
        Alternative::Greater => "greater",
        Alternative::Less => "less",
    }
}
fn normal_tail(z: f64, alternative: Alternative) -> Result<f64, Error> {
    let distribution = Normal::new(0.0, 1.0).map_err(|_| Error::ComputationFailed)?;
    bounded(
        match alternative {
            Alternative::TwoSided => 2.0 * distribution.sf(z.abs()),
            Alternative::Greater => distribution.sf(z),
            Alternative::Less => distribution.cdf(z),
        },
        0.0,
        1.0,
    )
}
fn normal_quantile(confidence: f64) -> Result<f64, Error> {
    level(confidence)?;
    finite(
        Normal::new(0.0, 1.0)
            .map_err(|_| Error::ComputationFailed)?
            .inverse_cdf((1.0 + confidence) / 2.0),
    )
}
fn student_test(
    r: f64,
    df: usize,
    alternative: Alternative,
    method: &'static str,
) -> Result<TestInference, Error> {
    if df == 0 {
        return Err(invalid(Violation::EmptyInput));
    }
    let (statistic, p_value) = if r.abs() == 1.0 {
        let p = match alternative {
            Alternative::TwoSided => 0.0,
            Alternative::Greater => {
                if r > 0.0 {
                    0.0
                } else {
                    1.0
                }
            }
            Alternative::Less => {
                if r < 0.0 {
                    0.0
                } else {
                    1.0
                }
            }
        };
        (None, p)
    } else {
        let statistic = finite(r * ((df as f64) / ((1.0 - r) * (1.0 + r))).sqrt())?;
        let distribution =
            StudentsT::new(0.0, 1.0, df as f64).map_err(|_| Error::ComputationFailed)?;
        let p = crate::distribution::student_t_probability(&distribution, statistic, alternative);
        (Some(statistic), bounded(p, 0.0, 1.0)?)
    };
    Ok(TestInference {
        statistic_name: "t",
        statistic,
        degrees_of_freedom: vec![df as f64],
        p_value: Some(p_value),
        method,
    })
}
fn fisher_interval(
    r: f64,
    n: usize,
    controls: usize,
    confidence: f64,
) -> Result<Option<ConfidenceInterval>, Error> {
    level(confidence)?;
    if n <= controls + 3 {
        return Ok(None);
    }
    let (lower, upper) = if r.abs() == 1.0 {
        (r, r)
    } else {
        let delta = normal_quantile(confidence)? / ((n - controls - 3) as f64).sqrt();
        let z = r.atanh();
        ((z - delta).tanh(), (z + delta).tanh())
    };
    Ok(Some(ConfidenceInterval {
        lower,
        upper,
        level: confidence,
        method: "fisher_z",
    }))
}
pub(crate) struct Ranks {
    pub(crate) values: Vec<f64>,
    tie_pairs: u64,
    tie_cubic: f64,
    tie_variance: f64,
}
pub(crate) fn ranks(values: &[f64], control: &Control) -> Result<Ranks, Error> {
    validate(values, 1, control)?;
    let mut order = (0..values.len()).collect::<Vec<_>>();
    order.sort_unstable_by(|&a, &b| values[a].total_cmp(&values[b]));
    control.check()?;
    let mut output = vec![0.0; values.len()];
    let mut pairs = 0u64;
    let mut cubic = Sum::default();
    let mut variance = Sum::default();
    let mut first = 0;
    while first < order.len() {
        checkpoint(control, first)?;
        let mut last = first + 1;
        while last < order.len() && values[order[last]] == values[order[first]] {
            checkpoint(control, last)?;
            last += 1;
        }
        let rank = (first + last + 1) as f64 / 2.0;
        for &row in &order[first..last] {
            output[row] = rank;
        }
        let count = (last - first) as u64;
        pairs += count * (count - 1) / 2;
        let t = count as f64;
        cubic.add(t * (t - 1.0) * (t - 2.0));
        variance.add(t * (t - 1.0) * (2.0 * t + 5.0));
        first = last;
    }
    control.check()?;
    Ok(Ranks {
        values: output,
        tie_pairs: pairs,
        tie_cubic: cubic.total,
        tie_variance: variance.total,
    })
}

#[cfg(test)]
mod tests;
