//! Native workbench menus invoke existing resource and DockArea operations.
mod commands;
pub(super) use commands::MenuCommand;

use super::Workbench;
use crate::canvas::GraphCommand;
use crate::{
    canvas::SaveGraph,
    workbench::{OpenProjectDirectory, OpenRecentProject, SaveAllGraphs, ShowSettings},
};
use gpui::{App, Context, Menu, MenuItem, Window};
use gpui_component::dock::{BasePanelView, DockPlacement, panel_handle};
use std::sync::Arc;
use yss_graph_document::GraphResourceKind;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum WorkbenchPanel {
    Project,
    Nodes,
    Details,
    Problems,
    Output,
    Results,
    Logs,
    Plugins,
    Settings,
    Assistant,
}

impl Workbench {
    pub(super) fn save_current(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.closing {
            return;
        }
        if let Some(document) = self.details.read(cx).document() {
            document.update(cx, |document, cx| document.save(window, cx));
        } else if let Some(mind) = self.details.read(cx).mind() {
            mind.update(cx, |mind, cx| mind.save(window, cx));
        } else if let Some(editor) = self.details.read(cx).database() {
            editor.update(cx, |editor, cx| editor.save(window, cx));
        } else if let Some(chart) = self.details.read(cx).chart() {
            chart.update(cx, |chart, cx| chart.save(window, cx));
        } else if let Some(graph) = self.details.read(cx).graph() {
            graph.update(cx, |graph, cx| graph.submit(GraphCommand::Save, None, cx));
        }
    }

    pub(super) fn show_panel(
        &mut self,
        panel: WorkbenchPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match panel {
            WorkbenchPanel::Assistant => {
                self.show_assistant(window, cx);
                return true;
            }
            WorkbenchPanel::Settings => {
                self.show_settings(window, cx);
                return true;
            }
            WorkbenchPanel::Plugins => {
                self.show_plugins(window, cx);
                return true;
            }
            _ => {}
        }
        let Some((panel, placement)) = self.dock_panel(panel) else {
            return false;
        };
        self.present_panel(panel, placement, window, cx);
        true
    }

    pub(super) fn dock_panel(
        &self,
        panel: WorkbenchPanel,
    ) -> Option<(Arc<dyn BasePanelView>, DockPlacement)> {
        Some(match panel {
            WorkbenchPanel::Project | WorkbenchPanel::Nodes => {
                let key = if matches!(panel, WorkbenchPanel::Project) {
                    "project"
                } else {
                    "nodes"
                };
                let panel = self
                    .activities
                    .get(key)
                    .and_then(gpui::WeakEntity::upgrade)?;
                (panel_handle(panel), DockPlacement::Left)
            }
            WorkbenchPanel::Details => (panel_handle(self.details.clone()), DockPlacement::Right),
            WorkbenchPanel::Problems => {
                (panel_handle(self.problems.clone()), DockPlacement::Bottom)
            }
            WorkbenchPanel::Output => (panel_handle(self.output.clone()), DockPlacement::Bottom),
            WorkbenchPanel::Results => (panel_handle(self.results.clone()), DockPlacement::Bottom),
            WorkbenchPanel::Logs => (panel_handle(self.logs.clone()), DockPlacement::Bottom),
            WorkbenchPanel::Assistant | WorkbenchPanel::Plugins | WorkbenchPanel::Settings => {
                return None;
            }
        })
    }

    pub(super) fn panel_is_displayed(&self, panel: WorkbenchPanel, cx: &App) -> bool {
        self.dock_panel(panel).is_some_and(|(panel, _)| {
            self.displayed_panel_placement(panel.panel_id(cx), cx)
                .is_some()
        })
    }

    pub(super) fn toggle_panel(
        &mut self,
        panel: WorkbenchPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) {
            return;
        }
        let displayed = self
            .dock_panel(panel)
            .and_then(|(panel, _)| self.displayed_panel_placement(panel.panel_id(cx), cx));
        if let Some(placement) = displayed.filter(|place| *place != DockPlacement::Center) {
            self.dock
                .update(cx, |dock, cx| dock.toggle_dock(placement, window, cx));
        } else {
            self.show_panel(panel, window, cx);
        }
    }

    pub(super) fn prepare_menus(&mut self, cx: &mut Context<Self>) {
        let context = MenuContext {
            busy: self.is_closing(cx),
            project: self.project.is_some(),
        };
        if self.menu_context == Some(context) {
            return;
        }
        self.menu_context = Some(context);
        let menus = application_menus(context);
        cx.set_menus(application_menus(context));
        gpui_base::GlobalState::global_mut(cx)
            .set_app_menus(menus.into_iter().map(Menu::owned).collect());
        self.menu_bar.update(cx, |bar, cx| bar.reload(cx));
    }
}

