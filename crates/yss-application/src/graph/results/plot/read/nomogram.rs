use super::*;
use yss_sci_contract::survival::{NomogramAxis, NomogramPlot, NomogramTick};

pub(super) fn project(raw: &Record) -> Result<ResultPlotProjection, Invalid> {
    let axes = rows(raw, "axes", 4)?
        .iter()
        .map(|axis| {
            let axis = object(axis)?;
            let ticks = rows(axis, "ticks", 1)?
                .iter()
                .map(|tick| {
                    let tick = object(tick)?;
                    let position = number(field(tick, "position")?)?;
                    if !(0.0..=1.0).contains(&position) {
                        return Err(invalid());
                    }
                    Ok(NomogramTick {
                        position,
                        label: text(field(tick, "label")?)?.into(),
                    })
                })
                .collect::<Result<_, Invalid>>()?;
            Ok(NomogramAxis {
                label: text(field(axis, "label")?)?.into(),
                ticks,
            })
        })
        .collect::<Result<_, Invalid>>()?;
    Ok(ResultPlotProjection::Nomogram(NomogramPlot {
        axes,
        horizon: positive(field(raw, "horizon")?)?,
        maximum_total_points: positive(field(raw, "maximumTotalPoints")?)?,
        points_per_log_hazard: positive(field(raw, "pointsPerLogHazard")?)?,
        baseline_cumulative_hazard: positive(field(raw, "baselineCumulativeHazard")?)?,
    }))
}
