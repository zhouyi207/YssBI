use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use polars::prelude::{DataFrame, Float64Chunked};

use crate::application::bayes::{BayesArtifactReadError, BayesArtifactReader};
use crate::database::tabular_io::{read_ipc_dataframe, write_csv_dataframe};
use crate::sci::api::bayes::{
    AutocorrelationPlotData, AutocorrelationPoint, AutocorrelationSeries, DensityPlotData,
    DensityPoint, DensitySeries, PosteriorPredictivePage, PosteriorPredictiveRow,
    PosteriorPredictiveSummary, PosteriorSamplePage, PosteriorSampleRow, TracePlotData, TracePoint,
    TraceSeries,
};

pub(crate) struct PolarsBayesArtifactReader;

fn read_bayes_artifact_dataframe(
    path: &Path,
    _context: &'static str,
) -> Result<DataFrame, BayesArtifactReadError> {
    read_ipc_dataframe(path).map_err(|_| BayesArtifactReadError::Read)
}

fn samples_invalid(_source: impl fmt::Display) -> BayesArtifactReadError {
    BayesArtifactReadError::InvalidSamples
}

fn posterior_predictive_invalid(_source: impl fmt::Display) -> BayesArtifactReadError {
    BayesArtifactReadError::InvalidPosteriorPredictive
}

pub(crate) fn posterior_sample_page_from_dataframe(
    dataframe: &DataFrame,
    offset: usize,
    limit: usize,
    parameter: Option<&str>,
) -> Result<PosteriorSamplePage, BayesArtifactReadError> {
    let parameters = dataframe
        .column("parameter")
        .and_then(|column| column.str())
        .map_err(samples_invalid)?;
    let chains = dataframe
        .column("chain")
        .and_then(|column| column.i64())
        .map_err(samples_invalid)?;
    let draws = dataframe
        .column("draw")
        .and_then(|column| column.i64())
        .map_err(samples_invalid)?;
    let values = dataframe
        .column("value")
        .and_then(|column| column.f64())
        .map_err(samples_invalid)?;

    let mut matching_indices = Vec::new();
    for index in 0..dataframe.height() {
        let Some(row_parameter) = parameters.get(index) else {
            continue;
        };
        if parameter.is_none_or(|selected| selected == row_parameter) {
            matching_indices.push(index);
        }
    }

    let total = matching_indices.len();
    let rows = matching_indices
        .into_iter()
        .skip(offset)
        .take(limit)
        .filter_map(|index| {
            Some(PosteriorSampleRow {
                parameter: parameters.get(index)?.to_string(),
                chain: usize::try_from(chains.get(index)?).ok()?,
                draw: usize::try_from(draws.get(index)?).ok()?,
                value: values.get(index)?,
            })
        })
        .collect();

    Ok(PosteriorSamplePage {
        rows,
        offset,
        limit,
        total,
    })
}

pub(crate) fn posterior_predictive_page_from_dataframe(
    dataframe: &DataFrame,
    offset: usize,
    limit: usize,
) -> Result<PosteriorPredictivePage, BayesArtifactReadError> {
    let observations = dataframe
        .column("observation")
        .and_then(|column| column.i64())
        .map_err(posterior_predictive_invalid)?;
    let transforms = dataframe
        .column("response_transform")
        .and_then(|column| column.str())
        .map_err(posterior_predictive_invalid)?;
    let response_transform = transforms.get(0).unwrap_or("identity").to_string();
    if transforms
        .into_iter()
        .flatten()
        .any(|value| value != response_transform)
    {
        return Err(posterior_predictive_invalid(
            "posterior predictive rows contain inconsistent response transforms",
        ));
    }
    let observed_model = predictive_f64_column(dataframe, "observed_model")?;
    let mean_model = predictive_f64_column(dataframe, "mean_model")?;
    let q025_model = predictive_f64_column(dataframe, "q025_model")?;
    let q975_model = predictive_f64_column(dataframe, "q975_model")?;
    let observed_original = predictive_f64_column(dataframe, "observed_original")?;
    let mean_original = predictive_f64_column(dataframe, "mean_original")?;
    let q025_original = predictive_f64_column(dataframe, "q025_original")?;
    let q975_original = predictive_f64_column(dataframe, "q975_original")?;

    let total = dataframe.height();
    let rows = (offset..total.min(offset.saturating_add(limit)))
        .filter_map(|index| {
            Some(PosteriorPredictiveRow {
                observation: usize::try_from(observations.get(index)?).ok()?,
                model: PosteriorPredictiveSummary {
                    observed: observed_model.get(index)?,
                    mean: mean_model.get(index)?,
                    q025: q025_model.get(index)?,
                    q975: q975_model.get(index)?,
                },
                original: PosteriorPredictiveSummary {
                    observed: observed_original.get(index)?,
                    mean: mean_original.get(index)?,
                    q025: q025_original.get(index)?,
                    q975: q975_original.get(index)?,
                },
            })
        })
        .collect();

    Ok(PosteriorPredictivePage {
        rows,
        response_transform,
        offset,
        limit,
        total,
    })
}

