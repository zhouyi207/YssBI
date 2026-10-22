//! Project-wide graph facts shared by the Problems, Results and Output panels.
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use gpui_kit::{Context, Task};
use yss_application::graph::{
    editing::RunRecord,
    overview::GraphOverview,
    run::{RunIdentity, run_graph_outputs},
};
use yss_graph_document::GraphResourcePath;
use yss_graph_execution::{plan::PlanOutputRef, result::ResultReference};
use yss_project_identity::ProjectInstanceId;
use yss_ui::lifecycle::LogReleased;

use crate::GraphServices;

/// Bursts of edits and run events coalesce into one Application read.
const REFRESH_DELAY: Duration = Duration::from_millis(150);

#[derive(Default)]
struct Pending {
    all: bool,
    graphs: BTreeSet<GraphResourcePath>,
    results: BTreeSet<GraphResourcePath>,
    runs: bool,
}

impl Pending {
    fn is_empty(&self) -> bool {
        !self.all && self.graphs.is_empty() && self.results.is_empty() && !self.runs
    }
}

pub struct ProjectOverview {
    services: Arc<GraphServices>,
    project: ProjectInstanceId,
    graphs: BTreeMap<GraphResourcePath, GraphOverview>,
    unavailable: BTreeSet<GraphResourcePath>,
    runs: Rc<[RunRecord]>,
    loaded: bool,
    pending: Pending,
    refresh: Option<Task<()>>,
}

impl ProjectOverview {
    pub fn new(
        services: Arc<GraphServices>,
        project: ProjectInstanceId,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut overview = Self {
            services,
            project,
            graphs: BTreeMap::new(),
            unavailable: BTreeSet::new(),
            runs: Rc::default(),
            loaded: false,
            pending: Pending::default(),
            refresh: None,
        };
        overview.invalidate_all(cx);
        overview
    }

    pub fn project(&self) -> &ProjectInstanceId {
        &self.project
    }

    /// Whether every project graph has been read at least once.
    pub fn loaded(&self) -> bool {
        self.loaded
    }

    pub fn graphs(&self) -> impl Iterator<Item = &GraphOverview> {
        self.graphs.values()
    }

    pub fn graph(&self, path: &GraphResourcePath) -> Option<&GraphOverview> {
        self.graphs.get(path)
    }

    pub fn unavailable(&self) -> &BTreeSet<GraphResourcePath> {
        &self.unavailable
    }

    /// Recent runs of this execution session, in run order.
    pub fn runs(&self) -> &Rc<[RunRecord]> {
        &self.runs
    }

