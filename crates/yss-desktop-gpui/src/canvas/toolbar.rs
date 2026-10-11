//! Compact graph controls live outside the pointer-interaction surface.
use super::{GraphCanvas, commands::*};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    menu::DropdownMenu,
};
use gpui_kit::{Context, IntoElement, Window, div, prelude::*, px};

impl GraphCanvas {
    pub(super) fn render_toolbar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let run_node = self.can_run() && self.selected.len() == 1;
        let running = self.is_running();
        let run_unavailable = self.graph_run_unavailable_reason();
        let clear_run = self.clearable_run().cloned();
        let event_graph = self.graph.projection.graph_path.kind()
            == yss_graph_document::GraphResourceKind::EventGraph;
        let focus = self.focus.clone();
        div()
            .h_8()
            .flex_shrink_0()
            .px_2()
            .flex()
            .items_center()
            .gap_1()
            .bg(cx.theme().table)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("save-graph")
                    .small()
                    .ghost()
                    .icon(IconName::Save)
                    .tooltip(crate::text::t("native.canvas.saveShortcut"))
                    .disabled(!self.can_edit() || !self.dirty())
                    .on_click(
                        cx.listener(|view, _, _, cx| view.submit(GraphCommand::Save, None, cx)),
                    ),
            )
            .child(div().h_4().w(px(1.)).mx_1().bg(cx.theme().border))
            .child(
                Button::new("undo-graph")
                    .small()
                    .ghost()
                    .icon(IconName::Undo2)
                    .tooltip(crate::text::t("native.canvas.undoShortcut"))
                    .disabled(!self.can_edit() || !self.graph.editing.can_undo)
                    .on_click(
                        cx.listener(|view, _, _, cx| view.submit(GraphCommand::Undo, None, cx)),
                    ),
            )
            .child(
                Button::new("redo-graph")
                    .small()
                    .ghost()
                    .icon(IconName::Redo2)
                    .tooltip(crate::text::t("native.canvas.redoShortcut"))
                    .disabled(!self.can_edit() || !self.graph.editing.can_redo)
                    .on_click(
                        cx.listener(|view, _, _, cx| view.submit(GraphCommand::Redo, None, cx)),
                    ),
            )
            .when(event_graph, |bar| bar.child(self.result_search(window, cx)))
            .child(div().flex_1().min_w_0())
            .when_some(self.execution_sync_status(), |view, status| {
                view.child(
                    div()
                        .max_w(px(200.))
                        .truncate()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::text::t(status)),
                )
            })
            .child(
                Button::new("inspect-results")
                    .small()
                    .ghost()
                    .icon(IconName::Table)
                    .tooltip(crate::text::t("panel.assistantToolNames.inspect_result"))
                    .disabled(self.graph.results.outputs.is_empty())
                    .on_click(cx.listener(|view, _, _, cx| view.inspect_results(cx))),
            )
            .child(
                Button::new("graph-options")
                    .small()
                    .ghost()
                    .icon(IconName::Ellipsis)
                    .tooltip(crate::text::t("native.canvas.moreActions"))
                    .dropdown_menu(move |menu, _, _| {
                        menu.action_context(focus.clone())
                            .when(event_graph, |menu| {
                                menu.menu_with_enable(
                                    crate::text::t("contextMenu.node.runNode"),
                                    Box::new(RunCurrentNode),
                                    run_node,
                                )
                                .menu_with_enable(
                                    crate::text::t("contextMenu.node.runTo"),
                                    Box::new(RunToNode),
                                    run_node,
                                )
                                .separator()
                            })
                            .menu(
                                crate::text::t("native.canvas.refreshRun"),
                                Box::new(RefreshRunState),
                            )
                            .menu(
                                crate::text::t("native.canvas.resetView"),
                                Box::new(FrameGraph),
                            )
                    }),
            )
            .child(
                Button::new("clear-run-notice")
                    .small()
                    .ghost()
                    .icon(gpui_kit::assets::IconName::ListX)
                    .tooltip(crate::text::t(if clear_run.is_some() {
                        "canvas.clearExecutionArtifacts"
                    } else {
                        "canvas.clearExecutionArtifactsDisabled"
                    }))
                    .disabled(clear_run.is_none())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if let Some(run) = &clear_run {
                            view.clear_run_notice(run, cx);
                        }
                    })),
            )
            .child(
                Button::new("run-graph")
                    .small()
                    .when(!running, |button| button.primary())
                    .when(running, |button| button.ghost())
                    .icon(if running {
                        IconName::Square
                    } else {
                        IconName::Play
                    })
                    .label(if running {
                        crate::text::t("common.cancel")
                    } else {
                        crate::text::t("bayes.actions.run")
                    })
                    .tooltip(if running {
                        crate::text::t("native.canvas.cancelShortcut")
                    } else if let Some(reason) = run_unavailable {
                        crate::text::t(reason)
                    } else {
                        crate::text::t("native.canvas.runShortcut")
                    })
                    .disabled(!running && run_unavailable.is_some())
                    .on_click(cx.listener(|view, _, _, cx| {
                        if view.is_running() {
                            view.cancel_run(cx);
                        } else {
                            view.run_graph(yss_application::graph::run::RunDemand::Default, cx);
                        }
                    })),
            )
    }
}