fn predictive_f64_column<'a>(
    dataframe: &'a DataFrame,
    name: &str,
) -> Result<&'a Float64Chunked, BayesArtifactReadError> {
    dataframe
        .column(name)
        .and_then(|column| column.f64())
        .map_err(posterior_predictive_invalid)
}

pub(crate) fn trace_plot_data_from_dataframe(
    dataframe: &DataFrame,
    parameter: Option<&str>,
    max_points_per_chain: usize,
) -> Result<TracePlotData, BayesArtifactReadError> {
    let rows = sample_rows_from_dataframe(dataframe, parameter)?;
    let mut grouped: BTreeMap<(String, usize), Vec<TracePoint>> = BTreeMap::new();
    for row in rows {
        grouped
            .entry((row.parameter, row.chain))
            .or_default()
            .push(TracePoint {
                draw: row.draw,
                value: row.value,
            });
    }

    let mut stride = 1;
    let series = grouped
        .into_iter()
        .map(|((parameter, chain), mut points)| {
            points.sort_by_key(|point| point.draw);
            let local_stride = points.len().div_ceil(max_points_per_chain).max(1);
            stride = stride.max(local_stride);
            let points = points
                .into_iter()
                .enumerate()
                .filter_map(|(index, point)| (index % local_stride == 0).then_some(point))
                .collect();
            TraceSeries {
                parameter,
                chain,
                points,
            }
        })
        .collect();

    Ok(TracePlotData {
        series,
        max_points_per_chain,
        stride,
    })
}

pub(crate) fn density_plot_data_from_dataframe(
    dataframe: &DataFrame,
    parameter: Option<&str>,
    grid_points: usize,
) -> Result<DensityPlotData, BayesArtifactReadError> {
    let rows = sample_rows_from_dataframe(dataframe, parameter)?;
    let mut grouped: BTreeMap<String, BTreeMap<usize, Vec<f64>>> = BTreeMap::new();
    for row in rows {
        grouped
            .entry(row.parameter)
            .or_default()
            .entry(row.chain)
            .or_default()
            .push(row.value);
    }

    let mut series = Vec::new();
    for (parameter, chains) in grouped {
        let pooled = chains.values().flatten().copied().collect::<Vec<_>>();
        series.push(density_series(&parameter, None, &pooled, grid_points));
        series.extend(
            chains.into_iter().map(|(chain, values)| {
                density_series(&parameter, Some(chain), &values, grid_points)
            }),
        );
    }

    Ok(DensityPlotData {
        series,
        grid_points,
    })
}

fn density_series(
    parameter: &str,
    chain: Option<usize>,
    values: &[f64],
    grid_points: usize,
) -> DensitySeries {
    DensitySeries {
        parameter: parameter.to_string(),
        chain,
        points: crate::sci::api::density::compute_kernel_density(
            crate::sci::api::density::KernelDensityInput {
                values,
                grid_points,
                min_x: None,
            },
        )
        .points
        .into_iter()
        .map(|point| DensityPoint {
            x: point.x,
            density: point.density,
        })
        .collect(),
    }
}