    pub fn running_outputs<'a>(
        &'a self,
        graph: &'a GraphResourcePath,
    ) -> impl Iterator<Item = &'a PlanOutputRef> {
        self.runs
            .iter()
            .filter(move |run| run.finished_at().is_none() && run.identity().graph_path() == graph)
            .flat_map(RunRecord::outputs)
    }

    pub fn invalidate_all(&mut self, cx: &mut Context<Self>) {
        self.pending.all = true;
        self.pending.runs = true;
        self.schedule(cx);
    }

    pub fn invalidate_graph(&mut self, path: GraphResourcePath, cx: &mut Context<Self>) {
        self.pending.graphs.insert(path);
        self.schedule(cx);
    }

    /// Execution changed results and runs; documents are unchanged.
    pub fn invalidate_execution(&mut self, path: GraphResourcePath, cx: &mut Context<Self>) {
        self.pending.results.insert(path);
        self.pending.runs = true;
        self.schedule(cx);
    }

    pub fn invalidate_runs(&mut self, cx: &mut Context<Self>) {
        self.pending.runs = true;
        self.schedule(cx);
    }

    /// Computes one output, reusing current upstream results, and returns its result.
    pub fn compute(
        &self,
        graph: GraphResourcePath,
        output: PlanOutputRef,
    ) -> tokio::task::JoinHandle<anyhow::Result<Option<ResultReference>>> {
        let project = self.project.clone();
        self.services.run(move |application| {
            let receipt = run_graph_outputs(
                application,
                &project,
                &graph,
                Box::new([output.clone()]),
                Arc::new(AtomicBool::new(false)),
            )?;
            Ok(receipt
                .results
                .iter()
                .find(|result| result.output == output)
                .map(|result| ResultReference {
                    execution_session_id: *receipt.identity.execution_session_id(),
                    result_id: result.result_id,
                }))
        })
    }

    /// Dismisses run notices for every view of their graphs.
    pub fn acknowledge(&self, runs: Vec<RunIdentity>, cx: &mut Context<Self>) {
        if runs.is_empty() {
            return;
        }
        let project = self.project.clone();
        let task = self.services.run(move |application| {
            application.acknowledge_runs(&project, &runs)?;
            Ok(())
        });
        cx.spawn(async move |_, _| {
            if let Err(error) = task.await.map_err(anyhow::Error::from).and_then(|result| result) {
                tracing::warn!(%error, code = "run_acknowledge_failed", "Run notices were not dismissed");
            }
        })
        .detach();
    }

    fn schedule(&mut self, cx: &mut Context<Self>) {
        if self.refresh.is_some() || self.pending.is_empty() {
            return;
        }
        self.refresh = Some(cx.spawn(async move |view, cx| {
            cx.background_executor().timer(REFRESH_DELAY).await;
            let Ok(Some((pending, hashes, project, services))) = view.update(cx, |view, _| {
                let pending = std::mem::take(&mut view.pending);
                if pending.is_empty() {
                    view.refresh = None;
                    return None;
                }
                let hashes = pending
                    .results
                    .iter()
                    .filter(|path| !pending.all && !pending.graphs.contains(*path))
                    .filter_map(|path| {
                        let graph = view.graphs.get(path)?;
                        Some((path.clone(), graph.projection.basis.semantic_input_hash))
                    })
                    .collect::<Vec<_>>();
                Some((pending, hashes, view.project.clone(), view.services.clone()))
            }) else {
                return;
            };
            let locale = yss_ui::text::locale();
            let all = pending.all;
            let graphs = pending.graphs.iter().cloned().collect::<Vec<_>>();
            let runs = pending.runs;
            let task = services.run(move |application| {
                let overviews = if all {
                    Some(application.graph_overviews(&project, None, locale)?)
                } else if !graphs.is_empty() {
                    Some(application.graph_overviews(&project, Some(&graphs), locale)?)
                } else {
                    None
                };
                let states = if hashes.is_empty() {
                    Vec::new()
                } else {
                    application.graph_result_states(&project, &hashes)?
                };
                let journal = if runs {
                    Some(application.run_journal(&project)?)
                } else {
                    None
                };
                Ok((overviews, states, journal))
            });
            let result = task.await.map_err(anyhow::Error::from).and_then(|result| result);
            view.update(cx, |view, cx| {
                view.refresh = None;
                match result {
                    Ok((overviews, states, journal)) => {
                        if let Some(overviews) = overviews {
                            let project_graphs =
                                overviews.project_graphs.iter().collect::<BTreeSet<_>>();
                            view.graphs.retain(|path, _| project_graphs.contains(path));
                            view.unavailable.retain(|path| project_graphs.contains(path));
                            for path in overviews.unavailable {
                                view.graphs.remove(&path);
                                view.unavailable.insert(path);
                            }
                            for graph in overviews.graphs {
                                view.unavailable.remove(&graph.graph_path);
                                view.graphs.insert(graph.graph_path.clone(), graph);
                            }
                            view.loaded |= all;
                        }
                        for (path, state) in states {
                            if let Some(graph) = view.graphs.get_mut(&path)
                                && graph.projection.basis.semantic_input_hash
                                    == state.semantic_input_hash
                            {
                                graph.results = Some(state);
                            }
                        }
                        if let Some(journal) = journal {
                            view.runs = journal.into();
                        }
                        cx.notify();
                    }
                    Err(error) => {
                        // A replaced project rebuilds its overview; later events retry the rest.
                        tracing::debug!(%error, code = "project_overview_refresh_failed", "Project overview refresh failed");
                    }
                }
                view.schedule(cx);
            })
            .log_released();
        }));
    }
}
