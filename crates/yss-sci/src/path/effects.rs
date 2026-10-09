//! Linear contrasts and observed-range Johnson–Neyman regions from one fitted covariance.
use super::*;
use statrs::distribution::{ContinuousCDF, StudentsT};

pub(super) fn covariance(a: &[f64], b: &[f64], matrix: &[Vec<f64>]) -> Result<f64> {
    finite(
        a.iter()
            .enumerate()
            .map(|(i, a)| {
                b.iter()
                    .enumerate()
                    .map(|(j, b)| a * b * matrix[i][j])
                    .sum::<f64>()
            })
            .sum(),
    )
}
pub(super) fn estimate(
    beta: &[f64],
    weights: &[f64],
    covariance_matrix: &[Vec<f64>],
    t: &StudentsT,
) -> Result<PathEffect> {
    let estimate = finite(beta.iter().zip(weights).map(|(b, w)| b * w).sum())?;
    let variance = covariance(weights, weights, covariance_matrix)?;
    let se = if variance >= 0. {
        Some(variance.sqrt())
    } else {
        None
    };
    let statistic = se
        .filter(|&s| s > 0.)
        .map(|s| finite(estimate / s))
        .transpose()?;
    let p_value = statistic.map(|v| {
        (crate::distribution::student_t_probability(
            t,
            v,
            yss_sci_contract::hypothesis::Alternative::TwoSided,
        ))
        .clamp(0., 1.)
    });
    let confidence_interval = se
        .map(|s| -> Result<_> {
            let width = t.inverse_cdf(0.975) * s;
            Ok([finite(estimate - width)?, finite(estimate + width)?])
        })
        .transpose()?;
    Ok(PathEffect {
        estimate,
        standard_error: se,
        statistic,
        p_value,
        confidence_interval,
    })
}

pub(super) struct AffineEffect {
    pub intercept: f64,
    pub slope: f64,
    pub variance_intercept: f64,
    pub covariance: f64,
    pub variance_slope: f64,
}
pub(super) fn johnson_neyman(
    effect: AffineEffect,
    t: &StudentsT,
    center: f64,
    observed_range: [f64; 2],
    second_moderator: Option<f64>,
) -> Result<JohnsonNeyman> {
    // Solve in units of the observed radius. Otherwise changing W's units can
    // make a genuinely quadratic equation look numerically linear.
    let radius = finite(
        (observed_range[0] - center)
            .abs()
            .max((observed_range[1] - center).abs()),
    )?;
    if radius <= 0. {
        return Err(parameter());
    }
    let effect = AffineEffect {
        intercept: effect.intercept,
        slope: finite(effect.slope * radius)?,
        variance_intercept: effect.variance_intercept,
        covariance: finite(effect.covariance * radius)?,
        variance_slope: finite((effect.variance_slope * radius) * radius)?,
    };
    let critical = t.inverse_cdf(0.975).powi(2);
    let a = finite(effect.slope.powi(2) - critical * effect.variance_slope)?;
    let b = finite(2. * (effect.intercept * effect.slope - critical * effect.covariance))?;
    let c = finite(effect.intercept.powi(2) - critical * effect.variance_intercept)?;
    let scale = a.abs().max(b.abs()).max(c.abs());
    let (a, b, c) = if scale > 0. {
        (a / scale, b / scale, c / scale)
    } else {
        (0., 0., 0.)
    };
    let roots = if a.abs() <= 64. * f64::EPSILON {
        if b.abs() <= 64. * f64::EPSILON {
            vec![]
        } else {
            vec![-c / b]
        }
    } else {
        let discriminant = b * b - 4. * a * c;
        if discriminant < 0. {
            vec![]
        } else {
            let q = -0.5 * (b + discriminant.sqrt().copysign(b));
            if q == 0. {
                vec![-b / (2. * a)]
            } else {
                vec![q / a, c / q]
            }
        }
    };
    let mut boundaries: Vec<_> = roots
        .into_iter()
        .map(|x| x * radius + center)
        .filter(|x| x.is_finite() && *x > observed_range[0] && *x < observed_range[1])
        .collect();
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup_by(|a, b| *a == *b);
    let mut points = vec![observed_range[0]];
    points.extend(&boundaries);
    points.push(observed_range[1]);
    let mut significant_ranges = Vec::new();
    for pair in points.windows(2) {
        let x = (pair[0] / 2. + pair[1] / 2. - center) / radius;
        let variance =
            effect.variance_intercept + 2. * x * effect.covariance + x * x * effect.variance_slope;
        let slope = effect.intercept + effect.slope * x;
        if variance > 0. && finite(slope * slope - critical * variance)? > 0. {
            significant_ranges.push([pair[0], pair[1]]);
        }
    }
    Ok(JohnsonNeyman {
        second_moderator,
        observed_range,
        boundaries,
        significant_ranges,
    })
}
