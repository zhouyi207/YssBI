//! Native DockArea persistence and resource rehydration. The DockArea owns the live topology.
pub(super) mod columns;
mod missing;
mod replacement;

use super::Workbench;
use gpui_kit::component::dock::{
    BasePanelView, DockAreaState, DockEvent, DockLayout, DockPlacement, PaneRef, Panel, PanelId,
    PanelInfo, PanelState, panel_handle, register_panel,
};
use gpui_kit::{App, AppContext, Context, WeakEntity, Window};
use missing::MissingPanel;
use std::{collections::BTreeSet, sync::Arc, time::Duration};
use yss_graph_document::GraphResourcePath;
use yss_ui::lifecycle::LogReleased;

#[derive(Default)]
pub(super) struct ResponsiveLayout {
    narrow: bool,
    // Only visibility is temporary. Native DockArea retains topology and sizes.
    restore_open: [bool; 2],
}

impl ResponsiveLayout {
    fn target(&self, width: gpui_kit::Pixels, rem: gpui_kit::Pixels) -> bool {
        width < rem * if self.narrow { 78. } else { 72. }
    }

    fn preserve_preferences(&self, state: &mut DockAreaState) {
        for (restore, dock) in self
            .restore_open
            .iter()
            .zip([&mut state.left_dock, &mut state.right_dock])
        {
            if *restore && let Some(dock) = dock {
                *dock = gpui_kit::component::dock::DockState::new(
                    dock.panel().clone(),
                    dock.placement(),
                    dock.size(),
                    true,
                );
            }
        }
    }
}

pub(super) fn fit_window(
    owner: &WeakEntity<Workbench>,
    width: gpui_kit::Pixels,
    window: &mut Window,
    cx: &mut App,
) {
    let rem = window.rem_size();
    let Some(view) = owner.upgrade() else { return };
    let state = view.read(cx);
    let narrow = state.responsive_layout.target(width, rem);
    if state.restoring_layout
        || state.project.is_none()
        || state.dock.read(cx).is_zoomed()
        || state.dock.read(cx).is_locked()
        || narrow == state.responsive_layout.narrow
    {
        return;
    }
    let owner = owner.clone();
    window.defer(cx, move |window, cx| {
        owner
            .update(cx, |view, cx| view.adapt_window(width, window, cx))
            .log_released();
    });
}

impl Workbench {
    fn adapt_window(
        &mut self,
        width: gpui_kit::Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.restoring_layout
            || self.project.is_none()
            || self.dock.read(cx).is_zoomed()
            || self.dock.read(cx).is_locked()
        {
            return;
        }
        let narrow = self.responsive_layout.target(width, window.rem_size());
        if narrow == self.responsive_layout.narrow {
            return;
        }
        self.responsive_layout.narrow = narrow;
        let editor_placement = self
            .active_editor
            .and_then(|id| self.panel_placement(id, cx));
        for (index, placement) in [DockPlacement::Left, DockPlacement::Right]
            .into_iter()
            .enumerate()
        {
            if narrow && editor_placement == Some(placement) {
                continue;
            }
            let open = self.dock.read(cx).is_dock_open(placement);
            let toggle = if narrow {
                self.responsive_layout.restore_open[index] = open;
                open
            } else {
                let restore = std::mem::take(&mut self.responsive_layout.restore_open[index]);
                restore && !open
            };
            if toggle {
                self.dock
                    .update(cx, |dock, cx| dock.toggle_dock(placement, window, cx));
            }
        }
        cx.notify();
    }

    pub(in crate::workbench) fn active_editor_panel(
        &self,
        cx: &App,
    ) -> Option<Arc<dyn BasePanelView>> {
        let id = self.active_editor?;
        self.displayed_panel_placement(id, cx)?;
        self.dock.read(cx).panel(id).cloned()
    }

    pub(super) fn active_graph(
        &self,
        cx: &App,
    ) -> Option<gpui_kit::Entity<crate::canvas::GraphCanvas>> {
        self.active_editor_panel(cx)?.view().downcast().ok()
    }

    pub(super) fn panel_placement(&self, id: PanelId, cx: &App) -> Option<DockPlacement> {
        let dock = self.dock.read(cx);
        [
            DockPlacement::Center,
            DockPlacement::Left,
            DockPlacement::Right,
            DockPlacement::Bottom,
        ]
        .into_iter()
        .find(|placement| {
            dock.layout(*placement)
                .is_some_and(|tree| tree.contains_panel(id))
        })
    }

