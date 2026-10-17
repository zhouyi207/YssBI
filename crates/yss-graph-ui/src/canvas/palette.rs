//! A transient catalog browser. Creation still belongs to the captured graph transaction.
mod browser;
mod query;
mod render;

use super::GraphCanvas;
use crate::{
    GraphServices,
    node_creation::{CreationEvent, CreationTarget, NodeCreationView},
};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    App, AppContext, Context, Entity, EntityInputHandler, EventEmitter, Focusable, KeyDownEvent,
    ScrollStrategy, Subscription, Task, UniformListScrollHandle, WeakEntity, Window,
};
use std::sync::Arc;
use yss_application::activity_panel::{ActivityItem, ActivityPanelDocument, ActivityRowContent};
use yss_graph_document::{GraphResourcePath, PortAddress};
use yss_node_catalog::NodeCreation;
use yss_node_protocol::{InitialPortCounts, ParameterValues};
use yss_project::GraphEditVersion;
use yss_project_identity::ProjectInstanceId;

pub(super) struct PaletteTarget {
    pub graph: WeakEntity<GraphCanvas>,
    pub project: ProjectInstanceId,
    pub path: GraphResourcePath,
    pub version: GraphEditVersion,
    pub source: Option<PortAddress>,
}

pub(super) enum PaletteEvent {
    Dismiss,
    ConfigurationChanged,
    Create {
        descriptor: NodeCreation,
        parameters: ParameterValues,
        port_counts: InitialPortCounts,
    },
}

