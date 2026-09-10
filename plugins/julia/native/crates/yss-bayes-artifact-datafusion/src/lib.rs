//! DataFusion queries over immutable Julia Arrow artifacts.
mod plots;
mod rows;
#[cfg(test)]
mod tests;

use std::future::Future;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use arrow::record_batch::RecordBatch;
use datafusion::common::Column;
use datafusion::dataframe::DataFrame;
use datafusion::datasource::file_format::arrow::ArrowFormat;
use datafusion::datasource::listing::{
    ListingOptions, ListingTable, ListingTableConfig, ListingTableUrl,
};
use datafusion::execution::runtime_env::{RuntimeEnv, RuntimeEnvBuilder};
use datafusion::prelude::{SessionConfig, SessionContext, col, lit};
use futures_util::StreamExt;
use yss_bayes_artifact_contract::{BayesArtifactReadError, BayesArtifactReader};
use yss_bayes_result::{
    AutocorrelationPlotData, AutocorrelationPoint, AutocorrelationSeries, DensityPlotData,
    DensityPoint, DensitySeries, PosteriorPredictivePage, PosteriorPredictiveRow,
    PosteriorPredictiveSummary, PosteriorSamplePage, PosteriorSampleRow, TracePlotData, TracePoint,
    TraceSeries,
};

const MAX_BYTES: usize = 128 * 1024 * 1024;
const QUERY_DEADLINE: Duration = Duration::from_secs(60);

pub struct DataFusionBayesArtifactReader {
    environment: Arc<RuntimeEnv>,
    runtime: Option<tokio::runtime::Runtime>,
    admission: Mutex<()>,
}

const fn samples_invalid() -> BayesArtifactReadError {
    BayesArtifactReadError::InvalidSamples
}
const fn posterior_predictive_invalid() -> BayesArtifactReadError {
    BayesArtifactReadError::InvalidPosteriorPredictive
}

impl DataFusionBayesArtifactReader {
    pub fn new() -> Result<Self, BayesArtifactReadError> {
        let environment = RuntimeEnvBuilder::new()
            .with_memory_limit(MAX_BYTES, 1.0)
            .build_arc()
            .map_err(|_| BayesArtifactReadError::Read)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| BayesArtifactReadError::Read)?;
        Ok(Self {
            environment,
            runtime: Some(runtime),
            admission: Mutex::new(()),
        })
    }

    fn run<T: Send>(
        &self,
        operation: impl Future<Output = Result<T, BayesArtifactReadError>> + Send,
    ) -> Result<T, BayesArtifactReadError> {
        let execute = || {
            let _admission = self
                .admission
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            self.runtime
                .as_ref()
                .ok_or(BayesArtifactReadError::Read)?
                .block_on(async {
                    tokio::time::timeout(QUERY_DEADLINE, operation)
                        .await
                        .map_err(|_| BayesArtifactReadError::Read)?
                })
        };
        if tokio::runtime::Handle::try_current().is_err() {
            return execute();
        }
        std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name("bayes-artifact-query".into())
                .spawn_scoped(scope, execute)
                .map_err(|_| BayesArtifactReadError::Read)?
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
        })
    }

    fn source(&self, source: &Path) -> Result<DataFrame, BayesArtifactReadError> {
        let file = std::fs::File::open(source).map_err(|_| BayesArtifactReadError::Read)?;
        let schema = arrow::ipc::reader::FileReader::try_new(file, None)
            .map_err(|_| BayesArtifactReadError::Read)?
            .schema();
        let path = std::fs::canonicalize(source).map_err(|_| BayesArtifactReadError::Read)?;
        let url = url::Url::from_file_path(path).map_err(|_| BayesArtifactReadError::Read)?;
        // Literal paths may contain glob metacharacters. A single file/partition preserves
        // the artifact's row order through projection, filtering and pagination.
        let url = ListingTableUrl::try_new(url, None).map_err(|_| BayesArtifactReadError::Read)?;
        let options = ListingOptions::new(Arc::new(ArrowFormat)).with_file_extension("");
        let table = ListingTable::try_new(
            ListingTableConfig::new(url)
                .with_listing_options(options)
                .with_schema(schema),
        )
        .map_err(|_| BayesArtifactReadError::Read)?;
        SessionContext::new_with_config_rt(
            SessionConfig::new()
                .with_target_partitions(1)
                .with_batch_size(8192)
                .with_collect_statistics(false),
            self.environment.clone(),
        )
        .read_table(Arc::new(table))
        .map_err(|_| BayesArtifactReadError::Read)
    }

    async fn selected_samples(
        &self,
        source: &Path,
        parameter: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> Result<PosteriorSamplePage, BayesArtifactReadError> {
        let frame = self
            .source(source)?
            .select(vec![
                col("parameter"),
                col("chain"),
                col("draw"),
                col("value"),
            ])
            .map_err(|_| samples_invalid())?;
        let mut total = 0usize;
        visit(frame.clone(), &mut |batch| {
            rows::samples(&batch, |name, _, _, _| {
                if parameter.is_none_or(|selected| selected == name) {
                    total = total.checked_add(1).ok_or_else(samples_invalid)?;
                }
                Ok(())
            })
        })
        .await?;
        let filtered = if let Some(parameter) = parameter {
            frame
                .filter(col("parameter").eq(lit(parameter)))
                .map_err(|_| samples_invalid())?
        } else {
            frame
        };
        let frame = filtered
            .limit(offset, (limit != usize::MAX).then_some(limit))
            .map_err(|_| samples_invalid())?;
        let mut selected = Vec::new();
        let mut bytes = 0usize;
        visit(frame, &mut |batch| {
            rows::samples(&batch, |parameter, chain, draw, value| {
                bytes = bytes
                    .checked_add(parameter.len())
                    .and_then(|size| {
                        size.checked_add(2 * std::mem::size_of::<PosteriorSampleRow>())
                    })
                    .ok_or(BayesArtifactReadError::Read)?;
                if bytes > MAX_BYTES {
                    return Err(BayesArtifactReadError::Read);
                }
                selected.push(PosteriorSampleRow {
                    parameter: parameter.into(),
                    chain,
                    draw,
                    value,
                });
                Ok(())
            })
        })
        .await?;
        Ok(PosteriorSamplePage {
            rows: selected,
            offset,
            limit,
            total,
        })
    }
}