    pub(super) fn displayed_panel_placement(&self, id: PanelId, cx: &App) -> Option<DockPlacement> {
        let placement = self.panel_placement(id, cx)?;
        let dock = self.dock.read(cx);
        if placement != DockPlacement::Center && !dock.is_dock_open(placement) {
            return None;
        }
        let tree = dock.layout(placement)?;
        let node = tree.find_node(tree.find_panel_node(id)?)?;
        if dock
            .zoomed_group()
            .is_some_and(|zoomed| zoomed != node.id())
            && !(placement == DockPlacement::Left && columns::conversation_zoomed(dock))
        {
            return None;
        }
        match node.kind() {
            PaneRef::Tabs { panels, active_ix } if panels.get(active_ix) == Some(&id) => {
                Some(placement)
            }
            _ => None,
        }
    }

    pub(super) fn present_panel(
        &self,
        panel: Arc<dyn BasePanelView>,
        fallback: DockPlacement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = panel.panel_id(cx);
        let placement = self.panel_placement(id, cx);
        self.dock.update(cx, |dock, cx| {
            if columns::is_conversation(&panel) {
                if !columns::conversation_zoomed(dock) {
                    dock.set_zoomed_out(window, cx);
                }
                columns::present(dock, panel.clone(), window, cx);
                window.focus(&panel.focus_handle(cx), cx);
                return;
            }
            let group = placement.and_then(|placement| dock.layout(placement)?.find_panel_node(id));
            let in_zoomed_group = group.is_some_and(|node| dock.zoomed_group() == Some(node));
            let retained_sidebar = placement == Some(DockPlacement::Left)
                && columns::conversation_zoomed(dock)
                && super::sidebar::is_navigation(&panel);
            if dock.is_zoomed() && !in_zoomed_group && !retained_sidebar {
                dock.set_zoomed_out(window, cx);
            }
            if placement.is_some() {
                dock.select_panel(id, window, cx);
            } else if fallback == DockPlacement::Center {
                columns::present(dock, panel, window, cx);
            } else {
                dock.add_panel_view(panel, fallback, None, window, cx);
            }
            let placement = placement.unwrap_or(fallback);
            if placement != DockPlacement::Center && !dock.is_dock_open(placement) {
                dock.toggle_dock(placement, window, cx);
            }
        });
    }

