use super::KernelFragment;
use crate::node_system::plan::ResourceId;
use crate::node_system::protocol::Value;
use crate::node_system::runtime::{
    Kernel, KernelContext, KernelError, NullPolicy, NumericSeriesView, ResourceLease, RuntimeValue,
    numeric_series, require_data_series,
};
use crate::sci::api::time_series::acf_pacf::{AcfPacfInput, compute_acf_pacf};
use crate::sci::engine::SciContext;
use serde::Serialize;
use statrs::distribution::{ChiSquared, ContinuousCDF, StudentsT};
use std::any::Any;
use std::sync::Arc;

pub(crate) const PLOT_SINK: &str = "yssbi.runtime.plot_sink";
const KDE_GRID_POINTS: usize = 256;
const DEFAULT_MAX_LAG: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlotKind {
    Scatter,
    Line,
    Ecdf,
    Kde,
    Histogram,
    Correlation,
    Correlogram,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlotPublishError(pub Box<str>);

impl PlotPublishError {
    pub fn new(message: impl Into<Box<str>>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for PlotPublishError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PlotPublishError {}

pub trait PlotSink: Send + Sync {
    fn publish(&self, kind: PlotKind, payload: &str) -> Result<Box<str>, PlotPublishError>;
}

pub struct PlotSinkResource {
    resource_id: ResourceId,
    sink: Arc<dyn PlotSink>,
}

impl PlotSinkResource {
    pub fn new(sink: Arc<dyn PlotSink>) -> Self {
        Self {
            resource_id: ResourceId::new(PLOT_SINK).expect("plot sink resource id"),
            sink,
        }
    }
}

impl ResourceLease for PlotSinkResource {
    fn resource_id(&self) -> &ResourceId {
        &self.resource_id
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone, Copy)]
struct KernelSpec {
    handle: &'static str,
    kind: PlotKind,
}

const KERNELS: &[KernelSpec] = &[
    KernelSpec {
        handle: "yssbi.plot.scatter.view",
        kind: PlotKind::Scatter,
    },
    KernelSpec {
        handle: "yssbi.plot.line.view",
        kind: PlotKind::Line,
    },
    KernelSpec {
        handle: "yssbi.plot.ecdf.view",
        kind: PlotKind::Ecdf,
    },
    KernelSpec {
        handle: "yssbi.plot.kde.view",
        kind: PlotKind::Kde,
    },
    KernelSpec {
        handle: "yssbi.plot.histogram.view",
        kind: PlotKind::Histogram,
    },
    KernelSpec {
        handle: "yssbi.plot.correlation.view",
        kind: PlotKind::Correlation,
    },
    KernelSpec {
        handle: "yssbi.plot.correlogram.view",
        kind: PlotKind::Correlogram,
    },
];

pub(crate) fn build_kernel_fragment() -> KernelFragment {
    let mut fragment = KernelFragment::default();
    for spec in KERNELS {
        fragment.register(spec.handle, PlotKernel { kind: spec.kind });
    }
    fragment
}

struct PlotKernel {
    kind: PlotKind,
}

impl Kernel for PlotKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        context
            .cancellation
            .check()
            .map_err(|error| KernelError::cancelled(error.to_string()))?;
        let payload = match self.kind {
            PlotKind::Scatter => pair_payload(inputs, "Scatter")?,
            PlotKind::Line => pair_payload(inputs, "Line")?,
            PlotKind::Ecdf => ecdf_payload(inputs)?,
            PlotKind::Kde => kde_payload(inputs)?,
            PlotKind::Histogram => histogram_payload(inputs)?,
            PlotKind::Correlation => correlation_payload(inputs)?,
            PlotKind::Correlogram => correlogram_payload(inputs)?,
        };
        context
            .cancellation
            .check()
            .map_err(|error| KernelError::cancelled(error.to_string()))?;
        let resource_id = ResourceId::new(PLOT_SINK).expect("plot sink resource id");
        let resource = context
            .resources
            .get(&resource_id)
            .ok_or_else(|| KernelError::new("required plot-sink resource is unavailable"))?;
        let sink = if let Some(resource) = resource.as_any().downcast_ref::<PlotSinkResource>() {
            resource.sink.as_ref()
        } else if let Some(resource) = resource
            .as_any()
            .downcast_ref::<crate::node_system::runtime::ProjectResourceLease>(
        ) {
            resource
                .plot_sink()
                .ok_or_else(|| KernelError::new("plot-sink project resource has no sink adapter"))?
        } else {
            return Err(KernelError::new(
                "plot-sink resource has an incompatible adapter type",
            ));
        };
        let result = sink
            .publish(self.kind, &payload)
            .map_err(|error| KernelError::new(format!("plot publication failed: {error}")))?;
        Ok(vec![RuntimeValue::Scalar(Value::String(result))])
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PairPlotData {
    data: Vec<PairPoint>,
    x_label: Option<String>,
    y_label: Option<String>,
    x_format: String,
    y_format: String,
}

#[derive(Serialize)]
struct PairPoint {
    x: f64,
    y: f64,
}

fn pair_payload(inputs: &[RuntimeValue], name: &str) -> Result<String, KernelError> {
    expect_arity(inputs, 2, name)?;
    let x = series(inputs, 0, name)?;
    let y = series(inputs, 1, name)?;
    let data = x
        .values
        .iter()
        .zip(&y.values)
        .filter_map(|(x, y)| Some(PairPoint { x: (*x)?, y: (*y)? }))
        .collect::<Vec<_>>();
    if data.is_empty() {
        return Err(KernelError::new(format!(
            "{name}: no valid paired observations"
        )));
    }
    serialize(
        name,
        &PairPlotData {
            data,
            x_label: x.name,
            y_label: y.name,
            x_format: x.format,
            y_format: y.format,
        },
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct XYPlotData {
    data: Vec<XYPoint>,
    x_label: Option<String>,
    y_label: Option<String>,
}

#[derive(Serialize)]
struct XYPoint {
    x: f64,
    y: f64,
}

fn ecdf_payload(inputs: &[RuntimeValue]) -> Result<String, KernelError> {
    expect_arity(inputs, 1, "ECDF")?;
    let series = series(inputs, 0, "ECDF")?;
    let mut values = finite_values(&series);
    if values.is_empty() {
        return Err(KernelError::new("ECDF: no valid numeric values"));
    }
    values.sort_by(f64::total_cmp);
    let count = values.len() as f64;
    let data = values
        .into_iter()
        .enumerate()
        .map(|(index, x)| XYPoint {
            x,
            y: (index + 1) as f64 / count,
        })
        .collect();
    serialize(
        "ECDF",
        &XYPlotData {
            data,
            x_label: series.name,
            y_label: Some("Cumulative Proportion".to_owned()),
        },
    )
}

fn kde_payload(inputs: &[RuntimeValue]) -> Result<String, KernelError> {
    expect_arity(inputs, 1, "KDE")?;
    let series = series(inputs, 0, "KDE")?;
    let values = finite_values(&series);
    if values.len() < 2 {
        return Err(KernelError::new(
            "KDE: at least two valid numeric values are required",
        ));
    }
    let data = crate::sci::kde::gaussian_kde_grid(&values, KDE_GRID_POINTS)
        .into_iter()
        .map(|point| XYPoint {
            x: point.x,
            y: point.density,
        })
        .collect();
    serialize(
        "KDE",
        &XYPlotData {
            data,
            x_label: series.name,
            y_label: Some("Density".to_owned()),
        },
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HistogramPlotData {
    data: Vec<HistogramBin>,
    x_label: Option<String>,
    y_label: Option<String>,
}

#[derive(Serialize)]
struct HistogramBin {
    label: String,
    count: u32,
}

fn histogram_payload(inputs: &[RuntimeValue]) -> Result<String, KernelError> {
    expect_arity(inputs, 1, "Histogram")?;
    let series = series(inputs, 0, "Histogram")?;
    let values = finite_values(&series);
    if values.is_empty() {
        return Err(KernelError::new(
            "Histogram: at least one valid numeric value is required",
        ));
    }
    let bin_count = sturges_bins(values.len());
    let minimum = values.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let width = if maximum > minimum {
        (maximum - minimum) / bin_count as f64
    } else {
        1.0
    };
    let mut counts = vec![0_u32; bin_count];
    for value in values {
        let index = if value <= minimum {
            0
        } else if value >= maximum {
            bin_count - 1
        } else {
            (((value - minimum) / width).floor() as usize).min(bin_count - 1)
        };
        counts[index] = counts[index].saturating_add(1);
    }
    let data = counts
        .into_iter()
        .enumerate()
        .map(|(index, count)| HistogramBin {
            label: format!(
                "[{:.2}, {:.2})",
                minimum + index as f64 * width,
                minimum + (index + 1) as f64 * width
            ),
            count,
        })
        .collect();
    serialize(
        "Histogram",
        &HistogramPlotData {
            data,
            x_label: series.name,
            y_label: Some("Frequency".to_owned()),
        },
    )
}

fn sturges_bins(count: usize) -> usize {
    ((count as f64).log2().ceil() as usize + 1).clamp(1, 100)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CorrelationPlotData {
    labels: Vec<String>,
    matrix: Vec<Vec<Option<f64>>>,
    p_matrix: Vec<Vec<Option<f64>>>,
}

fn correlation_payload(inputs: &[RuntimeValue]) -> Result<String, KernelError> {
    if inputs.len() < 2 {
        return Err(KernelError::new(
            "Correlation Plot: at least two data series are required",
        ));
    }
    let series = inputs
        .iter()
        .enumerate()
        .map(|(index, _)| series(inputs, index, "Correlation Plot"))
        .collect::<Result<Vec<_>, _>>()?;
    let labels = series
        .iter()
        .enumerate()
        .map(|(index, series)| {
            series
                .name
                .clone()
                .unwrap_or_else(|| format!("Series {}", index + 1))
        })
        .collect::<Vec<_>>();
    let size = series.len();
    let mut matrix = vec![vec![None; size]; size];
    let mut p_matrix = vec![vec![None; size]; size];
    for row in 0..size {
        for column in 0..size {
            let pairs = series[row]
                .values
                .iter()
                .zip(&series[column].values)
                .filter_map(|(left, right)| Some(((*left)?, (*right)?)))
                .collect::<Vec<_>>();
            let (left, right): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();
            let correlation = pearson_correlation(&left, &right);
            matrix[row][column] = correlation.is_finite().then_some(correlation);
            let p_value = pearson_p_value(correlation, left.len());
            p_matrix[row][column] = p_value.is_finite().then_some(p_value);
        }
    }
    serialize(
        "Correlation Plot",
        &CorrelationPlotData {
            labels,
            matrix,
            p_matrix,
        },
    )
}

fn pearson_correlation(left: &[f64], right: &[f64]) -> f64 {
    let count = left.len().min(right.len());
    if count == 0 {
        return f64::NAN;
    }
    let left_mean = left.iter().take(count).sum::<f64>() / count as f64;
    let right_mean = right.iter().take(count).sum::<f64>() / count as f64;
    let (mut covariance, mut left_variance, mut right_variance) = (0.0, 0.0, 0.0);
    for index in 0..count {
        let left_delta = left[index] - left_mean;
        let right_delta = right[index] - right_mean;
        covariance += left_delta * right_delta;
        left_variance += left_delta * left_delta;
        right_variance += right_delta * right_delta;
    }
    let denominator = (left_variance * right_variance).sqrt();
    if denominator > 0.0 {
        covariance / denominator
    } else {
        f64::NAN
    }
}

fn pearson_p_value(correlation: f64, count: usize) -> f64 {
    if count < 3 || !correlation.is_finite() {
        return f64::NAN;
    }
    if correlation.abs() >= 1.0 {
        return 0.0;
    }
    let degrees_of_freedom = (count - 2) as f64;
    let statistic =
        correlation.abs() * (degrees_of_freedom / (1.0 - correlation * correlation)).sqrt();
    StudentsT::new(0.0, 1.0, degrees_of_freedom)
        .map(|distribution| 2.0 * (1.0 - distribution.cdf(statistic)))
        .unwrap_or(f64::NAN)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CorrelogramPlotData {
    acf: Vec<CorrelogramDatum>,
    pacf: Vec<CorrelogramDatum>,
    ci_half_width: f64,
    n: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CorrelogramDatum {
    lag: usize,
    value: f64,
    q_stat: f64,
    p_value: f64,
}

#[cfg(test)]
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlotPayloadContractRecord {
    pub chart: &'static str,
    pub data: serde_json::Value,
}

#[cfg(test)]
pub(crate) fn plot_payload_contract_records() -> Vec<PlotPayloadContractRecord> {
    let pair = PairPlotData {
        data: vec![PairPoint { x: 1.0, y: 2.0 }],
        x_label: Some("x".to_owned()),
        y_label: Some("y".to_owned()),
        x_format: "number".to_owned(),
        y_format: "number".to_owned(),
    };
    let xy = XYPlotData {
        data: vec![XYPoint { x: 1.0, y: 0.5 }],
        x_label: Some("value".to_owned()),
        y_label: Some("density".to_owned()),
    };
    vec![
        PlotPayloadContractRecord {
            chart: "scatter",
            data: serde_json::to_value(&pair).unwrap(),
        },
        PlotPayloadContractRecord {
            chart: "line",
            data: serde_json::to_value(&pair).unwrap(),
        },
        PlotPayloadContractRecord {
            chart: "ecdf",
            data: serde_json::to_value(&xy).unwrap(),
        },
        PlotPayloadContractRecord {
            chart: "kde",
            data: serde_json::to_value(&xy).unwrap(),
        },
        PlotPayloadContractRecord {
            chart: "histogram",
            data: serde_json::to_value(HistogramPlotData {
                data: vec![HistogramBin {
                    label: "[0, 1)".to_owned(),
                    count: 1,
                }],
                x_label: Some("value".to_owned()),
                y_label: Some("Frequency".to_owned()),
            })
            .unwrap(),
        },
        PlotPayloadContractRecord {
            chart: "correlation",
            data: serde_json::to_value(CorrelationPlotData {
                labels: vec!["a".to_owned(), "b".to_owned()],
                matrix: vec![vec![Some(1.0), Some(0.5)], vec![Some(0.5), Some(1.0)]],
                p_matrix: vec![vec![Some(0.0), Some(0.25)], vec![Some(0.25), Some(0.0)]],
            })
            .unwrap(),
        },
        PlotPayloadContractRecord {
            chart: "correlogram",
            data: serde_json::to_value(CorrelogramPlotData {
                acf: vec![CorrelogramDatum {
                    lag: 1,
                    value: 0.5,
                    q_stat: 1.5,
                    p_value: 0.2,
                }],
                pacf: vec![CorrelogramDatum {
                    lag: 1,
                    value: 0.4,
                    q_stat: 1.5,
                    p_value: 0.2,
                }],
                ci_half_width: 0.196,
                n: 100,
            })
            .unwrap(),
        },
    ]
}

#[cfg(test)]
impl PlotKind {
    pub(crate) fn payload_contract_records() -> impl Serialize {
        plot_payload_contract_records()
    }
}

fn correlogram_payload(inputs: &[RuntimeValue]) -> Result<String, KernelError> {
    expect_arity(inputs, 2, "Correlogram")?;
    let series = series(inputs, 0, "Correlogram")?;
    let values = finite_values(&series);
    if values.len() < 4 {
        return Err(KernelError::new(
            "Correlogram: at least four observations are required",
        ));
    }
    let maximum_lag = positive_integer(inputs, 1).unwrap_or(DEFAULT_MAX_LAG);
    let result = compute_acf_pacf(
        &SciContext::rust(),
        AcfPacfInput {
            residuals: values.clone(),
            max_lag: maximum_lag,
        },
    )
    .map_err(|error| KernelError::new(format!("Correlogram: {error}")))?;
    let acf = result.acf.get(1..).unwrap_or_default();
    let statistics = cumulative_ljung_box(acf, values.len());
    let acf = acf
        .iter()
        .zip(&statistics)
        .enumerate()
        .map(|(index, (value, (q_stat, p_value)))| CorrelogramDatum {
            lag: index + 1,
            value: *value,
            q_stat: *q_stat,
            p_value: *p_value,
        })
        .collect();
    let pacf = result
        .pacf
        .iter()
        .zip(&statistics)
        .enumerate()
        .map(|(index, (value, (q_stat, p_value)))| CorrelogramDatum {
            lag: index + 1,
            value: *value,
            q_stat: *q_stat,
            p_value: *p_value,
        })
        .collect();
    serialize(
        "Correlogram",
        &CorrelogramPlotData {
            acf,
            pacf,
            ci_half_width: 1.96 / (values.len() as f64).sqrt(),
            n: values.len(),
        },
    )
}

fn cumulative_ljung_box(acf: &[f64], count: usize) -> Vec<(f64, f64)> {
    let sample_size = count as f64;
    let mut sum = 0.0;
    acf.iter()
        .enumerate()
        .map(|(index, correlation)| {
            let lag = index + 1;
            sum += correlation.powi(2) / count.saturating_sub(lag).max(1) as f64;
            let statistic = sample_size * (sample_size + 2.0) * sum;
            let p_value = ChiSquared::new(lag as f64)
                .map(|distribution| 1.0 - distribution.cdf(statistic))
                .unwrap_or(f64::NAN);
            (statistic, p_value)
        })
        .collect()
}

struct NumericSeries {
    name: Option<String>,
    format: String,
    values: Vec<Option<f64>>,
}

fn series(inputs: &[RuntimeValue], index: usize, node: &str) -> Result<NumericSeries, KernelError> {
    let value = inputs
        .get(index)
        .ok_or_else(|| KernelError::new(format!("{node}: input {index} is missing")))?;
    let artifact = require_data_series(value)?;
    let view = numeric_series(artifact, NullPolicy::Propagate)?;
    let metadata = match &view {
        NumericSeriesView::Int64(series) => series.metadata(),
        NumericSeriesView::Float64(series) => series.metadata(),
    }
    .clone();
    let values = match view {
        NumericSeriesView::Int64(series) => series
            .values()
            .iter()
            .map(|value| value.map(|value| value as f64))
            .collect(),
        NumericSeriesView::Float64(series) => series.values().to_vec(),
    };
    Ok(NumericSeries {
        name: metadata.name.as_deref().map(str::to_owned),
        format: metadata.format.as_deref().unwrap_or("number").to_owned(),
        values,
    })
}

fn finite_values(series: &NumericSeries) -> Vec<f64> {
    series
        .values
        .iter()
        .filter_map(|value| *value)
        .filter(|value| value.is_finite())
        .collect()
}

fn positive_integer(inputs: &[RuntimeValue], index: usize) -> Option<usize> {
    let RuntimeValue::Scalar(value) = inputs.get(index)? else {
        return None;
    };
    let value = match value {
        Value::Integer(value) if *value > 0 => *value as usize,
        Value::Unsigned(value) if *value > 0 => usize::try_from(*value).ok()?,
        Value::Decimal(value) => {
            let value = value.as_str().parse::<f64>().ok()?;
            if value > 0.0 {
                value as usize
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some(value)
}

fn expect_arity(inputs: &[RuntimeValue], expected: usize, node: &str) -> Result<(), KernelError> {
    if inputs.len() == expected {
        Ok(())
    } else {
        Err(KernelError::new(format!(
            "{node}: received {} inputs; expected {expected}",
            inputs.len()
        )))
    }
}

fn serialize(name: &str, payload: &impl Serialize) -> Result<String, KernelError> {
    serde_json::to_string(payload)
        .map_err(|error| KernelError::new(format!("{name}: serialization failed: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn kernel_fragment_matches_current_plot_catalog_inventory() {
        let node_system = crate::node_system::catalog::build_builtin_node_system().unwrap();
        let catalog_handles = node_system
            .registry
            .iter()
            .map(|(id, _)| id.as_str())
            .filter(|handle| handle.starts_with("yssbi.plot.") && handle.ends_with(".view"))
            .collect::<BTreeSet<_>>();
        let spec_handles = KERNELS
            .iter()
            .map(|spec| spec.handle)
            .collect::<BTreeSet<_>>();
        let fragment = build_kernel_fragment();
        let fragment_handles = fragment
            .handles()
            .map(|handle| handle.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(fragment_handles, spec_handles);
        assert_eq!(fragment_handles, catalog_handles);
    }

    #[test]
    fn plot_sink_resource_uses_declared_resource_id() {
        struct Sink;
        impl PlotSink for Sink {
            fn publish(&self, _: PlotKind, _: &str) -> Result<Box<str>, PlotPublishError> {
                Ok("presentation:test".into())
            }
        }
        let resource = PlotSinkResource::new(Arc::new(Sink));
        assert_eq!(resource.resource_id().as_str(), PLOT_SINK);
    }
}
