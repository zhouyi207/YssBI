use crate::appearance;
use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window, div,
    prelude::*,
};
use gpui_component::{
    ActiveTheme, Icon, IconName,
    dock::{BasePanel, Panel, PanelEvent},
};
use std::sync::Arc;
use yss_graph_editor::projection::{EditorDiagnosticSeverity, EditorProjectionModel};

pub struct ProblemsPanel {
    focus: FocusHandle,
    projection: Option<Arc<EditorProjectionModel>>,
}
impl ProblemsPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            projection: None,
        }
    }
    pub fn set_projection(
        &mut self,
        projection: Arc<EditorProjectionModel>,
        cx: &mut Context<Self>,
    ) {
        self.projection = Some(projection);
        cx.notify();
    }
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.projection = None;
        cx.notify();
    }
}
impl Render for ProblemsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("problems")
            .track_focus(&self.focus)
            .size_full()
            .overflow_y_scroll()
            .bg(cx.theme().background)
            .text_sm()
            .children(
                self.projection
                    .iter()
                    .flat_map(|projection| projection.diagnostics.iter())
                    .map(|diagnostic| {
                        let (icon, color) = match diagnostic.severity {
                            EditorDiagnosticSeverity::Error => {
                                (IconName::CircleX, cx.theme().danger)
                            }
                            EditorDiagnosticSeverity::Warning => {
                                (IconName::TriangleAlert, cx.theme().warning)
                            }
                            EditorDiagnosticSeverity::Information => {
                                (IconName::Info, cx.theme().muted_foreground)
                            }
                        };
                        div()
                            .px_3()
                            .py_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(Icon::new(icon).size_3().text_color(color))
                            .child(crate::text::graph_diagnostic(diagnostic))
                    }),
            )
            .when(
                self.projection
                    .as_ref()
                    .is_none_or(|projection| projection.diagnostics.is_empty()),
                |view| {
                    view.child(appearance::empty_state(
                        IconName::CircleCheck,
                        if self.projection.is_some() {
                            "没有图诊断"
                        } else {
                            "图检查"
                        },
                        if self.projection.is_some() {
                            "当前图没有需要处理的诊断"
                        } else {
                            "打开图后查看类型与连接检查结果"
                        },
                        cx,
                    ))
                },
            )
    }
}
impl EventEmitter<PanelEvent> for ProblemsPanel {}
impl Focusable for ProblemsPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for ProblemsPanel {
    fn panel_name(&self) -> &'static str {
        "problems"
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
}

impl Panel for ProblemsPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(Icon::new(IconName::TriangleAlert).size_3())
            .child("问题")
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