impl Drop for DataFusionBayesArtifactReader {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

async fn visit(
    frame: DataFrame,
    visitor: &mut (dyn FnMut(RecordBatch) -> Result<(), BayesArtifactReadError> + Send),
) -> Result<(), BayesArtifactReadError> {
    // Also validate the schema for empty files, which may emit no batches.
    visitor(RecordBatch::new_empty(Arc::new(
        frame.schema().as_arrow().clone(),
    )))?;
    let mut stream = frame
        .execute_stream()
        .await
        .map_err(|_| BayesArtifactReadError::Read)?;
    while let Some(batch) = stream.next().await {
        let batch = batch.map_err(|_| BayesArtifactReadError::Read)?;
        if batch.get_array_memory_size() > MAX_BYTES {
            return Err(BayesArtifactReadError::Read);
        }
        visitor(batch)?;
    }
    Ok(())
}

impl BayesArtifactReader for DataFusionBayesArtifactReader {
    fn export_csv(&self, source: &Path, destination: &Path) -> Result<(), BayesArtifactReadError> {
        let frame = self.source(source)?;
        if let Some(parent) = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|_| BayesArtifactReadError::Export)?;
        }
        let file =
            std::fs::File::create(destination).map_err(|_| BayesArtifactReadError::Export)?;
        let mut writer = arrow::csv::Writer::new(&file);
        self.run(visit(frame, &mut |batch| {
            writer
                .write(&batch)
                .map_err(|_| BayesArtifactReadError::Export)
        }))?;
        file.sync_all().map_err(|_| BayesArtifactReadError::Export)
    }

    fn sample_page(
        &self,
        source: &Path,
        offset: usize,
        limit: usize,
        parameter: Option<&str>,
    ) -> Result<PosteriorSamplePage, BayesArtifactReadError> {
        self.run(self.selected_samples(source, parameter, offset, limit))
    }

    fn trace_plot_data(
        &self,
        source: &Path,
        parameter: Option<&str>,
        max_points_per_chain: usize,
    ) -> Result<TracePlotData, BayesArtifactReadError> {
        if max_points_per_chain == 0 {
            return Err(samples_invalid());
        }
        let rows = self
            .run(self.selected_samples(source, parameter, 0, usize::MAX))?
            .rows;
        plots::trace_plot_data(rows, max_points_per_chain)
    }

    fn density_plot_data(
        &self,
        source: &Path,
        parameter: Option<&str>,
        grid_points: usize,
    ) -> Result<DensityPlotData, BayesArtifactReadError> {
        if grid_points < 2 {
            return Err(samples_invalid());
        }
        let rows = self
            .run(self.selected_samples(source, parameter, 0, usize::MAX))?
            .rows;
        plots::density_plot_data(rows, grid_points)
    }

    fn autocorrelation_plot_data(
        &self,
        source: &Path,
        parameter: Option<&str>,
        max_lag: usize,
    ) -> Result<AutocorrelationPlotData, BayesArtifactReadError> {
        let rows = self
            .run(self.selected_samples(source, parameter, 0, usize::MAX))?
            .rows;
        plots::autocorrelation_plot_data(rows, max_lag)
    }

    fn posterior_predictive_page(
        &self,
        source: &Path,
        offset: usize,
        limit: usize,
    ) -> Result<PosteriorPredictivePage, BayesArtifactReadError> {
        self.run(async {
            let frame = self
                .source(source)?
                .select(
                    rows::PREDICTIVE_COLUMNS
                        .iter()
                        .map(|name| {
                            datafusion::logical_expr::Expr::Column(Column::from_name(*name))
                        })
                        .collect::<Vec<_>>(),
                )
                .map_err(|_| posterior_predictive_invalid())?;
            let mut total = 0usize;
            let mut transform: Option<String> = None;
            visit(frame.clone(), &mut |batch| {
                rows::predictive(&batch, |response_transform, _| {
                    match &transform {
                        Some(previous) if previous != response_transform => {
                            return Err(posterior_predictive_invalid());
                        }
                        None => transform = Some(response_transform.into()),
                        _ => {}
                    }
                    total = total
                        .checked_add(1)
                        .ok_or_else(posterior_predictive_invalid)?;
                    Ok(())
                })
            })
            .await?;
            let response_transform = transform.ok_or_else(posterior_predictive_invalid)?;
            let mut selected = Vec::new();
            let frame = frame
                .limit(offset, Some(limit))
                .map_err(|_| posterior_predictive_invalid())?;
            visit(frame, &mut |batch| {
                rows::predictive(&batch, |_, row| {
                    if selected.len()
                        >= MAX_BYTES / (2 * std::mem::size_of::<PosteriorPredictiveRow>())
                    {
                        return Err(BayesArtifactReadError::Read);
                    }
                    selected.push(row);
                    Ok(())
                })
            })
            .await?;
            Ok(PosteriorPredictivePage {
                rows: selected,
                response_transform,
                offset,
                limit,
                total,
            })
        })
    }
}
