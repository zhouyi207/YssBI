//! Study-level measurements and explicit effect-scale selection.
use super::*;

pub(super) fn execute(
    method: &str,
    data: &[Vec<f64>],
    inv: &KernelInvocation<'_>,
    control: &Control,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let confidence = number(inv, "confidence_level")?;
    let arm = |offset: usize| ArmSummary {
        mean: &data[offset],
        sd: &data[offset + 1],
        size: &data[offset + 2],
    };
    let binomial = |offset: usize| BinomialSummary {
        events: &data[offset],
        total: &data[offset + 1],
    };
    let result = match method {
        "continuous" => sci::effects::continuous(
            arm(0),
            arm(3),
            match text(inv, "effect_measure")? {
                "mean_difference" => EffectMeasure::MeanDifference,
                "hedges_g" => EffectMeasure::HedgesG,
                _ => return Err(KernelError::InvalidParameter),
            },
            confidence,
            control,
        ),
        "binary" => sci::effects::binary(
            binomial(0),
            binomial(2),
            match text(inv, "effect_measure")? {
                "log_odds_ratio" => EffectMeasure::LogOddsRatio,
                "log_risk_ratio" => EffectMeasure::LogRiskRatio,
                "risk_difference" => EffectMeasure::RiskDifference,
                _ => return Err(KernelError::InvalidParameter),
            },
            number(inv, "continuity_correction")?,
            confidence,
            control,
        ),
        "single_proportion" => sci::effects::proportion(
            binomial(0),
            match text(inv, "effect_measure")? {
                "proportion" => EffectMeasure::Proportion,
                "logit_proportion" => EffectMeasure::LogitProportion,
                "arcsine_proportion" => EffectMeasure::ArcsineProportion,
                _ => return Err(KernelError::InvalidParameter),
            },
            number(inv, "continuity_correction")?,
            confidence,
            control,
        ),
        "mean" => sci::effects::mean(arm(0), confidence, control),
        "correlation" => sci::effects::correlation(&data[0], &data[1], confidence, control),
        "or_hr" => sci::effects::ratio(
            &data[0],
            &data[1],
            &data[2],
            number(inv, "source_confidence_level")?,
            confidence,
            control,
        ),
        _ => return Err(KernelError::InvalidParameter),
    }
    .map_err(computation_error)?;
    Ok(vec![
        value(result.summary, inv)?,
        output::effects(&result.rows, inv)?,
    ])
}
