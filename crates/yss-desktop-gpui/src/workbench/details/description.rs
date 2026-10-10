//! Current-node description follows the canvas result identity without retaining history.
mod column;
mod render;

use gpui_kit::{App, AppContext, Context, Entity, Subscription, Task, WeakEntity, Window};
use std::sync::Arc;
use yss_application::graph::results::{ResultValueProjection, description};
use yss_graph_document::{NodeId, PortAddress};
use yss_graph_execution::{
    plan::{PlanGraphId, PlanOutputRef, PlanPortAddress},
    result::{ResultCacheState, ResultReference},
};

use crate::{
    canvas::{CanvasEvent, GraphCanvas},
    services::NativeServices,
};
use column::ColumnSummary;

struct Source {
    graph: WeakEntity<GraphCanvas>,
    output: PlanOutputRef,
}

enum Content {
    Unloaded,
    Unavailable,
    Loading,
    Ready(Vec<Entity<ColumnSummary>>),
    Invalid,
    Failed,
}

pub(super) struct NodeDescription {
    services: Arc<NativeServices>,
    source: Option<Source>,
    observation: Option<Subscription>,
    reference: Option<ResultReference>,
    content: Content,
    expanded: bool,
    page: usize,
    columns_scroll: gpui_kit::ScrollHandle,
    generation: u64,
    task: Option<Task<()>>,
}

impl NodeDescription {
    pub(super) fn new(services: Arc<NativeServices>) -> Self {
        Self {
            services,
            source: None,
            observation: None,
            reference: None,
            content: Content::Unavailable,
            expanded: false,
            page: 0,
            columns_scroll: gpui_kit::ScrollHandle::new(),
            generation: 0,
            task: None,
        }
    }

    pub(super) fn clear(&mut self, cx: &mut Context<Self>) {
        if self.source.is_none() {
            return;
        }
        self.source = None;
        self.observation = None;
        self.reference = None;
        self.content = Content::Unavailable;
        self.expanded = false;
        self.page = 0;
        self.columns_scroll.set_offset(gpui_kit::Point::default());
        self.generation = self.generation.wrapping_add(1);
        self.task = None;
        cx.notify();
    }

    pub(super) fn set_node(
        &mut self,
        graph: Entity<GraphCanvas>,
        node: NodeId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let output = PlanOutputRef::new(
            PlanGraphId::from_existing(graph.read(cx).path().into()),
            PlanPortAddress::from_existing(
                PortAddress::declared(node, "result".parse().expect("result port"))
                    .to_string()
                    .into(),
            ),
        );
        if self.source.as_ref().is_some_and(|source| {
            source.graph.entity_id() == graph.entity_id() && source.output == output
        }) {
            self.refresh(window, cx);
            return;
        }
        self.clear(cx);
        self.source = Some(Source {
            graph: graph.downgrade(),
            output,
        });
        self.observation = Some(
            cx.subscribe_in(&graph, window, |view, _, event, window, cx| {
                if matches!(
                    event,
                    CanvasEvent::Projection { .. } | CanvasEvent::Execution
                ) {
                    view.refresh(window, cx);
                }
            }),
        );
        self.refresh(window, cx);
        cx.notify();
    }

    fn current_reference(&self, cx: &App) -> Option<ResultReference> {
        let source = self.source.as_ref()?;
        let graph = source.graph.upgrade()?;
        let canvas = graph.read(cx);
        let results = &canvas.graph.results;
        if results.semantic_input_hash != canvas.graph.projection.basis.semantic_input_hash
            || canvas.result_waiting(&source.output)
        {
            return None;
        }
        let ResultCacheState::Valid { result_id } = results.outputs.get(&source.output)? else {
            return None;
        };
        Some(ResultReference {
            execution_session_id: results.execution_session_id,
            result_id: *result_id,
        })
    }

    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let reference = self.current_reference(cx);
        if self.reference == reference {
            return;
        }
        self.reference = reference;
        self.generation = self.generation.wrapping_add(1);
        self.task = None;
        self.page = 0;
        self.columns_scroll.set_offset(gpui_kit::Point::default());
        self.content = if reference.is_some() {
            Content::Unloaded
        } else {
            Content::Unavailable
        };
        if self.expanded {
            self.read(window, cx);
        }
        cx.notify();
    }

    fn read(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(reference) = self
            .reference
            .filter(|reference| Some(*reference) == self.current_reference(cx))
        else {
            return;
        };
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.content = Content::Loading;
        let task = self.services.run(move |services| {
            Ok(
                match services.application.query_result_projection(reference)? {
                    Some(ResultValueProjection::Value(value)) => {
                        Some(description::columns(&value).map_err(|_| ()))
                    }
                    Some(_) => Some(Err(())),
                    None => None,
                },
            )
        });
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, _, cx| {
                if view.generation != generation || view.current_reference(cx) != Some(reference) {
                    return;
                }
                view.content = match result {
                    Ok(Some(Ok(columns))) => Content::Ready(
                        columns
                            .into_iter()
                            .map(|column| cx.new(|_| ColumnSummary::new(column)))
                            .collect(),
                    ),
                    Ok(Some(Err(()))) => Content::Invalid,
                    Ok(None) => Content::Unavailable,
                    Err(_) => Content::Failed,
                };
                cx.notify();
            });
        }));
        cx.notify();
    }
}

impl super::DetailsPanel {
    pub(super) fn refresh_node_description(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self
            .node()
            .filter(|node| node.node_type.as_str() == "yssbi.statistics.describe")
            .map(|node| node.node_id)
            .zip(self.graph());
        self.description.update(cx, |description, cx| match target {
            Some((node, graph)) => description.set_node(graph, node, window, cx),
            None => description.clear(cx),
        });
    }
}
