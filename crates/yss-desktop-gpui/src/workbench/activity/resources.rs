//! Project resource presentation reads existing editor state and the accepted node catalog.
mod menu;

use super::*;
use gpui::{AnyElement, Div, Hsla, Stateful, WeakEntity};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    menu::ContextMenuExt,
    tooltip::Tooltip,
};
use yss_node_catalog::ResourceBoundCreateArgs;

pub(super) struct ResourceRows {
    workbench: WeakEntity<super::super::Workbench>,
    pub(super) catalog: Arc<ActivityPanelDocument>,
    nodes: BTreeMap<String, usize>,
}

impl ResourceRows {
    pub(super) fn creation(&self, path: &str) -> Option<(Arc<ActivityPanelDocument>, usize)> {
        Some((self.catalog.clone(), *self.nodes.get(path)?))
    }

    fn status(&self, item: &ActivityItem, cx: &App) -> Option<(Hsla, String)> {
        let workbench = self.workbench.upgrade()?;
        let workbench = workbench.read(cx);
        match item {
            ActivityItem::EventGraph { path, .. } | ActivityItem::FunctionGraph { path, .. } => {
                let graph = workbench.graphs.get(path)?.upgrade()?;
                let count = graph.read(cx).graph.projection.diagnostics.len();
                (count > 0).then(|| {
                    (
                        cx.theme().warning,
                        crate::text::format(
                            "graphDiagnostics.sidebarTooltip",
                            &[("count", count.to_string())],
                        ),
                    )
                })
            }
            ActivityItem::Database { id, .. } => {
                let editor = workbench.databases.get(id)?.upgrade()?;
                editor.read(cx).read_failed().then(|| {
                    (
                        cx.theme().danger,
                        crate::text::translate("sidebar.dataLoadFailed"),
                    )
                })
            }
            _ => None,
        }
    }
}

