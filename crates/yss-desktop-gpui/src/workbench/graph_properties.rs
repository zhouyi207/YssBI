//! Graph-wide read projections and input drafts. Project owns every committed value.
mod constants;
mod query;
mod signature;
mod value;

use crate::{canvas::GraphCanvas, services::NativeServices};
use constants::{ConstantDraft, ConstantOverview};
use gpui::{
    Context, Entity, IntoElement, Render, Subscription, WeakEntity, Window, div, prelude::*,
};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
};
use signature::SignatureDraft;
use std::sync::Arc;
use yss_project::GraphEditVersion;

pub(super) struct GraphProperties {
    services: Arc<NativeServices>,
    graph: Option<WeakEntity<GraphCanvas>>,
    version: Option<GraphEditVersion>,
    generation: u64,
    loading: bool,
    ready: bool,
    constants: Vec<ConstantDraft>,
    constants_page: usize,
    constants_open: bool,
    signature: Option<SignatureDraft>,
    error: Option<String>,
    graph_observer: Option<Subscription>,
}

impl GraphProperties {
    pub fn new(services: Arc<NativeServices>) -> Self {
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
            error: None,
            graph_observer: None,
        }
    }

    pub fn names(&self) -> Vec<(String, String)> {
        self.constants
            .iter()
            .map(|field| (field.model.id.to_string(), field.model.name.clone()))
            .collect()
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
        self.error = None;
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
                self.graph_observer = Some(cx.observe(&entity, |_, _, cx| cx.notify()));
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

    fn accepts_input(&self, generation: u64, cx: &gpui::App) -> bool {
        generation == self.generation
            && self.ready
            && !self.loading
            && self.graph().is_some_and(|graph| {
                let graph = graph.read(cx);
                !graph.busy() && Some(graph.graph.editing.version) == self.version
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
        let error = self.error.clone().or_else(|| {
            self.graph()
                .and_then(|graph| graph.read(cx).command_error().map(str::to_owned))
        });
        let title = self
            .graph()
            .map(|graph| graph.read(cx).title_text())
            .unwrap_or_default();
        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .truncate()
                            .child(title),
                    )
                    .child(
                        Button::new("refresh-graph-properties")
                            .small()
                            .ghost()
                            .icon(IconName::Redo2)
                            .tooltip("刷新图属性")
                            .disabled(self.loading)
                            .on_click(cx.listener(|view, _, window, cx| {
                                if let Some(graph) = view.graph() {
                                    graph.update(cx, |graph, cx| graph.refresh(cx));
                                }
                                view.read(window, cx);
                            })),
                    ),
            )
            .when(self.signature.is_some(), |view| {
                view.child(super::controls::hint(
                    crate::text::translate("detail.typeLabels.function_graph"),
                    cx,
                ))
            })
            .when(self.loading, |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("正在读取图属性…"),
                )
            })
            .children(error.as_ref().map(|error| {
                div()
                    .id("graph-properties-error")
                    .role(gpui::accesskit::Role::Alert)
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
            }))
            .children(
                self.signature
                    .as_ref()
                    .map(|signature| self.render_signature(signature, busy, cx)),
            )
            .child(self.render_constants(busy, cx))
    }
}
