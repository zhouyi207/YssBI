//! Polars-backed implementation of the backend-neutral Bayesian artifact contract.

use std::collections::BTreeMap;
use std::path::Path;

use polars::prelude::{DataFrame, Float64Chunked};

use yss_bayes_artifact_contract::{BayesArtifactReadError, BayesArtifactReader};
use yss_bayes_result::{
    AutocorrelationPlotData, AutocorrelationPoint, AutocorrelationSeries, DensityPlotData,
    DensityPoint, DensitySeries, PosteriorPredictivePage, PosteriorPredictiveRow,
    PosteriorPredictiveSummary, PosteriorSamplePage, PosteriorSampleRow, TracePlotData, TracePoint,
    TraceSeries,
};
use yss_tabular_io::{read_ipc_dataframe, write_csv_dataframe};

#[derive(Debug, Default, Clone, Copy)]
pub struct PolarsBayesArtifactReader;

impl PolarsBayesArtifactReader {
    pub const fn new() -> Self {
        Self
    }
}

fn read_bayes_artifact_dataframe(path: &Path) -> Result<DataFrame, BayesArtifactReadError> {
    read_ipc_dataframe(path).map_err(|_| BayesArtifactReadError::Read)
}

const fn samples_invalid() -> BayesArtifactReadError {
    BayesArtifactReadError::InvalidSamples
}

const fn posterior_predictive_invalid() -> BayesArtifactReadError {
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
        .map_err(|_| samples_invalid())?;
    let chains = dataframe
        .column("chain")
        .and_then(|column| column.i64())
        .map_err(|_| samples_invalid())?;
    let draws = dataframe
        .column("draw")
        .and_then(|column| column.i64())
        .map_err(|_| samples_invalid())?;
    let values = dataframe
        .column("value")
        .and_then(|column| column.f64())
        .map_err(|_| samples_invalid())?;

    let mut matching_indices = Vec::new();
    for index in 0..dataframe.height() {
        let row_parameter = parameters.get(index).ok_or_else(samples_invalid)?;
        usize::try_from(chains.get(index).ok_or_else(samples_invalid)?)
            .map_err(|_| samples_invalid())?;
        usize::try_from(draws.get(index).ok_or_else(samples_invalid)?)
            .map_err(|_| samples_invalid())?;
        let value = values.get(index).ok_or_else(samples_invalid)?;
        if !value.is_finite() {
            return Err(samples_invalid());
        }
        if parameter.is_none_or(|selected| selected == row_parameter) {
            matching_indices.push(index);
        }
    }

    let total = matching_indices.len();
    let rows = matching_indices
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|index| {
            let value = values.get(index).ok_or_else(samples_invalid)?;
            if !value.is_finite() {
                return Err(samples_invalid());
            }
            Ok(PosteriorSampleRow {
                parameter: parameters
                    .get(index)
                    .ok_or_else(samples_invalid)?
                    .to_string(),
                chain: usize::try_from(chains.get(index).ok_or_else(samples_invalid)?)
                    .map_err(|_| samples_invalid())?,
                draw: usize::try_from(draws.get(index).ok_or_else(samples_invalid)?)
                    .map_err(|_| samples_invalid())?,
                value,
            })
        })
        .collect::<Result<Vec<_>, BayesArtifactReadError>>()?;

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
    if dataframe.height() == 0 {
        return Err(posterior_predictive_invalid());
    }
    let observations = dataframe
        .column("observation")
        .and_then(|column| column.i64())
        .map_err(|_| posterior_predictive_invalid())?;
    let transforms = dataframe
        .column("response_transform")
        .and_then(|column| column.str())
        .map_err(|_| posterior_predictive_invalid())?;
    let response_transform = transforms
        .get(0)
        .ok_or_else(posterior_predictive_invalid)?
        .to_string();
    for index in 0..dataframe.height() {
        let transform = transforms
            .get(index)
            .ok_or_else(posterior_predictive_invalid)?;
        if transform != response_transform {
            return Err(posterior_predictive_invalid());
        }
    }
    let observed_model = predictive_f64_column(dataframe, "observed_model")?;
    let mean_model = predictive_f64_column(dataframe, "mean_model")?;
    let q025_model = predictive_f64_column(dataframe, "q025_model")?;
    let q975_model = predictive_f64_column(dataframe, "q975_model")?;
    let observed_original = predictive_f64_column(dataframe, "observed_original")?;
    let mean_original = predictive_f64_column(dataframe, "mean_original")?;
    let q025_original = predictive_f64_column(dataframe, "q025_original")?;
    let q975_original = predictive_f64_column(dataframe, "q975_original")?;

    for index in 0..dataframe.height() {
        usize::try_from(
            observations
                .get(index)
                .ok_or_else(posterior_predictive_invalid)?,
        )
        .map_err(|_| posterior_predictive_invalid())?;
        for column in [
            observed_model,
            mean_model,
            q025_model,
            q975_model,
            observed_original,
            mean_original,
            q025_original,
            q975_original,
        ] {
            predictive_value(column, index)?;
        }
    }

    let total = dataframe.height();
    let rows = (offset..total.min(offset.saturating_add(limit)))
        .map(|index| {
            Ok(PosteriorPredictiveRow {
                observation: usize::try_from(
                    observations
                        .get(index)
                        .ok_or_else(posterior_predictive_invalid)?,
                )
                .map_err(|_| posterior_predictive_invalid())?,
                model: PosteriorPredictiveSummary {
                    observed: predictive_value(observed_model, index)?,
                    mean: predictive_value(mean_model, index)?,
                    q025: predictive_value(q025_model, index)?,
                    q975: predictive_value(q975_model, index)?,
                },
                original: PosteriorPredictiveSummary {
                    observed: predictive_value(observed_original, index)?,
                    mean: predictive_value(mean_original, index)?,
                    q025: predictive_value(q025_original, index)?,
                    q975: predictive_value(q975_original, index)?,
                },
            })
        })
        .collect::<Result<Vec<_>, BayesArtifactReadError>>()?;

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
        .map_err(|_| posterior_predictive_invalid())
}

