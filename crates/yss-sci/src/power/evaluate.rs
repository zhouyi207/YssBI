//! Test-specific noncentralities and explicitly named planning approximations.
use super::distributions::{f_power, normal_power, t_power};
use super::*;
use crate::distribution::normal;
use statrs::distribution::{ChiSquared, ContinuousCDF};
pub(super) fn power(
    model: PowerModel,
    n: usize,
    alpha: f64,
    alternative: PowerAlternative,
    control: &Control,
) -> Result<f64> {
    control.check()?;
    design::layout(model, n)?;
    let n = n as f64;
    let value = match model {
        PowerModel::NormalMean {
            standardized_effect,
        } => normal_power(finite(standardized_effect * n.sqrt())?, alpha, alternative)?,
        PowerModel::TMean {
            standardized_effect,
            design,
        } => {
            let independent = design == MeanDesign::Independent;
            t_power(
                if independent { 2. * (n - 1.) } else { n - 1. },
                finite(standardized_effect * (n / if independent { 2. } else { 1. }).sqrt())?,
                alpha,
                alternative,
                control,
            )?
        }
        PowerModel::Variance { variance_ratio } => {
            variance(n, variance_ratio, alpha, alternative, control)?
        }
        PowerModel::Proportion {
            null_proportion: p0,
            proportion: p1,
        } => proportion(
            p1 - p0,
            p0 * (1. - p0),
            p1 * (1. - p1),
            n,
            alpha,
            alternative,
        )?,
        PowerModel::ProportionDifference {
            proportion1: p1,
            proportion2: p2,
        } => {
            let pooled = (p1 + p2) / 2.;
            proportion(
                p1 - p2,
                2. * pooled * (1. - pooled),
                p1 * (1. - p1) + p2 * (1. - p2),
                n,
                alpha,
                alternative,
            )?
        }
        PowerModel::Correlation {
            null_correlation,
            correlation,
        } => normal_power(
            finite((correlation.atanh() - null_correlation.atanh()) * (n - 3.).sqrt())?,
            alpha,
            alternative,
        )?,
        PowerModel::Anova { groups, effect_f } => f_power(
            (groups - 1) as f64,
            groups as f64 * (n - 1.),
            finite(groups as f64 * n * effect_f * effect_f)?,
            alpha,
            control,
        )?,
        PowerModel::LinearRegression {
            predictors,
            effect_f_squared,
        } => f_power(
            predictors as f64,
            n - predictors as f64 - 1.,
            finite(n * effect_f_squared)?,
            alpha,
            control,
        )?,
        PowerModel::PoissonRate {
            baseline_rate,
            rate_ratio,
            exposure,
        } => {
            let information = finite((baseline_rate * exposure) / (1. + 1. / rate_ratio))?;
            normal_power(
                finite(rate_ratio.ln() * (n * information).sqrt())?,
                alpha,
                alternative,
            )?
        }
        PowerModel::Logistic {
            baseline_probability: p0,
            odds_ratio,
        } => {
            let eta = (p0 / (1. - p0)).ln() + odds_ratio.ln();
            let p1 = if eta >= 0. {
                1. / (1. + (-eta).exp())
            } else {
                let e = eta.exp();
                e / (1. + e)
            };
            let variance = finite(1. / (p0 * (1. - p0)) + 1. / (p1 * (1. - p1)))?;
            normal_power(
                finite(odds_ratio.ln() * (n / variance).sqrt())?,
                alpha,
                alternative,
            )?
        }
        PowerModel::Survival {
            hazard_ratio,
            event_fraction,
            predictor_variance,
        } => normal_power(
            finite(hazard_ratio.ln() * (n * event_fraction * predictor_variance).sqrt())?,
            alpha,
            alternative,
        )?,
        PowerModel::ClusterRandomized {
            standardized_effect,
            cluster_size,
            intraclass_correlation,
        } => {
            let size = cluster_size as f64;
            let deff = 1. + (size - 1.) * intraclass_correlation;
            t_power(
                2. * (n - 1.),
                finite(standardized_effect * (n * size / (2. * deff)).sqrt())?,
                alpha,
                alternative,
                control,
            )?
        }
        PowerModel::Noninferiority {
            standardized_difference,
            margin,
            higher_is_better,
        } => {
            let delta = if higher_is_better {
                standardized_difference
            } else {
                -standardized_difference
            };
            normal_power(
                finite((delta + margin) * (n / 2.).sqrt())?,
                alpha,
                PowerAlternative::Greater,
            )?
        }
        PowerModel::Equivalence {
            standardized_difference,
            margin,
        } => {
            let z = crate::distribution::normal::upper_quantile(alpha);
            let lower = finite((-margin - standardized_difference) * (n / 2.).sqrt() + z)?;
            let upper = finite((margin - standardized_difference) * (n / 2.).sqrt() - z)?;
            if lower >= upper {
                0.
            } else if lower > 0. {
                normal::sf(lower) - normal::sf(upper)
            } else {
                normal::cdf(upper) - normal::cdf(lower)
            }
        }
    };
    control.check()?;
    Ok(finite(value)?.clamp(0., 1.))
}
fn proportion(
    delta: f64,
    null_variance: f64,
    variance: f64,
    n: f64,
    alpha: f64,
    alternative: PowerAlternative,
) -> Result<f64> {
    let z = if alternative == PowerAlternative::TwoSided {
        normal::two_sided_critical(alpha)
    } else {
        normal::upper_quantile(alpha)
    };
    let critical = z * (null_variance / n).sqrt();
    let statistic = |greater: bool| {
        let delta = if greater { delta } else { -delta };
        if variance == 0. {
            if delta > critical {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            }
        } else {
            (critical - delta) / (variance / n).sqrt()
        }
    };
    finite(match alternative {
        PowerAlternative::TwoSided => normal::sum_sf(statistic(true), statistic(false)),
        PowerAlternative::Greater => normal::sf(statistic(true)),
        PowerAlternative::Less => normal::sf(statistic(false)),
    })
}
fn variance(
    n: f64,
    ratio: f64,
    alpha: f64,
    alternative: PowerAlternative,
    control: &Control,
) -> Result<f64> {
    let chi = ChiSquared::new(n - 1.).map_err(|_| parameter())?;
    let tail = if alternative == PowerAlternative::TwoSided {
        alpha / 2.
    } else {
        alpha
    };
    let lo = chi.inverse_cdf(tail);
    let hi = if tail >= 1e-10 {
        chi.inverse_cdf(1. - tail)
    } else {
        // Invert survival directly when 1-alpha would round to one.
        let (mut lo, mut hi) = (0., n.max(1.));
        while chi.sf(hi) > tail {
            control.check()?;
            lo = hi;
            hi = finite(hi * 2.)?;
        }
        loop {
            control.check()?;
            let mid = lo / 2. + hi / 2.;
            if mid == lo || mid == hi {
                break hi;
            }
            if chi.sf(mid) > tail {
                lo = mid;
            } else {
                hi = mid;
            }
        }
    };
    finite(match alternative {
        PowerAlternative::TwoSided => chi.cdf(lo / ratio) + chi.sf(hi / ratio),
        PowerAlternative::Greater => chi.sf(hi / ratio),
        PowerAlternative::Less => chi.cdf(lo / ratio),
    })
}
pub(super) fn method(model: PowerModel) -> &'static str {
    match model {
        PowerModel::NormalMean { .. } => "normal_known_variance",
        PowerModel::TMean { .. } => "noncentral_t",
        PowerModel::Variance { .. } => "normal_population_chi_square",
        PowerModel::Proportion { .. } | PowerModel::ProportionDifference { .. } => {
            "normal_score_proportion_approximation"
        }
        PowerModel::Correlation { .. } => "fisher_z_normal_approximation",
        PowerModel::Anova { .. } | PowerModel::LinearRegression { .. } => "noncentral_f",
        PowerModel::PoissonRate { .. } => "poisson_log_rate_wald_approximation",
        PowerModel::Logistic { .. } => "binary_predictor_logistic_wald_approximation",
        PowerModel::Survival { .. } => "schoenfeld_normal_approximation",
        PowerModel::ClusterRandomized { .. } => "equal_cluster_size_t_approximation",
        PowerModel::Noninferiority { .. } => "known_variance_noninferiority_normal",
        PowerModel::Equivalence { .. } => "known_variance_tost_normal",
    }
}
