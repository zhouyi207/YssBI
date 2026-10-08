//! Result ownership and bounded queries reuse Application's store and leases.
use std::sync::Arc;

use anyhow::{Result, anyhow};
use yss_application::graph::results::{ResultValueProjection, report::ResultTablePart};
use yss_graph_execution::result::ResultReference;
use yss_node_kernel::RuntimeValue;

use super::table::ResultGrid;
use crate::services::NativeServices;

pub const PAGE_ROWS: usize = 100;

pub struct ResultLease {
    services: Arc<NativeServices>,
    id: uuid::Uuid,
    owner: String,
}

impl Drop for ResultLease {
    fn drop(&mut self) {
        let id = self.id;
        let owner = self.owner.clone();
        self.services.run(move |services| {
            services.application.release_result_lease(id, &owner)?;
            Ok(())
        });
    }
}

pub enum ResultContent {
    InvalidPlot,
    Plot(super::plot::PlotData),
    Value {
        value: Arc<RuntimeValue>,
        tables: bool,
        report: Option<ReportContent>,
    },
    Page(ResultGrid),
}

pub enum ReportContent {
    Structured(Result<Vec<yss_application::graph::results::report::structured::ReportSection>, ()>),
    Linear(Box<yss_application::graph::results::report::LinearRegressionReportProjection>),
}

pub fn open(
    services: Arc<NativeServices>,
    reference: ResultReference,
) -> Result<(ResultLease, ResultContent)> {
    let id = uuid::Uuid::new_v4();
    let owner = format!("gpui-result:{id}");
    let snapshot = services
        .application
        .application
        .retain_result(reference, id, &owner, None)?;
    // Construct inside the worker before reading; cancellation and failed queries release ownership.
    let lease = ResultLease {
        services: services.clone(),
        id,
        owner,
    };
    if matches!(
        snapshot.value().category(),
        yss_graph_execution::plan::ResultCategory::PlotData(_)
    ) {
        match services
            .application
            .application
            .query_result_plot(reference)
        {
            Ok(Some(plot)) => {
                return Ok((lease, ResultContent::Plot(super::plot::PlotData::new(plot))));
            }
            Err(
                yss_application::graph::results::ResultQueryApplicationError::UnrepresentableValue,
            ) => {
                return Ok((lease, ResultContent::InvalidPlot));
            }
            Err(error) => return Err(error.into()),
            Ok(None) => {}
        }
    }
    let content = match snapshot.value().value().unannotated() {
        RuntimeValue::Relation(_) | RuntimeValue::Series(_) | RuntimeValue::List(_) => {
            ResultContent::Page(page(&services, reference, None, 0)?)
        }
        _ => {
            let projection = services
                .application
                .application
                .query_result_projection(reference)?
                .ok_or_else(|| anyhow!("result unavailable"))?;
            let structured = snapshot.value().category()
                == yss_graph_execution::plan::ResultCategory::StatisticalReport(
                    yss_graph_execution::plan::StatisticalReportKind::Structured,
                );
            let tables = matches!(projection, ResultValueProjection::LinearReport(_)) || structured;
            let value = super::value::overview(&projection)?;
            let report = match projection {
                ResultValueProjection::LinearReport(report) => Some(ReportContent::Linear(report)),
                _ if structured => Some(ReportContent::Structured(
                    yss_application::graph::results::report::structured::sections(&value)
                        .map_err(|_| ()),
                )),
                _ => None,
            };
            ResultContent::Value {
                value,
                tables,
                report,
            }
        }
    };
    Ok((lease, content))
}

pub fn page(
    services: &NativeServices,
    reference: ResultReference,
    part: Option<ResultTablePart>,
    offset: usize,
) -> Result<ResultGrid> {
    let application = &services.application.application;
    let page = match part {
        Some(part) => application.query_result_table(reference, part, offset, PAGE_ROWS)?,
        None => application
            .query_result_page(reference, offset, PAGE_ROWS)?
            .ok_or_else(|| anyhow!("result unavailable"))?,
    };
    Ok(ResultGrid::from_page(page))
}