// Only caches menu availability, never the DockArea's placement or selected panels.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct MenuContext {
    busy: bool,
    project: bool,
}

fn application_menus(context: MenuContext) -> Vec<Menu> {
    let disabled = context.busy || !context.project;
    let resources = Menu::new("新建资源").items([
        MenuItem::action(
            "事件图",
            MenuCommand::NewGraph(GraphResourceKind::EventGraph),
        )
        .disabled(disabled),
        MenuItem::action(
            "函数图",
            MenuCommand::NewGraph(GraphResourceKind::FunctionGraph),
        )
        .disabled(disabled),
        MenuItem::separator(),
        MenuItem::action("Markdown 文档…", MenuCommand::NewDocument).disabled(disabled),
        MenuItem::action("思维导图…", MenuCommand::NewMind).disabled(disabled),
        MenuItem::action("图表…", MenuCommand::NewChart).disabled(disabled),
    ]);
    let file = Menu::new("文件").items([
        MenuItem::action("打开最近项目…", OpenRecentProject).disabled(context.busy),
        MenuItem::action("新建项目…", MenuCommand::NewProject).disabled(context.busy),
        MenuItem::action("打开项目目录…", OpenProjectDirectory).disabled(context.busy),
        MenuItem::action("项目另存为…", MenuCommand::SaveProjectAs).disabled(disabled),
        MenuItem::action("关闭项目", MenuCommand::CloseProject).disabled(disabled),
        MenuItem::separator(),
        MenuItem::submenu(resources).disabled(disabled),
        MenuItem::action("导入数据…", MenuCommand::ImportData).disabled(disabled),
        MenuItem::separator(),
        MenuItem::action("保存当前文件", SaveGraph).disabled(disabled),
        MenuItem::action("保存所有更改", SaveAllGraphs).disabled(context.busy),
        MenuItem::separator(),
        MenuItem::action("退出", MenuCommand::Exit).disabled(context.busy),
    ]);
    let mut panels = vec![];
    for (title, panel) in [
        ("项目", WorkbenchPanel::Project),
        ("节点目录", WorkbenchPanel::Nodes),
        ("属性", WorkbenchPanel::Details),
        ("问题", WorkbenchPanel::Problems),
        ("输出", WorkbenchPanel::Output),
        ("运行结果", WorkbenchPanel::Results),
        ("日志", WorkbenchPanel::Logs),
        ("插件", WorkbenchPanel::Plugins),
        ("助手", WorkbenchPanel::Assistant),
    ] {
        let requires_project =
            !matches!(panel, WorkbenchPanel::Plugins | WorkbenchPanel::Assistant);
        panels.push(
            MenuItem::action(title, MenuCommand::ShowPanel(panel))
                .disabled(context.busy || (requires_project && !context.project)),
        );
    }
    panels.push(MenuItem::action("设置…", ShowSettings).disabled(context.busy));
    panels.push(MenuItem::separator());
    for (title, placement) in [
        ("切换左侧栏", DockPlacement::Left),
        ("切换右侧栏", DockPlacement::Right),
        ("切换底部面板", DockPlacement::Bottom),
    ] {
        panels.push(MenuItem::action(title, MenuCommand::ToggleDock(placement)).disabled(disabled));
    }
    let view = Menu::new("视图").items(panels);
    let mut menus = vec![];
    if cfg!(target_os = "macos") {
        menus.push(Menu::new("YssBI").items([
            MenuItem::action("设置…", ShowSettings).disabled(context.busy),
            MenuItem::separator(),
            MenuItem::action("退出 YssBI", MenuCommand::Exit).disabled(context.busy),
        ]));
    }
    menus.extend([file, view]);
    menus
}