fn predictive_value(column: &Float64Chunked, index: usize) -> Result<f64, BayesArtifactReadError> {
    let value = column.get(index).ok_or_else(posterior_predictive_invalid)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(posterior_predictive_invalid())
    }
}

pub(crate) fn trace_plot_data_from_dataframe(
    dataframe: &DataFrame,
    parameter: Option<&str>,
    max_points_per_chain: usize,
) -> Result<TracePlotData, BayesArtifactReadError> {
    if max_points_per_chain == 0 {
        return Err(samples_invalid());
    }
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
    if grid_points < 2 {
        return Err(samples_invalid());
    }
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
        points: yss_sci_runtime::api::density::compute_kernel_density(
            yss_sci_runtime::api::density::KernelDensityInput {
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
        let mut dataframe = read_bayes_artifact_dataframe(source)?;
        write_csv_dataframe(destination, &mut dataframe).map_err(|_| BayesArtifactReadError::Export)
    }

    fn sample_page(
        &self,
        source: &Path,
        offset: usize,
        limit: usize,
        parameter: Option<&str>,
    ) -> Result<PosteriorSamplePage, BayesArtifactReadError> {
        let dataframe = read_bayes_artifact_dataframe(source)?;
        posterior_sample_page_from_dataframe(&dataframe, offset, limit, parameter)
    }

    fn trace_plot_data(
        &self,
        source: &Path,
        parameter: Option<&str>,
        max_points_per_chain: usize,
    ) -> Result<TracePlotData, BayesArtifactReadError> {
        let dataframe = read_bayes_artifact_dataframe(source)?;
        trace_plot_data_from_dataframe(&dataframe, parameter, max_points_per_chain)
    }

    fn density_plot_data(
        &self,
        source: &Path,
        parameter: Option<&str>,
        grid_points: usize,
    ) -> Result<DensityPlotData, BayesArtifactReadError> {
        let dataframe = read_bayes_artifact_dataframe(source)?;
        density_plot_data_from_dataframe(&dataframe, parameter, grid_points)
    }

    fn autocorrelation_plot_data(
        &self,
        source: &Path,
        parameter: Option<&str>,
        max_lag: usize,
    ) -> Result<AutocorrelationPlotData, BayesArtifactReadError> {
        let dataframe = read_bayes_artifact_dataframe(source)?;
        autocorrelation_plot_data_from_dataframe(&dataframe, parameter, max_lag)
    }

    fn posterior_predictive_page(
        &self,
        source: &Path,
        offset: usize,
        limit: usize,
    ) -> Result<PosteriorPredictivePage, BayesArtifactReadError> {
        let dataframe = read_bayes_artifact_dataframe(source)?;
        posterior_predictive_page_from_dataframe(&dataframe, offset, limit)
    }
}

#[cfg(test)]
mod tests {
    use polars::prelude::{Column, DataFrame};

    use yss_bayes_artifact_contract::BayesArtifactReadError;

    use super::{
        autocorrelation_plot_data_from_dataframe, density_plot_data_from_dataframe,
        posterior_predictive_page_from_dataframe, posterior_sample_page_from_dataframe,
        trace_plot_data_from_dataframe,
    };

    #[test]
    fn posterior_predictive_page_paginates_rows() {
        let dataframe = predictive_dataframe();

        let page = posterior_predictive_page_from_dataframe(&dataframe, 1, 1)
            .expect("posterior predictive page");
        assert_eq!(page.total, 3);
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.response_transform, "ln");
        assert_eq!(page.rows[0].observation, 2);
        assert_eq!(page.rows[0].model.observed, 2.0);
        assert_eq!(page.rows[0].original.mean, 5.1);
    }

    #[test]
    fn trace_density_and_autocorrelation_are_projected_from_samples() {
        let dataframe = sample_dataframe();

        let trace =
            trace_plot_data_from_dataframe(&dataframe, Some("a"), 1).expect("trace projection");
        assert_eq!(trace.series.len(), 2);
        assert_eq!(trace.series[0].parameter, "a");
        assert_eq!(trace.series[0].points.len(), 1);

        let density =
            density_plot_data_from_dataframe(&dataframe, Some("b"), 8).expect("density projection");
        assert_eq!(density.grid_points, 8);
        assert_eq!(density.series.len(), 3);
        assert_eq!(density.series[0].parameter, "b");
        assert_eq!(density.series[0].chain, None);
        assert_eq!(density.series[1].chain, Some(1));
        assert_eq!(density.series[2].chain, Some(2));
        assert_eq!(density.series[0].points.len(), 8);
        assert!(
            density.series[0]
                .points
                .iter()
                .all(|point| point.density.is_finite() && point.density >= 0.0)
        );

        let autocorrelation = autocorrelation_plot_data_from_dataframe(&dataframe, Some("a"), 2)
            .expect("autocorrelation projection");
        assert_eq!(autocorrelation.max_lag, 2);
        assert_eq!(autocorrelation.series.len(), 1);
        assert_eq!(autocorrelation.series[0].points[0].lag, 0);
        assert!((autocorrelation.series[0].points[0].autocorrelation - 1.0).abs() < 1e-12);
    }

    #[test]
    fn posterior_sample_page_filters_and_paginates() {
        let dataframe = sample_dataframe();

        let page = posterior_sample_page_from_dataframe(&dataframe, 1, 2, Some("a"))
            .expect("posterior sample page");

        assert_eq!(page.total, 3);
        assert_eq!(page.rows.len(), 2);
        assert_eq!(page.rows[0].parameter, "a");
        assert_eq!(page.rows[0].chain, 1);
        assert_eq!(page.rows[0].draw, 2);
        assert_eq!(page.rows[0].value, 1.1);
        assert_eq!(page.rows[1].chain, 2);
    }

    #[test]
    fn malformed_sample_rows_and_zero_plot_budgets_fail_closed() {
        let negative_chain = DataFrame::new(
            1,
            vec![
                Column::new("parameter".into(), &["a"]),
                Column::new("chain".into(), &[-1_i64]),
                Column::new("draw".into(), &[1_i64]),
                Column::new("value".into(), &[1.0]),
            ],
        )
        .expect("malformed samples dataframe still has a physical schema");

        assert_eq!(
            posterior_sample_page_from_dataframe(&negative_chain, 0, 1, None),
            Err(BayesArtifactReadError::InvalidSamples)
        );

        let mut non_finite_outside_page = sample_dataframe();
        non_finite_outside_page
            .replace(
                "value",
                Column::new("value".into(), &[1.0, 1.1, 2.0, 1.2, f64::NAN]),
            )
            .expect("replace sample values");
        assert_eq!(
            posterior_sample_page_from_dataframe(&non_finite_outside_page, 0, 1, None),
            Err(BayesArtifactReadError::InvalidSamples),
            "a malformed row outside the requested page must invalidate the artifact"
        );
        assert_eq!(
            trace_plot_data_from_dataframe(&sample_dataframe(), None, 0),
            Err(BayesArtifactReadError::InvalidSamples)
        );
        assert_eq!(
            density_plot_data_from_dataframe(&sample_dataframe(), None, 1),
            Err(BayesArtifactReadError::InvalidSamples)
        );
    }

    #[test]
    fn inconsistent_or_empty_predictive_rows_fail_closed() {
        let mut inconsistent = predictive_dataframe();
        inconsistent
            .replace(
                "response_transform",
                Column::new("response_transform".into(), &["ln", "identity", "ln"]),
            )
            .expect("replace response transform");
        assert_eq!(
            posterior_predictive_page_from_dataframe(&inconsistent, 0, 3),
            Err(BayesArtifactReadError::InvalidPosteriorPredictive)
        );

        let empty = inconsistent.head(Some(0));
        assert_eq!(
            posterior_predictive_page_from_dataframe(&empty, 0, 1),
            Err(BayesArtifactReadError::InvalidPosteriorPredictive)
        );

        let mut non_finite_outside_page = predictive_dataframe();
        non_finite_outside_page
            .replace(
                "q975_original",
                Column::new("q975_original".into(), &[3.8, 5.8, f64::INFINITY]),
            )
            .expect("replace posterior predictive values");
        assert_eq!(
            posterior_predictive_page_from_dataframe(&non_finite_outside_page, 0, 1),
            Err(BayesArtifactReadError::InvalidPosteriorPredictive),
            "a malformed row outside the requested page must invalidate the artifact"
        );
    }

    fn sample_dataframe() -> DataFrame {
        DataFrame::new(
            5,
            vec![
                Column::new("parameter".into(), &["a", "a", "b", "a", "b"]),
                Column::new("chain".into(), &[1_i64, 1, 1, 2, 2]),
                Column::new("draw".into(), &[1_i64, 2, 1, 1, 2]),
                Column::new("value".into(), &[1.0, 1.1, 2.0, 1.2, 2.1]),
            ],
        )
        .expect("valid samples dataframe")
    }

    fn predictive_dataframe() -> DataFrame {
        DataFrame::new(
            3,
            vec![
                Column::new("observation".into(), &[1_i64, 2, 3]),
                Column::new("response_transform".into(), &["ln", "ln", "ln"]),
                Column::new("observed_model".into(), &[1.0, 2.0, 3.0]),
                Column::new("mean_model".into(), &[1.1, 2.1, 3.1]),
                Column::new("q025_model".into(), &[0.5, 1.4, 2.2]),
                Column::new("q975_model".into(), &[1.8, 2.8, 3.7]),
                Column::new("observed_original".into(), &[3.0, 5.0, 7.0]),
                Column::new("mean_original".into(), &[3.1, 5.1, 6.9]),
                Column::new("q025_original".into(), &[2.5, 4.4, 6.2]),
                Column::new("q975_original".into(), &[3.8, 5.8, 7.7]),
            ],
        )
        .expect("valid posterior predictive dataframe")
    }
}
