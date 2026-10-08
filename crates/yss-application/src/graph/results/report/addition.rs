//! Explicit additions edit the current Summary and execute through the existing graph owners.
mod options;
pub use options::{LinearSummaryAddition, LinearSummaryContent};

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use thiserror::Error;
use yss_graph_document::GraphResourcePath;
use yss_graph_editor::EditorGraphMutation;
use yss_graph_execution::{
    plan::{ResultCategory, StatisticalReportKind},
    result::ResultReference,
};
use yss_project::GraphEditVersion;
use yss_project_identity::{OperationId, ProjectInstanceId};

use super::LinearRegressionReportProjection;
use crate::{
    graph::{
        editing::GraphEditRequest,
        open::{OpenGraphApplicationError, OpenGraphRequest},
        resources::ResourceMutationApplicationError,
        results::{
            ResultLease, ResultPinQuery, ResultQueryApplicationError, ResultValueProjection,
        },
        run::{ExecutionApplicationError, RunDemand, RunGraphRequest, run_graph_with_sink},
    },
    session::ApplicationState,
};

#[derive(Debug, Error)]
pub enum ReportAdditionError {
    #[error("report source is unavailable")]
    Unavailable,
    #[error("report source or consumer changed")]
    Changed,
    #[error("invalid report additions")]
    InvalidSelection,
    #[error(transparent)]
    Open(#[from] OpenGraphApplicationError),
    #[error(transparent)]
    Edit(#[from] ResourceMutationApplicationError),
    #[error(transparent)]
    Run(#[from] ExecutionApplicationError),
    #[error(transparent)]
    Query(#[from] ResultQueryApplicationError),
}

/// Held until the consumer accepts the exact edit version or drops the delivery.
pub struct LinearReportUpdate {
    lease: ResultLease,
    report: Box<LinearRegressionReportProjection>,
    project: ProjectInstanceId,
    graph: GraphResourcePath,
    version: GraphEditVersion,
}

impl LinearReportUpdate {
    pub fn report(&self) -> &LinearRegressionReportProjection {
        &self.report
    }

    /// This final gate only reads resident identities; it performs no I/O or Resolve.
    pub fn accept(
        self,
        application: &ApplicationState,
    ) -> Result<(ResultLease, Box<LinearRegressionReportProjection>), ReportAdditionError> {
        self.revalidate(application)?;
        Ok((self.lease, self.report))
    }

    fn revalidate(&self, application: &ApplicationState) -> Result<(), ReportAdditionError> {
        let session = application
            .capture_session()
            .map_err(|_| ReportAdditionError::Changed)?;
        if session.execution_session_id() != self.lease.reference().execution_session_id {
            return Err(ReportAdditionError::Changed);
        }
        application
            .current_graph_document(&self.project, &self.graph, self.version)
            .map_err(|_| ReportAdditionError::Changed)?;
        application
            .revalidate_captured_session(&session)
            .map_err(|_| ReportAdditionError::Changed)
    }
}

impl ApplicationState {
    pub fn add_linear_report_contents(
        &self,
        reference: ResultReference,
        additions: LinearSummaryAddition,
        locale: &str,
        cancellation: Arc<AtomicBool>,
    ) -> Result<LinearReportUpdate, ReportAdditionError> {
        let parameters = additions.parameters()?;
        check_cancelled(&cancellation)?;
        let captured = self
            .capture_session()
            .map_err(|_| ReportAdditionError::Changed)?;
        let original = self
            .query_result(reference)?
            .ok_or(ReportAdditionError::Unavailable)?;
        if original.value().category()
            != ResultCategory::StatisticalReport(StatisticalReportKind::LinearRegressionSummary)
        {
            return Err(ReportAdditionError::Unavailable);
        }
        let source = original.output().clone();
        let graph = GraphResourcePath::new(source.graph().as_str())
            .map_err(|_| ReportAdditionError::Unavailable)?;
        let project = captured.project_instance_id().clone();
        let opened = self.open_graph(OpenGraphRequest::new(
            project.clone(),
            graph.clone(),
            0,
            locale,
        ))?;
        let output = opened
            .projection()
            .nodes
            .iter()
            .flat_map(|node| &node.ports)
            .find(|port| port.address.to_string() == source.port().as_str())
            .map(|port| port.address.clone())
            .ok_or(ReportAdditionError::Unavailable)?;
        let node = opened
            .document()
            .nodes
            .get(&output.node_id)
            .filter(|node| node.node_type.as_str() == "yssbi.statistics.linear.summary")
            .ok_or(ReportAdditionError::Unavailable)?;
        self.revalidate_captured_session(&captured)
            .map_err(|_| ReportAdditionError::Changed)?;
        check_cancelled(&cancellation)?;
        let edited = self.edit_graph(
            GraphEditRequest {
                project_instance_id: project.clone(),
                graph_path: graph.clone(),
                version: opened.editing().version,
                operation_id: OperationId::new(),
                locale: locale.into(),
            },
            EditorGraphMutation::SetParameters {
                node_id: node.id,
                parameters,
            },
        )?;
        let version = edited.editing.version;
        let document = self
            .current_graph_document(&project, &graph, version)
            .map_err(|_| ReportAdditionError::Changed)?;
        check_cancelled(&cancellation)?;
        let receipt = run_graph_with_sink(
            self,
            RunGraphRequest::new(
                project.clone(),
                graph.clone(),
                document,
                edited
                    .update
                    .projection_replacement
                    .projection
                    .basis
                    .semantic_input_hash,
            )
            .with_demand(RunDemand::Outputs {
                outputs: Box::new([source.clone()]),
                include_default_results: false,
                reuse_inputs: true,
            })
            .with_cancellation(cancellation.clone()),
            |_| true,
        )?;

        // Match this run's committed output, not whichever later run occupies the pin.
        let result = receipt
            .results
            .iter()
            .find(|result| result.output == source)
            .ok_or(ReportAdditionError::Unavailable)?;
        let reference = ResultReference {
            execution_session_id: *receipt.identity.execution_session_id(),
            result_id: result.result_id,
        };
        let current = self
            .query_pin_result(ResultPinQuery::new(graph.clone(), output))?
            .ok_or(ReportAdditionError::Changed)?;
        if current.provenance().reference() != reference {
            return Err(ReportAdditionError::Changed);
        }
        let (lease, _) = self.retain_owned_result(reference)?;
        let Some(ResultValueProjection::LinearReport(report)) =
            self.query_result_projection(reference)?
        else {
            return Err(ReportAdditionError::Unavailable);
        };
        let update = LinearReportUpdate {
            lease,
            report,
            project,
            graph,
            version,
        };
        check_cancelled(&cancellation)?;
        update.revalidate(self)?;
        Ok(update)
    }
}

fn check_cancelled(cancellation: &AtomicBool) -> Result<(), ReportAdditionError> {
    if cancellation.load(Ordering::Acquire) {
        Err(ReportAdditionError::Changed)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
