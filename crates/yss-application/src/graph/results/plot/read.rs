use super::{
    CartesianResultPlot, HistogramResultBin, HistogramResultPlot, PlotAnnotation,
    ResultPlotProjection,
};
use crate::{
    chart::{ChartPlotResult, PlotAxisFormat, PlotPoint},
    graph::results::ResultQueryApplicationError,
};
use std::collections::BTreeMap;
use yss_data_contract::TabularScalar;
use yss_graph_execution::plan::PlotDataKind;
use yss_node_kernel::RuntimeValue;
use yss_sci_contract::visualization::PlotMetadata;

type Record = BTreeMap<Box<str>, RuntimeValue>;
type Invalid = ResultQueryApplicationError;
fn invalid() -> Invalid {
    Invalid::UnrepresentableValue
}

pub(super) fn supports(kind: PlotDataKind) -> bool {
    matches!(
        kind,
        PlotDataKind::Scatter
            | PlotDataKind::Line
            | PlotDataKind::Ecdf
            | PlotDataKind::Kde
            | PlotDataKind::Histogram
            | PlotDataKind::Bubble
            | PlotDataKind::PpQq
            | PlotDataKind::Roc
            | PlotDataKind::Quadrant
    )
}

pub(super) fn project(
    kind: PlotDataKind,
    value: &RuntimeValue,
) -> Result<ResultPlotProjection, Invalid> {
    let record = object(value)?;
    let rows = list(field(record, "data")?)?;
    if rows.is_empty() {
        return Err(invalid());
    }
    if kind == PlotDataKind::Histogram {
        return Ok(ResultPlotProjection::Histogram(HistogramResultPlot {
            bins: rows
                .iter()
                .map(|row| {
                    let row = object(row)?;
                    Ok(HistogramResultBin {
                        label: text(field(row, "label")?)?.into(),
                        count: count(field(row, "count")?)?,
                    })
                })
                .collect::<Result<_, Invalid>>()?,
            x_label: optional_text(record, "xLabel")?,
            y_label: optional_text(record, "yLabel")?,
            observations: record.get("observations").map(count).transpose()?,
        }));
    }
    let data = rows.iter().map(point).collect::<Result<Vec<_>, _>>()?;
    let reference_lines = record
        .get("referenceLines")
        .map(|value| {
            list(value)?
                .iter()
                .map(|line| {
                    let line = object(line)?;
                    Ok([point(field(line, "start")?)?, point(field(line, "end")?)?])
                })
                .collect::<Result<Vec<_>, Invalid>>()
        })
        .transpose()?
        .unwrap_or_default();
    let point_sizes = if kind == PlotDataKind::Bubble {
        Some(
            rows.iter()
                .map(|row| {
                    let size = number(field(object(row)?, "size")?)?;
                    if size < 0. {
                        return Err(invalid());
                    }
                    Ok(size)
                })
                .collect::<Result<_, Invalid>>()?,
        )
    } else {
        None
    };
    let y_domain = record
        .get("yDomain")
        .map(|value| {
            let domain = list(value)?;
            if domain.len() != 2 {
                return Err(invalid());
            }
            let result = [number(&domain[0])?, number(&domain[1])?];
            if result[0] == result[1] {
                return Err(invalid());
            }
            Ok(result)
        })
        .transpose()?;
    let annotation = annotation(kind, record, &data)?;
    Ok(ResultPlotProjection::Cartesian(CartesianResultPlot {
        kind,
        series: ChartPlotResult {
            data,
            x_label: optional_text(record, "xLabel")?,
            y_label: optional_text(record, "yLabel")?,
            x_format: axis_format(record.get("xFormat"))?,
            y_format: axis_format(record.get("yFormat"))?,
        },
        reference_lines,
        point_sizes,
        y_domain,
        metadata: record.get("metadata").map(metadata).transpose()?,
        annotation,
    }))
}

