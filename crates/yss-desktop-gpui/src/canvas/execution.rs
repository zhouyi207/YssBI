//! Read projections of Application run facts and native command lifecycle.
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use gpui_kit::Context;
use yss_application::graph::run::{
    ExecutionApplicationError, RunApplicationEvent, RunApplicationEventKind, RunDemand,
    RunGraphRequest, RunIdentity, cancel_run, run_graph_with_sink,
};
use yss_graph_execution::plan::PlanOutputRef;
use yss_graph_execution::run_registry::RunId;

use super::{CanvasEvent, GraphCanvas};

#[derive(Default)]
pub(super) struct ExecutionView {
    runs: BTreeMap<RunId, RunProjection>,
    submitting: Option<Arc<AtomicBool>>,
    cancelling: bool,
    recovery: Option<Vec<RunApplicationEvent>>,
    recover_again: bool,
    unknown: bool,
    cleared: Option<RunIdentity>,
}

struct RunProjection {
    event: RunApplicationEvent,
    outputs: Box<[PlanOutputRef]>,
}

impl ExecutionView {
    fn current_runs<'a>(
        &'a self,
        graph: &'a crate::project::OpenedGraph,
    ) -> impl Iterator<Item = &'a RunProjection> {
        self.runs.values().filter(|run| {
            let identity = run.event.identity();
            identity.graph_path() == &graph.projection.graph_path
                && identity.execution_session_id() == &graph.results.execution_session_id
                && identity.semantic_input_hash() == &graph.projection.basis.semantic_input_hash
        })
    }

    pub(super) fn running_outputs<'a>(
        &'a self,
        graph: &'a crate::project::OpenedGraph,
    ) -> impl Iterator<Item = &'a PlanOutputRef> {
        self.current_runs(graph)
            .filter(|run| matches!(run.event.kind(), RunApplicationEventKind::RunStarted { .. }))
            .flat_map(|run| run.outputs.iter())
    }

    pub(super) fn pending_outputs<'a>(
        &'a self,
        graph: &'a crate::project::OpenedGraph,
    ) -> impl Iterator<Item = &'a PlanOutputRef> {
        self.current_runs(graph)
            .filter(|run| run.event.result_revision() > graph.results.revision)
            .flat_map(|run| run.outputs.iter())
    }
    pub fn running(&self) -> bool {
        self.submitting.is_some() || self.active().is_some()
    }

    fn active(&self) -> Option<&RunIdentity> {
        self.runs.values().rev().find_map(|projection| {
            let event = &projection.event;
            matches!(event.kind(), RunApplicationEventKind::RunStarted { .. })
                .then_some(event.identity())
        })
    }

    fn install(&mut self, event: RunApplicationEvent) -> bool {
        let id = event.identity().run_id();
        if let Some(previous) = self.runs.get(&id).map(|projection| &projection.event)
            && (!matches!(previous.kind(), RunApplicationEventKind::RunStarted { .. })
                || previous == &event)
        {
            return false;
        }
        let outputs = match event.kind() {
            RunApplicationEventKind::RunStarted { outputs } => outputs.clone(),
            _ => self
                .runs
                .get(&id)
                .map(|projection| projection.outputs.clone())
                .unwrap_or_default(),
        };
        self.runs.insert(id, RunProjection { event, outputs });
        let newest = self.runs.last_key_value().map(|(id, _)| *id);
        self.runs.retain(|id, projection| {
            Some(*id) == newest
                || matches!(
                    projection.event.kind(),
                    RunApplicationEventKind::RunStarted { .. }
                )
        });
        true
    }
}

impl GraphCanvas {
    pub(crate) fn can_run(&self) -> bool {
        self.execution_unavailable_reason().is_none()
    }

