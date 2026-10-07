//! Effect sizes use independent study summaries; orientation is treatment minus control.
use super::data::*;
use statrs::function::gamma::ln_gamma;

fn construct(
    n: usize,
    measure: EffectMeasure,
    level: f64,
    control: &Control,
    calculate: impl Fn(usize) -> Result<(f64, f64)>,
) -> Result<EffectResult> {
    let q = critical(level, None)?;
    let mut rows = Vec::with_capacity(n);
    for i in 0..n {
        control.check()?;
        let (effect, variance) = calculate(i)?;
        rows.push(study(i, effect, variance, q)?);
    }
    Ok(EffectResult {
        summary: EffectSummary {
            measure,
            studies: n,
            confidence_level: level,
        },
        rows,
    })
}
pub fn continuous(
    treatment: ArmSummary<'_>,
    reference: ArmSummary<'_>,
    measure: EffectMeasure,
    confidence: f64,
    control: &Control,
) -> Result<EffectResult> {
    let n = aligned(
        &[
            treatment.mean,
            treatment.sd,
            treatment.size,
            reference.mean,
            reference.sd,
            reference.size,
        ],
        control,
    )?;
    if !matches!(
        measure,
        EffectMeasure::MeanDifference | EffectMeasure::HedgesG
    ) {
        return Err(parameter());
    }
    construct(n, measure, confidence, control, |i| {
        let (nt, nc, st, sc) = (
            treatment.size[i],
            reference.size[i],
            treatment.sd[i],
            reference.sd[i],
        );
        size(nt, 2.)?;
        size(nc, 2.)?;
        if st < 0. || sc < 0. {
            return Err(parameter());
        }
        let difference = treatment.mean[i] - reference.mean[i];
        if measure == EffectMeasure::MeanDifference {
            return Ok((difference, st * st / nt + sc * sc / nc));
        }
        let df = nt + nc - 2.;
        let pooled = (((nt - 1.) * st * st + (nc - 1.) * sc * sc) / df).sqrt();
        let correction =
            (ln_gamma(df / 2.) - 0.5 * (df / 2.).ln() - ln_gamma((df - 1.) / 2.)).exp();
        let g = correction * difference / pooled;
        // Hedges (1982) large-sample sampling variance, metafor's vtype="LS".
        Ok((g, 1. / nt + 1. / nc + g * g / (2. * (nt + nc))))
    })
}
pub fn binary(
    treatment: BinomialSummary<'_>,
    reference: BinomialSummary<'_>,
    measure: EffectMeasure,
    correction: f64,
    confidence: f64,
    control: &Control,
) -> Result<EffectResult> {
    let n = aligned(
        &[
            treatment.events,
            treatment.total,
            reference.events,
            reference.total,
        ],
        control,
    )?;
    if !correction.is_finite()
        || correction < 0.
        || !matches!(
            measure,
            EffectMeasure::LogOddsRatio
                | EffectMeasure::LogRiskRatio
                | EffectMeasure::RiskDifference
        )
    {
        return Err(parameter());
    }
    construct(n, measure, confidence, control, |i| {
        let mut cells = [
            treatment.events[i],
            treatment.total[i] - treatment.events[i],
            reference.events[i],
            reference.total[i] - reference.events[i],
        ];
        size(treatment.total[i], 1.)?;
        size(reference.total[i], 1.)?;
        for &c in &cells {
            size(c, 0.)?;
        }
        if cells.contains(&0.) {
            for c in &mut cells {
                *c += correction;
            }
        }
        let [a, b, c, d] = cells;
        let (nt, nc) = (a + b, c + d);
        match measure {
            EffectMeasure::LogOddsRatio => Ok((
                a.ln() + d.ln() - b.ln() - c.ln(),
                1. / a + 1. / b + 1. / c + 1. / d,
            )),
            EffectMeasure::LogRiskRatio => Ok((
                (a / nt).ln() - (c / nc).ln(),
                1. / a - 1. / nt + 1. / c - 1. / nc,
            )),
            _ => Ok((a / nt - c / nc, a * b / nt.powi(3) + c * d / nc.powi(3))),
        }
    })
}
pub fn proportion(
    data: BinomialSummary<'_>,
    measure: EffectMeasure,
    correction: f64,
    confidence: f64,
    control: &Control,
) -> Result<EffectResult> {
    let n = aligned(&[data.events, data.total], control)?;
    if !correction.is_finite()
        || correction < 0.
        || !matches!(
            measure,
            EffectMeasure::Proportion
                | EffectMeasure::LogitProportion
                | EffectMeasure::ArcsineProportion
        )
    {
        return Err(parameter());
    }
    construct(n, measure, confidence, control, |i| {
        let (mut x, mut total) = (data.events[i], data.total[i]);
        size(total, 1.)?;
        size(x, 0.)?;
        if x > total {
            return Err(parameter());
        }
        if measure != EffectMeasure::ArcsineProportion && (x == 0. || x == total) {
            x += correction;
            total += 2. * correction;
        }
        let p = x / total;
        match measure {
            EffectMeasure::Proportion => Ok((p, p * (1. - p) / total)),
            EffectMeasure::LogitProportion => {
                Ok((p.ln() - (-p).ln_1p(), 1. / x + 1. / (total - x)))
            }
            _ => Ok((p.sqrt().asin(), 1. / (4. * total))),
        }
    })
}
pub fn mean(data: ArmSummary<'_>, confidence: f64, control: &Control) -> Result<EffectResult> {
    let n = aligned(&[data.mean, data.sd, data.size], control)?;
    construct(n, EffectMeasure::Mean, confidence, control, |i| {
        size(data.size[i], 2.)?;
        if data.sd[i] <= 0. {
            return Err(parameter());
        }
        Ok((data.mean[i], data.sd[i].powi(2) / data.size[i]))
    })
}
pub fn correlation(
    r: &[f64],
    sample_size: &[f64],
    confidence: f64,
    control: &Control,
) -> Result<EffectResult> {
    let n = aligned(&[r, sample_size], control)?;
    construct(n, EffectMeasure::FisherZ, confidence, control, |i| {
        size(sample_size[i], 4.)?;
        if r[i].abs() >= 1. {
            return Err(parameter());
        }
        Ok((r[i].atanh(), 1. / (sample_size[i] - 3.)))
    })
}
pub fn ratio(
    ratio: &[f64],
    lower: &[f64],
    upper: &[f64],
    source_confidence: f64,
    confidence: f64,
    control: &Control,
) -> Result<EffectResult> {
    let n = aligned(&[ratio, lower, upper], control)?;
    let q = critical(source_confidence, None)?;
    construct(n, EffectMeasure::LogRatio, confidence, control, |i| {
        if lower[i] <= 0. || lower[i] >= ratio[i] || upper[i] <= ratio[i] {
            return Err(parameter());
        }
        Ok((
            ratio[i].ln(),
            ((upper[i].ln() - lower[i].ln()) / (2. * q)).powi(2),
        ))
    })
}
