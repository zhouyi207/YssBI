use crate::regression::models::common::{Result, failed, finite, parameter};
use statrs::distribution::{ContinuousCDF, Normal, StudentsT};
use yss_sci_contract::{execution::*, inference::*};

pub(crate) fn validate_confidence(confidence: f64) -> Result<()> {
    if !confidence.is_finite() || confidence <= 0. || confidence >= 1. {
        return Err(parameter());
    }
    Ok(())
}
pub(crate) fn critical(confidence: f64, df: Option<f64>) -> Result<f64> {
    validate_confidence(confidence)?;
    match df {
        // Central normal probability is erf(z / sqrt(2)); avoid rounding a CDF to 0.5 or 1.
        None => finite(std::f64::consts::SQRT_2 * statrs::function::erf::erf_inv(confidence)),
        Some(degrees) => {
            if !degrees.is_finite() || degrees <= 0. {
                return Err(parameter());
            }
            match crate::distribution::student_t_center_quantile(confidence, degrees) {
                Some(q) => finite(q),
                None => critical_tail((1. - confidence) / 2., df),
            }
        }
    }
}
pub(super) fn critical_tail(tail: f64, df: Option<f64>) -> Result<f64> {
    if !tail.is_finite() || tail <= 0. || tail >= 0.5 {
        return Err(parameter());
    }
    let q = match df {
        None => -Normal::new(0., 1.).map_err(|_| failed())?.inverse_cdf(tail),
        Some(df) if df.is_finite() && df > 0. => -StudentsT::new(0., 1., df)
            .map_err(|_| parameter())?
            .inverse_cdf(tail),
        Some(_) => return Err(parameter()),
    };
    finite(q)
}
pub fn calculate(
    estimates: &[f64],
    standard_errors: &[f64],
    options: IntervalOptions,
    control: &ScientificExecutionControl,
) -> Result<IntervalResult> {
    control.check()?;
    if estimates.is_empty() || estimates.len() != standard_errors.len() {
        return Err(ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::ShapeMismatch,
        });
    }
    let q = critical(options.confidence_level, options.degrees_of_freedom)?;
    let rows = estimates
        .iter()
        .zip(standard_errors)
        .enumerate()
        .map(|(i, (&estimate, &se))| {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            if !estimate.is_finite() || !se.is_finite() || se < 0. {
                return Err(parameter());
            }
            Ok(IntervalRow {
                index: i + 1,
                estimate,
                standard_error: se,
                lower: finite(estimate - q * se)?,
                upper: finite(estimate + q * se)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(IntervalResult {
        summary: IntervalSummary {
            rows: rows.len(),
            confidence_level: options.confidence_level,
            reference_distribution: if options.degrees_of_freedom.is_some() {
                "student_t"
            } else {
                "normal"
            },
            degrees_of_freedom: options.degrees_of_freedom,
        },
        rows,
    })
}
