use super::*;
use yss_sci_contract::visualization::*;

fn labels(raw: &Record, key: &str, minimum: usize) -> Result<Vec<String>, Invalid> {
    rows(raw, key, minimum)?
        .iter()
        .map(|v| text(v).map(String::from))
        .collect()
}
fn matrix<T>(
    value: &RuntimeValue,
    height: usize,
    width: usize,
    cell: impl Fn(&RuntimeValue) -> Result<T, Invalid>,
) -> Result<Vec<Vec<T>>, Invalid> {
    let rows = list(value)?;
    if rows.len() != height {
        return Err(invalid());
    }
    rows.iter()
        .map(|row| {
            let row = list(row)?;
            if row.len() != width {
                return Err(invalid());
            }
            row.iter().map(&cell).collect()
        })
        .collect()
}
pub(super) fn correlation(raw: &Record) -> Result<ResultPlotProjection, Invalid> {
    let labels = labels(raw, "labels", 2)?;
    let size = labels.len();
    Ok(ResultPlotProjection::Correlation(CorrelationPlot {
        matrix: matrix(field(raw, "matrix")?, size, size, nullable_number)?,
        p_matrix: matrix(field(raw, "pMatrix")?, size, size, |v| {
            let value = nullable_number(v)?;
            if value.is_some_and(|p| !(0.0..=1.0).contains(&p)) {
                return Err(invalid());
            }
            Ok(value)
        })?,
        labels,
        observations: positive_count(field(raw, "observations")?)?,
    }))
}
pub(super) fn heatmap(raw: &Record) -> Result<ResultPlotProjection, Invalid> {
    let x_labels = labels(raw, "xLabels", 1)?;
    let y_labels = labels(raw, "yLabels", 1)?;
    Ok(ResultPlotProjection::Heatmap(HeatmapPlot {
        matrix: matrix(
            field(raw, "matrix")?,
            y_labels.len(),
            x_labels.len(),
            number,
        )?,
        x_labels,
        y_labels,
        metadata: metadata(field(raw, "metadata")?)?,
    }))
}
pub(super) fn correlogram(raw: &Record) -> Result<ResultPlotProjection, Invalid> {
    let series = |key: &str, inference: bool| -> Result<Vec<CorrelogramPoint>, Invalid> {
        rows(raw, key, 1)?
            .iter()
            .map(|value| {
                let row = object(value)?;
                let q_stat = nullable_number(field(row, "qStat")?)?;
                let p_value = nullable_number(field(row, "pValue")?)?;
                if (inference && (q_stat.is_none() || p_value.is_none()))
                    || q_stat.is_some_and(|q| q < 0.)
                    || p_value.is_some_and(|p| !(0.0..=1.0).contains(&p))
                {
                    return Err(invalid());
                }
                Ok(CorrelogramPoint {
                    lag: count(field(row, "lag")?)?,
                    value: number(field(row, "value")?)?,
                    q_stat,
                    p_value,
                })
            })
            .collect()
    };
    Ok(ResultPlotProjection::Correlogram(CorrelogramPlot {
        acf: series("acf", true)?,
        pacf: series("pacf", false)?,
        ci_half_width: positive(field(raw, "ciHalfWidth")?)?,
        n: positive_count(field(raw, "n")?)?,
    }))
}
pub(super) fn distribution(raw: &Record, violin: bool) -> Result<ResultPlotProjection, Invalid> {
    let groups = rows(raw, "groups", 1)?
        .iter()
        .map(|value| {
            let row = object(value)?;
            let group = DistributionGroup {
                label: text(field(row, "label")?)?.into(),
                observations: positive_count(field(row, "observations")?)?,
                lower_whisker: number(field(row, "lowerWhisker")?)?,
                q1: number(field(row, "q1")?)?,
                median: number(field(row, "median")?)?,
                q3: number(field(row, "q3")?)?,
                upper_whisker: number(field(row, "upperWhisker")?)?,
                outliers: rows(row, "outliers", 0)?
                    .iter()
                    .map(number)
                    .collect::<Result<_, _>>()?,
                outlier_count: count(field(row, "outlierCount")?)?,
                density: rows(row, "density", if violin { 2 } else { 0 })?
                    .iter()
                    .map(|v| {
                        let point = point(v)?;
                        Ok(yss_sci_contract::visualization::PlotPoint {
                            x: point.x,
                            y: point.y,
                        })
                    })
                    .collect::<Result<_, Invalid>>()?,
            };
            if [
                group.lower_whisker,
                group.q1,
                group.median,
                group.q3,
                group.upper_whisker,
            ]
            .windows(2)
            .any(|p| p[0] > p[1])
                || group.outlier_count < group.outliers.len()
                || group.density.iter().any(|p| p.y < 0.)
            {
                return Err(invalid());
            }
            Ok(group)
        })
        .collect::<Result<_, Invalid>>()?;
    Ok(ResultPlotProjection::Distribution {
        plot: DistributionPlot { groups },
        violin,
    })
}
pub(super) fn interval(raw: &Record) -> Result<ResultPlotProjection, Invalid> {
    let data = rows(raw, "data", 1)?
        .iter()
        .map(|value| {
            let row = object(value)?;
            let p = IntervalPoint {
                x: number(field(row, "x")?)?,
                y: number(field(row, "y")?)?,
                lower: number(field(row, "lower")?)?,
                upper: number(field(row, "upper")?)?,
            };
            if p.lower > p.y || p.y > p.upper {
                return Err(invalid());
            }
            Ok(p)
        })
        .collect::<Result<_, Invalid>>()?;
    Ok(ResultPlotProjection::Interval(IntervalPlot {
        data,
        metadata: metadata(field(raw, "metadata")?)?,
    }))
}
pub(super) fn coefficient(raw: &Record) -> Result<ResultPlotProjection, Invalid> {
    let confidence_level = positive(field(raw, "confidenceLevel")?)?;
    if confidence_level >= 1. {
        return Err(invalid());
    }
    let data = rows(raw, "data", 1)?
        .iter()
        .map(|value| {
            let row = object(value)?;
            let p = CoefficientPoint {
                label: text(field(row, "label")?)?.into(),
                value: number(field(row, "value")?)?,
                lower: number(field(row, "lower")?)?,
                upper: number(field(row, "upper")?)?,
            };
            if p.lower > p.value || p.value > p.upper {
                return Err(invalid());
            }
            Ok(p)
        })
        .collect::<Result<_, Invalid>>()?;
    Ok(ResultPlotProjection::Coefficient(CoefficientPlot {
        data,
        confidence_level,
    }))
}