    fn execution_unavailable_reason(&self) -> Option<&'static str> {
        if self.graph.projection.graph_path.kind()
            != yss_graph_document::GraphResourceKind::EventGraph
        {
            Some("canvas.functionRunUnavailable")
        } else if self.busy {
            Some("native.canvas.operationInProgress")
        } else if self.refresh_failed {
            Some("native.canvas.refreshFailed")
        } else if self.resource_move.is_some() {
            Some("native.canvas.loadingRenamedGraph")
        } else if self.execution.running() {
            Some("canvas.executing")
        } else {
            self.execution_sync_status()
        }
    }

    pub(super) fn execution_sync_status(&self) -> Option<&'static str> {
        if self.execution.unknown {
            Some("native.canvas.runUnknown")
        } else if self.execution.recovery.is_some() {
            Some("native.canvas.syncingRun")
        } else {
            None
        }
    }

    pub(super) fn graph_run_unavailable_reason(&self) -> Option<&'static str> {
        self.execution_unavailable_reason().or_else(|| {
            // Pending port input may fix the problem; recheck after its transaction.
            (self.presentation.graph_blocked && !self.has_dirty_port_inputs())
                .then_some("canvas.problemsBlockExecution")
        })
    }

    pub(super) fn run_selected(
        &mut self,
        mode: yss_graph_execution::plan::NodeExecutionMode,
        cx: &mut Context<Self>,
    ) {
        if self.selected.len() == 1
            && let Some(node_id) = self.selected.iter().next().copied()
        {
            self.run_graph(RunDemand::Node { node_id, mode }, cx);
        }
    }

    pub(crate) fn run_status(&self) -> &'static str {
        if self.execution.cancelling
            || self
                .execution
                .submitting
                .as_ref()
                .is_some_and(|cancellation| cancellation.load(Ordering::Acquire))
        {
            crate::text::t("native.canvas.cancelling")
        } else if self.execution.running() {
            crate::text::t("native.canvas.running")
        } else if self.execution.unknown {
            crate::text::t("native.canvas.runUnknown")
        } else if self.execution.recovery.is_some() {
            crate::text::t("native.canvas.syncingRun")
        } else {
            match self.run_notice().map(RunApplicationEvent::kind) {
                Some(RunApplicationEventKind::RunCompleted) => {
                    crate::text::t("native.canvas.runCompleted")
                }
                Some(RunApplicationEventKind::RunCancelled) => {
                    crate::text::t("plugins.taskStates.cancelled")
                }
                Some(RunApplicationEventKind::RunErrored { .. }) => {
                    crate::text::t("native.canvas.runFailed")
                }
                _ => "",
            }
        }
    }

    pub(crate) fn is_running(&self) -> bool {
        self.execution.running()
    }

    pub(crate) fn run_graph(&mut self, demand: RunDemand, cx: &mut Context<Self>) {
        if !self.can_run()
            || (matches!(demand, RunDemand::Default)
                && self.graph_run_unavailable_reason().is_some())
        {
            return;
        }
        if self.has_dirty_port_inputs() {
            self.submit(super::GraphCommand::RunAfterPortInputs(demand), None, cx);
            return;
        }
        self.context_menu = None;
        let project = self.graph.project.clone();
        let path = self.graph.projection.graph_path.clone();
        let version = self.graph.editing.version;
        let hash = self.graph.projection.basis.semantic_input_hash;
        let execution_session_id = self.graph.results.execution_session_id;
        let cancellation = Arc::new(AtomicBool::new(false));
        self.execution.submitting = Some(cancellation.clone());
        self.execution.cleared = self
            .execution
            .runs
            .last_key_value()
            .map(|(_, projection)| projection.event.identity().clone());
        self.error = None;
        self.refresh_presentation();
        let task = self.services.run(move |services| {
            let application = &services.application;
            // Capture the document matching the displayed projection, never a later revision.
            let document = application.current_graph_document(&project, &path, version)?;
            let request = RunGraphRequest::new(project, path, document, hash)
                .with_demand(demand)
                .with_cancellation(cancellation);
            let mut terminal_received = false;
            let mut failed = false;
            let result = run_graph_with_sink(application, request, |event| {
                match event.kind() {
                    RunApplicationEventKind::RunErrored { .. } => {
                        terminal_received = true;
                        failed = true;
                    }
                    RunApplicationEventKind::RunCompleted
                    | RunApplicationEventKind::RunCancelled => terminal_received = true,
                    _ => {}
                }
                true
            });
            // Application publishes the terminal fact before invoking this sink.
            // Only failures before that publication need local command feedback.
            if !terminal_received {
                result?;
            }
            Ok(failed)
        });
        cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update(cx, |view, cx| {
                view.execution.submitting = None;
                if view.graph.results.execution_session_id != execution_session_id {
                    view.resync_execution(cx);
                    return;
                }
                match result {
                    Ok(true) if view.graph.projection.basis.semantic_input_hash == hash => {
                        cx.emit(CanvasEvent::ShowOutput);
                    }
                    Ok(_) => {}
                    Err(error) => {
                        tracing::warn!(
                            code = "native_run_request_rejected",
                            "Native run request rejected"
                        );
                        view.error = Some(run_rejection(&error));
                    }
                }
                view.resync_execution(cx);
                cx.emit(CanvasEvent::Execution);
                cx.notify();
            });
        })
        .detach();
        cx.emit(CanvasEvent::Execution);
        cx.notify();
    }

    pub(crate) fn cancel_run(&mut self, cx: &mut Context<Self>) {
        if self.execution.cancelling {
            return;
        }
        if let Some(cancellation) = &self.execution.submitting {
            cancellation.store(true, Ordering::Release);
        }
        let Some(identity) = self.execution.active().cloned() else {
            cx.emit(CanvasEvent::Execution);
            cx.notify();
            return;
        };
        self.execution.cancelling = true;
        let task = self.services.run(move |services| {
            Ok(cancel_run(
                &services.application,
                *identity.execution_session_id(),
                identity.run_id(),
            )?)
        });
        cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update(cx, |view, cx| {
                view.execution.cancelling = false;
                if result.is_err() {
                    view.error = Some(crate::text::t("native.canvas.cancelFailed").into());
                }
                view.resync_execution(cx);
                cx.emit(CanvasEvent::Execution);
                cx.notify();
            });
        })
        .detach();
        cx.emit(CanvasEvent::Execution);
        cx.notify();
    }

    pub(crate) fn accept_execution(&mut self, event: RunApplicationEvent, cx: &mut Context<Self>) {
        if event.identity().graph_path() != &self.graph.projection.graph_path
            || event.identity().execution_session_id() != &self.graph.results.execution_session_id
            || matches!(
                event.kind(),
                RunApplicationEventKind::ResultInspectionRequested { .. }
            )
        {
            return;
        }
        if let Some(buffer) = &mut self.execution.recovery {
            if buffer.len() >= 256 {
                buffer.clear();
                self.execution.recover_again = true;
            }
            buffer.push(event);
            return;
        }
        let result_revision = event.result_revision();
        if self.execution.install(event) {
            self.refresh_presentation();
            if result_revision > self.graph.results.revision {
                self.refresh(cx);
            }
            cx.emit(CanvasEvent::Execution);
            cx.notify();
        }
    }

    pub(crate) fn resync_execution(&mut self, cx: &mut Context<Self>) {
        if self.execution.recovery.is_some() {
            self.execution.recover_again = true;
            return;
        }
        self.execution.recovery = Some(Vec::new());
        self.execution.recover_again = false;
        let project = self.graph.project.clone();
        let task = self
            .services
            .run(move |services| Ok(services.application.execution_snapshot(&project)?));
        cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update(cx, |view, cx| {
                let buffered = view.execution.recovery.take().unwrap_or_default();
                if view.execution.recover_again {
                    view.resync_execution(cx);
                    return;
                }
                match result {
                    Ok(events) => {
                        view.execution.unknown = false;
                        view.execution.runs.clear();
                        for event in events.into_iter().chain(buffered) {
                            view.accept_execution(event, cx);
                        }
                    }
                    Err(_) => {
                        view.execution.unknown = true;
                        for event in buffered {
                            view.accept_execution(event, cx);
                        }
                    }
                }
                view.refresh_presentation();
                cx.emit(CanvasEvent::Execution);
                cx.notify();
            });
        })
        .detach();
        cx.emit(CanvasEvent::Execution);
        cx.notify();
    }

    fn run_notice(&self) -> Option<&RunApplicationEvent> {
        let (_, projection) = self.execution.runs.last_key_value()?;
        let event = &projection.event;
        (event.identity().graph_path() == &self.graph.projection.graph_path
            && event.identity().execution_session_id() == &self.graph.results.execution_session_id
            && event.identity().semantic_input_hash()
                == &self.graph.projection.basis.semantic_input_hash
            && self.execution.cleared.as_ref() != Some(event.identity()))
        .then_some(event)
    }

    pub(crate) fn run_failure(&self) -> Option<&RunApplicationEvent> {
        self.run_notice()
            .filter(|event| matches!(event.kind(), RunApplicationEventKind::RunErrored { .. }))
    }

    pub(crate) fn clearable_run(&self) -> Option<&RunIdentity> {
        if self.execution.running() || self.execution_sync_status().is_some() {
            return None;
        }
        self.run_notice()
            .filter(|event| {
                matches!(
                    event.kind(),
                    RunApplicationEventKind::RunCompleted
                        | RunApplicationEventKind::RunCancelled
                        | RunApplicationEventKind::RunErrored { .. }
                )
            })
            .map(RunApplicationEvent::identity)
    }

    pub(crate) fn clear_run_notice(&mut self, run: &RunIdentity, cx: &mut Context<Self>) {
        if self.clearable_run() != Some(run) {
            return;
        }
        self.execution.cleared = Some(run.clone());
        self.refresh_presentation();
        cx.emit(CanvasEvent::Execution);
        cx.notify();
    }

    pub(crate) fn inspect_results(&self, cx: &mut Context<Self>) {
        cx.emit(CanvasEvent::ShowResults);
    }

    pub(crate) fn result_waiting(&self, output: &PlanOutputRef) -> bool {
        self.execution
            .pending_outputs(&self.graph)
            .any(|pending| pending == output)
    }
}

