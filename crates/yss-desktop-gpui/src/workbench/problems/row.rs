use super::{ProblemsPanel, location};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon,
    button::{Button, ButtonVariants},
};
use gpui_kit::{AnyElement, Context, IntoElement, div, prelude::*};
use std::sync::Arc;
use yss_graph_editor::projection::{EditorDiagnosticSeverity, EditorProjectionModel};

pub(super) fn render(
    projection: &Arc<EditorProjectionModel>,
    index: usize,
    cx: &mut Context<ProblemsPanel>,
) -> AnyElement {
    let Some(diagnostic) = projection.diagnostics.get(index) else {
        return div().into_any_element();
    };
    let (icon, color) = match diagnostic.severity {
        EditorDiagnosticSeverity::Error => (IconName::CircleX, cx.theme().danger),
        EditorDiagnosticSeverity::Warning => (IconName::TriangleAlert, cx.theme().warning),
        EditorDiagnosticSeverity::Information => (IconName::Info, cx.theme().muted_foreground),
    };
    let label = location::label(&diagnostic.location, projection, None)
        .unwrap_or_else(|| projection.graph_path.to_string());
    let message = crate::text::graph_diagnostic(diagnostic);
    let source = projection.clone();
    div()
        .id(("problem-row", index))
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .border_b_1()
        .border_color(cx.theme().border.opacity(0.5))
        .child(
            Button::new(("problem", index))
                .ghost()
                .rounded_none()
                .w_full()
                .h_auto()
                .min_h_0()
                .px_3()
                .py_2()
                .items_start()
                .justify_start()
                .text_xs()
                .tooltip(label.clone())
                .accessibility_label(format!("{label}: {message}"))
                .child(Icon::new(icon).size_3().mt_0p5().text_color(color))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .text_left()
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap_x_2()
                                .gap_y_1()
                                .child(
                                    div()
                                        .min_w_0()
                                        .max_w_full()
                                        .truncate()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .child(label),
                                )
                                .child(
                                    div()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(diagnostic.code.to_string()),
                                )
                                .when(diagnostic.blocking, |view| {
                                    view.child(
                                        div()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(crate::text::t("panel.problemsBlocking")),
                                    )
                                }),
                        )
                        .child(div().whitespace_normal().child(message)),
                )
                .on_click(cx.listener(move |view, _, _, cx| view.locate(&source, index, None, cx))),
        )
        .children(
            diagnostic
                .related
                .iter()
                .enumerate()
                .map(|(related, address)| {
                    let label = location::label(address, projection, None)
                        .unwrap_or_else(|| projection.graph_path.to_string());
                    let source = projection.clone();
                    Button::new(("problem-related", related))
                        .tooltip(label.clone())
                        .ghost()
                        .rounded_none()
                        .w_full()
                        .h_auto()
                        .min_h_0()
                        .pl_8()
                        .pr_3()
                        .py_1()
                        .justify_start()
                        .text_xs()
                        .text_color(cx.theme().primary)
                        .child(div().flex_1().min_w_0().truncate().text_left().child(
                            crate::text::format(
                                "panel.problemsRelatedLocation",
                                &[("location", label)],
                            ),
                        ))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.locate(&source, index, Some(related), cx)
                        }))
                }),
        )
        .into_any_element()
}
