//! Meta plots reuse the existing coefficient and XY payloads.
use super::{data::*, model};
use yss_sci_contract::visualization::*;

pub fn forest(
    y: &[f64],
    v: &[f64],
    options: MetaOptions,
    exponentiate: bool,
    control: &Control,
) -> Result<CoefficientPlot> {
    let result = model::fit(y, v, &[], options, control)?;
    let convert = |v: f64| finite(if exponentiate { v.exp() } else { v });
    let mut data = result
        .studies
        .iter()
        .map(|s| {
            Ok(CoefficientPoint {
                label: format!("Study {}", s.effect.study),
                value: convert(s.effect.effect)?,
                lower: convert(s.effect.lower)?,
                upper: convert(s.effect.upper)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let pooled = &result.summary.coefficients[0];
    let ci = pooled.confidence_interval.ok_or_else(failed)?;
    data.push(CoefficientPoint {
        label: "Pooled".into(),
        value: convert(pooled.estimate)?,
        lower: convert(ci[0])?,
        upper: convert(ci[1])?,
    });
    control.check()?;
    Ok(CoefficientPlot {
        data,
        confidence_level: options.confidence_level,
    })
}
pub fn funnel(
    y: &[f64],
    v: &[f64],
    estimator: MetaEstimator,
    confidence: f64,
    control: &Control,
) -> Result<FunnelPlot> {
    let center = model::pooled_estimate(y, v, estimator, confidence, control)?;
    let se = v.iter().map(|v| v.sqrt()).collect::<Vec<_>>();
    let mut plot = crate::visualization::xy(y, &se, false, control)?;
    let high = se.iter().copied().fold(0., f64::max);
    let q = critical(confidence, None)?;
    plot.x_label = "Effect".into();
    plot.y_label = "Standard error".into();
    plot.reference_lines = [-1., 0., 1.]
        .into_iter()
        .map(|direction| {
            Ok(ReferenceLine {
                start: PlotPoint { x: center, y: 0. },
                end: PlotPoint {
                    x: finite(center + direction * q * high)?,
                    y: high,
                },
            })
        })
        .collect::<Result<_>>()?;
    Ok(FunnelPlot {
        plot,
        y_domain: [finite(high * 1.05)?, 0.],
    })
}