impl Drop for GraphCanvas {
    fn drop(&mut self) {
        if let Some(cancellation) = &self.execution.submitting {
            cancellation.store(true, Ordering::Release);
        }
        let active: Vec<_> = self
            .execution
            .runs
            .values()
            .filter(|projection| {
                matches!(
                    projection.event.kind(),
                    RunApplicationEventKind::RunStarted { .. }
                )
            })
            .map(|projection| projection.event.identity().clone())
            .collect();
        if !active.is_empty() {
            self.services.run(move |services| {
                for identity in active {
                    let _ = cancel_run(
                        &services.application,
                        *identity.execution_session_id(),
                        identity.run_id(),
                    );
                }
                Ok(())
            });
        }
    }
}

fn run_rejection(error: &anyhow::Error) -> String {
    let cause = match error.downcast_ref::<ExecutionApplicationError>() {
        Some(ExecutionApplicationError::DraftChanged) => "graph_draft_changed",
        Some(ExecutionApplicationError::GraphNotReady) => "graph_not_ready",
        Some(ExecutionApplicationError::GraphResolutionFailed { .. }) => "graph_resolution_failed",
        Some(ExecutionApplicationError::GraphPlan(_)) => "graph_plan_failed",
        Some(
            ExecutionApplicationError::SessionCapture(_)
            | ExecutionApplicationError::StaleSession(_),
        ) => "stale_project_lifecycle",
        Some(ExecutionApplicationError::Cancelled) => {
            return crate::text::t("native.canvas.runCancelled").into();
        }
        Some(ExecutionApplicationError::DeadlineExceeded) => "deadlineExceeded",
        _ => return crate::text::t("native.canvas.runRequestFailed").into(),
    };
    crate::text::translate(&format!("runFailure.causes.{cause}"))
}
