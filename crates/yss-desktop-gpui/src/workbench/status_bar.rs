//! Compact panel shortcuts project the root DockArea's current selection and visibility.
use super::{Workbench, menus::WorkbenchPanel};
use crate::{appearance, assets::NativeIcon};
use gpui::{Context, IntoElement, div, prelude::*, px, rgb};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Selectable, Sizable,
    button::{Button, ButtonVariants},
    dock::DockPlacement,
};

impl Workbench {
    pub(super) fn render_status_bar(
        &self,
        message: String,
        dirty: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        div()
            .h(px(28.))
            .flex_shrink_0()
            .px_2()
            .flex()
            .items_center()
            .gap_1()
            .border_t_1()
            .border_color(cx.theme().status_bar_border)
            .bg(cx.theme().status_bar)
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(self.panel_button(
                "status-project",
                IconName::Folder,
                "项目",
                WorkbenchPanel::Project,
                cx,
            ))
            .child(self.panel_button(
                "status-nodes",
                IconName::Frame,
                "节点目录",
                WorkbenchPanel::Nodes,
                cx,
            ))
            .child(self.dock_button(
                "status-left",
                IconName::PanelLeft,
                "切换左侧栏",
                DockPlacement::Left,
                cx,
            ))
            .child(div().w(px(1.)).h_3().mx_2().bg(cx.theme().border))
            .child(div().size(px(5.)).rounded_full().bg(rgb(if dirty {
                appearance::AMBER
            } else {
                appearance::GREEN
            })))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px_1()
                    .truncate()
                    .when(self.error.is_some(), |view| {
                        view.text_color(cx.theme().danger)
                    })
                    .child(message),
            )
            .child(self.panel_button(
                "status-problems",
                IconName::TriangleAlert,
                "问题",
                WorkbenchPanel::Problems,
                cx,
            ))
            .child(self.panel_button(
                "status-output",
                IconName::SquareTerminal,
                "运行输出",
                WorkbenchPanel::Output,
                cx,
            ))
            .child(self.panel_button(
                "status-results",
                NativeIcon::Table,
                "运行结果",
                WorkbenchPanel::Results,
                cx,
            ))
            .child(self.panel_button(
                "status-logs",
                IconName::FileText,
                "日志",
                WorkbenchPanel::Logs,
                cx,
            ))
            .child(self.dock_button(
                "status-bottom",
                IconName::PanelBottom,
                "切换底部工具区",
                DockPlacement::Bottom,
                cx,
            ))
            .child(div().w(px(1.)).h_3().mx_2().bg(cx.theme().border))
            .child(self.panel_button(
                "status-details",
                IconName::Inspector,
                "属性",
                WorkbenchPanel::Details,
                cx,
            ))
            .child(
                Button::new("status-assistant")
                    .xsmall()
                    .compact()
                    .ghost()
                    .icon(Icon::new(NativeIcon::Chat).size_3())
                    .tooltip("助手")
                    .disabled(self.is_closing(cx))
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.show_panel(WorkbenchPanel::Assistant, window, cx);
                    })),
            )
            .child(self.dock_button(
                "status-right",
                IconName::PanelRight,
                "切换右侧栏",
                DockPlacement::Right,
                cx,
            ))
    }

    fn panel_button(
        &self,
        id: &'static str,
        icon: impl Into<Icon>,
        tooltip: &'static str,
        panel: WorkbenchPanel,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .xsmall()
            .compact()
            .ghost()
            .icon(Icon::new(icon).size_3())
            .tooltip(tooltip)
            .selected(self.panel_is_displayed(panel, cx))
            .disabled(self.project.is_none() || self.is_closing(cx))
            .on_click(cx.listener(move |view, _, window, cx| {
                view.toggle_panel(panel, window, cx);
            }))
    }

    fn dock_button(
        &self,
        id: &'static str,
        icon: IconName,
        tooltip: &'static str,
        placement: DockPlacement,
        cx: &mut Context<Self>,
    ) -> Button {
        let dock = self.dock.read(cx);
        Button::new(id)
            .xsmall()
            .compact()
            .ghost()
            .icon(Icon::new(icon).size_3())
            .tooltip(tooltip)
            .selected(dock.is_dock_open(placement))
            .disabled(self.project.is_none() || !dock.has_dock(placement) || self.is_closing(cx))
            .on_click(cx.listener(move |view, _, window, cx| {
                if !view.is_closing(cx) && view.project.is_some() {
                    view.dock.update(cx, |dock, cx| {
                        dock.toggle_dock(placement, window, cx);
                    });
                }
            }))
    }
}