    pub(super) fn install_default_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.responsive_layout = ResponsiveLayout::default();
        self.active_editor = None;
        let defaults = &crate::preferences::current(cx).workspace;
        let show_details = defaults.show_details;
        let show_bottom = defaults.show_bottom_panel;
        let right = DockLayout::tabs().panel_view(panel_handle(self.details.clone()), cx);
        let bottom = DockLayout::tabs()
            .panel_view(panel_handle(self.problems.clone()), cx)
            .panel_view(panel_handle(self.output.clone()), cx)
            .panel_view(panel_handle(self.results.clone()), cx)
            .panel_view(panel_handle(self.logs.clone()), cx);
        self.dock.update(cx, |dock, cx| {
            dock.set_center(DockLayout::tabs(), window, cx);
            dock.remove_dock(DockPlacement::Left, window, cx);
            dock.set_dock(DockPlacement::Right, right, window, cx);
            dock.set_dock_size(DockPlacement::Right, gpui_kit::px(300.), window, cx);
            dock.set_dock(DockPlacement::Bottom, bottom, window, cx);
            dock.set_dock_size(DockPlacement::Bottom, gpui_kit::px(220.), window, cx);
            if dock.is_dock_open(DockPlacement::Right) != show_details {
                dock.toggle_dock(DockPlacement::Right, window, cx);
            }
            if dock.is_dock_open(DockPlacement::Bottom) != show_bottom {
                dock.toggle_dock(DockPlacement::Bottom, window, cx);
            }
        });
    }

    pub(super) fn connect_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let subscription = cx.subscribe_in(&self.dock, window, |view, _, event, window, cx| {
            if !matches!(event, DockEvent::LayoutChanged) {
                return;
            }
            cx.notify();
            if view.restoring_layout {
                return;
            }
            for (index, placement) in [DockPlacement::Left, DockPlacement::Right]
                .into_iter()
                .enumerate()
            {
                if view.dock.read(cx).is_dock_open(placement) {
                    // A user or explicit show-panel command reopened it while narrow.
                    view.responsive_layout.restore_open[index] = false;
                }
            }
            let lifecycle = view.lifecycle;
            // Closing the last editor first inserts its watermark, then emits a
            // native close event. Reconcile after both, so the insertion event
            // cannot remove the watermark before the editor has actually closed.
            cx.defer_in(window, move |view, window, cx| {
                if view.restoring_layout || view.lifecycle != lifecycle {
                    return;
                }
                view.dock.update(cx, |dock, cx| {
                    columns::maintain_editor_space(dock, window, cx)
                });
                view.sync_active_conversation(cx);
            });
            let timer = cx.background_executor().timer(Duration::from_millis(300));
            view.layout_task = Some(cx.spawn(async move |view, cx| {
                timer.await;
                view.update(cx, |view, cx| view.persist_layout(cx))
                    .log_released();
            }));
        });
        self.layout_subscription = Some(subscription);
    }

    pub(super) fn persist_layout(&self, cx: &mut Context<Self>) {
        if self.restoring_layout {
            return;
        }
        let Some(root) = self.layout_root.clone() else {
            return;
        };
        let mut snapshot = self.dock.read(cx).dump(cx);
        self.responsive_layout.preserve_preferences(&mut snapshot);
        let job = self.services.layouts.save(root.clone(), snapshot);
        cx.spawn(async move |view, cx| {
            let failed = !matches!(job.await, Ok(Ok(())));
            if failed {
                view.update(cx, |view, cx| {
                    if view.layout_root.as_ref() == Some(&root) {
                        view.error =
                            Some(crate::text::t("native.workbench.layoutSaveFailed").into());
                        cx.notify();
                    }
                })
                .log_released();
            }
        })
        .detach();
    }

    pub(super) fn restore_layout(
        &mut self,
        initial_resource: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = &self.project else {
            return;
        };
        let database_entries = project.index.databases.clone();
        let identity = project.identity.clone();
        let expected = identity.clone();
        let lifecycle = self.lifecycle;
        let restore_enabled = crate::preferences::current(cx).workspace.restore_layout;
        let page_size = crate::preferences::current(cx).tables.page_size;
        self.busy = true;
        self.restoring_layout = true;
        self.responsive_layout = ResponsiveLayout::default();
        self.layout_root = None;
        let owner = self.services.clone();
        let job = self.services.run(move |services| {
            let root = services
                .application
                .query_project_path(identity.clone())?
                .ok_or_else(|| anyhow::anyhow!("project root unavailable"))?;
            let (layout, layout_failed) = if restore_enabled {
                match owner.layouts.read(&root) {
                    Ok(layout) => (layout, false),
                    Err(_) => {
                        tracing::warn!(
                            code = "native_layout_load_failed",
                            "Native layout could not be read"
                        );
                        (None, true)
                    }
                }
            } else {
                (None, false)
            };
            let viewport_failed = restore_enabled && owner.layouts.load_viewports(&root).is_err();
            if viewport_failed {
                tracing::warn!(
                    code = "native_viewport_load_failed",
                    "Graph view checkpoints could not be read"
                );
            }
            let mut documents = vec![];
            for path in layout
                .as_ref()
                .map(|state| resource_paths(state, "document-editor", "documentPath"))
                .unwrap_or_default()
            {
                if let Ok(path) = yss_project_model::doc::DocPath::parse(&path)
                    && let Ok(snapshot) = services.application.read_doc(identity.clone(), path)
                {
                    documents.push(snapshot);
                }
            }
            let mut minds = vec![];
            for path in layout
                .as_ref()
                .map(|state| resource_paths(state, "mind-editor", "mindPath"))
                .unwrap_or_default()
            {
                if let Ok(path) = yss_project_model::mind::MindPath::parse(&path)
                    && let Ok(snapshot) = services.application.read_mind(identity.clone(), path)
                {
                    minds.push(snapshot);
                }
            }
            let mut databases = vec![];
            for id in layout
                .as_ref()
                .map(|state| resource_paths(state, "database-editor", "databaseId"))
                .unwrap_or_default()
            {
                if let Some(entry) = database_entries.iter().find(|entry| entry.id == id)
                    && let Ok(read) = crate::databases::query::read(
                        &services.application,
                        identity.clone(),
                        entry.id.clone(),
                        entry.revision,
                        0,
                        page_size,
                        None,
                    )
                {
                    databases.push((entry.clone(), read));
                }
            }
            let mut charts = vec![];
            for path in layout
                .as_ref()
                .map(|state| resource_paths(state, "chart-editor", "chartPath"))
                .unwrap_or_default()
            {
                if let Ok(path) = yss_chart_document::ChartResourcePath::parse(&path)
                    && let Ok(read) =
                        crate::charts::read(&services.application, identity.clone(), path)
                {
                    charts.push(read);
                }
            }
            let mut conversations = vec![];
            for id in layout
                .as_ref()
                .map(|state| resource_paths(state, "assistant-conversation", "sessionId"))
                .unwrap_or_default()
            {
                if let Ok(id) = yss_harness_contract::HarnessSessionId::try_new(id)
                    && let Ok(session) =
                        owner
                            .executor
                            .block_on(services.application.open_harness_session(
                                &services.harness.host,
                                &crate::assistant::principal(),
                                &id,
                            ))
                {
                    conversations.push(session);
                }
            }
            let assistant =
                owner
                    .executor
                    .block_on(services.application.assistant_activity_panel(
                        &services.harness.host,
                        &crate::assistant::principal(),
                        Some(identity.clone()),
                    ))?;
            services.application.query_project_path(identity)?;
            Ok((
                root,
                layout_failed,
                viewport_failed,
                layout,
                documents,
                minds,
                databases,
                charts,
                conversations,
                assistant,
            ))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|project| project.identity != expected)
                {
                    return;
                }
                view.busy = false;
                match result {
                    Ok((
                        root,
                        layout_failed,
                        viewport_failed,
                        layout,
                        documents,
                        minds,
                        databases,
                        charts,
                        conversations,
                        assistant,
                    )) => {
                        // A clean session must not erase the saved topology or seed
                        // canvases from same-process retained viewport checkpoints.
                        view.layout_root = restore_enabled.then_some(root);
                        if layout_failed {
                            view.error = Some(crate::text::translate(
                                "native.workbench.layoutRestoreFailed",
                            ));
                        }
                        if viewport_failed {
                            view.error =
                                Some(crate::text::translate("native.canvas.viewportLoadFailed"));
                        }
                        let openings = layout
                            .as_ref()
                            .map(|layout| resource_paths(layout, "graph-editor", "graphPath"))
                            .unwrap_or_default()
                            .into_iter()
                            .filter_map(|path| GraphResourcePath::new(path).ok())
                            .map(|path| view.create_graph_opening(path, window, cx))
                            .collect::<Vec<_>>();
                        for document in documents {
                            view.install_document(document, window, cx);
                        }
                        for mind in minds {
                            view.install_mind(mind, window, cx);
                        }
                        for (entry, read) in databases {
                            view.install_database(entry, Some(read), window, cx);
                        }
                        for chart in charts {
                            view.install_chart(chart, window, cx);
                        }
                        view.install_assistant_document(assistant, window, cx);
                        for session in conversations {
                            view.assistant.update(cx, |assistant, cx| {
                                assistant.retain_conversation(session, window, cx)
                            });
                        }
                        view.register_layout_panels(cx);
                        let restored = if let Some(layout) = layout {
                            let loaded = view
                                .dock
                                .update(cx, |dock, cx| dock.load(layout, window, cx));
                            if loaded.is_err() {
                                view.install_default_layout(window, cx);
                                view.install_activity(window, cx);
                                view.error = Some(crate::text::translate(
                                    "native.workbench.layoutRestoreFailed",
                                ));
                            }
                            loaded.is_ok()
                        } else {
                            false
                        };
                        // Restored conversation groups are user topology too; do not
                        // consolidate them into the default column on every restart.
                        view.dock.update(cx, |dock, cx| {
                            columns::maintain_editor_space(dock, window, cx)
                        });
                        view.restoring_layout = false;
                        drop(openings);
                        let path = initial_resource.or_else(|| {
                            if restored {
                                None
                            } else {
                                view.project
                                    .as_ref()
                                    .and_then(crate::project::DesktopProject::first_graph)
                            }
                        });
                        if let Some(path) = path {
                            if view.project.as_ref().is_some_and(|project| {
                                project
                                    .index
                                    .docs
                                    .iter()
                                    .any(|document| document.path.as_str() == path)
                            }) {
                                view.open_document(path, None, window, cx);
                            } else if view.project.as_ref().is_some_and(|project| {
                                project
                                    .index
                                    .minds
                                    .iter()
                                    .any(|mind| mind.path.as_str() == path)
                            }) {
                                view.open_mind(path, None, window, cx);
                            } else if view.project.as_ref().is_some_and(|project| {
                                project.index.databases.iter().any(|entry| entry.id == path)
                            }) {
                                view.open_database(path, None, window, cx);
                            } else if yss_chart_document::ChartResourcePath::parse(&path).is_ok() {
                                view.open_chart(path, None, window, cx);
                            } else {
                                view.open_graph(path, window, cx);
                            }
                        }
                    }
                    Err(_) => {
                        view.restoring_layout = false;
                        view.error =
                            Some(crate::text::t("native.workbench.layoutRestoreFailed").into());
                    }
                }
                cx.notify();
            })
            .log_released();
        })
        .detach();
        cx.notify();
    }

    fn register_layout_panels(&self, cx: &mut Context<Self>) {
        register_panel(cx, "empty-editor", |_, _, cx| {
            panel_handle(cx.new(super::dock::EmptyEditor::new))
        });
        register_fixed("details", self.details.downgrade(), cx);
        register_fixed("problems", self.problems.downgrade(), cx);
        register_fixed("output", self.output.downgrade(), cx);
        register_fixed("results", self.results.downgrade(), cx);
        register_fixed("logs", self.logs.downgrade(), cx);
        register_fixed("plugins", self.plugins.downgrade(), cx);
        register_fixed("plugins-directory", self.plugins_sidebar.downgrade(), cx);
        let services = self.services.plugins.clone();
        let plugins = self.plugins.clone();
        register_panel(cx, "plugin-view", move |context, window, cx| {
            if let PanelInfo::Panel(info) = context.info()
                && let Some(plugin_id) = info.get("pluginId").and_then(serde_json::Value::as_str)
                && let Some(view_id) = info.get("viewId").and_then(serde_json::Value::as_str)
            {
                let panel = cx.new(|cx| {
                    crate::plugins::PluginViewPanel::new(
                        services.clone(),
                        plugin_id.into(),
                        view_id.into(),
                        None,
                        window,
                        cx,
                    )
                });
                let plugins = plugins.clone();
                cx.subscribe(
                    &panel,
                    move |_, event: &crate::plugins::OpenNativeView, cx| {
                        plugins.update(cx, |_, cx| {
                            cx.emit(event.clone());
                        });
                    },
                )
                .detach();
                return panel_handle(panel);
            }
            panel_handle(cx.new(|cx| MissingPanel::new(context.state().clone(), cx)))
        });
        let assistant = self.assistant.downgrade();
        register_panel(cx, "assistant-conversation", move |context, _, cx| {
            if let PanelInfo::Panel(info) = context.info()
                && let Some(id) = info.get("sessionId").and_then(serde_json::Value::as_str)
                && let Some(assistant) = assistant.upgrade()
                && let Some(panel) = assistant.read(cx).conversation(id).cloned()
            {
                return panel_handle(panel);
            }
            panel_handle(cx.new(|cx| MissingPanel::new(context.state().clone(), cx)))
        });
        for (name, panel) in &self.activities {
            register_fixed(name, panel.clone(), cx);
        }
        let graphs = self.graphs.clone();
        let openings = self.graph_openings.clone();
        register_panel(cx, "graph-editor", move |context, _, cx| {
            if let PanelInfo::Panel(info) = context.info()
                && let Some(path) = info.get("graphPath").and_then(serde_json::Value::as_str)
            {
                if let Some(graph) = graphs.get(path).and_then(WeakEntity::upgrade) {
                    return panel_handle(graph);
                }
                if let Some(opening) = openings.get(path).and_then(WeakEntity::upgrade) {
                    return panel_handle(opening);
                }
            }
            panel_handle(cx.new(|cx| MissingPanel::new(context.state().clone(), cx)))
        });
        let documents = self.documents.clone();
        register_panel(cx, "document-editor", move |context, _, cx| {
            if let PanelInfo::Panel(info) = context.info()
                && let Some(path) = info.get("documentPath").and_then(serde_json::Value::as_str)
                && let Some(document) = documents.get(path).and_then(WeakEntity::upgrade)
            {
                return panel_handle(document);
            }
            panel_handle(cx.new(|cx| MissingPanel::new(context.state().clone(), cx)))
        });
        let minds = self.minds.clone();
        register_panel(cx, "mind-editor", move |context, _, cx| {
            if let PanelInfo::Panel(info) = context.info()
                && let Some(path) = info.get("mindPath").and_then(serde_json::Value::as_str)
                && let Some(mind) = minds.get(path).and_then(WeakEntity::upgrade)
            {
                return panel_handle(mind);
            }
            panel_handle(cx.new(|cx| MissingPanel::new(context.state().clone(), cx)))
        });
        let databases = self.databases.clone();
        register_panel(cx, "database-editor", move |context, _, cx| {
            if let PanelInfo::Panel(info) = context.info()
                && let Some(id) = info.get("databaseId").and_then(serde_json::Value::as_str)
                && let Some(editor) = databases.get(id).and_then(WeakEntity::upgrade)
            {
                return panel_handle(editor);
            }
            panel_handle(cx.new(|cx| MissingPanel::new(context.state().clone(), cx)))
        });
        let charts = self.charts.clone();
        register_panel(cx, "chart-editor", move |context, _, cx| {
            if let PanelInfo::Panel(info) = context.info()
                && let Some(path) = info.get("chartPath").and_then(serde_json::Value::as_str)
                && let Some(chart) = charts.get(path).and_then(WeakEntity::upgrade)
            {
                return panel_handle(chart);
            }
            panel_handle(cx.new(|cx| MissingPanel::new(context.state().clone(), cx)))
        });
    }
}