pub(super) struct NodePalette {
    services: Arc<GraphServices>,
    target: PaletteTarget,
    browser: browser::Browser,
    search: Entity<InputState>,
    focus: gpui_kit::FocusHandle,
    scroll: UniformListScrollHandle,
    configure_first: bool,
    configuration: Option<Entity<NodeCreationView>>,
    configuration_subscription: Option<Subscription>,
    creating: bool,
    language: &'static str,
    loading: bool,
    refresh_pending: bool,
    generation: u64,
    error: Option<&'static str>,
    notifications: yss_ui::notifications::Notifications,
    task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl NodePalette {
    fn set_error(&mut self, error: Option<&'static str>, cx: &mut gpui_kit::Context<Self>) {
        if let Some(error) = &error {
            if *error == "native.canvas.catalogFailed" {
                self.notifications.error_with_retry(
                    yss_ui::text::translate(error),
                    cx,
                    |view, _, cx| view.query(cx),
                );
            } else {
                self.notifications.error(yss_ui::text::translate(error), cx);
            }
        }
        self.error = error;
    }

    pub(super) fn new(
        services: Arc<GraphServices>,
        target: PaletteTarget,
        catalog: Option<Arc<ActivityPanelDocument>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(yss_ui::text::translate(
                "canvas.nodePalette.searchPlaceholder",
            ))
        });
        window.focus(&search.focus_handle(cx), cx);
        let mut subscriptions = vec![cx.subscribe(&search, |view, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                view.browser.set_query(&view.search.read(cx).value());
                view.scroll_to_active();
                cx.notify();
            }
        })];
        if let Some(graph) = target.graph.upgrade() {
            let mut current = true;
            subscriptions.push(cx.observe(&graph, move |view, _, cx| {
                let next = view.current(cx);
                if current && !next && !view.creating && view.configuration.is_none() {
                    view.notifications
                        .error(yss_ui::text::t("native.workbench.creationChanged"), cx);
                }
                current = next;
                cx.notify();
            }));
        }
        let refresh_pending = catalog.is_none();
        Self {
            services,
            target,
            browser: browser::Browser::new(catalog),
            search,
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            configure_first: false,
            configuration: None,
            configuration_subscription: None,
            creating: false,
            language: yss_ui::text::locale(),
            loading: false,
            refresh_pending,
            generation: 0,
            error: None,
            notifications: yss_ui::notifications::Notifications::new(window),
            task: None,
            _subscriptions: subscriptions,
        }
    }

    fn current(&self, cx: &App) -> bool {
        self.target.graph.upgrade().is_some_and(|graph| {
            let graph = graph.read(cx);
            graph.graph.project == self.target.project
                && graph.graph.projection.graph_path == self.target.path
                && graph.graph.editing.version == self.target.version
        })
    }

    fn can_create(&self, cx: &App) -> bool {
        self.browser.catalog.is_some()
            && !self.loading
            && !self.refresh_pending
            && !self.creating
            && self.language == yss_ui::text::locale()
            && self.current(cx)
            && self
                .target
                .graph
                .upgrade()
                .is_some_and(|graph| graph.read(cx).can_edit())
    }

    pub(in crate::canvas) fn connection_source(
        &self,
        graph: &crate::OpenedGraph,
    ) -> Option<&PortAddress> {
        (self.target.project == graph.project
            && self.target.path == graph.projection.graph_path
            && self.target.version == graph.editing.version)
            .then_some(self.target.source.as_ref())
            .flatten()
    }

    pub(super) fn configuring(&self) -> bool {
        self.configuration.is_some()
    }

    pub(super) fn creation_failed(&mut self, cx: &mut Context<Self>) {
        self.creating = false;
        if let Some(form) = &self.configuration {
            form.update(cx, |form, cx| form.creation_failed(cx));
        }
        cx.notify();
    }

    pub(super) fn creation_succeeded(&mut self, cx: &mut Context<Self>) {
        cx.emit(PaletteEvent::Dismiss);
    }

    pub(super) fn dismiss(&self, cx: &mut Context<Self>) {
        if !self.creating {
            cx.emit(PaletteEvent::Dismiss);
        }
    }

    fn scroll_to_active(&self) {
        if let Some(index) = self.browser.active {
            self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.configuration.is_some()
            || !self.search.focus_handle(cx).is_focused(window)
            || event.keystroke.modifiers != gpui_kit::Modifiers::default()
            || !matches!(
                event.keystroke.key.as_str(),
                "up" | "down" | "enter" | "escape"
            )
        {
            return;
        }
        if self.search.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            return;
        }
        match event.keystroke.key.as_str() {
            "escape" => self.dismiss(cx),
            "enter" => {
                if let Some(index) = self.browser.active {
                    self.choose(index, window, cx);
                }
            }
            key => {
                self.browser.move_selection(key == "down");
                self.scroll_to_active();
                cx.notify();
            }
        }
        cx.stop_propagation();
    }

    fn choose(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_create(cx) {
            return;
        }
        let Some(row) = self.browser.row(index) else {
            return;
        };
        let ActivityRowContent::Item(ActivityItem::Node {
            available: true,
            title,
            creation,
            ..
        }) = &row.content
        else {
            return;
        };
        let descriptor = creation.clone();
        if !self.configure_first {
            self.creating = true;
            self.error = None;
            cx.emit(PaletteEvent::Create {
                descriptor,
                parameters: Default::default(),
                port_counts: Default::default(),
            });
            cx.notify();
            return;
        }
        let target = CreationTarget {
            graph: self.target.graph.clone(),
            project: self.target.project.clone(),
            version: self.target.version,
            descriptor: descriptor.clone(),
            title: title.clone(),
        };
        let form = cx.new(|cx| NodeCreationView::new(self.services.clone(), target, window, cx));
        self.configuration_subscription =
            Some(
                cx.subscribe_in(&form, window, move |view, form, event, window, cx| {
                    if view.configuration.as_ref() != Some(form) {
                        return;
                    }
                    match event {
                        CreationEvent::Back => {
                            view.configuration = None;
                            view.configuration_subscription = None;
                            view.error = None;
                            window.focus(&view.search.focus_handle(cx), cx);
                            cx.emit(PaletteEvent::ConfigurationChanged);
                        }
                        CreationEvent::Create {
                            parameters,
                            port_counts,
                        } => {
                            if !view.can_create(cx) {
                                view.notifications
                                    .error(yss_ui::text::t("native.workbench.creationChanged"), cx);
                                form.update(cx, |form, cx| form.creation_failed(cx));
                                return;
                            }
                            view.creating = true;
                            cx.emit(PaletteEvent::Create {
                                descriptor: descriptor.clone(),
                                parameters: parameters.clone(),
                                port_counts: port_counts.clone(),
                            });
                        }
                    }
                    cx.notify();
                }),
            );
        self.configuration = Some(form);
        self.error = None;
        window.focus(&self.focus, cx);
        cx.emit(PaletteEvent::ConfigurationChanged);
        cx.notify();
    }
}

impl EventEmitter<PaletteEvent> for NodePalette {}