fn annotation(
    kind: PlotDataKind,
    raw: &Record,
    data: &[PlotPoint],
) -> Result<PlotAnnotation, Invalid> {
    match kind {
        PlotDataKind::PpQq => {
            let pp = match text(field(raw, "mode")?)? {
                "pp" => true,
                "qq" => false,
                _ => return Err(invalid()),
            };
            let mean = number(field(raw, "referenceMean")?)?;
            let standard_deviation = number(field(raw, "referenceStandardDeviation")?)?;
            if standard_deviation <= 0. {
                return Err(invalid());
            }
            Ok(PlotAnnotation::Probability {
                pp,
                mean,
                standard_deviation,
            })
        }
        PlotDataKind::Roc => {
            let auc = number(field(raw, "auc")?)?;
            let positives = count(field(raw, "positives")?)?;
            let negatives = count(field(raw, "negatives")?)?;
            if !(0.0..=1.0).contains(&auc)
                || positives == 0
                || negatives == 0
                || data.first() != Some(&PlotPoint { x: 0., y: 0. })
                || data.last() != Some(&PlotPoint { x: 1., y: 1. })
                || data
                    .iter()
                    .any(|p| !(0.0..=1.0).contains(&p.x) || !(0.0..=1.0).contains(&p.y))
                || data.windows(2).any(|p| p[0].x > p[1].x || p[0].y > p[1].y)
            {
                return Err(invalid());
            }
            Ok(PlotAnnotation::Roc {
                auc,
                positives,
                negatives,
            })
        }
        PlotDataKind::Quadrant => {
            number(field(raw, "xCut")?)?;
            number(field(raw, "yCut")?)?;
            let values = list(field(raw, "counts")?)?;
            let counts = values
                .iter()
                .map(count)
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| invalid())?;
            Ok(PlotAnnotation::Quadrant { counts })
        }
        _ => Ok(PlotAnnotation::None),
    }
}

fn metadata(value: &RuntimeValue) -> Result<PlotMetadata, Invalid> {
    let raw = object(value)?;
    let observations = count(field(raw, "observations")?)?;
    let displayed = count(field(raw, "displayed")?)?;
    let RuntimeValue::Scalar(TabularScalar::Bool(sampled)) = field(raw, "sampled")?.unannotated()
    else {
        return Err(invalid());
    };
    if observations == 0 || displayed == 0 {
        return Err(invalid());
    }
    Ok(PlotMetadata {
        observations,
        displayed,
        sampled: *sampled,
    })
}
fn point(value: &RuntimeValue) -> Result<PlotPoint, Invalid> {
    let raw = object(value)?;
    Ok(PlotPoint {
        x: number(field(raw, "x")?)?,
        y: number(field(raw, "y")?)?,
    })
}
fn object(value: &RuntimeValue) -> Result<&Record, Invalid> {
    match value.unannotated() {
        RuntimeValue::Record(value) => Ok(value),
        _ => Err(invalid()),
    }
}
fn list(value: &RuntimeValue) -> Result<&[RuntimeValue], Invalid> {
    match value.unannotated() {
        RuntimeValue::List(value) => Ok(value),
        _ => Err(invalid()),
    }
}
fn field<'a>(record: &'a Record, key: &str) -> Result<&'a RuntimeValue, Invalid> {
    record.get(key).ok_or_else(invalid)
}
fn number(value: &RuntimeValue) -> Result<f64, Invalid> {
    match value.unannotated() {
        RuntimeValue::Scalar(TabularScalar::Float64(value)) => Ok(value.as_f64()),
        RuntimeValue::Scalar(TabularScalar::Integer(value))
            if value.unsigned_abs() <= (1_u64 << 53) =>
        {
            Ok(*value as f64)
        }
        RuntimeValue::Scalar(TabularScalar::Unsigned(value)) if *value <= (1_u64 << 53) => {
            Ok(*value as f64)
        }
        _ => Err(invalid()),
    }
}
fn count(value: &RuntimeValue) -> Result<usize, Invalid> {
    match value.unannotated() {
        RuntimeValue::Scalar(TabularScalar::Integer(value)) => {
            usize::try_from(*value).map_err(|_| invalid())
        }
        RuntimeValue::Scalar(TabularScalar::Unsigned(value)) => {
            usize::try_from(*value).map_err(|_| invalid())
        }
        _ => Err(invalid()),
    }
}
fn text(value: &RuntimeValue) -> Result<&str, Invalid> {
    match value.unannotated() {
        RuntimeValue::Scalar(TabularScalar::String(value)) => Ok(value),
        _ => Err(invalid()),
    }
}
fn optional_text(raw: &Record, key: &str) -> Result<Option<Box<str>>, Invalid> {
    raw.get(key)
        .map(|value| text(value).map(Into::into))
        .transpose()
}
fn axis_format(value: Option<&RuntimeValue>) -> Result<PlotAxisFormat, Invalid> {
    match value.map(text).transpose()? {
        None | Some("number") => Ok(PlotAxisFormat::Number),
        Some("date") => Ok(PlotAxisFormat::Date),
        Some("datetime") => Ok(PlotAxisFormat::Datetime),
        _ => Err(invalid()),
    }
}