fn register_fixed<P: Panel>(name: &str, panel: WeakEntity<P>, cx: &mut App) {
    register_panel(cx, name, move |context, _, cx| {
        if let Some(panel) = panel.upgrade() {
            panel_handle(panel)
        } else {
            panel_handle(cx.new(|cx| MissingPanel::new(context.state().clone(), cx)))
        }
    });
}

fn resource_paths(state: &DockAreaState, panel_name: &str, path_key: &str) -> BTreeSet<String> {
    fn collect(state: &PanelState, panel_name: &str, path_key: &str, paths: &mut BTreeSet<String>) {
        if state.panel_name == panel_name
            && let PanelInfo::Panel(info) = &state.info
            && let Some(path) = info.get(path_key).and_then(serde_json::Value::as_str)
        {
            paths.insert(path.into());
        }
        for child in &state.children {
            collect(child, panel_name, path_key, paths);
        }
    }
    let mut paths = BTreeSet::new();
    collect(&state.center, panel_name, path_key, &mut paths);
    for dock in [
        state.left_dock.as_ref(),
        state.right_dock.as_ref(),
        state.bottom_dock.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        collect(dock.panel(), panel_name, path_key, &mut paths);
    }
    paths
}

#[cfg(test)]
mod responsive_tests {
    use super::*;
    use gpui_kit::component::dock::DockState;
    use gpui_kit::px;

