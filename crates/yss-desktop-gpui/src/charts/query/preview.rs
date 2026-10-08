//! Preview projection consumes revision-bound metadata and only the requested data columns.
use std::sync::Arc;
use yss_application::{
    chart::{ChartPlotApplicationError, ChartPlotQuery},
    database::DatabaseMetaResult,
    runtime::ApplicationServices,
};
use yss_chart_document::{ChartDocument, ChartType};
use yss_data_contract::TabularColumnName;
use yss_database_contract::DatabaseId;
use yss_dataset_profile::ColumnDistribution;
use yss_project::ProjectIndex;
use yss_project_identity::ProjectInstanceId;

use crate::plots::histogram::HistogramDatum;
pub(in crate::charts) enum PreviewFailure {
    Read,
    SourceMissing,
    ColumnMissing(String),
    NoFinitePoints,
}
impl PreviewFailure {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Read => "chart_preview_read_failed",
            Self::SourceMissing => "chart_preview_source_not_found",
            Self::ColumnMissing(_) => "chart_preview_column_not_found",
            Self::NoFinitePoints => "chart_plot_data_empty",
        }
    }
    pub fn summary(&self) -> String {
        match self {
            Self::Read => crate::text::translate("chart.previewLoadFailed"),
            Self::SourceMissing => crate::text::translate("native.charts.removedDataset"),
            Self::ColumnMissing(column) => {
                crate::text::format("chart.previewColumnNotFound", &[("column", column.clone())])
            }
            Self::NoFinitePoints => crate::text::translate("native.charts.noFinitePoints"),
        }
    }
}
pub(in crate::charts) enum PreviewData {
    Empty(&'static str),
    Failed(PreviewFailure),
    Histogram {
        bins: Vec<HistogramDatum>,
        column: String,
        other_count: usize,
    },
    Cartesian(Arc<crate::plots::cartesian::CartesianData>),
}
pub(super) struct PreviewRead {
    pub meta: Option<Arc<DatabaseMetaResult>>,
    pub data: PreviewData,
}
pub(super) fn read(
    services: &ApplicationServices,
    project: ProjectInstanceId,
    catalog: &ProjectIndex,
    document: &ChartDocument,
    metadata: Option<Arc<DatabaseMetaResult>>,
) -> PreviewRead {
    if document.database_id.is_empty() {
        return PreviewRead {
            meta: None,
            data: PreviewData::Empty("chart.previewEmptyHint"),
        };
    }
    let Some(source) = catalog
        .databases
        .iter()
        .find(|entry| entry.id == document.database_id)
    else {
        return PreviewRead {
            meta: None,
            data: PreviewData::Failed(PreviewFailure::SourceMissing),
        };
    };
    // The caller only supplies metadata admitted for this exact database resource revision.
    let meta = metadata.or_else(|| {
        services
            .application
            .query_database_meta_for_application(
                project.clone(),
                source.id.clone(),
                source.revision,
            )
            .ok()
            .map(Arc::new)
    });
    let Some(meta) = meta else {
        return PreviewRead {
            meta: None,
            data: PreviewData::Failed(PreviewFailure::Read),
        };
    };
    let data = (|| -> Result<PreviewData, PreviewFailure> {
        let column = |name: &str| {
            if meta
                .columns
                .iter()
                .any(|column| column.name().as_str() == name)
            {
                Ok(())
            } else {
                Err(PreviewFailure::ColumnMissing(name.to_owned()))
            }
        };
        match document.chart_type {
            ChartType::Histogram => {
                let Some(name) = document
                    .encodings
                    .y
                    .as_ref()
                    .or(document.encodings.x.as_ref())
                else {
                    return Ok(PreviewData::Empty("chart.previewEmptyHint"));
                };
                column(name)?;
                let distributions = services
                    .application
                    .query_column_distributions_for_application(
                        project,
                        source.id.clone(),
                        source.revision,
                        std::slice::from_ref(name),
                    )
                    .map_err(|_| PreviewFailure::Read)?;
                histogram(distributions, name)
            }
            ChartType::Scatter | ChartType::Line => {
                let (Some(x), Some(y)) = (&document.encodings.x, &document.encodings.y) else {
                    return Ok(PreviewData::Empty("chart.previewEmptyHint"));
                };
                column(x)?;
                column(y)?;
                let result = services
                    .application
                    .query_chart_plot(ChartPlotQuery {
                        project_instance_id: project,
                        database_id: DatabaseId::from_existing(source.id.clone().into()),
                        expected_revision: source.revision,
                        x_column: TabularColumnName::try_from(x.as_str())
                            .map_err(|_| PreviewFailure::Read)?,
                        y_column: TabularColumnName::try_from(y.as_str())
                            .map_err(|_| PreviewFailure::Read)?,
                        max_points: None,
                    })
                    .map_err(|error| match error {
                        ChartPlotApplicationError::PlotDataEmpty => PreviewFailure::NoFinitePoints,
                        _ => PreviewFailure::Read,
                    })?;
                Ok(PreviewData::Cartesian(Arc::new(
                    crate::plots::cartesian::CartesianData::new(
                        result,
                        crate::plots::cartesian::CartesianOptions::new(
                            if document.chart_type == ChartType::Line {
                                crate::plots::cartesian::CartesianKind::Line
                            } else {
                                crate::plots::cartesian::CartesianKind::Scatter
                            },
                        ),
                    ),
                )))
            }
        }
    })()
    .unwrap_or_else(PreviewData::Failed);
    PreviewRead {
        meta: Some(meta),
        data,
    }
}

fn histogram(
    distributions: Vec<ColumnDistribution>,
    column: &str,
) -> Result<PreviewData, PreviewFailure> {
    for distribution in distributions {
        match distribution {
            ColumnDistribution::Numeric(value) if value.column_name == column => {
                return Ok(PreviewData::Histogram {
                    bins: value
                        .bins
                        .into_iter()
                        .enumerate()
                        .map(|(index, bin)| HistogramDatum {
                            index,
                            label: bin.label,
                            count: bin.count,
                        })
                        .collect(),
                    column: column.into(),
                    other_count: 0,
                });
            }
            ColumnDistribution::String(value) if value.column_name == column => {
                return Ok(PreviewData::Histogram {
                    bins: value
                        .categories
                        .into_iter()
                        .enumerate()
                        .map(|(index, bin)| HistogramDatum {
                            index,
                            label: bin.label,
                            count: bin.value,
                        })
                        .collect(),
                    column: column.into(),
                    other_count: value.other_count,
                });
            }
            _ => {}
        }
    }
    Err(PreviewFailure::ColumnMissing(column.to_owned()))
}
