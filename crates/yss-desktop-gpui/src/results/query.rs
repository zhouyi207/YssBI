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
    Value {
        value: Arc<RuntimeValue>,
        tables: bool,
    },
    Page(ResultGrid),
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
            let tables = matches!(projection, ResultValueProjection::LinearReport(_))
                || snapshot.value().category()
                    == yss_graph_execution::plan::ResultCategory::StatisticalReport(
                        yss_graph_execution::plan::StatisticalReportKind::Structured,
                    );
            ResultContent::Value {
                value: super::value::overview(&projection)?,
                tables,
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