    #[test]
    fn narrow_policy_scales_with_font_and_has_restore_hysteresis() {
        let mut policy = ResponsiveLayout::default();
        assert!(policy.target(px(900.), px(14.)));
        assert!(!policy.target(px(1100.), px(14.)));
        assert!(policy.target(px(1100.), px(18.)));
        policy.narrow = true;
        assert!(policy.target(px(1050.), px(14.)));
        assert!(!policy.target(px(1100.), px(14.)));
    }

    #[test]
    fn temporary_collapse_never_changes_persisted_topology_sizes_or_preference() {
        let left = DockState::new(
            PanelState::new("project"),
            DockPlacement::Left,
            px(240.),
            true,
        );
        let right = DockState::new(
            PanelState::new("details"),
            DockPlacement::Right,
            px(300.),
            true,
        );
        let preferred = DockAreaState {
            left_dock: Some(left.clone()),
            right_dock: Some(right.clone()),
            ..Default::default()
        };
        let mut visible = preferred.clone();
        visible.left_dock = Some(DockState::new(
            left.panel().clone(),
            left.placement(),
            left.size(),
            false,
        ));
        visible.right_dock = Some(DockState::new(
            right.panel().clone(),
            right.placement(),
            right.size(),
            false,
        ));
        let policy = ResponsiveLayout {
            narrow: true,
            restore_open: [true, true],
        };
        policy.preserve_preferences(&mut visible);
        assert_eq!(visible, preferred);
    }

    #[test]
    fn user_visibility_override_is_persisted_instead_of_previous_preference() {
        let mut state = DockAreaState {
            right_dock: Some(DockState::new(
                PanelState::new("details"),
                DockPlacement::Right,
                px(350.),
                false,
            )),
            ..Default::default()
        };
        let expected = state.clone();
        let policy = ResponsiveLayout {
            narrow: true,
            restore_open: [true, false],
        };
        policy.preserve_preferences(&mut state);
        assert_eq!(state, expected);
    }
}
