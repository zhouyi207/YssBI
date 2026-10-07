//! Native DockArea persistence and resource rehydration. The DockArea owns the live topology.
mod missing;

use super::Workbench;
use crate::project::OpenedGraph;
use gpui::{App, AppContext, Context, WeakEntity, Window};
use gpui_component::dock::{
    DockAreaState, DockEvent, DockLayout, DockPlacement, Panel, PanelInfo, PanelState,
    panel_handle, register_panel,
};
use missing::MissingPanel;
use std::{collections::BTreeSet, time::Duration};
use yss_application::graph::open::OpenGraphRequest;
use yss_graph_document::GraphResourcePath;

impl Workbench {
    pub(super) fn install_default_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
            dock.set_dock_size(DockPlacement::Right, gpui::px(310.), window, cx);
            dock.set_dock(DockPlacement::Bottom, bottom, window, cx);
            dock.set_dock_size(DockPlacement::Bottom, gpui::px(170.), window, cx);
            for placement in [DockPlacement::Right, DockPlacement::Bottom] {
                if !dock.is_dock_open(placement) {
                    dock.toggle_dock(placement, window, cx);
                }
            }
        });
    }

    pub(super) fn connect_layout(&mut self, cx: &mut Context<Self>) {
        self.layout_subscription = Some(cx.subscribe(&self.dock, |view, _, event, cx| {
            if matches!(event, DockEvent::LayoutChanged) && !view.restoring_layout {
                let timer = cx.background_executor().timer(Duration::from_millis(300));
                view.layout_task = Some(cx.spawn(async move |view, cx| {
                    timer.await;
                    let _ = view.update(cx, |view, cx| view.persist_layout(cx));
                }));
            }
        }));
    }

    pub(super) fn persist_layout(&self, cx: &mut Context<Self>) {
        if self.restoring_layout {
            return;
        }
        let Some(root) = self.layout_root.clone() else {
            return;
        };
        let snapshot = self.dock.read(cx).dump(cx);
        let job = self
            .services
            .layouts
            .save(root.clone(), snapshot, &self.services.executor);
        cx.spawn(async move |view, cx| {
            let failed = !matches!(job.await, Ok(Ok(())));
            if failed {
                let _ = view.update(cx, |view, cx| {
                    if view.layout_root.as_ref() == Some(&root) {
                        view.error = Some("工作台布局未能保存，请检查应用数据目录。".into());
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    pub(super) fn restore_layout(
        &mut self,
        initial_graph: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = &self.project else {
            return;
        };
        let identity = project.identity.clone();
        let expected = identity.clone();
        let lifecycle = self.lifecycle;
        self.busy = true;
        self.restoring_layout = true;
        self.layout_root = None;
        let owner = self.services.clone();
        let job = self.services.run(move |services| {
            let root = services
                .application
                .query_project_path(identity.clone())?
                .ok_or_else(|| anyhow::anyhow!("project root unavailable"))?;
            let layout = match owner.layouts.read(&root) {
                Ok(layout) => layout,
                Err(_) => {
                    tracing::warn!(
                        code = "native_layout_load_failed",
                        "Native layout could not be read"
                    );
                    None
                }
            };
            let paths = layout.as_ref().map(graph_paths).unwrap_or_default();
            let mut graphs = vec![];
            for path in paths {
                let Ok(path) = GraphResourcePath::new(path) else {
                    continue;
                };
                match services.application.open_graph(OpenGraphRequest::new(
                    identity.clone(),
                    path,
                    lifecycle,
                    "zh-CN",
                )) {
                    Ok(receipt) => graphs.push(OpenedGraph::from_open(receipt)),
                    Err(_) => tracing::debug!(
                        code = "native_layout_resource_unavailable",
                        "Restored graph is unavailable"
                    ),
                }
            }
            services.application.query_project_path(identity)?;
            Ok((root, layout, graphs))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
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
                    Ok((root, layout, graphs)) => {
                        view.layout_root = Some(root);
                        for graph in graphs {
                            view.install_graph(graph, window, cx);
                        }
                        view.register_layout_panels(cx);
                        let restored = layout.is_some();
                        if let Some(layout) = layout {
                            view.dock.update(cx, |dock, cx| {
                                if dock.load(layout, window, cx).is_err() {
                                    tracing::warn!(
                                        code = "native_layout_install_failed",
                                        "Native layout could not be installed"
                                    );
                                }
                                cx.notify();
                            });
                        }
                        view.restoring_layout = false;
                        let path = initial_graph.or_else(|| {
                            if restored {
                                None
                            } else {
                                view.project
                                    .as_ref()
                                    .and_then(crate::project::DesktopProject::first_graph)
                            }
                        });
                        if let Some(path) = path {
                            view.open_graph(path, window, cx);
                        }
                    }
                    Err(_) => {
                        view.restoring_layout = false;
                        view.error = Some("工作台恢复失败，可以从项目目录打开资源。".into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn register_layout_panels(&self, cx: &mut Context<Self>) {
        register_fixed("details", self.details.downgrade(), cx);
        register_fixed("problems", self.problems.downgrade(), cx);
        register_fixed("output", self.output.downgrade(), cx);
        register_fixed("results", self.results.downgrade(), cx);
        register_fixed("logs", self.logs.downgrade(), cx);
        for (name, panel) in &self.activities {
            register_fixed(name, panel.clone(), cx);
        }
        let graphs = self.graphs.clone();
        register_panel(cx, "graph-editor", move |context, _, cx| {
            if let PanelInfo::Panel(info) = context.info()
                && let Some(path) = info.get("graphPath").and_then(serde_json::Value::as_str)
                && let Some(graph) = graphs.get(path).and_then(WeakEntity::upgrade)
            {
                return panel_handle(graph);
            }
            panel_handle(cx.new(|cx| MissingPanel::new(context.state().clone(), cx)))
        });
        register_panel(cx, "result", |context, _, cx| {
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

fn graph_paths(state: &DockAreaState) -> BTreeSet<String> {
    fn collect(state: &PanelState, paths: &mut BTreeSet<String>) {
        if state.panel_name == "graph-editor"
            && let PanelInfo::Panel(info) = &state.info
            && let Some(path) = info.get("graphPath").and_then(serde_json::Value::as_str)
        {
            paths.insert(path.into());
        }
        for child in &state.children {
            collect(child, paths);
        }
    }
    let mut paths = BTreeSet::new();
    collect(&state.center, &mut paths);
    for dock in [
        state.left_dock.as_ref(),
        state.right_dock.as_ref(),
        state.bottom_dock.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        collect(dock.panel(), &mut paths);
    }
    paths
}
