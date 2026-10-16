//! Graph-wide read projections and input drafts. Project owns every committed value.
mod constants;
mod query;
mod signature;
mod value;

use crate::{GraphServices, canvas::GraphCanvas};
use constants::{ConstantDraft, ConstantOverview};
use gpui_kit::assets::IconName;
use gpui_kit::component::Disableable;
use gpui_kit::{
    Context, Entity, IntoElement, Render, Subscription, WeakEntity, Window, div, prelude::*,
};
use signature::SignatureDraft;
use std::sync::Arc;
use yss_project::GraphEditVersion;
use yss_ui::inspector::{self, InspectorHeader};

pub(super) struct GraphProperties {
    services: Arc<GraphServices>,
    graph: Option<WeakEntity<GraphCanvas>>,
    version: Option<GraphEditVersion>,
    generation: u64,
    loading: bool,
    ready: bool,
    constants: Vec<ConstantDraft>,
    constants_page: usize,
    constants_open: bool,
    signature: Option<SignatureDraft>,
    notifications: yss_ui::notifications::Notifications,
    graph_observer: Option<Subscription>,
}

impl GraphProperties {
    pub fn new(services: Arc<GraphServices>, window: &Window) -> Self {
        Self {
            services,
            graph: None,
            version: None,
            generation: 0,
            loading: false,
            ready: false,
            constants: vec![],
            constants_page: 0,
            constants_open: true,
            signature: None,
            notifications: yss_ui::notifications::Notifications::new(window),
            graph_observer: None,
        }
    }

    pub fn names(&self) -> impl Iterator<Item = (yss_graph_document::ConstantId, &str)> + Clone {
        self.constants
            .iter()
            .map(|field| (field.model.id, field.model.name.as_str()))
    }

    pub fn loading(&self) -> bool {
        self.loading || !self.ready
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.graph = None;
        self.version = None;
        self.constants.clear();
        self.constants_page = 0;
        self.constants_open = true;
        self.signature = None;
        self.graph_observer = None;
        self.generation = self.generation.wrapping_add(1);
        self.loading = false;
        self.ready = false;

        cx.notify();
    }

    pub fn set_graph(
        &mut self,
        graph: WeakEntity<GraphCanvas>,
        needed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let same = self
            .graph
            .as_ref()
            .is_some_and(|current| current.entity_id() == graph.entity_id());
        if !same {
            self.clear(cx);
            if let Some(entity) = graph.upgrade() {
                let canvas = entity.read(cx);
                let mut display = (
                    canvas.can_edit(),
                    canvas.dirty(cx),
                    canvas.graph.editing.version,
                );
                let mut projection = canvas.graph.projection.clone();
                self.graph_observer = Some(cx.observe(&entity, move |_, entity, cx| {
                    let canvas = entity.read(cx);
                    let next = (
                        canvas.can_edit(),
                        canvas.dirty(cx),
                        canvas.graph.editing.version,
                    );
                    if display == next && Arc::ptr_eq(&projection, &canvas.graph.projection) {
                        return;
                    }
                    display = next;
                    projection = canvas.graph.projection.clone();
                    cx.notify();
                }));
            }
            self.graph = Some(graph);
        }
        let version = self
            .graph()
            .map(|graph| graph.read(cx).graph.editing.version);
        if needed && (self.version != version || (!self.ready && !self.loading)) {
            self.read(window, cx);
        }
    }

    fn graph(&self) -> Option<Entity<GraphCanvas>> {
        self.graph.as_ref().and_then(WeakEntity::upgrade)
    }

    fn accepts_input(&self, generation: u64, cx: &gpui_kit::App) -> bool {
        generation == self.generation
            && self.ready
            && !self.loading
            && self.graph().is_some_and(|graph| {
                let graph = graph.read(cx);
                graph.can_edit() && Some(graph.graph.editing.version) == self.version
            })
    }

    fn install_constants(&mut self, constants: Vec<ConstantOverview>) {
        let mut previous = std::mem::take(&mut self.constants)
            .into_iter()
            .map(|field| (field.model.id, field))
            .collect::<std::collections::HashMap<_, _>>();
        self.constants = constants
            .into_iter()
            .map(|model| match previous.remove(&model.id) {
                Some(field) if field.model == model => field,
                _ => ConstantDraft::new(model),
            })
            .collect();
        self.constants_page = self
            .constants_page
            .min(self.constants.len().saturating_sub(1) / constants::PAGE_CONSTANTS);
    }
}

impl Render for GraphProperties {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.constants_open {
            for field in self
                .constants
                .iter_mut()
                .skip(self.constants_page * constants::PAGE_CONSTANTS)
                .take(constants::PAGE_CONSTANTS)
            {
                field.ensure_inputs(window, cx);
            }
        }
        let busy = !self.accepts_input(self.generation, cx);
        let title = self
            .graph()
            .map(|graph| graph.read(cx).title_text(cx))
            .unwrap_or_default();
        let header = InspectorHeader::new(IconName::Workflow, title)
            .when(self.signature.is_some(), |header| {
                header.detail(yss_ui::text::translate("detail.typeLabels.function_graph"))
            })
            .tool(
                inspector::action_button(
                    "refresh-graph-properties",
                    IconName::RefreshCw,
                    yss_ui::text::translate("native.workbench.refreshGraphProperties"),
                )
                .disabled(self.loading)
                .on_click(cx.listener(|view, _, window, cx| {
                    if let Some(graph) = view.graph() {
                        graph.update(cx, |graph, cx| graph.refresh(cx));
                    }
                    view.read(window, cx);
                })),
            );
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .child(header)
            .when(self.loading, |view| {
                view.child(inspector::status_band(
                    inspector::hint(
                        yss_ui::text::t("native.workbench.loadingGraphProperties"),
                        cx,
                    ),
                    cx,
                ))
            })
            .children(
                self.signature
                    .as_ref()
                    .map(|signature| self.render_signature(signature, busy, cx)),
            )
            .child(self.render_constants(busy, cx))
    }
}