impl ActivityPanel {
    pub(in crate::workbench) fn set_project_resources(
        &mut self,
        workbench: WeakEntity<super::super::Workbench>,
        catalog: Arc<ActivityPanelDocument>,
        cx: &mut Context<Self>,
    ) {
        if self.panel_id != "project" {
            return;
        }
        let Some(document) = &self.document else {
            return;
        };
        if document.project_instance_id != catalog.project_instance_id
            || document.publication_revision != catalog.publication_revision
        {
            self.resources = None;
            cx.notify();
            return;
        }
        if self
            .resources
            .as_ref()
            .is_some_and(|resources| Arc::ptr_eq(&resources.catalog, &catalog))
        {
            return;
        }
        let nodes = catalog
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                let ActivityRowContent::Item(ActivityItem::Node {
                    available: true,
                    creation:
                        NodeCreation::ResourceBound {
                            node_type_id,
                            resource_path,
                            create_args,
                            ..
                        },
                    ..
                }) = &row.content
                else {
                    return None;
                };
                match (node_type_id.as_str(), create_args) {
                    ("yssbi.project.function.call", ResourceBoundCreateArgs::FunctionGraph)
                    | ("yssbi.dataframe.source.get", ResourceBoundCreateArgs::Database) => {
                        Some((resource_path.as_str().to_owned(), index))
                    }
                    _ => None,
                }
            })
            .collect();
        self.resources = Some(ResourceRows {
            workbench,
            catalog,
            nodes,
        });
        cx.notify();
    }

    pub(super) fn open_resource(
        &self,
        expected: &Arc<ActivityPanelDocument>,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        if !self.accepts(expected) {
            return;
        }
        let event = match &expected.rows[index].content {
            ActivityRowContent::Item(
                ActivityItem::EventGraph { path, .. } | ActivityItem::FunctionGraph { path, .. },
            ) => ActivityEvent::OpenGraph(path.clone()),
            ActivityRowContent::Item(ActivityItem::Chart { path, .. }) => {
                ActivityEvent::OpenChart(path.clone())
            }
            ActivityRowContent::Item(ActivityItem::Mind { path, .. }) => {
                ActivityEvent::OpenMind(path.clone())
            }
            ActivityRowContent::Item(ActivityItem::Doc { path, .. }) => {
                ActivityEvent::OpenDocument(path.clone())
            }
            ActivityRowContent::Item(ActivityItem::Database { id, .. }) => {
                ActivityEvent::OpenDatabase(id.clone())
            }
            _ => return,
        };
        cx.emit(event);
    }

    pub(super) fn render_resource(
        &self,
        index: usize,
        row: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(document) = &self.document else {
            return row.into_any_element();
        };
        let ActivityRowContent::Item(item) = &document.rows[index].content else {
            return row.into_any_element();
        };
        let id = match item {
            ActivityItem::Database { id, .. } => id,
            ActivityItem::EventGraph { path, .. }
            | ActivityItem::FunctionGraph { path, .. }
            | ActivityItem::Chart { path, .. }
            | ActivityItem::Mind { path, .. }
            | ActivityItem::Doc { path, .. } => path,
            _ => return row.into_any_element(),
        };
        let selected = self.active_resource.as_deref() == Some(id);
        let expected = document.clone();
        let expected_button = expected.clone();
        let expected_menu = expected.clone();
        let owner = cx.entity().downgrade();
        let drag = ActivityDrag::new(self, index, cx);
        let needs_catalog = drag.is_none()
            && matches!(
                item,
                ActivityItem::FunctionGraph { .. } | ActivityItem::Database { .. }
            );
        let row = row
            .group("activity-resource")
            .aria_selected(selected)
            .child(label(item, cx))
            .cursor_pointer()
            .when(selected, |row| row.bg(cx.theme().sidebar_accent))
            .hover(|row| row.bg(cx.theme().muted))
            .when_some(
                self.resources
                    .as_ref()
                    .and_then(|resources| resources.status(item, cx)),
                |row, (color, message)| {
                    row.child(
                        div()
                            .id("resource-status")
                            .size(px(6.))
                            .flex_shrink_0()
                            .rounded_full()
                            .bg(color)
                            .tooltip(move |window, cx| {
                                Tooltip::new(message.clone()).build(window, cx)
                            }),
                    )
                },
            )
            .child(
                Button::new("open-resource")
                    .small()
                    .ghost()
                    .size_5()
                    .icon(IconName::ChevronRight)
                    .tooltip(crate::text::translate(
                        if matches!(item, ActivityItem::Database { .. }) {
                            "sidebar.viewInDatabaseEditor"
                        } else {
                            "sidebar.open"
                        },
                    ))
                    .opacity(0.)
                    .group_hover("activity-resource", |style| style.opacity(1.))
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        cx.stop_propagation();
                        view.open_resource(&expected_button, index, cx);
                    })),
            )
            .on_click(cx.listener(move |view, _, _, cx| {
                cx.stop_propagation();
                view.open_resource(&expected, index, cx);
            }))
            .when_some(drag, |row, drag| {
                row.on_drag(drag, |drag, _, _, cx| {
                    cx.stop_propagation();
                    cx.new(|_| drag.clone())
                })
            })
            .when(needs_catalog, |row| {
                row.tooltip(|window, cx| {
                    Tooltip::new(crate::text::translate(
                        "notifications.editor.resourceCatalogRefreshing",
                    ))
                    .build(window, cx)
                })
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|_, _, _, cx| cx.emit(ActivityEvent::RefreshResources)),
                )
            });
        row.context_menu(move |menu, _, _| {
            menu::resource_menu(menu, owner.clone(), expected_menu.clone(), index)
        })
        .into_any_element()
    }
}

pub(super) fn label(item: &ActivityItem, cx: &App) -> impl IntoElement {
    let (icon, color, name) = match item {
        ActivityItem::EventGraph { name, .. } => (IconName::Zap, cx.theme().info, name.as_str()),
        ActivityItem::FunctionGraph { name, .. } => {
            (IconName::Braces, cx.theme().success, name.as_str())
        }
        ActivityItem::Database { name, .. } => {
            (IconName::Database, cx.theme().success, name.as_str())
        }
        ActivityItem::Chart { name, .. } => {
            (IconName::ChartLine, cx.theme().chart_5, name.as_str())
        }
        ActivityItem::Mind { name, .. } => {
            (IconName::Network, cx.theme().foreground, name.as_str())
        }
        ActivityItem::Doc { name, .. } => {
            (IconName::FileText, cx.theme().foreground, name.as_str())
        }
        _ => (IconName::File, cx.theme().foreground, ""),
    };
    div()
        .flex()
        .flex_1()
        .min_w_0()
        .items_center()
        .gap_1p5()
        .child(Icon::new(icon).size_3().flex_shrink_0().text_color(color))
        .child(div().flex_1().min_w_0().truncate().child(name.to_owned()))
}

impl super::super::Workbench {
    pub(in crate::workbench) fn refresh_resource_rows(&self, cx: &mut Context<Self>) {
        if let Some(panel) = self.activities.get("project").and_then(WeakEntity::upgrade) {
            panel.update(cx, |_, cx| cx.notify());
        }
    }
}
