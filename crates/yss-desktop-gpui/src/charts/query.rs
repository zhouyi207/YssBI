//! All chart reads use the original project publication and database resource identities.
use super::ChartEditor;
use gpui::{Context, Window};
use std::{sync::Arc, time::Duration};
use yss_application::{
    chart::{ChartPlotQuery, ChartPlotResult},
    database::DatabaseMetaResult,
    runtime::ApplicationServices,
};
use yss_chart_document::{ChartDocument, ChartResourcePath, ChartType};
use yss_data_contract::TabularColumnName;
use yss_database_contract::DatabaseId;
use yss_dataset_profile::ColumnDistribution;
use yss_project::ProjectIndex;
use yss_project_identity::{ProjectInstanceId, ResourceRevision};

pub(crate) struct ChartRead {
    pub project: ProjectInstanceId,
    pub path: ChartResourcePath,
    pub revision: ResourceRevision,
    pub catalog: Arc<ProjectIndex>,
    pub document: ChartDocument,
}
pub(crate) fn read(
    services: &ApplicationServices,
    project: ProjectInstanceId,
    path: ChartResourcePath,
) -> anyhow::Result<ChartRead> {
    let catalog = services
        .application
        .query_project_index(project.clone(), "zh-CN", true)?
        .index;
    let revision = catalog
        .charts
        .iter()
        .find(|entry| entry.chart_path == path)
        .ok_or_else(|| anyhow::anyhow!("chart absent from project index"))?
        .revision;
    let document = services.application.load_chart_resource(
        project.clone(),
        path.clone(),
        Some(catalog.publication_revision),
    )?;
    Ok(ChartRead {
        project,
        path,
        revision,
        catalog: Arc::new(catalog),
        document,
    })
}
#[derive(Clone)]
pub(super) struct HistogramDatum {
    pub index: usize,
    pub label: String,
    pub count: usize,
}
pub(super) enum PreviewData {
    Empty(&'static str),
    Failed(&'static str),
    Histogram {
        bins: Vec<HistogramDatum>,
        column: String,
        other_count: usize,
    },
    Cartesian(Arc<super::plot::CartesianData>),
}
struct PreviewRead {
    meta: Option<DatabaseMetaResult>,
    data: PreviewData,
}
fn preview(
    services: &ApplicationServices,
    project: ProjectInstanceId,
    catalog: &ProjectIndex,
    document: &ChartDocument,
) -> anyhow::Result<PreviewRead> {
    if document.database_id.is_empty() {
        return Ok(PreviewRead {
            meta: None,
            data: PreviewData::Empty("在右侧属性中选择数据集和绘图列"),
        });
    }
    let source = catalog
        .databases
        .iter()
        .find(|entry| entry.id == document.database_id)
        .ok_or_else(|| anyhow::anyhow!("chart source absent from project index"))?;
    let meta = services.application.query_database_meta_for_application(
        project.clone(),
        source.id.clone(),
        source.revision,
    )?;
    let data = (|| -> anyhow::Result<PreviewData> {
        Ok(match document.chart_type {
            ChartType::Histogram => {
                if let Some(column) = document
                    .encodings
                    .y
                    .as_ref()
                    .or(document.encodings.x.as_ref())
                {
                    let distributions = services
                        .application
                        .query_column_distributions_for_application(
                            project,
                            source.id.clone(),
                            source.revision,
                        )?;
                    histogram(distributions, column)?
                } else {
                    PreviewData::Empty("选择一列，查看它的数值或类别分布")
                }
            }
            ChartType::Scatter | ChartType::Line => {
                if let (Some(x), Some(y)) = (&document.encodings.x, &document.encodings.y) {
                    let pair: ChartPlotResult =
                        services.application.query_chart_plot(ChartPlotQuery {
                            project_instance_id: project,
                            database_id: DatabaseId::from_existing(source.id.clone().into()),
                            expected_revision: source.revision,
                            x_column: TabularColumnName::try_from(x.as_str())?,
                            y_column: TabularColumnName::try_from(y.as_str())?,
                            max_points: None,
                        })?;
                    PreviewData::Cartesian(Arc::new(super::plot::CartesianData::new(
                        pair,
                        document.chart_type,
                    )))
                } else {
                    PreviewData::Empty("选择 X 轴和 Y 轴列，查看散点或折线")
                }
            }
        })
    })()
    .unwrap_or(PreviewData::Failed(
        "无法读取当前配置的绘图数据，请重新选择列或刷新。",
    ));
    Ok(PreviewRead {
        meta: Some(meta),
        data,
    })
}
fn histogram(distributions: Vec<ColumnDistribution>, column: &str) -> anyhow::Result<PreviewData> {
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
    Err(anyhow::anyhow!("chart column unavailable"))
}
impl ChartEditor {
    pub(super) fn schedule_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_generation = self.preview_generation.wrapping_add(1);
        let generation = self.preview_generation;
        self.preview_loading = true;
        self.preview = None;
        let project = self.project.clone();
        let catalog = self.catalog.clone();
        let document = self.draft.clone();
        let source = catalog
            .databases
            .iter()
            .find(|entry| entry.id == document.database_id)
            .map(|entry| (entry.id.clone(), entry.revision));
        if self.meta_source != source {
            self.meta = None;
            self.meta_source = None;
        }
        let owner = self.services.clone();
        let timer = cx.background_executor().timer(Duration::from_millis(250));
        self.preview_task = Some(cx.spawn_in(window, async move |view, cx| {
            timer.await;
            let result = owner
                .run(move |services| preview(services, project, &catalog, &document))
                .await
                .ok()
                .and_then(Result::ok);
            let _ = view.update(cx, |view, cx| {
                if view.preview_generation != generation {
                    return;
                }
                view.preview_loading = false;
                match result {
                    Some(read) => {
                        view.meta = read.meta;
                        view.meta_source = source;
                        view.preview = Some(Arc::new(read.data));
                    }
                    None => {
                        view.error =
                            Some("图表预览未读取，请检查数据集、列类型或当前资源状态。".into())
                    }
                }
                view.changed(cx);
            });
        }));
        self.changed(cx);
    }
    pub(crate) fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            self.refresh_again = true;
            return;
        }
        self.read_generation = self.read_generation.wrapping_add(1);
        let generation = self.read_generation;
        self.reading = true;
        self.error = None;
        let project = self.project.clone();
        let path = self.path.clone();
        let task = self
            .services
            .run(move |services| read(services, project, path));
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.read_generation != generation {
                    return;
                }
                view.reading = false;
                if let Some(read) = result {
                    if read.catalog.publication_revision < view.catalog.publication_revision {
                        view.refresh_again = view.available;
                    } else {
                        let dirty = view.dirty();
                        view.external_change = dirty
                            && view.draft != read.document
                            && (view.external_change || read.document != view.saved);
                        view.revision = read.revision;
                        view.catalog = read.catalog;
                        view.saved = read.document.clone();
                        if !dirty {
                            view.draft = read.document;
                            view.draft_epoch = view.draft_epoch.wrapping_add(1);
                        }
                        view.available = true;
                        view.schedule_preview(window, cx);
                    }
                } else {
                    view.error = Some("图表未读取，本地配置已保留，请刷新项目后重试。".into());
                }
                if view.refresh_again {
                    view.refresh_again = false;
                    view.refresh(window, cx);
                }
                view.changed(cx);
            });
        })
        .detach();
        self.changed(cx);
    }
    pub(crate) fn replace_catalog(
        &mut self,
        catalog: Arc<ProjectIndex>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if catalog.publication_revision < self.catalog.publication_revision {
            return;
        }
        let revision = catalog
            .charts
            .iter()
            .find(|entry| entry.chart_path == self.path)
            .map(|entry| entry.revision);
        let source_revision = |catalog: &ProjectIndex| {
            catalog
                .databases
                .iter()
                .find(|entry| entry.id == self.draft.database_id)
                .map(|entry| entry.revision)
        };
        let source_changed = source_revision(&self.catalog) != source_revision(&catalog);
        self.catalog = catalog;
        let Some(revision) = revision else {
            self.available = false;
            self.read_generation = self.read_generation.wrapping_add(1);
            self.reading = false;
            self.refresh_again = false;
            self.preview_generation = self.preview_generation.wrapping_add(1);
            self.preview_task = None;
            self.preview_loading = false;
            self.preview = None;
            self.error = Some("图表已移除，本地未保存的配置仍保留。".into());
            self.changed(cx);
            return;
        };
        self.available = true;
        if revision != self.revision {
            self.refresh(window, cx);
        } else if source_changed {
            self.schedule_preview(window, cx);
        }
        self.changed(cx);
    }
}