pub(crate) fn autocorrelation_plot_data_from_dataframe(
    dataframe: &DataFrame,
    parameter: Option<&str>,
    max_lag: usize,
) -> Result<AutocorrelationPlotData, BayesArtifactReadError> {
    let rows = sample_rows_from_dataframe(dataframe, parameter)?;
    let mut grouped: BTreeMap<(String, usize), Vec<PosteriorSampleRow>> = BTreeMap::new();
    for row in rows {
        grouped
            .entry((row.parameter.clone(), row.chain))
            .or_default()
            .push(row);
    }

    let series = grouped
        .into_iter()
        .filter_map(|((parameter, chain), mut rows)| {
            rows.sort_by_key(|row| row.draw);
            let values = rows.into_iter().map(|row| row.value).collect::<Vec<_>>();
            let points = autocorrelation_points(&values, max_lag);
            (!points.is_empty()).then_some(AutocorrelationSeries {
                parameter,
                chain,
                points,
            })
        })
        .collect();

    Ok(AutocorrelationPlotData { series, max_lag })
}

fn autocorrelation_points(values: &[f64], max_lag: usize) -> Vec<AutocorrelationPoint> {
    let values = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if values.len() < 2 {
        return Vec::new();
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| {
            let centered = value - mean;
            centered * centered
        })
        .sum::<f64>();
    if variance <= f64::EPSILON {
        return Vec::new();
    }

    let max_lag = max_lag.min(values.len() - 1);
    (0..=max_lag)
        .map(|lag| {
            let covariance = values
                .iter()
                .take(values.len() - lag)
                .zip(values.iter().skip(lag))
                .map(|(left, right)| (left - mean) * (right - mean))
                .sum::<f64>();
            AutocorrelationPoint {
                lag,
                autocorrelation: covariance / variance,
            }
        })
        .collect()
}

fn sample_rows_from_dataframe(
    dataframe: &DataFrame,
    parameter: Option<&str>,
) -> Result<Vec<PosteriorSampleRow>, BayesArtifactReadError> {
    let page = posterior_sample_page_from_dataframe(dataframe, 0, usize::MAX, parameter)?;
    Ok(page.rows)
}

impl BayesArtifactReader for PolarsBayesArtifactReader {
    fn export_csv(&self, source: &Path, destination: &Path) -> Result<(), BayesArtifactReadError> {
        let mut dataframe = read_bayes_artifact_dataframe(source, "artifact")?;
        write_csv_dataframe(destination, &mut dataframe).map_err(|_| BayesArtifactReadError::Export)
    }

    fn sample_page(
        &self,
        source: &Path,
        offset: usize,
        limit: usize,
        parameter: Option<&str>,
    ) -> Result<PosteriorSamplePage, BayesArtifactReadError> {
        let dataframe = read_bayes_artifact_dataframe(source, "posterior samples")?;
        posterior_sample_page_from_dataframe(&dataframe, offset, limit, parameter)
    }

    fn trace_plot_data(
        &self,
        source: &Path,
        parameter: Option<&str>,
        max_points_per_chain: usize,
    ) -> Result<TracePlotData, BayesArtifactReadError> {
        let dataframe = read_bayes_artifact_dataframe(source, "posterior samples")?;
        trace_plot_data_from_dataframe(&dataframe, parameter, max_points_per_chain)
    }

    fn density_plot_data(
        &self,
        source: &Path,
        parameter: Option<&str>,
        grid_points: usize,
    ) -> Result<DensityPlotData, BayesArtifactReadError> {
        let dataframe = read_bayes_artifact_dataframe(source, "posterior samples")?;
        density_plot_data_from_dataframe(&dataframe, parameter, grid_points)
    }

    fn autocorrelation_plot_data(
        &self,
        source: &Path,
        parameter: Option<&str>,
        max_lag: usize,
    ) -> Result<AutocorrelationPlotData, BayesArtifactReadError> {
        let dataframe = read_bayes_artifact_dataframe(source, "posterior samples")?;
        autocorrelation_plot_data_from_dataframe(&dataframe, parameter, max_lag)
    }

    fn posterior_predictive_page(
        &self,
        source: &Path,
        offset: usize,
        limit: usize,
    ) -> Result<PosteriorPredictivePage, BayesArtifactReadError> {
        let dataframe = read_bayes_artifact_dataframe(source, "posterior predictive data")?;
        posterior_predictive_page_from_dataframe(&dataframe, offset, limit)
    }
}
