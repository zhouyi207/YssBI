use crate::event::ResourceMutationResultDto;
use crate::node_system::plan::ExecutionDemand;

use crate::graph_document::GraphResourcePath;
use crate::node_system::runtime::{RunEvent, RunEventKind, RunEventSink, RunOutputMessage};
use crate::project::{ProjectExecutionError, ProjectInstanceId, ProjectState};
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphExecutionRequest {
    pub project_instance_id: ProjectInstanceId,
    pub graph_path: GraphResourcePath,
    pub demand: ExecutionDemand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphExecutionStreamEvent {
    RunEvent(RunEvent),
    RunOutput(RunOutputMessage),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryDisposition {
    Delivered,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalRunEventKind {
    Completed,
    Errored,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalRunEventDelivery {
    pub kind: TerminalRunEventKind,
    pub disposition: DeliveryDisposition,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphExecutionDeliveryReport {
    pub delivered_event_count: usize,
    pub rejected_event_count: usize,
    pub terminal: Option<TerminalRunEventDelivery>,
}

impl GraphExecutionDeliveryReport {
    pub const fn delivery_failed(&self) -> bool {
        self.rejected_event_count != 0
    }

    pub const fn delivered_terminal_kind(&self) -> Option<TerminalRunEventKind> {
        match self.terminal {
            Some(TerminalRunEventDelivery {
                kind,
                disposition: DeliveryDisposition::Delivered,
                ..
            }) => Some(kind),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct GraphExecutionOutcome {
    pub resource_mutation: Option<ResourceMutationResultDto>,
    pub delivery: GraphExecutionDeliveryReport,
}

#[derive(Debug)]
pub struct GraphExecutionError {
    pub project_error: ProjectExecutionError,
    pub delivery: GraphExecutionDeliveryReport,
}

impl std::fmt::Display for GraphExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.project_error.fmt(formatter)
    }
}

impl std::error::Error for GraphExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.project_error)
    }
}

pub fn execute_graph<D>(
    state: &ProjectState,
    request: GraphExecutionRequest,
    deliver: D,
) -> Result<GraphExecutionOutcome, GraphExecutionError>
where
    D: Fn(GraphExecutionStreamEvent) -> bool + Send + Sync,
{
    let bridge = RunEventDeliveryBridge::new(&deliver);
    let execution = state.execute_graph(
        &request.project_instance_id,
        &request.graph_path,
        &request.demand,
        &bridge,
    );
    let delivery = bridge.into_report();

    match execution {
        Ok(result) => Ok(GraphExecutionOutcome {
            resource_mutation: result.resource_mutation,
            delivery,
        }),
        Err(project_error) => Err(GraphExecutionError {
            project_error,
            delivery,
        }),
    }
}

struct RunEventDeliveryBridge<'a, D> {
    deliver: &'a D,
    report: Mutex<GraphExecutionDeliveryReport>,
}

impl<'a, D> RunEventDeliveryBridge<'a, D>
where
    D: Fn(GraphExecutionStreamEvent) -> bool + Send + Sync,
{
    fn new(deliver: &'a D) -> Self {
        Self {
            deliver,
            report: Mutex::new(GraphExecutionDeliveryReport::default()),
        }
    }

    fn deliver(&self, event: GraphExecutionStreamEvent, terminal: Option<TerminalRunEventKind>) {
        let disposition = if (self.deliver)(event) {
            DeliveryDisposition::Delivered
        } else {
            DeliveryDisposition::Rejected
        };
        let mut report = self.report.lock().unwrap();
        match disposition {
            DeliveryDisposition::Delivered => report.delivered_event_count += 1,
            DeliveryDisposition::Rejected => report.rejected_event_count += 1,
        }
        if let Some(kind) = terminal {
            report.terminal = Some(TerminalRunEventDelivery { kind, disposition });
        }
    }

    fn into_report(self) -> GraphExecutionDeliveryReport {
        self.report.into_inner().unwrap()
    }
}

impl<D> RunEventSink for RunEventDeliveryBridge<'_, D>
where
    D: Fn(GraphExecutionStreamEvent) -> bool + Send + Sync,
{
    fn record(&self, event: RunEvent) {
        let terminal = match event.kind {
            RunEventKind::RunCompleted => Some(TerminalRunEventKind::Completed),
            RunEventKind::RunErrored { .. } => Some(TerminalRunEventKind::Errored),
            RunEventKind::RunCancelled => Some(TerminalRunEventKind::Cancelled),
            _ => None,
        };
        self.deliver(GraphExecutionStreamEvent::RunEvent(event), terminal);
    }

    fn record_run_output(&self, event: RunOutputMessage) {
        self.deliver(GraphExecutionStreamEvent::RunOutput(event), None);
    }
}

#[cfg(test)]
#[path = "graph_execution/tests.rs"]
mod tests;
