//! Model domains, minimum identifiable sample sizes and sample-unit accounting.
use super::*;
// Integer-search neighbors must remain distinguishable in floating-point formulas.
pub(super) const MAX_EXACT_SIZE: usize = if usize::BITS < 53 {
    usize::MAX
} else {
    9_007_199_254_740_991_u64 as usize
};
fn positive(v: f64) -> bool {
    v.is_finite() && v > 0.
}
fn probability(v: f64) -> bool {
    v.is_finite() && (0. ..=1.).contains(&v)
}
pub(super) fn validate(model: PowerModel, options: PowerOptions) -> Result<usize> {
    if !options.alpha.is_finite() || options.alpha <= 0. || options.alpha >= 1. {
        return Err(parameter());
    }
    let (valid, minimum) = match model {
        PowerModel::NormalMean {
            standardized_effect,
        } => (standardized_effect.is_finite(), 1),
        PowerModel::TMean {
            standardized_effect,
            ..
        } => (standardized_effect.is_finite(), 2),
        PowerModel::Variance { variance_ratio } => (positive(variance_ratio), 2),
        PowerModel::Proportion {
            null_proportion,
            proportion,
        } => (
            positive(null_proportion) && null_proportion < 1. && probability(proportion),
            1,
        ),
        PowerModel::ProportionDifference {
            proportion1,
            proportion2,
        } => (
            probability(proportion1)
                && probability(proportion2)
                && (proportion1 + proportion2) > 0.
                && (proportion1 + proportion2) < 2.,
            1,
        ),
        PowerModel::Correlation {
            null_correlation,
            correlation,
        } => (
            null_correlation.is_finite()
                && correlation.is_finite()
                && null_correlation.abs() < 1.
                && correlation.abs() < 1.,
            4,
        ),
        PowerModel::Anova { groups, effect_f } => (
            (2..=MAX_EXACT_SIZE).contains(&groups)
                && effect_f.is_finite()
                && effect_f >= 0.
                && options.alternative == PowerAlternative::Greater,
            2,
        ),
        PowerModel::LinearRegression {
            predictors,
            effect_f_squared,
        } => (
            predictors > 0
                && effect_f_squared.is_finite()
                && effect_f_squared >= 0.
                && options.alternative == PowerAlternative::Greater,
            predictors.checked_add(2).ok_or_else(parameter)?,
        ),
        PowerModel::PoissonRate {
            baseline_rate,
            rate_ratio,
            exposure,
        } => (
            positive(baseline_rate) && positive(rate_ratio) && positive(exposure),
            1,
        ),
        PowerModel::Logistic {
            baseline_probability,
            odds_ratio,
        } => (
            positive(baseline_probability) && baseline_probability < 1. && positive(odds_ratio),
            1,
        ),
        PowerModel::Survival {
            hazard_ratio,
            event_fraction,
            predictor_variance,
        } => (
            positive(hazard_ratio)
                && positive(event_fraction)
                && event_fraction <= 1.
                && positive(predictor_variance),
            1,
        ),
        PowerModel::ClusterRandomized {
            standardized_effect,
            cluster_size,
            intraclass_correlation,
        } => (
            standardized_effect.is_finite()
                && cluster_size > 0
                && cluster_size <= MAX_EXACT_SIZE
                && probability(intraclass_correlation),
            2,
        ),
        PowerModel::Noninferiority {
            standardized_difference,
            margin,
            ..
        } => (
            standardized_difference.is_finite()
                && positive(margin)
                && options.alternative == PowerAlternative::Greater,
            1,
        ),
        PowerModel::Equivalence {
            standardized_difference,
            margin,
        } => (
            standardized_difference.is_finite()
                && positive(margin)
                && options.alternative == PowerAlternative::TwoSided,
            1,
        ),
    };
    if !valid || minimum > MAX_EXACT_SIZE {
        return Err(parameter());
    }
    Ok(minimum)
}
pub(super) fn increasing(model: PowerModel, alternative: PowerAlternative) -> bool {
    let difference = match model {
        PowerModel::NormalMean {
            standardized_effect,
        }
        | PowerModel::TMean {
            standardized_effect,
            ..
        }
        | PowerModel::ClusterRandomized {
            standardized_effect,
            ..
        } => standardized_effect,
        PowerModel::Variance { variance_ratio } => variance_ratio - 1.,
        PowerModel::Proportion {
            null_proportion,
            proportion,
        } => proportion - null_proportion,
        PowerModel::ProportionDifference {
            proportion1,
            proportion2,
        } => proportion1 - proportion2,
        PowerModel::Correlation {
            null_correlation,
            correlation,
        } => correlation - null_correlation,
        PowerModel::Anova { effect_f, .. } => return effect_f > 0.,
        PowerModel::LinearRegression {
            effect_f_squared, ..
        } => return effect_f_squared > 0.,
        PowerModel::PoissonRate { rate_ratio, .. } => rate_ratio.ln(),
        PowerModel::Logistic { odds_ratio, .. } => odds_ratio.ln(),
        PowerModel::Survival { hazard_ratio, .. } => hazard_ratio.ln(),
        PowerModel::Noninferiority {
            standardized_difference,
            margin,
            higher_is_better,
        } => {
            return (if higher_is_better {
                standardized_difference
            } else {
                -standardized_difference
            }) > -margin;
        }
        PowerModel::Equivalence {
            standardized_difference,
            margin,
        } => return standardized_difference.abs() < margin,
    };
    match alternative {
        PowerAlternative::TwoSided => difference != 0.,
        PowerAlternative::Greater => difference > 0.,
        PowerAlternative::Less => difference < 0.,
    }
}
pub(super) fn layout(model: PowerModel, n: usize) -> Result<(&'static str, usize)> {
    let (unit, multiplier) = match model {
        PowerModel::TMean {
            design: MeanDesign::Independent,
            ..
        }
        | PowerModel::ProportionDifference { .. }
        | PowerModel::PoissonRate { .. }
        | PowerModel::Logistic { .. }
        | PowerModel::Noninferiority { .. }
        | PowerModel::Equivalence { .. } => ("per_group", 2),
        PowerModel::TMean {
            design: MeanDesign::Paired,
            ..
        } => ("pairs", 2),
        PowerModel::Anova { groups, .. } => ("per_group", groups),
        PowerModel::ClusterRandomized { cluster_size, .. } => (
            "clusters_per_group",
            cluster_size.checked_mul(2).ok_or_else(parameter)?,
        ),
        _ => ("observations", 1),
    };
    Ok((
        unit,
        n.checked_mul(multiplier)
            .filter(|&n| n <= MAX_EXACT_SIZE)
            .ok_or_else(parameter)?,
    ))
}
